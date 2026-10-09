// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import {expect,test} from 'bun:test';
import {mobileOutboxRead,mobileOutboxSnapshot,mobileOutboxEnqueue,mobileOutboxEnqueueTransfer,mobileOutboxTransferLookup,
 mobileOutboxTransferStatus,mobileOutboxCompleteTransfer,mobileOutboxEnqueueThreadTransfer as enqueue,
 mobileOutboxThreadTransferLookup as lookup,mobileOutboxThreadTransferStatus as status,
 mobileOutboxCompleteThreadTransfer as complete,mobileOutboxReleaseFailedThreadTransfer as release,type MobileOutboxThreadTarget} from './mobile-outbox';
import type {ThreadSendTransferCapture,ThreadSendTransferClaim} from './thread-send-transfer-model';
import type {MobileOutboxTransferClaim} from './mobile-outbox-transfer-model';
import type {MobileOutboxRecord} from './mobile-outbox-model';
import type {Native} from './shared/protocol';
const target=(patch:Partial<MobileOutboxThreadTarget>={}):MobileOutboxThreadTarget=>({origin:'https://server.test',environmentId:'env',threadId:'thread',draftKey:'env:thread',...patch});
const capture=(scope=target()):ThreadSendTransferCapture=>({version:2,kind:'ordinary',draft:{key:scope.draftKey,origin:scope.origin,environmentId:scope.environmentId,threadId:scope.threadId,
 document:{incarnation:'document-1',revision:3,selection:{start:2,end:2}},text:'  original  ',context:null,contextRevision:0,images:[],files:[],attachmentIds:[],attachmentOrder:null}});
const record=(id='message',scope=target()):MobileOutboxRecord=>({schemaVersion:1,origin:scope.origin,environmentId:scope.environmentId,threadId:scope.threadId,messageId:id,commandId:`command-${id}`,
 text:'original',attachments:[],modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'full-access',interactionMode:'default',createdAt:'2026-10-09T00:00:00.000Z'});
function claim(id='message',state:ThreadSendTransferClaim['state']='queued',scope=target()):ThreadSendTransferClaim {
 return {kind:'ordinary',origin:scope.origin,environmentId:scope.environmentId,transferId:id,draftKey:scope.draftKey,fingerprint:'a'.repeat(64),messageId:id,threadId:scope.threadId,
  commandId:`command-${id}`,mutationId:'epoch:1',state,record:['completed','released'].includes(state)?null:record(id,scope),capture:['completed','released'].includes(state)?null:capture(scope)};
}
function legacy():MobileOutboxTransferClaim {
 return {transferId:'new-task-message',draftKey:'new-task:a',fingerprint:'b'.repeat(64),messageId:'new-task-message',threadId:'created-thread',commandId:'new-task-command',mutationId:'epoch:2',state:'queued',
 record:{...record('new-task-message'),threadId:'created-thread',commandId:'new-task-command',creation:{projectId:'p',workspaceMode:'local',branch:null,worktreePath:null}},
 capture:{version:1,draft:{key:'new-task:a',origin:'https://server.test',environmentId:'env',projectId:'p',createdAt:'2026-10-09T00:00:00.000Z',revision:1,choices:null,
 text:'original',images:[],files:[],attachmentIds:[],workspace:null}}};
}
const current=(value:MobileOutboxRecord|null=record(),token='epoch:1',revision=1)=>({record:value,token,revision,pending:false});
const outcome=(owner:ThreadSendTransferClaim|MobileOutboxTransferClaim=claim(),row=current(owner.record),result='committed')=>({messageId:owner.messageId,mutationId:owner.mutationId,
 status:result,revision:1,message:'',record:owner.record,removed:null,ownerEpoch:'epoch',sequenceFloor:1,current:row});
const inventory=(transfers:unknown[]=[])=>({ownerEpoch:'epoch',sequenceFloor:0,complete:true,errors:[],records:[],outcomes:[],mutations:[],revisions:{},tokens:{},transfers});
type Call={request:any;resolve(value:unknown):void;reject(error:unknown):void};
async function fixture(initial:unknown=inventory()) {
 const client={revision:0},calls:Call[]=[];
 const native:Native={available:true,watch(){throw Error('No watch')},later(request){return new Promise((resolve,reject)=>calls.push({request,resolve,reject}))}};
 const answer=(i:number,value:unknown)=>calls[i]!.resolve({ok:true,generation:0,value});
 const reading=mobileOutboxRead(client,native);answer(0,initial);await reading;return {client,native,calls,answer};
}

test('mixed inventory has one owner and independent narrow snapshot projections',async()=>{
 const f=await fixture(inventory([legacy(),claim()])),snapshot=mobileOutboxSnapshot(f.client);
 expect(snapshot.complete).toBe(true);expect(snapshot.transfers.map(x=>x.transferId)).toEqual(['new-task-message']);expect(snapshot.threadTransfers.map(x=>x.transferId)).toEqual(['message']);
 snapshot.threadTransfers[0]!.capture!.draft.text='alias';expect(mobileOutboxSnapshot(f.client).threadTransfers[0]!.capture!.draft.text).toBe('  original  ');
});
test('unknown explicit kinds and duplicate IDs across kinds preserve known rows and block admission',async()=>{
 for(const bad of [{...claim('bad'),kind:'future'},{...legacy(),transferId:'message',messageId:'message'}]){
  const f=await fixture(inventory([claim(),bad]));expect(mobileOutboxSnapshot(f.client).complete).toBe(false);expect(mobileOutboxSnapshot(f.client).threadTransfers).toHaveLength(1);
  await expect(enqueue(f.client,f.native,record('new'),capture())).rejects.toThrow('incomplete');expect(f.calls).toHaveLength(1);
 }
});
test('ordinary enqueue sends scoped immutable request immediately and shares sequence with NewTask and row mutations',async()=>{
 const f=await fixture(),input=record(),captured=capture(),pending=enqueue(f.client,f.native,input,captured);
 expect(f.calls[1]!.request).toMatchObject({op:'mobileOutbox',action:'enqueueTransfer',kind:'ordinary',target:target(),ownerEpoch:'epoch',mutationId:'epoch:1'});
 captured.draft.text='changed';input.text='changed';expect(f.calls[1]!.request.capture.draft.text).toBe('  original  ');expect(mobileOutboxSnapshot(f.client).intents[0]?.threadTransfer?.capture.draft.text).toBe('  original  ');
 f.answer(1,{disposition:'created',claim:claim(),outcome:outcome()});expect((await pending).disposition).toBe('created');
 const old=legacy(),next=mobileOutboxEnqueueTransfer(f.client,f.native,old.record!,old.capture!);expect(f.calls[2]!.request.mutationId).toBe('epoch:2');expect(f.calls[2]!.request.kind).toBeUndefined();
 f.answer(2,{disposition:'created',claim:old,outcome:{...outcome(old),sequenceFloor:2}});expect((await next).disposition).toBe('created');
 const mutation=mobileOutboxEnqueue(f.client,f.native,record('third'));expect(f.calls[3]!.request.mutationId).toBe('epoch:3');f.calls[3]!.reject(Error('uncertain'));await mutation;
 expect(mobileOutboxSnapshot(f.client).transfers).toHaveLength(1);expect(mobileOutboxSnapshot(f.client).threadTransfers).toHaveLength(1);
});
test('ordinary existing claim retains original IDs and removes only duplicate proposal',async()=>{
 const f=await fixture(),sending=enqueue(f.client,f.native,record('new'),capture());f.answer(1,{disposition:'existing',claim:claim(),outcome:outcome()});
 const result=await sending;expect(result.claim?.messageId).toBe('message');expect(mobileOutboxSnapshot(f.client).rows.map(r=>r.record.messageId)).toEqual(['message']);expect(mobileOutboxSnapshot(f.client).intents).toEqual([]);
});
test('ordinary conflict cannot erase later optimistic replacement and never grants requested capture',async()=>{
 const f=await fixture(),c=capture();c.draft.text='different';const sending=enqueue(f.client,f.native,{...record('new'),text:'different'},c),replacement=mobileOutboxEnqueue(f.client,f.native,{...record('new'),text:'later'});
 f.answer(1,{disposition:'conflict',claim:claim(),outcome:outcome()});expect((await sending).disposition).toBe('conflict');expect(mobileOutboxSnapshot(f.client).rows.find(r=>r.record.messageId==='new')?.record.text).toBe('later');
 f.calls[2]!.reject(Error('uncertain'));await replacement;
});
test('foreign scope or wrong-kind admission becomes exact unknown without adopting returned rows',async()=>{
 for(const foreign of [claim('message','queued',target({origin:'https://other.test'})),claim('message','queued',target({environmentId:'other',draftKey:'other:thread'})),legacy()]){
  const f=await fixture(),sending=enqueue(f.client,f.native,record(),capture());f.answer(1,{disposition:'conflict',claim:foreign,outcome:outcome(foreign)});
  expect((await sending).disposition).toBe('unknown');const view=mobileOutboxSnapshot(f.client);expect(view.threadTransfers).toEqual([]);expect(view.transfers).toEqual([]);expect(view.intents[0]?.threadTransfer?.target).toEqual(target());expect(view.rows[0]?.status).toBe('uncertain');
 }
});
test('lost ordinary answer retains immutable intent and letGo propagates without another native call',async()=>{
 const f=await fixture(),sentinel={name:'FetchError',kind:'Aborted'},sending=enqueue(f.client,f.native,record(),capture());
 f.calls[1]!.reject(sentinel);await expect(sending).rejects.toMatchObject({kind:'superseded'});expect(f.calls).toHaveLength(2);
 const view=mobileOutboxSnapshot(f.client);expect(view.intents[0]?.status).toBe('unknown');expect(view.intents[0]?.threadTransfer?.capture).toEqual(capture());expect(view.rows[0]?.status).toBe('uncertain');
 const checking=status(f.client,f.native,target(),'message');f.answer(2,{claim:claim(),outcome:outcome()});expect((await checking).outcome?.status).toBe('committed');
});
test('ordinary error retains unknown admission without pretending failure or removing bytes',async()=>{
 const f=await fixture(),sending=enqueue(f.client,f.native,record(),capture());f.calls[1]!.reject(Error('disk reply lost'));
 expect((await sending).disposition).toBe('unknown');expect(mobileOutboxSnapshot(f.client).intents[0]?.threadTransfer?.capture).toEqual(capture());expect(f.calls).toHaveLength(2);
});
test('lookup scopes same draftKey across homes, validates fingerprints and incomplete recovery',async()=>{
 const f=await fixture(inventory([claim(),claim('other','queued',target({origin:'https://other.test'}))]));
 const searching=lookup(f.client,f.native,target(),capture());expect(f.calls[1]!.request).toMatchObject({action:'transferLookup',kind:'ordinary',target:target(),capture:capture()});
 f.answer(1,{complete:false,fingerprint:'a'.repeat(64),claims:[claim()]});expect((await searching).complete).toBe(false);expect(mobileOutboxSnapshot(f.client).threadTransfers).toHaveLength(2);
 const bad=lookup(f.client,f.native,target());f.answer(2,{complete:true,fingerprint:null,claims:[claim('other','queued',target({origin:'https://other.test'}))]});await expect(bad).rejects.toThrow();
});
test('lookup rejects duplicate owners, malformed target and mismatched capture before dispatch',async()=>{
 const f=await fixture();await expect(lookup(f.client,f.native,target({threadId:'other'}),capture())).rejects.toThrow();expect(f.calls).toHaveLength(1);
 const searching=lookup(f.client,f.native,target());f.answer(1,{complete:true,fingerprint:null,claims:[claim(),claim()]});await expect(searching).rejects.toThrow();expect(mobileOutboxSnapshot(f.client).threadTransfers).toEqual([]);
 let reads=0;const bad:any={...target()};Object.defineProperty(bad,'origin',{enumerable:true,get(){reads++;return 'https://server.test'}});await expect(status(f.client,f.native,bad,'message')).rejects.toThrow();expect(reads).toBe(0);
});
test('NewTask public lookup/status/retirement reject ordinary input and cannot publish it',async()=>{
 const f=await fixture();await expect(mobileOutboxCompleteTransfer(f.client,f.native,claim() as never)).rejects.toThrow();expect(f.calls).toHaveLength(1);
 const looking=mobileOutboxTransferLookup(f.client,f.native,'new-task:a');f.answer(1,{complete:true,fingerprint:null,claims:[claim()]});await expect(looking).rejects.toThrow();
 const checking=mobileOutboxTransferStatus(f.client,f.native,'message');f.answer(2,{claim:claim(),outcome:outcome()});await expect(checking).rejects.toThrow();expect(mobileOutboxSnapshot(f.client).threadTransfers).toEqual([]);
});
test('ordinary status refuses wrong-kind/scope or mutated immutable capture before outcome adoption',async()=>{
 for(const kind of ['scope','capture','kind'] as const){const f=await fixture(inventory([claim()]));let altered:ThreadSendTransferClaim|MobileOutboxTransferClaim=claim();
  if(kind==='scope')altered=claim('message','queued',target({origin:'https://other.test'}));if(kind==='kind')altered=legacy();
  if(kind==='capture'){altered.record!.text='other';altered.capture!.draft.text='other'}
  const checking=status(f.client,f.native,target(),'message');f.answer(1,{claim:altered,outcome:outcome(altered)});await expect(checking).rejects.toThrow();expect(mobileOutboxSnapshot(f.client).rows).toEqual([]);expect(mobileOutboxSnapshot(f.client).threadTransfers[0]?.capture).toEqual(capture());
 }
});
test('ordinary completion sends exact scope and cannot be reversed by late status or inventory',async()=>{
 const f=await fixture(inventory([claim()])),checking=status(f.client,f.native,target(),'message'),reading=mobileOutboxRead(f.client,f.native),finishing=complete(f.client,f.native,claim());
 expect(f.calls[3]!.request).toEqual({op:'mobileOutbox',action:'completeTransfer',kind:'ordinary',target:target(),transferId:'message',fingerprint:'a'.repeat(64)});
 f.answer(3,{completed:true,claim:claim('message','completed')});expect(await finishing).toBe(true);
 f.answer(1,{claim:claim(),outcome:outcome()});const late=await checking;expect(late.claim?.state).toBe('completed');expect(late.claim?.capture).toBeNull();f.answer(2,inventory([claim()]));await reading;
 const final=mobileOutboxSnapshot(f.client).threadTransfers[0]!;expect(final.state).toBe('completed');expect(final.capture).toBeNull();
});
test('terminal replay with no current row cannot fabricate another optimistic owner',async()=>{
 const f=await fixture(inventory([claim('message','completed')])),sending=enqueue(f.client,f.native,record('new'),capture());
 f.answer(1,{disposition:'existing',claim:claim('message','completed'),outcome:null});expect((await sending).disposition).toBe('existing');expect(mobileOutboxSnapshot(f.client).rows).toEqual([]);
});
test('retirement validates supplied immutable identity even before inventory, and preserves native failure',async()=>{
 for(const bad of [claim('message','completed',target({origin:'https://other.test'})),{...claim('message','completed'),commandId:'different'},claim('message','released')]){
  const f=await fixture(),finishing=complete(f.client,f.native,claim());f.answer(1,{completed:true,claim:bad});await expect(finishing).rejects.toThrow();expect(mobileOutboxSnapshot(f.client).threadTransfers).toEqual([]);
 }
 const f=await fixture(),finishing=complete(f.client,f.native,claim());f.answer(1,{completed:false,claim:claim()});expect(await finishing).toBe(false);expect(mobileOutboxSnapshot(f.client).threadTransfers[0]?.state).toBe('queued');
});
test('failed release is scoped, idempotent and cannot turn queued into released',async()=>{
 const f=await fixture(),releasing=release(f.client,f.native,claim('message','failed'));expect(f.calls[1]!.request).toMatchObject({action:'releaseFailedTransfer',kind:'ordinary',target:target()});
 f.answer(1,{released:true,claim:claim('message','released')});expect(await releasing).toBe(true);
 const again=release(f.client,f.native,claim('message','released'));f.answer(2,{released:true,claim:claim('message','released')});expect(await again).toBe(true);
 const g=await fixture(),invalid=release(g.client,g.native,claim());g.answer(1,{released:true,claim:claim('message','released')});await expect(invalid).rejects.toThrow();
});
test('terminal supplied receipt plus late nonterminal retirement cannot resurrect even in a fresh JS owner',async()=>{
 const f=await fixture(),finishing=complete(f.client,f.native,claim('message','completed'));f.answer(1,{completed:false,claim:claim('message','prepared')});expect(await finishing).toBe(false);expect(mobileOutboxSnapshot(f.client).threadTransfers[0]?.state).toBe('completed');
});
test('old epoch outcome cannot replace a newer native epoch row',async()=>{
 const f=await fixture(),checking=status(f.client,f.native,target(),'message'),reading=mobileOutboxRead(f.client,f.native),newer={...record(),text:'newer pending'};
 f.answer(2,{...inventory(),ownerEpoch:'new-epoch',sequenceFloor:4,records:[{...current(newer,'new-epoch:4',4),held:false}],revisions:{message:4},tokens:{message:'new-epoch:4'}});await reading;
 f.answer(1,{claim:claim(),outcome:outcome()});await checking;expect(mobileOutboxSnapshot(f.client).rows[0]?.record.text).toBe('newer pending');expect(mobileOutboxSnapshot(f.client).rows[0]?.token).toBe('new-epoch:4');
});

test('lookup validates all known transitions before adopting any new claim',async()=>{
 const f=await fixture(inventory([claim()])),changed=claim();changed.capture!.draft.text='changed';changed.record!.text='changed';
 const pending=lookup(f.client,f.native,target());f.answer(1,{complete:true,fingerprint:null,claims:[claim('new'),changed]});
 await expect(pending).rejects.toThrow();expect(mobileOutboxSnapshot(f.client).threadTransfers.map(c=>c.transferId)).toEqual(['message']);
});

test('ordinary disposition is an exact vocabulary string, never array coercion',async()=>{
 const f=await fixture(),sending=enqueue(f.client,f.native,record(),capture());f.answer(1,{disposition:['created'],claim:claim(),outcome:outcome()});
 expect((await sending).disposition).toBe('unknown');expect(mobileOutboxSnapshot(f.client).threadTransfers).toEqual([]);
});

test('ordinary lookup and enqueue return retained terminal receipt after later completion',async()=>{
 for(const kind of ['lookup','enqueue'] as const){
  const f=await fixture(inventory([claim()]));
  const earlier=kind==='lookup'?lookup(f.client,f.native,target()):enqueue(f.client,f.native,record('new'),capture());
  const finishing=complete(f.client,f.native,claim());f.answer(2,{completed:true,claim:claim('message','completed')});await finishing;
  f.answer(1,kind==='lookup'?{complete:true,fingerprint:null,claims:[claim()]}:{disposition:'existing',claim:claim(),outcome:outcome()});
  const response=await earlier,owner='claims' in response?response.claims[0]:response.claim;
  expect(owner?.state).toBe('completed');expect(owner?.capture).toBeNull();
 }
 const f=await fixture(inventory([claim('message','completed')])),checking=status(f.client,f.native,target(),'message');
 f.answer(1,{claim:null,outcome:null});expect((await checking).claim?.state).toBe('completed');
});
