// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {expect,test} from 'bun:test';
import {createHash} from 'node:crypto';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileThreadTransferSubmit as submit,mobileThreadTransferResume as resume,mobileThreadTransferBusy as busy} from './thread-send-transfer';
import {mobileComposerTarget} from './composer-target';
import {mobileDraftChanged} from './draft';
import {mobileOutboxSnapshot,type MobileOutboxThreadTarget} from './mobile-outbox';
import type {ThreadSendTransferClaim} from './thread-send-transfer-model';
import type {ThreadSendRecord} from './thread-send-admission';
import {mobileOutboxTransferCanonical as canonical} from './mobile-outbox-transfer-model';
import {fleet} from './shared/settings-b-fleet';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';
const now=Date.parse('2026-10-09T12:00:00Z'),copy=<T>(v:T):T=>JSON.parse(JSON.stringify(v));
const fingerprint=(v:unknown)=>createHash('sha256').update(canonical(v)).digest('hex');
let serial=0;
async function fixture(options:{saved?:Obj;environmentId?:string;claim?:ThreadSendTransferClaim}={}) {
  const client=new MobileDraftClient(),environmentId=options.environmentId??`transfer-${++serial}`;
  let saved:Obj=copy(options.saved??{}),claim:ThreadSendTransferClaim|null=copy(options.claim??null),failPersist=0,complete=false;
  let inventoryComplete=true,uncertain=false;
  let hook:((request:Obj)=>Promise<void>|void)|undefined;
  const events:string[]=[],requests:Obj[]=[],snapshots:Obj[]=[];
  const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode(JSON.stringify(saved)).buffer},async atomicWriteFile(_path,bytes){
    events.push('persist');if(failPersist>0){failPersist--;throw Error('save failed')}
    saved=obj(JSON.parse(new TextDecoder().decode(bytes)));snapshots.push(copy(saved));
  }}};
  const hydration=mobileDraftRecoveryHandles(client,{available:true,watch(){},async later(){return {ok:false,generation:0,error:{message:'Offline fixture'}}}},storage);
  await client.refresh(hydration.native,hydration.storage);
  Object.assign(client,{origin:'https://transfer.test',environmentId,threadId:'thread',projectId:'project',generation:4});
  if(!options.saved)client.local.drafts[client.draftKey]='  captured  ';
  if(!fleet.saved.some(s=>s.environmentId===environmentId))fleet.saved.push({origin:client.origin,environmentId});
  const target=mobileComposerTarget(client),scope:MobileOutboxThreadTarget={origin:client.origin,environmentId,threadId:'thread',draftKey:target.key};
  const row=()=>claim?.record?{record:copy(claim.record),revision:1,token:claim.mutationId,pending:claim.state==='prepared',held:false}:null;
  const outcome=()=>claim?{messageId:claim.messageId,mutationId:claim.mutationId,status:uncertain?'uncertain':claim.state==='failed'?'failed':'committed',revision:1,message:'',record:copy(claim.record),removed:null,
    ownerEpoch:'epoch',sequenceFloor:1,current:row()??{record:null,revision:1,token:claim.mutationId,pending:false}}:null;
  const native:Native={available:true,watch(){throw Error('No watch')},async later(raw){
    const r=obj(raw);requests.push(copy(r));events.push(String(r.action??r.op));let value:unknown;
    if(r.op==='ids')value=['message','command'];
    else if(r.op==='mobileOutbox'){
      if(r.action==='read')value={ownerEpoch:'epoch',sequenceFloor:claim?1:0,complete:inventoryComplete,errors:[],records:row()?[row()]:[],
        revisions:claim?{[claim.messageId]:1}:{},tokens:claim?{[claim.messageId]:claim.mutationId}:{},outcomes:[],mutations:[],transfers:claim?[copy(claim)]:[]};
      else if(r.action==='transferLookup')value={complete:inventoryComplete,fingerprint:r.capture?fingerprint(r.capture):null,claims:claim?[copy(claim)]:[]};
      else if(r.action==='transferStatus')value={claim:copy(claim),outcome:uncertain||claim?.state==='queued'||claim?.state==='failed'?outcome():null};
      else if(r.action==='enqueueTransfer'){
        const record=r.record as ThreadSendTransferClaim['record'],capture=r.capture as ThreadSendTransferClaim['capture'];
        if(!claim)claim={kind:'ordinary',origin:scope.origin,environmentId,transferId:record!.messageId,messageId:record!.messageId,threadId:'thread',draftKey:target.key,
          commandId:record!.commandId,mutationId:String(r.mutationId),fingerprint:fingerprint(capture),state:'queued',record:copy(record),capture:copy(capture)};
        value={disposition:claim.messageId===record!.messageId?'created':'existing',claim:copy(claim),outcome:outcome()};
      }else if(r.action==='completeTransfer'){
        const proof=obj(obj(saved.mobileOutboxTransferCompletions)[String(r.transferId)]);
        expect(proof.fingerprint).toBe(claim!.fingerprint);expect(obj(proof.after).text).toBe(obj(saved.drafts)[target.key]);
        complete=true;claim={...claim!,state:'completed',record:null,capture:null};value={completed:true,claim:copy(claim)};
      }else if(r.action==='recover'){
        expect(r.messageId).toBe(claim!.messageId);expect(r.mutationId).toBe(claim!.mutationId);
        claim={...claim!,state:r.decision==='rollback'?'failed':'queued'};value=outcome();
      }else if(r.action==='releaseFailedTransfer'){claim={...claim!,state:'released',record:null,capture:null};value={released:true,claim:copy(claim)}}
      else throw Error(`Unexpected action ${r.action}`);
    }else throw Error(`Unexpected op ${r.op}`);
    await hook?.(r);return {ok:true,generation:r.generation??client.generation,value};
  }};
  const record:ThreadSendRecord={schemaVersion:1,...{origin:scope.origin,environmentId,threadId:'thread'},text:client.draft.trim(),attachments:[],modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'full-access',interactionMode:'default'};
  const input={target,record,now,current:()=>true};
  events.length=0;snapshots.length=0;
  return {client,target,scope,input,native,storage,events,requests,snapshots,disk:()=>copy(saved),claim:()=>copy(claim),completed:()=>complete,
    setUncertain(value:boolean){uncertain=value},setComplete(value:boolean){inventoryComplete=value},failSave(count=1){failPersist=count},intercept(next:typeof hook){hook=next},setClaim(next:ThreadSendTransferClaim|null){claim=copy(next)}};
}
function gate(){let resolve!:()=>void;const promise=new Promise<void>(done=>{resolve=done});return {promise,resolve}}

test('one ordinary submit persists enrollment, admits capture, then persists retirement before native completion',async()=>{
 const f=await fixture();expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('completed');
 expect(f.events).toEqual(['read','transferLookup','persist','ids','enqueueTransfer','persist','completeTransfer']);
 expect(f.snapshots[0]!.drafts).toMatchObject({[f.target.key]:'  captured  '});expect(f.client.draft).toBe('');
 expect(f.claim()?.state).toBe('completed');expect(busy(f.client,f.scope)).toBe(false);expect(f.requests.some(r=>r.op==='request')).toBe(false);
});
test('malformed outgoing binding or stale route refuses before native/storage work',async()=>{
 for(const mode of ['text','owner','route'] as const){const f=await fixture();
  if(mode==='text')f.input.record.text='wrong';if(mode==='owner')f.input.record.environmentId='other';if(mode==='route')f.input.current=()=>false;
  const answer=submit(f.client,f.native,f.storage,f.input);
  if(mode==='route')await expect(answer).rejects.toHaveProperty('kind','superseded');else expect((await answer).status).toBe('blocked');
  expect(f.events).toEqual([]);expect(f.client.draft).toBe('  captured  ');expect(busy(f.client,f.scope)).toBe(false);
 }
});
test('new plain typing during native admission remains tracked and is preserved after queued capture',async()=>{
 const f=await fixture();f.intercept(async r=>{if(r.action==='enqueueTransfer')await mobileDraftChanged(f.client,'newer text',f.native,f.storage,f.target.owner)});
 expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('completed');expect(f.client.draft).toBe('newer text');
 const proof=obj(obj(f.disk().mobileOutboxTransferCompletions).message);expect(proof.disposition).toBe('preserved');expect(obj(proof.after).text).toBe('newer text');
});
test('focus may move after native admission without clearing the other draft',async()=>{
 const f=await fixture();f.intercept(r=>{if(r.action==='enqueueTransfer'){f.client.threadId='other';f.client.local.drafts[f.client.draftKey]='other draft'}});
 expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('completed');expect(f.client.draft).toBe('other draft');expect(f.client.local.drafts[f.target.key]).toBe('');
});
test('connection replacement after admission preserves old draft and leaves original transfer recoverable',async()=>{
 const f=await fixture();f.intercept(r=>{if(r.action==='enqueueTransfer')f.client.generation++});
 await expect(submit(f.client,f.native,f.storage,f.input)).rejects.toHaveProperty('kind','superseded');expect(f.client.draft).toBe('  captured  ');expect(f.completed()).toBe(false);expect(f.claim()?.state).toBe('queued');expect(busy(f.client,f.scope)).toBe(false);
});
test('failed enrollment persistence cannot allocate IDs or enqueue',async()=>{
 const f=await fixture();f.failSave();expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('blocked');expect(f.events).toEqual(['read','transferLookup','persist']);expect(f.claim()).toBeNull();expect(f.client.draft).toBe('  captured  ');
});
test('failed retirement persistence retains queued claim and resumes with exact IDs after newer typing',async()=>{
 const f=await fixture();f.intercept(r=>{if(r.action==='enqueueTransfer')f.failSave()});
 expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('cleanup-pending');expect(f.completed()).toBe(false);expect(f.client.draft).toBe('');
 f.intercept(undefined);await mobileDraftChanged(f.client,'next draft',f.native,f.storage,f.target.owner);f.events.length=0;
 expect((await resume(f.client,f.native,f.storage,f.scope,'message')).status).toBe('completed');expect(f.client.draft).toBe('next draft');expect(f.events).toEqual(['read','transferStatus','persist','completeTransfer']);
});
test('lost accepted enqueue reply propagates let-go and cold recovery uses saved original capture',async()=>{
 const f=await fixture();f.intercept(r=>{if(r.action==='enqueueTransfer')throw {name:'FetchError',kind:'Aborted'}});
 await expect(submit(f.client,f.native,f.storage,f.input)).rejects.toHaveProperty('kind','superseded');expect(f.client.draft).toBe('  captured  ');expect(mobileOutboxSnapshot(f.client).intents[0]?.status).toBe('unknown');
 const cold=await fixture({saved:f.disk(),environmentId:f.scope.environmentId,claim:f.claim()!});
 const resumed=await resume(cold.client,cold.native,cold.storage,cold.scope,'message');
 expect(resumed).toMatchObject({status:'completed',message:''});expect(cold.client.draft).toBe('');expect(cold.events).toEqual(['read','transferStatus','persist','completeTransfer']);
});
test('concurrent second tap cannot issue another request while first submit owns its draft',async()=>{
 const f=await fixture(),entered=gate(),release=gate();f.intercept(async r=>{if(r.action==='read'){entered.resolve();await release.promise}});
 const sending=submit(f.client,f.native,f.storage,f.input);await entered.promise;const before=f.events.length;
 try{expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('busy');expect(f.events).toHaveLength(before)}finally{release.resolve()}
 expect((await sending).status).toBe('completed');expect(busy(f.client,f.scope)).toBe(false);
});
test('completed fingerprint replay cannot clear later text or allocate new identities',async()=>{
 const f=await fixture();expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('completed');
 await mobileDraftChanged(f.client,'next',f.native,f.storage,f.target.owner);f.events.length=0;
 expect((await resume(f.client,f.native,f.storage,f.scope,'message')).status).toBe('completed');expect(f.client.draft).toBe('next');expect(f.events).toEqual(['read','transferStatus']);
});

test('prepared save recovery needs an explicit exact-ID decision and never enqueues again',async()=>{
 for(const decision of ['commit','rollback'] as const){
  const f=await fixture();f.intercept(r=>{if(r.action==='enqueueTransfer')throw Error('reply lost')});
  expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('recovery-required');
  f.intercept(undefined);f.setClaim({...f.claim()!,state:'prepared'});f.events.length=0;
  expect((await resume(f.client,f.native,f.storage,f.scope,'message')).status).toBe('recovery-required');expect(f.client.draft).toBe('  captured  ');
  f.events.length=0;
  expect((await resume(f.client,f.native,f.storage,f.scope,'message',decision)).status).toBe(decision==='commit'?'completed':'failed');
  expect(f.events).toEqual(decision==='commit'?['read','transferStatus','recover','read','transferStatus','persist','completeTransfer']:['read','transferStatus','recover','read','transferStatus']);
  expect(f.client.draft).toBe(decision==='commit'?'':'  captured  ');
 }
});

for(const state of ['queued','completed'] as const)test(`incomplete inventory cannot retire or report success for ${state} claim`,async()=>{
 const f=await fixture();f.intercept(r=>{if(r.action==='enqueueTransfer')throw Error('reply lost')});
 expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('recovery-required');
 f.intercept(undefined);if(state==='completed')f.setClaim({...f.claim()!,state,record:null,capture:null});
 f.setComplete(false);f.events.length=0;
 expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('blocked');
 expect(f.events).toEqual(['read','transferLookup']);expect(f.client.draft).toBe('  captured  ');
 f.events.length=0;
 expect((await resume(f.client,f.native,f.storage,f.scope,'message')).status).toBe('recovery-required');
 expect(f.events).toEqual(['read','transferStatus']);expect(f.client.draft).toBe('  captured  ');
});
test('explicit prepared recovery may repair incomplete inventory before retirement',async()=>{
 const f=await fixture();f.intercept(r=>{if(r.action==='enqueueTransfer')throw Error('reply lost')});
 expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('recovery-required');
 f.setClaim({...f.claim()!,state:'prepared'});f.setComplete(false);f.events.length=0;
 f.intercept(r=>{if(r.action==='recover')f.setComplete(true)});
 expect((await resume(f.client,f.native,f.storage,f.scope,'message','commit')).status).toBe('completed');
 expect(f.events).toEqual(['read','transferStatus','recover','read','transferStatus','persist','completeTransfer']);
});

test('retained queued claim permits only exact uncertain durability retry',async()=>{
 const f=await fixture();f.intercept(r=>{if(r.action==='enqueueTransfer')f.failSave()});
 expect((await submit(f.client,f.native,f.storage,f.input)).status).toBe('cleanup-pending');
 f.intercept(undefined);f.setClaim({...f.claim()!,state:'prepared'});f.setComplete(false);f.setUncertain(true);
 for(const decision of ['commit','rollback'] as const){f.events.length=0;
  expect((await resume(f.client,f.native,f.storage,f.scope,'message',decision)).status).toBe('recovery-required');
  expect(f.events).toEqual(['read','transferStatus']);
 }
 f.intercept(r=>{if(r.action==='recover'){f.setUncertain(false);f.setComplete(true)}});f.events.length=0;
 expect((await resume(f.client,f.native,f.storage,f.scope,'message','retry')).status).toBe('completed');
 expect(f.events).toEqual(['read','transferStatus','recover','read','transferStatus','persist','completeTransfer']);
 expect(f.client.draft).toBe('');
});
