---
name: rwi-windows
description: Operate an explicitly selected Windows device with Real World Interaction. Use only after device discovery reports platform windows; covers HWND/UIA identity, interactive sessions, focus, desktop pixels and Windows capability checks.
---
# Real World Interaction — Windows

Read the sibling real-world-interaction skill first and retain its selected device identity. Require `platform=windows`; the Host may itself be macOS. A local terminal command is not a Windows guest command.

- Check online state, implemented/authorized capabilities, interactive user/session/seat/desktop identity and the exact selected window. VM availability and OS permission are separate facts.
- Discover and bind a fresh native window/owner/process generation. Never substitute a Mac PID, AX element, Quartz point or Mac frame for HWND/UIA evidence.
- Ground input in the returned current frame. Desktop physical pixels and resized previews differ; resolve image views through the supplied mapping, especially across DPI/display changes.
- Lifecycle/focus operations must be explicitly advertised for this device. Activation and writable keyboard-element focus are separate; observation-only state is not input permission.
- Owned dialogs, Save/Open and Explorer remain within the selected authorized desktop when identity verifies. Secure desktop/session changes require fresh identity and authority; do not infer them from ordinary popup access.
- UIA failure, elevated target boundaries or an input fence do not authorize a different input mechanism. Preserve uncertainty and query the original operation rather than retrying.
- Read actual runtime pointer-feedback state; visibility counters do not prove latency or completion. Capture support must be reported by this Windows device, never copied from the Mac capability list.

One interactive desktop has one input/focus stream. Separate windows do not permit concurrent input. Separate physical devices may be independent only after their native identity and input domains are independently verified.
