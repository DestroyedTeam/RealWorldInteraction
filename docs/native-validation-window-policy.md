# Native validation window policy

Status: preparation policy only; no VM, screen, window or input permission is allocated by this document.

## Separation of work

1. **Offline preparation**: finish implementation and reviews; run `cargo fmt --all -- --check`, `cargo test --workspace --locked`, and `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (offline when cached); build required macOS and Windows ARM64 artifacts; dry-run orchestration without native operations; prepare bounded scenarios, exact target/application/file permissions, evidence paths, stop conditions and cleanup.
2. **Candidate freeze**: archive the exact source tree and binaries, record hashes and build provenance, and validate package integrity. A code, dependency, script or binary change invalidates readiness and requires offline revalidation before requesting a window.
3. **Owner reservation**: request a continuous **60-minute** native-only slot, with explicit start/end, native identity and allocated windows/actions/files. An available five-minute slot is not sufficient for the full suite. A shorter slot of at least 30 minutes is usable only when its reduced scope and cleanup budget are explicit and adequate; otherwise reschedule. The reservation does not renew automatically or replace owner consent.
4. **Live validation only**: run the frozen candidate against allocated targets, collect native identity/window/event/input/goal evidence, and exercise only the agreed scenarios. No code changes, unit tests, compilation, dependency installation, repackaging or unrelated work during the slot. Distribution, where required, is a prebuilt verified package and remains within explicit owner authorization.
5. **Stop and return**: when a code or orchestration change is needed, preserve failure evidence and safely return the allocation; fix and rerun offline gates before booking a new slot. Do not keep the VM or desktop occupied while debugging offline. Cancellation, identity drift, user intervention or approaching deadline stops new actions.

## 60-minute budget

- 0–5 min: owner handoff, identity/scope/deadline and artifact hash checks.
- 5–40 min: primary native scenarios, direct goal verification and evidence.
- 40–50 min: agreed negative/reuse/cancellation scenarios and evidence completeness.
- 50–60 min: cease new scenarios, release hooks/process handles/leases, close only explicitly owned temporary windows, write result and owner handback. Never kill a shared application.

If setup cannot fit its budget, stop and replan rather than consume the whole slot on preparation. No started input is represented as a verified goal. A setup-only run is setup-only, not application acceptance.

## Independent time limits

An owner reservation (up to 60 min), grant expiry, cooperative per-operation lease, and cancellation/native-call deadline are distinct controls. A short cooperative lease must not be misreported as a five-minute owner reservation or weakened to extend access. All controls remain bounded by the owner end time; only fresh authorization permits another slot.

## Current M3a boundary

M3a is being implemented in `work/m3a-product/source`, isolated from frozen r1/r2/r3 sources and evidence. This policy does not declare that implementation, offline regressions or live acceptance passed. No live booking should be requested until the frozen candidate and offline gate evidence actually exist.
