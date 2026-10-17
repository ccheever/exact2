// Ordinary Send's scoped draft publication; native queue receipts own byte admission.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { T3Client } from './shared/client';
import type { MobileComposerTarget } from './composer-target';
import { mobileEditorCompleteQueuedSend, mobileEditorOwner, mobileEditorContextTargetCurrent } from './composer-editor-owner';
import { mobileComposerContextSendSnapshot } from './composer-command-context';
import { mobileEditorDocumentEnroll, mobileEditorDocumentCapture } from './composer-editor-persistence';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { fleet } from './shared/settings-b-fleet';
import { threadSendTransferDecodeDraftContent, threadSendTransferDecodeCapture, threadSendTransferDecodeClaim,
  type ThreadSendTransferCapture, type ThreadSendTransferCompletion } from './thread-send-transfer-model';

export type ThreadSendDraftCaptureResult={status:'captured';capture:ThreadSendTransferCapture}|{status:'blocked';reason:string};
export type ThreadSendDraftApplyResult={status:'applied'|'already-applied';marker:ThreadSendTransferCompletion}|{status:'blocked';reason:string};
const blocked=(reason:string)=>({status:'blocked' as const,reason});
function home(client:T3Client):string{return mobileQueuedEditOrigin(client).trim().replace(/\/+$/,'')}
function catalog(client:T3Client,environmentId:string,origin:string):boolean {
  return environmentId===client.environmentId&&home(client)===origin
    &&mobileCacheCatalogIdentity(fleet.saved,environmentId)===JSON.stringify([environmentId,origin]);
}
/** A ready handshake can arrive after this turn: also refuse an admitted, not-yet-mounted rich editor. */
function active(client:T3Client,target:MobileComposerTarget):boolean {
  const owner=mobileEditorOwner(client);
  return !!owner&&owner.target.origin===target.origin&&owner.target.environmentId===target.environmentId&&owner.target.key===target.key;
}
/** Explicit Send enrollment only after full nonmutating content/store validation. No Native/IO,
 * equality-based write detection, textarea caret guess, or synthetic provisional document. */
export function mobileThreadSendCaptureDraft(client:T3Client,target:MobileComposerTarget):ThreadSendDraftCaptureResult {
  try {
    if(!client.preferencesLoaded)return blocked('Load saved draft ownership before sending.');
    // Preflight before target helpers: malformed raw optional stores must not be defaulted.
    const snapshot=mobileComposerContextSendSnapshot(client,target);
    if(!snapshot||!mobileEditorContextTargetCurrent(client,target)||active(client,target)
      ||!catalog(client,target.environmentId,home(client)))return blocked('The ordinary draft is unavailable or has an active rich editor.');
    const content=threadSendTransferDecodeDraftContent({key:target.key,origin:home(client),environmentId:target.environmentId,threadId:target.threadId,
      text:snapshot.text,context:snapshot.context,contextRevision:snapshot.contextRevision,images:snapshot.images,files:snapshot.files,
      attachmentIds:snapshot.attachmentIds,attachmentOrder:snapshot.attachmentOrder});
    const document=mobileEditorDocumentEnroll(client,target);
    if(!document)return blocked('The saved document ownership is unavailable.');
    const capture=threadSendTransferDecodeCapture({version:2,kind:'ordinary',draft:{...content,
      document:{incarnation:document.incarnation,revision:document.revision,selection:document.selection}}});
    return {status:'captured',capture};
  } catch{return blocked('The ordinary draft snapshot is invalid. Keep the draft.');}
}
/** The caller must obtain this exact claim from the native queued-transfer operation. Decoding
 * validates DTOs, not authenticity. This synchronous function has no native/persistence authority.
 * The same confirmed claim may finish after focus moves, but only in its original home environment. */
export function mobileThreadSendApplyQueuedDraft(client:T3Client,input:unknown):ThreadSendDraftApplyResult {
  try {
    if(!client.preferencesLoaded)return blocked('Load saved draft ownership before applying this transfer.');
    const claim=threadSendTransferDecodeClaim(input);
    if(claim.state!=='queued'||!claim.capture||!claim.record||!catalog(client,claim.environmentId,claim.origin))
      return blocked('The ordinary transfer has not been queued for this environment.');
    // A named ordinary document target; never resolve current focus or a NewTask presentation.
    const target:MobileComposerTarget={kind:'ordinary',origin:client.origin,environmentId:claim.environmentId,generation:client.generation,
      projectId:client.projectId,threadId:claim.threadId,key:claim.draftKey,editorOwner:claim.draftKey,editOwner:'',incarnation:'',
      owner:JSON.stringify(['ordinary',client.origin,claim.environmentId,client.generation,claim.draftKey])};
    if(active(client,target))return blocked('Finish the active rich editor before applying this transfer.');
    const current=mobileEditorDocumentCapture(client,target,'ordinary-send-transfer');
    if(!current)return blocked('The current enrolled document is unavailable. Keep the queued transfer.');
    const result=mobileEditorCompleteQueuedSend(client,current,claim);
    return result.ok?{status:result.alreadyApplied?'already-applied':'applied',marker:result.marker}
      :blocked('The draft projection could not be safely recorded. Keep the queued transfer.');
  } catch{return blocked('The ordinary queued-transfer proof is invalid.');}
}
