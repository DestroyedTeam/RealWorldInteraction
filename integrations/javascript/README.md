# RCU Host-side client helpers

## Built-in persistent MCP entry

The Rust adapter now exposes `computer.js` and `computer.js_reset`. No external
Node import/transport is needed there: `rcu`, `state`, `emit` and `emitImage` are
preloaded in an owned QuickJS worker. Use `state.*` for persistent objects;
cell-local declarations are intentionally not retained. See
[the runtime contract](../../docs/architecture/persistent-js-entry.md).

```js
// computer.js arguments: {code: "...", timeout_ms: 30000}
state.inventory = await rcu.targets();
emit(state.inventory.targets);
```

Later cells can reuse selected `state.window` objects and the same authenticated
adapter contexts. Every cell remains bounded; native services and direct tools
do not depend on the JS worker. This is a public RCU MCP entry, not registration
inside Codex's private CUA REPL. Existing MCP processes need a fresh process to
discover new tools; the adapter cannot change a Host's cached catalog itself.

For concise GUI state, use `const s = await state.window.snapshot(); emit(s.data)`.
Controls use short refs, roles and meaningful flags. Dispatch with
`act({ref, frame:s.data.frame, intent, requestId})` inside `withInput`; a ref without
its exact original frame is rejected. `summary({offset,limit})` is a local page,
while `elementsPage` retrieves missing original elements from retained metadata.
`snapshot({preview:true})` returns a mapped 1024px-long-edge PNG preview in `media`;
no display resolution or original artifact is changed. `crop` uses original
pixels and `maxEdge` can request detail explicitly. Preview mappings remain
authoritative; this does not establish lower whole-task model latency.

### Device-paced drag (advertised Windows capability)

When the target advertises `pointer_drag.implemented`, one checked intent may
carry a bounded path. No per-point MCP loop is needed:

```js
const s = await state.window.snapshot();
// ref is selected from this exact snapshot, not a remembered alias.
await state.window.withInput(input => input.act({
  ref: selectedRef, frame: s.data.frame, requestId: uniqueIntendedDrag,
  intent: { drag: { waypoints: [
    { at_ms: 0, offset: [0, 0] },
    { at_ms: 500, offset: [100, 50] }
  ] } }
}));
```

Offsets are **device physical desktop pixels from the native reference**, not
preview/controller pixels. Use 2..128 vertices with strictly increasing times,
ending at 16..5000ms. The Windows executor retains every vertex and samples each
line at nominal 16ms intervals, holding/releasing the left button in one native
operation. It is not a guaranteed 60fps renderer. The entire path must remain in
the same fixed-geometry foreground window and selected element ancestry.
Cancellation/focus loss/late schedule stops further movement; release-only
cleanup does not replay the path. Failed release remains uncertain/fenced.
Mac drag, cross-window drag, free-standing button-down and an SSH API are not
implemented by this interface. See [the contract](../../docs/architecture/bounded-pointer-trajectories.md).

## Optional integration into another JS Host

`rcu-client.mjs` is an optional JavaScript client layer over an injected,
authenticated MCP transport. It does not implement OS input, spawn a shell,
install a REPL/plugin or provide an SSH service. Rust remains the product backend.

In a persistent JS host, import the module once and supply the Host's MCP client:

```js
const { RcuClient } = await import('/absolute/installation/rcu-client.mjs');
const rcu = new RcuClient((name, args) => mcpClient.callTool({ name, arguments: args }));
```

`mcpClient` above must be the real initialized Host transport, not a fabricated
global. This repository does not register an RCU trusted service in Codex's
private REPL. Importing these helpers does not make `functions.exec` persistent.
The SDK alone does not install a Plugin. The built-in Rust entry above is the
implemented REPL wiring; global Plugin packaging remains separate work.

- `targets()` returns current device status. Use an exact returned target with
  `window(target)`, then `bind(selection)` from native discovery.
- `observe()` returns `{data, media}`; emit the data once and images separately.
  `presentation.elements.next_offset` means the display is paged, not complete.
  `elementsPage(offset)` reads retained metadata without another screenshot and
  admits those original-frame elements for checked actions.
- `withInput(callback, {ttlMs, maxActions})` claims once for the prepared callback,
  offers `act({element,intent,value,requestId})` and `observe()`, then releases.
  Await it in the same Host evaluation. Observe after mutation before selecting
  another action. A caught/ignored action failure still poisons the batch.
- Errors retain original replies/execution references and `actionResults`;
  release failure is attached rather than hiding an earlier operation failure.
  There is no retry, automatic reconnection/replay, focus change or goal-success
  inference. Raw Native/MCP authorization and no-replay rules remain authoritative.
- The helper is not a JS sandbox or owned-worker supervisor. Arbitrary callbacks
  can wait indefinitely; native lease expiry remains enforced, and Host Stop/
  transport cancellation must be wired by the actual Host integration. Do not
  claim these helpers alone solve REPL cancellation or native provider hangs.

## Complementary GUI and terminal peers

GUI and terminal are peers in one user-authorized flow, not substitutes or a
fixed preferred/fallback hierarchy. A flow may use GUI execution with terminal
verification, terminal execution with GUI verification, or feedback from both
before the next step. This SDK currently implements only the GUI/MCP peer.
An existing authorized terminal connector can be composed by its caller; no
`rcu.terminal` API is advertised until a real provider exists.

After terminal actions replace a GUI process/window, invalidate the prior
binding and discover/bind/observe anew. Keep each channel's host/user/session,
authorization, effect uncertainty and evidence distinct. A successful exit code
does not establish a visual goal, and a successful click does not establish a
filesystem/process result. Honor explicitly GUI-only acceptance tests.

## Offline tests

```sh
node --test integrations/javascript/rcu-client.test.mjs
```

These tests inject a synthetic MCP function. They validate client sequencing and
failure handling, not Windows execution or overall model latency.
## Readiness rejection

`withInput` requires an observed `mutation_ready: true` and `blocked: false`;
a known input-domain fence also stops it before claim. Every newly observed
step inside a batch is checked again. `RcuInputNotReadyError.data` identifies
`client_precondition`, the frame, readiness flags and retained detail reference.
This rejects known-unready work locally; it never grants native permission,
automatically recovers a fence, observes again or retries prior input. The
server remains authoritative and earlier action receipts/lease cleanup survive.
