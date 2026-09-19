---
name: real-world-interaction
description: Select and operate explicitly authorized devices through the Real World Interaction plugin. Use for multi-device discovery, target routing, native GUI observation and input; then load the matching macOS or Windows skill.
---
# Real World Interaction

This plugin contains one platform-neutral Rust MCP gateway and platform-specific skills. Other Hosts may register its MCP entry and load these same skills without Codex.

1. Use this plugin's currently loaded `computer.targets`. Read each device's `device_id`, `platform`, native context and capability flags. A globally listed tool is not necessarily implemented, authorized or ready on that device.
2. Select the user's exact device. Do not infer the controlled OS from the OS running Codex or from a nickname. An offline device never redirects work to another device.
3. Read the matching sibling skill: `../rwi-macos/SKILL.md` for `platform=macos`, or `../rwi-windows/SKILL.md` for `platform=windows`. Unsupported platforms remain unsupported.
4. Discover and bind a fresh native window on that device. Preserve the complete returned target and frame; keep per-device wrappers in distinct state entries. Never reuse a Mac frame, point, lease or reference for Windows, or vice versa.
5. Before dispatch check that the intended tool is implemented and authorized on the selected device, then satisfy its native binding/focus/lease prerequisites. Skills do not grant authority or make unavailable capabilities available.
6. Claim briefly, dispatch one stable mutation ID, observe the effect, and release. After uncertainty query the original operation; never replay input.

Prefer `computer.js` for bounded related calls when advertised. `rwi` is the canonical client (`rcu` is a same-object compatibility alias). Use `state.devices`/`state.windows` keyed by full returned identity rather than one global window variable shared across platforms. The JS helper is not a shell or filesystem bridge.

Use [the detailed workflow](references/native-workflow.md) for frame pagination, compact refs, execution identity, cancellation and release. GUI actions used to validate this plugin must originate from its loaded MCP tools. A standalone stdio probe validates transport only, not Host-native GUI acceptance.

Additional MCP servers may be added to this plugin later. Give each a distinct logical server name and explicit routing purpose; do not infer that matching tool names, numeric IDs or coordinates mean shared devices. The current gateway already enforces device ownership of handles; platform labels in a skill are not the isolation mechanism.
