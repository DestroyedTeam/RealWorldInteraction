# Real World Interaction

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
| `rwi-core` | Targets, frames, grounded actions, leases, no-replay receipts, verification |
| `rwi-imaging` | Image/view mapping and bounded conversion |
| `rwi-windows`, `rwi-macos` | Native platform capture/input and identity evidence |
| `rwi-link-trust` | Private pairing and TLS identity |
| `rwi-native-link` | Platform-neutral authenticated device RPC; old workflows are explicitly legacy-gated |
| `rwi-agent` | Independent device service, principal policy, native contexts, shared leases and durable operations |
| `rwi-mcp` | Official MCP SDK adapter, stable catalog, device routing, Host cancellation and presentation |

## Develop from this root

```sh
rtk cargo fmt --all -- --check
rtk cargo test --workspace --locked --offline
rtk cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
rtk proxy python3 tools/check_architecture.py --workspace . --rules architecture/baseline.json
```

Rust 1.95 or newer is required by the current workspace; use the locked dependency
graph. Remove `--offline` only when an explicit dependency fetch is needed.
Default tests do not control a desktop. `rwi-mcp` without `--profile` has zero
paired devices but a complete catalog. Native input requires explicit target/grant/lease
binding; a dispatch acknowledgement never proves the requested goal.

## Status and boundaries

Current development evidence is dated **2026-10-11**. Earlier milestone reports
are historical, not the current tool count or a full-release certificate.

- The installed gateway advertises **41 model-visible tools**, or **42 when MCP Apps**
  is negotiated (the component-only presence tool is not a model tool). Typed
  snapshot, click, text, key, drag, hover, scroll and placement helpers coexist
  with an owned Rust/QuickJS orchestration worker. JS is optional, not required
  for an ordinary click. Timings are adapter wall time, not native CPU time.
- Production Codex integration is user-scoped. The installed adapter and paired
  client identity remain outside source checkouts. The Plugin groups its MCP
  server and platform Skills; it does not make every tool available on every OS.
- macOS background pointer delivery is PID/window-addressed and does not warp
  the system cursor or explicitly request system frontmost. **AppKit active/key/order may change**;
  client behavior and the actual effect must be independently observed. Controlled
  native drawing, foreground transitions, cancellation and paired release have
  been measured; this is not a guarantee for every application or input framework.
- Foreground/system shortcuts use an explicit original-desktop target, its exact
  lease and a fresh foreground tuple. A child-window lease cannot authorize a
  desktop shortcut. A returned key-post receipt is not a verified window switch.
- Live pixels appear in a native floating panel on an explicitly selected viewer
  display; the control card is not a video player. MCP Apps supplies conversation
  controls without a separate browser tab. A browser-panel compatibility route
  remains for Hosts that do not negotiate Apps. Expiring component presence gates
  visibility; it is **not** an attested Host task ID. Hide, resume, close and bounded
  same-source recovery have native evidence. Window-family compositing remains open.
- Snapshot previews default to a 2048-pixel maximum edge and preserve original
  artifacts for readable crops. AX coverage is bounded and may be incomplete.
  A terminal/released `AXWindows` read failure may retain the error and attempt a
  separate exact-bound-window visual: **no frame, view, or input authority** is
  created. Unknown controls are not reported as absent; images cannot authorize input.
- Latest recorded regression: 722 workspace tests passed, 11 ignored, plus 51
  receiver-fixture and 7 Live-UI tests. These scopes are distinct from native GUI
  acceptance, model routing, throughput and performance. See the
  [current acceptance matrix](docs/product/macos-background-acceptance-status.json)
  and [chronological native evidence](docs/validation/2026-10-10-comprehensive-native-acceptance-and-public-readiness.md).
- **The whole feature/platform acceptance is not complete.** General hidden or
  minimized-window input, mixed/rotated-display mapping, universal hover/tooltips,
  raw game input, cross-app drag-and-drop and experimental private transports are
  not implied by the current tests. During-gesture physical-user shortcut coverage
  and full framework/stress coverage remain open. Windows evidence is limited;
  this is not a cross-platform release.
- Native operations still use same-process `spawn_blocking`. Provider-hang/crash
  isolation in owned workers is not implemented; see the
  [implementation gap](docs/migrations/2026-09-12-owned-worker-isolation-gap.md).

Source and workspace crates retain **GPL-3.0-only**, as declared in Cargo metadata
and [LICENSE](LICENSE). Dependency and third-party material retain their own
licenses; a passing inventory scan is not a blanket legal-compliance conclusion.

## Engineering and integration

- [Architecture decision and sources](docs/adr/0001-rust-workspace-boundaries.md)
- [Governance, migration and remaining debt](docs/architecture-governance.md)
- [Native Codex MCP/plugin/Skill entry](integrations/codex/README.md)
- [Source consolidation manifest](docs/migrations/2026-09-11-source-consolidation.json)

Git branches and reviewable commits are the version history. `work/`, binaries,
credentials, raw captures and local editor/Codex settings are not source releases.
Earlier milestone documents are historical evidence; the first baseline commit
preserves the old root and its original README.

## Naming and existing installations

The product is **Real World Interaction (RWI)**. Cargo packages and binaries use
`rwi-*`, the MCP registration is `real_world_interaction`, and the Skill/Plugin
is `real-world-interaction`. The host-side SDK is `rwi-client.mjs` (`RwiClient`);
embedded JavaScript exposes `rwi`, with `rcu` as the same-object compatibility alias.

Existing paired installations retain their certificate DNS (`rcu-agent.local`),
wire schemas/ALPN, legacy environment variables and durable state/lock identities.
Renaming these would require a coordinated protocol/state migration, not a label
change. Never reset a journal or create a second input domain during renaming.
Historical evidence and raw session archives keep their original names and hashes.

## Codex Plugin

Use the [Real World Interaction Plugin](integrations/codex/plugin/real-world-interaction/README.md) to manage MCP and platform Skills together. Direct MCP integration remains available for other Hosts.
