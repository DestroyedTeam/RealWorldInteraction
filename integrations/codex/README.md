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

## User-level installation, not project activation

RCU is general Host infrastructure. Register the production adapter once in the
user's Codex configuration (`~/.codex/config.toml`) or through an actually
installed user-level plugin. New projects and worktrees must not need their own
RCU configuration, binary, copied credentials, or a prepared business task.
Project-scoped registrations are only appropriate for deliberately isolated
development fixtures, not the normal product installation.

Install the binary, profile and paired **client** identity under a stable private
user-owned directory outside the source checkout. On macOS the current layout is
`~/Library/Application Support/RealComputerUse/client-v1/`, with `bin/rcu-mcp`,
`profile.json` and `identity/`. Profile identity paths must resolve inside that
installation, not to `.rcu/`, `work/` or a validation manifest in a checkout.
Use private directory permissions (0700) and identity/profile permissions (0600).
Moving an existing client identity preserves its certificate, device pin and
authorization; do not generate a new principal or copy device/CA private keys.

After explicitly authorizing user-level installation, use the official CLI:

```sh
codex mcp add real_computer_use_native_link -- \
  "$HOME/Library/Application Support/RealComputerUse/client-v1/bin/rcu-mcp" \
  --profile "$HOME/Library/Application Support/RealComputerUse/client-v1/profile.json"
```

Keep the entry enabled, with `startup_timeout_sec = 30`,
`tool_timeout_sec = 120` and `required = false`. The latter avoids making an
adapter startup failure fatal to unrelated Host work; device unavailability is
already represented by the adapter without hiding its tool catalog. Preserve
all unrelated Codex settings. Remove an obsolete project RCU override only after
the user entry is valid, so the source checkout also uses the installed client.
Use one registration, not competing direct MCP and plugin entries.

CLI configuration inspection is not native-tool acceptance. Verify a newly
created real Codex task in a checkout without local configuration: it must
discover the generic tool catalog and call `computer.targets` directly. Existing
tasks may retain their old tool catalog after registration changes; do not
interpret that cached result as a device transport failure, or claim that
restarting the Host creates a missing registration. Retain before/after results.

The maintained Skill is [real-computer-use-native](skills/real-computer-use-native/SKILL.md).
Global MCP registration, local generated plugin packaging, Skill discovery and
a registered ChatGPT App are distinct facts. This installation does not claim
that a plugin or global Skill was installed, and no App ID is invented.

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
