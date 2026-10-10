// Pure projections adapted from T3 Code365aa87982 use-composer-drafts.ts (MIT, LICENSE-T3).
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { MobileMessageContext } from './mobile-new-task-context';
import { collectComposerContextReferences } from './composer-editor-document';

export interface MobileComposerInsertion { readonly text:string; readonly start:number; readonly end:number }
export interface MobileComposerInsertionAttachment { readonly id:string; readonly type:'image'|'file' }
export interface MobileComposerInsertionDraft<A extends MobileComposerInsertionAttachment> {
  readonly text:string; readonly context?:MobileMessageContext; readonly attachments:readonly A[];
  // Opaque source settings affect empty-draft retention, not insertion ownership.
  readonly modelSelection?:unknown; readonly runtimeMode?:string; readonly interactionMode?:string;
  readonly workspaceSelection?:unknown; readonly project?:unknown;
}
export interface MobileComposerInsertionContent<A extends MobileComposerInsertionAttachment> {
  readonly text:string; readonly context:MobileMessageContext; readonly attachments?:readonly A[];
}
export interface MobileComposerInsertionResult<A extends MobileComposerInsertionAttachment> {
  readonly draft:MobileComposerInsertionDraft<A>; readonly selection:{start:number;end:number}; readonly removed:readonly A[];
}
/** Caller supplies its captured or remembered range for this exact document owner.
 * Null means no remembered range. Moving the caret alone preserves captured offsets;
 * changed text appends, including returning to captured offsets after text ABA. */
export function mobileComposerInsertionRange(currentText:string,target:MobileComposerInsertion|null=null):{start:number;end:number} {
  const selection=target?.text===currentText?target:null;
  const start=Math.max(0,Math.min(selection?.start??currentText.length,currentText.length));
  return {start,end:Math.max(start,Math.min(selection?.end??start,currentText.length))};
}
/** Pinned ordered dependency pass; callers validate full raw records before projection. */
function referenced(text:string,context?:MobileMessageContext):MobileMessageContext|undefined {
  if(!context)return undefined;
  const ids=new Set(collectComposerContextReferences(text).map(ref=>ref.contextId));
  for(const record of context.records){
    if(ids.has(String(record.contextId))&&record.kind==='preview-annotation'&&'screenshotContextId' in record&&record.screenshotContextId)
      ids.add(String(record.screenshotContextId));
  }
  const records=context.records.filter(record=>ids.has(String(record.contextId)));
  if(records.length===context.records.length)return context;
  return records.length?{version:1,records}:undefined;
}
/** All images and non-context files survive; context files require a retained record. */
function withReferencedFiles<A extends MobileComposerInsertionAttachment>(draft:MobileComposerInsertionDraft<A>,text:string,context:MobileMessageContext|undefined):MobileComposerInsertionDraft<A> {
  const previousIds=new Set(draft.context?.records.flatMap(record=>'attachmentId' in record?[record.attachmentId]:[]));
  const retainedIds=new Set(context?.records.flatMap(record=>'attachmentId' in record?[record.attachmentId]:[]));
  return {...draft,text,context,attachments:draft.attachments.filter(attachment=>attachment.type==='image'||!previousIds.has(attachment.id)||retainedIds.has(attachment.id))};
}
/** The source counts the retained prefix/suffix with exactly one literal separator. */
export function mobileComposerAfterSelection<A extends MobileComposerInsertionAttachment>(draft:MobileComposerInsertionDraft<A>,target:MobileComposerInsertion|null=null):MobileComposerInsertionDraft<A> {
  const {start,end}=mobileComposerInsertionRange(draft.text,target),text=`${draft.text.slice(0,start)} ${draft.text.slice(end)}`;
  return withReferencedFiles(draft,text,referenced(text,draft.context));
}
export function mobileComposerCountAttachmentsAfterSelection<A extends MobileComposerInsertionAttachment>(draft:MobileComposerInsertionDraft<A>,target:MobileComposerInsertion|null=null):number {
  return mobileComposerAfterSelection(draft,target).attachments.length;
}
/** Rich import is all-or-nothing at its live attachment cap; append's accepted-prefix
 * policy is a separate source operation and is intentionally not implemented here.
 * Null leaves ALL incoming content.attachments owned by the caller for guarded cleanup.
 * Success removed contains prior draft objects only, not incoming attachment disposal.
 * No IO, owner admission, history, mutation, record decoding or byte disposal occurs. */
export function mobileComposerInsertContext<A extends MobileComposerInsertionAttachment>(draft:MobileComposerInsertionDraft<A>,content:MobileComposerInsertionContent<A>,target:MobileComposerInsertion|null=null):MobileComposerInsertionResult<A>|null {
  const attachments=content.attachments??[],retained=mobileComposerAfterSelection(draft,target);
  if(attachments.length>0&&retained.attachments.length+attachments.length>100)return null;
  const {start,end}=mobileComposerInsertionRange(draft.text,target),before=draft.text.slice(0,start),after=draft.text.slice(end);
  const insertion=`${before.length>0&&!/\s$/.test(before)&&!/^\s/.test(content.text)?' ':''}${content.text}${!/\s$/.test(content.text)&&(after.length===0||!/^\s/.test(after))?' ':''}`;
  const text=before+insertion+after,records=new Map(draft.context?.records.map(record=>[record.contextId,record]));
  for(const record of content.context.records)records.set(record.contextId,record);
  const context=referenced(text,{version:1,records:[...records.values()]});
  if((context?.records.length??0)>200)return null;
  const next=withReferencedFiles({...draft,attachments:[...draft.attachments,...attachments]},text,context),cursor=start+insertion.length;
  return {draft:next,selection:{start:cursor,end:cursor},removed:draft.attachments.filter(attachment=>!next.attachments.includes(attachment))};
}
/** Invalid rich flavor falls back to this raw insertion, without rich separators. */
export function mobileComposerInsertText<A extends MobileComposerInsertionAttachment>(draft:MobileComposerInsertionDraft<A>,value:string,target:MobileComposerInsertion):MobileComposerInsertionResult<A> {
  const {start,end}=mobileComposerInsertionRange(draft.text,target),text=draft.text.slice(0,start)+value+draft.text.slice(end);
  const projected=withReferencedFiles(draft,text,referenced(text,draft.context)),cursor=start+value.length;
  // Source setComposerDraftText drops an empty store row; snapshot returns EMPTY_DRAFT.
  const empty=projected.text.length===0&&projected.attachments.length===0&&projected.modelSelection===undefined
    &&projected.runtimeMode===undefined&&projected.interactionMode===undefined&&projected.workspaceSelection===undefined&&projected.project===undefined;
  const next:MobileComposerInsertionDraft<A>=empty?{text:'',attachments:[],context:undefined}:projected;
  return {draft:next,selection:{start:cursor,end:cursor},removed:draft.attachments.filter(attachment=>!next.attachments.includes(attachment))};
}
