/** Host-side helpers over an injected real MCP client. No OS or shell bridge. */
const asError = reason => reason instanceof Error ? reason : new Error(String(reason), { cause: reason });
export class RcuToolError extends Error {
  constructor(tool, reply) {
    let data = reply.structuredContent;
    if (data == null) {
      const text = reply.content?.find(c => c.type === 'text')?.text;
      try { data = JSON.parse(text); } catch { data = { message: text }; }
    }
    data ??= {};
    super(`${tool}: ${data.code ?? 'operation_failed'}: ${data.message ?? 'inspect retained reply'}`);
    this.name = 'RcuToolError';
    this.tool = tool;
    this.reply = reply;
    this.data = data;
  }
}

export class RcuClient {
  #callTool;
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
    return { data, media: (reply.content ?? []).filter(c => c.type !== 'text') };
  }
  async targets() { return (await this.call('computer.targets')).data; }
  window(target) {
    if (typeof target !== 'string' || !target) throw new TypeError('Use an exact returned target');
    return new RcuWindow(this, target);
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
  #active = null;
  constructor(client, target) { this.#client = client; this.#target = target; }
  async bind(selection) {
    if (this.#active) throw new Error('Do not replace a binding during an input batch');
    this.#observation = null;
    this.#elements.clear();
    return (await this.#client.call('computer.bind_window', { target: this.#target, selection })).data;
  }
  async observe(options = {}) {
    if (this.#active?.inFlight) throw new Error('Do not observe concurrently with a pending action');
    const result = await this.#client.call('computer.observe', { ...options, target: this.#target, inline_image: options.inline_image ?? false });
    if (!result.data.frame || !Array.isArray(result.data.scene?.elements)) throw new Error('Observation lacks an exact frame/element list');
    this.#observation = result.data;
    this.#elements = new Map(result.data.scene.elements.map(e => [e.id, e]));
    return result; // Text once via data; images explicitly via media.
  }
  async elementsPage(offset, limit = 16) {
    const observation = this.#observation;
    const reference = observation?.presentation?.result_ref;
    if (!reference) throw new Error('This observation has no retained metadata reference');
    const page = await this.#client.result(reference, { pointer: '/scene/elements', offset, limit });
    if (this.#observation !== observation || page.result_ref !== reference || page.source_tool !== 'computer.observe' || !Array.isArray(page.value)) throw new Error('Retained element page does not match this observation');
    for (const element of page.value) {
      if (typeof element?.id !== 'string') throw new Error('Invalid retained element');
      this.#elements.set(element.id, element);
    }
    return page;
  }
  async withInput(callback, { ttlMs = 30000, maxActions = 16 } = {}) {
    if (this.#active) throw new Error('An input batch is already active on this binding');
    if (typeof callback !== 'function' || !Number.isInteger(ttlMs) || ttlMs < 100 || ttlMs > 60000 || !Number.isInteger(maxActions) || maxActions < 1 || maxActions > 64) throw new TypeError('Use a bounded input batch');
    if (!this.#observation) throw new Error('Observe before preparing an input batch');
    // Reserve locally before awaiting claim, so concurrent batches cannot race.
    const state = { lease: null, actions: 0, closed: false, inFlight: false, failure: null, pending: new Set(), actionResults: [] };
    this.#active = state;
    let value, failure, release;
    try {
      const claimed = (await this.#client.call('computer.claim', { target: this.#target, ttl_ms: ttlMs })).data;
      if (typeof claimed.lease !== 'string') throw new Error('Claim did not return a lease');
      state.lease = claimed.lease;
      const perform = async ({ element, intent, value, requestId }) => {
        if (state.closed || this.#active !== state) throw new Error('Input handle expired');
        if (state.failure) throw state.failure;
        if (!requestId || typeof requestId !== 'string') throw new TypeError('Each intended action needs a stable requestId');
        if (!this.#observation) throw new Error('Observe the resulting state before the next action');
        if (state.actions >= maxActions) throw new Error('Input batch action limit');
        if (!this.#elements.has(element)) throw new Error('Select an element from this observation or retrieve its omitted page first');
        const frame = this.#observation.frame;
        // Invalidate immediately, including on uncertain failure. Never reuse stale state.
        this.#observation = null;
        this.#elements.clear();
        state.actions++;
        const record = { requestId, reply: null };
        state.actionResults.push(record);
        let result;
        try { result = await this.#client.call('computer.act_checked', { target: this.#target, frame, element, intent, value, lease: state.lease, request_id: requestId }); }
        catch (error) { record.reply = error?.reply ?? null; throw error; }
        record.reply = result;
        if (result.data.system_accepted !== true) throw new RcuToolError('computer.act_checked', { structuredContent: result.data, isError: true });
        return result;
      };
      const act = args => {
        if (state.closed) return Promise.reject(new Error('Input handle expired'));
        if (state.inFlight) {
          state.failure ??= new Error('Concurrent input is not permitted');
          return Promise.reject(state.failure);
        }
        state.inFlight = true;
        const pending = perform(args).catch(reason => { const error = asError(reason); state.failure ??= error; throw error; }).finally(() => { state.inFlight = false; });
        state.pending.add(pending);
        pending.then(() => state.pending.delete(pending), () => state.pending.delete(pending));
        return pending;
      };
      value = await callback({ act, observe: options => this.observe(options) });
    } catch (error) {
      failure = asError(error);
    } finally {
      state.closed = true;
      // Even an accidentally unawaited action must settle before cleanup; its
      // error cannot be hidden by the callback catching/ignoring it.
      await Promise.allSettled([...state.pending]);
      failure ??= state.failure;
      if (state.lease) {
        try { release = await this.#client.call('computer.release', { lease: state.lease }); }
        catch (reason) {
          const error = asError(reason);
          if (failure) failure.releaseError = error;
          else failure = error;
        }
      }
      this.#active = null;
    }
    if (failure) { failure.actionResults = state.actionResults; throw failure; }
    return { value, release }; // Batch completion is not business-goal verification.
  }
}
