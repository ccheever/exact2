// Mounted ordinary Send; the existing outbox remains the only queue/delivery owner.
// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import type {T3Client} from './shared/client';
import type {MobileDraftClient} from './mobile-draft-recovery';
import type {MobileComposerTarget} from './composer-target';
import {ClientError,type Native,type Files,reply} from './shared/protocol';
import {letGo} from './shared/let-go';
import {obj,type Obj} from './shared/domain';
import {fleet} from './shared/settings-b-fleet';
import {mobileCacheCatalogIdentity} from './mobile-client-cache-catalog';
import {mobileQueuedEditOrigin} from './queued-edit-origin';
import {mobileOutboxSnapshot} from './mobile-outbox';
import {mobileOutboxTransferCanonical as canonical} from './mobile-outbox-transfer-model';
import {mobileComposerContextSendSnapshot} from './composer-command-context';
import {mobileEditorOwner,mobileEditorOwnerChanged,mobileEditorContextTargetCurrent,mobileEditorCompleteMountedSend,mobileEditorCaptureIntent} from './composer-editor-owner';
import {mobileEditorDocumentCapture} from './composer-editor-persistence';
import {mobileComposerEditorCommand,mobileComposerEditorAccept,mobileComposerEditorStageEffect,mobileComposerEditorCommitted,
  type ComposerMountedIdentity,type ComposerEditorCommand} from './composer-editor-state';
import {threadSendTransferDecodeClaim,threadSendTransferDecodeCapture,type ThreadSendTransferClaim,type ThreadSendTransferCompletion} from './thread-send-transfer-model';
import type {ThreadSendDraftCaptureResult,ThreadSendDraftApplyResult} from './thread-send-handoff-draft';

export interface MountedSendAdmission {admission:string;identity:ComposerMountedIdentity;eventCount:number;incarnation:string;revision:number}
interface Pending {admission:MountedSendAdmission;claim:ThreadSendTransferClaim;marker:string;command:ComposerEditorCommand;
  revoked:boolean;result:ThreadSendDraftApplyResult|null}
const pending=new WeakMap<T3Client,Map<string,Pending>>();
const copy=<T>(v:T):T=>JSON.parse(JSON.stringify(v)) as T;
const blocked=(reason:string)=>({status:'blocked' as const,reason});
const fail=()=>new ClientError('The mounted composer changed. Keep the queued message and recover its draft.','superseded');
const identity=(owner:NonNullable<ReturnType<typeof mobileEditorOwner>>):ComposerMountedIdentity=>({...owner.state.identity,mountId:owner.state.mountId});
function ownerFor(client:T3Client,target:MobileComposerTarget,a:MountedSendAdmission) {
  const owner=mobileEditorOwner(client),catalog=mobileCacheCatalogIdentity(fleet.saved,target.environmentId);
  return owner&&owner.admission===a.admission&&canonical(identity(owner))===canonical(a.identity)
    &&owner.target.owner===target.owner&&owner.route.active&&owner.state.mountId
    &&mobileEditorContextTargetCurrent(client,target)
    &&catalog===JSON.stringify([target.environmentId,mobileQueuedEditOrigin(client).trim().replace(/\/+$/,'')])
    &&owner.signature===JSON.stringify([target.owner,owner.route.routeVisit,owner.route.editorId,catalog])?owner:null;
}
export function mobileThreadMountedSendAdmission(client:T3Client):MountedSendAdmission|null {
  const owner=mobileEditorOwner(client);if(!owner||!owner.state.mountId)return null;
  const result={admission:owner.admission,identity:identity(owner),eventCount:owner.state.eventCount,
    incarnation:owner.document.incarnation,revision:owner.document.revision};
  return ownerFor(client,owner.target,result)?copy(result):null;
}
/** A plain receipt cannot bypass mounted admission. Exactness is for new Send, not recovery. */
export function mobileThreadMountedSendCurrent(client:T3Client,target:MobileComposerTarget,a:MountedSendAdmission):boolean {
  try {const owner=ownerFor(client,target,a);return !!owner&&!owner.route.readOnly&&!owner.route.voiceBusy&&!owner.state.composing&&!owner.pending
    &&owner.state.eventCount===a.eventCount&&owner.state.stagedEventCount===a.eventCount
    &&owner.document.incarnation===a.incarnation&&owner.document.revision===a.revision
    &&owner.document.value===owner.state.value&&owner.state.value===(client.local.drafts[target.key]??'')}
  catch{return false}
}
export function mobileThreadMountedSendCapture(client:T3Client,target:MobileComposerTarget,a:MountedSendAdmission):ThreadSendDraftCaptureResult {
  try {
    if(!client.preferencesLoaded||!mobileThreadMountedSendCurrent(client,target,a))return blocked('The native editor has an unobserved or pending edit.');
    const snapshot=mobileComposerContextSendSnapshot(client,target),document=mobileEditorDocumentCapture(client,target,'mounted-send');
    if(!snapshot||!document||document.incarnation!==a.incarnation||document.revision!==a.revision)return blocked('The enrolled editor snapshot is unavailable.');
    const capture=threadSendTransferDecodeCapture({version:2,kind:'ordinary',draft:{key:target.key,origin:mobileQueuedEditOrigin(client),environmentId:target.environmentId,
      threadId:target.threadId,text:snapshot.text,context:snapshot.context,contextRevision:snapshot.contextRevision,images:snapshot.images,files:snapshot.files,
      attachmentIds:snapshot.attachmentIds,attachmentOrder:snapshot.attachmentOrder,
      document:{incarnation:document.incarnation,revision:document.revision,selection:document.selection}}});
    return {status:'captured',capture};
  }catch{return blocked('The mounted draft snapshot is invalid. Keep the draft.')}
}
/** Existing native source selection verifies the observation; it never creates an event. */
export async function mobileThreadMountedSendCheck(client:T3Client,target:MobileComposerTarget,a:MountedSendAdmission,native:Native):Promise<void> {
  if(!mobileThreadMountedSendCurrent(client,target,a))throw fail();
  const owner=mobileEditorOwner(client)!,expected=owner.state.value;
  const response=reply(await native.later({op:'mobileVoice',action:'selection',generation:target.generation,owner:target.editorOwner,
    text:expected,sourceRevision:a.revision}));
  if(!response.ok)throw new ClientError(response.error?.message??'The editor observation is unavailable.',response.error?.kind);
  const value=obj(response.value),capture=obj(value.capture);
  if(response.generation!==target.generation||!mobileThreadMountedSendCurrent(client,target,a)
    ||canonical(capture.identity)!==canonical(a.identity)||capture.eventCount!==a.eventCount||capture.sourceRevision!==a.revision
    ||value.start!==owner.state.selection.start||value.end!==owner.state.selection.end)throw fail();
}
function currentClaim(client:T3Client,p:Pending):boolean {
  const snapshot=mobileOutboxSnapshot(client),claim=snapshot.threadTransfers.find(c=>c.transferId===p.claim.transferId);
  return snapshot.complete&&!!claim&&claim.state==='queued'&&canonical(claim)===canonical(p.claim);
}
function marker(client:T3Client,id:string):unknown{return obj((client.local as unknown as Obj).mobileOutboxTransferCompletions)[id]}
function map(client:T3Client):Map<string,Pending>{
  let value=pending.get(client);if(!value){value=new Map();pending.set(client,value)}
  const active=mobileEditorOwner(client)?.admission;
  for(const [id,p]of value)if(p.admission.admission!==active){p.revoked=true;value.delete(id)}
  return value;
}
/** Any fallback completion revokes a former live CAS upgrade before native completion. */
export function mobileThreadMountedSendRevoke(client:T3Client,id:string):void {
  const rows=pending.get(client),p=rows?.get(id);if(p)p.revoked=true;
  if(mobileEditorOwner(client)?.pending?.queuedSend!==id)rows?.delete(id);
}

/** Called BEFORE generic runtime text reduction. Native event/reply share this concrete
 * publication and ACK path; a late terminal cannot retire an already completed transfer. */
export function mobileThreadMountedSendConsume(client:T3Client):boolean {
  const owner=mobileEditorOwner(client),id=owner?.pending?.queuedSend;if(!owner||!id)return false;
  const p=pending.get(client)?.get(id),terminal=owner.state.commandEffect,latest=owner.state.lastEvent;
  if(!p||!terminal||!latest)return false;
  if(terminal.event.commandId!==p.command.commandId||terminal.event.commandRevision!==p.command.commandRevision)throw fail();
  if(p.revoked||!currentClaim(client,p)){
    p.revoked=true;pending.get(client)?.delete(id);return false; // Generic reduction may retain text; no attachment retirement authority.
  }
  if(!ownerFor(client,owner.target,p.admission)||canonical(marker(client,id))!==p.marker)throw fail();
  const current=mobileEditorDocumentCapture(client,owner.target,'mounted-send-terminal');if(!current)throw fail();
  const snapshot=mobileComposerContextSendSnapshot(client,owner.target),captured=p.claim.capture!.draft;
  const exact=!!snapshot&&current.incarnation===captured.document.incarnation&&current.revision===captured.document.revision
    &&canonical(current.selection)===canonical(captured.document.selection)&&snapshot.text===captured.text
    &&snapshot.contextRevision===captured.contextRevision&&canonical(snapshot.context)===canonical(captured.context)
    &&canonical(snapshot.images)===canonical(captured.images)&&canonical(snapshot.files)===canonical(captured.files)
    &&canonical(snapshot.attachmentIds)===canonical(captured.attachmentIds)&&canonical(snapshot.attachmentOrder)===canonical(captured.attachmentOrder);
  const clear=exact&&terminal.event.kind==='commandApplied'&&latest.eventCount===terminal.event.eventCount
    &&latest.value===''&&latest.selection.start===0&&latest.selection.end===0;
  const applied=mobileEditorCompleteMountedSend(client,current,p.claim,{expectedMarker:p.marker,next:{value:latest.value,selection:latest.selection},clear,
    proof:{command:p.command,terminal:terminal.event,latest}});
  if(!applied.ok)throw fail();
  const effect=owner.state.latestEffect;if(effect)owner.state=mobileComposerEditorStageEffect(owner.state,effect.id).state;
  owner.state=mobileComposerEditorStageEffect(owner.state,terminal.id).state;owner.pending=null;
  p.result={status:'applied',marker:copy(applied.marker)};p.revoked=true;mobileEditorOwnerChanged(client);return true;
}
/** Positive write observation is necessary: shared persist catches ordinary save failures. */
async function persistProof(client:MobileDraftClient,storage:Files,id:string,expected:ThreadSendTransferCompletion):Promise<void> {
  let written=false,error:unknown;
  const files:Files={fs:{...storage.fs,atomicWriteFile:async(path,bytes)=>{
    try {
      const saved=obj(JSON.parse(new TextDecoder().decode(bytes)));
      if(canonical(obj(saved.mobileOutboxTransferCompletions)[id])!==canonical(expected))throw fail();
      await storage.fs.atomicWriteFile(path,bytes);written=true;
    }catch(e){error=e;throw e}
  }}};
  await client.persist(files);
  if(error!==undefined)throw error;if(!written)throw new ClientError('The mounted draft proof was not saved. Recover before retrying.','retained');
}
async function invoke(client:MobileDraftClient,p:Pending,native:Native,storage:Files):Promise<ThreadSendDraftApplyResult> {
  const owner=mobileEditorOwner(client);if(!owner||!ownerFor(client,owner.target,p.admission)||p.revoked)return blocked('Recover the retained draft projection.');
  const response=reply(await native.later({op:'composerEditorApply',generation:owner.target.generation,identity:p.admission.identity,command:p.command}));
  if(!response.ok)throw new ClientError(response.error?.message??'The editor replacement is unavailable.',response.error?.kind);
  if(response.generation!==owner.target.generation||mobileEditorOwner(client)!==owner)throw fail();
  const accepted=mobileComposerEditorAccept(owner.state,obj(response.value).event);
  if(accepted.accepted)owner.state=accepted.state;
  else if(!p.result)throw fail();
  if(!p.result)mobileThreadMountedSendConsume(client);
  if(!p.result)return blocked('The native editor retirement has not been observed. Recover this queued message.');
  if(p.result.status!=='blocked')await persistProof(client,storage,p.claim.transferId,p.result.marker);
  const effect=owner.state.latestEffect;if(effect)owner.state=mobileComposerEditorCommitted(owner.state,effect);
  return copy(p.result);
}
/** Actual native queue proof is required from the facade. Shape decoding is not authenticity. */
export async function mobileThreadMountedSendApplyQueued(client:MobileDraftClient,input:unknown,a:MountedSendAdmission,
  native:Native,storage:Files):Promise<ThreadSendDraftApplyResult> {
  const claim=threadSendTransferDecodeClaim(input);if(claim.state!=='queued'||!claim.capture)return blocked('The message is not queued.');
  const owner=mobileEditorOwner(client);if(!owner||!ownerFor(client,owner.target,a)||owner.target.key!==claim.draftKey)return blocked('The mounted draft owner changed.');
  const previous=map(client).get(claim.transferId);
  if(previous&&!previous.revoked&&owner.pending?.queuedSend===claim.transferId){
    if(canonical(previous.claim)!==canonical(claim))throw fail();return invoke(client,previous,native,storage);
  }
  const current=mobileEditorDocumentCapture(client,owner.target,'mounted-send-fence');if(!current)return blocked('The saved editor ownership is unavailable.');
  const hadMarker=marker(client,claim.transferId)!==undefined;
  const fence=mobileEditorCompleteMountedSend(client,current,claim,null);if(!fence.ok)return blocked('The queued draft proof could not be prepared.');
  await persistProof(client,storage,claim.transferId,fence.marker);
  if(!ownerFor(client,owner.target,a))throw fail();
  // Existing markers (including cold recovery) are a permanent no-reclear fence.
  const nowCapture=mobileThreadMountedSendCapture(client,owner.target,mobileThreadMountedSendAdmission(client)!);
  if(hadMarker||!fence.exact||nowCapture.status!=='captured'||canonical(nowCapture.capture)!==canonical(claim.capture)){
    const latest=mobileEditorDocumentCapture(client,owner.target,'mounted-send-preserve');
    const preserved=latest?mobileEditorCompleteMountedSend(client,latest,claim,null):null;
    return preserved?.ok?{status:'applied',marker:preserved.marker}:blocked('The current draft cannot be safely preserved.');
  }
  const capture=mobileEditorCaptureIntent(client,owner.target,'queued-send');if(!capture)throw fail();
  const commandId=`${owner.state.identity.renderEpoch}-queued-send-${++owner.serial}`;
  const reserved=mobileComposerEditorCommand(owner.state,commandId,owner.state.lastCommandRevision+1,{value:'',selection:{start:0,end:0},tokensJson:'[]'});
  if(!reserved.command)throw fail();
  const p:Pending={admission:copy(a),claim:copy(claim),marker:canonical(fence.marker),command:copy(reserved.command),revoked:false,result:null};
  if(!currentClaim(client,p))throw fail();
  owner.state=reserved.state;owner.pending={queuedSend:claim.transferId,id:commandId,revision:reserved.command.commandRevision,mode:null,settings:'',intent:capture};map(client).set(claim.transferId,p);
  try{return await invoke(client,p,native,storage)}catch(error){if(letGo(error))throw error;throw error}
}
