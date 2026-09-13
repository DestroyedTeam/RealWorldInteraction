# Persistent JS orchestration over the public Rust MCP adapter

Date: 2026-09-12. Implements an adapter-side entry within ADR 0002; it does not
replace the approved architecture or require device-side JS.

## Dependency and ownership boundaries

```text
AI Host ── MCP ── rcu-mcp gateway ── authenticated device RPC ── independent agent
                       │
                       └── owned rcu-mcp --js-worker process
                              rcu-js / QuickJS + canonical JS client SDK
```

The adapter supervises exactly one active interpreter per MCP process. It starts
on first JS use, independent of business task, window, stage, text or oracle.
The worker has no device profile, private keys or inherited environment, and
exposes no filesystem, process, shell, network or module-loading JS API. Its
typed Rust bridge emits only tool calls, caller output and image events; JS
cannot forge a worker-completion message by emitting JSON. Native operations
use the same gateway admission, device validation, Prepare/Execute, cancellation,
presentation and operation-reference paths as direct MCP calls.

`rcu-js` has no dependency on device transport, platform FFI or business tests.
It embeds the one canonical `integrations/javascript/rcu-client.mjs` at build
time. Rust remains the MCP/backend implementation. QuickJS is an interpreter
dependency, not a Python/PowerShell/Node OS bridge. The engine is pinned to
`rquickjs = 0.13.0`, using its standard allocator, interrupt hook and memory/stack
limits; no loader, custom allocator or asynchronous Rust executor features.
See [rquickjs runtime documentation](https://docs.rs/rquickjs/0.13.0/rquickjs/struct.Runtime.html).

This is owned-process containment of **JS**, not an OS security sandbox and not
completion of A01 native-provider process isolation. Rust/native allocations and
an interpreter vulnerability are outside the QuickJS heap limit. A stuck native
agent remains governed by existing native cancellation/uncertainty rules.

## Public contract

The canonical adapter catalog includes device capabilities plus `computer.result`,
`computer.js`, `computer.js_reset`; tool presence does not depend on a selected
window. Device wire protocol remains v2.

`computer.js` accepts an async function body in `code`, optional `timeout_ms`
(default 30000, range 100..60000), and optional `generation`. `state.*` and its
JS objects persist; cell-local `let`/`const` do not. This deliberately avoids
rewriting arbitrary source to invent top-level-await/redeclaration semantics.

Preloaded API:

- `rcu.targets()`, `rcu.call(advertisedTool, args)`, `rcu.result(ref, options)`,
  `rcu.operation(selector)`.
- `rcu.window(exactTarget)` → `bind`, `observe`, `elementsPage`, `withInput`.
- `emit(data)` explicitly emits JSON once. `emitImage(reply.media[i])` emits a
  separate MCP image block. JS return values/tool replies are not auto-dumped.
- `state` holds caller-selected device/window objects between cells. Its
  generation is not an authorization, device identity or execution locator.

Only one cell runs at once; concurrent JS receives `js_busy`, with no queue.
Direct native/status/cancel tools do not acquire the JS mutex. Reset signals the
running cell before waiting for cleanup and destroys this worker only. Successful
reset never claims the device stopped or a business task succeeded.

Limits: 64 KiB source, 64 nested calls, 64 emitted JSON values with 64 KiB combined
JSON payload, four images / 8 MiB combined base64 payload, 64 MiB QuickJS-managed
heap, 512 KiB JS stack. Private IPC is bounded at 32 MiB per frame; tool argument
JSON is bounded at 1 MiB. These are different budgets, not a total OS-memory cap.

All calls must be awaited. Unresolved root promises, queued jobs at cell end,
unhandled promise rejection, exception, OOM, timeout, stop or protocol failure
discard JS state. No jobs are resumed in a later cell. A new cell may rebuild
read-only bindings, but must not repeat uncertain input from the failed cell.

## Cancellation, receipts and cleanup

The parent owns the per-cell call journal before dispatch. Each returned call
retains its stable request ID, exact accepted `operation_ref`, error/status flags
and a `computer.result` metadata reference. Metadata retention is adapter-local
and bounded (300s maximum, earlier eviction under pressure); it is not durable
execution history. Original execution refs remain inline independently of cache
retention. Images are not retained in that metadata cache.

During a native call, Stop/reset/deadline cancels the child RequestContext and
allows the existing cancellation path to return its exact accepted reference.
No further JS-originated device operation is forwarded. A cancellation receipt
does not imply `terminal` or `resources_released`; query the original operation.
The parent never automatically retries an original input or failed JS cell.

During pure JS or IPC wait, the supervisor kills its owned worker on Stop/EOF,
reset or deadline. A dropped evaluation future also owns and kills that worker;
it cannot leave a live interpreter in the reusable slot. The child exits on pipe
EOF when idle. The interpreter interrupt is an additional CPU deadline, not a
substitute for parent process ownership.

The SDK normally releases input leases in `finally`. The parent additionally
tracks leases created in the cell and attempts bounded release if JS throws,
hangs or exits first. Cleanup outcomes are explicit; failure discards JS state.
No lease is intentionally retained across a model deliberation turn. A claim
whose response was lost can remain uncertain; its accepted operation reference
and native expiry/control evidence, not an invented lease ID, are authoritative.
Cell deadline can be followed by up to 3s native cancellation wait, 1s worker
reaping and 2s parallel lease cleanup. These control operations are not new input.

The 2026-09-13 cleanup revision associates a cell's explicitly acknowledged native
cancellations with its own claim receipts. It queries the **full original
execution reference** through the independent control lane; terminal=true,
native_call_pending=false and resources_released=true are required before
reporting quiescence. The shared 2s cleanup budget includes at most 1.5s of
settling and 64 queries across all leases, not a fresh budget per lease. Normal
successful calls require no settling queries. Expiry, identity change, failed
reads and unknown association remain unconfirmed.

A known lease is released through its original context: Prepare must return an
exact same-device/instance/context reference before one Execute is sent. It never
borrows the adapter's newer context. Only an exact context_busy release response
plus a fresh original-execution quiescence check permits one additional release
attempt inside the same deadline. Original call errors, cleanup attempts and
uncertain accepted references remain evidence. No automatic recover_input,
fence clearing or original input replay is performed. Live Host acceptance of
this revision remains separate from the earlier cancellation evidence.

Cleanup jobs themselves belong to Trace. Cancelling an await, resuming cleanup
or dropping Trace transfers/continues the **same** jobs and budget, rather than
reconstructing a new release from the lease token. Even an empty/partially joined
job set cannot authorize a second send. Deterministic held-response tests cover
Drop after the first Execute and after the one qualified second Execute, with
total releases bounded to one and two respectively.

## Complementary channels and measurement

GUI and terminal remain **parallel, complementary peers**. Caller flows may use
either for execution, verification, output, feedback and iteration. This JS
entry currently exposes implemented GUI/device capabilities only. It does not
provide `rcu.terminal`, SSH, `sshd` deployment or a reason to modify port 22.
An already authorized Host terminal connector remains usable alongside it;
local terminal identity must not be confused with guest identity.

Receipts report per-call `elapsed_ms` and aggregate `adapter_elapsed_ms` through
cleanup. They do **not** measure model deliberation, Host queueing, tool-output
reading or the next model/tool transition. Whole-flow acceptance must still use
the correct task's Host start/end and between-batch gaps. No speedup over the
previous 557s task can be asserted from these process/fixture measurements.

Global MCP registration continues to target an installed adapter outside this
checkout. Updating a binary does not update an already-running process or the
Host's cached tool catalog. This runtime is not a globally installed Plugin or
an integration into Codex's private trusted-service REPL.
