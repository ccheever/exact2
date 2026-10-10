// Mounted ordinary Photos/Files admission. Native and storage handles live only in invocations.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type {T3Client} from './shared/client';
import {ClientError,bridgeReply,type Native,type Files} from './shared/protocol';
import {letGo} from './shared/let-go';
import {localId} from './shared/composer-editor';
import {mobileComposerCountAttachmentsAfterSelection} from './composer-context-insertion';
import {pendingRequests} from './shared/requests';
import {fleet} from './shared/settings-b-fleet';
import {obj,str} from './shared/domain';
import {mobileCacheCatalogIdentity} from './mobile-client-cache-catalog';
import {mobileEditorOwner,mobileEditorCaptureIntent,mobileEditorContextTargetCurrent,type EditorIntentCapture,type EditorCommandReceipt} from './composer-editor-owner';
import {mobileEditorAction,mobileEditorRequestIntent} from './composer-editor-runtime';
import {mobileEditorFilesReady,mobileEditorFilesIntake,mobileEditorFilesIntakeEnd} from './composer-file-runtime';
import {mobileComposerContextPickerRead,mobileComposerContextPickerAfter,type ComposerPickerPublication} from './composer-command-context';
import {mobileComposerPickerAppend,mobileComposerPickerRemove,type ComposerPickerPlan} from './composer-picker-plan';
import {collectComposerContextReferences} from './composer-editor-document';
import {mobilePickerIntakeRequest,mobilePickerIntakeInvoke,type PickerIntakeRequest,type PickerIntakeValue} from './composer-picker-intake-io';
import type {OrdinaryInventoryAttachment} from './composer-attachment-publication';
interface Capture {intent:EditorIntentCapture;catalog:string}
interface Work {
  capture:Capture;request:PickerIntakeRequest;issued:PickerIntakeValue|null;
  command:{receipt:EditorCommandReceipt;publication:ComposerPickerPublication}|null;
  finish:PickerIntakeRequest|null;message:string;
}
const works=new WeakMap<T3Client,Work>();
// Transport replacement can postpone a saved-image handoff. Keep its plain original request
// separately so it never blocks the replacement editor or impersonates its ownership.
const deferred=new WeakMap<T3Client,Work[]>();
/** Mobile source Files support differs from the desktop upload capability fallback. */
export function mobileComposerPickerFileLimit(client:T3Client,source:'photos'|'files'):number {
  const caps=obj(obj(client.config.environment).capabilities),limit=obj(caps.fileAttachments).maxUploadBytes;
  if(source==='photos'&&caps.attachmentUploads!==true)return 0;
  return typeof limit==='number'&&Number.isSafeInteger(limit)&&limit>0?Math.min(limit,50*1024*1024):0;
}
const fail=(message='The attachment editor changed. Keep the current draft.')=>new ClientError(message,'retained');
function current(client:T3Client,capture:Capture):boolean {
  const owner=mobileEditorOwner(client),i=capture.intent;
  return !!owner&&owner.admission===i.admission&&owner.state.mountId===i.mountId&&owner.document.incarnation===i.incarnation
    &&owner.document.key===i.key&&owner.route.active&&!owner.route.readOnly&&!owner.route.voiceBusy
    &&mobileEditorContextTargetCurrent(client,i.target)&&!pendingRequests(client.projection).inputs.length
    &&capture.catalog===mobileCacheCatalogIdentity(fleet.saved,i.target.environmentId);
}
function namedTarget(client:T3Client,work:Work){return {...work.capture.intent.target,generation:client.generation}}
export const mobileComposerPickerHasWork=(client:T3Client):boolean=>works.has(client);
function namedCurrent(client:T3Client,work:Work):boolean {
  const {intent,catalog}=work.capture,t=intent.target;
  return client.origin===t.origin&&client.environmentId===t.environmentId
    &&catalog===mobileCacheCatalogIdentity(fleet.saved,t.environmentId)
    &&!!mobileComposerContextPickerAfter(client,namedTarget(client,work),intent.key,intent.incarnation);
}
function capture(client:T3Client,expectedOwner:string):Capture {
  const owner=mobileEditorOwner(client);
  if(!client.preferencesLoaded||!owner||owner.target.owner!==expectedOwner||!owner.state.mountId||owner.pending||owner.state.composing
    ||!mobileEditorFilesReady(client,owner.admission)||!mobileComposerContextPickerRead(client,owner.target))throw fail('The composer is not ready for attachments.');
  const intent=mobileEditorCaptureIntent(client,owner.target,'picker');if(!intent)throw fail();
  const value={intent,catalog:mobileCacheCatalogIdentity(fleet.saved,owner.target.environmentId)};
  if(!current(client,value))throw fail();return value;
}
function base(work:Work,action:'status'|'cancel'|'finish') {
  const {generation,identity,operationId}=work.request;return {op:'composerPickerIntake',action,generation,identity,operationId};
}
function attachments(work:Work):OrdinaryInventoryAttachment[] {
  const target=work.capture.intent.target;
  return (work.issued?.files??[]).map(row=>row.kind==='image'?{type:'image',id:row.id,
    image:{id:row.id,name:row.name,mimeType:row.mimeType,sizeBytes:row.sizeBytes}}:
    {type:'file',id:row.id,file:{id:row.id,contextId:row.id,draftKey:target.key,environmentId:target.environmentId,
      name:row.name,mimeType:row.mimeType,sizeBytes:row.sizeBytes,source:'attached',attachmentId:'',status:'staged',
      ...(row.videoWidth===undefined?{}:{videoWidth:row.videoWidth}),...(row.videoHeight===undefined?{}:{videoHeight:row.videoHeight})}});
}
/** Actual native owner retirement keeps unresolved cleanup safe. Cancellation never asserts deletion. */
async function cancel(client:T3Client,work:Work,native:Native):Promise<void> {
  const answer=await mobilePickerIntakeInvoke(mobilePickerIntakeRequest(base(work,'cancel')),native,()=>true);
  if(answer.value?.status!=='cancelled'&&answer.value?.status!=='finished')throw fail(answer.message||'Attachment cleanup is pending. Retry this action.');
  mobileEditorFilesIntakeEnd(client,work.capture.intent.admission,work.request.operationId);
  if(works.get(client)===work)works.delete(client);
}
function removalSelection(before:string,plan:ComposerPickerPlan,selection:{start:number;end:number},records:readonly import('./shared/domain').Obj[]) {
  const removedIds=new Set(plan.removed.map(a=>a.id)),contexts=new Set(records.filter(r=>removedIds.has(String(r.attachmentId))).map(r=>r.contextId));
  const spans=collectComposerContextReferences(before).filter(r=>contexts.has(r.contextId));
  const offset=(value:number)=>{let deleted=0;for(const span of spans){if(value<=span.start)break;deleted+=Math.min(value,span.end)-span.start}return Math.max(0,Math.min(plan.draft.text.length,value-deleted))};
  return {start:offset(selection.start),end:offset(selection.end)};
}
async function apply(client:T3Client,captured:Capture,plan:ComposerPickerPlan,native:Native,storage:Files,work?:Work):Promise<void> {
  if(!current(client,captured))throw fail();
  const before=mobileComposerContextPickerRead(client,captured.intent.target),intent=mobileEditorCaptureIntent(client,captured.intent.target,'picker');
  if(!before||!intent)throw fail();
  const acceptedIds=new Set(plan.accepted.map(a=>a.id));
  const publication:ComposerPickerPublication={before:before.attachments,after:[...plan.draft.attachments],
    added:(plan.draft.context?.records??[]).filter(r=>acceptedIds.has(String(r.attachmentId))),context:before.context,contextRevision:before.contextRevision,published:false,applied:null};
  const receipt:EditorCommandReceipt={id:'',applied:null,contentCleared:null};
  if(work)work.command={receipt,publication};
  const selection=plan.selection??removalSelection(before.text,plan,intent.selection,before.context?.records??[]);
  let answer;
  try {answer=await mobileEditorRequestIntent(client,intent,{value:plan.draft.text,selection},undefined,native,storage,null,'',undefined,undefined,receipt,publication)}
  catch(error){if(work&&!receipt.id)work.command=null;throw error}
  if(!answer.command?.applied||!publication.published)throw fail(answer.message||'The draft changed before the attachment edit. Try again.');
  if(publication.applied===false){const message='Attachment metadata changed during the edit. Your latest text was kept; attach the files again.';if(work)work.message=message;else throw fail(message)}
}
async function finish(client:T3Client,work:Work,native:Native,storage:Files):Promise<void> {
  if(work.finish){
    const status=await mobilePickerIntakeInvoke(mobilePickerIntakeRequest(base(work,'status')),native,()=>true);
    if(status.state!=='answered'||!status.value)throw fail(status.message||'Attachment adoption is still unresolved.');
    if(status.value.status==='staged')work.finish=null;
    else if(status.value.status!=='finished')throw fail('Attachment adoption is not ready.');
  }
  if(!work.finish){
    if(!namedCurrent(client,work))throw fail();
    let after:ReturnType<typeof mobileComposerContextPickerAfter>=null,written=false;
    const {intent}=work.capture;
    const guarded:Files={fs:{...storage.fs,atomicWriteFile:async(path,bytes)=>{
      if(!namedCurrent(client,work))throw fail();
      const projection=mobileComposerContextPickerAfter(client,namedTarget(client,work),intent.key,intent.incarnation);if(!projection)throw fail();
      await storage.fs.atomicWriteFile(path,bytes);after=projection;written=true;
    }}};
    await client.persist(guarded);
    if(!written||!after)throw fail('The attachment draft was not saved. Retry this action.');
    const saved=after as NonNullable<ReturnType<typeof mobileComposerContextPickerAfter>>,ids=new Set(saved.attachmentIds),issued=work.issued!.files.map(f=>f.id);
    work.finish=mobilePickerIntakeRequest({...base(work,'finish'),publication:{after:saved,
      acceptedIds:issued.filter(id=>ids.has(id)),discardedIds:issued.filter(id=>!ids.has(id))}});
  }
  const answer=await mobilePickerIntakeInvoke(work.finish,native,()=>namedCurrent(client,work));
  if(answer.value?.status!=='finished')throw fail(answer.message||'Attachment adoption is pending. Retry this action.');
  mobileEditorFilesIntakeEnd(client,work.capture.intent.admission,work.request.operationId);
  if(works.get(client)===work)works.delete(client);
}
/** Existing attachment busy ownership serializes invocations. Retained plain work retries the
 * same issued operation/command; it never reopens a picker or reapplies a successful edit. */
export async function mobileComposerPickerAction(client:T3Client,source:string,id:string,expectedOwner:string,native:Native,storage:Files):Promise<string> {
  let work=works.get(client);
  try {
    for(const old of [...(deferred.get(client)??[])])if(namedCurrent(client,old)&&(old.command||old.finish)){
      await finish(client,old,native,storage);deferred.set(client,(deferred.get(client)??[]).filter(item=>item!==old));
    }
    if(work&&!current(client,work.capture)){
      const owner=mobileEditorOwner(client),retired=!owner||owner.admission!==work.capture.intent.admission||owner.state.mountId!==work.capture.intent.mountId;
      if(work.command&&!work.command.publication.published&&!retired)throw fail('The original editor still owns an unresolved attachment edit. Finish that edit before retrying.');
      if(work.command||work.finish){
        // Positively retired admission cannot publish a late terminal. Reconcile only
        // the actual current named save, without guessing or replaying the old CAS.
        if(namedCurrent(client,work))await finish(client,work,native,storage);
        else {deferred.set(client,[...(deferred.get(client)??[]),work]);works.delete(client)}
      }else await cancel(client,work,native);
      throw fail('The previous editor closed. Its attachment work remains bound to that draft; try this action again.');
    }
    if(!work){
      const captured=capture(client,expectedOwner);
      if(source==='remove-image'||source==='remove-file'){
        const draft=mobileComposerContextPickerRead(client,captured.intent.target)!;
        const plan=mobileComposerPickerRemove(draft,id);if(plan.applied)await apply(client,captured,plan,native,storage);
        return '';
      }
      if(!client.projectId)throw fail('Choose a project first.');
      if(source==='menu'){
        const answer=await bridgeReply(native,{op:'mobileAttachmentSource',supportsFiles:mobileComposerPickerFileLimit(client,'files')>0});
        if(!answer.ok)throw fail(answer.error!.message);
        if(str(obj(answer.value).error))throw fail(str(obj(answer.value).error));source=str(obj(answer.value).source);
        if(!source)return '';
      }
      if(!current(client,captured))throw fail();
      if(source!=='photos'&&source!=='files')throw fail('Choose Photo Library or Choose Files.');
      const draft=mobileComposerContextPickerRead(client,captured.intent.target);if(!draft)throw fail();
      // Native selection capacity counts the retained source range. Acceptance is checked again after IO.
      const fileLimit=mobileComposerPickerFileLimit(client,source);if(source==='files'&&!fileLimit)throw fail('This server does not support file attachments.');
      const i=captured.intent,owner=mobileEditorOwner(client)!,remaining=Math.max(0,100-mobileComposerCountAttachmentsAfterSelection(draft,{text:i.before,...i.selection}));
      if(!remaining)throw fail('You can attach up to 100 attachments per message.');
      const request=mobilePickerIntakeRequest({op:'composerPickerIntake',action:'pick',generation:i.target.generation,
        identity:{...owner.state.identity,mountId:i.mountId},operationId:localId(),source,remaining,fileLimit,
        document:{incarnation:i.incarnation,revision:i.revision},target:{origin:i.target.origin,environmentId:i.target.environmentId,threadId:i.target.threadId,draftKey:i.target.key}});
      work={capture:captured,request,issued:null,command:null,finish:null,message:''};works.set(client,work);
    }
    if(work.command?.receipt.applied===false){await cancel(client,work,native);return 'The draft changed before the attachment edit. Choose the files again.'}
    if(!work.issued){
      const result=await mobilePickerIntakeInvoke(work.request,native,()=>current(client,work!.capture));
      if(result.value)work.issued=result.value;
      if(!current(client,work.capture)){await cancel(client,work,native);throw fail()}
      if(result.state!=='answered'||result.value?.status!=='staged'){work.issued=null;throw fail(result.message||'The picker is still pending. Retry this action.')}
      work.message=result.value.error;
    }
    if(!current(client,work.capture)){if(work.command?.publication.published&&namedCurrent(client,work))await finish(client,work,native,storage);else if(!work.command)await cancel(client,work,native);throw fail()}
    if(work.command){
      if(work.command.receipt.applied!==true){
        const owner=mobileEditorOwner(client)!;
        if(owner.pending?.id!==work.command.receipt.id)throw fail('The attachment command is still unresolved. Keep this draft.');
        await mobileEditorAction(client,owner.route,'retry',work.command.receipt.id,native,storage,()=>0);
      }
      if(work.command.receipt.applied!==true||!work.command.publication.published)throw fail('The attachment edit was not accepted. Retry this action.');
    }else{
      const incoming=attachments(work);
      await mobileEditorFilesIntake(client,work.capture.intent.admission,work.request.operationId,incoming.flatMap(a=>a.type==='file'?[a.file]:[]),native);
      if(!current(client,work.capture)){await cancel(client,work,native);throw fail()}
      const draft=mobileComposerContextPickerRead(client,work.capture.intent.target);if(!draft)throw fail();
      const i=work.capture.intent,plan=mobileComposerPickerAppend(draft,incoming,{text:i.before,...i.selection});
      if(plan.rejected.length)work.message=[work.message,`${plan.rejected.length} attachment(s) did not fit in this draft.`].filter(Boolean).join(' ');
      if(plan.applied)await apply(client,work.capture,plan,native,storage,work);
    }
    await finish(client,work,native,storage);return work.message;
  }catch(error){if(letGo(error))throw error;return error instanceof Error?error.message:'Could not attach files.'}
}
