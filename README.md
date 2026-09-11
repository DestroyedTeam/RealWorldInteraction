> 当前开发候选为 M1 `0.2.0`，参见 [Windows 原生观察与映射](docs/m1-windows-observe.md)。下方 v0 材料及原始 `outputs/verification.json` 是冻结历史证据，不代表 M1 或 Windows live 已验收。

# Real Computer Use — Rust candidate

A goal-directed, target/session-aware computer-use MCP, plugin and skill. **This is an independently buildable first candidate, not a cross-platform release or live acceptance result.**

## Runtime

| Component | Implemented now | Verification boundary |
|---|---|---|
| `rcu-core` | Target identities, frame/element references, shared desktop locks, durable at-most-once receipts, scoped verification, bounded in-memory artifacts | Synthetic app workflows and actual cross-process lock tests |
| `rcu-mcp` | Official `rmcp` SDK, stdio lifecycle, 8 typed tools, initialization instructions, explicit zero-target default | Actual compiled process initialization/discovery/error/EOF tests |
| `rcu-macos` | Rust FFI to ScreenCaptureKit, CoreGraphics and Accessibility; optional native input | Compiled; pure/offscreen framework tests only; **live pending** |
| Plugin/skill | Project-local manifest, Rust binary command, goal/recovery guidance | Static validation + binary stdio tests; not installed globally |
| Windows / Linux / remote / VM | Platform and authenticated transport contracts in roadmap | **Not implemented; no parity claim** |

The new source root is independent of the external CrossMacro C# reference. Production code does not launch Python, .NET, PowerShell, Swift, AppleScript, `prlctl` or shell input helpers. The benchmark example alone launches the Rust MCP executable for process measurements.

## Build and offline checks

Run from this repository root with Rust 1.95.0 and the platform SDK:

```sh
rtk cargo build --release -p rcu-mcp --locked
rtk cargo test --workspace --locked
rtk cargo fmt --all -- --check
rtk cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

`Cargo.lock` pins the actually compiled dependency graph. `rmcp=1.7.0` is the cached official SDK version used in this candidate, **not a claim to use the newest SDK**. A fresh dependency download of a newer version failed in this environment; no network fallback is required by the binary itself.

Launch the server with an absolute state path. Without native selectors it registers no targets:

```sh
rtk proxy ./target/release/rcu-mcp --state-dir "$PWD/work/manual-state"
```

This is a stdio MCP endpoint, not a human-readable interactive shell. Configure an MCP client or run the integration tests; do not type arbitrary commands into it.

## Next documents

- [Architecture and current limitations](docs/architecture.md)
- [Candidate packaging, activation gate and rollback](docs/candidate-runbook.md)
- [Evidence and historical failure mapping](docs/evidence.md)
- [Measured performance](docs/performance.md)

No release, commit, push, install, VM wakeup, permission change or live desktop acceptance is implied by these checks.
