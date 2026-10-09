// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {expect,test} from 'bun:test';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileThreadSendRecoveryRead as read,mobileThreadSendRecoveryAction as act,mobileThreadSendRecoveryPresentation as present,
  type ThreadSendRecoveryOperation} from './thread-send-recovery';
import {mobileThreadSendCaptureDraft} from './thread-send-handoff-draft';
import {mobileThreadTransferLease,mobileThreadTransferBusy} from './thread-send-transfer';
import {mobileComposerTarget} from './composer-target';
import {mobileDraftChanged} from './draft';
import {mobileOutboxRead} from './mobile-outbox';
import type {ThreadSendTransferClaim} from './thread-send-transfer-model';
import {fleet} from './shared/settings-b-fleet';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';

const copy=<T>(value:T):T=>JSON.parse(JSON.stringify(value));
let serial=0;
async function fixture(state:ThreadSendTransferClaim['state']='prepared',complete=true) {
  const client=new MobileDraftClient(),environmentId=`recover-${++serial}`;
  let disk:Obj={},epoch='epoch',pending=state==='prepared',uncertain=false,active=true,missing=false,revision=1;
  let hook:((request:Obj)=>void|Promise<void>)|undefined,changeReply:((request:Obj,value:unknown)=>unknown)|undefined;
  const calls:Obj[]=[],events:string[]=[],writes:Obj[]=[];
  const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode(JSON.stringify(disk)).buffer},
    async atomicWriteFile(_path,bytes){disk=obj(JSON.parse(new TextDecoder().decode(bytes)));writes.push(copy(disk));events.push('persist')}}};
  const handles=mobileDraftRecoveryHandles(client,{available:true,watch(){},async later(){return {ok:false,generation:0,error:{message:'Offline'}}}},storage);
  await client.refresh(handles.native,handles.storage);
  Object.assign(client,{origin:'https://recovery.test',environmentId,threadId:'thread',projectId:'project',generation:4});
  fleet.saved.push({environmentId,origin:client.origin});client.local.drafts[client.draftKey]='captured';
  const composer=mobileComposerTarget(client),captured=mobileThreadSendCaptureDraft(client,composer);
  if(captured.status!=='captured')throw Error(captured.reason);
  await client.persist(storage);writes.length=0;events.length=0;
  const target={origin:client.origin,environmentId,threadId:'thread',draftKey:composer.key};
  let claim:ThreadSendTransferClaim={kind:'ordinary',...target,transferId:'message',messageId:'message',commandId:'command',
    fingerprint:'a'.repeat(64),mutationId:'epoch:1',state,record:{schemaVersion:1,origin:target.origin,environmentId,threadId:'thread',
      messageId:'message',commandId:'command',text:'captured',attachments:[],modelSelection:{instanceId:'p',model:'m'},
      runtimeMode:'full-access',interactionMode:'default',createdAt:'2026-10-09T00:00:00.000Z'},capture:captured.capture};
  if(['completed','released'].includes(state))claim={...claim,record:null,capture:null};
  let extra:ThreadSendTransferClaim[]=[];
  const outcome=()=>claim.state==='prepared'&&!uncertain?null:{messageId:claim.messageId,mutationId:claim.mutationId,
    status:uncertain?'uncertain':claim.state==='failed'?'failed':'committed',revision,message:'',record:claim.state==='failed'?null:claim.record,
    removed:null,ownerEpoch:epoch,sequenceFloor:1,current:{record:claim.state==='failed'?null:claim.record,revision,
      token:claim.mutationId,pending:claim.state==='prepared'}};
  const marker=()=>({messageId:claim.messageId,state:'pending',mutation:{operation:'enqueue',mutationId:claim.mutationId,
    record:claim.record,transfer:{...claim,outcome:null}}});
  let markers:unknown[]|undefined;
  const native:Native={available:true,watch(){throw Error('No watch')},async later(raw){
    const r=obj(raw);calls.push(copy(r));events.push(String(r.action??r.op));let value:unknown;
    if(r.op!=='mobileOutbox')throw Error(`Unexpected operation ${r.op}`);
    if(r.action==='read')value={ownerEpoch:epoch,sequenceFloor:1,complete,errors:[],records:[],revisions:{},tokens:{},outcomes:[],
      mutations:markers??(pending?[marker()]:[]),transfers:[copy(claim),...copy(extra)]};
    else if(r.action==='transferLookup')value={complete,fingerprint:null,claims:[copy(claim),...copy(extra)]};
    else if(r.action==='transferStatus')value={claim:missing?null:copy(claim),outcome:missing||['completed','released'].includes(claim.state)?null:outcome()};
    else if(r.action==='recover'){
      expect(r.messageId).toBe(claim.messageId);expect(r.mutationId).toBe(claim.mutationId);
      claim={...claim,state:r.decision==='rollback'?'failed':'queued'};pending=false;uncertain=false;complete=true;markers=undefined;value=outcome();
    }else if(r.action==='completeTransfer'){
      expect(obj(obj(disk.mobileOutboxTransferCompletions).message).fingerprint).toBe(claim.fingerprint);
      claim={...claim,state:'completed',record:null,capture:null};value={completed:true,claim:copy(claim)};
    }else if(r.action==='releaseFailedTransfer'){
      claim={...claim,state:'released',record:null,capture:null};value={released:true,claim:copy(claim)};
    }else throw Error(`Unexpected action ${r.action}`);
    await hook?.(r);return {ok:true,generation:client.generation,value:changeReply?changeReply(r,value):value};
  }};
  const input={target,routeVisit:'visit-1',current:()=>active};
  return {client,native,storage,input,composer,calls,events,writes,get claim(){return copy(claim)},get disk(){return copy(disk)},
    setClaim(next:ThreadSendTransferClaim){claim=copy(next)},setComplete(value:boolean){complete=value},setPending(value:boolean){pending=value},
    setUncertain(value:boolean){uncertain=value},setEpoch(value:string){epoch=value},setRevision(value:number){revision=value},
    setMarkers(value:unknown[]){markers=value},setExtra(value:ThreadSendTransferClaim[]){extra=copy(value)},setMissing(value=true){missing=value},
    leave(){active=false},intercept(value:typeof hook){hook=value},reply(value:typeof changeReply){changeReply=value}};
}
async function actionKey(f:Awaited<ReturnType<typeof fixture>>,kind:ThreadSendRecoveryOperation){
  const view=await read(f.client,f.native,f.input),key=view.items.flatMap(item=>item.actions).find(action=>action.kind===kind)?.key;
  if(!key)throw Error(`Missing ${kind}: ${JSON.stringify(view)}`);f.calls.length=0;f.events.length=0;return key;
}
const mutations=(f:Awaited<ReturnType<typeof fixture>>)=>f.calls.filter(r=>['recover','completeTransfer','releaseFailedTransfer'].includes(String(r.action)));

test('exact pending write offers explicit choices despite incomplete global inventory, never automatic mutation',async()=>{
  const f=await fixture('prepared',false),view=await read(f.client,f.native,f.input);
  expect(view).toMatchObject({complete:false,blocksSend:true,busy:false});expect(view.items[0]!.actions.map(a=>a.kind)).toEqual(['commit','rollback']);
  expect(present(view).actions).toHaveLength(2);expect(mutations(f)).toEqual([]);expect(f.client.draft).toBe('captured');expect(f.writes).toEqual([]);
});
for(const kind of ['commit','rollback'] as const)test(`explicit ${kind} uses original mutation through real facade`,async()=>{
  const f=await fixture('prepared',false),key=await actionKey(f,kind),result=await act(f.client,f.native,f.storage,f.input,key);
  expect(result.message).toBe('');expect(result.transfer?.status).toBe(kind==='commit'?'completed':'failed');
  expect(mutations(f).map(r=>r.action)).toEqual(kind==='commit'?['recover','completeTransfer']:['recover']);
  expect(mutations(f)[0]).toMatchObject({messageId:'message',mutationId:'epoch:1',decision:kind});
  expect(f.client.draft).toBe(kind==='commit'?'':'captured');expect(mobileThreadTransferBusy(f.client,f.input.target)).toBe(false);
});
test('failed capture release preserves draft and rereads exact terminal outcome',async()=>{
  const f=await fixture('failed'),key=await actionKey(f,'release'),result=await act(f.client,f.native,f.storage,f.input,key);
  expect(result).toMatchObject({released:true,message:'',transfer:null});expect(f.client.draft).toBe('captured');expect(f.writes).toEqual([]);
  expect(f.events).toEqual(['read','transferLookup','transferStatus','releaseFailedTransfer','read','transferLookup','transferStatus']);
  expect(present(await read(f.client,f.native,f.input)).visible).toBe(false);
});
test('finish retains text typed after captured enqueue and stores preserved proof',async()=>{
  const f=await fixture('queued'),key=await actionKey(f,'finish');await mobileDraftChanged(f.client,'new text',f.native,f.storage,f.composer.owner);
  const result=await act(f.client,f.native,f.storage,f.input,key);expect(result.transfer?.status).toBe('completed');expect(f.client.draft).toBe('new text');
  expect(obj(obj(f.disk.mobileOutboxTransferCompletions).message).disposition).toBe('preserved');
});
test('incomplete queued or failed inventory exposes no finish or release',async()=>{
  for(const state of ['queued','failed'] as const){const f=await fixture(state,false),view=await read(f.client,f.native,f.input);
    expect(view.items[0]!.actions).toEqual([]);expect(view.blocksSend).toBe(true);expect(mutations(f)).toEqual([])}
});
test('missing or ambiguous journal ownership offers no guessed recovery',async()=>{
  for(const mode of ['missing','wrong','duplicate','conflict','no-proof'] as const){const f=await fixture('prepared',false);
    if(mode==='missing')f.setMissing();
    if(mode==='wrong')f.setMarkers([{messageId:'message',state:'pending',mutation:{mutationId:'wrong'}}]);
    if(mode==='duplicate')f.setMarkers([{messageId:'message',mutation:{mutationId:'epoch:1'}},{messageId:'message',mutation:{mutationId:'epoch:1'}}]);
    if(mode==='conflict')f.setExtra([{...f.claim,transferId:'other',messageId:'other',commandId:'other-command',mutationId:'epoch:2',
      record:{...f.claim.record!,messageId:'other',commandId:'other-command'}}]);
    if(mode==='no-proof')f.setPending(false);
    const view=await read(f.client,f.native,f.input);expect(view.items.flatMap(i=>i.actions)).toEqual([]);expect(view.blocksSend).toBe(true);
  }
});
test('durability retry requires positive matching outcome and owner epoch',async()=>{
  const f=await fixture('prepared',false);f.setPending(false);f.setUncertain(true);
  expect((await read(f.client,f.native,f.input)).items[0]!.actions.map(a=>a.kind)).toEqual(['retry']);
  f.reply((r,value)=>r.action==='transferStatus'?{...obj(value),outcome:{...obj(obj(value).outcome),ownerEpoch:'foreign'}}:value);
  expect((await read(f.client,f.native,f.input)).items[0]!.actions).toEqual([]);
});
test('retained queued state can explicitly retry exact raw prepared uncertainty',async()=>{
  const f=await fixture('queued');await mobileOutboxRead(f.client,f.native);
  f.setClaim({...f.claim,state:'prepared'});f.setPending(false);f.setUncertain(true);f.setComplete(false);
  const key=await actionKey(f,'retry'),result=await act(f.client,f.native,f.storage,f.input,key);
  expect(result.transfer?.status).toBe('completed');expect(mutations(f).map(r=>r.action)).toEqual(['recover','completeTransfer']);
});
test('old visit or epoch button is never reinterpreted',async()=>{
  for(const mode of ['visit','epoch','revision'] as const){const f=await fixture('queued'),key=await actionKey(f,'finish');
    if(mode==='visit')f.input.routeVisit='visit-2';if(mode==='epoch')f.setEpoch('epoch-2');if(mode==='revision')f.setRevision(2);
    const result=await act(f.client,f.native,f.storage,f.input,key);expect(result.transfer).toBeNull();expect(mutations(f)).toEqual([]);expect(f.client.draft).toBe('captured');
  }
});
test('state changes during Resume second status cannot convert rollback to finish',async()=>{
  const f=await fixture(),key=await actionKey(f,'rollback');let count=0;
  f.reply((r,value)=>{if(r.action==='transferStatus'&&++count===2){const changed={...f.claim,state:'queued' as const};
    return {claim:changed,outcome:{messageId:'message',mutationId:'epoch:1',status:'committed',revision:1,message:'',record:changed.record,removed:null,ownerEpoch:'epoch',sequenceFloor:1,
      current:{record:changed.record,revision:1,token:'epoch:1',pending:false}}}}return value});
  await expect(act(f.client,f.native,f.storage,f.input,key)).rejects.toHaveProperty('kind','superseded');expect(mutations(f)).toEqual([]);expect(f.writes).toEqual([]);
});
test('route exit and let-go abort inspection without mutation or cleanup',async()=>{
  for(const mode of ['route','letgo'] as const){const f=await fixture('queued'),key=await actionKey(f,'finish');
    f.intercept(r=>{if(r.action==='transferStatus'){if(mode==='route')f.leave();else throw {name:'FetchError',kind:'Aborted'}}});
    await expect(act(f.client,f.native,f.storage,f.input,key)).rejects.toHaveProperty('kind','superseded');expect(mutations(f)).toEqual([]);expect(f.writes).toEqual([]);
  }
});
test('shared submission lease hides actions and rejects release without native work',async()=>{
  const f=await fixture('failed'),key=await actionKey(f,'release'),release=mobileThreadTransferLease(f.client,f.input.target)!;
  try {expect((await read(f.client,f.native,f.input)).items[0]!.actions).toEqual([]);f.calls.length=0;
    expect((await act(f.client,f.native,f.storage,f.input,key)).released).toBe(false);expect(f.calls).toEqual([])}finally{release()}
});
