// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { describe, expect, test } from 'bun:test';
import { mobileComposerPasteInvoke as invoke, mobileComposerPasteRequest as request, type ComposerPasteStageRequest } from './composer-paste-io';
import type { EditorPasteProof } from './composer-editor-persistence';
import { ClientError, type Native } from './shared/protocol';

const id=(n:number)=>`00000000-0000-0000-0000-${n.toString(16).padStart(12,'0')}`;
const clone=<T>(v:T):T=>JSON.parse(JSON.stringify(v)) as T;
const stage=():ComposerPasteStageRequest=>({op:'composerEditorPasteFiles',action:'stage',generation:3,operationId:id(1),
  identity:{owner:'owned',editorId:'composer',routeVisit:'visit',renderEpoch:'epoch',mountId:'mount'},richEventId:id(2),
  target:{origin:'https://home.example',environmentId:'e',draftKey:'e:t',incarnation:'incarnation',capturedRevision:4},
  sources:[10,11,12,13].map(n=>({kind:'editorImage',leaseId:id(n)})),remaining:3});
const proof=(s=stage()):EditorPasteProof=>({version:1,operationId:s.operationId,identity:s.identity,richEventId:s.richEventId,target:s.target,
  files:[{leaseId:id(10),id:id(100),kind:'image',name:'pasted-image.png',mimeType:'image/png',sizeBytes:42,sha256:'a'.repeat(64)}]});
const value=()=>({status:'staged',proof:proof(),skipped:[{leaseId:id(11),reason:'unreadable'},{leaseId:id(12),reason:'too-large'},{leaseId:id(13),reason:'excess'}]});
const ok=(v:unknown=value(),generation=3)=>({ok:true,generation,value:v});
function native(answer:(r:unknown)=>Promise<unknown>|unknown){const calls:unknown[]=[];const port:Native={available:true,watch(){throw Error('Unexpected watch')},later:async r=>{calls.push(r);return answer(r)}};return {port,calls}}
function deferred<T>(){let resolve!:(v:T)=>void,reject!:(e:unknown)=>void;const promise=new Promise<T>((a,b)=>{resolve=a;reject=b});return {promise,resolve,reject}}
const transition=(action:'adopt'|'discard'|'retire')=>request({op:'composerEditorPasteFiles',action,generation:3,operationId:id(1),proof:proof()});

describe('native pasted image invocation boundary',()=>{
  test('partial success keeps exact source-slot behavior and caller intent frozen',async()=>{
    const input=stage(),saved=request(input),n=native(()=>ok());input.target.draftKey='other';input.sources.reverse();
    const result=await invoke(saved,n.port,()=>true);
    expect(result.state).toBe('answered');expect(result.reply).toEqual(value());expect(n.calls).toEqual([saved]);
    expect(result.request).toEqual(saved);expect(Object.isFrozen(saved)).toBe(true);expect(Object.isFrozen(saved.action==='stage'?saved.target:saved.proof.target)).toBe(true);
    expect(Object.isFrozen(result.reply?.proof.files)).toBe(true);expect(saved.action==='stage'&&saved.sources[0].leaseId).toBe(id(10));
  });
  test('zero remaining and no source are successful no-op stages',async()=>{
    for(const sources of [stage().sources,[]]){
      const s=stage();s.remaining=0;s.sources=sources;const p=proof(s);p.files=[];
      const v={status:'staged',proof:p,skipped:sources.map(x=>({leaseId:x.leaseId,reason:'excess'}))};
      expect((await invoke(s,native(()=>ok(v)).port,()=>true)).reply).toEqual(v);
    }
  });
  test('same operation and exact intent survive a lost reply; no implicit cleanup or retry',async()=>{
    const saved=request(stage());let count=0;const n=native(()=>{if(++count===1)throw Error('reply lost');return ok()});
    const lost=await invoke(saved,n.port,()=>true);expect(lost.state).toBe('uncertain');expect(lost.request).toEqual(saved);expect(lost.reply).toBeNull();expect(n.calls).toHaveLength(1);
    expect((await invoke(lost.request,n.port,()=>true)).state).toBe('answered');expect(n.calls).toEqual([saved,saved]);
  });
  test('letGo keeps exact rejection identity and performs no cleanup for every action',async()=>{
    for(const r of [request(stage()),transition('adopt'),transition('discard'),transition('retire')]){
      const gone={name:'FetchError',kind:'Aborted'},n=native(()=>Promise.reject(gone));
      await expect(invoke(r,n.port,()=>true)).rejects.toBe(gone);expect(n.calls).toEqual([r]);
    }
    const gone=new ClientError('gone','superseded'),n=native(()=>{throw gone});
    await expect(invoke(stage(),n.port,()=>true)).rejects.toBe(gone);expect(n.calls).toHaveLength(1);
  });
  test('stale admission and unavailable native never dispatch',async()=>{
    const n=native(()=>ok());expect((await invoke(stage(),n.port,()=>false)).state).toBe('stale');
    n.port.available=false;expect((await invoke(stage(),n.port,()=>true)).state).toBe('unavailable');expect(n.calls).toHaveLength(0);
  });
  test('late route/catalog retirement retains authenticated receipt only for cleanup',async()=>{
    const held=deferred<unknown>(),n=native(()=>held.promise);let current=true,calls=0;
    const pending=invoke(stage(),n.port,()=>{calls++;return current});expect(calls).toBe(1);current=false;held.resolve(ok());
    const answer=await pending;expect(calls).toBe(2);expect(answer.state).toBe('stale');expect(answer.reply).toEqual(value());expect(n.calls).toHaveLength(1);
  });
  test('post-reply guard failure retains receipt; abandoned guard makes no additional IO',async()=>{
    const n=native(()=>ok());let count=0;
    const answer=await invoke(stage(),n.port,()=>{if(++count===2)throw Error('catalog unavailable');return true});
    expect(answer.state).toBe('uncertain');expect(answer.reply).toEqual(value());expect(n.calls).toHaveLength(1);
    const gone=new ClientError('gone','superseded');count=0;
    await expect(invoke(stage(),n.port,()=>{if(++count===2)throw gone;return true})).rejects.toBe(gone);expect(n.calls).toHaveLength(2);
  });
  test('transition action does not predict durable terminal outcome',async()=>{
    for(const action of ['adopt','discard'] as const)for(const status of ['adopted','discarded']){
      const v={...value(),status};expect((await invoke(transition(action),native(()=>ok(v)).port,()=>true)).reply).toEqual(v);
    }
    for(const status of ['adopted','discarded'])expect((await invoke(stage(),native(()=>ok({...value(),status})).port,()=>true)).state).toBe('answered');
    const v={status:'retired',proof:proof()};expect((await invoke(transition('retire'),native(()=>ok(v)).port,()=>true)).reply).toEqual(v);
  });
  test('generation and all proof ownership fields are exact, never coerced',async()=>{
    const altered:Array<unknown>=[ok(value(),4),ok(value(),true as unknown as number),{...ok(),ok:'true'}];
    for(const field of ['owner','editorId','routeVisit','renderEpoch','mountId'] as const){const v=value();v.proof.identity[field]+='-foreign';altered.push(ok(v))}
    for(const field of ['origin','environmentId','draftKey','incarnation'] as const){const v=value();v.proof.target[field]+='-foreign';altered.push(ok(v))}
    for(const mutate of [(p:EditorPasteProof)=>p.target.capturedRevision++,(p:EditorPasteProof)=>p.operationId=id(7),(p:EditorPasteProof)=>p.richEventId=id(8)]){const v=value();mutate(v.proof);altered.push(ok(v))}
    for(const response of altered){const n=native(()=>response),out=await invoke(stage(),n.port,()=>true);expect(out.state).toBe('uncertain');expect(out.reply).toBeNull();expect(n.calls).toHaveLength(1)}
  });
  test('rejects malformed partial partitions without losing known request',async()=>{
    const variants=[
      (v:ReturnType<typeof value>)=>v.skipped.pop(),
      (v:ReturnType<typeof value>)=>v.skipped.reverse(),
      (v:ReturnType<typeof value>)=>v.skipped.push(v.skipped[0]),
      (v:ReturnType<typeof value>)=>v.skipped[0].leaseId=id(90),
      (v:ReturnType<typeof value>)=>v.skipped[0].reason='excess',
      (v:ReturnType<typeof value>)=>v.skipped[2].reason='unreadable',
      (v:ReturnType<typeof value>)=>v.proof.files[0].leaseId=id(13),
      (v:ReturnType<typeof value>)=>v.proof.files.push(clone(v.proof.files[0])),
      (v:ReturnType<typeof value>)=>v.proof.files[0].sizeBytes=0,
      (v:ReturnType<typeof value>)=>v.proof.files[0].sha256='A'.repeat(64),
    ];
    for(const change of variants){const v=value();change(v);const out=await invoke(stage(),native(()=>ok(v)).port,()=>true);expect(out.state).toBe('uncertain');expect(out.request).toEqual(stage())}
    const s=stage();s.remaining=4;const p=proof(s);p.files.push({...p.files[0],id:id(101),leaseId:id(12)});p.files.reverse();
    expect((await invoke(s,native(()=>ok({status:'staged',proof:p,skipped:[{leaseId:id(11),reason:'unreadable'},{leaseId:id(13),reason:'unreadable'}]})).port,()=>true)).state).toBe('uncertain');
  });
  test('transition proof cannot change or stage/retire masquerade as another action',async()=>{
    const v=value();v.proof.files[0].id=id(999);
    expect((await invoke(transition('adopt'),native(()=>ok({...v,status:'adopted'})).port,()=>true)).state).toBe('uncertain');
    expect((await invoke(transition('adopt'),native(()=>ok()).port,()=>true)).state).toBe('uncertain');
    expect((await invoke(transition('retire'),native(()=>ok({...value(),status:'retired'})).port,()=>true)).state).toBe('uncertain');
    expect((await invoke(stage(),native(()=>ok({status:'retired',proof:proof()})).port,()=>true)).state).toBe('uncertain');
  });
  test('raw requests refuse coercion, extra keys, sparse arrays, accessors and arbitrary URI sources',()=>{
    const invalid:unknown[]=[{...stage(),generation:true},{...stage(),remaining:0.5},{...stage(),remaining:101},{...stage(),generation:Infinity},
      {...stage(),sources:[{kind:'editorImage',leaseId:id(10),uri:'file:///tmp/foreign'}]},
      {...stage(),sources:[{kind:'localFile',leaseId:id(10)}]},
      {...stage(),sources:[stage().sources[0],stage().sources[0]]},
      {...stage(),sources:new Array(1)}, {...stage(),action:['stage']}, {...stage(),unexpected:1}];
    const extra=stage();Object.assign(extra.sources,{extra:true});invalid.push(extra);
    const symbol=stage();Object.assign(symbol,{[Symbol('authority')]:1});invalid.push(symbol);
    let gets=0;const getter=stage();Object.defineProperty(getter.target,'origin',{enumerable:true,get(){gets++;return 'home'}});invalid.push(getter);
    for(const v of invalid)expect(()=>request(v)).toThrow();expect(gets).toBe(0);
  });
  test('malformed reply accessors are rejected without execution; strict terminal enum',async()=>{
    let gets=0;const v=value();Object.defineProperty(v.proof.target,'origin',{enumerable:true,get(){gets++;return 'home'}});
    expect((await invoke(stage(),native(()=>ok(v)).port,()=>true)).state).toBe('uncertain');expect(gets).toBe(0);
    expect((await invoke(stage(),native(()=>ok({...value(),status:['staged']})).port,()=>true)).state).toBe('uncertain');
  });
  test('classified native errors remain unresolved; transport generation cannot bless foreign errors',async()=>{
    for(const response of [{ok:false,generation:3,error:{kind:'Persistence',message:'Retained bytes',uncertain:true}},
      {ok:false,generation:99,error:{kind:'Persistence',message:'foreign'}}]){
      const out=await invoke(stage(),native(()=>response).port,()=>true);expect(out.state).toBe('uncertain');expect(out.reply).toBeNull();expect(out.request.operationId).toBe(id(1));
    }
  });
});
