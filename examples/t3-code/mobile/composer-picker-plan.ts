// T3 Code365aa87982 use-composer-drafts.ts append/remove policy (MIT, LICENSE-T3).
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { OrdinaryInventoryAttachment } from './composer-attachment-publication';
import { mobileComposerAfterSelection, mobileComposerInsertContext,
  type MobileComposerInsertion, type MobileComposerInsertionDraft } from './composer-context-insertion';
import { collectComposerContextReferences, formatComposerContextReference, imageMimeType,
  replaceComposerContextReferences, sanitizeComposerContextLabel } from './composer-editor-document';
import type { MobileMessageContext } from './mobile-new-task-context';

export type ComposerPickerDraft = MobileComposerInsertionDraft<OrdinaryInventoryAttachment>;
export interface ComposerPickerPlan {
  readonly draft:ComposerPickerDraft;
  /** Only insertion assigns a new remembered caret in the source. */
  readonly selection:{start:number;end:number}|null;
  readonly accepted:readonly OrdinaryInventoryAttachment[];
  readonly rejected:readonly OrdinaryInventoryAttachment[];
  readonly removed:readonly OrdinaryInventoryAttachment[];
  readonly applied:boolean;
}

/** Source storage kind and semantic kind differ for pictures selected through Files.
 * Input is already validated, issued inventory; this helper grants no byte authority. */
function attachmentRecord(attachment:OrdinaryInventoryAttachment) {
  const row=attachment.type==='image'?attachment.image:attachment.file;
  const name=String(row.name),mimeType=String(row.mimeType);
  return {version:1,contextId:attachment.id,label:sanitizeComposerContextLabel(name,attachment.type),
    attachmentId:attachment.id,name,mimeType,sizeBytes:Number(row.sizeBytes),
    kind:attachment.type==='image'||imageMimeType({name,mimeType})!==null?'image':'file'};
}

/** Latest validated draft + immutable semantic picker capture. Same-text ABA uses
 * the captured range; changed text appends through the shared source insertion helper.
 * The returned rejected/removed rows remain caller-owned cleanup obligations. */
export function mobileComposerPickerAppend(draft:ComposerPickerDraft,incoming:readonly OrdinaryInventoryAttachment[],
  insertion:MobileComposerInsertion,maxAttachments=100):ComposerPickerPlan {
  const unchanged=(rejected:readonly OrdinaryInventoryAttachment[]):ComposerPickerPlan=>
    ({draft,selection:null,accepted:[],rejected,removed:[],applied:false});
  if(incoming.length===0)return unchanged([]);
  const retained=mobileComposerAfterSelection(draft,insertion);
  const remaining=Math.max(0,Math.min(100,maxAttachments)-retained.attachments.length);
  const capacity=Math.max(0,200-(retained.context?.records.length??0));
  const accepted=incoming.slice(0,Math.min(remaining,capacity));
  if(accepted.length===0)return unchanged(incoming);
  const records=accepted.map(attachmentRecord);
  // Append to the old inventory first, exactly as source append does. The shared
  // insertion then prunes only old context-owned file rows after reference merge.
  const augmented={...draft,attachments:[...draft.attachments,...accepted]};
  const next=mobileComposerInsertContext(augmented,{text:records.map(formatComposerContextReference).join(' '),
    context:{version:1,records}},insertion);
  if(!next)return unchanged(incoming);
  return {draft:next.draft,selection:next.selection,accepted,rejected:incoming.slice(accepted.length),
    removed:draft.attachments.filter(attachment=>!next.draft.attachments.includes(attachment)),applied:true};
}

/** Pinned referencedComposerContext dependency pass, using the same exact parser
 * as insertion. Removal must not prune additional unselected attachment rows. */
function referenced(text:string,context?:MobileMessageContext):MobileMessageContext|undefined {
  if(!context)return undefined;
  const ids=new Set(collectComposerContextReferences(text).map(ref=>ref.contextId));
  for(const record of context.records)if(ids.has(String(record.contextId))&&record.kind==='preview-annotation'
    &&record.screenshotContextId)ids.add(String(record.screenshotContextId));
  const records=context.records.filter(record=>ids.has(String(record.contextId)));
  if(records.length===context.records.length)return context;
  return records.length?{version:1,records}:undefined;
}

/** Remove every source reference bound to this actual attachment, not merely the
 * file row's preferred contextId. Whitespace and unrelated inventory are unchanged. */
export function mobileComposerPickerRemove(draft:ComposerPickerDraft,attachmentId:string):ComposerPickerPlan {
  const ids=new Set(draft.context?.records.filter(record=>'attachmentId' in record&&record.attachmentId===attachmentId)
    .map(record=>record.contextId));
  const text=replaceComposerContextReferences(draft.text,ref=>ids.has(ref.contextId)?'':ref.source);
  const context=referenced(text,draft.context),removed=draft.attachments.filter(row=>row.id===attachmentId);
  const attachments=draft.attachments.filter(row=>row.id!==attachmentId);
  const empty=text.length===0&&attachments.length===0&&draft.modelSelection===undefined
    &&draft.runtimeMode===undefined&&draft.interactionMode===undefined&&draft.workspaceSelection===undefined&&draft.project===undefined;
  const next:ComposerPickerDraft=empty?{text:'',attachments:[],context:undefined}:{...draft,text,context,attachments};
  const applied=text!==draft.text||next.context!==draft.context||removed.length>0;
  return {draft:applied?next:draft,selection:null,
    accepted:[],rejected:[],removed,applied};
}
