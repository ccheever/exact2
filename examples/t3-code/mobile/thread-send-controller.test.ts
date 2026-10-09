// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {expect,test} from 'bun:test';
import {createHash} from 'node:crypto';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileThreadSendRead as read,mobileThreadSendSubmit as submit,type ThreadSendControllerInput,type ThreadSendLocalCommands} from './thread-send-controller';
import {mobileComposerTarget} from './composer-target';
import {mobileDraftChanged} from './draft';
import {mobileOutboxRead,mobileOutboxSnapshot,type MobileOutboxThreadTarget} from './mobile-outbox';
import {mobileComposerContextsHydrate} from './composer-command-context';
import {queuedEditState} from './queued-edit-state';
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
