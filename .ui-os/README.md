# UIOS scope: device pointer feedback

This design contract scopes the currently implemented Windows pointer surface.
It does not narrow RWI's approved multi-endpoint architecture or claim production
support for macOS, Linux, Android, iOS or hardware endpoints. Those pointer
renderers are deferred and unverified; common service/input meanings remain
platform-neutral. A future native target needs its own references and evidence.

This is a Standard material component redesign (two distinct directions), not a
new application UI and not a Micro color adjustment. Human selection and exact
design review precede production visual changes. Local animated prototypes are
design demonstrations, never evidence of native Windows input or display FPS.

## Delete / merge before adding

- Replace the ordinary arrow silhouette; it is too easy to mistake for the human
  system cursor. Do not replace or hide the actual OS cursor.
- Replace the coarse rectangular bitmap label and diffuse halo with one coherent
  identity treatment; no additional toolbar, dashboard, global banner or window.
- Merge all activity hints into the one service-owned pointer presenter.
- Keep idle presence. Do not create fake activity through looping spinners,
  constant traveling trails or a second moving pointer.
- Animate only truthful visual state changes; never add interpolated native mouse
  events, replay, cosmetic input delay, or a goal-success signal.
- Preserve capture hide acknowledgement, transparency, disabled/nonactivating
  window styles, resource binding, input leases, and operation reservations.

## Evidence gates

Current runtime confirmation is recorded separately in
`../docs/validation/2026-09-12-pointer-feedback-persistent.md`.
The user's “visible” confirmation is not approval of the new visual direction.
Tokens are intentionally unresolved until design selection. There is no active
DirectionLock, implementation candidate or all-lane Windows capture authority.
No formal UIOS release or Computer Use parity is claimed.
