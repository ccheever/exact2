// App-owned ordinary editor admission and explicit document revision ledger.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { T3Client } from './shared/client';
import type { MobileComposerTarget } from './composer-target';
import type { Obj } from './shared/domain';
import { mobileComposerContextCaptureTarget,mobileComposerContextObserveTarget } from './composer-command-context';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileEditorDocumentEnroll, mobileEditorDocumentWritten, mobileEditorDocument, mobileEditorDocumentKey, mobileEditorDocumentCapture, mobileEditorDocumentCommit, type EditorDocumentIntent } from './composer-editor-persistence';
import { mobileComposerEditorAdmit, type ComposerEditorState, type ComposerEditorDocument } from './composer-editor-state';

export interface EditorRouteInput {
  active:boolean; routeVisit:string; editorId:string; environmentId:string; threadId:string;
  readOnly:boolean; voiceBusy:boolean;
  focusIntent:{serial:string;attempt:number;operation:'none'|'focus'|'blur'};
}
export interface EditorDocumentLedger { key:string; incarnation:string; revision:number; value:string; selection:{start:number;end:number}|null }
export interface EditorIntentCapture {
  id:string; producer:string; target:MobileComposerTarget; key:string; incarnation:string; revision:number;
  before:string; selection:{start:number;end:number}; admission:string; mountId:string; eventCount:number;
}
export interface EditorCommandEffect { id:string; revision:number; added?:Obj; mode:'plan'|'default'|null; settings:string; intent:EditorIntentCapture; retirementKey?:string }
export interface EditorRootEffect { id:string; kind:'focus'|'blur'|'submit'; payload:string }
export interface EditorOwner {
  admission:string; signature:string; target:MobileComposerTarget; route:EditorRouteInput;
  state:ComposerEditorState; document:EditorDocumentLedger; pending:EditorCommandEffect|null;
  effects:EditorRootEffect[]; serial:number; revision:number; dismissed:string; error:string;
}
interface Registry { serial:number; revision:number; active:EditorOwner|null; documents:Map<string,EditorDocumentLedger> }
const registries=new WeakMap<object,Registry>();
export const editorCopy=<T>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
function registry(client:T3Client):Registry {
  let result=registries.get(client.local);
  if(!result){result={serial:0,revision:0,active:null,documents:new Map()};registries.set(client.local,result)}
  return result;
}
const ordinary=(target:MobileComposerTarget)=>target.kind==='ordinary' && !!target.threadId && target.key===`${target.environmentId}:${target.threadId}`;
const keyOf=(target:MobileComposerTarget)=>JSON.stringify([target.origin,target.environmentId,target.key]);
function document(client:T3Client,target:MobileComposerTarget,value:string):EditorDocumentLedger {
  const r=registry(client),key=keyOf(target);let entry=r.documents.get(key);
  if(!entry){entry={key,incarnation:`volatile-${++r.serial}`,revision:0,value,selection:null};r.documents.set(key,entry)}
  return entry;
}
export function mobileEditorOwner(client:T3Client):EditorOwner|null{return registry(client).active}
export function mobileEditorOwnerRevision(client:T3Client):number{return registry(client).revision}
export function mobileEditorOwnerChanged(client:T3Client):void{registry(client).revision++}
/** No draft mismatch heuristic: same admission keeps its independently observed document. */
export function mobileEditorOwnerAdmit(client:T3Client,target:MobileComposerTarget,route:EditorRouteInput,catalog:string):EditorOwner|null {
  const r=registry(client);
  if(!route.active || !ordinary(target) || target.origin!==client.origin || target.environmentId!==client.environmentId || target.generation!==client.generation || target.key!==client.draftKey || route.environmentId!==target.environmentId || route.threadId!==target.threadId
    || !route.editorId || !route.routeVisit){if(r.active){r.active=null;r.revision++}return null}
  const signature=JSON.stringify([target.owner,route.routeVisit,route.editorId,catalog]);
  if(r.active?.signature===signature){r.active.route=editorCopy(route);return r.active}
  const durable=mobileEditorDocumentEnroll(client,target);if(!durable){r.active=null;r.revision++;return null}
  const entry=document(client,target,client.local.drafts[target.key]??'');
  Object.assign(entry,{incarnation:durable.incarnation,revision:durable.revision,value:durable.value,selection:durable.selection});
  if(entry.value!==(client.local.drafts[target.key]??'')){r.active=null;r.revision++;return null}
  const epoch=`editor-${++r.serial}`,identity={owner:target.owner,editorId:route.editorId,routeVisit:route.routeVisit,renderEpoch:epoch};
  const state=mobileComposerEditorAdmit(null,identity,{value:entry.value,selection:entry.selection??{start:entry.value.length,end:entry.value.length}});
  r.active={admission:JSON.stringify([signature,epoch]),signature,target:editorCopy(target),route:editorCopy(route),state,document:entry,
    pending:null,effects:[],serial:0,revision:++r.revision,dismissed:'',error:''};
  return r.active;
}
/** Called at the actual synchronous ordinary reducer boundary, including plain input.
 * The before value is captured before reducing; projection never calls this to infer a write. */
export function mobileEditorOwnerWritten(client:T3Client,target:MobileComposerTarget,before:string,after:string):boolean {
  if(!ordinary(target) || target.origin!==client.origin || target.environmentId!==client.environmentId || target.generation!==client.generation
    || target.key!==client.draftKey || (client.local.drafts[target.key]??'')!==after)return false;
  const entry=document(client,target,before);if(entry.value!==before)return false;
  const durableKey=mobileEditorDocumentKey({origin:mobileQueuedEditOrigin(client).trim().replace(/\/+$/,''),environmentId:target.environmentId,draftKey:target.key});
  if(mobileEditorDocument(client,durableKey)&&!mobileEditorDocumentWritten(client,target,before,after))return false;
  if(before!==after){entry.value=after;entry.selection=null;entry.revision++;registry(client).revision++}
  return true;
}
export function mobileEditorCaptureIntent(client:T3Client,target:MobileComposerTarget,producer:string,
  selection?:{start:number;end:number}):EditorIntentCapture|null {
  const owner=mobileEditorOwner(client);if(!owner || owner.target.owner!==target.owner || !ordinary(target) || !producer || target.origin!==client.origin || target.environmentId!==client.environmentId || target.generation!==client.generation || target.key!==client.draftKey
    || owner.document.value!==(client.local.drafts[target.key]??'') || owner.state.value!==owner.document.value)return null;
  const range=selection??owner.state.selection;
  if(!Number.isSafeInteger(range.start)||!Number.isSafeInteger(range.end)||range.start<0||range.end<range.start||range.end>owner.state.value.length)return null;
  return {id:`${owner.state.identity.renderEpoch}-intent-${++owner.serial}`,producer,target:editorCopy(target),key:owner.document.key,
    incarnation:owner.document.incarnation,revision:owner.document.revision,before:owner.document.value,selection:{...range},
    admission:owner.admission,mountId:owner.state.mountId,eventCount:owner.state.eventCount};
}
export function mobileEditorIntentCurrent(client:T3Client,capture:EditorIntentCapture):boolean {
  const owner=mobileEditorOwner(client);
  return !!owner && owner.admission===capture.admission && owner.target.owner===capture.target.owner
    && owner.target.origin===client.origin && owner.target.generation===client.generation && owner.target.key===client.draftKey
    && owner.document.key===capture.key && owner.document.incarnation===capture.incarnation && owner.document.revision===capture.revision
    && owner.document.value===capture.before && (client.local.drafts[capture.target.key]??'')===capture.before
    && owner.state.mountId===capture.mountId && owner.state.eventCount===capture.eventCount;
}
/** Explicit publication API only. Existing producers are not activated here. A mounted
 * replacement requires the runtime's native CAS; this handles already committed unmounted work. */
export function mobileEditorPublishCommitted(client:T3Client,capture:EditorIntentCapture,next:ComposerEditorDocument):boolean {
  const r=registry(client),entry=r.documents.get(capture.key);
  if(!entry || entry.incarnation!==capture.incarnation || entry.revision!==capture.revision || entry.value!==capture.before
    || capture.target.origin!==client.origin || capture.target.environmentId!==client.environmentId || capture.target.generation!==client.generation
    || r.active?.target.owner===capture.target.owner && !!r.active.state.mountId
    || (client.local.drafts[capture.target.key]??'')!==next.value || next.value.length>1_000_000
    || !Number.isSafeInteger(next.selection.start)||!Number.isSafeInteger(next.selection.end)||next.selection.start<0||next.selection.end<next.selection.start||next.selection.end>next.value.length)return false;
  if(!mobileEditorDocumentWritten(client,capture.target,capture.before,next.value,next.selection))return false;
  entry.value=next.value;entry.selection={...next.selection};entry.revision++;r.revision++;
  // Retire an unmounted admission so a future mount seeds from this named intent.
  if(r.active?.target.owner===capture.target.owner)r.active=null;
  return true;
}
export function mobileEditorClaimEffect(client:T3Client,admission:string,id:string):EditorRootEffect|null {
  const owner=mobileEditorOwner(client);if(!owner || owner.admission!==admission || owner.target.origin!==client.origin || owner.target.environmentId!==client.environmentId || owner.target.generation!==client.generation || owner.target.key!==client.draftKey)return null;
  const index=owner.effects.findIndex(effect=>effect.id===id);if(index<0)return null;
  const effect=owner.effects.splice(index,1)[0]!;registry(client).revision++;
  if(effect.kind==='submit'){
    const captured=JSON.parse(effect.payload) as {documentRevision:number;value:string};
    if(owner.document.revision!==captured.documentRevision || owner.document.value!==captured.value || owner.state.value!==captured.value || owner.state.composing){
      owner.error='The draft changed before the submit action was handled. Review it and send again.';return null;
    }
  }
  return effect;
}

/** Named capture is independent of active route, but keeps the actual connection receipt. */
export function mobileEditorCaptureDocumentIntent(client:T3Client,target:MobileComposerTarget,producer:string,selection?:{start:number;end:number}):EditorDocumentIntent|null {
  return mobileEditorDocumentCapture(client,target,producer,selection);
}
/** No await between mounted-owner refusal and the exact named slot mutation. */
export function mobileEditorCommitDocumentIntent(client:T3Client,capture:EditorDocumentIntent,next:ComposerEditorDocument):boolean {
  const r=registry(client),active=r.active;
  if(active?.document.incarnation===capture.incarnation && active.target.key===capture.target.key && active.target.origin===capture.target.origin && active.state.mountId)return false;
  const context=mobileComposerContextCaptureTarget(client,capture.target);if(!context)return false;
  if(!mobileEditorDocumentCommit(client,capture,next))return false;
  if(!mobileComposerContextObserveTarget(client,context,next.value))return false;
  const entry=r.documents.get(keyOf(capture.target));
  if(entry)Object.assign(entry,{value:next.value,selection:{...next.selection},revision:capture.revision+1,incarnation:capture.incarnation});
  if(active?.target.key===capture.target.key&&active.target.origin===capture.target.origin)r.active=null;
  r.revision++;return true;
}
