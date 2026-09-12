# User-level RCU MCP installation and fresh-task acceptance

## Verdict and scope

**Passed:** the installed user-level adapter was discovered and called directly
from a newly created real Codex desktop task whose worktree had no local Codex
configuration. This is installation, tool-discovery and authenticated read-only
device communication acceptance, not screenshot/input/full-product acceptance.

No Codex application restart, Windows restart, firewall change, new pairing,
permission expansion, GUI operation or input replay was performed for this
installation. The application-bundled CLI was used to register/inspect the MCP;
CLI output was not substituted for the real task's native MCP calls.

## Original failure retained

- Task `01a09380-a9ff-7db0-8382-b09bcb557390`, **验证新 RCU MCP 的真实可用性**,
  inspected all 143 available/deferred tools and found zero RCU tools.
- Its worktree `0192` had no `.codex/config.toml`. Before this change, only the
  original project registered `real_computer_use_native_link`; the user's
  configuration had no corresponding entry or installed RCU plugin.
- After global registration, a follow-up turn in that already-created task
  still reported zero RCU tools. Configuration changes had not refreshed that
  task's catalog. This result remains a failure; it is not credited with the
  later new task's success. The task was archived, not deleted.

## Installation changes

- Registered `mcp_servers.real_computer_use_native_link` in the macOS user's
  `~/.codex/config.toml`, using the official application's `codex mcp add`.
- Installed `bin/rcu-mcp`, `profile.json` and the existing paired client identity
  under `~/Library/Application Support/RealComputerUse/client-v1/`.
- Adapter source commit: `e2f35980f641337b32fc65fc3ca9f594e0863c4e`.
- Adapter SHA-256:
  `631a8ed1ec6dbc940b5574b60fd1e27492178bd4254ba007df5d1cea6586c8c7`.
- Copied only the client's CA certificate, client certificate and client private
  key, preserving their bytes. No agent or CA private key was copied; endpoint,
  device identity, pin and principal authorization were unchanged.
- Installed identity/profile files are private (0600), installation directories
  and executable 0700. The profile has no checkout-dependent paths; the binary
  links only system libraries. No runtime source copy was created.
- Set `enabled = true`, `required = false`, `startup_timeout_sec = 30`, and
  `tool_timeout_sec = 120`. An RCU adapter failure must not make unrelated Host
  work fatal. Offline device behavior remains distinct from adapter startup.
- Removed only the original project's RCU override after validating the user
  registration. Neither validation worktree received local config or credentials.
- The CLI initially normalized four unrelated explicit default fields away:
  `crossmacro_debian.enabled`, `crossmacro_mac.enabled`,
  `crossmacro_windows.enabled`, and `node_repl.args`. The preservation guard
  detected this before removing the project override. Restored those original
  values/bytes and applied only the intended RCU addition. Parsed before/after
  configurations match after excluding the RCU entry.
- Private pre-change configuration backups and an installation manifest are in
  the installation's `rollback/` directory and `install.json`, respectively.
  They are not Git artifacts. A later rollback must retain concurrent user
  changes, not blindly replace the whole user configuration with a backup.

The locally generated plugin was **not** installed globally. No global Skill or
ChatGPT App was installed. Direct user-level MCP is the active integration, not
a claim that every integration form is now delivered.

## Independent fresh-task evidence

Task: **验收全局 RCU MCP**, `01a09390-6394-7d13-8119-460e93c6b804`.

The task was created after registration, in default worktree `af85` at
`afee3d9`. It did not use the engineering checkout or a copied local config.

1. Discovered 20 generic RCU tools, including exactly one `computer.targets`
   and one `computer.operation`, with no `DIAGNOSTIC ONLY` marker.
2. Direct `computer.targets({})` completed. The task reported `win11-vm` available,
   service healthy, device protocol 2, execution reference version 1,
   `context_valid = true`, `workflow_required = false`, `profile_error = null`,
   and the `windows-native-rust` backend.
3. Direct `computer.operation` used the explicit request selector for
   `win11-vm` / `native-general-ui-performance-20260912-1`. The stored request
   returned finished, resources released, `system_accepted = true`,
   `goal_verified = false`, and `replay_allowed = false`. Querying that record
   does not establish the old business goal or run its input again.
4. Worktree-local `.codex/config.toml` was absent. An initial PATH-resolved
   `codex mcp get` failed to parse `model_reasoning_effort = max`; this failure
   is retained. A single follow-up using the application's explicit
   `/Applications/ChatGPT.app/Contents/Resources/codex` succeeded and resolved
   the installed user-level command/profile, with `enabled = true`, `cwd = null`.
   No model setting was changed to work around the unrelated CLI mismatch.
5. The task made no screenshot, activation, application launch, input or replay;
   returned context input-dispatch counters were zero. Its only device calls
   were the two reads above, and they were not repeated for the CLI follow-up.

Tool-call markers in the actual desktop task:

- `exec-d1118747-475f-4cee-82ef-f4d32c6e0ac1`: `computer.targets`, completed.
- `exec-92729a1c-f8a1-4d1c-9c89-9f084f23f19e`: `computer.operation`, completed.
- `exec-861225ae-ed32-4258-8f17-ce1a2876b627`: bundled CLI config read, exit 0.

The saved `read_thread` projection (call metadata, command outputs and task
reports; not an untruncated MCP response archive) is retained privately at
`work/global-mcp-install-20260912/fresh-task-evidence.json`.

## Remaining boundaries

- The older task's catalog did not refresh automatically in this observation.
  A task created after configuration picked up the entry without restarting the
  whole application. This does not establish universal hot-reload behavior.
- Global means the configured local macOS user/Host, not all machines/users or
  a cloud Host. Other projects were not opened or operated as part of this test.
- No native GUI operation, failure/recovery matrix, portable installer, global
  plugin, registered App, process-isolated worker or cross-platform completion
  is claimed by this configuration change.
- No Rust product code changed. Configuration invariants and `git diff --check`
  were checked; Cargo regression suites were not rerun or credited in this turn.

Configuration model reference:
[official OpenAI MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).
