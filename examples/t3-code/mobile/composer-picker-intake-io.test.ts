// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import {expect,test} from 'bun:test';
import {mobilePickerIntakeRequest as request,mobilePickerIntakeInvoke as invoke,type PickerIntakePick,type PickerIntakeFinish} from './composer-picker-intake-io';
import {mobileFileHoldsCreate as create,mobileFileHoldReserve as reserve,mobileFileHoldAcquire as acquire,mobileFileHoldHeld as held,
  mobileFileHoldReleaseNeeded as releaseNeeded,mobileFileHoldRelease as release,mobileFileHoldsRetire as retire} from './composer-file-holds-io';
import type {DraftFile} from './shared/composer-editor-files';
import {ClientError,type Native} from './shared/protocol';
const id=(n:number)=>`10000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const target={origin:'https://picker-intake.test',environmentId:'picker-intake-env',threadId:'thread',draftKey:'picker-intake-env:thread'};
const identity={owner:JSON.stringify(['ordinary',target.origin,target.environmentId,3,target.draftKey]),editorId:'composer',routeVisit:'visit',renderEpoch:'epoch',mountId:'mount'};
const pick:PickerIntakePick={op:'composerPickerIntake',action:'pick',generation:3,identity,operationId:id(1),target,document:{incarnation:'document',revision:2},source:'files',remaining:3,fileLimit:50*1024*1024};
const file={kind:'file' as const,id:id(2),name:'file.txt',mimeType:'text/plain',sizeBytes:5};
const saved:DraftFile={id:file.id,name:file.name,mimeType:file.mimeType,sizeBytes:file.sizeBytes,source:'attached',contextId:file.id,draftKey:target.draftKey,environmentId:target.environmentId,status:'staged',attachmentId:''};
const clone=<T>(v:T):T=>structuredClone(v);
const answer=(value:unknown)=>({ok:true,generation:3,value});
const value=(status='staged')=>({operationId:pick.operationId,identity,status,files:[file],error:''});
function native(run:(v:any)=>unknown|Promise<unknown>):Native{return {available:true,watch(){throw Error('No watch')},async later(v){return run(v)}}}
function deferred(){let resolve!:(v:unknown)=>void;const promise=new Promise<unknown>(yes=>{resolve=yes});return {promise,resolve}}
const finish:PickerIntakeFinish={op:'composerPickerIntake',action:'finish',generation:3,identity,operationId:id(1),publication:{after:{
  document:{...target,incarnation:'document',revision:3,selection:{start:0,end:0}},text:'',context:null,images:[],files:[saved],attachmentIds:[file.id],attachmentOrder:[file.id]},acceptedIds:[file.id],discardedIds:[]}};

test('request clones immutable actual ordinary scope before invoking the picker',async()=>{
  const input=clone(pick),owned=request(input);input.target.origin='changed';input.document.revision=9;
  expect(Object.isFrozen(owned)).toBe(true);expect((owned as PickerIntakePick).target.origin).toBe(target.origin);
  let calls=0;const result=await invoke(owned,native(r=>{calls++;expect(r).toEqual(pick);return answer(value())}),()=>true);
  expect(result.state).toBe('answered');expect(result.value?.files).toEqual([file]);expect(calls).toBe(1);expect(Object.isFrozen(result.value?.files[0])).toBe(true);
});
for(const patch of [{remaining:0},{remaining:101},{fileLimit:0},{generation:true},{source:['files']},{operationId:'bad'},{document:{incarnation:'document',revision:1.2}},
  {target:{...target,draftKey:'new:other'}},{identity:{...identity,mountId:''}},{extra:true}])test(`invalid pick boundary ${JSON.stringify(patch)}`,()=>{expect(()=>request({...pick,...patch})).toThrow()});
test('refuses sparse/extra JSON, accessors and unknown fields rather than normalizing them',()=>{
  const p=clone(finish) as any;p.publication.acceptedIds=new Array(1);p.publication.acceptedIds.extra=file.id;expect(()=>request(p)).toThrow();
  const r={...pick};Object.defineProperty(r,'source',{enumerable:true,get(){throw Error('getter executed')}});expect(()=>request(r)).toThrow();
});
test('finish validates exact complete saved projection and partition',async()=>{
  expect(request(finish)).toEqual(finish);
  for(const mutate of [(v:any)=>v.publication.acceptedIds.push(file.id),(v:any)=>v.publication.after.attachmentOrder=[],(v:any)=>v.publication.after.document.blocked=true,
    (v:any)=>v.publication.after.files[0].draftKey='foreign',(v:any)=>v.publication.after.context={bad:true}]){const v=clone(finish);mutate(v);expect(()=>request(v)).toThrow()}
  const out=await invoke(finish,native(()=>answer(value('finished'))),()=>true);expect(out.state).toBe('answered');
  expect((await invoke(finish,native(()=>answer(value('staged'))),()=>true)).state).toBe('uncertain');
});
test('actual status, cancellation and empty picker values are admitted without invented attachments',async()=>{
  for(const action of ['status','cancel'] as const){const r=request({op:pick.op,action,generation:3,identity,operationId:pick.operationId});
    expect((await invoke(r,native(()=>answer({...value(action==='cancel'?'cancelled':'picking'),files:[]})),()=>true)).value?.files).toEqual([])}
});
test('stale invocation never calls native, late success retains cleanup receipt without becoming current',async()=>{
  let calls=0;expect((await invoke(pick,native(()=>{calls++;return answer(value())}),()=>false)).state).toBe('stale');expect(calls).toBe(0);
  const wait=deferred();let current=true;const pending=invoke(pick,native(()=>wait.promise),()=>current);current=false;wait.resolve(answer(value()));
  const result=await pending;expect(result.state).toBe('stale');expect(result.value?.files[0]?.id).toBe(file.id);
});
test('unavailable native and uncertain replies retain the exact request for explicit retry',async()=>{
  const missing=native(()=>{throw Error('must not call')});missing.available=false;expect((await invoke(pick,missing,()=>true)).state).toBe('unavailable');
  const calls:unknown[]=[];let fail=true;const io=native(r=>{calls.push(r);if(fail)throw Error('reply lost');return answer(value())});
  const first=await invoke(pick,io,()=>true);expect(first.state).toBe('uncertain');fail=false;
  expect((await invoke(first.request,io,()=>true)).state).toBe('answered');expect(calls[0]).toEqual(calls[1]);
});
for(const err of [new ClientError('superseded','superseded'),{name:'FetchError',kind:'Aborted'}])test(`exact letGo ${JSON.stringify(err)} has no cleanup`,async()=>{
  let calls=0;await expect(invoke(pick,native(()=>{calls++;throw err}),()=>true)).rejects.toBe(err);expect(calls).toBe(1);
});
for(const mutate of [(v:any)=>v.generation=4,(v:any)=>v.value.identity.mountId='new',(v:any)=>v.value.operationId=id(8),(v:any)=>v.value.files.push(file),
  (v:any)=>v.value.files[0].sizeBytes=0,(v:any)=>v.value.files[0].kind=['file'],(v:any)=>v.value.files[0].extra=true])test(`rejects contradictory native receipt ${mutate.toString()}`,async()=>{
  const raw=clone(answer(value()));mutate(raw);expect((await invoke(pick,native(()=>raw),()=>true)).state).toBe('uncertain');
});
function heldReply(r:any){const q=r.request??r;return answer({status:'held',receipt:{identity:q.identity,requestId:q.requestId,holdId:id(4),fileIdentity:id(5),id:q.file.id,sizeBytes:q.file.sizeBytes}})}
test('real standard hold ledger invokes issued path and retains provenance across lost reply',async()=>{
  const ledger=create(identity,3,target),provenance={operationId:id(1)};const reserved=reserve(ledger,saved,id(3),provenance);provenance.operationId=id(99);
  let lost=true;const calls:any[]=[];const io=native(r=>{calls.push(r);if(lost)throw Error('lost');return heldReply(r)});
  expect((await acquire(ledger,id(3),io)).phase).toBe('acquire-uncertain');lost=false;
  expect((await acquire(ledger,id(3),io)).phase).toBe('held');expect(calls[0]).toEqual({op:'composerPickerIntake',action:'hold',generation:3,identity,operationId:id(1),request:reserved});
  expect(calls[1]).toEqual(calls[0]);expect(held(ledger,id(3))?.receipt.id).toBe(file.id);
  expect(()=>reserve(ledger,saved,id(3))).toThrow();expect(()=>reserve(ledger,saved,id(3),{operationId:id(8)})).toThrow();
});
test('issued in-flight hold becomes cleanup-only after release intent; releases use existing real hold owner',async()=>{
  const ledger=create(identity,3,target);reserve(ledger,saved,id(3),{operationId:id(1)});const wait=deferred();let sent:any;
  const pending=acquire(ledger,id(3),native(r=>{sent=r;return wait.promise}));releaseNeeded(ledger,id(3));wait.resolve(heldReply(sent));
  expect((await pending).held).toBeNull();expect(held(ledger,id(3))).toBeNull();
  let releasing:any;expect((await release(ledger,id(3),native(r=>{releasing=r;return answer({status:'released',requestId:id(3)})}))).phase).toBe('released');
  expect(releasing.op).toBe('composerFileHold');expect(releasing.holdId).toBe(id(4));await expect(acquire(ledger,id(3),native(()=>null))).rejects.toThrow();
});
test('retired issued acquisition remains cleanup-only and normal saved acquisitions stay unchanged',async()=>{
  const ledger=create(identity,3,target);reserve(ledger,saved,id(3),{operationId:id(1)});const wait=deferred();let sent:any;
  const p=acquire(ledger,id(3),native(r=>{sent=r;return wait.promise}));retire(ledger);wait.resolve(heldReply(sent));expect((await p).phase).toBe('release-needed');
  const ordinary=create(identity,3,target),r=reserve(ordinary,saved,id(6));await acquire(ordinary,id(6),native(v=>{expect(v).toBe(r);return heldReply(v)}));
  expect(held(ordinary,id(6))?.request.file.id).toBe(file.id);
});
