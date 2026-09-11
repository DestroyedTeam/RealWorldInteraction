# M3a.2 independent-review correction contract

Baseline M3a.1 is frozen and NOT approved for live use. This revision is isolated in work/m3a-product-r4/source, version 0.4.0-m3a.4. No VM reservation/native operation is part of this correction.

## Required correctness changes

1. Every observed native closer failure is sticky across nested lifecycle cleanup audits and the active Operation. Launch/activation primary receipts and persisted results must not report resources_released=true when operation cleanup is uncertain. Success classification is blocked; early precommit errors keep the operation state available for the MCP envelope. The independent parent probes are copied into crates/rcu-core/tests/parent_review.rs; parent evidence itself is unchanged.
2. native_call_pending is derived from native_call_depth. Inner guard Drop and unwind cannot clear an outer scope. finish requests completion but cannot erase a live scope or report terminal/released before its final Drop. New scopes are rejected after finish is requested.
3. Every response after Operation registration (success/core error/worker failure) carries a queryable operation_id and an accurate status snapshot. Read computer.operation to follow changes; no status is synthesized from an RPC acknowledgement.
4. computer.lifecycle_preflight is read-only. It returns bounded partial coverage/count/error notes and cleanup state, with no launch/activation/input/reservation. Windows coverage notes aggregate actual fault codes and at most eight HWND examples. Unknown coverage remains incomplete.

## Negative/regression coverage

- Parent temporary-inventory-handle cleanup probe and nested guard probe.
- Nested native scopes, early finish, inner unwind and audit scope restoration.
- Direct controller invocation without an Operation registry still records native cleanup failures.
- Activation final/journal cleanup agreement; launch and activation precommit cleanup errors with no journal reservation.
- Partial/oversized preflight diagnostics preserve count and incomplete status, never dispatch; good preflight is readiness only.
- Actual MCP automatic operation IDs on errors, query-back status, worker panic/cleanup fixtures and read-only preflight exposure.

## Delivery and stage gate

Freeze source after required offline fmt/test/host+Windows ARM64 clippy and release builds, real zero-target MCP smoke and plugin/skill validation. Verify predecessor hashes, source archive and actual build roots. No unit tests, builds or code changes inside any owner window. Only after independent review request a continuous 60-minute native-only slot with final 10 minutes cleanup/handback; insufficient actual budget means reschedule.

No bit-for-bit reproducible-build or live-acceptance claim follows from this contract. Native epoch is not a persistent HWND generation counter, locked image SHA is disk evidence rather than memory integrity, and complete inventory remains conservative on a real desktop.
