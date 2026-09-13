import test from 'node:test';
import assert from 'node:assert/strict';
import { RcuClient, RcuToolError, RcuInputNotReadyError } from './rcu-client.mjs';

function setup(failAction = false, failRelease = false) {
  const calls = []; let frame = 0;
  const client = new RcuClient(async (name, args) => {
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
test('typed drag remains one checked frame-bound intent, not per-point MCP calls', async () => {
  const {window,calls}=setup();const snapshot=await window.snapshot();
  const intent={drag:{waypoints:[{at_ms:0,offset:[0,0]},{at_ms:500,offset:[100,50]}]}};
  const result=await window.withInput(input=>input.act({ref:'e0',frame:snapshot.data.frame,intent,requestId:'drag-once'}));
  const actions=calls.filter(c=>c.name==='computer.act_checked');assert.equal(actions.length,1);assert.deepEqual(actions[0].args.intent,intent);assert.equal(actions[0].args.element,'button');assert.equal(result.value.data.goal_verified,false);
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
  const client=new RcuClient(async (name,args)=>{
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
  const client=new RcuClient(async()=>({isError:true,content:[{type:'text',text:'{"code":"effect_unknown","operation_ref":{"execution_nonce":"exact"}}'}]}));
  await assert.rejects(client.targets(),e=>e instanceof RcuToolError&&e.data.operation_ref.execution_nonce==='exact');
});
test('non-Error transport rejections cannot hide the request identity during cleanup', async () => {
  for (const reason of ['transport_unknown', null, undefined]) {
  const client=new RcuClient(async name=>{
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

test('known blocked, unknown readiness or fenced observation sends no claim or action', async () => {
  for (const flags of [{mutation_ready:false,blocked:false},{mutation_ready:true,blocked:true},{mutation_ready:true,blocked:null},{blocked:false},{mutation_ready:true,blocked:false,fenced:true}]) {
    const calls=[];
    const client=new RcuClient(async name=>{calls.push(name);return {structuredContent:{frame:'f',scene:{...flags,elements:[{id:'button'}]},input_domain_fenced:flags.fenced??false,presentation:{result_ref:'details'}}};});
    const w=client.window('t');await w.observe();
    await assert.rejects(w.withInput(()=>{throw Error('callback must not run');}),e=>e instanceof RcuInputNotReadyError&&e.data.native_dispatched===false&&e.data.details_ref==='details');
    assert.deepEqual(calls,['computer.observe']);
  }
});

test('new blocked observation inside a batch stops second action and still releases', async () => {
  const calls=[];let observed=0;
  const client=new RcuClient(async name=>{
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
  }),e=>e instanceof RcuInputNotReadyError&&e.actionResults.length===1);
  assert.equal(calls.filter(n=>n==='computer.act_checked').length,1);
  assert.equal(calls.at(-1),'computer.release');
});

test('a fresh ready observation recovers only client state, not a replay', async () => {
  const {window,calls}=setup();const o=await window.observe();
  o.data.scene.mutation_ready=false;
  await assert.rejects(window.withInput(()=>{}),RcuInputNotReadyError);
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
  const client = new RcuClient(async (name, args) => {
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
  const client=new RcuClient(async()=>({structuredContent:{frame:'f',scene:{mutation_ready:true,blocked:false,elements:[{id:'a',visible:true,occluded:null}]},adapter_status:{native_details:{uia:{walk:{truncation:'uia_walk_failed: '+ 'x'.repeat(500)}}}}}}));
  const s=await client.window('t').snapshot();
  assert(s.data.coverage.reason.startsWith('uia_walk_failed:'));
  assert.equal(s.data.coverage.reason.length,240);
  assert.equal(s.data.elements[0].unconfirmed,true);
});

test('late observations cannot restore aliases across activation before or after its completion', async () => {
  for(const late of ['during','after']){
    let finishObserve, finishActivation;
    const selected={window:'hwnd:1',pid:7,native_epoch:'e'};const native={...selected,process_created:'1'};
    const client=new RcuClient(async name=>{
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
  const client=new RcuClient(async name=> name==='computer.observe'? await new Promise(r=>pending.push(r)):{structuredContent:{}});
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
    const client=new RcuClient(async name=>{
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
