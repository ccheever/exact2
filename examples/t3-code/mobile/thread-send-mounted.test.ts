// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {expect,test} from 'bun:test';
import {createHash} from 'node:crypto';
import {MobileDraftClient,mobileDraftRecoveryHandles} from './mobile-draft-recovery';
import {mobileThreadTransferSubmit as submit,mobileThreadTransferResume as resume} from './thread-send-transfer';
import {mobileThreadMountedSendAdmission as admission,mobileThreadMountedSendCurrent as current} from './thread-send-mounted';
import {mobileEditorSnapshot as snapshot,mobileEditorAction as action,type EditorPresentation,type EditorRouteInput} from './composer-editor-runtime';
import {mobileEditorOwner} from './composer-editor-owner';
import {mobileComposerTarget} from './composer-target';
import {mobileOutboxTransferCanonical as canonical} from './mobile-outbox-transfer-model';
import {mobileOutboxSnapshot} from './mobile-outbox';
import {mobileThreadSendRead} from './thread-send-controller';
import {mobileComposerContextCaptureTarget,mobileComposerContextCommitBatch} from './composer-command-context';
import type {ThreadSendTransferClaim} from './thread-send-transfer-model';
import type {ThreadSendRecord} from './thread-send-admission';
import {fleet} from './shared/settings-b-fleet';
import {obj,type Obj} from './shared/domain';
import type {Native,Files} from './shared/protocol';
const copy=<T>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
const now=Date.parse('2026-10-09T12:00:00Z'),hash=(value:unknown)=>createHash('sha256').update(canonical(value)).digest('hex');
const uuid=(n:number)=>`00000000-0000-0000-0000-${String(n).padStart(12,'0')}`;
let sequence=0;
async function fixture(options:{saved?:Obj;claim?:ThreadSendTransferClaim;environmentId?:string;mount?:boolean;text?:string;files?:boolean}={}) {
 const client=new MobileDraftClient(),environmentId=options.environmentId??`mounted-send-${++sequence}`;
 let saved:Obj=copy(options.saved??{}),claim:ThreadSendTransferClaim|null=copy(options.claim??null),failAt=0,writes=0;
 let hook:((r:Obj,value:any)=>Promise<void>|void)|undefined;
 const requests:Obj[]=[],events:string[]=[],snapshots:Obj[]=[],terminals=new Map<string,Obj>();
 const storage:Files={fs:{async mkdir(){},async readFile(){return new TextEncoder().encode(JSON.stringify(saved)).buffer},async atomicWriteFile(_p,bytes){
   events.push('persist');writes++;if(failAt===writes)throw Error('save failed');saved=obj(JSON.parse(new TextDecoder().decode(bytes)));snapshots.push(copy(saved));
 }}};
 const hydration=mobileDraftRecoveryHandles(client,{available:true,watch(){},async later(){return {ok:false,generation:0,error:{message:'offline'}}}},storage);
 await client.refresh(hydration.native,hydration.storage);
 Object.assign(client,{origin:'https://mounted.test',environmentId,threadId:'thread',projectId:'project',generation:3,connection:'disconnected',providerId:'provider',modelId:'model',runtimeMode:'full-access',interactionMode:'default'});
 if(!options.saved)client.local.drafts[client.draftKey]=options.text??'  captured  ';
 if(!fleet.saved.some(s=>s.environmentId===environmentId))fleet.saved.push({environmentId,origin:client.origin});
 client.shell.projects=[{id:'project',workspaceRoot:'/repo'}];client.shell.threads=[{id:'thread',projectId:'project',title:'Thread',modelSelection:{instanceId:'provider',model:'model'}}];
 client.config={providers:[{instanceId:'provider',driver:'codex',skills:[],slashCommands:[],workspaceSnapshots:[{cwd:'/repo',skills:[],slashCommands:[]}]}]};
 const target=mobileComposerTarget(client),scope={origin:client.origin,environmentId,threadId:'thread',draftKey:target.key};
 if(options.files){client.local.snapshotDrafts[target.key]=[{id:uuid(1),name:'image.png',mimeType:'image/png',sizeBytes:8}];
   (client.local as any).composerFiles=[{id:uuid(2),draftKey:target.key,environmentId,contextId:'file2',name:'file.txt',mimeType:'text/plain',sizeBytes:9,attachmentId:'',source:'attached',status:'staged'}];
   (client.local as any).mobileAttachmentOrder={[target.key]:[uuid(2),uuid(1)]};}
 const route:EditorRouteInput={active:true,routeVisit:'visit',editorId:'composer',environmentId,threadId:'thread',readOnly:false,voiceBusy:false,focusIntent:{serial:'',attempt:0,operation:'none'}};
 const presentation:EditorPresentation={themeJson:'{}',placeholder:'Message',fontSize:16,lineHeight:20,enterBehavior:'send',iconUris:{},hasCompactableConversation:true,offersUsageLimits:false,allowInteractionMode:true,repository:'repo',permissionRevision:'permission'};
 const row=()=>claim?.record?{record:copy(claim.record),revision:1,token:claim.mutationId,pending:claim.state==='prepared',held:false}:null;
 const outcome=()=>claim?{messageId:claim.messageId,mutationId:claim.mutationId,status:claim.state==='failed'?'failed':'committed',revision:1,message:'',record:copy(claim.record),removed:null,ownerEpoch:'epoch',sequenceFloor:1,current:row()??{record:null,revision:1,token:claim.mutationId,pending:false}}:null;
 const native:Native={available:true,watch(){},async later(raw){
   const r=obj(raw);requests.push(copy(r));events.push(String(r.action??r.op));let value:any;
   if(r.op==='ids')value=['message','command'];
   else if(r.op==='mobileVoice'){
     const o=mobileEditorOwner(client)!;value={start:o.state.selection.start,end:o.state.selection.end,
       capture:{identity:{...o.state.identity,mountId:o.state.mountId},eventCount:o.state.eventCount,sourceRevision:r.sourceRevision}};
   }else if(r.op==='composerEditorApply'){
     const c=obj(r.command),next=obj(c.next),old=terminals.get(String(c.commandId));
     value={event:old??{...obj(r.identity),kind:'commandApplied',commandId:c.commandId,commandRevision:c.commandRevision,
       eventCount:Number(obj(c.expected).eventCount)+1,value:next.value,selection:next.selection,composing:false,focused:true,reason:''}};
     if(!old)terminals.set(String(c.commandId),copy(value.event));
   }else if(r.op==='mobileOutbox'){
     if(r.action==='read')value={ownerEpoch:'epoch',sequenceFloor:claim?1:0,complete:true,errors:[],records:row()?[row()]:[],
       revisions:claim?{message:1}:{},tokens:claim?{message:claim.mutationId}:{},outcomes:[],mutations:[],transfers:claim?[copy(claim)]:[]};
     else if(r.action==='transferLookup')value={complete:true,fingerprint:r.capture?hash(r.capture):null,claims:claim?[copy(claim)]:[]};
     else if(r.action==='transferStatus')value={claim:copy(claim),outcome:claim?.state==='queued'?outcome():null};
     else if(r.action==='enqueueTransfer'){
       const record=r.record as ThreadSendTransferClaim['record'],capture=r.capture as ThreadSendTransferClaim['capture'];
       claim={kind:'ordinary',origin:scope.origin,environmentId,threadId:'thread',draftKey:target.key,transferId:'message',messageId:'message',commandId:'command',mutationId:String(r.mutationId),fingerprint:hash(capture),state:'queued',record:copy(record),capture:copy(capture)};
       value={disposition:'created',claim:copy(claim),outcome:outcome()};
     }else if(r.action==='completeTransfer'){
       const marker=obj(obj(saved.mobileOutboxTransferCompletions).message),after=obj(marker.after);
       expect(marker.fingerprint).toBe(claim!.fingerprint);expect(after.text).toBe(obj(saved.drafts)[target.key]);
       expect(after.document).toEqual(obj(obj(saved.mobileComposerEditor).documents)[JSON.stringify([scope.origin,environmentId,target.key])]);
       expect(after.context).toEqual(obj(obj(saved.mobileComposerContexts).entries)[JSON.stringify([scope.origin,environmentId,target.key])]??null);
       expect(after.images).toEqual(obj(saved.snapshotDrafts)[target.key]??[]);
       expect(after.files).toEqual((saved.composerFiles as any[]??[]).filter(f=>f.draftKey===target.key));
       expect(after.attachmentOrder).toEqual(obj(saved.mobileAttachmentOrder)[target.key]??null);
       claim={...claim!,state:'completed',record:null,capture:null};value={completed:true,claim:copy(claim)};
     }else throw Error(`Unexpected outbox ${r.action}`);
   }else throw Error(`Unexpected op ${r.op}`);
   await hook?.(r,value);return {ok:true,generation:r.generation??client.generation,value};
 }};
 const view=(raw='')=>snapshot(client,route,raw,presentation,now);
 const event=(kind:string,value=mobileEditorOwner(client)!.state.value,extra:Obj={})=>{
   const o=mobileEditorOwner(client)!;return {...o.state.identity,mountId:o.state.mountId||'mount',kind,eventCount:o.state.eventCount+1,value,
     selection:{start:value.length,end:value.length},composing:false,focused:true,...extra};
 };
 const send=(e:unknown)=>action(client,route,'event',JSON.stringify(e),native,storage,()=>now);
 if(options.mount!==false){view();await send(event('ready'));}
 const attachments:ThreadSendRecord['attachments']=options.files?[{kind:'file',id:uuid(2),name:'file.txt',mimeType:'text/plain',sizeBytes:9,uploadId:'',status:'staged',contextId:'file2',source:'attached'},
   {kind:'image',id:uuid(1),name:'image.png',mimeType:'image/png',sizeBytes:8,uploadId:'',status:'staged'}]:[];
 const record:ThreadSendRecord={schemaVersion:1,origin:scope.origin,environmentId,threadId:'thread',text:client.draft.trim(),attachments,modelSelection:{instanceId:'provider',model:'model'},runtimeMode:'full-access',interactionMode:'default'};
 const input=()=>({target,record,now,current:()=>true,editor:admission(client)!});
 events.length=0;requests.length=0;snapshots.length=0;writes=0;
 return {client,target,scope,route,native,storage,events,requests,snapshots,view,event,send,input,record,
   disk:()=>copy(saved),claim:()=>copy(claim),hook:(next:typeof hook)=>{hook=next},failWrite:(at:number)=>{failAt=at},
   proof:()=>obj(obj(saved.mobileOutboxTransferCompletions).message)};
}

test('mounted capture uses same queue and atomic terminal publication before same native completion',async()=>{
 const f=await fixture();expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('completed');
 expect(f.events).toEqual(['selection','read','transferLookup','persist','ids','enqueueTransfer','persist','composerEditorApply','persist','persist','completeTransfer']);
 expect(f.snapshots[1]!.drafts).toMatchObject({[f.target.key]:'  captured  '});expect(obj(obj(f.snapshots[1]!.mobileOutboxTransferCompletions).message).disposition).toBe('preserved');
 expect(f.client.draft).toBe('');expect(f.proof().disposition).toBe('cleared');expect(mobileEditorOwner(f.client)?.state.ackCommandId).not.toBe('');
 expect(f.requests.filter(r=>r.action==='enqueueTransfer')).toHaveLength(1);
});
test('plain entry still refuses rich and explicit controller admission reuses settings/file-only facts',async()=>{
 const f=await fixture({text:'',files:true}),input={expectedOwner:f.target.owner,alternate:false,preferences:{followUpBehavior:'queue' as const,planModeEnabled:true},activity:{contextImporting:false,pendingPastedText:false,uploadStates:{},uploadOwners:{}}};
 expect(mobileThreadSendRead(f.client,input).kind).toBe('blocked');expect(mobileThreadSendRead(f.client,{...input,editor:admission(f.client)!})).toMatchObject({kind:'ready',hasContent:true});
 const {editor,...plain}=f.input();expect((await submit(f.client,f.native,f.storage,plain)).status).toBe('blocked');expect(f.requests).toHaveLength(0);
});
test('empty text file-only Send advances native and durable revision and clears every named row without unlink',async()=>{
 const f=await fixture({text:'',files:true});const before=admission(f.client)!.revision;
 expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('completed');
 expect(obj(obj(f.proof().after).document).revision).toBe(before+1);expect(f.proof().disposition).toBe('cleared');
 expect(f.client.local.snapshotDrafts[f.target.key]).toEqual([]);expect((f.client.local as any).composerFiles).toEqual([]);
 expect(f.requests.some(r=>['snapshotDraftRemove','composerAttachRemove'].includes(String(r.op)))).toBe(false);
});
test('native observation mismatch refuses before enqueue without guessing count',async()=>{
 const f=await fixture();f.hook((r,v)=>{if(r.op==='mobileVoice')v.capture.eventCount++});
 await expect(submit(f.client,f.native,f.storage,f.input())).rejects.toHaveProperty('kind','superseded');
 expect(f.events).toEqual(['selection']);expect(f.client.draft).toBe('  captured  ');
});
test('new typing during enqueue preserves and avoids a native clear',async()=>{
 const f=await fixture();f.hook(async r=>{if(r.action==='enqueueTransfer')await f.send(f.event('text','newer'))});
 expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('completed');expect(f.client.draft).toBe('newer');expect(f.proof().disposition).toBe('preserved');
 expect(f.requests.some(r=>r.op==='composerEditorApply')).toBe(false);
});
test('native rejected replacement adopts its actual newer document and preserves inventory',async()=>{
 const f=await fixture({files:true});f.hook((r,v)=>{if(r.op==='composerEditorApply')Object.assign(v.event,{kind:'commandRejected',value:'new native',selection:{start:10,end:10},reason:'changed'})});
 expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('completed');expect(f.client.draft).toBe('new native');expect(f.proof().disposition).toBe('preserved');
 expect((f.client.local as any).composerFiles).toHaveLength(1);
});
test('event before reply and newer text keep newest text; an empty ABA also preserves captured inventory',async()=>{
 for(const value of ['newer','']){const f=await fixture({files:true});f.hook(async(r,v)=>{if(r.op==='composerEditorApply'){
   const terminal=copy(v.event),{owner,editorId,routeVisit,renderEpoch,mountId,...pendingCommand}=terminal;
   await f.send({...terminal,kind:'text',eventCount:Number(terminal.eventCount)+2,value,selection:{start:value.length,end:value.length},pendingCommand});
 }});
 expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('completed');expect(f.client.draft).toBe(value);expect(f.proof().disposition).toBe('preserved');
 expect((f.client.local as any).composerFiles).toHaveLength(1);expect(f.requests.filter(r=>r.op==='composerEditorApply')).toHaveLength(1)}
});
test('positive fence save required: swallowed ordinary save failure never dispatches CAS',async()=>{
 const f=await fixture();f.failWrite(2);const answer=await submit(f.client,f.native,f.storage,f.input());expect(answer.status).toBe('blocked');
 expect(f.claim()?.state).toBe('queued');expect(f.client.draft).toBe('  captured  ');expect(f.requests.some(r=>r.op==='composerEditorApply')).toBe(false);
});
test('lost reply replays same retained command and completes once',async()=>{
 const f=await fixture(),sentinel={name:'FetchError',kind:'Aborted'};let first=true;
 f.hook(r=>{if(r.op==='composerEditorApply'&&first){first=false;throw sentinel}});
 await expect(submit(f.client,f.native,f.storage,f.input())).rejects.toHaveProperty('kind','superseded');
 const q=f.claim()!;expect(q.state).toBe('queued');expect(f.proof().disposition).toBe('preserved');
 expect((await resume(f.client,f.native,f.storage,f.scope,q.transferId,undefined,admission(f.client)!)).status).toBe('completed');
 const calls=f.requests.filter(r=>r.op==='composerEditorApply');expect(calls).toHaveLength(2);expect(calls[0]!.command).toEqual(calls[1]!.command);expect(f.client.draft).toBe('');
});
test('cold recovery after ambiguous native reply preserves disk text and never invents a new clear',async()=>{
 const f=await fixture(),sentinel={name:'FetchError',kind:'Aborted'};f.hook(r=>{if(r.op==='composerEditorApply')throw sentinel});
 await expect(submit(f.client,f.native,f.storage,f.input())).rejects.toHaveProperty('kind','superseded');
 const cold=await fixture({saved:f.disk(),claim:f.claim()!,environmentId:f.scope.environmentId,mount:false});
 expect((await resume(cold.client,cold.native,cold.storage,cold.scope,'message')).status).toBe('completed');
 expect(cold.client.draft).toBe('  captured  ');expect(cold.requests.some(r=>r.op==='composerEditorApply')).toBe(false);expect(cold.proof().disposition).toBe('preserved');
});
test('return selection/modal or route changes invalidate admission without disabling offline editing',async()=>{
 const f=await fixture(),a=admission(f.client)!;expect(current(f.client,f.target,a)).toBe(true);
 f.route.readOnly=true;f.view();expect(current(f.client,f.target,a)).toBe(false);expect(mobileEditorOwner(f.client)?.admission).toBe(a.admission);
 f.route.readOnly=false;f.view();expect(current(f.client,f.target,a)).toBe(true);f.route.active=false;f.view();expect(current(f.client,f.target,a)).toBe(false);
});
test('context/inventory malformed before capture never defaults raw state or issues IO',async()=>{
 const f=await fixture();(f.client.local as any).mobileAttachmentOrder=null;const before=copy(f.client.local);
 expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('blocked');expect(f.requests).toHaveLength(0);expect(f.client.local).toEqual(before);
});
test('failed final publication save keeps queued proof and explicit retry never reapplies native clear',async()=>{
 const f=await fixture();f.failWrite(3);
 expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('blocked');expect(f.client.draft).toBe('');expect(f.claim()?.state).toBe('queued');
 expect(f.proof().disposition).toBe('preserved');expect(obj(f.disk().drafts)[f.target.key]).toBe('  captured  ');
 expect((await resume(f.client,f.native,f.storage,f.scope,'message',undefined,admission(f.client)!)).status).toBe('completed');
 expect(f.requests.filter(r=>r.op==='composerEditorApply')).toHaveLength(1);expect(f.client.draft).toBe('');expect(obj(f.proof().after).text).toBe('');
});
test('fallback preserved completion revokes old terminal inventory retirement before a delayed event',async()=>{
 const f=await fixture({files:true}),sentinel={name:'FetchError',kind:'Aborted'};let terminal:Obj={};
 f.hook((r,v)=>{if(r.op==='composerEditorApply'){terminal=copy(v.event);throw sentinel}});
 await expect(submit(f.client,f.native,f.storage,f.input())).rejects.toHaveProperty('kind','superseded');f.hook(undefined);
 expect((await resume(f.client,f.native,f.storage,f.scope,'message')).status).toBe('cleanup-pending');
 expect((await resume(f.client,f.native,f.storage,f.scope,'message',undefined,admission(f.client)!)).status).toBe('completed');
 expect(f.proof().disposition).toBe('preserved');expect(f.claim()?.state).toBe('completed');
 await f.send(terminal);expect((f.client.local as any).composerFiles).toHaveLength(1);expect(f.client.local.snapshotDrafts[f.target.key]).toHaveLength(1);
 expect(f.requests.filter(r=>r.op==='composerEditorApply')).toHaveLength(1);expect(f.requests.filter(r=>r.action==='completeTransfer')).toHaveLength(1);
});
test('metadata-only changes while native CAS is held preserve the latest inventory without partial subtraction',async()=>{
 const f=await fixture({files:true});f.hook((r)=>{if(r.op==='composerEditorApply'){
   (f.client.local as any).composerFiles[0].name='renamed.txt';f.client.local.snapshotDrafts[f.target.key]![0]!.name='renamed.png';
 }});
 expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('completed');expect(f.proof().disposition).toBe('preserved');
 expect((f.client.local as any).composerFiles[0].name).toBe('renamed.txt');expect(f.client.local.snapshotDrafts[f.target.key]![0]!.name).toBe('renamed.png');
});
test('retained terminal carried by focus or submit still publishes its captured root effect exactly once',async()=>{
 for(const kind of ['focus','submit']){const f=await fixture();let count=0;f.hook(async(r,v)=>{if(r.op==='composerEditorApply'){
   const terminal=copy(v.event),{owner,editorId,routeVisit,renderEpoch,mountId,...pendingCommand}=terminal;
   count=Number(terminal.eventCount)+1;
   await f.send({...terminal,kind,eventCount:count,value:'new draft',selection:{start:9,end:9},alternate:false,pendingCommand});
 }});
 expect((await submit(f.client,f.native,f.storage,f.input())).status).toBe('completed');const effects=mobileEditorOwner(f.client)!.effects;
 expect(effects).toHaveLength(1);expect(effects[0]!.kind).toBe(kind);expect(JSON.parse(effects[0]!.payload)).toMatchObject({eventCount:count,value:'new draft',documentRevision:admission(f.client)!.revision});
 }
});
test('preserved terminal proof keeps absent context absent and an existing explicit empty row serialized',async()=>{
 for(const existing of [false,true]){const f=await fixture();if(existing){const guard=mobileComposerContextCaptureTarget(f.client,f.target)!;expect(mobileComposerContextCommitBatch(f.client,guard,[])).toBe(true);f.record.context={version:1,records:[]}}
 f.hook((r,v)=>{if(r.op==='composerEditorApply')Object.assign(v.event,{kind:'commandRejected',value:'later',selection:{start:5,end:5},reason:'changed'})});
 const answer=await submit(f.client,f.native,f.storage,f.input());expect(answer).toMatchObject({status:'completed',message:''});
 const after=obj(f.proof().after);expect(after.context).toEqual(existing?expect.objectContaining({text:'later',context:{version:1,records:[]}}):null);
 }
});
