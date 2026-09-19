# RWI Host-side client helpers

## Built-in persistent MCP entry

The Rust adapter now exposes `computer.js` and `computer.js_reset`. No external
Node import/transport is needed there: `rwi`, `state`, `emit` and `emitImage` are
preloaded in an owned QuickJS worker. Use `state.*` for persistent objects;
cell-local declarations are intentionally not retained. See
[the runtime contract](../../docs/architecture/persistent-js-entry.md).

```js
// computer.js arguments: {code: "...", timeout_ms: 30000}
state.inventory = await rwi.targets();
emit(state.inventory.targets);
```

Later cells can reuse selected `state.window` objects and the same authenticated
adapter contexts. Every cell remains bounded; native services and direct tools
do not depend on the JS worker. This is a public RWI MCP entry, not registration
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

### Explicit focus resumption

Do not reuse a previous task's foreground assumption. Discover the exact window
again and bind it; binding never focuses. `discover` reports a separate native
`foreground` fact even when a title filter omits that foreground from its rows.
Only `foreground.status === 'known'` supplies `identity` (null means truly none).

```js
const d = await rwi.call('computer.discover', {target, title_contains: title});
// Select one exact returned native window; reject ambiguity/transition states.
await state.window.bind(selection);
await state.window.activate({
  expectedForeground: d.data.foreground.identity,
  requestId: uniqueActivationRequest
});
const current = await state.window.snapshot();
```

Use discovery and activation in the same bounded cell, not across a model turn.
The helper claims/releases a separate lease without treating background input as
ready. It checks the exact native activation receipt and then requires a fresh
observation. Delayed observation replies cannot restore old aliases. A rejected,
yielded or uncertain activation is never automatically repeated; its error keeps
the native receipt/reference and any release error. Actual foreground success is
not a verified application goal. Minimized/newly opening windows remain explicit
states, not permission to choose another similarly named window.

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
ending at 16..5000ms. The Windows executor retains every requested vertex and
uses the advertised `pointer_drag.sample_interval_ms` for optional interpolation
(the current development native build uses 64ms, not the portable 16ms default),
holding/releasing the left button in one native operation. It is not a guaranteed
60fps renderer. The entire path must remain in
the same fixed-geometry foreground window and selected element ancestry.
Cancellation/focus loss/late schedule stops further movement; release-only
cleanup does not replay the path. Failed release remains uncertain/fenced.
Mac drag, cross-window drag, free-standing button-down and an SSH API are not
implemented by this interface. See [the contract](../../docs/architecture/bounded-pointer-trajectories.md).

## Optional integration into another JS Host

`rwi-client.mjs` is an optional JavaScript client layer over an injected,
authenticated MCP transport. It does not implement OS input, spawn a shell,
install a REPL/plugin or provide an SSH service. Rust remains the product backend.

In a persistent JS host, import the module once and supply the Host's MCP client:

```js
const { RwiClient } = await import('/absolute/installation/rwi-client.mjs');
const rcu = new RwiClient((name, args) => mcpClient.callTool({ name, arguments: args }));
```

`mcpClient` above must be the real initialized Host transport, not a fabricated
global. This repository does not register an RWI trusted service in Codex's
private REPL. Importing these helpers does not make `functions.exec` persistent.
The SDK alone does not install a Plugin. The built-in Rust entry above is the
implemented REPL wiring; global Plugin packaging remains separate work.

- `targets()` returns current device status. Use an exact returned target with
  `window(target)`, then `bind(selection)` from native discovery.
- `observe()` returns `{data, media}`; emit the data once and images separately.
  `presentation.elements.next_offset` means the display is paged, not complete.
  `elementsPage(offset)` reads retained metadata without another screenshot and
  admits those original-frame elements for checked actions.
- `withInput(callback, {ttlMs, maxActions, maxDurationMs})` claims once for the prepared callback,
  offers `act({element,intent,value,requestId})` and `observe()`, then releases.
  Await it in the same Host evaluation. Observe after mutation before selecting
  another action. A caught/ignored action failure still poisons the batch.
- Wrappers from one `RwiClient` coordinate per exact target: ownership covers
  claim, native work, reads and cleanup. Inside a batch use the callback's
  `observe`, `snapshot`, and `elementsPage`, not outside window methods. Reads on
  a different target remain independent. Rebinding/observing through another
  wrapper invalidates old local aliases; the service lease remains authoritative
  across other clients and Hosts. Local coordination retains at most 64 targets.
- `maxDurationMs` is a monotonic **scheduling cutoff**, not a hard native kill.
  Default is 25s or less for a short lease; native work, claim and internal reads
  count against it, and TTL reserves cleanup headroom. Known drag duration must
  fit before dispatch. No further step is sent after an expired/invalid clock,
  a duplicate request ID, or an existing deduplicated receipt. Pending reads and
  actions settle before release; Stop remains the Rust supervisor's job.
- Errors retain original replies/execution references and `actionResults`;
  release failure is attached rather than hiding an earlier operation failure.
  There is no retry, automatic reconnection/replay, focus change or goal-success
  inference. Raw Native/MCP authorization and no-replay rules remain authoritative.
- The helper is not a JS sandbox or owned-worker supervisor. Arbitrary callbacks
  can wait indefinitely; native lease expiry remains enforced, and Host Stop/
  transport cancellation must be wired by the actual Host integration. Do not
  claim these helpers alone solve REPL cancellation or native provider hangs.

### Several already-decided steps, one cell

Use the existing bounded callback rather than a stored business workflow or an
implicit state-transition DSL. Every action still requires fresh observation:

```js
await state.window.snapshot();
const batch = await state.window.withInput(async input => {
  for (const step of preparedSteps) {
    const s = await input.snapshot();
    const selected = selectExactObservedControl(s.data, step); // Caller decision.
    if (!selected) throw new Error('State changed; stop, do not guess');
    await input.act({ref: selected.ref, frame: s.data.frame,
      intent: step.intent, value: step.value, requestId: step.requestId});
  }
}, {maxActions: preparedSteps.length, maxDurationMs: 15000, ttlMs: 20000});
// Lease released before final visual feedback/model deliberation.
const after = await state.window.snapshot({preview: true});
```

This is host-side continuous orchestration with device-paced individual drags,
not an atomic device multi-stroke transaction. It does not reuse `e0` across
frames, assume value changes are authorized, or select a new window on failure.
The caller explicitly reads and chooses each next action. `batch.timing` covers
only the callback/claim/cleanup segment, not the whole model task or time between
separate tool calls. Visible focus changes stop input; an unobserved away-and-back
user intervention is not claimed detectable from these snapshots alone.

## Complementary GUI and terminal peers

GUI and terminal are peers in one user-authorized flow, not substitutes or a
fixed preferred/fallback hierarchy. A flow may use GUI execution with terminal
verification, terminal execution with GUI verification, or feedback from both
before the next step. This SDK currently implements only the GUI/MCP peer.
An existing authorized terminal connector can be composed by its caller; no
`rwi.terminal` API is advertised until a real provider exists.

After terminal actions replace a GUI process/window, invalidate the prior
binding and discover/bind/observe anew. Keep each channel's host/user/session,
authorization, effect uncertainty and evidence distinct. A successful exit code
does not establish a visual goal, and a successful click does not establish a
filesystem/process result. Honor explicitly GUI-only acceptance tests.

## Offline tests

```sh
node --test integrations/javascript/rwi-client.test.mjs
```

These tests inject a synthetic MCP function. They validate client sequencing and
failure handling, not Windows execution or overall model latency.
## Readiness rejection

`withInput` requires an observed `mutation_ready: true` and `blocked: false`;
a known input-domain fence also stops it before claim. Every newly observed
step inside a batch is checked again. `RwiInputNotReadyError.data` identifies
`client_precondition`, the frame, readiness flags and retained detail reference.
This rejects known-unready work locally; it never grants native permission,
automatically recovers a fence, observes again or retries prior input. The
server remains authoritative and earlier action receipts/lease cleanup survive.

The canonical worker global is `rwi`. `rcu` remains an alias of the same client for existing cells; it does not allocate a second owner or lease domain.
