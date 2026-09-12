# Windows native Pointer feedback

This is a device-agent facility, not a Paint workflow or a new controller UI.
It does not inject input. The Rust input sink notifies it only after an original
pointer event batch was fully acknowledged by SendInput; insertion still does
not establish application effect or task completion.

## Presentation and scope

- One lazy device-local presenter thread, started by pointer input, never by
  discovery or observation. One coalescing event slot; no replay queue.
- A premultiplied BGRA halo, pointer and fixed `RCU` badge. No typed text,
  document contents, app names, credentials or task labels are rendered.
- A layered, disabled, transparent, non-activating tool window. The presenter
  has its own window/message processing thread and per-monitor-v2 DPI context.
- Positions come from native GetCursorPos. Presentation additionally requires
  the selected HWND/PID to remain foreground and the cursor to remain close to
  the most recent inserted pointer position. It is not an input ownership or
  comprehensive physical takeover detector.
- An 800 ms expiry is refreshed by actual progress, with a pressed ring during
  a drag. Expiry is a scheduling policy, not a hard compositor latency promise.
  The presenter polls while active; it cannot attest monitor FPS or display delay.
- An error/unwind clears only the matching gesture's feedback. Newer gestures
  cannot be cleared by an old guard. Existing release-only input cleanup remains
  authoritative; visual cleanup never replays an input.

Layered-window mouse pass-through with WS_EX_TRANSPARENT is documented by
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features).
The additional disabled style makes WindowFromPoint skip the overlay according
to its [documented contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-windowfrompoint).
These properties still need real UIA/foreground/interaction regression; they are
not inferred proof that every accessibility provider ignores the overlay.

## Grounding images are not visual-monitor frames

An expiring halo must not change the content hash used to authorize the next
pixel-grounded action. The first implementation therefore excludes RCU's own
feedback from existing `computer.observe` grounding PNGs:

1. A capture guard increments a suspension count and obtains a revision ticket.
2. The presentation thread hides the overlay, flushes pending compositor work,
   and acknowledges that revision. An older in-flight render cannot satisfy it.
3. GDI copies the actual target/occlusion pixels into a separate DIB.
4. The capture guard releases before conversion/encoding. The screen can resume
   an unexpired hint, but the stored grounding raster remains unchanged.

Nested captures cannot resume each other's suspension. A 250 ms hide-ack timeout
returns `pointer_feedback_capture_busy` rather than returning a possibly
contaminated grounding frame. It does not disable the MCP service or replay
input. Observations before any pointer use do not initialize the presenter.

**Current limitation:** this phase supplies device-screen feedback, not feedback
inside RCU grounding screenshots. A later explicitly visual/read-only capture
path must separate presentation from action authority, also allowing image-only
observation without the current UIA walk. Hiding briefly during GDI copying may
be visible on the device; measure this rather than claiming perfect smoothness.
Do not simply remove content-hash or target checks to avoid this boundary.

## Capability and evidence

`pointer_feedback` under Windows target status reports initialization, native
visibility state, requested updates, successful render submissions, capture
suspensions and errors. A submission is not a physical screen presentation.
Keyboard feedback, feedback-inclusive visual capture, dynamic latency and
cross-platform feedback are not attested by this implementation.

Portable tests cover bounded DPI/raster allocation, premultiplied pixels, distinct
pressed visuals, and suspension acknowledgement/nesting. Windows cross-build and
native smoke tests remain separate gates. The integration must preserve native
identity, focused-window checks, UIA point guards, leases and the operation ledger.
