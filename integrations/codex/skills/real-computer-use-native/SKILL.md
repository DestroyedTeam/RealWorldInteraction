---
name: real-computer-use-native
description: Use Real Computer Use native MCP capabilities to inspect and operate user-authorized devices, windows, mouse and keyboard through independent Rust device agents. Not a shell automation bridge.
---

# Real Computer Use

Use the available `computer.*` MCP tools directly. The MCP adapter has a stable
tool catalog; each paired device runs its own headless agent. Neither service
requires a business task, a controller stage, a test application, or a prior
business result. A window closing does not invalidate the installation.

## Preserve the requested Host entry

For native MCP/plugin/skill acceptance, GUI actions must originate from the
Host's currently loaded RCU tools. Do not substitute a shell-launched Python,
Node, stdio or HTTP test client, even when it talks to the real Rust MCP server.
Such clients can test transport/integration when requested, but cannot prove the
Host's native entry works. If the loaded entry cannot carry an advertised action,
report that adapter/catalog mismatch and fix the entry rather than silently
changing the GUI execution path. Complementary terminal work does not waive this
boundary. Keep GUI drawing, screenshot capture, application storage, downloaded
file export and file reopening as separate acceptance nodes.

## Coordinate complementary channels

- GUI and terminal are parallel, complementary peers, not replacements or a
  preferred/fallback hierarchy. Within one authorized flow, either may execute,
  verify the other's effect, supply evidence or inform the next iteration.
  Combine native GUI and existing terminal/API tools when useful. Use local terminal tools on the local
  machine; use an already authorized SSH/remote connector for another device.
  A local command is not a guest command. Confirm the remote host identity and
  effective user; GUI and SSH may belong to different OS sessions.
- RCU does not currently provide an SSH tool. Do not invent one, install a remote
  shell service, or expand network access just because this skill is loaded.
- Respect explicitly requested GUI-only tasks and native MCP acceptance tests.
  Terminal work is not evidence that native input/capture works. A rejected GUI
  authorization is not permission to perform the same denied action via SSH.
  After terminal changes affect a GUI resource, refresh its native binding/state.

## Select the intended resource

- Read `computer.targets` for paired device availability, actual native identity,
  capabilities and authorized client targets. Respect the user's device/window
  allocation. Tool presence, `implemented`, `available`, and `authorized` are
  different facts; action-specific binding/focus/lease checks still apply.
- Discover and bind a fresh exact window when needed. Binding does not focus it.
  A returned device/context/window generation belongs only to that context.
  Pass `device_id` when cached artifact identifiers collide across devices.
- Observe before grounded input. `inline_image: false` omits the inline image;
  it does not guarantee that the current backend avoids capture internally.
  Obtain images/tiles through `computer.image`; view coordinates need
  `computer.resolve_view`, not original-image pixel interpretation.
- In compact-v1 responses, inspect `presentation.elements.next_offset` and
  native coverage separately. Use `computer.result` with the returned result_ref
  and `/scene/elements` array offset to read omitted elements without recapture.
  Other JSON pointers retrieve retained diagnostics. This adapter-local metadata
  reference can expire; it is never an execution identity or permission to replay.

## Persistent JS entry when advertised

Prefer `computer.js` for already-determined related calls. It provides `rcu`,
`state`, `emit(data)` and `emitImage(reply.media[i])` in an async cell body.
For example: `state.inventory = await rcu.targets(); emit(state.inventory.targets)`.
Persist selected windows with `state.win = rcu.window(exactReturnedTarget)`;
cell-local `let`/`const` do not persist. Use `rcu.call(tool, args)` for other
advertised tools; no import or hand-written transport is needed.

`state.win` supports `bind(selection)`, `observe(options)`, `elementsPage(offset)`
and `withInput(async ({act, observe}) => { ... })`. Await every operation.
`act({element, intent, value, requestId})` requires an observed exact element and
stable mutation ID; it invalidates old observations. Observe again before another
action. The helper releases its lease; the Rust supervisor also cleans up leases
created by the cell. No lease survives intentionally across model turns.

For model-facing observations prefer `const s = await state.win.snapshot()` and
`emit(s.data)`. This returns concise controls with frame-scoped refs such as `e3`,
roles and meaningful flags, not repeated native IDs and diagnostic trees.
Use `act({ref: 'e3', frame: s.data.frame, intent, requestId})`; both ref and exact
frame are required, and stale aliases are rejected. Native bindings remain intact.
`summary({offset,limit})` pages already-loaded controls without native calls;
`elementsPage` retrieves omitted original elements before summarizing them.
Coverage, blocked state and detail references remain visible; incomplete is not
evidence of absence. For pixels, use `snapshot({preview:true})`, which defaults
to a mapped 1024px long-edge preview. Emit `s.media` as images separately. Request
an original-coordinate crop or higher maxEdge only for detail that needs it.
Never change the user's display resolution or treat preview pixels as originals.

Native drag support does not attest visual Pointer feedback. A static cursor or
halo in one screenshot is not proof of continuous following or its latency.
Report missing feedback capabilities and unmeasured motion honestly; do not add
a marker to a screenshot and present it as device-side live feedback.

If the target advertises `pointer_drag.implemented`, `act` may take a typed
`intent: {drag:{waypoints:[{at_ms:0,offset:[0,0]}, ...]}}` instead of a string.
This is one device-paced left-button drag, not one MCP call per point. Select
the original ref/frame normally. Offsets are device physical desktop pixels
from the grounded reference, never preview/controller pixels. Times increase
strictly, 2..128 points, total 16..5000ms; stay in the same fixed-geometry native
window and selected element ancestry. Observe the effect afterward. Cancellation
or failure may leave a partial stroke; query the original operation, never replay
the whole drag. Nominal 16ms sampling is not proof of rendering FPS or latency.

The cell has a 30s default / 60s maximum execution deadline and a 64-call limit;
it is not a background task. Emit only the needed data, since calls are not
automatically echoed. Parent-owned `calls` retain operation references and detail
`result_ref`s. Cancellation/error/reset discard JS state, not native history;
query uncertain operations instead of repeating the cell. A returned `generation`
can guard subsequent calls against a reset, but is not a native identity.

`computer.js_reset` stops only this adapter's JS worker. Direct native MCP tools
remain independent. If JS is absent, use the existing native tools—not a fake JS
bridge. The worker has no Node/shell/network API; existing terminal tools remain
a separate complementary channel in the same task flow.

## Execution identity (device wire v2)

Native work returns a service-issued `operation_ref`. Pass it unchanged:
- Query execution: `computer.operation` with
  `{"selector":{"kind":"execution","operation_ref":<returned reference>}}`.
- Query a durable mutation, including after a lost response/restart:
  `{"selector":{"kind":"request","device_id":"paired device","request_id":"original request"}}`.
- Cancel: `computer.cancel` with `{"operation_ref":<returned reference>}`.

These are mutually exclusive selectors, not an auto lookup. Bare `operation_id`,
`monitor_id`, and `request_id` cannot cancel execution. Optional `correlation_id`
is a caller label only; it cannot query, cancel or route work. The legacy input
alias `operation_id` is also only a label, never an execution identity.

A fresh Host may query an exact still-live reference without first opening a
context. Agent restart, actual context closure or bounded terminal-history
retirement expires it. Never reconstruct a reference or replace an expired ref
with a similar label. Expiry is not evidence that native work stopped or that no
effect occurred; use the explicit durable request query where applicable.

Acceptance and cancellation are internal adapter/agent mechanics, not extra
model preparation tools or business stages. Host Stop before acceptance prevents
Execute; later Stop addresses the exact accepted instance. Cached artifact/resolve
reads are not native executions and do not receive execution references.
Adapter and agent must both support device wire v2; a version mismatch must not
be worked around by enabling the obsolete task-bound entry.

## Act and verify

Select the next action before claiming a short cooperative desktop input lease.
Do not hold a short lease across a model deliberation turn: prepare the action,
then claim and dispatch in the same bounded tool execution, leaving time for
native checks. Release before further planning; never renew by replaying input.
Prefer `computer.act_checked` with an exact observed element. Native-element
policy checks the selected window and element; document text is not a universal
precondition for every button. Keyboard input requires the exact focused,
enabled, non-sensitive native element and fresh state; text entry additionally
requires a writable element. Do not downgrade a rejected checked action to
unchecked/pixel input to bypass its failed preconditions.

Use a stable `request_id` per intended mutation. Keep its returned `operation_ref`
for execution status/cancellation. Dispatch once; independently read the expected native
predicate with `computer.verify`; release promptly. A dispatch acknowledgement or
a matched single predicate is not proof of the entire business task.

One desktop has one input/focus stream. Multiple windows do not create independent
mouse/keyboards. Independent devices and read-only reasoning can be concurrent.

For concise execution, inspect only needed tool metadata rather than printing
the complete tool directory. Emit one result representation (`structuredContent`
when present, otherwise text content), not the whole duplicated MCP wrapper;
emit image blocks separately. Keep errors, execution references and uncertainty
visible. Group already-determined calls and their resulting observation when
possible, but stop on changed state or failure before choosing further actions.

## Errors, reconnection and cancellation

- An unavailable device does not disable other devices or tool discovery.
  Reconnection and fresh observation are allowed. Reconnecting must **never
  replay an uncertain input**; query the original operation instead.
- Old context/window/frame references require fresh discovery after restart or
  replacement. Do not reuse handles from a previous client instance.
- `computer.cancel` and Host Stop request cooperative cancellation. Inspect
  `terminal`, `native_call_pending` and `resources_released`; acknowledgement
  does not prove a native provider stopped. Do not poll already terminal success.
- `input_domain_fenced` affects that native resource domain, not the service.
  `computer.recover_input`, when authorized, performs explicit independent idle
  input-resource checks. It retains uncertain action history, does not replay an
  action and does not establish its business effect.
- If tools are absent, report native-entry discovery failure. If descriptions say
  `DIAGNOSTIC ONLY`, report that the Host loaded the obsolete task-bound entry,
  not that a business task must be prepared to unlock the new service. Do not
  disable the MCP entry, fabricate success, or substitute old CrossMacro,
  Python/PowerShell/AppleScript input, or another screenshot tool as acceptance.

Report installation, discovery, authenticated transport, native execution,
independent effect verification and business completion separately. Unsupported
platform capabilities remain unsupported until actually implemented and tested.
