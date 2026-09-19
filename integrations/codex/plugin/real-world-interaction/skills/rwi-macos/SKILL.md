---
name: rwi-macos
description: Operate an explicitly selected macOS device with Real World Interaction. Use only after device discovery reports platform macos; covers AX controls, Quartz coordinates, owned panels and macOS input prerequisites.
---
# Real World Interaction — macOS

Read the sibling real-world-interaction skill first and retain its selected device identity. Require `platform=macos`; never select a Windows target to work around a Mac failure.

- Read live capability and permission facts. Global tool discovery does not prove that lifecycle, visual capture or any other action exists on this Mac agent.
- Bind fresh PID/window/native epoch. Bind is selection, not proof of focus or window movement.
- Use observed AX roles, enabled state, advertised actions and current selected/value fields where present. Missing attributes are unknown, not false. An incomplete AX tree does not prove the control is absent.
- Use the capture's current pixel-to-desktop transform. macOS points, Retina image pixels and canvas coordinates differ; never apply a Windows pixel offset or assume scale=2. Re-observe after display changes, scrolling, window geometry changes or panel opening.
- Enumerate owned popup windows after opening a panel. Bind the actual popup when it receives input; same PID does not mean the main window remains the input receiver.
- Respect current foreground and keyboard-element prerequisites, cooperative human yield and durable input fences. Do not downgrade a refused action to another input path.
- For application drawing, independently inspect each intended layer and its actual position. An acknowledged drag may have moved a selection rather than drawn a shape. Confirm the active tool after focus changes.

Use only the current tool schemas. Historical macOS observations are documented in the shared [workflow reference](../real-world-interaction/references/native-workflow.md); they are not guarantees for every application/version. For Paint S acceptance, preserve non-reproducible source imagery as independent image layers and verify placement in the actual application.
