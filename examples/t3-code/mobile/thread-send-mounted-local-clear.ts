// Pinned365aa87982 typed usage setComposerDraftText / feedback clearComposerDraftContentState.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type {MobileDraftClient} from './mobile-draft-recovery';
import type {MobileComposerTarget} from './composer-target';
import type {ThreadSendSnapshot} from './thread-send-admission';
import type {ThreadLocalClearResult} from './thread-send-local-clear';
import {mobileThreadMountedSendCurrent,type MountedSendAdmission} from './thread-send-mounted';
import {mobileComposerContextSendSnapshot} from './composer-command-context';
import {mobileEditorOwner,mobileEditorCaptureIntent,mobileEditorContextTargetCurrent,type EditorCommandReceipt} from './composer-editor-owner';
import {mobileEditorRequestIntent} from './composer-editor-runtime';
import {mobileEditorDocumentKey,mobileEditorDocument} from './composer-editor-persistence';
import {mobileOutboxTransferCanonical as canonical} from './mobile-outbox-transfer-model';
import {mobileCacheCatalogIdentity} from './mobile-client-cache-catalog';
import {mobileQueuedEditOrigin} from './queued-edit-origin';
import {mobileQueuedEditCurrent} from './queued-edit-state';
import {pendingRequests} from './shared/requests';
import {fleet} from './shared/settings-b-fleet';
import {obj} from './shared/domain';
import {ClientError,type Native,type Files} from './shared/protocol';
import {letGo} from './shared/let-go';

/** Explicit mounted callback only. Local-command owner retains report/notice/RPC authority.
 * No transfer, busy owner or journal is created; the existing pending CAS owns the effect. */
export async function mobileThreadMountedLocalCommandClear(client:MobileDraftClient,target:MobileComposerTarget,
  captured:ThreadSendSnapshot,mode:'text'|'content',admission:MountedSendAdmission,native:Native,storage:Files):Promise<ThreadLocalClearResult> {
  let applied=false,expectedIncarnation='',minimumRevision=0;const receipt:EditorCommandReceipt={id:'',applied:null,contentCleared:null};
  const refuse=()=>({applied:false,message:'The command draft changed. Its content has been kept.'});
  const owner=mobileEditorOwner(client),catalog=mobileCacheCatalogIdentity(fleet.saved,target.environmentId);
  const current=()=>!!owner&&mobileEditorOwner(client)===owner&&owner.route.active&&!owner.route.readOnly&&!owner.route.voiceBusy
    &&mobileEditorContextTargetCurrent(client,target)&&!mobileQueuedEditCurrent(client)&&!pendingRequests(client.projection).inputs.length&&catalog===mobileCacheCatalogIdentity(fleet.saved,target.environmentId)
    &&catalog===JSON.stringify([target.environmentId,mobileQueuedEditOrigin(client).trim().replace(/\/+$/,'')]);
  const transport=()=>client.origin===target.origin&&client.environmentId===target.environmentId&&client.generation===target.generation
    &&catalog===mobileCacheCatalogIdentity(fleet.saved,target.environmentId);
  const check=()=>{
    if(!transport())throw new ClientError('The selected composer changed.','superseded');
    if(expectedIncarnation){const document=mobileEditorDocument(client,mobileEditorDocumentKey({origin:captured.origin,environmentId:target.environmentId,draftKey:target.key}));
      if(!document||document.blocked||document.incarnation!==expectedIncarnation||document.revision<minimumRevision
        ||document.value!==(client.local.drafts[target.key]??''))throw new ClientError('The command document ownership changed.','superseded');}
  };
  try {
    if(!client.preferencesLoaded||!['text','content'].includes(mode)||target.kind!=='ordinary'||!owner)return refuse();
    // Strict raw read precedes legacy target/queued metadata readers.
    const saved=mobileComposerContextSendSnapshot(client,target);
    if(!saved||saved.images.length||saved.files.length||captured.attachments.length
      ||saved.text!==captured.rawText||canonical(saved.context)!==canonical(captured.context??null)
      ||captured.origin!==mobileQueuedEditOrigin(client)||captured.environmentId!==target.environmentId
      ||captured.threadId!==target.threadId||captured.draftKey!==target.key||!current()
      ||!mobileThreadMountedSendCurrent(client,target,admission)||mobileQueuedEditCurrent(client)
      ||pendingRequests(client.projection).inputs.length)return refuse();
    const intent=mobileEditorCaptureIntent(client,target,'typed-local-clear');if(!intent)return refuse();
    expectedIncarnation=intent.incarnation;minimumRevision=intent.revision;
    const request=mobileEditorRequestIntent(client,intent,{value:'',selection:{start:0,end:0}},undefined,native,storage,null,'',undefined,
      {mode,snapshot:saved,incarnation:intent.incarnation,revision:intent.revision,cleared:null},receipt);
    const answer=await request;
    applied=!!receipt.id&&answer.command?.id===receipt.id&&answer.command.applied
      &&(mode==='text'||answer.command.contentCleared===true);
    if(!applied)return refuse();
    if(answer.message)return {applied,message:answer.message};
    check();
    // An event-first ACK does not prove that invocation's save succeeded. Observe a fresh
    // full preference write here before the caller can dispatch feedback on this answer.
    let written=false;
    const guarded:Files={fs:{...storage.fs,atomicWriteFile:async(path,bytes)=>{
      check();const latest=mobileComposerContextSendSnapshot(client,target);
      if(!latest)throw new ClientError('The command draft cannot be saved safely.','retained');
      const wire=obj(JSON.parse(new TextDecoder().decode(bytes))),documentKey=mobileEditorDocumentKey({origin:captured.origin,environmentId:target.environmentId,draftKey:target.key});
      const documents=obj(obj(wire.mobileComposerEditor).documents),document=obj(documents[documentKey]),durable=mobileEditorDocument(client,documentKey);
      if(!durable||durable.blocked||durable.incarnation!==intent.incarnation)throw new ClientError('The command document ownership changed.','superseded');
      if(obj(wire.drafts)[target.key]!==latest.text||document.incarnation!==durable.incarnation||document.revision!==durable.revision
        ||canonical(obj(obj(wire.mobileComposerContexts).entries)[documentKey]??null)!==canonical(latest.contextRow)
        ||canonical(obj(wire.snapshotDrafts)[target.key]??[])!==canonical(latest.images)
        ||!Array.isArray(wire.composerFiles??[])||wire.composerFiles===null
        ||canonical(((wire.composerFiles??[]) as unknown[]).filter(file=>obj(file).draftKey===target.key))!==canonical(latest.files))throw new ClientError('The command save projection changed.','retained');
      await storage.fs.atomicWriteFile(path,bytes);written=true;check();
    }}};
    await client.persist(guarded);check();
    if(!written)throw new ClientError('The command draft was not saved.','retained');
    return {applied:true,message:''};
  }catch(error){
    if(letGo(error))throw error;
    applied=applied||receipt.applied===true&&(mode==='text'||receipt.contentCleared===true);
    return {applied,message:error instanceof Error?error.message:'Could not save the cleared command draft.'};
  }
}
