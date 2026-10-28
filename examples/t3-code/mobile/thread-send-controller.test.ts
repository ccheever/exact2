// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {expect,test} from 'bun:test';
import {createHash} from 'node:crypto';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileThreadSendRead as read,mobileThreadSendSubmit as submit,type ThreadSendControllerInput,type ThreadSendLocalCommands} from './thread-send-controller';
import {mobileComposerTarget} from './composer-target';
import {mobileDraftChanged} from './draft';
import {mobileOutboxRead,mobileOutboxSnapshot,type MobileOutboxThreadTarget} from './mobile-outbox';
import {mobileComposerContextsHydrate} from './composer-command-context';
import { queuedEditState } from './queued-edit-memory';
import type {ThreadSendTransferClaim} from './thread-send-transfer-model';
import type {ThreadSendRecord} from './thread-send-admission';
import {mobileOutboxTransferCanonical as canonical} from './mobile-outbox-transfer-model';
import {fleet} from './shared/settings-b-fleet';
import {arr,obj,type Obj} from './shared/domain';
import {collectProviderUsageLimits} from './thread-local-usage-model';
import {stage} from './shared/composer-controls';
import {mobileOutboxSettingsRequests} from './mobile-outbox-wire';
import type {MobileOutboxRecord} from './mobile-outbox-model';
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
  Object.assign(client,{providerId:'provider',modelId:'model',runtimeMode:'full-access',interactionMode:'plan',config:{providers:[{instanceId:'provider',driver:'codex',showInteractionModeToggle:true}]}});
  Object.assign(client.shell,{threads:[{id:'thread',projectId:'project',activeProviderThreadId:'provider-thread'}]});
  const input:ThreadSendControllerInput&{now:number;current():boolean}={expectedOwner:target.owner,alternate:false,preferences:{followUpBehavior:'queue',planModeEnabled:false},activity:{contextImporting:false,pendingPastedText:false,uploadStates:{},uploadOwners:{}},now,current:()=>true};
  events.length=0;snapshots.length=0;
  return {client,target,scope,input,native,storage,events,requests,snapshots,disk:()=>copy(saved),claim:()=>copy(claim),completed:()=>complete,
    setUncertain(value:boolean){uncertain=value},setComplete(value:boolean){inventoryComplete=value},failSave(count=1){failPersist=count},intercept(next:typeof hook){hook=next},setClaim(next:ThreadSendTransferClaim|null){claim=copy(next)}};
}
const commands:ThreadSendLocalCommands={async usageLimits(){throw Error('Unexpected usage callback')},async feedback(){throw Error('Unexpected feedback callback')}};
type Fixture=Awaited<ReturnType<typeof fixture>>;
const id=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
function file(f:Fixture,n=1){return {id:id(n),contextId:`file_${n}`,draftKey:f.target.key,environmentId:f.target.environmentId,
 name:`${n}.txt`,mimeType:'text/plain',sizeBytes:21,source:'attached',attachmentId:'',status:'staged'};}
function files(f:Fixture,rows:Obj[],order:string[]=rows.map(r=>String(r.id))){Object.assign(f.client.local,{composerFiles:rows,mobileAttachmentOrder:{[f.target.key]:order}})}
function ready(f:Fixture){const r=read(f.client,f.input);expect(r.kind).toBe('ready');if(r.kind!=='ready')throw Error(JSON.stringify(r));return r}
function projection(f:Fixture,value:Obj){Object.assign(f.client,{thread:{projection:value}})}
function context(f:Fixture,records:Obj[]){mobileComposerContextsHydrate(f.client,{mobileComposerContexts:{version:1,entries:{
 [JSON.stringify([f.client.origin,f.target.environmentId,f.target.key])]:{origin:f.client.origin,environmentId:f.target.environmentId,key:f.target.key,revision:1,text:f.client.draft,context:{version:1,records}}}}})}
function gate(){let resolve!:()=>void;const promise=new Promise<void>(done=>{resolve=done});return {promise,resolve}}

test('read is detached and read-only; offline ordinary Send enters real transfer with trim-only Plan settings',async()=>{
 const f=await fixture(),before=copy(f.client.local),revision=f.client.revision;
 const r=ready(f);expect(r.plan.kind).toBe('message');expect(r.facts.connected).toBe(false);expect(r.hasContent).toBe(true);
 expect(f.client.local).toEqual(before);expect(f.client.revision).toBe(revision);expect(f.events).toEqual([]);
 r.snapshot.rawText='mutated';r.facts.config!.providers=[];expect(f.client.draft).toBe('  captured  ');expect(f.client.config.providers).toHaveLength(1);
 const result=await submit(f.client,f.native,f.storage,f.input,commands);expect(result.kind).toBe('transfer');
 if(result.kind!=='transfer')return;expect(result.result.status).toBe('completed');expect(f.client.draft).toBe('');
 const queued=f.requests.find(r=>r.action==='enqueueTransfer')!;expect(obj(queued.record).text).toBe('captured');expect(obj(queued.record).interactionMode).toBe('plan');
 expect(obj(obj(queued.capture).draft).text).toBe('  captured  ');expect(f.requests.some(r=>r.op==='request')).toBe(false);
});
test('file-only and mixed ordered attachments use canonical local IDs and full supplied context',async()=>{
 const f=await fixture();f.client.local.drafts[f.target.key]='  ';files(f,[file(f,1),file(f,3)],[id(3),id(2),id(1)]);
 f.client.local.snapshotDrafts[f.target.key]=[{id:id(2),name:'2.png',mimeType:'image/png',sizeBytes:11}];
 context(f,['a','b'].map(contextId=>({version:1,kind:'file',contextId,label:'1.txt',attachmentId:id(1),name:'1.txt',mimeType:'text/plain',sizeBytes:21})));
 const r=ready(f);expect(r.hasContent).toBe(true);if(r.plan.kind!=='message')throw Error('message');
 expect(r.plan.record.attachments.map(a=>a.id)).toEqual([id(3),id(2),id(1)]);expect(r.plan.record.context!.records).toHaveLength(2);
 const result=await submit(f.client,f.native,f.storage,f.input,commands);expect(result.kind).toBe('transfer');if(result.kind==='transfer')expect(result.result.status).toBe('completed');
 expect(obj(f.requests.find(r=>r.action==='enqueueTransfer')!.record).text).toBe('');
});
test('staged model/options/modes win at tap and remain immutable through later native awaits',async()=>{
 const f=await fixture();Object.assign(f.client.local,{composerControls:{staged:{[f.target.key]:{providerId:'other',modelId:'new-model',options:[{id:'reasoning',value:'high'}],runtimeMode:'auto',interactionMode:'plan'}}}});
 f.client.config.providers=[{instanceId:'other',driver:'claudeAgent',showInteractionModeToggle:false}];
 const wait=gate();f.intercept(async r=>{if(r.action==='read')await wait.promise});const pending=submit(f.client,f.native,f.storage,f.input,commands);
 Object.assign(f.client,{providerId:'later',modelId:'later',runtimeMode:'approval-required',interactionMode:'default'});f.input.preferences.followUpBehavior='steer';wait.resolve();
 const result=await pending;expect(result.kind).toBe('transfer');if(result.kind==='transfer')expect(result.result.status).toBe('completed');
 const record=obj(f.requests.find(r=>r.action==='enqueueTransfer')!.record);expect(record.modelSelection).toEqual({instanceId:'other',model:'new-model',options:[{id:'reasoning',value:'high'}]});
 expect(record.runtimeMode).toBe('auto');expect(record.interactionMode).toBe('default');
});
test('sparse thread settings capture the explicit model group and fall back to current modes and model',async()=>{
 const f=await fixture(),key=f.target.key,high=[{id:'reasoning',value:'high'}];
 Object.assign(f.client.local,{composerControls:{staged:{[key]:{providerId:'provider',modelId:'b',options:high}}}});
 expect(ready(f).snapshot).toMatchObject({modelSelection:{instanceId:'provider',model:'b',options:high},runtimeMode:'full-access',interactionMode:'plan'});
 Object.assign(f.client.local,{composerControls:{staged:{[key]:{runtimeMode:'approval-required'}}}});
 expect(ready(f).snapshot).toMatchObject({modelSelection:{instanceId:'provider',model:'model',options:[]},runtimeMode:'approval-required',interactionMode:'plan'});
 for(const half of [{providerId:'provider'},{providerId:'provider',modelId:'b'},{modelId:'b',options:[]},{providerId:'provider',modelId:7,options:[]},{runtimeMode:7}]){
  Object.assign(f.client.local,{composerControls:{staged:{[key]:half}}});expect(read(f.client,f.input).kind).toBe('blocked');
 }
});
test('a pick after the tap leaves the queued record on the captured model and the thread setting on the new one',async()=>{
 const f=await fixture(),key=f.target.key;Object.assign(f.client.local,{composerControls:{staged:{[key]:{providerId:'provider',modelId:'b',options:[]}}}});
 const wait=gate();f.intercept(async r=>{if(r.action==='read')await wait.promise});const pending=submit(f.client,f.native,f.storage,f.input,commands);
 stage(f.client,{providerId:'provider',modelId:'c',options:[]});wait.resolve();
 const result=await pending;expect(result.kind).toBe('transfer');if(result.kind==='transfer')expect(result.result.status).toBe('completed');
 expect(obj(f.requests.find(r=>r.action==='enqueueTransfer')!.record).modelSelection).toEqual({instanceId:'provider',model:'b',options:[]});
 expect(f.client.local.composerControls.staged[key]).toEqual({providerId:'provider',modelId:'c',options:[]});expect(f.client.modelId).toBe('c');
});
test('a runtime-only thread setting is synchronized before the queued turn and survives Send',async()=>{
 const f=await fixture(),key=f.target.key;Object.assign(f.client,{runtimeMode:'full-access',interactionMode:'default'});
 stage(f.client,{runtimeMode:'approval-required'});
 const result=await submit(f.client,f.native,f.storage,f.input,commands);expect(result.kind).toBe('transfer');if(result.kind==='transfer')expect(result.result.status).toBe('completed');
 const record=f.requests.find(r=>r.action==='enqueueTransfer')!.record as MobileOutboxRecord;
 expect(record).toMatchObject({modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'approval-required'});
 expect(f.client.local.composerControls.staged[key]).toEqual({runtimeMode:'approval-required'});
 expect(obj(obj(f.disk().composerControls).staged)[key]).toEqual({runtimeMode:'approval-required'});
 const wire={origin:record.origin,environmentId:record.environmentId,attachments:[],
  config:{environment:{environmentId:record.environmentId,capabilities:{serverResolvedCommandContext:true}},providers:f.client.config.providers}};
 const thread={origin:record.origin,environmentId:record.environmentId,threadId:'thread',modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'full-access' as const,interactionMode:'default' as const};
 const sync=mobileOutboxSettingsRequests(record,wire,thread);expect(sync.status).toBe('ready');
 if(sync.status==='ready')expect(sync.value.map(r=>[r.stage,r.payload])).toEqual([['settings-sync',{type:'thread.runtime-mode.set',commandId:`${record.commandId}:runtime-mode`,threadId:'thread',runtimeMode:'approval-required'}]]);
});
test('cached model choices survive missing provider rows without inventing a local-command driver',async()=>{
 const f=await fixture();f.client.config={settings:{providerInstances:{provider:{driver:'claudeAgent'}}}};expect(ready(f).snapshot.providerDriver).toBe('unknown');
 f.client.config={};expect(read(f.client,f.input).kind).toBe('ready');
 f.client.local.drafts[f.target.key]='/feedback';f.client.config={settings:{providerInstances:{provider:{driver:'codex'}}}};
 expect(ready(f).plan.kind).toBe('message');expect(ready(f).snapshot.providerDriver).toBe('unknown');
 f.client.config={settings:{providerInstances:{provider:{driver:'antigravity'}}}};expect(read(f.client,f.input).kind).toBe('ready');
 f.client.connection='connected';f.client.scopes=['orchestration:operate'];expect(read(f.client,f.input)).toMatchObject({kind:'blocked',reason:expect.stringContaining('Antigravity')});
});
test('connected scope denial, required import/paste facts and failed compatible upload block without IO',async()=>{
 const f=await fixture();f.client.connection='connected';expect(read(f.client,f.input).kind).toBe('blocked');f.client.scopes=['orchestration:operate'];
 for(const field of ['contextImporting','pendingPastedText'] as const){f.input.activity[field]=true;expect(read(f.client,f.input).kind).toBe('blocked');f.input.activity[field]=false}
 files(f,[file(f)]);f.client.config.environment={capabilities:{attachmentUploads:true,fileAttachments:{maxUploadBytes:100000}}};
 f.input.activity.uploadStates[id(1)]='failed';expect(read(f.client,f.input).kind).toBe('blocked');f.client.connection='disconnected';expect(read(f.client,f.input).kind).toBe('ready');
 expect(f.events).toEqual([]);
});
test('remote upload IDs require exact provenance and legacy memory files are never silently omitted',async()=>{
 const f=await fixture();files(f,[{...file(f),attachmentId:'remote',status:'ready'}]);expect(read(f.client,f.input).kind).toBe('blocked');
 f.input.activity.uploadOwners[id(1)]={origin:'https://wrong.test',environmentId:f.target.environmentId};expect(read(f.client,f.input).kind).toBe('blocked');
 f.input.activity.uploadOwners[id(1)]={origin:f.client.origin,environmentId:f.target.environmentId};expect(read(f.client,f.input).kind).toBe('ready');
 files(f,[{...file(f),source:'pasted-text'}]);expect(read(f.client,f.input).kind).toBe('blocked');expect(f.events).toEqual([]);
});
test('actual queue capability and running attempt control source alternate dispatch',async()=>{
 const f=await fixture();projection(f,{thread:{id:'thread',activeProviderThreadId:'pt'},runs:[{id:'run',status:'running',providerThreadId:'pt',activeAttemptId:'attempt'}],
 providerThreads:[{id:'pt',providerSessionId:'session'}],providerSessions:[{id:'session',capabilities:{turns:{supportsActiveSteering:true}}}],providerTurns:[{runAttemptId:'attempt',status:'running'}]});
 let r=ready(f);if(r.plan.kind!=='message')throw Error('message');expect(r.plan.record.dispatchMode).toBe('queue');
 f.input.alternate=true;r=ready(f);if(r.plan.kind!=='message')throw Error('message');expect(r.plan.record.dispatchMode).toBe('auto');
 obj(f.client.projection).providerTurns=[];r=ready(f);if(r.plan.kind!=='message')throw Error('message');expect(r.plan.record).not.toHaveProperty('dispatchMode');
});
test('NewTask, questions and queued edit are separate lanes without draft enrollment or native work',async()=>{
 const f=await fixture();f.client.threadId='';expect(read(f.client,f.input)).toEqual({kind:'separate',lane:'new-task'});f.client.threadId='thread';
 projection(f,{runtimeRequests:[{id:'question',kind:'user_input',status:'pending'}],turnItems:[{type:'user_input_request',requestId:'question',questions:[]}]});
 expect(read(f.client,f.input)).toEqual({kind:'separate',lane:'question'});projection(f,{});
 const state=queuedEditState(f.client);state.active.set(`${f.target.environmentId}:thread`,'edit');state.sessions.set('edit',{owner:'edit',draftKey:'queued',origin:f.client.origin,
 environmentId:f.target.environmentId,threadId:'thread',projectId:'project',generation:4,session:'s',revision:0,runId:'r',messageId:'m',text:'q',attachments:[],existingAttachments:[],saving:false});
 expect(read(f.client,f.input)).toEqual({kind:'separate',lane:'queued-edit'});expect(f.events).toEqual([]);expect(f.client.draft).toBe('  captured  ');
});
test('queued thread creation remains its own lane even after a real shell thread appears',async()=>{
 const f=await fixture();const record={schemaVersion:1,origin:f.client.origin,environmentId:f.target.environmentId,threadId:'thread',messageId:'creation',commandId:'create-command',
 text:'initial',attachments:[],createdAt:new Date(now).toISOString(),creation:{projectId:'project',workspaceMode:'local',branch:null,worktreePath:null}};
 const native:Native={available:true,watch(){},async later(){return {ok:true,generation:0,value:{ownerEpoch:'creation-epoch',sequenceFloor:1,complete:true,errors:[],
 records:[{record,revision:1,token:'create-mutation',pending:false,held:false}],revisions:{creation:1},tokens:{creation:'create-mutation'},outcomes:[],mutations:[],transfers:[]}}}};
 expect(await mobileOutboxRead(f.client,native)).toBe(true);expect(read(f.client,f.input)).toEqual({kind:'separate',lane:'pending-creation'});expect(f.events).toEqual([]);
});
test('whole malformed raw foreign storage and malformed staged settings refuse without normalizing recovery bytes',async()=>{
 for(const kind of ['foreign-file','cleanup','stage','activity'] as const){const f=await fixture();
 if(kind==='foreign-file')files(f,[{...file(f),draftKey:'other:thread',environmentId:'other',status:['staged']}]);
 if(kind==='cleanup')Object.assign(f.client.local,{mobileNewTaskDrafts:null});
 if(kind==='stage')Object.assign(f.client.local,{composerControls:{staged:{[f.target.key]:{providerId:'provider',modelId:'model',options:[],runtimeMode:'full-access',interactionMode:['plan']}}}});
 if(kind==='activity')Object.assign(f.input.activity,{contextImporting:undefined});
 const before=structuredClone(f.client.local),revision=f.client.revision;expect(read(f.client,f.input).kind).toBe('blocked');
 expect(f.client.local).toEqual(before);expect(f.client.revision).toBe(revision);expect(f.events).toEqual([]);
 }
});
test('usage command uses captured staged provider and callback only; unavailable result preserves draft',async()=>{
 const f=await fixture();f.client.local.drafts[f.target.key]=' /USAGE-LIMITS ';f.client.connection='connected';
 f.client.config.providers=[{instanceId:'provider',driver:'codex',enabled:true,installed:true,auth:{status:'authenticated'},usageLimits:{checkedAt:'2026-10-09T12:00:00Z',windows:[]}}];let called=0;
 const result=await submit(f.client,f.native,f.storage,f.input,{...commands,async usageLimits(c){called++;expect(c.snapshot.rawText).toBe(' /USAGE-LIMITS ');expect(c.now).toBe(now);return {accepted:false,message:'Unavailable'}}});
 expect(result).toMatchObject({kind:'local',command:'usage-limits',result:{accepted:false,message:'Unavailable'}});expect(called).toBe(1);
 expect(f.client.draft).toBe(' /USAGE-LIMITS ');expect(f.events).toEqual([]);
});
test('Codex feedback callback captures reason and current owner; attachments turn it into ordinary content',async()=>{
 const f=await fixture();f.client.local.drafts[f.target.key]=' /feedback  useful ';f.client.scopes=['orchestration:operate'];let called=0;
 const result=await submit(f.client,f.native,f.storage,f.input,{...commands,async feedback(c,_n,_s,reason){called++;expect(reason).toBe('useful');expect(c.snapshot.activeProviderThreadId).toBe('provider-thread');return {accepted:true,message:''}}});
 expect(result.kind).toBe('local');expect(called).toBe(1);expect(f.events).toEqual([]);files(f,[file(f)]);expect(ready(f).plan.kind).toBe('message');
});
test('cancellation and stale route never become fallback provider sends',async()=>{
 const f=await fixture();f.input.current=()=>false;await expect(submit(f.client,f.native,f.storage,f.input,commands)).rejects.toHaveProperty('kind','superseded');expect(f.events).toEqual([]);
 f.input.current=()=>true;f.client.local.drafts[f.target.key]='/usage-limits';f.client.config.providers=[{instanceId:'provider',driver:'codex',enabled:true,installed:true,auth:{status:'authenticated'},usageLimits:{checkedAt:'2026-10-09T12:00:00Z',windows:[]}}];
 const sentinel={name:'FetchError',kind:'Aborted'};await expect(submit(f.client,f.native,f.storage,f.input,{...commands,async usageLimits(){throw sentinel}})).rejects.toBe(sentinel);expect(f.events).toEqual([]);
});
test('later typing is preserved by actual handoff and tap scope remains captured after route changes',async()=>{
 const f=await fixture();f.intercept(async request=>{if(request.action==='enqueueTransfer')await mobileDraftChanged(f.client,'later',f.native,f.storage,f.target.owner)});
 const result=await submit(f.client,f.native,f.storage,f.input,commands);expect(result.kind).toBe('transfer');if(result.kind==='transfer'){expect(result.scope).toEqual(f.target);expect(result.result.status).toBe('completed')}
 expect(f.client.draft).toBe('later');expect(obj(obj(f.disk().mobileOutboxTransferCompletions).message).disposition).toBe('preserved');
});

test('usage offering reuses source available-provider filter and account/error coverage',async()=>{
 const f=await fixture();f.client.local.drafts[f.target.key]='/usage-limits';
 const provider={instanceId:'provider',driver:'codex',enabled:true,installed:true,auth:{status:'authenticated'},usageLimits:{checkedAt:'2026-10-09T12:00:00Z',windows:[]}};
 for(const patch of [{enabled:false},{installed:false},{availability:'unavailable'}]){f.client.config.providers=[{...provider,...patch}];expect(ready(f).plan.kind).toBe('message')}
 f.client.config.providers=[provider];expect(ready(f).plan.kind).toBe('usage-limits');
 f.client.config.providers=[{...provider,enabled:false}];f.client.config.usageLimitSources=[{id:'source',label:'Source',error:'unavailable',accounts:[]}];expect(ready(f).plan.kind).toBe('usage-limits');
 f.client.config.usageLimitSources=[{id:'source',label:'Source',accounts:[{id:'a',driver:'codex',usageLimits:provider.usageLimits}]}];expect(ready(f).plan.kind).toBe('usage-limits');
});
test('Send reads named policy fields without inspecting excluded getters, nested catalogues or toJSON',async()=>{
 const f=await fixture(),trap=():never=>{throw Error('Excluded catalogue was inspected')};
 const catalogue=new Proxy({},{ownKeys:trap});
 const hidden=(value:Obj)=>{
  Object.defineProperties(value,{skills:{enumerable:true,get:trap},toJSON:{enumerable:true,get:trap}});
  value.workspaceSnapshots=catalogue;value.slashCommands=catalogue;return new Proxy(value,{ownKeys:trap});
 };
 const provider:Obj={instanceId:'provider',driver:'codex',enabled:false,installed:null,availability:null,auth:null,usageLimits:null,
  showInteractionModeToggle:false,models:[hidden({slug:'model'})]};
 const other:Obj={instanceId:'provider',driver:'antigravity'};Object.defineProperties(other,{models:{enumerable:true,get:trap},showInteractionModeToggle:{enumerable:true,get:trap}});
 f.client.config=hidden({environment:hidden({capabilities:hidden({attachmentUploads:null,fileAttachments:hidden({maxUploadBytes:21})})}),
  providers:[hidden(provider),hidden(other)],settings:hidden({providerInstances:hidden({provider:hidden({driver:'codex'}),other:catalogue})}),usageLimitSources:[]});
 const r=ready(f);expect(r.facts.config).toEqual({environment:{capabilities:{attachmentUploads:null,fileAttachments:{maxUploadBytes:21}}},
  providers:[{instanceId:'provider',driver:'codex',enabled:false,installed:null,availability:null,auth:null,usageLimits:null,showInteractionModeToggle:false,models:[{slug:'model'}]},
   {instanceId:'provider',driver:'antigravity'}],settings:{providerInstances:{provider:{driver:'codex'}}},usageLimitSources:[]});
 expect(r.snapshot.showInteractionModeToggle).toBe(false);expect(r.plan.kind==='message'&&r.plan.record.interactionMode).toBe('default');
 delete provider.showInteractionModeToggle;expect(ready(f).snapshot.showInteractionModeToggle).toBe(true);
 expect(Object.hasOwn(arr(ready(f).facts.config?.providers)[0]!,'showInteractionModeToggle')).toBe(false);
 provider.showInteractionModeToggle=null;expect(arr(ready(f).facts.config?.providers)[0]!.showInteractionModeToggle).toBeNull();
 expect(f.events).toEqual([]);
});
test('retained malformed policy refuses before IO or draft mutation without invoking accessors',async()=>{
 const f=await fixture(),before=copy(f.client.local),revision=f.client.revision;let invoked=0;
 const accessor=(key:string):Obj=>Object.defineProperty({},key,{enumerable:true,get(){invoked++;throw Error('Accessor was invoked')}});
 const cyclic:Obj={};cyclic.self=cyclic;
 const providers=new Array(1),models=new Array(1),accounts=new Array(1);
 const invalid:unknown[]=[null,{environment:null},{environment:{capabilities:{fileAttachments:false}}},{providers:{}},{providers:[null]},
  {providers},{providers:[{instanceId:'provider',models}]},{providers:[{instanceId:'provider',models:[null]}]},
  {settings:{providerInstances:{provider:null}}},{usageLimitSources:[{accounts:false}]},{usageLimitSources:[{accounts}]},
  {providers:[Object.assign(accessor('driver'),{instanceId:'provider'})]},
  {providers:[Object.assign(accessor('auth'),{instanceId:'provider'})]},{environment:{capabilities:accessor('attachmentUploads')}},
  {providers:[{instanceId:'provider',auth:cyclic}]},{usageLimitSources:[{accounts:[],error:accessor('message')}]}];
 for(const value of [undefined,NaN,Infinity,()=>true,Symbol('invalid'),1n,new Date(),new Array(1)])invalid.push({providers:[{instanceId:'provider',usageLimits:value}]});
 const ancestor:Obj={};ancestor.environment=ancestor;invalid.push(ancestor);
 for(const config of invalid){Object.assign(f.client,{config});expect((await submit(f.client,f.native,f.storage,f.input,commands)).kind).toBe('blocked');
  expect(f.client.local).toEqual(before);expect(f.client.revision).toBe(revision);expect(f.events).toEqual([])}
 expect(invoked).toBe(0);
});
test('selected Antigravity policy and inclusive upload cap are recaptured on every tap',async()=>{
 const f=await fixture();f.client.connection='connected';f.client.scopes=['orchestration:operate'];
 const provider={instanceId:'provider',driver:'antigravity',enabled:true,installed:true,auth:{status:'authenticated'},models:[{slug:'model'}]};
 f.client.config={providers:[provider]};expect(ready(f).plan.kind).toBe('message');
 for(const patch of [{enabled:false},{installed:false},{availability:'unavailable'},{auth:{status:'unauthenticated'}},{models:[{slug:'other'}]}]){
  f.client.config.providers=[{...provider,...patch}];expect(read(f.client,f.input)).toMatchObject({kind:'blocked',reason:expect.stringContaining('Antigravity')});
 }
 f.client.config.providers=[provider];files(f,[file(f)]);f.input.activity.uploadStates[id(1)]='failed';
 for(const [cap,kind] of [[20,'ready'],[21,'blocked'],[22,'blocked']] as const){f.client.config.environment={capabilities:{attachmentUploads:true,fileAttachments:{maxUploadBytes:cap}}};expect(read(f.client,f.input).kind).toBe(kind)}
 f.client.config.environment={capabilities:{attachmentUploads:false,fileAttachments:{maxUploadBytes:21}}};expect(read(f.client,f.input).kind).toBe('ready');
 f.client.connection='disconnected';f.client.config.environment={capabilities:{attachmentUploads:true,fileAttachments:{maxUploadBytes:21}}};expect(read(f.client,f.input).kind).toBe('ready');
 expect(f.events).toEqual([]);
});
test('captured local usage keeps complete pooled reports and detaches raw metadata before its first await',async()=>{
 const f=await fixture();f.client.local.drafts[f.target.key]='/usage-limits';
 const limits=(checkedAt:string,availableCount:number,credentialFingerprint:string):Obj=>({checkedAt,credentialFingerprint,
  windows:[{id:'primary',kind:'primary',label:'Raw daily',usedPercent:27,resetsAt:'2026-10-10T00:00:00Z',windowDurationMins:1440,extra:{raw:true}}],
  resetCredits:{availableCount,nextCreditId:`credit-${availableCount}`},externalUsage:{label:'Usage',url:'https://usage.test/limits'}});
 const first={instanceId:'provider',driver:'codex',enabled:true,installed:true,displayName:'Personal',accentColor:'#123456',
  auth:{status:'authenticated',email:'a@example.com',label:'Plus',subscriptionSharing:true,extra:{raw:true}},usageLimits:limits('2026-10-09T12:00:00Z',3,'first')};
 const second={...first,instanceId:'second',displayName:'Work',auth:{status:'authenticated',label:'Team'},usageLimits:limits('2026-10-09T11:00:00Z',1,'second')};
 const sources=[{id:'hub',label:'Hub',accounts:[{id:'same-email',driver:'codex',email:' A@EXAMPLE.COM ',usageLimits:limits('2026-10-09T10:00:00Z',2,'first')},
  {id:'same-credential',driver:'codex',usageLimits:limits('2026-10-09T13:00:00Z',5,'second')},
  {id:'hub-only',driver:'codex',email:'hub@example.com',plan:'Pro',usageLimits:limits('2026-10-09T12:00:00Z',4,'third')}]},
  {id:'failed',label:'Failed source',error:{message:'Read failed'},accounts:[]},
  {id:'empty',label:'Empty source',error:'Offline',accounts:[]}];
 f.client.config={providers:[first,second,{...first,instanceId:'other-driver',driver:'claudeAgent'},{...first,instanceId:'unavailable',availability:'unavailable'}],usageLimitSources:sources};
 const expected=copy(collectProviderUsageLimits('provider',arr(f.client.config.providers),sources,now));
 expect(expected?.accounts).toHaveLength(3);expect(expected?.notices).toEqual(['Failed source: Read failed','Empty source: Offline']);
 expect(obj(expected?.accounts[0]!.limits.resetCredits).availableCount).toBe(3);
 expect(obj(expected?.accounts[1]!.limits.resetCredits).availableCount).toBe(5);
 expect(expected?.accounts[0]!.resetCreditInput).toEqual({sourceId:'hub',accountId:'same-email',creditId:'credit-2'});
 const wait=gate();let entered=false;
 const pending=submit(f.client,f.native,f.storage,f.input,{...commands,async usageLimits(c){entered=true;await wait.promise;
  expect(c.facts.usageLimitsOffered).toBe(true);expect(arr(c.facts.config?.providers)).toHaveLength(4);
  expect(obj(arr(c.facts.config?.providers)[0]!.auth).extra).toEqual({raw:true});
  expect(collectProviderUsageLimits('provider',arr(c.facts.config?.providers),arr(c.facts.config?.usageLimitSources),now)).toEqual(expected);
  return {accepted:false,message:'Captured'};
 }});
 expect(entered).toBe(true);first.auth.extra.raw=false;first.usageLimits.windows=[];sources[0]!.accounts=[];
 f.client.config={};wait.resolve();expect(await pending).toMatchObject({kind:'local',command:'usage-limits',result:{accepted:false,message:'Captured'}});
 expect(f.client.draft).toBe('/usage-limits');expect(f.events).toEqual([]);
});
