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
  for (const reason of ['transport_unknown', null, undefined]) {
  const client=new RcuClient(async name=>{
    if(name==='computer.observe')return {structuredContent:{frame:'frame',scene:{elements:[{id:'button'}]}}};
    if(name==='computer.claim')return {structuredContent:{lease:'lease'}};
    if(name==='computer.act_checked')throw reason;
    return {structuredContent:{released:true}};
  });
  const window=client.window('target');await window.observe();
  await assert.rejects(window.withInput(async i=>{try{await i.act({element:'button',intent:'click',requestId:'original'});}catch{}}),e=>e.message===String(reason)&&e.actionResults[0].requestId==='original');
  }
});

test('nested native rejection is a readable error without losing uncertainty or reference', async () => {
  const data = {dispatch:'uncertain', native_dispatch_entered:false, system_accepted:null, dispatch_error:{code:'unsupported_chord',next_step:'Inspect supported keys'},operation_ref:{execution_nonce:'exact'}};
  const client = new RcuClient(async()=>({isError:false,structuredContent:data}));
  await assert.rejects(client.call('computer.act_checked'),e=>e instanceof RcuToolError && e.message.includes('unsupported_chord') && e.message.includes('not dispatched') && e.data === data && e.data.dispatch === 'uncertain');
});

test('snapshot aliases are readable and resolve only with their original frame', async () => {
  const {window,calls} = setup();
  const first = await window.snapshot();
  assert.deepEqual(first.data.elements,[{ref:'e0',name:''}]);
  assert.equal(first.data.coverage.native_complete,null);
  assert(!JSON.stringify(first.data).includes('button'));
  await window.withInput(i=>i.act({ref:'e0',frame:first.data.frame,intent:'click',requestId:'one'}));
  assert.equal(calls.find(c=>c.name==='computer.act_checked').args.element,'button');
  await window.snapshot();
  await assert.rejects(window.withInput(i=>i.act({ref:'e0',frame:first.data.frame,intent:'click',requestId:'two'})),/exact observed frame/);
  assert.equal(calls.filter(c=>c.name==='computer.act_checked').length,1);
});

test('preview defaults to 1024 and retains authoritative view mapping without inline full capture', async () => {
  const seen=[];
  const mapping={view_size:[1024,768],original_size:[2048,1536],view_to_original:[2,0,0,2,.5,.5]};
  const media={type:'image',mimeType:'image/png',data:'fixture'};
  const client=new RcuClient(async(name,args)=>{
    seen.push({name,args});
    if(name==='computer.observe')return {structuredContent:{frame:'f',artifact_id:'a',scene:{blocked:false,mutation_ready:true,elements:[{id:'long-native-id',control_type:50000,name:'Confirm',keyboard_focus:true}]},presentation:{result_ref:'detail',elements:{total:2,native_complete:false,next_offset:1}}}};
    return {structuredContent:{view_id:'v',mapping,original_sha256:'original'},content:[media]};
  });
  const w=client.window('t'),s=await w.snapshot({preview:true});
  assert.equal(seen[0].args.inline_image,false);
  assert.equal(seen[1].args.max_edge,1024);
  assert.equal(seen[1].args.artifact_id,'a');
  assert.deepEqual(s.data.preview.mapping,mapping);
  assert.deepEqual(s.media,[media]);
  assert.deepEqual(s.data.elements,[{ref:'e0',name:'Confirm',role:'button',focused:true}]);
  assert.equal(s.data.coverage.native_complete,false);
  assert.equal(s.data.coverage.adapter_next_offset,1);
  assert.equal(s.data.details_ref,'detail');
});

test('paged summary refs do not accept element overrides or missing frame', async () => {
  const {window,calls}=setup();await window.snapshot();
  assert.equal(window.summary({limit:1}).elements[0].ref,'e0');
  assert.throws(()=>window.summary({offset:2}),/bounded summary/);
  await assert.rejects(window.withInput(i=>i.act({ref:'e0',intent:'click',requestId:'one'})),/exact observed frame/);
  assert(!calls.some(c=>c.name==='computer.act_checked'));
});
