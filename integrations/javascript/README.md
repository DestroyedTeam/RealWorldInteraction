# RCU Host-side client helpers

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
The SDK has offline behavioral tests; production REPL/plugin wiring is still open.

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
