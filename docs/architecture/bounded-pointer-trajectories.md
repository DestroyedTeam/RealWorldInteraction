# Bounded native pointer trajectories

Date: 2026-09-12. Implements the input capability within ADR 0002, not a new
task-dependent service or Windows-only product architecture.

## Public contract and ownership

The existing `computer.act` / `computer.act_checked` intent union adds a typed
`{ "drag": { "waypoints": [{"at_ms": 0, "offset": [0, 0]}, ...] } }` variant.
Existing string intents and tool names remain unchanged. The Rust shared schema
defines the path; old adapters/agents reject unknown variants rather than
silently downgrading them. Discover `pointer_drag.implemented` before use.

`DragPath` requires 2..128 finite vertices, strictly increasing timestamps,
first timestamp/offset zero, final timestamp 16..5000ms, and offset components
within +/-16384 device physical desktop pixels. A separate string `value` is
invalid. Coordinates are relative to the grounded native reference, not the
Host screen, preview pixels or physical hardware outside the selected device.
The Windows implementation is fixed-window, left-button only. Mac explicitly
reports drag unsupported; no other platform support is inferred.

The core validates the typed path and includes **every vertex and time** in the
same durable request fingerprint as the original frame/element/intent. One path
is one mutation reservation, not hundreds of independent request IDs. Changing
the path under the same request ID conflicts; cancellation/reconnection never
replays a partial stroke. The adapter does not inject mouse events or execute
Shell/PowerShell/Python. Device agents remain independently deployed Rust services.

## Windows execution

The portable planner retains every vertex and linearly samples intervals at
16ms, with at most 440 samples. Before any movement/down, all sample coordinates
must lie in the native desktop and selected window bounds. Before each dispatched
sample, native checks revalidate the allocated identity/session, foreground,
geometry, layout, native UIA hit/ancestry, input state, authorization and lease.
There is no screenshot or model round trip per sample.

The native sink sends one left-down, the paced moves, then left-up. Move samples
use `MOUSEEVENTF_MOVE_NOCOALESCE`; this disables normal WM_MOUSEMOVE coalescing,
not application rendering delays. Absolute virtual-desktop coordinates avoid
pointer acceleration. [Microsoft MOUSEINPUT reference](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput).

`SendInput` returns insertion counts, not drawing/compositor completion. UIPI
failure cannot be diagnosed from the count alone. Events in each call are
serialized, but the entire multi-call trajectory is not an OS transaction.
[Microsoft SendInput reference](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).

The executor wakes/checks cancellation at intervals of at most 8ms **while
waiting between calls**. Native providers may still block. If the schedule is
more than 250ms late after guards, it aborts rather than skipping vertices,
bursting delayed samples or replaying down. Nominal sampling is not an FPS,
zero-latency or strong-cancellation guarantee.

## Button lifetime, failures and evidence

- A release-owning scope begins immediately before attempting down. Every normal
  post-down exit, cancellation, partial insertion, late schedule and unwind
  attempts exactly one **release-only** cleanup. No down/move retry.
- A pre-down guard rejection does not release a button potentially held by a
  user. During dragging, only the attempted left button is exempted from the
  idle-key guard; other keys/buttons, or an unexpected left-up, stop the path.
- A failed release marks sticky `resource_release_uncertain`. Finishing the
  Rust operation does not falsely set `resources_released=true`; the resource
  remains subject to the existing explicit native reconciliation/fencing rules.
- Failure diagnostics preserve the original fault and expose requested samples,
  acknowledged moves, elapsed time and release result. A successful insertion
  still has `goal_verified=false`.

Physical motion by another source or a user re-pressing the same left button is
not reliably attributable here. Cooperative leases are not exclusive ownership
of the Windows desktop. Process termination/power loss can prevent in-process
cleanup; native-provider worker isolation and stronger stuck-input recovery
remain separate open work. No design claim removes those boundaries.

## Validation separation

Offline tests cover bounds/interpolation, original request fingerprinting,
pre-down rejection, post-down cancellation/partial insertion/schedule failure,
release failure fencing and unwind cleanup. The JS test proves the typed path
is forwarded as one original-frame checked call, not a per-point MCP loop.

The opt-in Rust example `rcu-windows/examples/pointer_fixture.rs` is an independent
test application, not a backend. It draws mouse events actually received by its
own window and writes a create-new event log. It never injects input, changes
authorization, launches the agent, or defines a production task/oracle. GUI
capture and SSH access to that log provide complementary visual/event evidence.
Only a completed live test can establish actual sampling, button-up or drawing
outcomes; compiling/running the fixture or opening its window is insufficient.

Existing Host MCP processes keep their old compiled schema. Installing a new
binary updates **future processes**; restarting only a JS worker does not upgrade
its old Rust parent validator. Test fresh adapter processes and do not work
around an obsolete schema by hiding a new intent inside a string or script.
