> Historical v0 architecture. For the current root workspace and module boundaries, see [ADR 0001](adr/0001-rust-workspace-boundaries.md) and [governance status](architecture-governance.md). Platform support claims below are dated history, not current status.

# Architecture and boundaries

## Intent protocol

`targets → observe → resolve → claim → act → observe/verify → release` is a control loop, not an unconditional macro. The caller defines a postcondition and decides the next bounded step from current evidence.

- Identity is host + optional guest + session + seat + desktop. Registration pins it; a later adapter identity cannot redefine it silently.
- Observations separate metadata, image artifact and mapping. Up to 64 metadata frames, 256 references, 64 image artifacts, 32 MiB total retained PNG and 16 MiB per PNG. Maximum inline chunk is 8 MiB. These limits do not claim the process's entire RSS stays below 32 MiB.
- Semantic references carry element identity, state revision, point, window and PID. Revalidation ignores unrelated image pixels, **not** native focus/layout/modal/process epoch, target value/state or geometry changes. Pixel-only references additionally require whole-frame content equality.
- Input leases use OS advisory file locks keyed by desktop identity, TTL 100–60000 ms, one expiry worker, explicit unlock and RAII close. A dispatch guard prevents expiry from allowing overlap while an operation is still executing. Cooperating processes must use the **same state root**. This is not a distributed lock, a security sandbox, or a lock against humans/other tools.
- A durable request reservation is written and synced before dispatch; the Unix containing directory is synced too. A result is appended afterward. Interrupted/uncertain reservations forbid replay across restart. No automatic retry of input is implemented. Power-loss guarantees still depend on filesystem/OS storage semantics.
- Receipts distinguish `returned` from `uncertain`; `system_accepted=null` for CGEventPost because that API returns no effect acknowledgement. `goal_verified` is always false in action receipts. A backend failure before its first event and after a partial event sequence currently share conservative `uncertain`; finer dispatch stages remain pending.
- Verification reads the precise window/PID/element independently. The same text in another window/process/element fails. Optional request provenance must match the exact scope. Predicate equality is not proof of causation.

## macOS native candidate

Only explicit `--macos-window PID:CGWindowID` registers a native target; `--allow-input` is an additional opt-in. Nothing is focused, launched, unlocked or permission-prompted automatically.

- Session: public Quartz console dictionary, console UID/seat, boot epoch and local host UUID; process start identity from `proc_pidinfo`.
- Window: exact on-screen CGWindowID and owner PID, a unique matching AX focused window/bounds, independent AX focused app and hit-test. No fallback to title-only selection.
- Layout: active display bounds, pixel dimensions and rotation are hashed. Only a single containing unrotated display is currently mapped; straddling/rotated cases return unknown rather than assuming scale 1.
- Capture: ScreenCaptureKit shareable window owner/bounds, desktop-independent filter and point/pixel scale, cursor/shadow exclusion, exact configured native dimensions, pre/post metadata comparison and bounded completion wait. No persistent capture stream.
- Image pipeline: the callback CGImage is retained for RAII conversion through a **device-RGB bitmap**, then unpremultiplied to straight RGBA PNG. Dimensions are preserved and there is no automatic resizing. Color/alpha normalization means this is **not a byte-identical source-pixel promise**. Offscreen tests cover row direction, channels, alpha and dimensions, not every color profile/HDR display.
- Input: direct CGEvent click/double-click/context-menu, bounded vertical scroll, explicit physical key chords, and surrogate-safe Unicode chunks for an exact focused non-secure text target. Pre-dispatch native checks are not an atomic OS transaction; physical user input or another controller can still race.
- Text: `AXSecureTextField` is an **AXSubrole**. Unified classification gates description-time AXValue reading, input and verification. A text role with unavailable/empty/AXUnknown subrole is unknown and rejected; classification is not guessed from role alone.
- Oracle: AX readback, not an echo of the requested value or dispatch receipt. Tree traversal is partial and bounded by node/depth/time budgets.

## Known limitations that block production parity

1. No live macOS test has run. ScreenCaptureKit callback timing, OS consent/code signing behavior, real app hit-test mapping, native goal workflows, permission denial, locked screen and display transitions still require controlled acceptance.
2. Cross-window/modal rebinding and discovery are not implemented. Fixed foreground-window binding can reject an ordinary workflow when it opens a separate dialog. Do not call that goal failure proof or pretend it supports general autonomous desktop work yet.
3. Windows UIA/SendInput/Desktop Duplication, Linux AT-SPI/X11/Wayland portal capture/input, VM sessions and authenticated remote transport are pending. The generic Backend trait is not an implementation of them.
4. Modal state unavailable, mixed-display/rotated windows, unknown hit tests and unclassified text fail closed. Secure text is deliberately unavailable in this candidate.
5. Native polling has a bounded iteration/deadline policy and AX call timeouts, but an individual OS call can overrun the caller's requested poll duration. Strong cancellation/supervised platform workers need further design.
6. Receipt storage grows with unique requests and is intentionally not pruned. Production disk quotas, compaction with replay tombstones, and multi-user/path hardening are pending. Deleting receipts is **not** an approved cleanup/retry operation.
7. The MCP stdio surface is tested; network transport/authentication, hostile-client framing/resource hardening and host-side plugin installation are not.

## Next implementation gates

Implement native discovery/rebinding and per-session supervised Rust workers, then Windows and Linux native adapters. Each must independently satisfy the identity/mapping/lease/no-replay/oracle contracts. Add controlled complete goal scenarios and degradation/resource measurements on each OS before claiming native parity. Keep historical failures as regression seeds rather than extending CrossMacro's command parser indefinitely.
