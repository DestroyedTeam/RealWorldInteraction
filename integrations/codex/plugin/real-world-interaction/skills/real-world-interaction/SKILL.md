---
name: real-world-interaction
description: Select and operate explicitly authorized devices through the Real World Interaction plugin. Use for multi-device discovery, target routing, native GUI observation and input; then load the matching macOS or Windows skill.
---
# Real World Interaction

This plugin contains one platform-neutral Rust MCP gateway and platform-specific skills. Other Hosts may register its MCP entry and load these same skills without Codex.

1. Use this plugin's currently loaded `computer.targets`. If the user's exact paired `device_id` is already known, pass it to avoid waiting for unrelated offline endpoints; otherwise omit it and inspect all configured devices. Read the selected device's `device_id`, `platform`, native context and capability flags. A filtered result says nothing about other devices' availability. A globally listed tool is not necessarily implemented, authorized or ready on that device.
2. Select the user's exact device. Do not infer the controlled OS from the OS running Codex or from a nickname. An offline device never redirects work to another device.
3. Read the matching sibling skill: `../rwi-macos/SKILL.md` for `platform=macos`, or `../rwi-windows/SKILL.md` for `platform=windows`. Unsupported platforms remain unsupported.
4. Discover a fresh native window on that device. When independent attachment is advertised, use `computer.attach_window` on the original desktop target and retain its new target; otherwise use the platform's explicit binding path. Preserve the complete returned target and frame; keep per-device wrappers in distinct state entries. Never reuse a Mac frame, point, lease or reference for Windows, or vice versa.
5. Before dispatch check that the intended tool is implemented and authorized on the selected device, then satisfy its native binding/focus/lease prerequisites. Skills do not grant authority or make unavailable capabilities available.
6. Dispatch one stable mutation ID and independently observe its effect. Convenience input tools own and release their short lease; claim/release manually only for lower-level tools that require it. After uncertainty query the original operation; never replay input.

For ordinary inspection/input, prefer the advertised typed tools: `computer.snapshot`, `computer.controls`, `computer.click`, `computer.type_text`, `computer.press_key`, `computer.drag`, and `computer.scroll` when loaded. `snapshot` returns an image plus compact controls; `controls` searches/pages its retained metadata without another capture. Use the exact returned frame/element or mapped view. Do not write a temporary JS wrapper merely to click one control. `scroll` owns one short lease and sends one intent; positive `delta_y` is up, negative down. macOS uses pixels (nonzero +/-2000); Windows uses wheel units (multiples of 120, +/-1200). Do not carry units across devices or retry a refused scroll.

Use `computer.js` for logic that genuinely benefits from bounded related calls. `rwi` is the canonical client (`rcu` is a same-object compatibility alias). Persist objects in `state.*`. A window wrapper wraps one exact returned target. The original desktop target has a replaceable current binding; an advertised `attach_window` creates a new fixed window target. `await desktop.attach(selection)` returns that target's wrapper without replacing the source binding. Multiple variable names wrapping the same target still share state; they do not create independent windows. All windows on a desktop share its physical input lease. The JS helper is not a shell or filesystem bridge.

A failed JS cell is not a rollback: an earlier input may have returned before a later observation failed. Inspect `execution_summary` and original nested mutation receipts, then independently read the resulting UI; do not rerun the cell. JS errors discard local state, not native execution history. See [macOS multi-window and error recovery](../rwi-macos/references/multi-window-recovery.md) when applicable.

Use [the detailed workflow](references/native-workflow.md) for frame pagination, compact refs, execution identity, cancellation and release. GUI actions used to validate this plugin must originate from its loaded MCP tools. A standalone stdio probe validates transport only, not Host-native GUI acceptance.

Additional MCP servers may be added to this plugin later. Give each a distinct logical server name and explicit routing purpose; do not infer that matching tool names, numeric IDs or coordinates mean shared devices. The current gateway already enforces device ownership of handles; platform labels in a skill are not the isolation mechanism.
