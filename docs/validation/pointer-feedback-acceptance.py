#!/usr/bin/env python3
"""#4 acceptance: visual-only pointer overlay through the MCP surface.
Shows/hides the marker, reads it back natively (discover + screencapture -l),
asserts pixel evidence, and proves acts update it automatically."""
import json, os, select, subprocess, sys, time

BIN = "/Users/vincentadamnemessis/Library/Application Support/RealComputerUse/client-v1/bin/rcu-mcp"
PROFILE = "/Users/vincentadamnemessis/Library/Application Support/RealComputerUse/client-v1/profile.json"
EVIDENCE = os.path.join(os.path.dirname(os.path.abspath(__file__)), f"evidence-pointer-feedback-{int(time.time())}.json")
RUN = str(int(time.time()))
POINT = [400.0, 1300.0]
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

def discover_all(mcp, target):
    rows, cursor = [], None
    for _ in range(6):
        args = {"target": target} if cursor is None else {"target": target, "cursor": cursor}
        page = mcp.call("computer.discover", args)
        rows.extend(page.get("windows", []))
        cursor = page.get("next_cursor") or page.get("cursor")
        if not cursor or not page.get("complete", False):
            break
    return rows

def overlay_rows(mcp, target, pid=None):
    return [w for w in discover_all(mcp, target)
            if (pid is None or w.get("pid") == pid) and w.get("title") == "RCU AI Cursor"]

def main():
    mcp = Mcp()
    try:
        mcp.request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                   "clientInfo": {"name": "pointer-feedback-acceptance", "version": "0.1"}})
        mcp.notify("notifications/initialized", {})
        targets = mcp.call("computer.targets", {})
        target = next((t["target"] for t in targets.get("targets", []) if "mac-host" in t.get("target", "")), None)
        if not target:
            record("targets", False, {"error": "no mac-host target"})
            return 1
        record("targets", True, {"target": target})

        shown = mcp.call("computer.pointer_feedback", {"target": target, "visible": True, "point": POINT})
        record("feedback.show_receipt",
               shown.get("visible") is True and shown.get("created") is True and shown.get("point") == POINT,
               {"status": {k: shown.get(k) for k in ("installed", "created", "visible", "point", "last_error")}})
        time.sleep(0.6)
        rows = overlay_rows(mcp, target)
        row = rows[0] if rows else None
        expected = [POINT[0] - 13.0, POINT[1] - 13.0, 26.0, 26.0]
        near = row is not None and all(abs(row["bounds"][i] - expected[i]) <= 2.0 for i in range(2)) \
            and abs(row["bounds"][2] - 26.0) <= 2.0 and abs(row["bounds"][3] - 26.0) <= 2.0
        record("feedback.native_window_readback", near,
               {"row": row and {"window": row["window"], "bounds": row["bounds"], "display": row.get("display")}, "expected": expected})
        shot = "/tmp/rcu-overlay-shot.png"
        if os.path.exists(shot):
            os.remove(shot)
        if row:
            cgid = int(row["window"].split(":")[1])
            subprocess.run(["screencapture", "-x", "-o", "-l", str(cgid), shot], check=False, timeout=20)
        pixel_ok = os.path.exists(shot) and os.path.getsize(shot) > 400
        record("feedback.pixel_evidence", bool(pixel_ok),
               {"png": os.path.exists(shot), "bytes": os.path.getsize(shot) if os.path.exists(shot) else 0,
                "note": "screencapture -l of the overlay window; a fully transparent frame compresses far below this"})

        hidden = mcp.call("computer.pointer_feedback", {"target": target, "visible": False})
        record("feedback.hide_receipt", hidden.get("visible") is False, {"status": hidden.get("visible")})
        time.sleep(0.6)
        gone = not overlay_rows(mcp, target, None)
        record("feedback.hidden_native_readback", gone, {"overlay_windows": len(overlay_rows(mcp, target, None))})

        pid = int(subprocess.run(["pgrep", "-x", "Paint S"], capture_output=True, text=True).stdout.split()[0])
        windows = [w for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", []) if w.get("title")]
        panel = next((w for w in windows if w.get("title") == "Open"), None)
        lease = mcp.call("computer.claim", {"target": target, "ttl_ms": 30000}).get("lease")

        def bind(row):
            return mcp.call("computer.bind_window", {"target": target, "selection": {
                "window": row["window"], "pid": row["pid"], "native_epoch": row["native_epoch"], "relation": "initial"}})

        def tap_cancel():
            o = mcp.call("computer.observe", {"target": target, "inline_image": False})
            cancel = next((e for e in (o.get("scene") or {}).get("elements", [])
                           if e.get("name") == "Cancel" and e.get("enabled") is not False), None)
            if cancel is None:
                return {"code": "cancel_missing"}
            r = mcp.call("computer.resolve", {"target": target, "frame": o.get("frame"),
                                              "locator": {"kind": "element", "id": cancel["id"]}})
            return mcp.call("computer.act", {"target": target, "reference": r.get("reference"), "lease": lease,
                                             "request_id": f"feedback-cancel-{RUN}", "intent": "click"})

        attempts, after = [], []
        if panel is not None:
            # A leftover modal disables the toolbar; closing it is a real act and
            # doubles as the auto-show proof. The product refusing the disabled
            # toolbar is correct and already covered by earlier negatives.
            record("feedback.leftover_panel", True, {"window": panel["window"]})
            b = bind(panel)
            if b.get("bound") is None:
                record("feedback.auto_show_on_act", False, {"error": "panel bind failed", "code": b.get("code")})
            else:
                act = tap_cancel()
                attempts.append(act.get("code") or act.get("dispatch"))
                time.sleep(0.8)
                after = overlay_rows(mcp, target, None)
                closed = not any(w.get("title") == "Open" for w in mcp.call("computer.discover", {"target": target, "pid": pid}).get("windows", []))
                record("feedback.panel_closed_by_act", closed, {"dispatch": act.get("dispatch"), "code": act.get("code")})
        else:
            bound = next((w for w in windows if w.get("title") != "Open"), None)
            b = bind(bound) if bound else {"bound": None}
            if b.get("bound") is None:
                record("feedback.auto_show_on_act", False, {"error": "no bindable Paint S window"})
            else:
                for attempt in range(3):
                    o = mcp.call("computer.observe", {"target": target, "inline_image": False})
                    el = next((e for e in (o.get("scene") or {}).get("elements", [])
                               if e.get("name") == "Add Image" and e.get("visible") is True
                               and not e.get("occluded") and e.get("enabled") is not False), None)
                    if el is None:
                        attempts.append("add_image_unavailable")
                        time.sleep(0.4)
                        continue
                    r = mcp.call("computer.resolve", {"target": target, "frame": o.get("frame"),
                                                      "locator": {"kind": "element", "id": el["id"]}})
                    act = mcp.call("computer.act", {"target": target, "reference": r.get("reference"), "lease": lease,
                                                    "request_id": f"feedback-act-{RUN}-{attempt}", "intent": "click"})
                    attempts.append(act.get("code") or act.get("dispatch"))
                    if act.get("code") is None:
                        time.sleep(0.8)
                        after = overlay_rows(mcp, target, None)
                        break
                    time.sleep(0.4)
        record("feedback.auto_show_on_act", len(after) > 0,
               {"attempts": attempts, "overlay": after[0]["bounds"] if after else None})
        mcp.call("computer.release", {"lease": lease})
        mcp.call("computer.pointer_feedback", {"target": target, "visible": False})
    finally:
        mcp.close()
        report["failures"] = failures
        with open(EVIDENCE, "w") as fh:
            json.dump(report, fh, indent=2)
        print("EVIDENCE " + EVIDENCE, flush=True)
    return 1 if failures else 0

if __name__ == "__main__":
    sys.exit(main())
