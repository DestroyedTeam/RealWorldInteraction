---
name: real-computer-use-native
description: Use Real Computer Use native MCP capabilities to inspect and operate user-authorized devices, windows, mouse and keyboard through independent Rust device agents. Not a shell automation bridge.
---

# Real Computer Use

Use the available `computer.*` MCP tools directly. The MCP adapter has a stable
tool catalog; each paired device runs its own headless agent. Neither service
requires a business task, a controller stage, a test application, or a prior
business result. A window closing does not invalidate the installation.

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
Prefer `computer.act_checked` with an exact observed element. Native-element
policy checks the selected window and element; document text is not a universal
precondition for every button. Text/key input still requires the correct focused
editable element and fresh state. Do not downgrade a rejected checked action to
unchecked/pixel input to bypass its failed preconditions.

Use a stable `request_id` per intended mutation. Keep its returned `operation_ref`
for execution status/cancellation. Dispatch once; independently read the expected native
predicate with `computer.verify`; release promptly. A dispatch acknowledgement or
a matched single predicate is not proof of the entire business task.

One desktop has one input/focus stream. Multiple windows do not create independent
mouse/keyboards. Independent devices and read-only reasoning can be concurrent.

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
