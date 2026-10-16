// Actual embedded component logic in an isolated VM; not a real Host/native proof.
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

const html = readFileSync(new URL('../src/gateway/live_ui.html', import.meta.url), 'utf8');
const code = html.match(/<script>([\s\S]*?)<\/script>/)[1];
const drain = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
function harness({ observer = true, deferDecode = false, decodedSize = [1, 1], decodeError = null, denyBlob = false } = {}) {
  const sent = [], timers = new Map(), delays = new Map(), intervals = new Map(), listeners = new Map();
  const blobs = new Map(), revoked = [], decoders = [];
  let timerId = 0, intersection, focused = true;
  const body = { id: '', tagName: 'BODY' };
  const document = { visibilityState: 'visible', activeElement: body, hasFocus: () => focused };
  const nodes = new Map(['state', 'toggle', 'close', 'panel', 'location', 'preview', 'image-state'].map(id => [id, {
    id, tagName: id === 'state' ? 'H1' : ['toggle', 'close'].includes(id) ? 'BUTTON' : 'DIV',
    _disabled: ['toggle', 'close'].includes(id), textContent: '', events: {}, attrs: {}, hidden: id === 'preview', sources: [],
    get src() { return this._src; },
    set src(value) { this._src = value; this.sources.push(value); },
    get disabled() { return this._disabled; },
    set disabled(value) {
      this._disabled = value;
      if (value && document.activeElement === this) document.activeElement = body;
    },
    focus(options) {
      if (this.disabled || (id === 'state' && !/<h1\b[^>]*\btabindex="-1"/.test(html))) return;
      this.focusOptions = options;
      document.activeElement = this;
    },
    addEventListener(name, fn) { this.events[name] = fn; },
    setAttribute(name, value) { this.attrs[name] = value; },
    removeAttribute(name) { if (name === 'src') this._src = undefined; else delete this[name]; delete this.attrs[name]; }
  }]));
  const listen = prefix => (name, fn) => listeners.set(`${prefix}:${name}`, fn);
  const parent = { postMessage(message, origin) { assert.equal(origin, '*'); sent.push(message); } };
  const window = { parent, addEventListener: listen('window') };
  Object.assign(document, { getElementById: id => nodes.get(id), addEventListener: listen('document') });
  const context = { window, document, console,
    setTimeout: (fn, ms) => { timers.set(++timerId, fn); delays.set(timerId, ms); return timerId; },
    clearTimeout: id => { timers.delete(id); delays.delete(id); },
    setInterval: fn => { intervals.set(++timerId, fn); return timerId; }, clearInterval: id => intervals.delete(id),
    Blob, Uint8Array, DataView, atob,
    URL: {
      createObjectURL(blob) { const url = `blob:fixture-${blobs.size + 1}`; blobs.set(url, blob); return url; },
      revokeObjectURL(url) { revoked.push(url); }
    },
    Image: class {
      constructor() { this.naturalWidth = decodedSize[0]; this.naturalHeight = decodedSize[1]; decoders.push(this); }
      decode() {
        if (decodeError) return Promise.reject(decodeError);
        if (denyBlob && this.src?.startsWith('blob:')) return Promise.reject({ name: 'EncodingError', code: 0 });
        return deferDecode ? new Promise((resolve, reject) => { this.resolve = resolve; this.reject = reject; }) : Promise.resolve();
      }
      removeAttribute(name) { delete this[name]; }
    }
  };
  if (observer) context.IntersectionObserver = class {
    constructor(fn) { intersection = fn; } observe() {}
  };
  vm.runInNewContext(code, context, { timeout: 1000 });
  const deliver = (message, source = parent) => listeners.get('window:message')({ source, data: { jsonrpc: '2.0', ...message } });
  return { sent, nodes, timers, delays, intervals, document, deliver, blobs, revoked, decoders,
    setDocumentFocus(value) { focused = value; },
    async init() {
      const init = sent.find(m => m.method === 'ui/initialize');
      await deliver({ id: init.id, result: { protocolVersion: '2026-01-26' } });
      await drain();
    },
    result(cap = { view_key: 'a'.repeat(32), page: 'b'.repeat(32) }) {
      return deliver({ method: 'ui/notifications/tool-result', params: { _meta: { 'rwi/live_panel': cap } } });
    },
    async intersect(shown) { intersection?.([{ isIntersecting: shown, intersectionRatio: shown ? 1 : 0 }]); await drain(); },
    calls() { return sent.filter(m => m.method === 'tools/call' && m.params.name === 'computer.live_panel'); },
    imageCalls() { return sent.filter(m => m.method === 'tools/call' && m.params.name === 'computer.live_panel_image'); },
    async imageAck(value, call = this.imageCalls().at(-1)) {
      await deliver({ id: call.id, result: { _meta: { 'rwi/live_image': { request_sequence: call.params.arguments.sequence, ...value } } } }); await drain();
    },
    async runTimer(ms) {
      const id = [...timers.keys()].find(id => delays.get(id) === ms);
      if (id === undefined) return false;
      const fn = timers.get(id); timers.delete(id); delays.delete(id); fn(); await drain(); return true;
    },
    async ack(call = this.calls().at(-1), data = { accepted: true }) {
      await deliver({ id: call.id, result: { structuredContent: data } }); await drain();
    },
    async hidden() { document.visibilityState = 'hidden'; listeners.get('document:visibilitychange')(); await drain(); },
    async shown() { document.visibilityState = 'visible'; listeners.get('document:visibilitychange')(); await drain(); },
    async event(name) { listeners.get(`window:${name}`)?.(); await drain(); },
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

test('f87 close transfers owned focus before disabling, without waiting for the Host reply', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
  h.nodes.get('close').focus();
  await h.click('close');
  assert.equal(h.document.activeElement, h.nodes.get('state'));
  assert.equal(h.nodes.get('state').focusOptions.preventScroll, true);
  const close = h.calls().at(-1);
  assert.equal(close.params.arguments.close, true);
  await h.ack(close);
  assert.equal(h.document.activeElement, h.nodes.get('state'));
  assert.equal(h.calls().filter(m => m.params.arguments.close).length, 1);
  assert.equal(h.intervals.size, 0);
});

test('f87 terminal expiry preserves a focus destination for the focused toggle', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
  h.nodes.get('toggle').focus();
  await h.ack(undefined, { accepted: false, reason: 'preview_stopped' });
  assert.equal(h.document.activeElement, h.nodes.get('state'));
  assert.match(h.nodes.get('state').textContent, /本轮预览已结束/);
  assert.equal(h.nodes.get('toggle').disabled, true);
  assert.equal(h.nodes.get('close').disabled, true);
});

test('f87 terminal state never steals focus from an unfocused or hidden component', async () => {
  for (const mode of ['unfocused', 'hidden']) {
    const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
    h.nodes.get('toggle').focus();
    if (mode === 'unfocused') h.setDocumentFocus(false);
    else h.document.visibilityState = 'hidden';
    await h.ack(undefined, { accepted: false, reason: 'preview_stopped' });
    assert.equal(h.document.activeElement.tagName, 'BODY', mode);
    assert.equal(h.nodes.get('state').focusOptions, undefined, mode);
  }
});

test('f87 delayed close response does not steal focus after the user moves elsewhere', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
  h.nodes.get('close').focus(); await h.click('close');
  assert.equal(h.document.activeElement, h.nodes.get('state'));
  const outside = { id: 'outside', tagName: 'INPUT' };
  h.document.activeElement = outside;
  await h.ack();
  assert.equal(h.document.activeElement, outside);
});

test('f87 existing outside focus and missing focus APIs never block one close request', async () => {
  for (const mode of ['outside', 'missing-has-focus', 'missing-focus', 'throwing-focus']) {
    const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
    if (mode === 'outside') h.document.activeElement = { id: 'outside', tagName: 'INPUT' };
    else h.nodes.get('close').focus();
    const before = h.document.activeElement;
    if (mode === 'missing-has-focus') delete h.document.hasFocus;
    if (mode === 'missing-focus') h.nodes.get('state').focus = undefined;
    if (mode === 'throwing-focus') h.nodes.get('state').focus = () => { throw new Error('not focusable'); };
    await h.click('close'); await h.ack(); await h.tick(); await h.click('close');
    assert.equal(h.calls().filter(m => m.params.arguments.close).length, 1, mode);
    assert.equal(h.intervals.size, 0, mode);
    if (mode === 'outside') assert.equal(h.document.activeElement, before);
  }
});

// UI-only cached PNG acceptance. This harness is not real Host/render proof.
const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMQMYr6DwACngGgfe0KRgAAAABJRU5ErkJggg==', 'base64');
const image = (sequence = 1, overrides = {}) => ({
  state: 'ready', cursor: { generation: 1, sequence }, mime: 'image/png',
  base64: png.toString('base64'), image_size: [1, 1], png_bytes: png.length,
  png_sha256: createHash('sha256').update(png).digest('hex'), ...overrides
});
async function imageHarness(options) {
  const h = harness(options); await h.init(); await h.intersect(true); await h.result(); await drain(); await h.ack();
  await h.runTimer(500); assert.equal(h.imageCalls().length, 1); return h;
}

test('f122 cache reads require acknowledged visible presence, not handshake or an unacknowledged event', async () => {
  const h = harness(); await h.init(); await h.intersect(true); await h.result(); await drain();
  await h.runTimer(500); assert.equal(h.imageCalls().length, 0);
  await h.ack(undefined, { accepted: false }); await h.runTimer(500);
  assert.equal(h.imageCalls().length, 0);
  await h.tick(); await h.ack(); await h.runTimer(500);
  const args = h.imageCalls()[0].params.arguments;
  assert.deepEqual(JSON.parse(JSON.stringify(args)), { view_key: 'a'.repeat(32), page: 'b'.repeat(32), sequence: 1, after: null });
  assert.equal(h.calls().at(-1).params.arguments.sequence, 2);
  await h.imageAck(image());
  assert.equal(h.nodes.get('preview').hidden, false);
  assert.equal(h.nodes.get('preview').src, 'data:image/png;base64,' + png.toString('base64'));
  assert.match(h.nodes.get('image-state').textContent, /1 × 1 px/);
  assert.equal(h.blobs.size, 0, 'inline PNG needs no object URL permission or registry');
});

test('f122 reads/decode are single-flight and newer cursor replaces the previous source', async () => {
  const h = await imageHarness({ deferDecode: true });
  await h.tick(); await h.ack(); await h.runTimer(500);
  assert.equal(h.imageCalls().length, 1);
  await h.imageAck(image()); await h.runTimer(500);
  assert.equal(h.nodes.get('preview').hidden, true);
  assert.equal(h.imageCalls().length, 1);
  h.decoders[0].resolve(); await drain();
  assert.equal(h.nodes.get('preview').hidden, false);
  await h.runTimer(500);
  assert.deepEqual(JSON.parse(JSON.stringify(h.imageCalls()[1].params.arguments.after)), { generation: 1, sequence: 1 });
  assert.equal(h.imageCalls()[1].params.arguments.sequence, 2);
  await h.imageAck(image(2)); h.decoders[1].resolve(); await drain();
  assert.equal(h.nodes.get('preview').sources.length, 2);
  assert.equal(h.nodes.get('preview').src, 'data:image/png;base64,' + png.toString('base64'));
  assert.deepEqual(h.revoked, []);
  assert.ok(h.decoders.every(d => d.src === undefined), 'decoder sources released after publication');
});

test('f122 hide clears pixels synchronously and a late image cannot revive them; resume requests a fresh cache', async () => {
  const h = await imageHarness(); await h.imageAck(image()); await h.runTimer(500);
  const pending = h.imageCalls().at(-1);
  await h.click('toggle');
  assert.equal(h.nodes.get('preview').hidden, true); assert.equal(h.nodes.get('preview').src, undefined);
  assert.deepEqual(h.revoked, []);
  await h.imageAck(image(2), pending); assert.equal(h.decoders.length, 1);
  await h.ack(); await h.runTimer(500); assert.equal(h.imageCalls().length, 2);
  await h.click('toggle'); await h.ack(); await h.runTimer(500);
  assert.equal(h.imageCalls().at(-1).params.arguments.sequence, 3);
  assert.equal(h.imageCalls().at(-1).params.arguments.after, null);
  await h.imageAck(image(3)); assert.equal(h.nodes.get('preview').hidden, false);
});

test('f122 hiding during decode releases the pending source and discards the decoded result', async () => {
  const h = await imageHarness({ deferDecode: true }); await h.imageAck(image());
  assert.equal(h.decoders.length, 1); await h.hidden();
  assert.equal(h.decoders[0].src, undefined); assert.deepEqual(h.revoked, []);
  h.decoders[0].resolve(); await drain(); await h.runTimer(500);
  assert.equal(h.nodes.get('preview').hidden, true); assert.equal(h.imageCalls().length, 1);
});

test('f122 pagehide, intersection loss, close, cancellation and teardown clear rendered pixels immediately', async () => {
  for (const event of ['pagehide', 'intersection', 'close', 'cancel', 'teardown']) {
    const h = await imageHarness(); await h.imageAck(image());
    if (event === 'pagehide') await h.event('pagehide');
    else if (event === 'intersection') await h.intersect(false);
    else if (event === 'close') await h.click('close');
    else {
      const response = h.deliver({ id: 70, method: event === 'cancel' ? 'ui/notifications/tool-cancelled' : 'ui/resource-teardown' });
      await drain();
      assert.equal(h.nodes.get('preview').hidden, true, event); await h.ack(); await response;
    }
    assert.equal(h.nodes.get('preview').hidden, true, event);
    assert.equal(h.nodes.get('preview').src, undefined, event);
    assert.ok(h.decoders.every(d => d.src === undefined), event);
    assert.deepEqual(h.revoked, [], event);
    await h.runTimer(500); assert.equal(h.imageCalls().length, 1, event);
  }
});

test('f122 capability replacement discards the old image reply and resets only the new image sequence', async () => {
  const h = await imageHarness(); const old = h.imageCalls()[0];
  await h.result({ view_key: 'c'.repeat(32), page: 'd'.repeat(32) }); await drain(); await h.ack();
  await h.imageAck(image(), old); assert.equal(h.decoders.length, 0);
  await h.runTimer(500); const next = h.imageCalls()[1];
  assert.equal(next.params.arguments.view_key, 'c'.repeat(32)); assert.equal(next.params.arguments.sequence, 1);
  await h.imageAck(image(), next); assert.equal(h.nodes.get('preview').hidden, false);
});

test('f122 unknown image endpoint is not polled repeatedly and never falls back to capture/input', async () => {
  for (const mode of ['jsonrpc', 'structured']) {
    const h = await imageHarness(); const call = h.imageCalls()[0];
    await h.deliver({ id: call.id, ...(mode === 'jsonrpc' ? { error: { code: -32601, message: 'unknown' } } :
      { result: { isError: true, structuredContent: { code: 'capability_unavailable' } } }) }); await drain();
    for (let i = 0; i < 4; i++) { await h.tick(); await h.ack(); await h.runTimer(500); }
    assert.equal(h.imageCalls().length, 1); assert.equal(h.nodes.get('preview').hidden, true);
    assert.match(h.nodes.get('image-state').textContent, /不支持/);
    assert.deepEqual([...new Set(h.sent.filter(m => m.method === 'tools/call').map(m => m.params.name))].sort(), ['computer.live_panel', 'computer.live_panel_image']);
    assert.equal(h.nodes.get('close').disabled, false);
  }
});

test('f122 temporary image errors clear stale pixels but leave hide/close operational', async () => {
  const h = await imageHarness(); await h.imageAck(image()); await h.runTimer(500);
  const call = h.imageCalls().at(-1);
  await h.deliver({ id: call.id, result: { isError: true, structuredContent: { code: 'live_image_context_busy' } } }); await drain();
  assert.equal(h.nodes.get('preview').hidden, true); assert.equal(h.nodes.get('close').disabled, false);
  await h.runTimer(500); assert.equal(h.imageCalls().length, 3);
  await h.imageAck(image(3)); assert.equal(h.nodes.get('preview').hidden, false);
});

test('f122 unchanged cursor cannot renew stale pixels or add a hidden image payload', async () => {
  const h = await imageHarness(); await h.imageAck(image()); await h.runTimer(500);
  await h.imageAck({ state: 'unchanged', cursor: { generation: 1, sequence: 1 } });
  assert.equal(h.decoders.length, 1); assert.equal(h.nodes.get('preview').hidden, false);
  assert.equal(await h.runTimer(2000), true);
  assert.equal(h.nodes.get('preview').hidden, true); assert.equal(h.nodes.get('preview').src, undefined);
  await h.runTimer(500); assert.equal(h.imageCalls().at(-1).params.arguments.after, null);
});

test('f122 malformed PNG metadata/header/cursor are never decoded or published', async () => {
  const corruptHeader = Buffer.from(png); corruptHeader.writeUInt32BE(4096, 16);
  const modes = [
    { request_sequence: 900 }, { cursor: { generation: 0, sequence: 1 } },
    { cursor: { generation: 1, sequence: Number.MAX_SAFE_INTEGER + 1 } }, { mime: 'image/svg+xml' },
    { image_size: [2049, 1] }, { image_size: [1, 0] }, { image_size: [1, 1, 1] },
    { png_bytes: 4194305 }, { png_bytes: 23 }, { png_bytes: png.length + 1 },
    { base64: 'http://untrusted' }, { base64: 'A'.repeat(5592412) },
    { base64: corruptHeader.toString('base64') }, { png_sha256: 'not-a-hash' },
    { state: 'unchanged' }, { state: 'unknown' }
  ];
  for (const bad of modes) {
    const h = await imageHarness(); await h.imageAck(image(1, bad));
    assert.equal(h.decoders.length, 0, JSON.stringify(bad).slice(0, 90));
    assert.equal(h.nodes.get('preview').hidden, true);
    assert.equal(h.nodes.get('close').disabled, false);
  }
});

test('f122 decoded dimensions and bounded decode timeout are checked before publication', async () => {
  const wrong = await imageHarness({ decodedSize: [2, 1] }); await wrong.imageAck(image());
  assert.equal(wrong.nodes.get('preview').hidden, true); assert.equal(wrong.decoders[0].src, undefined);
  const h = await imageHarness({ deferDecode: true }); await h.imageAck(image());
  await h.runTimer(1500); assert.equal(h.nodes.get('preview').hidden, true);
  assert.equal(h.decoders[0].src, undefined); assert.deepEqual(h.revoked, []);
  h.decoders[0].resolve(); await drain(); assert.equal(h.nodes.get('preview').hidden, true);
  await h.runTimer(500); assert.equal(h.imageCalls().length, 2);
});

test('f122 image RPC timeout clears the frame and a late timed-out response is ignored', async () => {
  const h = await imageHarness(); const old = h.imageCalls()[0];
  await h.runTimer(3000); await h.imageAck(image(), old);
  assert.equal(h.decoders.length, 0); assert.equal(h.nodes.get('preview').hidden, true);
  await h.runTimer(500); assert.equal(h.imageCalls().length, 2);
  await h.imageAck(image(2)); assert.equal(h.nodes.get('preview').hidden, false);
});

test('f122 a stopped native observation clears pixels and never revives on resume', async () => {
  const h = await imageHarness(); await h.imageAck(image()); await h.tick();
  await h.ack(undefined, { accepted: false, reason: 'preview_stopped' });
  await h.click('toggle'); await h.runTimer(500);
  assert.equal(h.nodes.get('preview').hidden, true); assert.equal(h.imageCalls().length, 1);
  assert.equal(h.nodes.get('preview').src, undefined); assert.deepEqual(h.revoked, []); assert.equal(h.intervals.size, 0);
});

test('f127 image failures expose a bounded exact stage/code, never backend messages or payloads', async () => {
  for (const code of ['unknown_target', 'live_image_unavailable', '<img secret>']) {
    const h = await imageHarness(); const call = h.imageCalls()[0];
    await h.deliver({ id: call.id, result: { isError: true, structuredContent: { code, message: 'PRIVATE_BACKEND_MESSAGE', base64: 'PRIVATE_PIXELS' } } }); await drain();
    const node = h.nodes.get('image-state');
    assert.equal(node.attrs['data-error-stage'], 'cache_rpc');
    assert.equal(node.attrs['data-error-code'], code.startsWith('<') ? 'image_tool_error' : code);
    assert.doesNotMatch(node.textContent, /secret|PRIVATE|<img/);
    await h.click('toggle'); assert.equal(node.attrs['data-error-code'], undefined);
  }
});

test('f127 missing metadata, RPC failure and decode failure remain distinct safe diagnostics', async () => {
  for (const mode of ['metadata', 'rpc', 'decode']) {
    const h = await imageHarness({ decodedSize: mode === 'decode' ? [2, 1] : [1, 1] });
    const call = h.imageCalls()[0];
    if (mode === 'metadata') await h.deliver({ id: call.id, result: { structuredContent: { state: 'ready' } } });
    else if (mode === 'rpc') await h.deliver({ id: call.id, error: { code: -32000, message: 'PRIVATE_MESSAGE' } });
    else await h.imageAck(image());
    await drain(); const node = h.nodes.get('image-state');
    assert.equal(node.attrs['data-error-stage'], {metadata:'validate_envelope',rpc:'cache_rpc',decode:'decode_png'}[mode]);
    assert.equal(node.attrs['data-error-code'], {metadata:'image_meta_missing',rpc:'rpc_-32000',decode:'decode_failed'}[mode]);
    assert.doesNotMatch(node.textContent, /PRIVATE_MESSAGE/);
    await h.runTimer(500); await h.imageAck({state:'unchanged',cursor:{generation:1,sequence:1}});
    assert.equal(h.nodes.get('preview').hidden, true);
  }
});

test('f127 a successful frame clears the old diagnostic without resetting the image sequence', async () => {
  const h = await imageHarness(); await h.imageAck(image(1, {mime:'text/html'}));
  assert.equal(h.nodes.get('image-state').attrs['data-error-code'], 'invalid_image_envelope');
  await h.runTimer(500); assert.equal(h.imageCalls().at(-1).params.arguments.sequence, 2);
  await h.imageAck(image(2)); assert.equal(h.nodes.get('preview').hidden, false);
  assert.equal(h.nodes.get('image-state').attrs['data-error-code'], undefined);
  assert.equal(h.nodes.get('image-state').attrs['data-error-stage'], undefined);
});

test('f131 verified PNG works with the restrictive default CSP without creating Blob URLs', async () => {
  const h = await imageHarness({ denyBlob: true }); await h.imageAck(image());
  assert.equal(h.nodes.get('preview').hidden, false);
  assert.match(h.nodes.get('preview').src, /^data:image\/png;base64,/);
  assert.equal(h.blobs.size, 0); assert.deepEqual(h.revoked, []);
  assert.doesNotMatch(html, /createObjectURL|revokeObjectURL|new Blob\(/);
});

test('f131 decode DOMException numeric code is never mislabeled as a JSON-RPC failure', async () => {
  const h = await imageHarness({ decodeError: { name: 'EncodingError', code: 0, message: 'PRIVATE_MESSAGE' } });
  await h.imageAck(image()); const node = h.nodes.get('image-state');
  assert.equal(node.attrs['data-error-stage'], 'decode_png');
  assert.equal(node.attrs['data-error-code'], 'decode_failed');
  assert.doesNotMatch(node.textContent, /rpc_0|PRIVATE_MESSAGE/);
  assert.equal(h.nodes.get('preview').hidden, true);
});

test('f131 decoded inline PNG remains the exact verified payload and hide releases both sources', async () => {
  const h = await imageHarness({ deferDecode: true, denyBlob: true }); await h.imageAck(image());
  assert.equal(h.decoders[0].src, 'data:image/png;base64,' + png.toString('base64'));
  await h.click('toggle'); assert.equal(h.decoders[0].src, undefined);
  assert.equal(h.nodes.get('preview').src, undefined);
  h.decoders[0].resolve(); await drain(); assert.equal(h.nodes.get('preview').hidden, true);
  assert.equal(h.blobs.size, 0);
});
