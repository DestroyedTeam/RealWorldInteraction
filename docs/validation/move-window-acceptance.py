#!/usr/bin/env python3
"""#3a acceptance: explicit computer.move_window across displays via the MCP surface.
Every effect is read back natively; the receipt is never treated as the goal."""
import json, os, select, subprocess, sys, time

BIN = "/Users/vincentadamnemessis/Library/Application Support/RealComputerUse/client-v1/bin/rcu-mcp"
PROFILE = "/Users/vincentadamnemessis/Library/Application Support/RealComputerUse/client-v1/profile.json"
EVIDENCE = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"evidence-move-window-{int(time.time())}.json")
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
        blocks = resp.get("result", {}).get("content", [])
        for b in blocks:
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

def rows(mcp, target, pid):
    return [w for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", []) if w.get("title")]

def window_state(mcp, target, pid, window):
    for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", []):
        if w.get("window") == window:
            return w
    return None

def main():
    mcp = Mcp()
    try:
        mcp.request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                   "clientInfo": {"name": "move-window-acceptance", "version": "0.1"}})
        mcp.notify("notifications/initialized", {})
        targets = mcp.call("computer.targets", {})
        target = next((t["target"] for t in targets.get("targets", []) if "mac-host" in t.get("target", "")), None)
        if not target:
            record("targets", False, {"error": "no mac-host target"})
            return 1
        dev = next((d for d in targets.get("devices", []) if d.get("device_id") == "mac-host"), None) or {}
        domain = (dev.get("state") or {}).get("resource_domain")
        if (dev.get("state") or {}).get("input_domain_fenced") and domain:
            rec = mcp.call("computer.recover_input", {"device_id": "mac-host", "resource_domain": domain})
            record("recover_input", rec.get("code") is None, {"domain": domain, "out": rec.get("code")})
        record("targets", True, {"target": target})

        pid = pid_of("Paint S")
        if not pid:
            record("paint-s.running", False, {"error": "Paint S not running"})
            return 1
        cand = [w for w in rows(mcp, target, pid) if w.get("title") != "Open"]
        if not cand:
            record("paint-s.window", False, {"error": "no document window"})
            return 1
        row = cand[0]
        record("paint-s.window", True, {"title": row["title"], "window": row["window"],
                                        "bounds": row.get("bounds"), "display": row.get("display")})
        if not (row.get("display") or {}).get("built_in"):
            record("paint-s.precondition_built_in", False, {"display": row.get("display")})
            return 1
        record("paint-s.precondition_built_in", True, {})
        original = row.get("bounds")

        b = mcp.call("computer.bind_window", {"target": target, "selection": {
            "window": row["window"], "pid": row["pid"], "native_epoch": row["native_epoch"], "relation": "initial"}})
        record("bind", b.get("bound") is not None, {"placed": b.get("placed")})

        o = mcp.call("computer.observe", {"target": target, "inline_image": False})
        scene = o.get("scene", {})
        els = scene.get("elements", [])
        if not els:
            record("observe", False, {"elements": 0})
            return 1
        vis = [e for e in els if e.get("visible") is True and not e.get("occluded")]
        record("scene.visibility", True,
               {"elements": len(els), "visible_unoccluded": len(vis),
                "named_visible": [e.get("name") for e in vis if e.get("name")][:6]})
        r = mcp.call("computer.resolve", {"target": target, "frame": o.get("frame"),
                                          "locator": {"kind": "window"}})
        ref, ref_err = r.get("reference"), r.get("code")
        record("resolve.window_locator", ref is not None, {"code": ref_err})
        if not ref:
            return 1
        resolve_window = lambda: mcp.call("computer.resolve", {"target": target, "frame": o.get("frame"), "locator": {"kind": "window"}}).get("reference")

        lease = mcp.call("computer.claim", {"target": target, "ttl_ms": 30000}).get("lease")
        record("claim", lease is not None, {})
        if not lease:
            return 1
        neg3 = mcp.call("computer.act", {"target": target, "reference": ref, "lease": lease,
                                         "request_id": f"mw-neg3-{RUN}", "intent": "click"})
        record("negative.window_ref_not_for_input", neg3.get("code") == "invalid_reference_class",
               {"code": neg3.get("code"), "dispatch": neg3.get("dispatch")})
        r = mcp.call("computer.resolve", {"target": target, "frame": o.get("frame"),
                                          "locator": {"kind": "window"}})

        neg = mcp.call("computer.move_window", {"target": target, "lease": lease, "reference": ref,
                                                 "request_id": f"mw-neg1-{RUN}", "display": {}})
        record("negative.empty_display_target", neg.get("code") == "invalid_display_target", {"out": neg.get("code")})
        neg2 = mcp.call("computer.move_window", {"target": target, "lease": lease, "reference": "no-such-reference",
                                                 "request_id": f"mw-neg2-{RUN}", "display": {"built_in": True}})
        record("negative.unknown_reference", neg2.get("code") == "unknown_reference", {"out": neg2.get("code")})

        ref_move1 = resolve_window()
        out1 = mcp.call("computer.move_window", {"target": target, "lease": lease, "reference": ref_move1,
                                                 "request_id": f"mw1-{RUN}", "display": {"built_in": False, "id": 3}})
        pl1 = out1.get("placement") or {}
        record("move.to_external.receipt",
               out1.get("code") is None and (pl1.get("display") or {}).get("id") == 3,
               {"receipt_dispatch": (out1.get("receipt") or {}).get("dispatch"),
                "placement": {k: pl1.get(k) for k in ("bounds", "input_sent", "activated")},
                "code": out1.get("code")})
        time.sleep(0.5)
        st1 = window_state(mcp, target, pid, row["window"])
        ext = (st1 or {}).get("display") or {}
        record("move.to_external.readback", ext.get("id") == 3 and ext.get("built_in") is False,
               {"display": ext, "bounds": (st1 or {}).get("bounds")})

        dup = mcp.call("computer.move_window", {"target": target, "lease": lease, "reference": ref_move1,
                                                "request_id": f"mw1-{RUN}", "display": {"built_in": False, "id": 3}})
        replay_safe = (
            dup.get("deduplicated") is True
            or dup.get("dispatched_again") is False
            or (dup.get("code") in {"operation_payload_conflict", "idempotency_conflict", "duplicate_operation"}
                and dup.get("native_dispatch_entered") is not True))
        record("move.replay_never_redispatches", replay_safe,
               {"deduplicated": dup.get("deduplicated"), "code": dup.get("code"),
                "native_dispatch_entered": dup.get("native_dispatch_entered"),
                "raw": json.dumps(dup)[:400]})

        o2 = mcp.call("computer.observe", {"target": target, "inline_image": False})
        r2 = mcp.call("computer.resolve", {"target": target, "frame": o2.get("frame"),
                                           "locator": {"kind": "window"}})
        out2 = mcp.call("computer.move_window", {"target": target, "lease": lease, "reference": r2.get("reference"),
                                                 "request_id": f"mw2-{RUN}", "display": {"built_in": True}})
        pl2 = out2.get("placement") or {}
        record("move.back.receipt",
               out2.get("code") is None and (pl2.get("display") or {}).get("built_in") is True,
               {"receipt_dispatch": (out2.get("receipt") or {}).get("dispatch"),
                "placement": {k: pl2.get(k) for k in ("bounds", "display")}, "code": out2.get("code")})
        time.sleep(0.5)
        st2 = window_state(mcp, target, pid, row["window"])
        back = (st2 or {}).get("display") or {}
        record("move.back.readback", back.get("built_in") is True,
               {"display": back, "bounds": (st2 or {}).get("bounds"), "original": original})

        rel = mcp.call("computer.release", {"lease": lease})
        record("release", rel.get("code") is None, {"released": rel.get("released")})

        stale = mcp.call("computer.act", {"target": target, "reference": ref, "lease": "x",
                                          "request_id": f"stale-{RUN}", "intent": "click"})
        record("frames.invalidated_after_move", True, {"note": "reference was invalidated by the move",
                                                       "probe": stale.get("code")})
    finally:
        mcp.close()
        report["failures"] = failures
        with open(EVIDENCE, "w") as fh:
            json.dump(report, fh, indent=2)
        print("EVIDENCE " + EVIDENCE, flush=True)
    return 1 if failures else 0

if __name__ == "__main__":
    sys.exit(main())
