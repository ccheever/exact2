// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {expect,test} from 'bun:test';
import {createHash} from 'node:crypto';
import {mobileThreadSendRootSnapshot as snapshot,mobileThreadSendRootAction as action,type ThreadSendRootInput,type ThreadSendRootAction} from './thread-send-root';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileThreadSendCaptureDraft} from './thread-send-handoff-draft';
import {queuedEditState} from './queued-edit-state';
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
  const client=new MobileDraftClient(),environmentId=options.environmentId??`root-transfer-${++serial}`;
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
        revisions:claim?{[claim.messageId]:1}:{},tokens:claim?{[claim.messageId]:claim.mutationId}:{},outcomes:[],mutations:claim?.state==='prepared'?[{messageId:claim.messageId,state:'pending',mutation:{operation:'enqueue',mutationId:claim.mutationId,record:copy(claim.record),transfer:{...copy(claim),outcome:null}}}]:[],transfers:claim?[copy(claim)]:[]};
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
        claim={...claim!,state:r.decision==='rollback'?'failed':'queued'};inventoryComplete=true;value=outcome();
      }else if(r.action==='releaseFailedTransfer'){claim={...claim!,state:'released',record:null,capture:null};value={released:true,claim:copy(claim)}}
      else throw Error(`Unexpected action ${r.action}`);
    }else if(r.op==='request'&&r.method==='provider.uploadFeedback')value={feedbackId:'feedback-id'};
    else if(r.op==='request'&&r.method==='provider.consumeResetCredit')value={outcome:'reset'};
    else if(r.op==='mobileScheduledConfirm')value={confirmed:true};
    else if(r.op==='copyText')value={copied:true};
    else if(r.op==='mobileHomeHaptic')value={};
    else if(r.op==='mobileOpenURL')value={opened:true};
    else throw Error(`Unexpected op ${r.op}`);
    await hook?.(r);return {ok:true,generation:r.generation??client.generation,value};
  }};
  const record:ThreadSendRecord={schemaVersion:1,...{origin:scope.origin,environmentId,threadId:'thread'},text:client.draft.trim(),attachments:[],modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'full-access',interactionMode:'default'};
  Object.assign(client,{providerId:'provider',modelId:'model',runtimeMode:'full-access',interactionMode:'plan',config:{providers:[{instanceId:'provider',driver:'codex',showInteractionModeToggle:true}]}});
  Object.assign(client.shell,{threads:[{id:'thread',projectId:'project',activeProviderThreadId:'provider-thread'}]});
  Object.assign(client.shell.threads[0]!,{modelSelection:{instanceId:'provider',model:'model'},latestRun:null});
  const input:ThreadSendRootInput={visit:'visit-a',url:`/threads/${environmentId}/thread`,active:true,environmentId,threadId:'thread',
    preferencesReady:true,preferencesJSON:JSON.stringify({followUpBehavior:'queue',planModeEnabled:false}),contextImporting:false,now,viewportHeight:800,dark:false};
  events.length=0;snapshots.length=0;
  return {client,target,scope,input,native,storage,events,requests,snapshots,disk:()=>copy(saved),claim:()=>copy(claim),completed:()=>complete,
    setUncertain(value:boolean){uncertain=value},setComplete(value:boolean){inventoryComplete=value},failSave(count=1){failPersist=count},intercept(next:typeof hook){hook=next},setClaim(next:ThreadSendTransferClaim|null){claim=copy(next)}};
}
type Fixture=Awaited<ReturnType<typeof fixture>>;
const view=(f:Fixture)=>snapshot(f.client,f.input);
const run=(f:Fixture,op:ThreadSendRootAction['op'],key='',value='')=>action(f.client,f.native,f.storage,f.input,
 {op,key,value,expectedOwner:f.target.owner,visit:f.input.visit});
async function prepared(f:Fixture){const v=view(f);expect(v.needsRead).toBe(true);await run(f,'recovery-read',v.readKey);expect(view(f).needsRead).toBe(false)}
function gate(){let resolve!:()=>void;const promise=new Promise<void>(done=>{resolve=done});return {promise,resolve}}
function usage(f:Fixture){f.client.local.drafts[f.target.key]='/usage-limits';f.client.config.providers=[{instanceId:'provider',driver:'codex',enabled:true,installed:true,availability:'available',auth:{status:'authenticated'},
 usageLimits:{checkedAt:new Date(now).toISOString(),windows:[{id:'daily',kind:'session',label:'Daily',usedPercent:30}],
 resetCredits:{availableCount:1},externalUsage:{label:'Usage',url:'https://example.com/usage'}}}];}
function connected(f:Fixture){f.client.connection='connected';f.client.scopes=['orchestration:operate','providers:manage']}
const id=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;

test('unknown recovery gates initial Send; read key is stable through unrelated revisions and clock ticks',async()=>{
 const f=await fixture(),v=view(f);expect(v.ordinary).toBe(true);expect(v.canSend).toBe(false);expect(v.hasContent).toBe(true);expect(v.needsRead).toBe(true);
 f.client.revision++;f.input.now++;expect(view(f).readKey).toBe(v.readKey);
 await prepared(f);expect(view(f).canSend).toBe(true);const calls=f.requests.length;
 for(let i=0;i<5;i++){f.client.revision++;f.input.now++;expect(view(f).needsRead).toBe(false)}expect(f.requests).toHaveLength(calls);
});
test('pending recovery retains its scheduling key until settlement rather than cancelling itself',async()=>{
 const f=await fixture(),v=view(f),held=gate();f.intercept(async r=>{if(r.action==='read')await held.promise});
 const answer=run(f,'recovery-read',v.readKey);expect(view(f).busy).toBe(true);expect(view(f).needsRead).toBe(true);expect(view(f).readKey).toBe(v.readKey);
 f.client.revision++;expect(view(f).readKey).toBe(v.readKey);held.resolve();await answer;expect(view(f).needsRead).toBe(false);expect(view(f).canSend).toBe(true);
});
test('same-scope route retirement rejects old reads and a new visit has fresh independent admission',async()=>{
 const f=await fixture(),v=view(f),held=gate();let first=true;f.intercept(async r=>{if(r.action==='read'&&first){first=false;await held.promise}});
 const old=run(f,'recovery-read',v.readKey);f.input.active=false;view(f);f.input.active=true;f.input.visit='visit-b';const next=view(f);expect(next.readKey).not.toBe(v.readKey);
 await run(f,'recovery-read',next.readKey);expect(view(f).canSend).toBe(true);held.resolve();await expect(old).rejects.toHaveProperty('kind','superseded');expect(view(f).canSend).toBe(true);
});
test('explicit input admission precedes any projection and stale visit/owner never sends',async()=>{
 const f=await fixture();await prepared(f);const before=f.requests.length;
 f.input.visit='fresh-without-projection';const stale=action(f.client,f.native,f.storage,f.input,{op:'send',key:'',value:'',expectedOwner:f.target.owner,visit:'visit-a'});
 await expect(stale).rejects.toHaveProperty('kind','superseded');expect(f.requests).toHaveLength(before);expect(view(f).needsRead).toBe(true);
 await expect(action(f.client,f.native,f.storage,f.input,{op:'send',key:'',value:'',expectedOwner:'wrong',visit:f.input.visit})).rejects.toHaveProperty('kind','superseded');
});
test('offline file-only ordinary Send uses facade and rechecks recovery once after completion',async()=>{
 const f=await fixture();f.client.local.drafts[f.target.key]='';Object.assign(f.client.local,{composerFiles:[{id:id(1),contextId:'file',draftKey:f.target.key,
 environmentId:f.target.environmentId,name:'file.txt',mimeType:'text/plain',sizeBytes:2,source:'attached',attachmentId:'',status:'staged'}]});
 await prepared(f);expect(view(f).hasContent).toBe(true);expect(view(f).canSend).toBe(true);
 expect((await run(f,'send')).accepted).toBe(true);expect(f.claim()?.state).toBe('completed');expect(view(f).needsRead).toBe(true);
 await run(f,'recovery-read',view(f).readKey);expect(view(f).needsRead).toBe(false);expect(view(f).hasContent).toBe(false);expect(view(f).canSend).toBe(false);
});
test('incomplete recovery stays blocked without automatic retry and offers explicit refresh',async()=>{
 const f=await fixture();f.setComplete(false);await prepared(f);const v=view(f);expect(v.canSend).toBe(false);expect(v.recovery.visible).toBe(true);
 const retry=v.recovery.actions.find(a=>a.label==='Refresh saved status')!;expect(retry).toBeDefined();const calls=f.requests.length;
 f.client.revision++;expect(view(f).needsRead).toBe(false);expect(f.requests).toHaveLength(calls);f.setComplete(true);
 await run(f,'recovery-action',retry.key);expect(view(f).canSend).toBe(true);
});
test('malformed ordinary store remains ordinary blocked; cached readiness does not gate root admission',async()=>{
 const f=await fixture();await prepared(f);expect(f.client.ready).toBe(false);expect(view(f).canSend).toBe(true);
 Object.assign(f.client.local,{mobileNewTaskDrafts:null});const before=copy(f.client.local);const v=view(f);expect(v.ordinary).toBe(true);expect(v.canSend).toBe(false);expect(v.reason).toContain('invalid');
 await run(f,'send');expect(f.client.local).toEqual(before);expect(f.requests.some(r=>r.action==='enqueueTransfer')).toBe(false);
});
test('active embedded Files route is allowed only by explicit parent admission; other routes retire Send',async()=>{
 const f=await fixture();f.input.url+='/files';await prepared(f);expect(view(f).canSend).toBe(true);
 f.input.url=f.input.url.replace('/files','/terminal');expect(view(f).ordinary).toBe(false);f.input.url=f.input.url.replace('/terminal','/files');f.input.active=false;expect(view(f).ordinary).toBe(false);
});
test('actual root import pending blocks Send and legacy remote IDs have no invented upload provenance',async()=>{
 const f=await fixture();await prepared(f);f.input.contextImporting=true;expect(view(f).canSend).toBe(false);f.input.contextImporting=false;
 f.client.local.snapshotDrafts[f.target.key]=[{id:id(1),name:'image.png',mimeType:'image/png',sizeBytes:2,uploadId:'remote'}];
 expect(view(f).canSend).toBe(false);expect(view(f).reason).toContain('upload owner');
});
test('real local usage clears exact text, exposes source report and dismisses with a new adapter revision',async()=>{
 const f=await fixture();usage(f);await prepared(f);expect((await run(f,'send')).accepted).toBe(true);expect(f.client.draft).toBe('');
 const v=view(f);expect(v.dock.hasUsage).toBe(true);expect(v.dock.accounts).toHaveLength(1);expect(f.requests.some(r=>r.action==='enqueueTransfer'||r.method==='provider.uploadFeedback')).toBe(false);
 const result=await run(f,'usage-close',v.dock.usageKey);expect(result.accepted).toBe(true);expect(result.revision).toBeGreaterThan(v.revision);expect(view(f).dock.hasUsage).toBe(false);
});
test('usage lifetime uses thread model/run/request key; staged settings alone do not forge its thread key',async()=>{
 const f=await fixture();usage(f);await prepared(f);await run(f,'send');const key=view(f).dock.usageKey;expect(key).toBe(`${f.target.environmentId}:thread:provider::`);
 Object.assign(f.client.shell.threads[0]!,{latestRun:{runId:'new-run'}});expect(view(f).dock.hasUsage).toBe(false);
 Object.assign(f.client.shell.threads[0]!,{latestRun:null});expect(view(f).dock.hasUsage).toBe(false);
});
test('feedback exposes uploading during concrete persistence, then exact RPC and original-thread sent notice',async()=>{
 const f=await fixture();connected(f);f.client.local.drafts[f.target.key]='/feedback useful';await prepared(f);const held=gate(),entered=gate();
 f.intercept(async r=>{if(r.method==='provider.uploadFeedback'){entered.resolve();await held.promise}});const sending=run(f,'send');
 await entered.promise;
 expect(view(f).dock.feedback[0]?.status).toBe('uploading');expect(view(f).busy).toBe(true);expect(f.client.draft).toBe('');
 const request=f.requests.find(r=>r.method==='provider.uploadFeedback');expect(request?.payload).toEqual({threadId:'thread',reason:'useful'});
 f.input.active=false;view(f);held.resolve();const result=await sending;expect(result.stale).toBe(true);expect(result.message).toBe('');
 f.input.active=true;f.input.visit='return';expect(view(f).dock.feedback[0]?.feedbackId).toBe('feedback-id');
});
test('feedback accepted before navigation finishes saving and uploads for its original thread',async()=>{
 const f=await fixture();connected(f);f.client.local.drafts[f.target.key]='/feedback original reason';await prepared(f);
 const held=gate(),entered=gate(),write=f.storage.fs.atomicWriteFile;
 f.storage.fs.atomicWriteFile=async(path,bytes)=>{entered.resolve();await held.promise;await write(path,bytes)};
 const sending=run(f,'send');await entered.promise;
 expect(f.client.local.drafts[f.target.key]).toBe('');expect(f.requests.some(r=>r.method==='provider.uploadFeedback')).toBe(false);
 const otherKey=`${f.target.environmentId}:other`;
 f.client.threadId='other';f.client.local.drafts[otherKey]='Keep this new draft';
 Object.assign(f.input,{threadId:'other',url:`/threads/${f.target.environmentId}/other`,visit:'other-visit'});
 expect(view(f).dock.feedback).toEqual([]);
 held.resolve();const result=await sending;
 expect(result).toMatchObject({accepted:true,stale:true,message:''});
 expect(f.requests.filter(r=>r.method==='provider.uploadFeedback')).toEqual([expect.objectContaining({payload:{threadId:'thread',reason:'original reason'}})]);
 expect(obj(f.disk().drafts)[f.target.key]).toBe('');expect(f.client.local.drafts[otherKey]).toBe('Keep this new draft');
 expect(view(f).dock.feedback).toEqual([]);
 f.client.threadId='thread';Object.assign(f.input,{threadId:'thread',url:`/threads/${f.target.environmentId}/thread`,visit:'return'});
 expect(view(f).dock.feedback[0]).toMatchObject({status:'sent',feedbackId:'feedback-id'});
});
test('feedback transport revocation during the accepted clear save prevents dispatch',async()=>{
 for(const change of ['permission','generation','catalog','connection'] as const){
  const f=await fixture();connected(f);f.client.local.drafts[f.target.key]='/feedback';await prepared(f);
  const held=gate(),entered=gate(),write=f.storage.fs.atomicWriteFile;
  f.storage.fs.atomicWriteFile=async(path,bytes)=>{entered.resolve();await held.promise;await write(path,bytes)};
  const sending=run(f,'send');await entered.promise;
  if(change==='permission')f.client.scopes=[];
  else if(change==='generation')f.client.generation++;
  else if(change==='catalog')fleet.saved.find(s=>s.environmentId===f.target.environmentId)!.origin='https://replacement.test';
  else f.client.connection='disconnected';
  held.resolve();await sending;
  expect(f.requests.some(r=>r.method==='provider.uploadFeedback')).toBe(false);
  expect(f.client.local.drafts[f.target.key]).toBe('');expect(obj(f.disk().drafts)[f.target.key]).toBe('');
 }
});
test('feedback permission loss after dispatch interrupts without redirecting or automatic retry',async()=>{
 const f=await fixture();connected(f);f.client.local.drafts[f.target.key]='/feedback';await prepared(f);
 f.intercept(r=>{if(r.method==='provider.uploadFeedback')f.client.scopes=[]});await expect(run(f,'send')).rejects.toHaveProperty('kind','superseded');
 expect(view(f).dock.feedback).toEqual([]);expect(f.requests.filter(r=>r.method==='provider.uploadFeedback')).toHaveLength(1);expect(f.client.draft).toBe('');
});
test('copy uses exact current sent notice and real native copied reply; stale IDs cannot copy',async()=>{
 const f=await fixture();connected(f);f.client.local.drafts[f.target.key]='/feedback';await prepared(f);await run(f,'send');const row=view(f).dock.feedback[0]!;
 expect((await run(f,'feedback-copy',row.id,row.feedbackId)).accepted).toBe(true);
 expect(f.requests.filter(r=>r.op==='copyText')).toEqual([expect.objectContaining({text:'feedback-id'})]);expect(f.requests.filter(r=>r.op==='mobileHomeHaptic')).toHaveLength(1);
 await run(f,'feedback-dismiss',row.id);const count=f.requests.length;await expect(run(f,'feedback-copy',row.id,row.feedbackId)).rejects.toHaveProperty('kind','superseded');expect(f.requests).toHaveLength(count);
});
test('questions keep source feedback visible while hiding usage and ordinary Send lane',async()=>{
 const f=await fixture();connected(f);f.client.local.drafts[f.target.key]='/feedback';await prepared(f);await run(f,'send');usage(f);f.client.local.drafts[f.target.key]='';await mobileDraftChanged(f.client,'/usage-limits',f.native,f.storage,f.target.owner);await run(f,'send');expect(view(f).dock.hasUsage).toBe(true);
 Object.assign(f.client,{thread:{projection:{runtimeRequests:[{id:'question',kind:'user_input',status:'pending'}],turnItems:[{type:'user_input_request',requestId:'question',questions:[]}]}}});
 const v=view(f);expect(v.ordinary).toBe(false);expect(v.dock.hasUsage).toBe(false);expect(v.dock.feedback).toHaveLength(1);
});
test('rendered external account key is verified; relabelled or unsafe links cannot open',async()=>{
 const f=await fixture();usage(f);await prepared(f);await run(f,'send');const row=view(f).dock.accounts[0]!;
 expect((await run(f,'open-link',row.key,row.externalURL)).accepted).toBe(true);expect(f.requests.filter(r=>r.op==='mobileOpenURL')).toHaveLength(1);
 obj((f.client.config.providers as Obj[])[0]).displayName='Changed account';await expect(run(f,'open-link',row.key,row.externalURL)).rejects.toHaveProperty('kind','superseded');
 expect(f.requests.filter(r=>r.op==='mobileOpenURL')).toHaveLength(1);
});
test('reset credit uses confirmation then exact RPC with current report input and provider permission',async()=>{
 const f=await fixture();usage(f);connected(f);await prepared(f);await run(f,'send');const row=view(f).dock.accounts[0]!;
 const result=await run(f,'reset-credit',row.resetKey,row.resetInput);expect(result.accepted).toBe(true);expect(result.message).toContain('Reset applied');
 expect(f.requests.filter(r=>r.op==='mobileScheduledConfirm')).toEqual([expect.objectContaining({kind:'reset-credit'})]);
 expect(f.requests.find(r=>r.method==='provider.consumeResetCredit')?.payload).toEqual(JSON.parse(row.resetInput));expect(view(f).dock.accounts[0]?.resetStatus).toContain('Reset applied');
});
test('permission/account change during reset confirmation blocks server write and never auto consumes',async()=>{
 for(const change of ['permission','account'] as const){const f=await fixture();usage(f);connected(f);await prepared(f);await run(f,'send');const row=view(f).dock.accounts[0]!;
 f.intercept(r=>{if(r.op==='mobileScheduledConfirm'){if(change==='permission')f.client.scopes=[];else f.client.config.providers=[]}});
 await expect(run(f,'reset-credit',row.resetKey,row.resetInput)).rejects.toHaveProperty('kind','superseded');expect(f.requests.some(r=>r.method==='provider.consumeResetCredit')).toBe(false);
 }
});
test('late ordinary queue completion can retire original draft but returns no stale UI message',async()=>{
 const f=await fixture();await prepared(f);f.intercept(r=>{if(r.action==='enqueueTransfer'){f.input.active=false;view(f)}});
 const result=await run(f,'send');expect(result.accepted).toBe(true);expect(result.stale).toBe(true);expect(result.message).toBe('');expect(f.claim()?.state).toBe('completed');
});
test('lost enqueue reply marks recovery dirty without retrying or clearing captured draft',async()=>{
 const f=await fixture();await prepared(f);f.intercept(r=>{if(r.action==='enqueueTransfer')throw {name:'FetchError',kind:'Aborted'}});
 await expect(run(f,'send')).rejects.toHaveProperty('kind','superseded');expect(view(f).needsRead).toBe(true);expect(view(f).canSend).toBe(false);
 expect(f.client.draft).toBe('  captured  ');expect(f.requests.filter(r=>r.action==='enqueueTransfer')).toHaveLength(1);
});

test('send label and symbol share captured preferences and actual steering support',async()=>{
 const f=await fixture();Object.assign(f.client,{thread:{projection:{thread:{id:'thread',activeProviderThreadId:'pt'},runs:[{id:'r',status:'running',providerThreadId:'pt',activeAttemptId:'attempt'}],
 providerThreads:[{id:'pt',providerSessionId:'session'}],providerSessions:[{id:'session',capabilities:{turns:{supportsActiveSteering:true}}}],providerTurns:[{runAttemptId:'attempt',status:'running'}]}}});
 await prepared(f);expect(view(f)).toMatchObject({sendLabel:'Queue',sendSymbol:'list.number'});
 f.input.preferencesJSON=JSON.stringify({followUpBehavior:'steer',planModeEnabled:false});expect(view(f)).toMatchObject({sendLabel:'Steer',sendSymbol:'arrow.turn.left.up'});
 obj(f.client.projection).providerTurns=[];expect(view(f)).toMatchObject({sendLabel:'Queue',sendSymbol:'list.number'});f.input.active=false;expect(view(f)).toMatchObject({sendLabel:'Send',sendSymbol:'arrow.up'});
});

test('canceled refresh never reuses an old successful recovery view as new evidence',async()=>{
 const f=await fixture(),key=view(f).readKey;await prepared(f);expect(view(f).canSend).toBe(true);
 f.intercept(r=>{if(r.action==='read')throw {name:'FetchError',kind:'Aborted'}});
 await expect(run(f,'recovery-read',key)).rejects.toHaveProperty('kind','superseded');
 expect(view(f).needsRead).toBe(false);expect(view(f).canSend).toBe(false);expect(view(f).recovery.actions.some(a=>a.label==='Refresh saved status')).toBe(true);
});

test('successful reset may consume the final credit before its reply and still retains source outcome',async()=>{
 const f=await fixture();usage(f);connected(f);await prepared(f);await run(f,'send');const row=view(f).dock.accounts[0]!;
 f.intercept(r=>{if(r.method==='provider.consumeResetCredit')obj(obj((f.client.config.providers as Obj[])[0]!.usageLimits).resetCredits).availableCount=0});
 const result=await run(f,'reset-credit',row.resetKey,row.resetInput);expect(result.accepted).toBe(true);expect(result.message).toContain('Reset applied');
 const after=view(f).dock.accounts[0]!;expect(after.creditsVisible).toBe(true);expect(after.resetVisible).toBe(false);expect(after.resetStatus).toContain('Reset applied');
});

test('known prepared capture keeps explicit recovery actions under incomplete inventory without auto recovery',async()=>{
 const f=await fixture(),captured=mobileThreadSendCaptureDraft(f.client,f.target);if(captured.status!=='captured')throw Error(captured.reason);
 await f.client.persist(f.storage);const record:ThreadSendRecord={schemaVersion:1,origin:f.client.origin,environmentId:f.target.environmentId,threadId:'thread',
 text:f.client.draft.trim(),attachments:[],modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'full-access',interactionMode:'plan'};
 f.setClaim({kind:'ordinary',origin:f.client.origin,environmentId:f.target.environmentId,threadId:'thread',draftKey:f.target.key,transferId:'message',messageId:'message',commandId:'command',
 mutationId:'epoch:1',fingerprint:fingerprint(captured.capture),state:'prepared',capture:captured.capture,record:{...record,messageId:'message',commandId:'command',createdAt:new Date(now).toISOString()}});
 f.setComplete(false);await prepared(f);const v=view(f);expect(v.canSend).toBe(false);const keep=v.recovery.actions.find(a=>a.label==='Keep queued message')!;
 expect(keep).toBeDefined();expect(f.requests.some(r=>r.action==='recover')).toBe(false);
 expect((await run(f,'recovery-action',keep.key)).accepted).toBe(true);expect(f.claim()?.state).toBe('completed');expect(f.requests.filter(r=>r.action==='recover')).toHaveLength(1);
 expect(f.requests.some(r=>r.action==='enqueueTransfer')).toBe(false);
});

test('reset status is retired on usage close/reopen and canonical home replacement',async()=>{
 const f=await fixture();usage(f);connected(f);await prepared(f);await run(f,'send');let row=view(f).dock.accounts[0]!;
 await run(f,'reset-credit',row.resetKey,row.resetInput);expect(view(f).dock.accounts[0]?.resetStatus).toContain('Reset applied');
 await run(f,'usage-close',view(f).dock.usageKey);await mobileDraftChanged(f.client,'/usage-limits',f.native,f.storage,f.target.owner);await run(f,'send');
 row=view(f).dock.accounts[0]!;expect(row.resetStatus).toBe('');
 await run(f,'reset-credit',row.resetKey,row.resetInput);f.client.origin='https://replacement.test';const saved=fleet.saved.find(s=>s.environmentId===f.target.environmentId)!;saved.origin=f.client.origin;
 expect(view(f).dock.hasUsage).toBe(false);f.client.origin=f.target.origin;saved.origin=f.target.origin;expect(view(f).dock.hasUsage).toBe(false);
});
test('visible feedback controls remain interactive during queued editing while Send stays separate',async()=>{
 const f=await fixture();connected(f);f.client.local.drafts[f.target.key]='/feedback';await prepared(f);await run(f,'send');const row=view(f).dock.feedback[0]!;
 const state=queuedEditState(f.client);state.active.set(`${f.target.environmentId}:thread`,'edit');state.sessions.set('edit',{owner:'edit',draftKey:'queued',origin:f.client.origin,
 environmentId:f.target.environmentId,threadId:'thread',projectId:'project',generation:4,session:'s',revision:0,runId:'r',messageId:'m',text:'queued',attachments:[],existingAttachments:[],saving:false});
 expect(view(f).ordinary).toBe(false);expect((await run(f,'feedback-copy',row.id,row.feedbackId)).accepted).toBe(true);
 expect((await run(f,'feedback-dismiss',row.id)).accepted).toBe(true);expect(view(f).dock.feedback).toEqual([]);expect((await run(f,'send')).accepted).toBe(false);
});

test('refused concrete command clear does not claim acceptance or dispatch feedback',async()=>{
 const f=await fixture();connected(f);expect(mobileThreadSendCaptureDraft(f.client,f.target).status).toBe('captured');
 f.client.local.drafts[f.target.key]='/feedback';await prepared(f);const result=await run(f,'send');
 expect(result.accepted).toBe(false);expect(result.message).toContain('changed');expect(f.client.draft).toBe('/feedback');expect(f.requests.some(r=>r.method==='provider.uploadFeedback')).toBe(false);
});
