// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { expect,test } from 'bun:test';
import { ClientError,type Native } from './shared/protocol';
import type { DraftFile } from './shared/composer-editor-files';
import { mobileFileHoldsCreate as create,mobileFileHoldReserve as reserve,mobileFileHoldAcquire as acquire,
  mobileFileHoldRelease as release,mobileFileHoldReleaseNeeded as releaseNeeded,mobileFileHoldsRetire as retire,
  mobileFileHoldHeld as held,type FileHoldRequest,type FileHoldReceipt } from './composer-file-holds-io';
const id=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const identity={owner:JSON.stringify(['ordinary','https://holds.test','env',3,'env:thread']),editorId:'composer',routeVisit:'visit',renderEpoch:'epoch',mountId:'native-mount'};
const target={origin:'https://holds.test',environmentId:'env',threadId:'thread',draftKey:'env:thread'};
const file:DraftFile={id:id(1),contextId:'context',draftKey:'env:thread',environmentId:'env',name:'file.txt',mimeType:'text/plain',sizeBytes:5,source:'attached',attachmentId:'',status:'staged'};
const clone=<T>(v:T):T=>structuredClone(v);
function fixture() {
  const ledger=create(identity,3,target),request=reserve(ledger,clone(file),id(2)),calls:unknown[]=[];
  let respond:(r:Record<string,unknown>)=>Promise<unknown>|unknown=r=>r.action==='acquire'?success(request):{ok:true,generation:3,value:{status:'released',requestId:r.requestId}};
  const native:Native={available:true,watch(){throw new Error('No watch')},async later(raw){calls.push(raw);return respond(raw as Record<string,unknown>)}};
  return {ledger,request,calls,native,respond:(f:typeof respond)=>{respond=f}};
}
function success(request:FileHoldRequest):unknown {return {ok:true,generation:request.generation,value:{status:'held',receipt:{identity:clone(request.identity),requestId:request.requestId,
  holdId:id(3),fileIdentity:id(4),id:request.file.id,sizeBytes:request.file.sizeBytes}}}}
function deferred<T=unknown>(){let resolve!:(v:T)=>void,reject!:(v:unknown)=>void;const promise=new Promise<T>((yes,no)=>{resolve=yes;reject=no});return {promise,resolve,reject}}

test('reservation is retained before invoke and immutable against caller metadata changes',async()=>{
  const ledger=create(identity,3,target),input=clone(file),request=reserve(ledger,input,id(2));input.name='changed';input.attachmentId='uploaded';
  expect(request.file.name).toBe('file.txt');expect(Object.isFrozen(request.file)).toBe(true);
  const native:Native={available:true,watch(){},async later(raw){expect(ledger.entries[id(2)]!.phase).toBe('acquiring');expect(raw).toBe(request);return success(request)}};
  const answer=await acquire(ledger,id(2),native);expect(answer.phase).toBe('held');expect(answer.held?.request.file.attachmentId).toBe('');
  expect(Object.isFrozen(answer.held!.receipt.identity)).toBe(true);expect(ledger.serial).toBe(1);
});
test('concurrent calls return pending without retaining a promise or dispatching twice',async()=>{
  const f=fixture(),wait=deferred();f.respond(()=>wait.promise);
  const one=acquire(f.ledger,id(2),f.native);expect(held(f.ledger,id(2))).toBeNull();
  expect((await acquire(f.ledger,id(2),f.native)).phase).toBe('acquiring');expect(f.calls).toHaveLength(1);
  expect(Object.values(f.ledger.entries[id(2)]!).some(v=>v instanceof Promise)).toBe(false);
  wait.resolve(success(f.request));expect((await one).phase).toBe('held');
});
test('lost acquire reply keeps exact request for a fresh invocation and stable native receipt',async()=>{
  const f=fixture(),lost=new Error('reply lost');f.respond(()=>{throw lost});
  expect((await acquire(f.ledger,id(2),f.native)).phase).toBe('acquire-uncertain');expect(held(f.ledger,id(2))).toBeNull();
  f.respond(()=>success(f.request));const next=await acquire(f.ledger,id(2),f.native);
  expect(next.phase).toBe('held');expect(f.calls[0]).toBe(f.calls[1]);expect(next.held?.receipt.holdId).toBe(id(3));
  expect((await acquire(f.ledger,id(2),f.native)).held).toEqual(next.held);
});
test('native refusal may follow an installed hold and stays uncertain',async()=>{
  const f=fixture();f.respond(()=>({ok:false,generation:3,error:{kind:'FileHold',message:'Queue write failed'}}));
  expect((await acquire(f.ledger,id(2),f.native)).phase).toBe('acquire-uncertain');expect(f.ledger.entries[id(2)]!.dispatched).toBe(true);
  f.respond(r=>({ok:true,generation:3,value:{status:'released',requestId:r.requestId}}));
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('released');expect(f.calls[1]).not.toHaveProperty('holdId');
});
for(const abandoned of [new ClientError('gone','superseded'),Object.assign(new Error('gone'),{name:'FetchError',kind:'Aborted'})])test(`exact ${abandoned.name} letGo retains acquire obligation without cleanup`,async()=>{
  const f=fixture();f.respond(()=>{throw abandoned});let caught:unknown;
  try{await acquire(f.ledger,id(2),f.native)}catch(error){caught=error}
  expect(caught).toBe(abandoned);expect(f.calls).toHaveLength(1);expect(f.ledger.entries[id(2)]!.phase).toBe('acquire-uncertain');
  expect(f.ledger.entries[id(2)]!.error).toBe('');
});
test('retired in-flight success becomes cleanup-only and release targets original identity',async()=>{
  const f=fixture(),wait=deferred();f.respond(()=>wait.promise);const acquiring=acquire(f.ledger,id(2),f.native);
  retire(f.ledger);wait.resolve(success(f.request));const result=await acquiring;
  expect(result.phase).toBe('release-needed');expect(result.held).toBeNull();
  f.respond(r=>({ok:true,generation:3,value:{status:'released',requestId:r.requestId}}));
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('released');
  expect(f.calls[1]).toMatchObject({identity,requestId:id(2),holdId:id(3),generation:3});
  expect(()=>reserve(f.ledger,file,id(5))).toThrow();
});
test('never-dispatched reservation retires locally and request ID remains a terminal tombstone',async()=>{
  const f=fixture();retire(f.ledger);expect((await release(f.ledger,id(2),f.native)).phase).toBe('released');expect(f.calls).toHaveLength(0);
  const other=fixture();releaseNeeded(other.ledger,id(2));expect(()=>reserve(other.ledger,file,id(2))).toThrow();
  expect((await release(other.ledger,id(2),other.native)).phase).toBe('released');expect(other.calls).toHaveLength(0);
});
test('release needed during acquire is irreversible without racing a native release',async()=>{
  const f=fixture(),wait=deferred();f.respond(()=>wait.promise);const pending=acquire(f.ledger,id(2),f.native);
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('acquiring');expect(f.calls).toHaveLength(1);
  wait.resolve(success(f.request));expect((await pending).phase).toBe('release-needed');expect(held(f.ledger,id(2))).toBeNull();
  await expect(acquire(f.ledger,id(2),f.native)).rejects.toThrow();
});
test('release lost reply never restores usability, never reacquires, and retries same release',async()=>{
  const f=fixture();await acquire(f.ledger,id(2),f.native);const wait=deferred();f.respond(()=>wait.promise);
  const pending=release(f.ledger,id(2),f.native);expect(held(f.ledger,id(2))).toBeNull();
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('releasing');expect(f.calls).toHaveLength(2);
  wait.reject(new Error('reply lost'));expect((await pending).phase).toBe('release-uncertain');
  await expect(acquire(f.ledger,id(2),f.native)).rejects.toThrow();expect(()=>reserve(f.ledger,file,id(2))).toThrow();
  f.respond(r=>({ok:true,generation:3,value:{status:'released',requestId:r.requestId}}));
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('released');expect(f.calls[2]).toEqual(f.calls[1]);
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('released');expect(f.calls).toHaveLength(3);
});
test('release letGo preserves exact identity with no cleanup or duplicate call',async()=>{
  const f=fixture();await acquire(f.ledger,id(2),f.native);const abandoned=Object.assign(new Error('gone'),{name:'FetchError',kind:'Aborted'});
  f.respond(()=>{throw abandoned});let caught:unknown;try{await release(f.ledger,id(2),f.native)}catch(error){caught=error}
  expect(caught).toBe(abandoned);expect(f.ledger.entries[id(2)]!.phase).toBe('release-uncertain');expect(f.calls).toHaveLength(2);
});
test('unknown release refusal remains unresolved, never parses error copy into authority',async()=>{
  const f=fixture();f.respond(()=>{throw new Error('lost')});await acquire(f.ledger,id(2),f.native);
  f.respond(()=>({ok:false,generation:3,error:{kind:'FileHold',message:'The captured native editor has ended or is unavailable.'}}));
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('release-uncertain');expect(held(f.ledger,id(2))).toBeNull();
});
for(const mutate of [
  (r:any)=>{r.generation=true},(r:any)=>{r.generation=3.5},(r:any)=>{r.generation=4},(r:any)=>{r.generation=Infinity},
  (r:any)=>{r.value.status='released'},(r:any)=>{r.value.receipt.identity.mountId='other'},(r:any)=>{r.value.receipt.identity.routeVisit='other'},
  (r:any)=>{r.value.receipt.requestId=id(9)},(r:any)=>{r.value.receipt.holdId='not-uuid'},(r:any)=>{r.value.receipt.fileIdentity='not-uuid'},
  (r:any)=>{r.value.receipt.id=id(9)},(r:any)=>{r.value.receipt.sizeBytes='5'},(r:any)=>{r.value.receipt.sizeBytes=true},
  (r:any)=>{r.value.receipt.sizeBytes=5.5},(r:any)=>{r.value.receipt.extra='untrusted'},(r:any)=>{delete r.value.receipt.identity.editorId},
])test('malformed or foreign held reply never becomes a usable receipt',async()=>{
  const f=fixture(),r=success(f.request);mutate(r);f.respond(()=>r);
  expect((await acquire(f.ledger,id(2),f.native)).phase).toBe('acquire-uncertain');expect(held(f.ledger,id(2))).toBeNull();
});
test('contradictory native replay fails closed and preserves first issued receipt for cleanup',async()=>{
  const f=fixture();await acquire(f.ledger,id(2),f.native);const r=success(f.request) as {value:{receipt:FileHoldReceipt}};r.value.receipt.holdId=id(9);f.respond(()=>r);
  expect((await acquire(f.ledger,id(2),f.native)).phase).toBe('acquire-uncertain');expect(held(f.ledger,id(2))).toBeNull();
  expect(f.ledger.entries[id(2)]!.receipt?.holdId).toBe(id(3));
});
for(const body of [{status:'released',requestId:id(9)},{status:'held',requestId:id(2)},{status:'released',requestId:id(2),extra:true},null])test('malformed release does not clear obligation',async()=>{
  const f=fixture();await acquire(f.ledger,id(2),f.native);f.respond(()=>({ok:true,generation:3,value:body}));
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('release-uncertain');
});
test('new mount cannot inherit an older receipt; authenticated receipt is not current-runtime admission',async()=>{
  const f=fixture();await acquire(f.ledger,id(2),f.native);
  const next=create({...identity,mountId:'next-mount'},3,target),r=reserve(next,file,id(8));
  const native:Native={available:true,watch(){},async later(){return success(f.request)}};
  expect((await acquire(next,r.requestId,native)).phase).toBe('acquire-uncertain');expect(held(next,r.requestId)).toBeNull();
  retire(f.ledger);expect(held(f.ledger,id(2))).toBeNull();
});
test('capture validation rejects numeric coercions, foreign target, unsupported file and request reuse',()=>{
  for(const value of [true,3.5,-1,Infinity,NaN,Number.MAX_SAFE_INTEGER+1])expect(()=>create(identity,value as number,target)).toThrow();
  expect(()=>create(identity,3,{...target,draftKey:'env:other'})).toThrow();
  for(const value of [true,5.5,-1,Infinity,NaN,50*1024*1024+1]){const f=fixture();expect(()=>reserve(f.ledger,{...file,sizeBytes:value as number},id(8))).toThrow()}
  for(const patch of [{source:'pasted-text'},{id:'UPPERCASE'},{environmentId:'other'},{status:true},{videoWidth:Infinity},{videoWidth:true}]){
    const f=fixture();expect(()=>reserve(f.ledger,{...file,...patch} as DraftFile,id(8))).toThrow();
  }
  const f=fixture();expect(reserve(f.ledger,clone(file),id(2))).toBe(f.request);expect(()=>reserve(f.ledger,{...file,name:'changed'},id(2))).toThrow();
});
test('counter exhaustion refuses native dispatch, unavailable native remains never-dispatched',async()=>{
  const f=fixture();f.ledger.serial=Number.MAX_SAFE_INTEGER;await expect(acquire(f.ledger,id(2),f.native)).rejects.toThrow();expect(f.calls).toHaveLength(0);
  const other=fixture();other.native.available=false;expect((await acquire(other.ledger,id(2),other.native)).phase).toBe('reserved');
  expect(other.ledger.entries[id(2)]!.dispatched).toBe(false);retire(other.ledger);expect(other.ledger.entries[id(2)]!.phase).toBe('released');
});
test('capacity refuses admission without evicting active or unresolved requests',async()=>{
  const f=fixture();await acquire(f.ledger,id(2),f.native);
  for(let n=10;n<4105;n++)reserve(f.ledger,file,id(n));
  expect(Object.keys(f.ledger.entries)).toHaveLength(4096);const original=held(f.ledger,id(2));
  expect(()=>reserve(f.ledger,file,id(5000))).toThrow();expect(held(f.ledger,id(2))).toEqual(original);
  expect((await release(f.ledger,id(2),f.native)).phase).toBe('released');expect(Object.keys(f.ledger.entries)).toHaveLength(4096);
});
