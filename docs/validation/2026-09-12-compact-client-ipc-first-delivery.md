# Compact output, Host helpers and installed REPL/IPC probes

## Delivery status

This is the first implementation increment following the full nine-minute task
analysis. It does not close the full RCU architecture or performance acceptance.

- `d2c4051`: Rust adapter compact output and retained metadata reader.
- `f8ed4a8`: optional Host-side JavaScript helpers, complementary-channel guidance.
- Installed adapter SHA-256:
  `102cd7086c22125ceb8904d4801b4fd07365beed5ffd90b6ac8df8c93b5ab2f2`.
- The user-level installation still points outside the source checkout. Only its
  owned adapter binary/installation record changed. Profile, paired identity,
  Codex registration, Windows agent, firewall and existing running Hosts were
  not changed/interrupted. The old binary is retained in the private rollback
  directory. New adapter processes expose 21 tools; existing Hosts can retain
  their older process/catalog until reloaded.

GUI and terminal are **parallel complementary peers** in one task flow. Either
may execute, verify the other, produce evidence, or inform a subsequent step.
The new client helpers implement the GUI peer only; they do not advertise a
terminal provider, automatically route a denied action through another channel,
or claim that one channel replaces the other.

## Implemented and tested

### Rust model-facing output

The adapter now projects successful observations/targets and exposes local
`computer.result` for retained original metadata. It keeps native operation
identity, errors, cancellation/resource state and uncertainty unchanged.
Large element lists are explicitly paged. Image bytes are not rewritten.

The new reader supports bounded JSON Pointer/array reads and UTF-8 JSON export
of any retained metadata value. It never forwards a device call. Cross-adapter
references fail, expiry/pressure is explicit, and native execution/durable
history is independent of metadata cache lifetime. See the compact-v1 migration
document for the exact compatibility and retention contract.

The original metadata from the user-selected cleanup task was processed offline
through the actual Rust projection code:

| Measurement | Result |
| --- | --- |
| Original normalized metadata | 82,003 bytes |
| Compact metadata | 16,345 bytes |
| Original / initially displayed elements | 63 / 56 |
| Exact full JSON reconstructed from bounded export | yes, 21 pages |
| Native calls made during this replay | 0 |

This is roughly 80% less default metadata in that sample, **not** an 80% latency
improvement. Seven elements require an explicit metadata page; the omitted
elements are not claimed absent. Full task/model correctness and elapsed-time
A/B testing remain necessary, including the cost of any extra detail reads.

### Optional JavaScript helpers

The dependency-free module takes an injected authenticated MCP transport. It
keeps window/observation objects and helper functions in a persistent JS Host,
supports reading omitted elements, and provides claim → callback → release
sequencing with original references retained on failure.

It rejects stale post-action state, stops a batch even if a callback catches an
action failure, never retries a transport error, and waits for an accidentally
unawaited action before cleanup. Release failure does not overwrite the earlier
operation error. Ten offline tests pass, including text-only MCP compatibility
and non-Error transport rejection handling.

This is not a JS sandbox or owned-worker supervisor. It does not guarantee a
wall-clock deadline for arbitrary callback code. Actual Host Stop/REPL transport
integration is not yet deployed, and importing the module does not make
`functions.exec` persistent. No new RCU `js`/SSH tool is falsely advertised.

## Actual installed implementation probes, without modifying Codex

### Installed Mac IPC client over an isolated Unix socket

The test imported `MacComputerUseClient` and its framing code from the actual
installed `@oai/sky` package. A private temporary Unix socket served only explicit
synthetic requests. It was not a replacement for the real Codex socket and did
not contact the native application service.

Observed/asserted:

- Two simultaneous client requests were serialized over one retained connection.
- Frames carried actual JSON-RPC IDs and length-prefix framing.
- A deliberately disconnected request was sent exactly once and rejected.
- The next independent request reconnected; the failed request was not replayed.
- Both fixture connections and the temporary socket were cleaned up.

Evidence: `work/cua-flow-analysis-20260912/installed-ipc-probe.json` and the
reproducible `probe-installed-ipc.mjs`. This confirms JS transport mechanics,
not native UIA, authorization, input behavior or provider-hang recovery.

### Actual installed REPL/MCP process

The test launched the installed `@oai/cua-repl` launcher and `node_repl` with an
isolated HOME and a synthetic trusted Sky service. It used the real `cua` facade,
not a rewritten imitation of its object/REPL behavior.

The real MCP catalog supplied `js`, `js_add_node_module_dir`, `js_reset` and
`turn_ended`. Across separate calls, variables and a bound app object persisted;
a deterministic fixture action plus resulting observation worked in one cell;
reset removed the previous variables. No actual app, user screen, keyboard,
clipboard, native socket or credential was accessed by this test.

Evidence: `work/cua-flow-analysis-20260912/installed-repl-probe.json` and
`probe-installed-repl.mjs`. This verifies actual REPL/facade behavior with fixture
effects, not a full native Codex Computer Use session or model-latency benchmark.

## Why not rely on Proxyman alone

The inspected native route uses Unix-domain IPC, not HTTP. A usable alternative
was implemented here: a framed Unix-socket recorder around the real installed
client, limited to an isolated fixture. This yielded execution evidence without
moving the system socket, altering peer authentication, disabling protections,
or decrypting unrelated HTTP traffic.

For actual native-service profiling, Xcode's `xctrace` is present. Its availability
does not prove attaching to a particular protected service is permitted or will
work. That native experiment has not been run. Kernel/IPC/native timing and the
Host's result-to-next-call timeline must be correlated; packet timing alone
does not explain model interpretation/planning/code-generation delay.

Open-source references evaluated as candidates, not installed dependencies or
blanket maturity endorsements:

- [socat](https://man7.org/linux/man-pages/man1/socat.1.html): documented Unix
  stream relay and directional text/hex/raw capture. A relay changes the peer
  seen by a service, so it cannot be assumed transparent to native authorization.
- [Posit mcp-repl](https://github.com/posit-dev/mcp-repl): persistent interpreter
  workers, explicit idle/interrupt/reset semantics and bounded output with
  detailed bundles. Relevant lifecycle/output ideas, not a reason to implement
  RCU native control through Python or R.
- [Apify mcpc](https://github.com/apify/mcpc): persistent MCP connections and
  progressive tool discovery. Useful client-layer reference; reconnect behavior
  must still be audited against RCU's no-replay contract before adoption.
- [mcp-js](https://github.com/r33drichards/mcp-js): Rust/V8 code-mode candidate.
  Its repository description is not evidence that its runtime/permission model
  meets RCU's requirements. No V8 dependency was added in this increment.

## Regression and live protocol checks

- Required default gates: fmt check, **232 passed / 0 failed / 2 ignored**, and
  workspace/all-targets/all-features Clippy with `-D warnings` passed.
- All-features: **319 passed / 0 failed / 3 ignored**, with the explicit prebuilt
  Rust fixture worker. The saved-metadata ignored test was also run separately
  and passed (1 test). GUI/native opt-in tests are not silently counted as run.
- Initial all-features invocation failed because the existing suite requires
  `RCU_NATIVE_TEST_WORKER`. That failure is preserved in
  `work/compact-v1-20260912/all-features-missing-worker.raw.log`; the suite was
  rerun with a prebuilt fixture worker rather than bypassing its requirement.
- Actual MCP process tests cover metadata reads without additional device calls,
  cross-adapter expiry, text/structured compatibility and existing cancellation.
- Architecture ratchet and staged-source hygiene passed; neither is complete
  architectural acceptance. Skill frontmatter validation passed.
- Both the newly built and then installed production binary were exercised by
  a standalone actual MCP client against the existing paired Windows agent:
  21 tools, `win11-vm` available/healthy, device wire v2, compact targets
  **8,241 → 6,038 bytes**, local metadata read passed, historical durable request
  still `replay_allowed=false`. No input or screenshot occurred.

The latter is protocol/authenticated-device read verification, **not** an actual
Codex model selecting the new tools, nor a repeat of the nine-minute GUI task.
Private live evidence is `work/compact-v1-20260912/live-status-probe.json`.

## Still open

- Wire the persistent client layer into a real user-level RCU REPL/plugin without
  requiring task/project-specific installation or importing Codex private APIs.
- Implement the independently authorized terminal/SSH peer and shared workflow
  evidence/feedback model; no port or SSH service was opened in this increment.
- Instrument and optimize repeated native UIA checks while preserving final
  safety/identity/lease checks, and implement owned-worker isolation.
- Run a controlled full model/task A/B: total wall time, every result-to-next-call
  gap, emitted bytes, code regeneration, lookup/recovery count, correctness,
  uncertainty and verified goal outcome. No overall speed improvement is claimed
  from the byte reduction or the isolated protocol probes alone.
