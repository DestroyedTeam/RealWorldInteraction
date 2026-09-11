# Real Computer Use

Rust-native, identity-bound computer control through MCP. **Development candidate,
not a cross-platform release.** The root workspace is the single editable product
source; historical `work/*/source` copies are no longer development roots.

## Components

| Crate | Responsibility |
| --- | --- |
| `rcu-core` | Targets, frames, grounded actions, leases, no-replay receipts, verification |
| `rcu-imaging` | Image/view mapping and bounded conversion |
| `rcu-windows`, `rcu-macos` | Native platform capture/input and identity evidence |
| `rcu-link-trust` | Private pairing and TLS identity |
| `rcu-native-link` | Relay/agent lifecycle, guarded task transport, AI-host integration |
| `rcu-mcp` | Official MCP SDK surface and worker process |

## Develop from this root

```sh
rtk cargo fmt --all -- --check
rtk cargo test --workspace --locked --offline
rtk cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
rtk proxy python3 tools/check_architecture.py --workspace . --rules architecture/baseline.json
```

Rust 1.95 or newer is required by the current workspace; use the locked dependency
graph. Remove `--offline` only when an explicit dependency fetch is needed.
Default tests do not control a desktop. A normal MCP launch without a native
selector has zero targets. Native input requires explicit target/grant/lease
binding; a dispatch acknowledgement never proves the requested goal.

## Status and boundaries

- Windows CrossMacro cleanup was separately verified through Rust native MCP,
  using a temporary SDK client. That does not certify the final TLS deployment.
- The native Codex plugin/Skill entry has a real, isolated Codex-host integration
  test over Rust TLS and an explicit fixture worker. It is **not** a Windows or
  model-comprehension benchmark.
- Compact output preserves full raw evidence and original execution guards.
- Full Windows TLS business acceptance, Ordinary/Admin deployment integration,
  remaining module decomposition and registered App integration are still open.

## Engineering and integration

- [Architecture decision and sources](docs/adr/0001-rust-workspace-boundaries.md)
- [Governance, migration and remaining debt](docs/architecture-governance.md)
- [Native Codex MCP/plugin/Skill entry](integrations/codex/README.md)
- [Source consolidation manifest](docs/migrations/2026-09-11-source-consolidation.json)

Git branches and reviewable commits are the version history. `work/`, binaries,
credentials, raw captures and local editor/Codex settings are not source releases.
Earlier milestone documents are historical evidence; the first baseline commit
preserves the old root and its original README.
