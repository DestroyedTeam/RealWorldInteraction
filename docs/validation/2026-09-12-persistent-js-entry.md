# Persistent JS MCP entry: implementation and verification

2026-09-12 · implementation `d4ba61685d1060569abf3913e0629481e75ebe0b`

## Delivered

- Public `computer.js` / `computer.js_reset`, available without a business task,
  window, fixture or Codex-private service. The total adapter catalog is 23;
  the device's 20 capabilities and wire v2 are unchanged.
- Owned Rust/QuickJS worker with persistent `state.*` and preloaded canonical
  `RcuClient`. Native calls go through the existing authenticated gateway.
- Parent-owned receipts, bounded execution/output, stop/reset/EOF handling,
  orphan-job rejection and lease cleanup. Direct MCP control remains available
  while JS executes. No replay or native-termination claim is introduced.
- Updated source Skill and SDK documentation. GUI and terminal remain parallel,
  complementary channels; this delivery does not invent an SSH implementation.

See [runtime architecture and contract](../architecture/persistent-js-entry.md).

## Regression evidence

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | passed |
| Default workspace tests, locked/offline | 239 passed, 0 failed, 2 ignored |
| All-feature workspace tests, locked/offline | 326 passed, 0 failed, 3 ignored |
| All-target/all-feature Clippy, `-D warnings` | passed |
| Optional JS SDK Node tests | 10 passed |
| Architecture ratchet / checker negative tests | passed / 4 passed |
| Canonical Skill validation / staged source hygiene | passed |

All-feature tests used an explicit prebuilt actual Rust fixture worker. Ignored
live/manual tests are not included in the pass counts. No Windows desktop input
is inferred from synthetic peers. Initial failures are retained in work evidence:
two Clippy test-allocation issues, and a test assumption that MCP request order
fixed concurrent JS admission. The latter now accepts either winner and checks
that exactly one overlapping cell gets `js_busy`; full gates were rerun.

Actual MCP child + owned interpreter tests cover persistence, generation/reset,
no Node/require/fetch globals, time/memory/output/call bounds, pending promises,
unhandled rejection, nested REPL refusal, Host Stop, EOF and independent status
while JS loops. Typed Rust event construction prevents caller-emitted JSON from
forging worker control frames, even after JS changes `JSON.stringify`.

Actual MCP + authenticated **synthetic device RPC** tests cover the high-level
observe/claim/action/observe/release sequence, parent release after a thrown
exception, and cancellation of a slow accepted operation. Stop retains the exact
execution nonce, sends cancellation, and does not send the next scripted native
call or replay the first Execute. This verifies plumbing, not real UI effects.

## Installed Mac adapter → real Windows agent

Ran the probe against the production release binary, then again against the
atomically updated installed binary, with the existing paired private profile.

- Catalog: 23 tools; Windows `win11-vm` healthy and available, protocol v2.
- First JS cell retained the real target inventory and state.
- Next cell reused the same worker generation/state, read the original durable
  request `native-general-ui-performance-20260912-1`, and read retained metadata.
  The durable result continued to report `replay_allowed: false`.
- Reset succeeded; passing the old JS generation was rejected before running
  the proposed code.
- No screenshot, input, application launch, foreground activation or guest
  configuration change was performed in this live probe.

The installed probe measured 32.36ms and 2.73ms inside the adapter for the two
small read-only cells. These exclude model/Host deliberation and are **not** a
comparison against the 557s CrossMacro task. The whole-flow model benchmark has
not yet been repeated with this entry.

## Deployment boundary and rollback

Installed adapter SHA-256:
`da29cc8a511e3a18eddc61b71aecdd3822b906af49168700d690ef09635b3886`.

The binary remains in the existing user-level `RealComputerUse/client-v1/bin`
installation, outside the repository. Only that binary and its installation
manifest changed. Hash guards confirmed the Codex global configuration, private
profile and three identity files were unchanged. The Windows agent, firewall,
grants and device protocol were not modified. The preceding adapter and manifest
are preserved under `client-v1/rollback/rcu-mcp-before-js-v1` and
`install-before-js-v1.json`.

Existing MCP processes were not interrupted. A newly launched process exposes
the 23-tool catalog; this does not prove the current Codex task's cached catalog
has refreshed. No fresh model task was created in this increment, no Codex
restart was forced, and no global Plugin was installed.

Remaining: fresh model-side whole-flow timing, actual GUI acceptance through
`computer.js`, complementary terminal-provider implementation, cross-platform
JS-worker compile validation and the separate A01 native-worker isolation work.
Full ADR 0002 delivery is not claimed complete.

Machine-readable evidence: [validation summary](2026-09-12-persistent-js-entry.json).
