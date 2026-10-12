// Actual embedded component logic in an isolated VM; not a real Host/native proof.
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';

const html = readFileSync(new URL('../src/gateway/live_ui.html', import.meta.url), 'utf8');
const code = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const drain = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
function harness({ observer = true } = {}) {
  const sent = [], timers = new Map(), intervals = new Map(), listeners = new Map();
  let timerId = 0, intersection;
  const nodes = new Map(['state', 'toggle', 'close', 'panel', 'location'].map(id => [id, {
    disabled: id !== 'panel', textContent: '', events: {}, attrs: {},
    addEventListener(name, fn) { this.events[name] = fn; },
    setAttribute(name, value) { this.attrs[name] = value; }
  }]));
  const listen = prefix => (name, fn) => listeners.set(`${prefix}:${name}`, fn);
  const parent = { postMessage(message, origin) { assert.equal(origin, '*'); sent.push(message); } };
  const window = { parent, addEventListener: listen('window') };
  const document = { visibilityState: 'visible', getElementById: id => nodes.get(id), addEventListener: listen('document') };
  const context = { window, document, console,
    setTimeout: fn => { timers.set(++timerId, fn); return timerId; }, clearTimeout: id => timers.delete(id),
    setInterval: fn => { intervals.set(++timerId, fn); return timerId; }, clearInterval: id => intervals.delete(id)
  };
  if (observer) context.IntersectionObserver = class {
    constructor(fn) { intersection = fn; } observe() {}
  };
  vm.runInNewContext(code, context, { timeout: 1000 });
  const deliver = (message, source = parent) => listeners.get('window:message')({ source, data: { jsonrpc: '2.0', ...message } });
  return { sent, nodes, timers, intervals, document, deliver,
    async init() {
      const init = sent.find(m => m.method === 'ui/initialize');
      await deliver({ id: init.id, result: { protocolVersion: '2026-01-26' } });
      await drain();
    },
    result(cap = { view_key: 'a'.repeat(32), page: 'b'.repeat(32) }) {
      return deliver({ method: 'ui/notifications/tool-result', params: { _meta: { 'rwi/live_panel': cap } } });
    },
    async intersect(shown) { intersection?.([{ isIntersecting: shown, intersectionRatio: shown ? 1 : 0 }]); await drain(); },
    calls() { return sent.filter(m => m.method === 'tools/call'); },
    async ack(call = this.calls().at(-1), data = { accepted: true }) {
      await deliver({ id: call.id, result: { structuredContent: data } }); await drain();
    },
    async hidden() { document.visibilityState = 'hidden'; listeners.get('document:visibilitychange')(); await drain(); },
    async click(id) { nodes.get(id).events.click(); await drain(); },
    async tick() { for (const fn of intervals.values()) fn(); await drain(); }
  };
}

test('no presence without handshake, owned capability and observed intersection', async () => {
  const h = harness();
  await h.result();
  assert.equal(h.calls().length, 0);
  await h.init();
  await h.deliver({ method: 'ui/notifications/tool-result', params: { _meta: { 'rwi/live_panel': { view_key: 'a'.repeat(32), page: 'b'.repeat(32) } } } }, {});
  assert.equal(h.calls().length, 0);
  await h.result(); await drain();
  assert.equal(h.calls()[0].params.arguments.visible, false);
  await h.ack();
  await h.intersect(true);
  assert.equal(h.calls()[1].params.arguments.visible, true);
  assert.equal(h.calls()[1].params.name, 'computer.live_panel');
});

test('hidden event supersedes queued heartbeat; one request in flight and monotonic sequence', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
  assert.equal(h.calls().length, 1);
  await h.tick(); await h.tick(); await h.hidden();
  assert.equal(h.calls().length, 1);
  await h.ack(h.calls()[0]);
  assert.equal(h.calls().length, 2);
  assert.equal(h.calls()[1].params.arguments.visible, false);
  assert.equal(h.calls()[1].params.arguments.sequence, 2);
  await h.ack();
  await h.tick();
  assert.equal(h.calls().length, 2);
  assert.match(h.nodes.get('state').textContent, /隐藏已确认/);
});

test('hide/resume controls publish visibility; close is one bounded non-replayed request', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
  await h.click('toggle');
  assert.equal(h.calls().at(-1).params.arguments.visible, false); await h.ack();
  await h.click('toggle');
  assert.equal(h.calls().at(-1).params.arguments.visible, true); await h.ack();
  await h.click('close');
  const close = h.calls().at(-1);
  assert.equal(close.params.arguments.close, true);
  assert.equal(close.params.arguments.visible, false);
  await h.deliver({ id: close.id, error: { code: -32000, message: 'uncertain' } }); await drain();
  const count = h.calls().length;
  await h.tick(); await h.click('close');
  assert.equal(h.calls().length, count);
  assert.equal(h.intervals.size, 0);
  assert.match(h.nodes.get('state').textContent, /不重试/);
});

test('teardown waits for hidden event then acknowledges; stops heartbeat', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
  const teardown = h.deliver({ id: 91, method: 'ui/resource-teardown', params: { reason: 'task-switch' } });
  await drain();
  assert.equal(h.calls().at(-1).params.arguments.visible, false);
  assert.equal(h.sent.some(m => m.id === 91 && m.result), false);
  await h.ack(); await teardown;
  assert.equal(h.sent.some(m => m.id === 91 && m.result), true);
  assert.equal(h.intervals.size, 0);
});

test('missing intersection API and absent Host fail hidden; resource has no network or input bridge', async () => {
  const h = harness({ observer: false }); await h.init(); await h.result(); await drain(); await h.ack();
  await h.tick(); assert.equal(h.calls().length, 1);
  assert.equal(h.calls()[0].params.arguments.visible, false);
  const missing = harness(); for (const fn of [...missing.timers.values()]) fn(); await drain();
  assert.equal(missing.calls().length, 0); assert.equal(missing.intervals.size, 0);
  assert.doesNotMatch(html, /\bfetch\(|\bWebSocket\b|https?:\/\/|computer\.(?:act|click|hotkey|snapshot)/);
});

test('stopped preview ends heartbeat and cannot misleadingly resume a terminal observation', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
  await h.ack(undefined, { accepted: false, reason: 'preview_stopped' });
  assert.equal(h.intervals.size, 0);
  assert.equal(h.nodes.get('toggle').disabled, true);
  assert.equal(h.nodes.get('close').disabled, true);
  const count = h.calls().length; await h.tick(); await h.click('toggle');
  assert.equal(h.calls().length, count);
  assert.match(h.nodes.get('state').textContent, /新建观察/);
});

test('component declares fullscreen before mount and explains the actual native viewer location', async () => {
  const h = harness();
  const init = h.sent.find(m => m.method === 'ui/initialize');
  assert.deepEqual(Array.from(init.params.appCapabilities.availableDisplayModes), ['fullscreen']);
  await h.init();
  await h.deliver({ method: 'ui/notifications/tool-result', params: {
    _meta: { 'rwi/live_panel': { view_key: 'a'.repeat(32), page: 'b'.repeat(32) } },
    structuredContent: { requested_preview_display: { id: 1, built_in: true } }
  } }); await drain();
  assert.match(h.nodes.get('location').textContent, /内建屏/);
  assert.match(h.nodes.get('location').textContent, /原生浮层/);
});

test('fresh owned capability reopens an ended component without reviving the old view', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
  await h.ack(undefined, { code: 'live_ui_event_rejected' });
  const oldCount = h.calls().length;
  await h.result(); await drain();
  assert.equal(h.calls().length, oldCount, 'same ended capability must not revive');
  await h.result({ view_key: 'c'.repeat(32), page: 'd'.repeat(32) }); await drain();
  assert.equal(h.calls().length, oldCount + 1, 'new owned capability must publish real observed visibility');
  assert.equal(h.calls().at(-1).params.arguments.view_key, 'c'.repeat(32));
  assert.equal(h.calls().at(-1).params.arguments.sequence, 1);
  await h.ack();
  assert.equal(h.intervals.size, 1);
  assert.equal(h.nodes.get('toggle').disabled, false);
});

test('new capability during a closing flight has exactly one live heartbeat', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
  await h.click('close'); const oldClose = h.calls().at(-1);
  await h.result({ view_key: 'c'.repeat(32), page: 'd'.repeat(32) }); await drain();
  await h.deliver({ id: oldClose.id, error: { code: -32000, message: 'old close failed' } }); await drain();
  assert.equal(h.calls().at(-1).params.arguments.view_key, 'c'.repeat(32));
  await h.ack();
  assert.equal(h.intervals.size, 1, 'new view owns one heartbeat even before old close ends');
  assert.equal(h.nodes.get('toggle').disabled, false);
});

test('old async teardown cannot end a newer owned capability', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
  const teardown = h.deliver({ id: 92, method: 'ui/resource-teardown' }); await drain();
  const oldHidden = h.calls().at(-1);
  await h.result({ view_key: 'c'.repeat(32), page: 'd'.repeat(32) }); await drain();
  await h.ack(oldHidden); await h.ack(); await teardown;
  assert.equal(h.sent.some(m => m.id === 92 && m.result), true);
  assert.equal(h.nodes.get('toggle').disabled, false, 'old teardown must not disable the new view');
  assert.equal(h.intervals.size, 1);
});

test('late rejection for a replaced view cannot invalidate the current view', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
  const oldCall = h.calls().at(-1);
  await h.result({ view_key: 'c'.repeat(32), page: 'd'.repeat(32) }); await drain();
  await h.ack(oldCall, { code: 'live_ui_event_rejected' });
  assert.equal(h.calls().at(-1).params.arguments.view_key, 'c'.repeat(32)); await h.ack();
  assert.equal(h.nodes.get('toggle').disabled, false);
  assert.equal(h.intervals.size, 1);
});

test('replacement never asserts visible while the real component is hidden', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
  await h.hidden(); await h.ack();
  await h.result({ view_key: 'c'.repeat(32), page: 'd'.repeat(32) }); await drain();
  assert.equal(h.calls().at(-1).params.arguments.visible, false); await h.ack();
  const count = h.calls().length; await h.tick(); assert.equal(h.calls().length, count);
});

test('rejected view exposes bounded exact reason rather than one ambiguous error', async () => {
  for (const reason of ['view_ended', 'stale_event', 'view_unavailable']) {
    const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
    await h.ack(undefined, { code: 'live_ui_event_rejected', message: reason });
    assert.ok(h.nodes.get('state').textContent.includes(reason), reason);
    assert.equal(h.intervals.size, 0);
  }
});

test('unrecognized rejected reason is not reflected or used as UI markup', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
  await h.ack(undefined, { code: 'live_ui_event_rejected', message: '<img secret-token>' });
  assert.doesNotMatch(h.nodes.get('state').textContent, /img|secret-token/);
  assert.equal(h.nodes.get('toggle').disabled, true);
});

test('different adapter response is diagnostic only and never replays presence or input', async () => {
  const h = harness(); await h.init(); await h.intersect(true);
  await h.result({ view_key: 'a'.repeat(32), page: 'b'.repeat(32), adapter_instance: 'c'.repeat(32) }); await drain();
  await h.ack(undefined, { code: 'live_ui_event_rejected', message: 'view_ended', adapter_instance: 'd'.repeat(32) });
  assert.match(h.nodes.get('state').textContent, /adapter_mismatch/);
  const count = h.calls().length; await h.tick(); await h.click('toggle');
  assert.equal(h.calls().length, count); assert.equal(h.intervals.size, 0);
  assert.equal(h.nodes.get('toggle').disabled, true);
});
