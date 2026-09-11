# Codex native MCP / plugin / Skill

The production path is `Codex → Rust stdio relay → private authenticated TLS → Rust worker`.
No Python or shell process implements native input/capture.

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
