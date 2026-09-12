import test from 'node:test';
import assert from 'node:assert/strict';
import { RcuClient, RcuToolError } from './rcu-client.mjs';

function setup(failAction = false, failRelease = false) {
  const calls = []; let frame = 0;
  const client = new RcuClient(async (name, args) => {
    calls.push({ name, args });
    let data = {};
    if (name === 'computer.observe') data = { frame: `frame-${++frame}`, scene: { elements: [{ id: 'button' }] } };
    if (name === 'computer.claim') data = { lease: 'lease', ttl_ms: args.ttl_ms };
    if (name === 'computer.release') {
      if (failRelease) return { isError: true, structuredContent: { code: 'unknown_lease' } };
      data = { released: true };
    }
    if (name === 'computer.act_checked') {
      if (failAction) return { isError: true, structuredContent: { code: 'effect_unknown', operation_ref: { execution_nonce: 'exact' } } };
      data = { system_accepted: true, goal_verified: false, operation_ref: { execution_nonce: 'exact' } };
    }
    return { structuredContent: data, content: [{ type: 'text', text: JSON.stringify(data) }] };
  });
  return { client, window: client.window('exact-target'), calls };
}
test('one bounded callback claims, acts, observes and releases without losing references', async () => {
  const { window, calls } = setup(); await window.observe();
  const batch = await window.withInput(async input => {
    const action = await input.act({ element: 'button', intent: 'click', requestId: 'intent-1' });
    assert.equal(action.data.goal_verified, false);
    assert.equal(action.data.operation_ref.execution_nonce, 'exact');
    await input.observe();
    return 'callback result, not verified goal';
  });
  assert.equal(batch.release.data.released, true);
  assert.deepEqual(calls.map(c => c.name), ['computer.observe','computer.claim','computer.act_checked','computer.observe','computer.release']);
});
test('uncertain action is never retried; cleanup cannot hide original reference', async () => {
  const { window, calls } = setup(true, true); await window.observe();
  await assert.rejects(window.withInput(input => input.act({ element: 'button', intent: 'click', requestId: 'intent-1' })), e => e instanceof RcuToolError && e.data.operation_ref.execution_nonce === 'exact' && e.releaseError.data.code === 'unknown_lease');
  assert.equal(calls.filter(c => c.name === 'computer.act_checked').length, 1);
});
test('fresh observation is mandatory after each mutation and input handle expires', async () => {
  const { window, calls } = setup(); await window.observe(); let retained;
  await assert.rejects(window.withInput(async input => {
    retained = input;
    await input.act({ element: 'button', intent: 'click', requestId: 'one' });
    await input.act({ element: 'button', intent: 'click', requestId: 'two' });
  }), /Observe the resulting state/);
  await assert.rejects(retained.act({ element: 'button', intent: 'click', requestId: 'three' }), /expired/);
  assert.equal(calls.filter(c => c.name === 'computer.act_checked').length, 1);
});
test('transport failure is surfaced with no automatic retry', async () => {
  let count = 0;
  const client = new RcuClient(async () => { count++; throw new Error('disconnected'); });
  await assert.rejects(client.targets(), /disconnected/); assert.equal(count, 1);
});
test('text-only MCP results remain supported without duplicate output', async () => {
  const client = new RcuClient(async () => ({ content: [{ type: 'text', text: '{"targets":[]}' }] }));
  assert.deepEqual(await client.targets(), { targets: [] });
});
test('caught action failure still stops the batch and cannot trigger a second dispatch', async () => {
  const { window, calls } = setup(true); await window.observe();
  await assert.rejects(window.withInput(async input => {
    try { await input.act({ element: 'button', intent: 'click', requestId: 'first' }); } catch {}
    await input.observe();
    try { await input.act({ element: 'button', intent: 'click', requestId: 'second' }); } catch {}
  }), /effect_unknown/);
  assert.equal(calls.filter(c => c.name === 'computer.act_checked').length, 1);
});
test('cleanup waits for an accidentally unawaited action', async () => {
  const calls=[]; let finished=false;
  const client=new RcuClient(async name=>{
    calls.push(name);
    if(name==='computer.observe')return {structuredContent:{frame:'one',scene:{elements:[{id:'button'}]}}};
    if(name==='computer.claim')return {structuredContent:{lease:'one'}};
    if(name==='computer.act_checked'){await new Promise(resolve=>setTimeout(resolve,10));finished=true;return {structuredContent:{system_accepted:true}};}
    if(name==='computer.release'){assert(finished);return {structuredContent:{released:true}};}
  });
  const window=client.window('target');await window.observe();
  await window.withInput(input=>{input.act({element:'button',intent:'click',requestId:'one'});});
  assert.equal(calls.at(-1),'computer.release');
});
test('omitted elements are usable only after reading this observation metadata page', async () => {
  const seen=[];
  const client=new RcuClient(async (name,args)=>{
    seen.push(name);
    const data=name==='computer.observe'?{frame:'frame',scene:{elements:[]},presentation:{result_ref:'metadata'}}
      :name==='computer.result'?{result_ref:args.result_ref,source_tool:'computer.observe',value:[{id:'tail'}]}
      :name==='computer.claim'?{lease:'lease'}
      :name==='computer.act_checked'?{system_accepted:true}:{released:true};
    return {structuredContent:data};
  });
  const window=client.window('target');await window.observe();await window.elementsPage(56);
  await window.withInput(i=>i.act({element:'tail',intent:'click',requestId:'one'}));
  assert.equal(seen.filter(n=>n==='computer.observe').length,1);
  assert.equal(seen.filter(n=>n==='computer.act_checked').length,1);
});
test('text-only failures retain their native execution reference', async () => {
  const client=new RcuClient(async()=>({isError:true,content:[{type:'text',text:'{"code":"effect_unknown","operation_ref":{"execution_nonce":"exact"}}'}]}));
  await assert.rejects(client.targets(),e=>e instanceof RcuToolError&&e.data.operation_ref.execution_nonce==='exact');
});
test('non-Error transport rejections cannot hide the request identity during cleanup', async () => {
  const client=new RcuClient(async name=>{
    if(name==='computer.observe')return {structuredContent:{frame:'frame',scene:{elements:[{id:'button'}]}}};
    if(name==='computer.claim')return {structuredContent:{lease:'lease'}};
    if(name==='computer.act_checked')throw 'transport_unknown';
    return {structuredContent:{released:true}};
  });
  const window=client.window('target');await window.observe();
  await assert.rejects(window.withInput(async i=>{try{await i.act({element:'button',intent:'click',requestId:'original'});}catch{}}),e=>e.message==='transport_unknown'&&e.actionResults[0].requestId==='original');
});
