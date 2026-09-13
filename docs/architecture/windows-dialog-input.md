# Windows dialog input: authorization, selection and focus

The 2026-09-13 user authorization covers the Windows desktop, including system
dialogs, Save/Open and Explorer. No application allowlist or business-task gate
is introduced. A future app allowlist belongs to independent device policy.

## Separate identities, not a foreground bypass

- Desktop service policy authorizes a fresh exact application selection. A new
  `initial` selection in this policy is not a new connection or installation.
  Each validated selection receives a fresh grant narrowed to that selected
  window. An independently selected dialog is not retained as an immutable grant
  root when returning to its proven owner. This does not modify legacy grants.
- Native owner-chain transitions select popups/return to owners. Same PID, title,
  class or overlapping rectangles do not prove ownership.
- A popup can be visible and enabled while its owner remains the foreground
  window. Requiring `selected HWND == GetForegroundWindow()` incorrectly blocks
  such menus. Physical foreground remains reported truthfully, never rewritten.
- Under desktop policy, input readiness accepts either the selected foreground
  window or its exact foreground native owner. The owner proof includes HWND,
  PID, process creation, epoch and complete ownership chain. Legacy root-only
  allocation is not broadened.
- Observation binds the foreground identity before/after UIA/capture. Each native
  batch rechecks the retained anchor, selected-window geometry/epoch/ownership,
  exact UIA ancestry, hit-test, desktop/session, user input and lease. A sibling,
  unrelated foreground or changed owner is rejected; there is no automatic focus
  loop, background input or input replay.

This follows the distinction in Microsoft's [window relationship documentation](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features)
and [GetForegroundWindow contract](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow).

## One authoritative keyboard target

Some native UIA providers expose `HasKeyboardFocus=true` on both a containing
pane and the actual focused control. Previously the observation promoted both
flags, while native input correctly required the exact `GetFocusedElement`
runtime ID. Picking the first advertised pane therefore failed at dispatch.

The agent now calls [GetFocusedElementBuildCache](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-getfocusedelementbuildcache),
proves its ancestry under the selected UIA root, and prioritizes that element in
the bounded observation. Only that runtime ID is promoted as focused. Cached
provider flags remain diagnostic; duplicate tree entries cannot replace the
priority record. Unknown/out-of-root focus is not guessed. Exact focus may be
outside the tree's depth/node budget, without implying complete tree coverage.

Input still re-reads the actual focused element, provider, enabled/password
classification and ancestry. Text additionally needs a writable native control
and bounded value revision. Keyboard identity is not replaced with arbitrary
container or pixel input after rejection.

## Ordinary desktop shortcuts

Windows desktop policy also enables Win-key and ordinary shell-switching chords
such as `Win+E`, `Alt+Tab` and `Ctrl+Escape`. Legacy window-limited grants retain
their previous restrictions. This policy is selected by the device backend, not
by a caller-supplied keyboard flag. A chord is one balanced native batch, with
extended-key flags and reverse-order key releases. New windows/focus must be
observed and selected afterward; an old frame never follows the shortcut into
another app. Secure attention (`Ctrl+Alt+Delete`) is not a SendInput capability.

## Verification scope

Opening a menu, inserting input, opening a Save/Open dialog, persisting a file
and reopening its pixels are separate evidence nodes. A rejected action is not
an application authorization denial unless device policy actually denied it.
Secure desktop, logon/session changes and native API limitations remain explicit
capability boundaries, not ordinary application allowlisting.
