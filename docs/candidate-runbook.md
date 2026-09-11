# Candidate packaging and rollback

## What this iteration did

Built source and a **project-local** plugin candidate. No Codex configuration, personal marketplace, installed CrossMacro bridge, permission database, VM or application checkout was changed. The `.mcp.json` command is intentionally an absolute path to this checkout's Rust binary; relocation requires regenerating that path. Portable plugin-root variable support has not been assumed or claimed.

## Prepare without activation

From the repository root:

```sh
rtk cargo build --release -p rcu-mcp --locked
rtk proxy mkdir -p plugins/real-computer-use/bin
rtk proxy cp target/release/rcu-mcp plugins/real-computer-use/bin/rcu-mcp
```

Review `.mcp.json`: no native selector, no `--allow-input`, and an explicit shared candidate state root. Run the plugin validator and stdio integration tests. They do not install the plugin or prove the current Codex host sees its tools.

## Before any real native acceptance

Arrange an exclusive human/automation test window for the exact host/guest/session, document who relinquished input, and specify the reversible goal and restoration oracle. Existing CrossMacro processes do not honor RCU's advisory lock. Do not wake a VM or take control because the plugin was installed.

After explicit authorization, first add only `--macos-window PID:CGWindowID` to a **separate candidate server entry**. Read-only observation still reads screen/AX data and is part of the authorization. Verify the recorded host/session/PID/process epoch, window bounds, screenshot dimensions/normalization, focus and native hit test. Grant OS permissions only as a separately approved setup operation; this server never requests them automatically.

Only after that passes may the candidate entry add `--allow-input`. The skill then uses claim/act/independent verify/release. Use a temporary benign app/document: open a setting, change a reversible value, verify that exact field, apply, verify the result, restore and verify restoration. Include negative cases for wrong window/PID, focus steal, modal change, scroll-stale reference, layout transition, uncertain transport and same-value decoy. Current fixed-window/cross-modal gaps can block this test and must be reported as gaps.

No exact live PID/window values are prefilled in the candidate. No real test was run in this iteration.

## Rollback

1. Stop or remove only the separate candidate MCP entry/plugin connection. Do not rewrite the original CrossMacro setup.
2. Allow an in-flight uncertain operation to settle, then inspect the target independently. Do not replay it during rollback.
3. Keep candidate receipt storage for reconciliation; do not clear it to make a retry succeed. In-memory frames/images disappear on process exit; kernel locks release on process exit.
4. Restore any test-app setting/document changed by the separately authorized acceptance, with its own oracle. This step is currently not applicable because no live input occurred.
5. The frozen reference is not auto-installed as fallback. Any future replacement of installed bridges requires its own explicit backup/hash/restore procedure.
