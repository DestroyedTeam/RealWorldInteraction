import test from 'node:test';
import assert from 'node:assert/strict';
import { RwiClient, RwiToolError, RwiInputNotReadyError } from './rwi-client.mjs';

function setup(failAction = false, failRelease = false) {
  const calls = []; let frame = 0;
  const client = new RwiClient(async (name, args) => {
    calls.push({ name, args });
    let data = {};
    if (name === 'computer.observe') data = { frame: `frame-${++frame}`, scene: { mutation_ready: true, blocked: false, elements: [{ id: 'button' }] } };
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
  await assert.rejects(window.withInput(input => input.act({ element: 'button', intent: 'click', requestId: 'intent-1' })), e => e instanceof RwiToolError && e.data.operation_ref.execution_nonce === 'exact' && e.releaseError.data.code === 'unknown_lease');
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
  const client = new RwiClient(async () => { count++; throw new Error('disconnected'); });
  await assert.rejects(client.targets(), /disconnected/); assert.equal(count, 1);
});
test('typed drag remains one checked frame-bound intent, not per-point MCP calls', async () => {
  const {window,calls}=setup();const snapshot=await window.snapshot();
  const intent={drag:{waypoints:[{at_ms:0,offset:[0,0]},{at_ms:500,offset:[100,50]}]}};
  const result=await window.withInput(input=>input.act({ref:'e0',frame:snapshot.data.frame,intent,requestId:'drag-once'}));
  const actions=calls.filter(c=>c.name==='computer.act_checked');assert.equal(actions.length,1);assert.deepEqual(actions[0].args.intent,intent);assert.equal(actions[0].args.element,'button');assert.equal(result.value.data.goal_verified,false);
});
test('text-only MCP results remain supported without duplicate output', async () => {
  const client = new RwiClient(async () => ({ content: [{ type: 'text', text: '{"targets":[]}' }] }));
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
  const client=new RwiClient(async name=>{
    calls.push(name);
    if(name==='computer.observe')return {structuredContent:{frame:'one',scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}};
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
  const client=new RwiClient(async (name,args)=>{
    seen.push(name);
    const data=name==='computer.observe'?{frame:'frame',scene:{mutation_ready:true,blocked:false,elements:[]},presentation:{result_ref:'metadata'}}
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
  const client=new RwiClient(async()=>({isError:true,content:[{type:'text',text:'{"code":"effect_unknown","operation_ref":{"execution_nonce":"exact"}}'}]}));
  await assert.rejects(client.targets(),e=>e instanceof RwiToolError&&e.data.operation_ref.execution_nonce==='exact');
});
test('non-Error transport rejections cannot hide the request identity during cleanup', async () => {
  for (const reason of ['transport_unknown', null, undefined]) {
  const client=new RwiClient(async name=>{
    if(name==='computer.observe')return {structuredContent:{frame:'frame',scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}};
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
  const client = new RwiClient(async()=>({isError:false,structuredContent:data}));
  await assert.rejects(client.call('computer.act_checked'),e=>e instanceof RwiToolError && e.message.includes('unsupported_chord') && e.message.includes('not dispatched') && e.data === data && e.data.dispatch === 'uncertain');
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
  const client=new RwiClient(async(name,args)=>{
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

test('known blocked, unknown readiness or fenced observation sends no claim or action', async () => {
  for (const flags of [{mutation_ready:false,blocked:false},{mutation_ready:true,blocked:true},{mutation_ready:true,blocked:null},{blocked:false},{mutation_ready:true,blocked:false,fenced:true}]) {
    const calls=[];
    const client=new RwiClient(async name=>{calls.push(name);return {structuredContent:{frame:'f',scene:{...flags,elements:[{id:'button'}]},input_domain_fenced:flags.fenced??false,presentation:{result_ref:'details'}}};});
    const w=client.window('t');await w.observe();
    await assert.rejects(w.withInput(()=>{throw Error('callback must not run');}),e=>e instanceof RwiInputNotReadyError&&e.data.native_dispatched===false&&e.data.details_ref==='details');
    assert.deepEqual(calls,['computer.observe']);
  }
});

test('new blocked observation inside a batch stops second action and still releases', async () => {
  const calls=[];let observed=0;
  const client=new RwiClient(async name=>{
    calls.push(name);
    if(name==='computer.observe')return {structuredContent:{frame:`f${++observed}`,scene:{elements:[{id:'button'}],mutation_ready:observed===1,blocked:observed!==1}}};
    if(name==='computer.claim')return {structuredContent:{lease:'lease'}};
    if(name==='computer.act_checked')return {structuredContent:{system_accepted:true}};
    return {structuredContent:{released:true}};
  });
  const w=client.window('t');await w.observe();
  await assert.rejects(w.withInput(async i=>{
    await i.act({element:'button',intent:'click',requestId:'one'});await i.observe();
    try {await i.act({element:'button',intent:'click',requestId:'two'});}catch {}
  }),e=>e instanceof RwiInputNotReadyError&&e.actionResults.length===1);
  assert.equal(calls.filter(n=>n==='computer.act_checked').length,1);
  assert.equal(calls.at(-1),'computer.release');
});

test('a fresh ready observation recovers only client state, not a replay', async () => {
  const {window,calls}=setup();const o=await window.observe();
  o.data.scene.mutation_ready=false;
  await assert.rejects(window.withInput(()=>{}),RwiInputNotReadyError);
  await window.observe();
  await window.withInput(i=>i.act({element:'button',intent:'click',requestId:'new-intent'}));
  assert.equal(calls.filter(n=>n.name==='computer.claim').length,1);
  assert.equal(calls.filter(n=>n.name==='computer.act_checked').length,1);
});

function activationSetup(state = 'foreground_observed', failRelease = false) {
  const calls = [];
  const selected = { window: 'hwnd:1', pid: 7, native_epoch: 'epoch' };
  const native = { ...selected, process_created: '123' };
  const previous = { window: 'hwnd:2', pid: 8, native_epoch: 'other', process_created: '456' };
  const client = new RwiClient(async (name, args) => {
    calls.push({ name, args });
    let data = {};
    if (name === 'computer.bind_window') data = { bound: selected };
    if (name === 'computer.observe') data = { frame: 'before-focus', scene: { mutation_ready: false, blocked: true, elements: [{ id: 'old', visible: true, occluded: false }] } };
    if (name === 'computer.claim') data = { lease: 'activation-lease' };
    if (name === 'computer.activate_window') data = { window: native, state, before_foreground: previous, after_foreground: state === 'wrong_window' ? previous : native, user_intervened: state === 'yielded', error: null, resources_released: true, resource_release_uncertain: false, operation_ref: { execution_nonce: 'native-activation' }, goal_verified: false, input_authority_granted: false };
    if (name === 'computer.release') {
      if (failRelease) throw new Error('release-unconfirmed');
      data = { released: true };
    }
    return { structuredContent: data };
  });
  return { win: client.window('device:context'), calls, selected, previous };
}
test('explicit activation uses a separate short lease and invalidates old frames, not a background input bypass', async () => {
  const {win,calls,selected,previous}=activationSetup();
  await win.bind({...selected,relation:'initial'}); await win.snapshot();
  const result=await win.activate({expectedForeground:previous,requestId:'focus-once'});
  assert.equal(result.value.data.goal_verified,false); assert.equal(result.release.data.released,true);
  assert.deepEqual(calls.map(c=>c.name),['computer.bind_window','computer.observe','computer.claim','computer.activate_window','computer.release']);
  assert.equal(calls.find(c=>c.name==='computer.claim').args.ttl_ms,5000);
  assert.deepEqual(calls.find(c=>c.name==='computer.activate_window').args.expected_foreground,previous);
  await assert.rejects(win.withInput(()=>{}),/Observe/);
  assert.throws(()=>win.summary(),/Observe/);
});
test('yielded or unconfirmed activation is never retried and cleanup cannot hide its native reference', async () => {
  for(const state of ['yielded','not_foreground','uncertain','wrong_window']){
    const {win,calls,selected,previous}=activationSetup(state,true);
    await win.bind({...selected,relation:'initial'});
    await assert.rejects(win.activate({expectedForeground:previous,requestId:'original-focus'}), e=>e.activationRequestId==='original-focus' && e.activationResult.data.operation_ref.execution_nonce==='native-activation' && e.releaseError.message==='release-unconfirmed');
    assert.equal(calls.filter(c=>c.name==='computer.activate_window').length,1);
    assert.equal(calls.filter(c=>c.name==='computer.act_checked').length,0);
  }
});
test('activation requires a bound selection and explicit lossless foreground evidence before claiming', async () => {
  const {win,calls,selected}=activationSetup();
  await assert.rejects(win.activate({expectedForeground:null,requestId:'one'}),/Bind/);
  await win.bind({...selected,relation:'initial'});
  await assert.rejects(win.activate({requestId:'one'}),/explicit fresh/);
  await assert.rejects(win.activate({expectedForeground:{window:'w',pid:1,process_created:123,native_epoch:'e'},requestId:'one'}),/lossless/);
  assert.equal(calls.filter(c=>c.name==='computer.claim').length,0);
});
test('activation reserves locally before await and never mutates caller foreground evidence', async () => {
  const {win,calls,selected,previous}=activationSetup();
  await win.bind({...selected,relation:'initial'});
  const expected={...previous};const running=win.activate({expectedForeground:expected,requestId:'one'});expected.window='mutated';
  await assert.rejects(win.activate({expectedForeground:previous,requestId:'two'}),/active/);
  await assert.rejects(win.snapshot(),/concurrently/);
  await running;
  assert.equal(calls.find(c=>c.name==='computer.activate_window').args.expected_foreground.window,previous.window);
});
test('partial UIA coverage exposes a bounded reason and unconfirmed controls instead of pretending readiness', async () => {
  const client=new RwiClient(async()=>({structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[{id:'a',visible:true,occluded:null}]},adapter_status:{native_details:{uia:{walk:{truncation:'uia_walk_failed: '+ 'x'.repeat(500)}}}}}}));
  const s=await client.window('t').snapshot();
  assert(s.data.coverage.reason.startsWith('uia_walk_failed:'));
  assert.equal(s.data.coverage.reason.length,240);
  assert.equal(s.data.elements[0].unconfirmed,true);
});

test('late observations cannot restore aliases across activation before or after its completion', async () => {
  for(const late of ['during','after']){
    let finishObserve, finishActivation;
    const selected={window:'hwnd:1',pid:7,native_epoch:'e'};const native={...selected,process_created:'1'};
    const client=new RwiClient(async name=>{
      if(name==='computer.observe')return await new Promise(r=>{finishObserve=r;});
      if(name==='computer.activate_window')return await new Promise(r=>{finishActivation=()=>r({structuredContent:{state:'foreground_observed',window:native,after_foreground:native,user_intervened:false,resources_released:true,resource_release_uncertain:false}});});
      return {structuredContent:name==='computer.claim'?{lease:'l'}:{released:true}};
    });
    const win=client.window('t');await win.bind({...selected,relation:'initial'});
    const old=win.observe();const rejected=assert.rejects(old,/superseded/);
    const activated=win.activate({expectedForeground:null,requestId:'focus'});
    while(!finishActivation)await Promise.resolve();
    if(late==='after'){finishActivation();await activated;}
    finishObserve({structuredContent:{frame:'old',scene:{mutation_ready:true,blocked:false,elements:[{id:'old'}]}}});await rejected;
    if(late==='during'){finishActivation();await activated;}
    assert.throws(()=>win.summary(),/Observe/);
  }
});
test('a late observation cannot cross a new binding or replace a newer observation', async () => {
  const pending=[];
  const client=new RwiClient(async name=> name==='computer.observe'? await new Promise(r=>pending.push(r)):{structuredContent:{}});
  const win=client.window('t');
  const old=win.observe();const rejected=assert.rejects(old,/superseded/);
  await win.bind({window:'new',pid:2,native_epoch:'e',relation:'initial'});
  pending.shift()({structuredContent:{frame:'old',scene:{elements:[]}}});await rejected;
  const a=win.observe();const aRejected=assert.rejects(a,/superseded/);const b=win.observe();
  pending[1]({structuredContent:{frame:'newest',scene:{mutation_ready:true,blocked:false,elements:[]}}});await b;
  pending[0]({structuredContent:{frame:'older',scene:{elements:[]}}});await aRejected;
  assert.equal(win.summary().frame,'newest');
});
test('an in-flight bind excludes reads and mutations until its native outcome is known', async () => {
  for(const fail of [false,true]){
    let finishBind;const calls=[];
    const client=new RwiClient(async name=>{
      calls.push(name);
      if(name==='computer.bind_window')return await new Promise((resolve,reject)=>{finishBind=()=> fail?reject(new Error('binding outcome unknown')):resolve({structuredContent:{}});});
      return {structuredContent:{frame:'old-binding',scene:{mutation_ready:true,blocked:false,elements:[]}}};
    });
    const win=client.window('t');await win.observe();
    const binding=win.bind({window:'B',pid:2,native_epoch:'e',relation:'initial'});
    const outcome=fail?assert.rejects(binding,/unknown/):binding;
    await assert.rejects(win.observe(),/concurrently/);
    await assert.rejects(win.withInput(()=>{}),/active/);
    await assert.rejects(win.bind({}),/binding during/);
    finishBind();await outcome;
    assert.throws(()=>win.summary(),/Observe/);
    assert.equal(calls.filter(c=>c==='computer.observe').length,1);
  }
});

test('same-target wrappers share ownership during claim and invalidate each other after binding', async () => {
  let finishClaim;const calls=[];
  const client=new RwiClient(async(name,args)=>{
    calls.push({name,args});
    if(name==='computer.claim')return await new Promise(r=>{finishClaim=()=>r({structuredContent:{lease:'lease'}});});
    if(name==='computer.observe')return {structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}};
    return {structuredContent:{released:true}};
  });
  const a=client.window('t'),b=client.window('t');await a.observe();
  const batch=a.withInput(async input=>{
    await assert.rejects(a.observe(),/owner-bound/);
    await assert.rejects(b.observe(),/owner-bound/);
    await assert.rejects(b.elementsPage(0),/owner-bound/);
    await input.observe();
  });
  await assert.rejects(b.bind({}),/active/);
  await assert.rejects(b.withInput(()=>{}),/active/);
  finishClaim();await batch;
  await b.bind({window:'other',pid:2,native_epoch:'e',relation:'initial'});
  assert.throws(()=>a.summary(),/superseded/);
  await assert.rejects(a.withInput(()=>{}),/superseded/);
  assert.equal(calls.filter(c=>c.name==='computer.claim').length,1);
});
test('owner-bound pagination can admit a fresh omitted control without external state mutation', async () => {
  const client=new RwiClient(async(name,args)=>({structuredContent:
    name==='computer.observe'?{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[]},presentation:{result_ref:'r'}}:
    name==='computer.result'?{result_ref:'r',source_tool:'computer.observe',value:[{id:'tail'}]}:
    name==='computer.claim'?{lease:'l'}:name==='computer.act_checked'?{system_accepted:true}:{released:true}}));
  const w=client.window('t');await w.observe();
  await w.withInput(async i=>{await i.elementsPage(0);await i.act({element:'tail',intent:'click',requestId:'one'});});
});
test('duplicate IDs and old deduplicated receipts poison further scheduling', async () => {
  for(const oldReceipt of [false,true]){
    let actions=0;const c=new RwiClient(async name=>({structuredContent:
      name==='computer.observe'?{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}:
      name==='computer.claim'?{lease:'l'}:name==='computer.act_checked'?{system_accepted:true,deduplicated:(actions++,oldReceipt),operation_ref:{execution_nonce:'original'}}:{released:true}}));
    const w=c.window('t');await w.observe();
    await assert.rejects(w.withInput(async i=>{
      try{await i.act({element:'button',intent:'click',requestId:'same'});}catch{}
      try{await i.observe();await i.act({element:'button',intent:'click',requestId:'same'});}catch{}
      try{await i.observe();await i.act({element:'button',intent:'click',requestId:'new-id'});}catch{}
    }), e=>e.actionResults.length===1&&(oldReceipt?e.data.code==='deduplicated_batch_stop':/unique stable/.test(e.message)));
    assert.equal(actions,1);
  }
});
async function clockTest(body){
  const original=Object.getOwnPropertyDescriptor(globalThis,'performance');const clock={now:0};
  Object.defineProperty(globalThis,'performance',{configurable:true,value:{now:()=>clock.now}});
  try{await body(clock);}finally{Object.defineProperty(globalThis,'performance',original);}
}
test('claim time, backwards clocks, invalid clocks and reserved drag duration stop before dispatch', async()=>{
  for(const mode of ['claim','backward','invalid','drag'])await clockTest(async clock=>{
    const calls=[];const c=new RwiClient(async name=>{
      calls.push(name);
      if(name==='computer.observe')return {structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}};
      if(name==='computer.claim'){clock.now=mode==='claim'?101:mode==='backward'?-1:mode==='invalid'?NaN:20;return {structuredContent:{lease:'l'}};}
      return {structuredContent:{released:true}};
    });
    const w=c.window('t');await w.observe();
    await assert.rejects(w.withInput(i=>i.act({element:'button',intent:{drag:{waypoints:[{at_ms:0,offset:[0,0]},{at_ms:90,offset:[1,1]}]}},requestId:'once'}),{ttlMs:1000,maxDurationMs:100}),/deadline|monotonic/);
    assert(!calls.includes('computer.act_checked'));assert.equal(calls.at(-1),'computer.release');
  });
});
test('deadline after an in-flight action retains its receipt, waits for it and releases', async()=>clockTest(async clock=>{
  let finish,release=false;const calls=[];
  const c=new RwiClient(async name=>{
    calls.push(name);
    if(name==='computer.observe')return {structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}};
    if(name==='computer.claim')return {structuredContent:{lease:'l'}};
    if(name==='computer.act_checked')return await new Promise(r=>{finish=()=>{clock.now=101;r({structuredContent:{system_accepted:true,operation_ref:{execution_nonce:'still-exact'}}});};});
    release=true;throw Error('release-unconfirmed');
  });
  const w=c.window('t');await w.observe();
  const running=w.withInput(async i=>{try{await i.act({element:'button',intent:'click',requestId:'one'});}catch{}try{await i.observe();await i.act({element:'button',intent:'click',requestId:'two'});}catch{}},{ttlMs:1000,maxDurationMs:100});
  while(!finish)await Promise.resolve();assert.equal(release,false);
  const outcome=assert.rejects(running,e=>/deadline/.test(e.message)&&e.actionResults[0].reply.data.operation_ref.execution_nonce==='still-exact'&&e.releaseError.message==='release-unconfirmed');
  finish();await outcome;assert.equal(calls.filter(n=>n==='computer.act_checked').length,1);assert.equal(release,true);
}));
test('unawaited read settles before release and a caught observation failure blocks more actions', async()=>{
  for(const fail of [false,true]){
    let done=false,observations=0;const calls=[];
    const c=new RwiClient(async name=>{
      calls.push(name);
      if(name==='computer.observe'){
        if(++observations===2){await new Promise(r=>setTimeout(r,5));done=true;if(fail)throw Error('provider-unavailable');}
        return {structuredContent:{frame:'f'+observations,scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}};
      }
      if(name==='computer.claim')return {structuredContent:{lease:'l'}};
      if(name==='computer.release')assert(done);
      return {structuredContent:{released:true}};
    });
    const w=c.window('t');await w.observe();
    const batch=w.withInput(async i=>{const read=i.observe();if(fail){try{await read;}catch{}try{await i.act({element:'button',intent:'click',requestId:'never'});}catch{}}});
    if(fail)await assert.rejects(batch,/provider-unavailable/);else await batch;
    assert.equal(calls.at(-1),'computer.release');assert(!calls.includes('computer.act_checked'));
  }
});
test('an in-flight internal observation cannot race another input in the callback',async()=>{
  let finish,observations=0;
  const c=new RwiClient(async name=>{
    if(name==='computer.observe'){if(++observations===2)await new Promise(r=>{finish=r;});return {structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}};}
    if(name==='computer.act_checked')assert.fail('no input while observing');
    return {structuredContent:name==='computer.claim'?{lease:'l'}:{released:true}};
  });
  const w=c.window('t');await w.observe();
  await assert.rejects(w.withInput(async i=>{const read=i.observe();while(!finish)await Promise.resolve();await assert.rejects(i.act({element:'button',intent:'click',requestId:'never'}),/Concurrent/);finish();await read;}),/Concurrent/);
});
test('late observations from another wrapper cannot replace a newer frame',async()=>{
  const pending=[];const c=new RwiClient(async()=>await new Promise(r=>pending.push(r)));
  const a=c.window('t'),b=c.window('t');const old=a.observe();const outcome=assert.rejects(old,/superseded/);const current=b.observe();
  pending[1]({structuredContent:{frame:'new',scene:{elements:[]}}});await current;
  pending[0]({structuredContent:{frame:'old',scene:{elements:[]}}});await outcome;
  assert.equal(b.summary().frame,'new');assert.throws(()=>a.summary(),/superseded/);
});
test('scheduling limits leave lease headroom and local target coordination is bounded',async()=>{
  const {window,calls,client}=setup();await window.observe();
  for(const options of [{ttlMs:1000,maxDurationMs:501},{maxDurationMs:Infinity},{maxDurationMs:0},{ttlMs:60000,maxDurationMs:55001}])await assert.rejects(window.withInput(()=>{},options),/bounded scheduling/);
  assert.equal(calls.length,1);
  for(let i=1;i<64;i++)client.window('target-'+i);
  assert.throws(()=>client.window('too-many'),/wrapper limit/);assert.doesNotThrow(()=>client.window('exact-target'));
});

test('response-only visual capture neither creates nor renews input frames or aliases',async()=>{
  const seen=[];const client=new RwiClient(async(name,args)=>{
    seen.push({name,args});
    if(name==='computer.observe')return {structuredContent:{frame:'grounded',scene:{elements:[{id:'button'}],mutation_ready:true,blocked:false}}};
    return {structuredContent:{visual_id:'visual:one',frame_created:false,input_authority_granted:false},content:[{type:'image',mimeType:'image/png',data:'fixture'}]};
  });
  const w=client.window('t');const v=await w.captureVisual();assert.equal(v.data.frame_created,false);assert.throws(()=>w.summary(),/Observe/);
  await w.observe();const before=w.summary();await w.captureVisual({maxEdge:800,feedback:'include'});assert.deepEqual(w.summary(),before);
  assert.equal(seen.at(-1).args.max_edge,800);assert.equal(seen.at(-1).args.feedback,'include');
  for(const o of [{maxEdge:63},{maxEdge:2049},{feedback:'guaranteed-visible'}])await assert.rejects(w.captureVisual(o),/maxEdge/);
  assert.equal(seen.length,3);
});

test('outside page/preview started earlier cannot cross claim, callback, release or completed ownership',async()=>{
  for(const kind of ['page','preview'])for(const phase of ['claim','callback','release','after']){
    let finishRead,finishClaim,finishCallback,finishRelease;
    const c=new RwiClient(async name=>{
      if(name==='computer.observe')return {structuredContent:{frame:'f',artifact_id:'a',scene:{mutation_ready:true,blocked:false,elements:[]},presentation:{result_ref:'r'}}};
      if(name==='computer.result'||name==='computer.image')return await new Promise(r=>{finishRead=()=>r({structuredContent:name==='computer.result'?{result_ref:'r',source_tool:'computer.observe',value:[{id:'outside'}]}:{view_id:'outside-preview',mapping:{}}});});
      if(name==='computer.claim')return await new Promise(r=>{finishClaim=()=>r({structuredContent:{lease:'l'}});});
      if(name==='computer.release')return await new Promise(r=>{finishRelease=()=>r({structuredContent:{released:true}});});
    });
    const w=c.window('t');await w.observe();
    const read=kind==='page'?w.elementsPage(0):w.snapshot({preview:true});const rejected=assert.rejects(read,/superseded/);
    while(!finishRead)await Promise.resolve();
    const batch=w.withInput(()=>new Promise(r=>{finishCallback=r;}));
    if(phase!=='claim'){finishClaim();while(!finishCallback)await Promise.resolve();}
    if(phase==='release'||phase==='after'){finishCallback();while(!finishRelease)await Promise.resolve();}
    if(phase==='after'){finishRelease();await batch;}
    finishRead();await rejected;assert.deepEqual(w.summary().elements,[]);
    if(phase==='claim'){finishClaim();while(!finishCallback)await Promise.resolve();}
    if(phase==='claim'||phase==='callback'){finishCallback();while(!finishRelease)await Promise.resolve();}
    if(phase!=='after'){finishRelease();await batch;}
  }
});

test('late read deadlines preserve the returned native reference across swallowed and release errors',async()=>clockTest(async clock=>{
  let count=0;const c=new RwiClient(async name=>{
    if(name==='computer.observe'){if(++count>1)clock.now=101;return {structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[]},operation_ref:{execution_nonce:'last-read-native'}}};}
    if(name==='computer.claim')return {structuredContent:{lease:'l'}};
    throw Error('release-unconfirmed');
  });
  const w=c.window('t');await w.observe();
  await assert.rejects(w.withInput(async i=>{try{await i.observe();}catch{}},{ttlMs:1000,maxDurationMs:100}),e=>e.reply?.data.operation_ref.execution_nonce==='last-read-native'&&e.releaseError.message==='release-unconfirmed'&&e.callbackError.message.includes('deadline'));
}));
test('ownership rejection retains the exact late native observation result',async()=>{
  let finish;const selected={window:'w',pid:1,native_epoch:'e'};
  const c=new RwiClient(async name=>name==='computer.observe'?await new Promise(r=>{finish=r;}):{structuredContent:name==='computer.claim'?{lease:'l'}:name==='computer.activate_window'?{state:'already_foreground',window:{...selected,process_created:'1'},after_foreground:{...selected,process_created:'1'},user_intervened:false,resources_released:true,resource_release_uncertain:false}:{released:true}});
  const w=c.window('t');await w.bind({...selected,relation:'initial'});const old=w.observe();const outcome=assert.rejects(old,e=>/superseded/.test(e.message)&&e.reply.data.operation_ref.execution_nonce==='late-native-read');
  await w.activate({expectedForeground:null,requestId:'activation'});
  finish({structuredContent:{frame:'late',scene:{elements:[]},operation_ref:{execution_nonce:'late-native-read'}}});await outcome;
});

test('direct and unawaited final read timeouts retain evidence through cleanup',async()=>{
  for(const mode of ['direct','unawaited'])await clockTest(async clock=>{
    let n=0;const c=new RwiClient(async name=>{
      if(name==='computer.observe'){if(++n>1)clock.now=101;return {structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[]},operation_ref:{execution_nonce:mode}}};}
      return {structuredContent:name==='computer.claim'?{lease:'l'}:{released:true}};
    });
    const w=c.window('t');await w.observe();
    await assert.rejects(w.withInput(async i=>{const read=i.observe();if(mode==='direct')await read;},{ttlMs:1000,maxDurationMs:100}),e=>e.reply.data.operation_ref.execution_nonce===mode&&/deadline/.test(e.message));
  });
});

test('local concurrency rejection cannot hide the later native read cleanup failure',async()=>{
  let n=0,finish;const c=new RwiClient(async name=>{
    if(name==='computer.observe'){
      if(++n===2)return await new Promise(r=>{finish=()=>r({isError:true,structuredContent:{code:'native_read_cleanup_uncertain',operation_ref:{execution_nonce:'pending-native-read'}}});});
      return {structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[{id:'button'}]}}};
    }
    if(name==='computer.act_checked')assert.fail('no input while reading');
    return {structuredContent:name==='computer.claim'?{lease:'l'}:{released:true}};
  });
  const w=c.window('t');await w.observe();
  await assert.rejects(w.withInput(async i=>{
    const read=i.observe();while(!finish)await Promise.resolve();
    try{await i.act({element:'button',intent:'click',requestId:'never'});}catch{}
    finish();try{await read;}catch{}
  }),e=>/Concurrent/.test(e.message)&&e.pendingError.data.operation_ref.execution_nonce==='pending-native-read'&&e.actionResults.length===0);
});

test('macOS compact controls retain actionable semantics without native identifiers', async () => {
  const client = new RwiClient(async () => ({structuredContent:{frame:'mac',scene:{blocked:false,mutation_ready:true,elements:[{id:'native',name:'',description:'Select and Move',role:'AXRadioButton',enabled:false,selected:false,value:'0',value_truncated:true,actions:['AXPress','Name:Remove from toolbar'],visible:null,occluded:null}]}}}));
  const {data}=await client.window('mac-host').snapshot();
  assert.deepEqual(data.elements,[{ref:'e0',name:'Select and Move',role:'AXRadioButton',disabled:true,selected:false,value:'0',value_truncated:true,actions:['AXPress'],unconfirmed:true}]);
  assert.equal(data.frame,'mac');
});

test('returned dispatch without OS acceptance stays unverified and requires readback', async () => {
  const client=new RwiClient(async name=>({structuredContent:name==='computer.observe'?{frame:'f',scene:{blocked:false,mutation_ready:true,elements:[{id:'e'}]}}:name==='computer.claim'?{lease:'l'}:name==='computer.act_checked'?{dispatch:'returned',dispatch_error:null,system_accepted:null,goal_verified:false}:{released:true}}));
  const w=client.window('mac');await w.observe();
  const result=await w.withInput(i=>i.act({element:'e',intent:'click',requestId:'once'}));
  assert.equal(result.value.data.system_accepted,null);
  assert.equal(result.value.data.goal_verified,false);
  assert.throws(()=>w.summary(),/Observe/);
});
