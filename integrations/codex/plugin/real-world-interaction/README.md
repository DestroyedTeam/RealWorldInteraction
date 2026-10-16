# Real World Interaction Plugin

Production Codex bundle: one Rust MCP gateway, one routing skill and two platform skills. The generic server catalog is stable; each agent reports and enforces its own platform capabilities and native target identity. Do not split shared authorization/lease state merely to create OS-specific server labels.

Run `tools/package_rwi_plugin.py` with explicit `--binary`, `--profile` and `--output` paths to render an installed bundle. The source `.mcp.json` starts a PATH-installed `rwi-mcp` with zero paired targets; the packaging step replaces it with explicit user-owned installation paths. No credentials, profiles, state journals or binaries are copied into the bundle. Installing the plugin neither grants OS permissions nor starts a VM.

Other MCP-capable agents can register the rendered `.mcp.json` and load `skills/` directly. OMP/OpenCode registrations remain independent of Codex plugin enablement. To avoid duplicate Codex catalogs, disable the old standalone server only after the plugin is installed and verified.

This is the production bundle. The Rust native-link legacy task-bound plugin exporter is historical and is not used here.
