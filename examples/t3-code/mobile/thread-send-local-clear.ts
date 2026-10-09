// Pinned365aa87982 typed usage command and feedback clearComposerDraftContent.
// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import type {MobileDraftClient} from './mobile-draft-recovery';
import type {MobileComposerTarget} from './composer-target';
import type {ThreadSendSnapshot} from './thread-send-admission';
import {mobileComposerContextSendSnapshot} from './composer-command-context';
import {mobileEditorOwner,mobileEditorContextTargetCurrent,mobileEditorCaptureDocumentIntent,mobileEditorCommitDocumentContextIntent} from './composer-editor-owner';
import {mobileEditorDocumentEnroll} from './composer-editor-persistence';
import {mobileDraftChanged} from './draft';
import {mobileQueuedEditOrigin} from './queued-edit-origin';
import {mobileCacheCatalogIdentity} from './mobile-client-cache-catalog';
import {mobileOutboxTransferCanonical as canonical} from './mobile-outbox-transfer-model';
import {pendingRequests} from './shared/requests';
import {fleet} from './shared/settings-b-fleet';
import {letGo} from './shared/let-go';
import type {Native,Files} from './shared/protocol';
export interface ThreadLocalClearResult {applied:boolean;message:string}
/** Local command callbacks invoke this synchronously before their first await. No queue
 * claim or upload permission can authorize this clear. Newer content refuses unchanged. */
export async function mobileThreadLocalCommandClear(client:MobileDraftClient,target:MobileComposerTarget,
  captured:ThreadSendSnapshot,mode:'text'|'content',native:Native,storage:Files):Promise<ThreadLocalClearResult> {
  let applied=false;
  const refuse=()=>({applied:false,message:'The command draft changed. Its content has been kept.'});
  try {
    if(!client.preferencesLoaded||!['text','content'].includes(mode)||target.kind!=='ordinary'
      ||!target.threadId||target.threadId.startsWith('new:')||target.key!==`${target.environmentId}:${target.threadId}`)return refuse();
    // Validate complete raw stores before helpers that may initialize legacy metadata.
    const saved=mobileComposerContextSendSnapshot(client,target);
    if(!saved||saved.images.length||saved.files.length||captured.attachments.length
      ||saved.text!==captured.rawText||canonical(saved.context)!==canonical(captured.context??null)
      ||captured.origin!==mobileQueuedEditOrigin(client)||captured.environmentId!==target.environmentId
      ||captured.threadId!==target.threadId||captured.draftKey!==target.key
      ||!mobileEditorContextTargetCurrent(client,target)||pendingRequests(client.projection).inputs.length
      ||mobileCacheCatalogIdentity(fleet.saved,target.environmentId)!==JSON.stringify([target.environmentId,captured.origin]))return refuse();
    const rich=mobileEditorOwner(client);
    if(rich&&rich.target.origin===target.origin&&rich.target.environmentId===target.environmentId&&rich.target.key===target.key)return refuse();
    const document=mobileEditorDocumentEnroll(client,target);
    if(!document||document.revision>=Number.MAX_SAFE_INTEGER)return refuse();
    if(mode==='text'){
      // The existing ordinary input reducer performs its write synchronously, including
      // the document and context observers; only persistence yields.
      const pending=mobileDraftChanged(client,'',native,storage,target.owner);
      applied=client.local.drafts[target.key]==='';
      const result=await pending;
      return {applied,message:result.message||(!applied?'The command draft could not be cleared.':'')};
    }
    const intent=mobileEditorCaptureDocumentIntent(client,target,'feedback-clear');
    if(!intent)return refuse();
    const committed=mobileEditorCommitDocumentContextIntent(client,intent,{value:'',selection:{start:0,end:0}},[]);
    if(!committed.ok)return refuse();
    applied=true;
    await client.persist(storage);
    return {applied,message:''};
  }catch(error){
    if(letGo(error))throw error;
    return {applied,message:error instanceof Error?error.message:'Could not save the cleared command draft.'};
  }
}
