#!/usr/bin/env python3
"""#3b acceptance: popup delivery through the MCP surface.
Observe must list the bound process's windows (including non-foreground
dialogs/sheets/panels) without guessing a purpose; opening the Paint S image
panel must deliver the new popup, and it must be closable."""
import json, os, select, subprocess, sys, time

BIN = "/Users/vincentadamnemessis/Library/Application Support/RealComputerUse/client-v1/bin/rcu-mcp"
PROFILE = "/Users/vincentadamnemessis/Library/Application Support/RealComputerUse/client-v1/profile.json"
EVIDENCE = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"evidence-popup-{int(time.time())}.json")
RUN = str(int(time.time()))
report = {"stages": {}}
failures = []

def record(stage, ok, detail):
    report["stages"][stage] = {"ok": bool(ok), "detail": detail}
    if not ok:
        failures.append(stage)
    print(("PASS " if ok else "FAIL ") + stage + " :: " + json.dumps(detail)[:320], flush=True)

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

def pid_of(name):
    out = subprocess.run(["pgrep", "-x", name], capture_output=True, text=True).stdout.split()
    return int(out[0]) if out else None

def discover(mcp, target, pid):
    return [w for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", []) if w.get("title")]

def main():
    mcp = Mcp()
    try:
        mcp.request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                   "clientInfo": {"name": "popup-acceptance", "version": "0.1"}})
        mcp.notify("notifications/initialized", {})
        targets = mcp.call("computer.targets", {})
        target = next((t["target"] for t in targets.get("targets", []) if "mac-host" in t.get("target", "")), None)
        if not target:
            record("targets", False, {"error": "no mac-host target"})
            return 1
        dev = next((d for d in targets.get("devices", []) if d.get("device_id") == "mac-host"), None) or {}
        state = dev.get("state") or {}
        if state.get("input_domain_fenced") and state.get("resource_domain"):
            mcp.call("computer.recover_input", {"device_id": "mac-host", "resource_domain": state["resource_domain"]})
        record("targets", True, {"target": target})

        pid = pid_of("Paint S")
        if not pid:
            record("paint-s.running", False, {"error": "Paint S not running"})
            return 1
        candidates = [w for w in discover(mcp, target, pid) if w.get("title") != "Open"]
        candidates.sort(key=lambda w: (0 if "canvas" in (w.get("title") or "").lower() else 1))
        row, o1, pops1, el = None, None, [], None
        for candidate in candidates[:3]:
            b = mcp.call("computer.bind_window", {"target": target, "selection": {
                "window": candidate["window"], "pid": candidate["pid"], "native_epoch": candidate["native_epoch"], "relation": "initial"}})
            if b.get("bound") is None:
                continue
            o = mcp.call("computer.observe", {"target": target, "inline_image": False})
            rows = o.get("popups") or []
            hit = next((e for e in (o.get("scene") or {}).get("elements", [])
                        if e.get("name") == "Add Image" and e.get("visible") is True and not e.get("occluded")), None)
            if hit is not None:
                row, o1, pops1, el = candidate, o, rows, hit
                break
        if row is None:
            record("paint-s.window_with_toolbar", False,
                   {"candidates": [w.get("title") for w in candidates[:3]]})
            return 1
        record("paint-s.window_with_toolbar", True,
               {"window": row["window"], "title": row["title"], "display": (row.get("display") or {}).get("built_in")})
        baseline = {p.get("window") for p in pops1 if p.get("window")}
        record("observe.popups_delivered", len(pops1) > 0,
               {"rows": [{k: p.get(k) for k in ("window", "kind", "subrole", "title")} for p in pops1][:6]})
        kinds = {p.get("kind") for p in pops1}
        record("observe.kinds_coarse_only", kinds <= {"standard", "dialog", "sheet", "panel", "alert", "other", "unknown"},
               {"kinds": sorted(kinds)})
        record("observe.bound_window_listed", row["window"] in baseline, {"bound": row["window"], "listed": sorted(baseline)})
        record("paint-s.add_image_visible", True, {"element": el.get("id")})
        lease = mcp.call("computer.claim", {"target": target, "ttl_ms": 30000}).get("lease")
        r = mcp.call("computer.resolve", {"target": target, "frame": o1.get("frame"),
                                          "locator": {"kind": "element", "id": el["id"]}})
        act = mcp.call("computer.act", {"target": target, "reference": r.get("reference"), "lease": lease,
                                        "request_id": f"popup-open-{RUN}", "intent": "click"})
        record("act.add_image", act.get("dispatch") == "returned" or act.get("code") is None,
               {"dispatch": act.get("dispatch"), "code": act.get("code")})

        opened, rows2 = None, []
        for _ in range(20):
            time.sleep(0.3)
            o2 = mcp.call("computer.observe", {"target": target, "inline_image": False})
            rows2 = o2.get("popups") or []
            opened = next((p for p in rows2 if p.get("window") and p.get("window") not in baseline), None)
            if opened is None:
                opened = next((p for p in rows2 if (p.get("title") or "") == "Open"), None)
            if opened:
                break
        record("popup.delivered", opened is not None,
               {"row": opened and {k: opened.get(k) for k in ("window", "kind", "subrole", "title", "bounds")},
                "rows": [{k: p.get(k) for k in ("window", "kind", "title")} for p in rows2][:6]})

        panel = next((w for w in discover(mcp, target, pid) if w.get("title") == "Open"), None)
        if panel is None:
            record("popup.discoverable", False, {"error": "panel not in discover"})
            return 1
        record("popup.discoverable", True, {"window": panel["window"]})
        bound = mcp.call("computer.bind_window", {"target": target, "selection": {
            "window": panel["window"], "pid": panel["pid"], "native_epoch": panel["native_epoch"], "relation": "owned_popup"}})
        if bound.get("bound") is None:
            bound = mcp.call("computer.bind_window", {"target": target, "selection": {
                "window": panel["window"], "pid": panel["pid"], "native_epoch": panel["native_epoch"], "relation": "initial"}})
        record("popup.bound", bound.get("bound") is not None, {"relation_receipt": "owned_popup_or_initial"})
        o3 = mcp.call("computer.observe", {"target": target, "inline_image": False})
        cancel = next((e for e in (o3.get("scene") or {}).get("elements", []) if e.get("name") == "Cancel"), None)
        if cancel is None:
            record("popup.cancel_visible", False, {"named": [e.get("name") for e in (o3.get("scene") or {}).get("elements", []) if e.get("name")][:8]})
        else:
            record("popup.cancel_visible", True, {})
            r3 = mcp.call("computer.resolve", {"target": target, "frame": o3.get("frame"),
                                               "locator": {"kind": "element", "id": cancel["id"]}})
            act3 = mcp.call("computer.act", {"target": target, "reference": r3.get("reference"), "lease": lease,
                                             "request_id": f"popup-cancel-{RUN}", "intent": "click"})
            gone = False
            for _ in range(20):
                time.sleep(0.3)
                gone = not any(w.get("title") == "Open" for w in discover(mcp, target, pid))
                if gone:
                    break
            record("popup.closed", gone, {"dispatch": act3.get("dispatch"), "code": act3.get("code")})
        mcp.call("computer.release", {"lease": lease})
        record("release", True, {})
    finally:
        mcp.close()
        report["failures"] = failures
        with open(EVIDENCE, "w") as fh:
            json.dump(report, fh, indent=2)
        print("EVIDENCE " + EVIDENCE, flush=True)
    return 1 if failures else 0

if __name__ == "__main__":
    sys.exit(main())
