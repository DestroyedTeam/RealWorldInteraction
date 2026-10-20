# Windows native Pointer feedback

This is a device-agent facility, not a Paint workflow or a new controller UI.
It does not inject input. The Rust input sink notifies it only after an original
pointer event batch was fully acknowledged by SendInput; insertion still does
not establish application effect or task completion.

## Presentation and scope

- One device-local presenter thread, explicitly started by the serving agent,
  not by a business task, discovery or observation. One coalescing input-progress
  slot; no replay queue. The service owns the idle marker as well as active feedback.
- A premultiplied BGRA atmospheric lens, fixed A1 diamond hotspot and `RWI` label. No typed text,
  document contents, app names, credentials or task labels are rendered.
- A layered, disabled, transparent, non-activating tool window. The presenter
  has its own window/message processing thread and per-monitor-v2 DPI context.
- Startup seeds the marker from GetCursorPos without moving the system cursor.
  Actual input progress updates the retained native position; an active highlight
  additionally requires the selected HWND/PID to remain foreground and the cursor
  to remain close to the inserted position. Otherwise it returns to idle.
- The idle marker stays at the last agent position even with no task or after an
  app focus change. A person's subsequent mouse motion does not drag the idle
  marker along, and the marker never warps the person's cursor back. Retention is
  currently in agent memory; a process restart seeds the current OS cursor rather
  than claiming disk-persistent recovery. Display changes clamp to an existing monitor.
- The 800 ms activity watchdog expires to a persistent idle lens, not to
  invisibility. Local light/stretch settles within 180 ms; pressure retargeting
  takes 90 ms. Only the material changes: the hotspot position is never smoothed,
  predicted or delayed. Unchanged idle frames are not re-submitted each tick.
- An error/unwind clears only the matching gesture's active highlight and leaves
  idle presence intact. Newer gestures cannot be cleared by an old guard. Existing
  release-only input cleanup remains authoritative; visual cleanup never replays input.
- Service shutdown destroys the marker. Locked/inactive desktops and unavailable
  native display access are not bypassed. This is neither input ownership nor a
  comprehensive physical takeover detector, and it cannot attest monitor FPS/latency.

Layered-window mouse pass-through with WS_EX_TRANSPARENT is documented by
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features).
The additional disabled style makes WindowFromPoint skip the overlay according
to its [documented contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-windowfrompoint).
These properties still need real UIA/foreground/interaction regression; they are
not inferred proof that every accessibility provider ignores the overlay.

## Grounding images are not visual-monitor frames

An expiring halo must not change the content hash used to authorize the next
pixel-grounded action. The first implementation therefore excludes RWI's own
feedback from existing `computer.observe` grounding PNGs:

1. A capture guard increments a suspension count and obtains a revision ticket.
2. The presentation thread hides the overlay, flushes pending compositor work,
   and acknowledges that revision. An older in-flight render cannot satisfy it.
3. GDI copies the actual target/occlusion pixels into a separate DIB.
4. The capture guard releases before conversion/encoding. The screen resumes its
   idle marker or active hint, but the stored grounding raster remains unchanged.

Nested captures cannot resume each other's suspension. A 250 ms hide-ack timeout
returns `pointer_feedback_capture_busy` rather than returning a possibly
contaminated grounding frame. It does not disable the MCP service or replay
input. Query paths do not initialize the presenter; the serving executable does.

**Current limitation:** this phase supplies device-screen feedback, not feedback
inside RWI grounding screenshots. A later explicitly visual/read-only capture
path must separate presentation from action authority, also allowing image-only
observation without the current UIA walk. Hiding briefly during GDI copying may
be visible on the device; measure this rather than claiming perfect smoothness.
Do not simply remove content-hash or target checks to avoid this boundary.

## Capability and evidence

`devices[].state.runtime_status.pointer_feedback` in `computer.targets` provides
fresh device state, including initialization, phase, anchor, native visibility,
requested updates, render submissions, capture suspensions and errors. Its native
timestamp is explicit. In contrast, remote `targets[].capabilities` descriptors
are context snapshots and must not be read as current visibility/activity.
`observe.adapter_status` reports another point-in-time native sample. A render
submission is not a physical screen presentation.
Keyboard feedback, feedback-inclusive visual capture, dynamic latency and
cross-platform feedback are not attested by this implementation.

## A1 development migration (2026-09-13)

The user selected the diamond design and explicitly approved reversible ordinary
engineering development without claiming formal UI Quality OS acceptance. UIOS
is design evidence for this migration, not a passed release/signature gate.

`motion` is a pure bounded expression model; `material` owns cached, premultiplied
textures and rasterization. The native presenter owns HWND/DIB lifetime and
capture synchronization; `native/preferences` reads animation, contrast and theme
preferences, and `native/label` owns an offscreen GDI Segoe UI coverage raster.
No product subprocess bridge or application-specific behavior was added.

Fractional DPI is supported at the retained physical anchor (96–288 DPI cap).
Idle HWND DPI changes invalidate raster resources, not the hotspot. Label edge
avoidance only moves the label. Reduced Motion disables speed/stretch expression;
a balanced click still has a bounded static contact cue, not a fake held state.
High contrast uses OS foreground/background contours without the diffuse halo.

Material blur is a bounded three-box approximation to the selected browser
prototype, not pixel-identical CSS rendering. `last_raster_ms` / `max_raster_ms`
measure raster generation only: they exclude cache construction, GDI submission,
compositor scheduling, transport and model turns. A nominal 16 ms animation wake
interval is not an attestation of 60 FPS or input-to-photon latency.

Portable tests cover bounded DPI/raster allocation, premultiplied pixels, distinct
idle/pressed visuals, idle-without-task/after-action behavior, and suspension acknowledgement/nesting. Windows cross-build and
native smoke tests remain separate gates. The integration must preserve native
identity, focused-window checks, UIA point guards, leases and the operation ledger.

## macOS presenter (same material, separate host)

The A1 material and motion expression live in the shared `rwi-pointer` crate;
`rwi-windows/src/pointer_feedback` re-exports them at its original paths so its
presenter is unchanged. The macOS presenter renders the identical material
inside a borderless, click-through AppKit window (`RWI AI Cursor`, accessory
policy, level 1000) owned by the device agent:

- AppKit work is confined to the process main thread, which pumps a bounded
  CFRunLoop; the serving agent runs on its own thread and exits with it.
- The raster dpi comes from the window's `backingScaleFactor` after it is
  ordered onto the target display (Retina 1:1), clamped to `MAX_DPI`.
- Short-horizon differences from Windows: no WS_EX_TRANSPARENT equivalent is
  needed (`setIgnoresMouseEvents`), per-monitor work-area clamping is not
  implemented (the hotspot follows the requested point exactly), and capture
  suspension is unnecessary because `computer.observe` captures the bound
  window, which never contains the overlay window.
- The idle lens auto-hides after ~45 s; every dispatched act re-shows it at the
  action point. It never writes to the pasted clipboard, never injects input and
  is not a goal-verification signal.
