/** Host-side helpers over an injected real MCP client. No OS or shell bridge. */
const asError = reason => reason instanceof Error ? reason : new Error(String(reason), { cause: reason });
// At most one owned read/action is in flight. Keep its failure even if a local
// concurrency rejection was already latched; never hide native cleanup evidence.
function rememberFailure(state, error) {
  if (state.failure && state.failure !== error) state.failure.pendingError ??= error;
  else state.failure ??= error;
}
export class RcuToolError extends Error {
  constructor(tool, reply) {
    let data = reply.structuredContent;
    if (data == null) {
      const text = reply.content?.find(c => c.type === 'text')?.text;
      try { data = JSON.parse(text); } catch { data = { message: text }; }
    }
    data ??= {};
    const failure = data.dispatch_error ?? {};
    super(`${tool}: ${data.code ?? failure.code ?? 'operation_failed'}: ${data.message ?? failure.message ?? failure.next_step ?? 'inspect retained reply'}${data.native_dispatch_entered === false ? ' [native input not dispatched]' : ''}`);
    this.name = 'RcuToolError';
    this.tool = tool;
    this.reply = reply;
    this.data = data;
  }
}

/** Local rejection only. This is not a native receipt or an authorization grant. */
export class RcuInputNotReadyError extends Error {
  constructor(observation) {
    const scene = observation.scene ?? {};
    const data = { code: 'client_observation_not_ready', source: 'client_precondition', frame: observation.frame, mutation_ready: scene.mutation_ready ?? null, blocked: scene.blocked ?? null, input_domain_fenced: observation.input_domain_fenced ?? null, details_ref: observation.presentation?.result_ref ?? null, native_dispatched: false };
    super(`${data.code}: mutation_ready=${data.mutation_ready}, blocked=${data.blocked}, input_domain_fenced=${data.input_domain_fenced}; no action sent for this attempt. Observe the changed state or reconcile the fence; never replay previous input.`);
    this.name = 'RcuInputNotReadyError';
    this.data = data;
  }
}

export class RcuActivationError extends Error {
  constructor(reply) {
    const data = reply.data;
    super(`Window activation ${data.state ?? 'unconfirmed'}; observe the actual foreground before another explicit action. No activation or input is replayed.`);
    this.name = 'RcuActivationError'; this.data = data; this.reply = reply;
  }
}

export class RcuClient {
  #callTool;
  #targets = new Map();
  constructor(callTool) {
    if (typeof callTool !== 'function') throw new TypeError('Inject an authenticated MCP callTool function');
    this.#callTool = callTool;
  }
  async call(tool, args = {}) {
    const reply = await this.#callTool(tool, args); // Never retry transport failures.
    if (reply?.isError) throw new RcuToolError(tool, reply);
    let data = reply?.structuredContent;
    if (data == null) {
      const text = reply?.content?.find(c => c.type === 'text')?.text;
      try { data = JSON.parse(text); } catch { throw new Error(`${tool}: invalid MCP JSON result`); }
    }
    if (data?.dispatch_error) throw new RcuToolError(tool, reply);
    return { data, media: (reply.content ?? []).filter(c => c.type !== 'text') };
  }
  async targets() { return (await this.call('computer.targets')).data; }
  window(target) {
    if (typeof target !== 'string' || !target) throw new TypeError('Use an exact returned target');
    if (!this.#targets.has(target) && this.#targets.size >= 64) throw new Error('Target wrapper limit; discard the client/JS worker rather than losing active ownership');
    if (!this.#targets.has(target)) this.#targets.set(target, { owner: null, version: 0, ownership: 0 });
    return new RcuWindow(this, target, this.#targets.get(target));
  }
  async result(resultRef, options = {}) {
    return (await this.call('computer.result', { ...options, result_ref: resultRef })).data;
  }
  async operation(selector) { return (await this.call('computer.operation', { selector })).data; }
}

class RcuWindow {
  #client;
  #target;
  #observation = null;
  #elements = new Map();
  #aliases = new Map();
  #active = null;
  #selection = null;
  #revision = 0;
  #coordinator;
  #seenVersion;
  constructor(client, target, coordinator) {
    this.#client = client; this.#target = target; this.#coordinator = coordinator; this.#seenVersion = coordinator.version;
  }
  #access(owner) {
    if (this.#coordinator.owner && this.#coordinator.owner !== owner) throw new Error('Do not access the target concurrently: an operation is active; replacing binding during it is forbidden. Use the owner-bound callback APIs');
  }
  #current(reply) {
    if (this.#seenVersion !== this.#coordinator.version) {
      this.#observation = null; this.#elements.clear(); this.#aliases.clear(); this.#selection = null;
      const error = new Error('Target state was superseded by another wrapper; observe/bind anew, never replay');
      error.reply = reply; throw error;
    }
  }
  #readStamp(owner) { this.#access(owner); return this.#coordinator.ownership; }
  #requireReadStamp(stamp, owner, reply) {
    if (stamp !== this.#coordinator.ownership) {
      const error = new Error('Read ownership was superseded while awaiting; obtain the current state, never replay');
      error.reply = reply; throw error;
    }
    this.#access(owner);
  }
  #own(state) { this.#access(null); this.#coordinator.owner = state; this.#coordinator.ownership++; this.#active = state; }
  #disown(state) {
    if (this.#coordinator.owner === state) { this.#coordinator.owner = null; this.#coordinator.ownership++; }
    if (this.#active === state) this.#active = null;
  }
  #invalidate() {
    if (this.#seenVersion !== this.#coordinator.version) this.#selection = null;
    this.#observation = null; this.#elements.clear(); this.#aliases.clear();
    this.#seenVersion = ++this.#coordinator.version;
    return [++this.#revision, this.#seenVersion];
  }
  #requireRevision(revision, reply) {
    if (revision[0] !== this.#revision || revision[1] !== this.#coordinator.version) {
      const error = new Error('Observation or binding was superseded while awaiting; obtain the current state. No operation is replayed.');
      error.reply = reply; throw error;
    }
  }
  async bind(selection) {
    this.#access(null);
    if (this.#active) throw new Error('Do not replace a binding during an input batch');
    const revision = this.#invalidate();
    this.#selection = null;
    const selected = Object.fromEntries(['window','pid','native_epoch','relation'].map(k => [k, selection[k]]));
    const state = { inFlight: true }; this.#own(state);
    try {
      const reply = await this.#client.call('computer.bind_window', { target: this.#target, selection: selected });
      this.#requireRevision(revision, reply);
      const result = reply.data;
      this.#selection = selected;
      return result;
    } finally { this.#disown(state); }
  }
  /** Explicit focus operation, not input on an observed background window. */
  async activate({ expectedForeground, requestId, authorizationId = 'selected-window', ttlMs = 5000 } = {}) {
    this.#access(null); this.#current();
    if (this.#active) throw new Error('Another input or activation batch is active');
    if (!this.#selection) throw new Error('Bind a fresh exact window before activation');
    if (expectedForeground === undefined || !requestId || typeof requestId !== 'string' || !Number.isInteger(ttlMs) || ttlMs < 100 || ttlMs > 60000) throw new TypeError('Supply a stable requestId, explicit fresh expectedForeground (null only when known absent), and bounded ttlMs');
    if (expectedForeground !== null && (typeof expectedForeground !== 'object' || typeof expectedForeground.window !== 'string' || !Number.isInteger(expectedForeground.pid) || expectedForeground.pid <= 0 || typeof expectedForeground.process_created !== 'string' || !/^\d+$/.test(expectedForeground.process_created) || typeof expectedForeground.native_epoch !== 'string')) throw new TypeError('Use the exact lossless foreground tuple from fresh discovery');
    const foreground = expectedForeground === null ? null : Object.fromEntries(['window', 'pid', 'process_created', 'native_epoch'].map(k => [k, expectedForeground[k]]));
    const selected = this.#selection;
    this.#invalidate();
    const state = { inFlight: true }; this.#own(state);
    let lease, value, release, failure;
    try {
      const claimed = (await this.#client.call('computer.claim', { target: this.#target, ttl_ms: ttlMs })).data;
      if (typeof claimed.lease !== 'string') throw new Error('Claim did not return a lease');
      lease = claimed.lease;
      value = await this.#client.call('computer.activate_window', { target: this.#target, authorization_id: authorizationId, expected_foreground: foreground, request_id: requestId, lease });
      if (!['foreground_observed', 'already_foreground'].includes(value.data.state) || value.data.user_intervened !== false || value.data.error || value.data.resources_released !== true || value.data.resource_release_uncertain !== false || !['window', 'pid', 'native_epoch'].every(k => value.data.window?.[k] === selected[k]) || !['window', 'pid', 'native_epoch', 'process_created'].every(k => value.data.after_foreground?.[k] === value.data.window?.[k])) throw new RcuActivationError(value);
    } catch (error) { failure = asError(error); }
    finally {
      if (lease) {
        try { release = await this.#client.call('computer.release', { lease }); }
        catch (error) { if (failure) failure.releaseError = asError(error); else failure = asError(error); }
      }
      this.#disown(state);
    }
    if (failure) { failure.activationRequestId = requestId; failure.activationResult = value ?? null; throw failure; }
    return { value, release }; // No observation or input authority is carried across the focus change.
  }
  async observe(options = {}) { return this.#observe(options, null); }
  /** Response-only visual feedback. Does not create, renew or replace input frames. */
  async captureVisual({ maxEdge = 1024, feedback = 'exclude' } = {}) {
    this.#access(null); this.#current();
    if (!Number.isInteger(maxEdge) || maxEdge < 64 || maxEdge > 2048 || !['exclude', 'include'].includes(feedback)) throw new TypeError('Use maxEdge 64..2048 and feedback exclude/include');
    return this.#client.call('computer.capture_visual', { target: this.#target, max_edge: maxEdge, feedback });
  }
  async #observe(options = {}, owner) {
    const ownership = this.#readStamp(owner);
    if (this.#active?.inFlight) throw new Error('Do not observe concurrently with a pending action');
    const revision = this.#invalidate();
    const result = await this.#client.call('computer.observe', { ...options, target: this.#target, inline_image: options.inline_image ?? false });
    this.#requireReadStamp(ownership, owner, result);
    this.#requireRevision(revision, result);
    if (!result.data.frame || !Array.isArray(result.data.scene?.elements)) throw new Error('Observation lacks an exact frame/element list');
    this.#observation = result.data;
    this.#elements = new Map(result.data.scene.elements.map(e => [e.id, e]));
    this.#aliases = new Map([...this.#elements.keys()].map((id, i) => [`e${i}`, id]));
    return result; // Text once via data; images explicitly via media.
  }
  summary({ offset = 0, limit = 64 } = {}) {
    this.#current();
    const o = this.#observation;
    if (!o) throw new Error('Observe before reading a summary');
    if (!Number.isInteger(offset) || offset < 0 || offset > this.#aliases.size || !Number.isInteger(limit) || limit < 1 || limit > 128) throw new TypeError('Use a bounded summary page');
    const rows = [...this.#aliases];
    const coverageReason = o.adapter_status?.native_details?.uia?.walk?.truncation ?? o.adapter_status?.native_details?.uia?.error?.code;
    const elements = rows.slice(offset, offset + limit).map(([ref, id]) => {
      const e = this.#elements.get(id);
      const row = { ref, name: e.name ?? '' };
      if (e.control_type !== undefined) row.role = controlRole(e.control_type);
      if (e.keyboard_focus === true) row.focused = true;
      if (e.visible === false) row.hidden = true;
      if (e.occluded === true) row.occluded = true;
      if (e.visible === null || e.occluded === null) row.unconfirmed = true;
      return row;
    });
    return {
      target: this.#target,
      frame: o.frame,
      window: o.adapter_status?.native_details?.window ? Object.fromEntries(['title','pid','window','native_epoch'].filter(k => o.adapter_status.native_details.window[k] !== undefined).map(k => [k,o.adapter_status.native_details.window[k]])) : null,
      elements,
      coverage: { loaded: rows.length, returned: elements.length, next_offset: offset + elements.length < rows.length ? offset + elements.length : null, native_total: o.presentation?.elements?.total ?? null, native_complete: o.presentation?.elements?.native_complete ?? null, adapter_next_offset: o.presentation?.elements?.next_offset ?? null, reason: coverageReason == null ? null : String(coverageReason).slice(0, 240) },
      blocked: o.scene.blocked ?? null,
      mutation_ready: o.scene.mutation_ready ?? null,
      assurance: o.scene.assurance ?? null,
      atomicity: o.scene.atomicity ?? null,
      input_domain_fenced: o.input_domain_fenced ?? null,
      details_ref: o.presentation?.result_ref ?? null,
      ref_contract: 'Use act({ref, frame, intent, requestId}); refs belong to this exact frame. Page retained metadata for missing controls; incomplete is not absent.',
    };
  }
  async snapshot(options = {}) { return this.#snapshot(options, null); }
  async #snapshot({ preview = false, maxEdge = 1024, crop, offset = 0, limit = 64, ...options } = {}, owner) {
    const ownership = this.#readStamp(owner);
    if (typeof preview !== 'boolean' || !Number.isInteger(maxEdge) || maxEdge < 64 || maxEdge > 2048) throw new TypeError('Use preview boolean and maxEdge 64..2048');
    const observed = await this.#observe({ ...options, inline_image: false }, owner);
    this.#requireReadStamp(ownership, owner, observed);
    const data = this.summary({ offset, limit });
    let media = [];
    if (preview) {
      const result = await this.#client.call('computer.image', { target: this.#target, frame: observed.data.frame, artifact_id: observed.data.artifact_id, max_edge: maxEdge, ...(crop ? { crop } : {}) });
      this.#requireReadStamp(ownership, owner, result);
      this.#current(result);
      if (this.#observation !== observed.data) { const error = new Error('Observation changed while preparing preview'); error.reply = result; throw error; }
      data.preview = Object.fromEntries(['view_id','original_artifact_id','original_sha256','view_sha256','mapping'].map(k => [k, result.data[k]])); // Native execution details remain in the Host call trace.
      media = result.media;
    }
    return { data, media };
  }
  async elementsPage(offset, limit = 16) { return this.#elementsPage(offset, limit, null); }
  async #elementsPage(offset, limit = 16, owner) {
    const ownership = this.#readStamp(owner); this.#current();
    const observation = this.#observation;
    const reference = observation?.presentation?.result_ref;
    if (!reference) throw new Error('This observation has no retained metadata reference');
    const page = await this.#client.result(reference, { pointer: '/scene/elements', offset, limit });
    this.#requireReadStamp(ownership, owner, page);
    this.#current(page);
    if (this.#observation !== observation || page.result_ref !== reference || page.source_tool !== 'computer.observe' || !Array.isArray(page.value)) { const error = new Error('Retained element page does not match this observation'); error.reply = page; throw error; }
    for (const element of page.value) {
      if (typeof element?.id !== 'string') throw new Error('Invalid retained element');
      this.#elements.set(element.id, element);
      if (![...this.#aliases.values()].includes(element.id)) this.#aliases.set(`e${this.#aliases.size}`, element.id);
    }
    return page;
  }
  #requireInputReady() {
    this.#current();
    const o = this.#observation;
    if (o.scene.mutation_ready !== true || o.scene.blocked !== false || o.input_domain_fenced === true) throw new RcuInputNotReadyError(o);
  }
  async withInput(callback, { ttlMs = 30000, maxActions = 16, maxDurationMs = Math.min(25000, ttlMs - Math.min(1000, Math.floor(ttlMs / 2))) } = {}) {
    const started = performance.now();
    if (!Number.isFinite(started)) throw new Error('Input batch requires a finite monotonic clock');
    this.#access(null); this.#current();
    if (this.#active) throw new Error('An input batch is already active on this binding');
    if (typeof callback !== 'function' || !Number.isInteger(ttlMs) || ttlMs < 100 || ttlMs > 60000 || !Number.isInteger(maxActions) || maxActions < 1 || maxActions > 64) throw new TypeError('Use a bounded input batch');
    if (!Number.isInteger(maxDurationMs) || maxDurationMs < 1 || maxDurationMs > Math.min(55000, ttlMs - Math.min(1000, Math.floor(ttlMs / 2)))) throw new TypeError('Use a bounded scheduling duration with lease cleanup headroom');
    let lastTime = started;
    const checkBudget = (scheduledMs = 0) => {
      const now = performance.now();
      if (!Number.isFinite(now) || now < lastTime || now + scheduledMs >= started + maxDurationMs) throw new Error('Input batch scheduling deadline or monotonic clock violated; no further action may be dispatched, in-flight work is not abandoned');
      lastTime = now;
    };
    if (!this.#observation) throw new Error('Observe before preparing an input batch');
    this.#requireInputReady(); // Known blocked/unknown state must not acquire a lease.
    // Reserve locally before awaiting claim, so concurrent batches cannot race.
    const state = { lease: null, actions: 0, closed: false, inFlight: false, reading: false, failure: null, pending: new Set(), requestIds: new Set(), actionResults: [] };
    this.#own(state);
    let value, failure, release;
    try {
      const claim = await this.#client.call('computer.claim', { target: this.#target, ttl_ms: ttlMs });
      if (typeof claim.data.lease !== 'string') { const error = new Error('Claim did not return a lease'); error.reply = claim; throw error; }
      state.lease = claim.data.lease;
      try { checkBudget(); } catch (reason) { const error = asError(reason); error.reply = claim; throw error; }
      const perform = async ({ element, ref, frame: expectedFrame, intent, value, requestId }) => {
        if (state.closed || this.#active !== state) throw new Error('Input handle expired');
        if (state.failure) throw state.failure;
        checkBudget();
        if (!requestId || typeof requestId !== 'string' || state.requestIds.has(requestId)) throw new TypeError('Each intended action needs a unique stable requestId; never reuse an earlier action');
        if (intent && typeof intent === 'object') intent = JSON.parse(JSON.stringify(intent));
        if (intent?.drag) {
          const duration = intent.drag.waypoints?.at(-1)?.at_ms;
          if (!Number.isInteger(duration) || duration < 16 || duration > 5000) throw new TypeError('Use a bounded typed drag duration');
          checkBudget(duration);
        }
        if (!this.#observation) throw new Error('Observe the resulting state before the next action');
        this.#requireInputReady(); // Also applies to each freshly observed step inside a batch.
        if (ref !== undefined) {
          if (element !== undefined || expectedFrame !== this.#observation.frame || !this.#aliases.has(ref)) throw new Error('Summary ref requires its exact observed frame and no element override');
          element = this.#aliases.get(ref);
        } else if (expectedFrame !== undefined && expectedFrame !== this.#observation.frame) throw new Error('Observed frame changed');
        if (state.actions >= maxActions) throw new Error('Input batch action limit');
        if (!this.#elements.has(element)) throw new Error('Select an element from this observation or retrieve its omitted page first');
        const frame = this.#observation.frame;
        // Invalidate immediately, including on uncertain failure. Never reuse stale state.
        this.#invalidate();
        state.actions++; state.requestIds.add(requestId);
        const record = { requestId, reply: null };
        state.actionResults.push(record);
        let result;
        try { result = await this.#client.call('computer.act_checked', { target: this.#target, frame, element, intent, value, lease: state.lease, request_id: requestId }); }
        catch (error) { record.reply = error?.reply ?? null; throw error; }
        record.reply = result;
        if (result.data.deduplicated === true) throw new RcuToolError('computer.act_checked', { structuredContent: { ...result.data, code: 'deduplicated_batch_stop', message: 'An existing receipt is not a new sequence step; inspect the original request, never continue implicitly' }, isError: true });
        checkBudget();
        if (result.data.system_accepted !== true) throw new RcuToolError('computer.act_checked', { structuredContent: result.data, isError: true });
        return result;
      };
      const act = args => {
        if (state.closed) return Promise.reject(new Error('Input handle expired'));
        if (state.inFlight || state.reading) {
          state.failure ??= new Error('Concurrent input is not permitted');
          return Promise.reject(state.failure);
        }
        state.inFlight = true;
        const pending = perform(args).catch(reason => { const error = asError(reason); rememberFailure(state, error); throw error; }).finally(() => { state.inFlight = false; });
        state.pending.add(pending);
        pending.then(() => state.pending.delete(pending), () => state.pending.delete(pending));
        return pending;
      };
      const read = operation => {
        if (state.closed || this.#active !== state) return Promise.reject(new Error('Input handle expired'));
        if (state.failure) return Promise.reject(state.failure);
        if (state.inFlight || state.reading) {
          state.failure ??= new Error('Concurrent batch reads/input are not permitted');
          return Promise.reject(state.failure);
        }
        state.reading = true;
        const pending = (async () => {
          checkBudget(); const result = await operation();
          try { checkBudget(); } catch (reason) { const error = asError(reason); error.reply ??= result; throw error; }
          return result;
        })().catch(reason => { const error = asError(reason); rememberFailure(state, error); throw error; }).finally(() => { state.reading = false; });
        state.pending.add(pending);
        pending.then(() => state.pending.delete(pending), () => state.pending.delete(pending));
        return pending;
      };
      value = await callback({ act, observe: options => read(() => this.#observe(options, state)), snapshot: options => read(() => this.#snapshot(options, state)), elementsPage: (offset, limit) => read(() => this.#elementsPage(offset, limit, state)) });
      checkBudget();
    } catch (error) {
      failure = asError(error);
    } finally {
      state.closed = true;
      // Even an accidentally unawaited action must settle before cleanup; its
      // error cannot be hidden by the callback catching/ignoring it.
      await Promise.allSettled([...state.pending]);
      if (state.failure) {
        if (failure && failure !== state.failure) state.failure.callbackError = failure;
        failure = state.failure;
      }
      if (state.lease) {
        try { release = await this.#client.call('computer.release', { lease: state.lease }); }
        catch (reason) {
          const error = asError(reason);
          if (failure) failure.releaseError = error;
          else failure = error;
        }
      }
      this.#disown(state);
    }
    if (failure) { failure.actionResults = state.actionResults; throw failure; }
    return { value, release, timing: { elapsed_ms: performance.now() - started, scheduling_budget_ms: maxDurationMs } }; // Not business-goal verification or total model time.
  }
}

// UIA control type names are presentation only, never semantic authority.
function controlRole(type) {
  const roles = ['button','calendar','checkbox','combobox','edit','link','image','listitem','list','menu','menubar','menuitem','progress','radio','scrollbar','slider','spinner','statusbar','tab','tabitem','text','toolbar','tooltip','tree','treeitem','custom','group','thumb','datagrid','dataitem','document','splitbutton','window','pane','header','headeritem','table','titlebar','separator','semanticzoom','appbar'];
  return roles[type - 50000] ?? `uia:${type}`;
}
