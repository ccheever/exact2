// Source365aa87982 ordinary enqueue; storage and delivery remain in the existing outbox.
// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import type { MobileDraftClient } from './mobile-draft-recovery';
import type { MobileComposerTarget } from './composer-target';
import { mobileEditorContextTargetCurrent } from './composer-editor-owner';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { queuedEditNoticeOwner, queuedEditNoticeKey, queuedEditClearCapturedNotice } from './queued-edit-memory';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { fleet } from './shared/settings-b-fleet';
import { ClientError, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileThreadSendCaptureDraft, mobileThreadSendApplyQueuedDraft } from './thread-send-handoff-draft';
import {mobileThreadMountedSendCapture,mobileThreadMountedSendCheck,mobileThreadMountedSendApplyQueued,mobileThreadMountedSendRevoke,type MountedSendAdmission} from './thread-send-mounted';
import { threadSendTransferDecodeCapture, type ThreadSendTransferClaim } from './thread-send-transfer-model';
import type { ThreadSendRecord } from './thread-send-admission';
import { mobileOutboxEncode } from './mobile-outbox-model';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxEnqueueThreadTransfer, mobileOutboxThreadTransferLookup,
  mobileOutboxThreadTransferStatus, mobileOutboxCompleteThreadTransfer, mobileOutboxReleaseFailedThreadTransfer,
  mobileOutboxRecover, type MobileOutboxThreadTarget } from './mobile-outbox';

export interface ThreadTransferResult {
  status:'completed'|'cleanup-pending'|'recovery-required'|'failed'|'blocked'|'busy';
  claim:ThreadSendTransferClaim|null;message:string;
}
export interface ThreadTransferInput {
  editor?:MountedSendAdmission;
  target:MobileComposerTarget;record:ThreadSendRecord;now:number;
  /** Route identity only. Text/settings captured at the tap are immutable payloads. */
  current():boolean;
  retryFailed?:boolean;
}
const result=(status:ThreadTransferResult['status'],message='',claim:ThreadSendTransferClaim|null=null):ThreadTransferResult=>({status,message,claim});
const leases=new WeakMap<MobileDraftClient,Set<string>>();
const key=(target:MobileOutboxThreadTarget)=>JSON.stringify([target.origin,target.environmentId,target.threadId,target.draftKey]);
export const mobileThreadTransferBusy=(client:MobileDraftClient,target:MobileOutboxThreadTarget)=>leases.get(client)?.has(key(target))??false;
function acquire(client:MobileDraftClient,target:MobileOutboxThreadTarget):(()=>void)|null {
  let held=leases.get(client);if(!held){held=new Set();leases.set(client,held)}
  const id=key(target);if(held.has(id))return null;held.add(id);client.revision++;
  return ()=>{held!.delete(id);client.revision++};
}
/** Recovery release shares the submission lease; callers must release in finally. */
export const mobileThreadTransferLease=acquire;
function invocation(client:MobileDraftClient,input:Native|null|undefined,target:MobileOutboxThreadTarget) {
  if(!input?.available)throw new ClientError('Open T3 Code on your iPhone or iPad to queue a message.');
  const generation=client.generation,origin=client.origin,base=letGoAware(input);
  const check=()=>{
    if(client.generation!==generation||client.origin!==origin||client.environmentId!==target.environmentId
      ||mobileQueuedEditOrigin(client)!==target.origin
      ||mobileCacheCatalogIdentity(fleet.saved,target.environmentId)!==JSON.stringify([target.environmentId,target.origin]))
      throw new ClientError('The message environment changed. Recover its saved transfer before retrying.','superseded');
  };
  check();
  const native:Native={available:true,watch:topic=>{check();base.watch(topic)},later:async request=>{
    check();const reply=await base.later(request);check();return reply;
  }};
  return {native,check};
}
async function status(client:MobileDraftClient,native:Native,target:MobileOutboxThreadTarget,id:string) {
  const read=await mobileOutboxThreadTransferStatus(client,native,target,id);
  if(!read.claim)throw new ClientError('The saved message transfer is unavailable. Keep the draft until it is recovered.');
  return read.claim;
}
async function finish(client:MobileDraftClient,native:Native,storage:Files,check:()=>void,claim:ThreadSendTransferClaim,editor?:MountedSendAdmission):Promise<ThreadTransferResult> {
  check();
  if(!editor||claim.state!=='queued')mobileThreadMountedSendRevoke(client,claim.transferId);
  if(['completed','queued'].includes(claim.state)&&!mobileOutboxSnapshot(client).complete)
    return result('recovery-required','Resolve the incomplete pending-message inventory before finishing this transfer.',claim);
  if(claim.state==='completed')return result('completed','',claim);
  if(claim.state==='prepared')return result('recovery-required','Resolve the interrupted local save before sending this draft again.',claim);
  if(claim.state!=='queued')return result('failed','The message was not queued. Your draft has been kept.',claim);
  const applied=editor?await mobileThreadMountedSendApplyQueued(client,claim,editor,native,storage):mobileThreadSendApplyQueuedDraft(client,claim);
  if(applied.status==='blocked')return result('cleanup-pending',applied.reason,claim);
  try {
    // The concrete reduction prepared the proof with the draft. Persist both before native completion.
    await client.persist(storage);check();
    if(!await mobileOutboxCompleteThreadTransfer(client,native,claim))
      return result('cleanup-pending','The message is queued. Its saved draft retirement still needs to finish.',claim);
    const completed=mobileOutboxSnapshot(client).threadTransfers.find(item=>item.transferId===claim.transferId);
    if(completed?.state==='completed')mobileThreadMountedSendRevoke(client,claim.transferId);
    return completed?.state==='completed'?result('completed','',completed)
      :result('recovery-required','Read the completed transfer before submitting again.',claim);
  }catch(error){
    if(letGo(error))throw error;
    return result('cleanup-pending',error instanceof Error?error.message:'Could not finish saving the queued message.',claim);
  }
}
/** One invocation; plain latches and DTOs survive, Native handles and promises do not.
 * This function never dispatches to the provider or navigates. Root owns those next steps. */
export async function mobileThreadTransferSubmit(client:MobileDraftClient,nativeInput:Native|null|undefined,
  storage:Files,input:ThreadTransferInput):Promise<ThreadTransferResult> {
  if(!client.preferencesLoaded)return result('blocked','Wait for saved drafts to load.');
  if(!Number.isFinite(input.now)||input.now<=0)return result('blocked','Wait for the app clock before queuing this message.');
  const target:MobileOutboxThreadTarget={origin:mobileQueuedEditOrigin(client),environmentId:input.target.environmentId,
    threadId:input.target.threadId,draftKey:input.target.key};
  const noticeOwner=queuedEditNoticeOwner(client),noticeKey=queuedEditNoticeKey(client,noticeOwner);
  const release=acquire(client,target);if(!release)return result('busy','This draft is already being submitted.');
  try {
    const {native,check}=invocation(client,nativeInput,target);
    const current=()=>{check();if(!input.current()||!mobileEditorContextTargetCurrent(client,input.target))
      throw new ClientError('The selected draft changed before the message was queued.','superseded')};
    current();
    const captured=input.editor?mobileThreadMountedSendCapture(client,input.target,input.editor):mobileThreadSendCaptureDraft(client,input.target);
    if(captured.status==='blocked')return result('blocked',captured.reason);
    // Detach the source-resolved settings and payload before any asynchronous work.
    const proposed=JSON.parse(JSON.stringify(input.record)) as ThreadSendRecord;
    // Placeholder identities exist only inside this pure binding check. Native allocates
    // the real IDs below; this validation record is never stored or admitted.
    threadSendTransferDecodeCapture(captured.capture,mobileOutboxEncode({...proposed,
      messageId:'capture-check-message',commandId:'capture-check-command',createdAt:new Date(input.now).toISOString()}));
    if(input.editor)await mobileThreadMountedSendCheck(client,input.target,input.editor,native);
    await mobileOutboxRead(client,native);check();
    let lookup=await mobileOutboxThreadTransferLookup(client,native,target,captured.capture);check();
    if(!lookup.complete||!mobileOutboxSnapshot(client).complete)return result('blocked','Resolve the incomplete pending-message inventory before sending.');
    const active=lookup.claims.filter(claim=>!['completed','released'].includes(claim.state));
    if(active.length>1)return result('recovery-required','More than one transfer claims this draft. Recover its storage first.');
    if(active[0]){
      const claim=await status(client,native,target,active[0].transferId);check();
      if(claim.state!=='failed'||!input.retryFailed)return await finish(client,native,storage,check,claim,input.editor);
      current();
      if(!await mobileOutboxReleaseFailedThreadTransfer(client,native,claim))return result('failed','The failed local save is still retained.',claim);
      await mobileOutboxRead(client,native);check();
      lookup=await mobileOutboxThreadTransferLookup(client,native,target,captured.capture);check();
      if(!lookup.complete||lookup.claims.some(item=>!['completed','released'].includes(item.state)))
        return result('recovery-required','Read the remaining saved transfer before trying again.');
    }
    if(!lookup.complete||!mobileOutboxSnapshot(client).complete)return result('blocked','Resolve the incomplete pending-message inventory before sending.');
    const duplicate=lookup.claims.find(claim=>claim.state==='completed'&&claim.fingerprint===lookup.fingerprint);
    if(duplicate)return finish(client,native,storage,check,await status(client,native,target,duplicate.transferId),input.editor);
    current();
    await client.persist(storage);current();
    const [messageId,commandId]=await client.ids(native,2);current();
    if(!messageId||!commandId||messageId===commandId)return result('blocked','Message identifiers were not unique.');
    const record=mobileOutboxEncode({...proposed,messageId,commandId,createdAt:new Date(input.now).toISOString()});
    const capture=threadSendTransferDecodeCapture(captured.capture,record);
    // Pinned mobile clears immediately before a fresh synchronous enqueue. Our
    // admission awaits must not erase a newer failure reported in the meantime.
    const admitted=await mobileOutboxEnqueueThreadTransfer(client,native,record,capture,
      ()=>queuedEditClearCapturedNotice(client,noticeOwner,noticeKey));check();
    if(!admitted.claim||admitted.disposition==='unknown')return result('recovery-required','The local save reply was interrupted. Recover this draft before retrying.');
    return await finish(client,native,storage,check,admitted.claim,input.editor);
  }catch(error){
    if(letGo(error))throw error;
    return result('blocked',error instanceof Error?error.message:'Could not queue the message.');
  }finally{release()}
}
/** Fresh native discovery owns recovery. No repeated enqueue or guessed rollback. */
export async function mobileThreadTransferResume(client:MobileDraftClient,nativeInput:Native|null|undefined,storage:Files,
  target:MobileOutboxThreadTarget,transferId:string,decision?:'commit'|'rollback'|'retry',editor?:MountedSendAdmission):Promise<ThreadTransferResult> {
  if(!client.preferencesLoaded)return result('blocked','Wait for saved drafts to load.');
  const release=acquire(client,target);if(!release)return result('busy','This message transfer is already being recovered.');
  try {
    const {native,check}=invocation(client,nativeInput,target);
    await mobileOutboxRead(client,native);check();
    const discovered=await mobileOutboxThreadTransferStatus(client,native,target,transferId);check();
    if(!discovered.claim)return result('blocked','The saved message transfer is unavailable. Keep the draft until it is recovered.');
    let claim=discovered.claim;
    const outcome=discovered.outcome,epoch=mobileOutboxSnapshot(client).ownerEpoch;
    const retryDurability=decision==='retry'&&['queued','failed'].includes(claim.state)&&outcome
      &&['uncertain','unknown'].includes(outcome.status)&&outcome.messageId===claim.messageId
      &&outcome.mutationId===claim.mutationId&&!!epoch&&outcome.ownerEpoch===epoch;
    if(decision&&(claim.state==='prepared'||retryDurability)){
      if(!['commit','rollback','retry'].includes(decision))return result('blocked','Choose a supported recovery action.');
      await mobileOutboxRecover(client,native,claim.messageId,claim.mutationId,decision);check();
      await mobileOutboxRead(client,native);check();
      claim=await status(client,native,target,transferId);check();
    }
    return await finish(client,native,storage,check,claim,editor);
  }catch(error){if(letGo(error))throw error;return result('blocked',error instanceof Error?error.message:'Could not recover the message.')}
  finally{release()}
}
