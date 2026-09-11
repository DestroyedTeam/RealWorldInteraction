# Codex native MCP / plugin / Skill

The production path is `AI Host → rcu-mcp → authenticated device RPC → rcu-agent → native OS driver`.
No Python or shell process implements native input/capture. ADR 0002 governs the
entire implementation, not a temporary first-phase topology.

## Stable adapter entry

Build `rcu-mcp` with default features (no `legacy-local` or `native-agent-worker`).
Run it with `--profile ABS_PRIVATE_PROFILE`, or without arguments for a healthy
zero-device catalog. The profile contains paired device endpoints and private
identity **paths**, not business tasks, windows, stages or expected output.

The independent device service runs `rcu-agent --config ABS_PRIVATE_CONFIG`.
An agent can be offline or lack native permissions without hiding adapter tools.
Reconnect creates/refreshes only the authenticated context; it never retries an
uncertain mutation. Device-specific status remains distinct from MCP startup.

The maintained Skill is [real-computer-use-native](skills/real-computer-use-native/SKILL.md).
Local generated plugin and project config must point at the same stable adapter
binary/profile, not at `work/` or a validation manifest. Use either project MCP
registration or plugin registration, not duplicate competing entries. Plugin
packaging, project Skill discovery and a registered ChatGPT App are distinct:
no App ID is invented and local packaging is not a global installation.

Tests `gateway_stdio` and `gateway_transport` exercise a real production MCP
process, zero-device startup, multi-device collisions, authenticated cancellation,
Host EOF and a bounded parallel cleanup budget. Synthetic peers are not native
Windows acceptance. `rcu-agent/tests/service.rs` tests service state separately.

## Historical evaluation entry (not production)

The commands below require `legacy-workflow` and document old evidence only.
They must not be used as a startup prerequisite for the production adapter.

`rcu-native-setup --make-client-entry` takes the explicit private `ClientConfig`
on stdin. Select `"model_output":"compact_v1"` for the native plugin. The legacy
omitted/default mode remains `full` for existing transport clients.

After a valid task-bound entry exists, `rcu-native-setup --make-plugin` takes:

```json
{"entry":"/absolute/private/model/.mcp.json","output":"/absolute/new/real-computer-use-native"}
```

Export validates the original entry, exact relay image and task binding. It writes
a new compatibility plugin manifest, MCP configuration and Skill. It does not
copy credentials, install globally, start a VM, approve input, or register an App.
The output is a local task-bound bundle, not a portable public release.

The actual Codex host integration test is opt-in: supply an absolute
`RCU_CODEX_BINARY`, a separately built `RCU_NATIVE_TEST_WORKER` with `link-fixture`,
and optionally `RCU_NATIVE_FLOW_EVIDENCE`. Run the `full_link` test
`actual_codex_plugin_skill_and_mcp_call_over_rust_tls` with `fixture` and `--ignored`.
It installs only into an isolated temporary `HOME/CODEX_HOME`, loads the Skill,
and makes an actual Codex MCP call over Rust TLS. It performs no model inference
or Windows operation.

Full responses are stored privately before compact projection, keyed by SHA.
Original server schemas, snapshots, guards and journal remain authoritative.
App registration is separate: no fabricated `.app.json` connection ID is generated.

References: [Codex MCP configuration](https://learn.chatgpt.com/docs/extend/mcp?surface=cli),
[official plugin packaging](https://developers.openai.com/plugins/build/plugins).
