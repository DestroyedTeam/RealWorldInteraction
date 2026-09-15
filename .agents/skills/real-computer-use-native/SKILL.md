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
- Discovery is paged (8 windows / 12 KiB). Prefer the known PID/title filter;
  `next_cursor` alone continues the cached catalog. An exact requested native
  tuple already present can be bound without reading unrelated pages. Incomplete
  pages cannot prove absence or uniqueness of a name-based search. Preserve
  `complete`/`next_cursor` when summarizing discovery, rather than hiding them.
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

Call this wrapper's `snapshot()` or `observe()` **before** `withInput`; its initial
readiness check needs an SDK-owned observation. Use that fresh frame for the first
action, then callback `input.snapshot()`/`input.observe()` before each next action.
Raw `rcu.call('computer.observe', ...)` does not populate a window wrapper's cache,
and raw input does not invalidate that cache. Either use the raw calls consistently
or refresh the wrapper before returning to SDK actions; do not mix stale state.

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

Window foreground/presentation is separate from keyboard-control focus.
`mutation_ready=true` is not proof that a keyboard target exists. If no exact
focused element is available, an explicit click on a grounded, non-sensitive
control followed by a fresh observation can establish it; never invent a focus
ref, repeat activation as a focus trick, or send keys through a rejected target.

Native drag support does not attest visual Pointer feedback. A static cursor or
halo in one screenshot is not proof of continuous following or its latency.
Report missing feedback capabilities and unmeasured motion honestly; do not add
a marker to a screenshot and present it as device-side live feedback.
When Windows reports `pointer_feedback`, read current state from
`devices[].state.runtime_status`, not cached target capability descriptors.
The service-owned idle marker persists without a task; activity expiry returns
to idle rather than hiding. It never moves the system cursor while idle.
Distinguish device-screen feedback from grounding images: the presenter is temporarily hidden
during grounding capture so its expiry cannot invalidate application pixels.
`visible` and `render_submissions` are native status/counters, not proof of display
latency or a complete drawing. Keyboard feedback and feedback-inclusive visual
capture are separate capabilities; do not infer them from pointer support.

If the target advertises `pointer_drag.implemented`, `act` may take a typed
`intent: {drag:{waypoints:[{at_ms:0,offset:[0,0]}, ...]}}` instead of a string.
This is one device-paced left-button drag, not one MCP call per point. Select
the original ref/frame normally. Offsets are device physical desktop pixels
from the grounded reference, never preview/controller pixels. Times increase
strictly, 2..128 points, total 16..5000ms; stay in the same fixed-geometry native
window and selected element ancestry. Observe the effect afterward. Cancellation
or failure may leave a partial stroke; query the original operation, never replay
the whole drag. The advertised sampling interval is not proof of rendering FPS or latency.

The cell has a 30s default / 60s maximum execution deadline and a 64-call limit;
it is not a background task. Emit only the needed data, since calls are not
automatically echoed. Parent-owned `calls` retain operation references and detail
`result_ref`s. Check `state_retained`: an unhandled cell error, cancellation or reset
discards JS state, not native history; a caught native-tool error need not discard
state. Query uncertain operations instead of repeating the cell. A returned `generation`
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
- An accepted action followed by `native_scene_changed` needs a new read, not a
  replay of the action or whole JS cell. If `state_retained=false`, rebuild the
  needed wrappers/bindings from current facts, while retaining native receipts.
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

## 本机 macOS 实测语义（2026-09-15 起，均带活体回执）

- **控件语义**：`computer.observe` 的 `scene.elements[]` 现含 `role`（AXRole，如 AXButton/
  AXTextField/AXScrollArea）、`actions`（元素自报 AX 动作名，≤12）、`enabled`（AXEnabled，
  未知时为缺省而非 false）。选目标前先看这三项，不要盲试。
- **按动作名执行**：`computer.act(intent="click", value="<元素自报的动作名>")` 会执行该原生
  动作（如 AXPress）；未声明的动作立即 `unsupported_action`（fail-closed）。无 value 的普通
  click 对非主窗口（sheet/panel/popover）自动走原生动作路径。
- **窗口角色**：`computer.bind_window` 返回 `subrole`（AXStandardWindow/AXSheet/AXDialog/
  AXPopover…；读不到为 null，不猜测）——判断"这是不是对话框"以绑定回执为准。
- **授权显示器**：agent 策略 `platform.authorized_display={built_in,id?,place_on_bind?}`。
  非授权屏窗口 bind → `unauthorized_display`（不绑定、不输入）；`place_on_bind` 默认 **false**
  （隐式移动会作用于任意被选中的窗口——2026-09-15 事故后改为显式开关，且移动前预检窗口尺寸）。
  系统文件面板可能**不可搬移**（AXPosition 不生效），跨屏时会被策略拒绝绑定。
- **后台启动与快归位**：`launch_background`（NSWorkspace，activates=false,hides 可选）+
  `launch_background_and_place`（50ms watcher → AXPosition 归位 → 原生复查）；hidden 启动的
  窗口不在 on-screen 发现范围。
- **输入域 fence**：一次 uncertain 派发会 fence 输入域（跨会话/重启持久），需
  `computer.recover_input(device_id, resource_domain)`；combined-session 陈旧键不再死锁恢复，
  但硬件源按键/任意按钮/momentary 修饰键仍 fail-closed。
- **失败码速查**：`stale_reference`（内容/焦点变化→重观察）、`input_domain_fenced`（先恢复）、
  `unsupported_action`、`native_press_rejected`/`native_action_rejected`（应用拒绝）、
  `unauthorized_display`、`placement_impossible`（窗口大于屏）、`visibility_unknown`
  （元素可见性未知，元素路径拒；可试像素路径）、`ax_attribute_unavailable`（AX 瞬时不可用）、
  `window_missing`（sheet/panel/独立进程窗口的解析回退链已覆盖常见情形）。

## Explicit window placement (computer.move_window)

- Purpose: move the **bound** window onto a requested display without ever
  pressing input or touching the real cursor. Placement is idempotent; a
  repeated request with the same `request_id` is deduplicated or answered as
  `dispatched_again=false`; reusing a `request_id` with different arguments
  fails closed (`operation_payload_conflict` / `idempotency_conflict`).
- Required arguments: `target`, `lease` (claim first), `request_id` (writes are
  always refused without a stable id; bound to 1..128 chars), `display`
  (`{built_in?: bool, id?: u32}`; at least one criterion, `{}` is rejected with
  `invalid_display_target`), `reference`.
- Reference choice: `computer.resolve` with locator `{kind:"window"}` yields a
  **placement-only** reference that needs no visibility/occlusion state, so a
  covered or off-screen window can still be parked on another display. Feeding
  that reference to `computer.act`/`act_checked` fails closed with
  `invalid_reference_class` — resolve an element or pixel for input.
- After a successful move every frame/reference on that desktop is invalidated
  (`unknown_reference`); re-observe before the next step. The receipt's
  `placement` reports the native bounds and the display actually containing the
  window; verify with a fresh `computer.discover` readback.
- Unknown display target → `display_not_found`; window vanished mid-move →
  `window_missing`; oversized window for the target display → the pre-check
  reports `placement_impossible` and nothing is moved.

## Binding semantics (updated 2026-09-15)

- `bind_window` is a pure selection: with implicit placement off it never moves
  a window and now also **accepts windows that sit on a non-authorized
  display** (previously refused with `unauthorized_display`). The refusal made
  explicit placement a one-way trap: once a window was moved elsewhere, it could
  not be bound, observed, or moved back (occlusion also blocks element
  references). Placement changes only through `computer.move_window`.
- With `authorized_display.place_on_bind=true` the old placement-on-bind
  behavior still applies and reports `placed` in the binding receipt.

## macOS deployment gotcha (found live)

- Replace an installed executable with `rm` + `cp` (fresh inode). Overwriting a
  binary in place with `cp` can leave it silently unexecutable (immediate silent
  exit, identical sha, no stderr). If a freshly deployed binary seems mute, `rm`
  it and copy again.

## Popup delivery (computer.observe -> popups)

- Every `computer.observe` result carries `popups`: the bound process's windows
  from the same AX surface as binding — `{window, title, role, subrole, kind,
  bounds, display}` with `kind` in
  `standard | dialog | sheet | panel | alert | other | unknown`.
- Non-foreground dialogs, sheets and sandboxed panels are listed as soon as they
  exist; purposes are never guessed. A sandboxed open panel typically arrives as
  `kind:"unknown", subrole:null` with the factual CG title (e.g. "Open").
- Use it to deliver dialogs instead of polling pixels: after a click that should
  open a dialog, observe again and match the new `window` id; then
  `computer.discover` -> `bind_window` (relation `owned_popup`) -> act.
- System panels may refuse to move (AXPosition has no effect). Placement is
  always explicit: `bind_window` accepts them on any display and the
  `move_window` receipt reports the window's **actual** bounds/display after the
  call — judge from that readout rather than from the request.

## Visual pointer feedback (computer.pointer_feedback)

- A borderless, click-through overlay window (title `RCU AI Cursor`) marks the
  point of the last dispatched action or an explicit point. It is visual only:
  no input, no real-cursor movement, no focus change, and it can never be
  mistaken for a goal.
- `{target, visible: bool, point?: [x, y]}` — `point` is required the first time
  you show it; hide with `visible: false`. The receipt is synchronous
  (`created/visible/point/last_error`), so it describes the state after the
  call. The marker auto-hides after ~45 s idle.
- Every dispatched act updates the marker automatically — after a click, expect
  the overlay at the element's point (read it back with `computer.discover` by
  title, or pixel-prove it locally with `screencapture -l <cgwindow>`).
- Operational note: the overlay is a separate window owned by the agent
  process; it does not occlude hit-testing for background delivery.

## Modal leftovers block toolbar actions (correct fail-closed)

- While a modal panel is open (e.g. the Paint S `Open` panel), the app's own
  toolbar buttons report `enabled=false`, so `computer.act` refuses with
  `input_boundary_changed` (`... (enabled_disabled); stop remaining batches.`).
  That is correct: close the modal first (bind the panel, tap `Cancel`), then
  the toolbar actions dispatch again. Never retry the same reference blindly —
  re-observe after closing the modal.

## Shared pointer material (rcu-pointer)

- The AI marker is the A1 atmospheric lens rendered by the shared `rcu-pointer`
  crate (diamond + rim + `RCU` label + animated highlight). Windows and macOS
  use the same material; do not re-implement the visuals per platform.
- macOS specifics: the overlay window title is `RCU AI Cursor`; its raster dpi
  follows the window's `backingScaleFactor` (192 on Retina, 96 on 1x); the lens
  hotspot is the window center, so readback bounds are `point - 80` in points.
- `computer.pointer_feedback` receipts report `dpi` and `material`, and every
  dispatched act re-shows the lens at the action point.

## Cooperative yield and the operation journal

- RCU yields while a person is driving the machine: if a real click or key
  happened within the last ~1.2 s, every input dispatch fails closed with
  `user_active` (message includes the observed seconds). Motion alone never
  gates. Nothing is queued and nothing is replayed: re-observe and retry after
  the person pauses (the receipt stays `uncertain` at the syscall layer, but
  `native_dispatch_entered:false` proves no native effect).
- Read `user_input_recent_seconds` from the target capabilities to know how long
  the machine has been quiet; it is also shown for cross-platform parity.
- Every act appends one JSON line to the per-principal `operations.jsonl`; the
  bounded tail is delivered in `computer.targets` ->
  `devices[].state.recent_operations` (newest first). It records what RCU
  attempted (`dispatch`, `dispatch_error`, `intent`, `point`, `scope`,
  `request_id`) and is never a claim that an action took effect.
- Presence: `pointer_feedback` in the capabilities reports `{installed,
  created, visible, point, dpi, material, last_error}` for the shared A1 lens.
