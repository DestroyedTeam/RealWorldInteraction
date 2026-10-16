# Native MCP startup discovery — 2026-09-11

**CrossMacro cleanup has not been performed.** This change fixes an entry/diagnostic
failure, not task scope, native readiness, or process-termination acceptance.

## Confirmed root cause

The real user project was trusted and its sole native MCP entry enabled, but that
entry still referred to `native-codex-win-20260911-admin5`. The Windows controller
returned `agent_restart_unfinished_task` HOLD. Its immutable approval was a Notepad
file task, not authorization for a Task Manager process-cleanup workflow.

A new Codex task does not replace the native approval, its expiry or its journal.
The previous relay attempted native Model admission before consuming MCP stdin;
on rejection it exited without an initialize response. Consequently the user task
saw no `computer.*` tools even though configuration and Skill discovery existed.

## Implemented, without input fallback

- Extract the existing model-attachment sequence in a behavior-preserving refactor.
- Mark failures at that precise **pre-MCP-input** boundary with a typed error.
- On that boundary only, keep a Rust/rmcp diagnostic server available. Its 19 tool
  definitions are the recorded worker schema, checked against the canonical
  digest. The server advertises **DIAGNOSTIC ONLY**, not native worker readiness.
- Every business call returns `isError=true`, `native_session_unavailable`,
  `action_authorized=false`, `operation_dispatched=false` and `goal_verified=false`.
  It never reconnects, starts a replacement task, sends input or clears HOLD.
- Post-admission disconnect/uncertainty still exits; it cannot switch into a new
  session or replay consumed MCP requests. Existing line and total-request bounds
  are preserved before the diagnostic SDK decodes input.
- The agent source now returns a task/nonce/role-bound admission rejection to an
  authenticated matching model peer. This source change is offline-tested; the
  currently running Windows build predates that richer rejection response.
- Keep the project native entry `enabled=true`; update only its pinned relay
  command/entry and set `required=true`. Leave global startup policy unchanged.

Codex's documented optional-server grace is 1 second by default, whereas required
servers use their startup timeout. Setting this specific RCU entry required avoids
silently omitting it from the initial catalog during slower initialization.
[Official MCP configuration reference](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).

## Actual Codex Host verification

Using the actual desktop executable (0.153.4), real user HOME and trusted project:

- A per-test ephemeral thread disabled unrelated/legacy configured MCPs and
  plugins only in its in-memory overrides; their saved configuration was untouched.
- `mcpServerStatus/list` returned 19 tools for `real_computer_use_native_link`.
- An actual `mcpServer/tool/call` to `computer.targets` returned the structured
  diagnostic-only error with no native operation or enumeration claimed.
- No model inference was used. This is actual Host/tool integration evidence,
  not proof that the currently running conversation hot-reloaded its tool list.
- The Host exited with code 0 without forced termination.
- Control Center independently reported that its own model-visible tool table
  now contains all 19 native tools, not merely the isolated Host catalog. It
  inspected metadata only and did not open a competing native Model session.

Evidence: `work/windows-native-plugin-20260911/real-diagnostic-host-r2/`, including
`response-0005.json` (catalog) and `response-0006.json` (actual tool response).
Raw configuration responses stay private and must not be committed or published.

Offline process regressions cover absent acknowledgements, bound HOLD refusals,
wrong-nonce refusals, no automatic reconnect, all tools failing closed, and no
false ModelEnd success. The existing post-admission disconnect tests also pass.
Held-session tests now distinguish clean diagnostic-server EOF from a committed
native ModelEnd, and still check unchanged application bytes and no replay.

## Regression results

On the current workspace: default offline tests **231 passed**; all-features
offline tests **274 passed, 2 tests ignored**; formatting, host
Clippy and Windows ARM64 Clippy passed. These are separate from live Windows
acceptance. The first all-features checks exposed old assertions that assumed
held entries must exit without MCP diagnostics; the updated assertions explicitly
check no dispatch, no replay and no ModelEnd. A concurrent default/all-features
build also overwrote a shared test executable; the final run was serialized and
passed.

## Still unfinished

The native task system currently requires a file manifest and a document-value /
file-oracle completion contract. A general process-cleanup task needs its own
scope and native absence oracle; reusing the Notepad grant or adding a dummy file
would not establish CrossMacro cleanup. No process was terminated by this fix,
no shell/old CrossMacro bridge replaced native input, and no HOLD was reset.
