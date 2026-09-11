# Real Computer Use

Rust-native, identity-bound computer control through MCP. **Development candidate,
not a cross-platform release.** The root workspace is the single editable product
source; historical `work/*/source` copies are no longer development roots.

The entire product follows [ADR 0002](docs/adr/0002-headless-control-infrastructure-review.md):
**AI Host → MCP adapter → authenticated device RPC → independent headless agent → native capabilities**.
Delivery can be incremental; startup, discovery and communication never require
a business task, test application, window, business stage or prior goal result.

## Components

| Crate | Responsibility |
| --- | --- |
| `rcu-core` | Targets, frames, grounded actions, leases, no-replay receipts, verification |
| `rcu-imaging` | Image/view mapping and bounded conversion |
| `rcu-windows`, `rcu-macos` | Native platform capture/input and identity evidence |
| `rcu-link-trust` | Private pairing and TLS identity |
| `rcu-native-link` | Platform-neutral authenticated device RPC; old workflows are explicitly legacy-gated |
| `rcu-agent` | Independent device service, principal policy, native contexts, shared leases and durable operations |
| `rcu-mcp` | Official MCP SDK adapter, stable catalog, device routing, Host cancellation and presentation |

## Develop from this root

```sh
rtk cargo fmt --all -- --check
rtk cargo test --workspace --locked --offline
rtk cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
rtk proxy python3 tools/check_architecture.py --workspace . --rules architecture/baseline.json
```

Rust 1.95 or newer is required by the current workspace; use the locked dependency
graph. Remove `--offline` only when an explicit dependency fetch is needed.
Default tests do not control a desktop. `rcu-mcp` without `--profile` has zero
paired devices but a complete catalog. Native input requires explicit target/grant/lease
binding; a dispatch acknowledgement never proves the requested goal.

## Status and boundaries

- The 2026-09-12 production entry was tested in an actual Codex Host: 20 compiled
  tools and project Skill discovery, not a diagnostic-only schema fallback.
- An independent Windows ARM64 GUI-subsystem agent was installed and connected
  over authenticated RPC. Two non-document UI clicks were independently observed;
  a new Host queried the old receipt without replaying input. This is a limited
  native acceptance scope, not complete feature/platform or model acceptance.
- Production compact presentation/evidence retrieval is still incomplete; old
  legacy compact-output evidence does not prove the new adapter is efficient.
- Generic application launch policy, full native failure/recovery and privilege
  matrix, other platform integration, remaining legacy-harness migration, actual
  global plugin installation and registered App integration remain open.
- [Current evidence and remaining work](docs/validation/2026-09-12-service-migration-progress.md)

## Engineering and integration

- [Architecture decision and sources](docs/adr/0001-rust-workspace-boundaries.md)
- [Governance, migration and remaining debt](docs/architecture-governance.md)
- [Native Codex MCP/plugin/Skill entry](integrations/codex/README.md)
- [Source consolidation manifest](docs/migrations/2026-09-11-source-consolidation.json)

Git branches and reviewable commits are the version history. `work/`, binaries,
credentials, raw captures and local editor/Codex settings are not source releases.
Earlier milestone documents are historical evidence; the first baseline commit
preserves the old root and its original README.
