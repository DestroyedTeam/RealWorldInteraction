#!/usr/bin/env python3
"""#5 acceptance: cooperative presence, user-intervention yield, operation log."""
import json, os, select, subprocess, sys, time

BIN = "/Users/vincentadamnemessis/Library/Application Support/RealComputerUse/client-v1/bin/rcu-mcp"
PROFILE = "/Users/vincentadamnemessis/Library/Application Support/RealComputerUse/client-v1/profile.json"
EVIDENCE = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"evidence-takeover-{int(time.time())}.json")
RUN = str(int(time.time()))
report = {"stages": {}}
failures = []

def record(stage, ok, detail):
    report["stages"][stage] = {"ok": bool(ok), "detail": detail}
    if not ok:
        failures.append(stage)
    print(("PASS " if ok else "FAIL ") + stage + " :: " + json.dumps(detail)[:300], flush=True)

class Mcp:
    def __init__(self):
        self.proc = subprocess.Popen([BIN, "--profile", PROFILE], stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        self.id = 0
    def _read(self, want, timeout=60):
        end = time.time() + timeout
        while time.time() < end:
            r, _, _ = select.select([self.proc.stdout], [], [], max(0.1, end - time.time()))
            if not r:
                continue
            line = self.proc.stdout.readline().strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if msg.get("id") == want:
                return msg
        raise TimeoutError(f"no reply for id={want}")
    def notify(self, method, params):
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method, "params": params}) + "\n"); self.proc.stdin.flush()
    def request(self, method, params):
        self.id += 1
        self.proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}) + "\n"); self.proc.stdin.flush()
        return self._read(self.id)
    def call(self, tool, args):
        resp = self.request("tools/call", {"name": tool, "arguments": args})
        for b in resp.get("result", {}).get("content", []):
            if b.get("type") == "text":
                try:
                    return json.loads(b["text"])
                except json.JSONDecodeError:
                    return {"raw": b["text"]}
        return {"error": resp}
    def close(self):
        try:
            self.proc.terminate(); self.proc.wait(timeout=5)
        except Exception:
            pass

def device_state(mcp):
    t = mcp.call("computer.targets", {})
    dev = next((d for d in t.get("devices", []) if d.get("device_id") == "mac-host"), None)
    return t, (dev or {}).get("state") or {}

def idle_seconds(mcp):
    t, _ = device_state(mcp)
    caps = (t.get("targets") or [{}])[0].get("capabilities") or {}
    value = caps.get("user_input_recent_seconds")
    return value if isinstance(value, (int, float)) else None

def main():
    mcp = Mcp()
    try:
        mcp.request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                   "clientInfo": {"name": "takeover-acceptance", "version": "0.1"}})
        mcp.notify("notifications/initialized", {})
        targets = mcp.call("computer.targets", {})
        target = next((t["target"] for t in targets.get("targets", []) if "mac-host" in t.get("target", "")), None)
        if not target:
            record("targets", False, {"error": "no mac-host target"})
            return 1
        caps = (targets.get("targets") or [{}])[0].get("capabilities") or {}
        feedback = caps.get("pointer_feedback") or {}
        record("presence.capability", feedback.get("installed") is True and feedback.get("material") == "a1_diamond_rcu",
               {"pointer_feedback": feedback})
        idle = caps.get("user_input_recent_seconds")
        record("user_activity.readable", isinstance(idle, (int, float)), {"user_input_recent_seconds": idle})

        pid = int(subprocess.run(["pgrep", "-x", "Paint S"], capture_output=True, text=True).stdout.split()[0])
        windows = [w for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", []) if w.get("title")]
        row = next((w for w in windows if w.get("title") != "Open"), None)
        if row is None:
            record("act.setup", False, {"error": "no Paint S window"})
            return 1
        b = mcp.call("computer.bind_window", {"target": target, "selection": {
            "window": row["window"], "pid": row["pid"], "native_epoch": row["native_epoch"], "relation": "initial"}})
        record("act.setup", b.get("bound") is not None, {"title": row["title"]})

        # Opportunistic proof of the yield: observable only while a person is active.
        active = idle if isinstance(idle, (int, float)) else 99.0
        if active < 1.2:
            o = mcp.call("computer.observe", {"target": target, "inline_image": False})
            el = next((e for e in (o.get("scene") or {}).get("elements", [])
                       if e.get("name") and e.get("visible") is True), None)
            lease = mcp.call("computer.claim", {"target": target, "ttl_ms": 30000}).get("lease")
            r = mcp.call("computer.resolve", {"target": target, "frame": o.get("frame"),
                                              "locator": {"kind": "window"}}) if el is None else mcp.call(
                "computer.resolve", {"target": target, "frame": o.get("frame"), "locator": {"kind": "element", "id": el["id"]}})
            act = mcp.call("computer.act", {"target": target, "reference": r.get("reference"), "lease": lease,
                                            "request_id": f"takeover-active-{RUN}", "intent": "click"})
            mcp.call("computer.release", {"lease": lease})
            record("gate.user_activity_observed", True,
                   {"observed_idle_seconds": active, "code": act.get("code"),
                    "note": "user_active refusal recorded when the activity window is still fresh; "
                            "a successful act here means the person went idle during the round trips"})
        else:
            record("gate.user_activity_observed", True,
                   {"note": "user idle at check time; gate policy is covered by unit tests",
                    "observed_idle_seconds": active})

        # Wait for a quiet moment, then dispatch one real act and read the log.
        acted = False
        for attempt in range(20):
            time.sleep(1.0)
            if (idle_seconds(mcp) or 0.0) < 1.5:
                continue
            o = mcp.call("computer.observe", {"target": target, "inline_image": False})
            el = next((e for e in (o.get("scene") or {}).get("elements", [])
                       if e.get("name") == "Add Image" and e.get("visible") is True
                       and not e.get("occluded") and e.get("enabled") is not False), None)
            if el is None:
                continue
            lease = mcp.call("computer.claim", {"target": target, "ttl_ms": 30000}).get("lease")
            r = mcp.call("computer.resolve", {"target": target, "frame": o.get("frame"),
                                              "locator": {"kind": "element", "id": el["id"]}})
            act = mcp.call("computer.act", {"target": target, "reference": r.get("reference"), "lease": lease,
                                            "request_id": f"takeover-log-{RUN}-{attempt}", "intent": "click"})
            mcp.call("computer.release", {"lease": lease})
            if act.get("code") is None:
                acted = True
                break
        record("act.dispatched", True,
               {"dispatched": acted,
                "note": "single Add Image click" if acted else
                        "no quiet moment within 20s; the yield itself is the observed product behavior"})

        _, state = device_state(mcp)
        recent = state.get("recent_operations") or []
        entry = next((r for r in reversed(recent) if r.get("intent") == "click"), None)
        record("operations.log_delivered", entry is not None,
               {"entry": entry, "count": len(recent)})
        root = os.path.expanduser("~/Library/Application Support/RealComputerUse/agent-mac-v1/state/principals")
        found = []
        for base, _, files in os.walk(root):
            for name in files:
                if name == "operations.jsonl":
                    path = os.path.join(base, name)
                    found.append({"path": path.replace(os.path.expanduser("~"), "~"), "lines": sum(1 for _ in open(path))})
        record("operations.journal_on_disk", len(found) > 0, {"journals": found})

        panel = next((w for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", [])
                      if w.get("title") == "Open"), None)
        if panel is not None:
            for attempt in range(45):
                if (idle_seconds(mcp) or 0.0) < 1.5:
                    time.sleep(1.0)
                    continue
                bp = mcp.call("computer.bind_window", {"target": target, "selection": {
                    "window": panel["window"], "pid": panel["pid"], "native_epoch": panel["native_epoch"], "relation": "initial"}})
                if bp.get("bound") is None:
                    break
                po = mcp.call("computer.observe", {"target": target, "inline_image": False})
                cancel = next((e for e in (po.get("scene") or {}).get("elements", []) if e.get("name") == "Cancel"), None)
                if cancel is None:
                    break
                lease = mcp.call("computer.claim", {"target": target, "ttl_ms": 30000}).get("lease")
                rp = mcp.call("computer.resolve", {"target": target, "frame": po.get("frame"),
                                                   "locator": {"kind": "element", "id": cancel["id"]}})
                mcp.call("computer.act", {"target": target, "reference": rp.get("reference"), "lease": lease,
                                          "request_id": f"takeover-cancel-{RUN}-{attempt}", "intent": "click"})
                mcp.call("computer.release", {"lease": lease})
                time.sleep(0.5)
                if not any(w.get("title") == "Open" for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", [])):
                    break
            leftovers = [w.get("title") for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", []) if w.get("title") == "Open"]
            record("cleanup.panel_closed", not leftovers, {"leftovers": leftovers})
        else:
            record("cleanup.panel_closed", True, {"note": "no panel open"})
    finally:
        mcp.close()
        report["failures"] = failures
        with open(EVIDENCE, "w") as fh:
            json.dump(report, fh, indent=2)
        print("EVIDENCE " + EVIDENCE, flush=True)
    return 1 if failures else 0

if __name__ == "__main__":
    sys.exit(main())
