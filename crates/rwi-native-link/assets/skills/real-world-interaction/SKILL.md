---
name: real-world-interaction
description: Use the Real World Interaction native MCP tools to observe and operate an explicitly allocated Windows target through the authenticated Rust relay. Use for native window inspection, grounded mouse and keyboard actions, and independent result verification; not for shell-based automation or uncontrolled desktop discovery.
---

# Real World Interaction native MCP

## Entry and scope

Discover and call the plugin's `computer.*` tools directly. Do not replace them
with Python, PowerShell, AppleScript, a shell-driven input script, or the old
CrossMacro bridge. If tools are absent, report native-entry discovery failure;
an SDK client or a screenshot from another tool is not native plugin acceptance.
This plugin does not start/approve a native-link task. The controller must have
prepared its authorized model stage before connecting. Do not retry a lost link.

If `computer.targets` reports `mode: diagnostic_only` or
`code: native_session_unavailable`, the MCP entry is loaded but its native task is
not ready. These are recorded tool schemas, not a live Windows inventory. Report
the task ID and startup error; do not execute cleanup, reset HOLD, or disable the
MCP entry. A new Codex session does not renew task scope or expiry. A Notepad test
grant cannot be reused to clean processes in Task Manager.

## Fast bounded workflow

1. Read `computer.targets` once. Match the allocated host/guest/session/desktop;
   tools being listed is not target readiness or an input grant.
2. Observe the bound window. Read `observation_view`, window binding, coverage,
   exact element IDs and role/focus candidates. Request an image when it helps;
   use `inline_image: false` for a semantic-only check instead of needless images.
3. Decide the next single action **before claiming a lease**. Then claim just
   before `computer.act_checked`, dispatch once, and release promptly. Independent
   verification uses `computer.verify`; an input acknowledgement is not success.
   Only attach `request_id` to verification when that exact action was dispatched.
4. Never batch dependent destructive actions blindly. Re-observe after a real
   state transition. Parallel read-only analysis is possible, but one Windows
   desktop still has one input/focus stream; multiple Task Managers do not make
   concurrent input safe or independent.

## Interpret the response correctly

- `checked_context_available` is only a context prerequisite, not permission,
  freshness, element eligibility or an acquired lease. `action_authorized: false`
  means this observation itself grants no action. `mutation_ready` alone is not
  enough: null document context or incomplete coverage can prevent checked input.
- Preserve exact IDs. A named parent and an Edit child can share a label. UIA can
  report multiple focus candidates; inspect role and occlusion, not just name.
- `presentation.full_response` identifies exact locally archived RPC bytes and
  their SHA. Default view omits hidden/occluded nonfocused elements and repeated
  diagnostics; it does not change the server snapshot or execution guards.
- `native preemption unavailable` describes cooperative cancellation, NOT whether
  the VM is visible on the Mac. Pending cancel/resources must reach terminal and
  released before another operation. Do not poll successful terminal operations.
- A lease expiry, changed precondition, missing context, privilege boundary or
  unknown action request are different failures. Report the actual code. Never
  refresh-loop, reconnect/replay, or downgrade to legacy/pixel input after failure.
- Ordinary/Admin deployment requires matching actual token/session bindings and
  explicit scope. Seeing an admin window image does not prove UIA/input access.

## Evidence and reporting

Report task completion only after native state verification. Keep entry loading,
tool discovery, TLS transport, native execution and goal verification separate.
Measure call count, RPC text bytes and wall time; do not infer model comprehension
or speed from payload reduction alone. Report only meaningful milestones.
