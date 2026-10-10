// Ordinary Thread context ownership, adapted from T3 Code (MIT); see LICENSE-T3.
// Source365aa87982 composerContext.ts/use-composer-drafts.ts; mobile-only persistence seam.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { T3Client } from './shared/client';
import { mobileEditorDocument as sendDocument } from './composer-editor-persistence';
import { threadSendTransferDecodeCompletion as sendCompletion } from './thread-send-transfer-model';
import { mobileOutboxTransferCanonical as sendCanonical } from './mobile-outbox-transfer-model';
import { obj, str, type Obj } from './shared/domain';
import { collectComposerContextReferences } from './composer-editor-document';
import { draftFiles, fileContextRecords } from './shared/composer-editor-files';
import { ClientError } from './shared/protocol';
import type { MobileComposerTarget } from './composer-target';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileContextRecordValid } from './mobile-context-record';
import { mobileCreateContextHistory, mobileNewTaskContextProject, mobileReferencedComposerContext, type MobileMessageContext } from './mobile-new-task-context';
import { mobileEditorDocumentCommit, mobileEditorDocumentCommitMounted, mobileEditorDocumentCommitObservation, mobileEditorDocumentIntentCurrent, type EditorDocumentIntent } from './composer-editor-persistence';
import { mobileComposerFileHistoryProject, type ComposerFileHistory } from './composer-file-history';
import type { HeldFile } from './composer-file-holds-io';
import type { ComposerEditorDocument, ComposerEditorEvent } from './composer-editor-state';
import { mobileComposerInsertContext, type MobileComposerInsertion } from './composer-context-insertion';
import { mobileComposerAttachmentInventoryRead, mobileComposerAttachmentPublicationPrepare } from './composer-attachment-publication';

interface Scope { origin:string; environmentId:string; key:string }
interface Entry extends Scope { incarnation:number; revision:number; text:string; context?:unknown }
interface Store { entries:Map<string,Entry>; invalid?:unknown }
export interface ComposerContextGuard extends Scope { slot:string; originAtCapture:string; generation:number; incarnation:number; revision:number; text:string }
export type ComposerContextRead = {ok:true;context:MobileMessageContext|undefined;revision:number} | {ok:false;error:string;revision:number};
const stores=new WeakMap<object,Store>();
const histories=new WeakMap<Entry,ReturnType<typeof mobileCreateContextHistory>>();
let incarnation=0;
const clone=<T>(value:T):T=>JSON.parse(JSON.stringify(value));
const normalized=(origin:string)=>origin.trim().replace(/\/+$/,'');
const slot=(scope:Scope)=>JSON.stringify([scope.origin,scope.environmentId,scope.key]);
function store(client:T3Client):Store {
  let current=stores.get(client.local);
  if(!current) { current={entries:new Map()}; stores.set(client.local,current); }
  return current;
}
function scopeFor(client:T3Client,key=client.draftKey):Scope|null {
  const environmentId=client.environmentId, origin=normalized(mobileQueuedEditOrigin(client));
  const prefix=`${environmentId}:`, threadId=key.startsWith(prefix)?key.slice(prefix.length):'';
  if(!origin || !environmentId || !threadId || threadId.startsWith('new:') || key.includes('~queued-edit~') || key.startsWith('new-task:')) return null;
  return {origin,environmentId,key};
}
function activeScope(client:T3Client):Scope|null {
  return client.threadId && client.draftKey===`${client.environmentId}:${client.threadId}` ? scopeFor(client):null;
}
function entryFor(client:T3Client,scope:Scope,create=false):Entry|undefined {
  const registry=store(client), id=slot(scope); let entry=registry.entries.get(id);
  if(!entry && create) {
    entry={...scope,incarnation:++incarnation,revision:0,text:client.local.drafts[scope.key]??''};
    registry.entries.set(id,entry);
  }
  return entry;
}
function invalid(revision=0):ComposerContextRead { return {ok:false,error:'This draft contains unsupported or invalid context. Keep the original draft.',revision}; }

/** Call at each admitted text reduction, not by guessing from stale view output.
 * Persist also observes as a fallback for existing unmigrated producers. */
export function mobileComposerContextObserve(client:T3Client):void {
  const scope=activeScope(client); if(!scope) return;
  const entry=entryFor(client,scope); if(!entry) return;
  const text=client.local.drafts[scope.key]??'';
  if(text===entry.text) return;
  const decoded=mobileNewTaskContextProject(entry.text,entry.context);
  if(decoded.ok) {
    const history=histories.get(entry)??mobileCreateContextHistory();
    const restored=history(text,decoded.context); histories.set(entry,history);
    if(restored) entry.context=clone(restored); else delete entry.context;
  }
  entry.text=text; entry.revision++;
}
export function mobileComposerContextCapture(client:T3Client,target:Pick<MobileComposerTarget,'kind'|'key'|'origin'|'generation'>):ComposerContextGuard|null {
  if(target.kind!=='ordinary' || target.key!==client.draftKey || target.origin!==client.origin || target.generation!==client.generation) return null;
  const scope=activeScope(client); if(!scope || store(client).invalid!==undefined) return null;
  mobileComposerContextObserve(client);
  const entry=entryFor(client,scope,true)!;
  return {...scope,slot:slot(scope),originAtCapture:client.origin,generation:client.generation,incarnation:entry.incarnation,revision:entry.revision,text:entry.text};
}
/** Explicit named-document preflight for an unmounted producer. Never borrows the focused thread. */
export function mobileComposerContextCaptureTarget(client:T3Client,target:MobileComposerTarget):ComposerContextGuard|null {
  if(target.kind!=='ordinary' || target.origin!==client.origin || target.environmentId!==client.environmentId
    || target.generation!==client.generation || !target.threadId || target.key!==`${target.environmentId}:${target.threadId}`) return null;
  const scope=scopeFor(client,target.key); if(!scope || store(client).invalid!==undefined) return null;
  const entry=entryFor(client,scope,true)!;
  if(entry.text!==(client.local.drafts[target.key]??'') || !mobileNewTaskContextProject(entry.text,entry.context).ok) return null;
  return {...scope,slot:slot(scope),originAtCapture:client.origin,generation:client.generation,
    incarnation:entry.incarnation,revision:entry.revision,text:entry.text};
}
/** Pair with capture and an accepted named text write in one synchronous turn. No guessed reconciliation. */
export function mobileComposerContextObserveTarget(client:T3Client,guard:ComposerContextGuard,after:string):boolean {
  const scope=scopeFor(client,guard.key), registry=store(client), entry=registry.entries.get(guard.slot);
  if(!scope || slot(scope)!==guard.slot || client.origin!==guard.originAtCapture || client.generation!==guard.generation
    || registry.invalid!==undefined || !entry || entry.incarnation!==guard.incarnation || entry.revision!==guard.revision
    || entry.text!==guard.text || (client.local.drafts[guard.key]??'')!==after || after.length>1_000_000) return false;
  const decoded=mobileNewTaskContextProject(entry.text,entry.context); if(!decoded.ok) return false;
  if(after===entry.text) return true;
  const history=histories.get(entry)??mobileCreateContextHistory(), restored=history(after,decoded.context);
  histories.set(entry,history);
  if(restored) entry.context=clone(restored); else delete entry.context;
  entry.text=after;entry.revision++;return true;
}
type BatchRead = {ok:true;records:Obj[];context:MobileMessageContext|undefined} | {ok:false;error:string};
/** Validate the entire payload before pruning. A refused batch never seeds undo history. */
function batchContext(raw:unknown,bounded=false):{ok:true;context:MobileMessageContext|undefined}|{ok:false} {
  if(raw===undefined) return {ok:true,context:undefined};
  try {
    const value=obj(raw);
    if(!Array.isArray(value.records) || bounded && (value.records.length>200 || JSON.stringify(value.records).length>16_000_000)
      || !mobileNewTaskContextProject('',raw).ok) return {ok:false};
    return {ok:true,context:clone(raw) as MobileMessageContext};
  } catch { return {ok:false}; }
}
function batchEntry(client:T3Client,guard:ComposerContextGuard):Entry|undefined {
  const scope=scopeFor(client,guard.key), registry=store(client), entry=registry.entries.get(guard.slot);
  return scope && slot(scope)===guard.slot && client.origin===guard.originAtCapture && client.generation===guard.generation
    && registry.invalid===undefined && entry?.incarnation===guard.incarnation && entry.revision===guard.revision
    && Number.isSafeInteger(entry.revision) && entry.revision<Number.MAX_SAFE_INTEGER
    && entry.text===guard.text && (client.local.drafts[guard.key]??'')===guard.text && guard.text.length<=1_000_000 ? entry:undefined;
}
/** Preflight for an import, not permission to publish after an await. Recapture at publication. */
export function mobileComposerContextPrepareBatch(client:T3Client,target:MobileComposerTarget,addedRecords:readonly Obj[]):BatchRead {
  const added=batchContext({version:1,records:addedRecords},true);
  if(!added.ok) return {ok:false,error:'This import contains unsupported or excessive context.'};
  const guard=mobileComposerContextCaptureTarget(client,target), entry=guard && batchEntry(client,guard);
  const current=entry && batchContext(entry.context);
  return current?.ok ? {ok:true,records:added.context!.records,context:current.context}
    :{ok:false,error:'This draft changed or contains unsupported context. Keep the original draft.'};
}
/** Metadata only: latest text must already be observed in this named entry. Never replay terminal text.
 * The caller owns native CAS, attachment holds and one-snapshot persistence after this synchronous turn. */
export function mobileComposerContextCommitBatch(client:T3Client,guard:ComposerContextGuard,addedRecords:readonly Obj[]):boolean {
  const entry=batchEntry(client,guard); if(!entry) return false;
  const current=batchContext(entry.context), added=batchContext({version:1,records:addedRecords},true);
  if(!current.ok || !added.ok) return false;
  const history=histories.get(entry)??mobileCreateContextHistory();
  const records=new Map(history.snapshot().map(record=>[str(record.contextId),record]));
  for(const record of [...(current.context?.records??[]),...added.context!.records]) records.set(str(record.contextId),record);
  // Merge before the dependency pass: a new annotation can refer to an undo-held screenshot.
  const next=mobileReferencedComposerContext(guard.text,{version:1,records:[...records.values()]});
  const checked=batchContext(next,true); if(!checked.ok) return false;
  // Retain late deleted imports for Undo, then refresh every live record so the bound evicts only undo payloads.
  history(guard.text,{version:1,records:[...(current.context?.records??[]),...added.context!.records]});
  history(guard.text,checked.context);
  histories.set(entry,history);
  if(checked.context) entry.context=checked.context; else delete entry.context;
  entry.revision++;client.revision++;return true;
}
export type ComposerContextDocumentResult = {ok:true;documentRevision:number;contextRevision:number}
  | {ok:false;reason:'superseded'|'invalid-document'|'invalid-context'|'limit'};
/** Internal transaction seam: only mobileEditorCommitDocumentContextIntent supplies mounted-owner
 * admission. The concrete document commit is the only write before metadata installation; no caller
 * callback or prepared installer is exposed. Attachment inventory and byte ownership are NOT reconciled. */
export function mobileComposerContextCommitDocument(client:T3Client,capture:EditorDocumentIntent,
  next:ComposerEditorDocument,addedRecords:readonly Obj[]):ComposerContextDocumentResult {
  if(!mobileEditorDocumentIntentCurrent(client,capture))return {ok:false,reason:'superseded'};
  if(typeof next.value!=='string'||!next.selection||!Number.isSafeInteger(next.selection.start)
    ||!Number.isSafeInteger(next.selection.end)||next.selection.start<0||next.selection.end<next.selection.start
    ||next.selection.end>next.value.length)return {ok:false,reason:'invalid-document'};
  if(next.value.length>1_000_000||!Number.isSafeInteger(capture.revision)||capture.revision<0
    ||capture.revision>=Number.MAX_SAFE_INTEGER||!Number.isSafeInteger(client.revision)||client.revision<0
    ||client.revision>=Number.MAX_SAFE_INTEGER)return {ok:false,reason:'limit'};
  const scope=scopeFor(client,capture.target.key),registry=store(client);
  if(!scope||registry.invalid!==undefined)return {ok:false,reason:'invalid-context'};
  const id=slot(scope),entry=registry.entries.get(id);
  if(entry&&entry.text!==capture.before)return {ok:false,reason:'superseded'};
  if(entry&&(!Number.isSafeInteger(entry.revision)||entry.revision<0||entry.revision>=Number.MAX_SAFE_INTEGER)
    ||!entry&&incarnation>=Number.MAX_SAFE_INTEGER)return {ok:false,reason:'limit'};
  const current=batchContext(entry?.context),added=batchContext({version:1,records:addedRecords},true);
  if(!current.ok||!added.ok)return {ok:false,reason:'invalid-context'};
  const previous=entry?histories.get(entry):undefined, snapshot=previous?.snapshot()??[];
  if(!batchContext({version:1,records:snapshot}).ok)return {ok:false,reason:'invalid-context'};
  const records=new Map(snapshot.map(record=>[str(record.contextId),record]));
  for(const record of [...(current.context?.records??[]),...added.context!.records])records.set(str(record.contextId),record);
  const checked=batchContext(mobileReferencedComposerContext(next.value,{version:1,records:[...records.values()]}),true);
  if(!checked.ok)return {ok:false,reason:'limit'};
  // Build a detached fork completely before the document write. A refusal never changes undo recency.
  const history=mobileCreateContextHistory();
  history('',{version:1,records:snapshot});
  history(next.value,{version:1,records:[...(current.context?.records??[]),...added.context!.records]});
  history(next.value,checked.context);
  const prepared={value:next.value,selection:{start:next.selection.start,end:next.selection.end}},
    contextRevision=(entry?.revision??0)+1,documentRevision=capture.revision+1;
  const installed:Entry=entry??{...scope,incarnation:incarnation+1,revision:0,text:capture.before};
  const result:ComposerContextDocumentResult={ok:true,documentRevision,contextRevision};
  if(!mobileEditorDocumentCommit(client,capture,prepared))return {ok:false,reason:'superseded'};
  // No callback, validation or throwing clone after text publication. The document commit notified once.
  if(!entry){incarnation=installed.incarnation;registry.entries.set(id,installed)}
  if(checked.context)installed.context=checked.context;else delete installed.context;
  installed.text=prepared.value;installed.revision=contextRevision;histories.set(installed,history);
  return result;
}
export interface ComposerExternalContextContent { text:string; context:MobileMessageContext }
export type ComposerExternalContextResult = {ok:true;documentRevision:number;contextRevision:number;selection:{start:number;end:number};removedFileIds:string[]}
  | {ok:false;reason:'superseded'|'invalid-document'|'invalid-context'|'invalid-inventory'|'unsupported'|'limit'};
export type ComposerExternalContextCommit = {result:ComposerExternalContextResult;ledger?:{
  value:string;selection:{start:number;end:number};revision:number;incarnation:string}};
const plainRaw=(value:unknown):value is Record<string,unknown>=>value!==null&&typeof value==='object'&&!Array.isArray(value)
  &&[Object.prototype,null].includes(Object.getPrototypeOf(value));
function jsonRaw(value:unknown,parents=new Set<object>()):boolean {
  if(value===null||typeof value==='string'||typeof value==='boolean')return true;
  if(typeof value==='number')return Number.isFinite(value);
  if(!Array.isArray(value)&&!plainRaw(value)||parents.has(value as object))return false;
  parents.add(value as object);const values=Object.values(value as object),keys=Object.keys(value as object);
  const valid=(!Array.isArray(value)||keys.length===value.length&&keys.every((key,index)=>key===String(index)))&&values.every(child=>jsonRaw(child,parents));parents.delete(value as object);return valid;
}
/** The owning store may carry opaque invalid recovery/context payloads. Preserve them exactly;
 * validate its structural queue owner without normalizing, filtering or interpreting those payloads. */
function releaseStore(raw:unknown):Record<string,unknown>|null {
  if(raw===undefined)return {version:1,records:{},receipts:{},claims:{},fileReleases:[]};
  if(!plainRaw(raw)||!jsonRaw(raw)||raw.version!==1||!plainRaw(raw.records)||!plainRaw(raw.receipts)
    ||!plainRaw(raw.claims)||!Object.values(raw.claims).every(v=>typeof v==='string')||!Array.isArray(raw.fileReleases))return null;
  return clone(raw);
}
/** Nonmutating raw queue-owner preflight, before legacy target readers can initialize a null store. */
export function mobileComposerContextInventoryOwnerAvailable(client:T3Client):boolean {
  return releaseStore((client.local as typeof client.local & {mobileNewTaskDrafts?:unknown}).mobileNewTaskDrafts)!==null;
}
/** Internal concrete seam, called only after the owner wrapper's unmounted semantic admission.
 * Future context uses latest live records, never restores undo-only file metadata. No caller installer
 * or callback is accepted. All inventory/history/ledger allocations precede the one document write. */
export function mobileComposerContextInsertDocument(client:T3Client,capture:EditorDocumentIntent,
  insertion:MobileComposerInsertion,content:ComposerExternalContextContent):ComposerExternalContextCommit {
  const fail=(reason:Extract<ComposerExternalContextResult,{ok:false}>['reason']):ComposerExternalContextCommit=>({result:{ok:false,reason}});
  if(!mobileEditorDocumentIntentCurrent(client,capture))return fail('superseded');
  if(!plainRaw(content)||Object.keys(content).some(k=>k!=='text'&&k!=='context')||typeof content.text!=='string'
    ||!jsonRaw(content)||!plainRaw(insertion)||typeof insertion.text!=='string'||!Number.isSafeInteger(insertion.start)
    ||!Number.isSafeInteger(insertion.end)||insertion.start<0||insertion.end<insertion.start||insertion.end>insertion.text.length)return fail('invalid-document');
  if(content.text.length>1_000_000||insertion.text.length>1_000_000||!Number.isSafeInteger(capture.revision)||capture.revision<0
    ||capture.revision>=Number.MAX_SAFE_INTEGER||!Number.isSafeInteger(client.revision)||client.revision<0||client.revision>=Number.MAX_SAFE_INTEGER)return fail('limit');
  const scope=scopeFor(client,capture.target.key),registry=store(client);
  if(!scope||registry.invalid!==undefined)return fail('invalid-context');
  const id=slot(scope),entry=registry.entries.get(id);
  if(entry&&entry.text!==capture.before)return fail('superseded');
  if(entry&&(!Number.isSafeInteger(entry.revision)||entry.revision<0||entry.revision>=Number.MAX_SAFE_INTEGER)
    ||!entry&&incarnation>=Number.MAX_SAFE_INTEGER)return fail('limit');
  if(entry?.context!==undefined&&!jsonRaw(entry.context))return fail('invalid-context');
  const current=batchContext(entry?.context),added=batchContext(content.context,true);
  if(!current.ok||!added.ok||!added.context||!added.context.records.length
    ||added.context.records.some(record=>!['terminal','review-comment'].includes(str(record.kind))||'attachmentId' in record))return fail('invalid-context');
  const previous=entry?histories.get(entry):undefined,snapshot=previous?.snapshot()??[];
  if(!batchContext({version:1,records:snapshot}).ok)return fail('invalid-context');
  const local=client.local as typeof client.local & {composerFiles?:unknown;mobileAttachmentOrder?:unknown;mobileNewTaskDrafts?:unknown};
  const cleanup=releaseStore(local.mobileNewTaskDrafts);if(!cleanup)return fail('invalid-inventory');
  const inventory={snapshotDrafts:local.snapshotDrafts,composerFiles:local.composerFiles,mobileAttachmentOrder:local.mobileAttachmentOrder,
    snapshotReleases:local.snapshotReleases,fileReleases:cleanup.fileReleases};
  const target={environmentId:capture.target.environmentId,threadId:capture.target.threadId,draftKey:capture.target.key};
  const read=mobileComposerAttachmentInventoryRead(inventory,target);if(!read.ok)return fail(read.reason==='invalid-target'?'invalid-inventory':read.reason);
  const projected=mobileComposerInsertContext({text:capture.before,context:current.context,attachments:read.attachments},
    {text:content.text,context:added.context},insertion);
  if(!projected||projected.draft.text.length>1_000_000)return fail('limit');
  const checked=batchContext(projected.draft.context,true);if(!checked.ok)return fail('limit');
  const files=mobileComposerAttachmentPublicationPrepare({inventory,target,previousContext:current.context,nextContext:checked.context,nextText:projected.draft.text});
  if(!files.ok)return fail(files.reason==='invalid-target'?'invalid-inventory':files.reason);
  const history=mobileCreateContextHistory();
  history('',{version:1,records:snapshot});
  history(projected.draft.text,{version:1,records:[...(current.context?.records??[]),...added.context.records]});
  history(projected.draft.text,checked.context);
  const prepared={value:projected.draft.text,selection:{...projected.selection}},documentRevision=capture.revision+1,contextRevision=(entry?.revision??0)+1;
  const installed:Entry=entry??{...scope,incarnation:incarnation+1,revision:0,text:capture.before};
  const nextCleanup={...cleanup,fileReleases:files.fileReleases};
  const result:ComposerExternalContextResult={ok:true,documentRevision,contextRevision,selection:{...prepared.selection},removedFileIds:files.removedFileIds};
  const outcome:ComposerExternalContextCommit={result,ledger:{value:prepared.value,selection:{...prepared.selection},revision:documentRevision,incarnation:capture.incarnation}};
  if(!mobileEditorDocumentCommit(client,capture,prepared))return fail('superseded');
  // Prepared assignments only. No parsing, cloning, callback, second acceptance or native cleanup.
  if(!entry){incarnation=installed.incarnation;registry.entries.set(id,installed)}
  if(checked.context)installed.context=checked.context;else delete installed.context;
  installed.text=prepared.value;installed.revision=contextRevision;histories.set(installed,history);
  local.snapshotDrafts=files.snapshotDrafts;local.composerFiles=files.composerFiles;local.mobileAttachmentOrder=files.mobileAttachmentOrder;
  local.snapshotReleases=files.snapshotReleases;local.mobileNewTaskDrafts=nextCleanup;
  return outcome;
}
export function mobileComposerContextRead(client:T3Client,key=client.draftKey,text=client.local.drafts[key]??''):ComposerContextRead {
  const scope=scopeFor(client,key); if(!scope) return {ok:true,context:undefined,revision:0};
  const registry=store(client); if(registry.invalid!==undefined) return invalid();
  const entry=entryFor(client,scope); if(!entry) return {ok:true,context:undefined,revision:0};
  const result=mobileNewTaskContextProject(text,entry.context);
  return result.ok ? {...result,revision:entry.revision}:invalid(entry.revision);
}
/** Synchronous accepted-command commit; caller owns native CAS/route admission.
 * NewTask and queued edit stores are intentionally not written here. */
export function mobileComposerContextCommit(client:T3Client,guard:ComposerContextGuard,text:string,
  added?:Obj,removeId=''):boolean {
  const scope=activeScope(client), registry=store(client), entry=registry.entries.get(guard.slot);
  if(!scope || slot(scope)!==guard.slot || (client.origin!==guard.originAtCapture || client.generation!==guard.generation) || registry.invalid!==undefined || !entry || entry.incarnation!==guard.incarnation
    || entry.revision!==guard.revision || entry.text!==guard.text || (client.local.drafts[scope.key]??'')!==guard.text
    || text.length>1_000_000 || added!==undefined&&!mobileContextRecordValid(added)) return false;
  const decoded=mobileNewTaskContextProject(entry.text,entry.context);
  if(!decoded.ok) return false;
  const history=histories.get(entry)??mobileCreateContextHistory();
  const restored=history(text,decoded.context);
  const records=new Map((restored?.records??[]).map(record=>[str(record.contextId),record]));
  if(removeId) records.delete(removeId);
  if(added) records.set(str(added.contextId),clone(added));
  const next=mobileReferencedComposerContext(text,{version:1,records:[...records.values()]});
  const checked=mobileNewTaskContextProject(text,next);
  if(!checked.ok) return false;
  // A retained native terminal can arrive after later typing deleted its link.
  // Keep its validated record in undo history without restoring the old text.
  history(text,checked.context);
  if(added) history(text,{version:1,records:[added]});
  histories.set(entry,history);
  if(checked.context) entry.context=clone(checked.context); else delete entry.context;
  entry.text=text; entry.revision++; client.local.drafts[scope.key]=text; client.revision++;
  return true;
}

/** Saved metadata stays under its full canonical environment identity. Invalid
 * payloads are retained and block this owner's send rather than silently dropped. */
export function mobileComposerContextsHydrate(client:T3Client,saved:Obj):void {
  if(saved.mobileComposerContexts===undefined) return;
  const raw=obj(saved.mobileComposerContexts), entries=obj(raw.entries), registry:Store={entries:new Map()};
  if(raw.version!==1 || !raw.entries || typeof raw.entries!=='object' || Array.isArray(raw.entries)) {
    registry.invalid=clone(saved.mobileComposerContexts); stores.set(client.local,registry); return;
  }
  for(const [id,value] of Object.entries(entries)) {
    const row=obj(value), scope={origin:str(row.origin),environmentId:str(row.environmentId),key:str(row.key)};
    if(!scope.origin || normalized(scope.origin)!==scope.origin || !scope.environmentId || !scope.key.startsWith(`${scope.environmentId}:`)
      || !scope.key.slice(scope.environmentId.length+1) || scope.key.slice(scope.environmentId.length+1).startsWith('new:')
      || scope.key.includes('~queued-edit~') || scope.key.startsWith('new-task:')
      || id!==slot(scope) || !Number.isSafeInteger(row.revision) || Number(row.revision)<0 || typeof row.text!=='string' || row.text.length>1_000_000) {
      registry.invalid=clone(saved.mobileComposerContexts); stores.set(client.local,registry); return;
    }
    registry.entries.set(id,{...scope,revision:Number(row.revision),text:row.text,incarnation:++incarnation,
      ...(Object.hasOwn(row,'context')?{context:clone(row.context)}:{})});
  }
  // An admitted local edit made while loading owns its current metadata.
  for(const [id,entry] of store(client).entries) if(entry.revision>0) registry.entries.set(id,entry);
  stores.set(client.local,registry);
}
export function mobileComposerContextsPersisted(client:T3Client):unknown {
  mobileComposerContextObserve(client);
  const registry=store(client); if(registry.invalid!==undefined) return clone(registry.invalid);
  const entries:Record<string,unknown>={};
  for(const [id,entry] of registry.entries) if(entry.context!==undefined) {
    entries[id]={origin:entry.origin,environmentId:entry.environmentId,key:entry.key,revision:entry.revision,text:entry.text,context:clone(entry.context)};
  }
  return {version:1,entries};
}
/** Merge before mobileRecoveredMessageContext so immutable outbox recovery
 * records retain their existing precedence. Retries bypass the caller's merge. */
export function mobileComposerContextForSend(client:T3Client,key:string,text:string,next?:Obj,attachments:Obj[]=[]):Obj|undefined {
  const scope=scopeFor(client,key); if(!scope) return next;
  const result=mobileComposerContextRead(client,key,text);
  if(!result.ok) throw new ClientError(result.error,'retained');
  if(!result.context) return next;
  const records=new Map<string,Obj>();
  for(const record of Array.isArray(next?.records)?next.records:[]) records.set(str(obj(record).contextId),obj(record));
  const liveIds=new Set(attachments.map(file=>str(file.id)));
  for(const record of result.context.records) {
    let value=record;
    if('attachmentId' in record) {
      const localId=str(record.attachmentId);
      const image=(client.local.snapshotDrafts[key]??[]).find(file=>file.id===localId);
      const file=draftFiles(client.local).find(file=>file.draftKey===key && file.id===localId);
      const id=str(image?.uploadId)||file?.attachmentId||(liveIds.has(localId)?localId:'');
      if(!id || !liveIds.has(id)) throw new ClientError('A context attachment is unavailable. Keep the draft and attach it again.','retained');
      value={...record,attachmentId:id};
    }
    records.set(str(value.contextId),value);
  }
  if(!records.size) return next;
  const merged=mobileNewTaskContextProject(text,{version:1,records:[...records.values()]});
  if(!merged.ok) throw new ClientError(merged.error,'retained');
  return merged.context ? clone(merged.context) as unknown as Obj : undefined;
}

/** Exact ordinary Send projection: unlike Persisted(), this never observes/prunes text.
 * Internal Send wrappers also enforce no active same-document rich editor. */
export interface ComposerSendDraftSnapshot {
  text:string;context:MobileMessageContext|null;contextRevision:number;
  contextRow:{origin:string;environmentId:string;key:string;revision:number;text:string;context:MobileMessageContext}|null;
  images:Obj[];files:import('./shared/composer-editor-files').DraftFile[];attachmentIds:string[];attachmentOrder:string[]|null;
}
function sendJSON(value:unknown,parents=new Set<object>()):boolean {
  if(value===null||typeof value==='string'||typeof value==='boolean')return true;
  if(typeof value==='number')return Number.isFinite(value);
  if(!Array.isArray(value)&&!plainRaw(value)||parents.has(value as object))return false;
  parents.add(value as object);
  const array=Array.isArray(value),keys=Reflect.ownKeys(value as object).filter(k=>!array||k!=='length');
  const valid=(!array||keys.length===value.length&&Array.from({length:value.length},(_,i)=>Object.hasOwn(value,i)).every(Boolean))
    &&keys.every(k=>{const d=Object.getOwnPropertyDescriptor(value,k)!;return typeof k==='string'&&d.enumerable&&'value'in d&&sendJSON(d.value,parents)});
  parents.delete(value as object);return valid;
}
function sendRaw(client:T3Client,target:MobileComposerTarget) {
  const local=client.local as typeof client.local & {composerFiles?:unknown;mobileAttachmentOrder?:unknown;mobileNewTaskDrafts?:unknown;mobileOutboxTransferCompletions?:unknown};
  const scope=scopeFor(client,target.key),registry=store(client);
  if(!scope||target.origin!==client.origin||target.environmentId!==client.environmentId||target.generation!==client.generation
    ||target.kind!=='ordinary'||target.key!==`${target.environmentId}:${target.threadId}`||registry.invalid!==undefined
    ||!plainRaw(local.drafts)||!sendJSON(local.drafts)||!Object.values(local.drafts).every(v=>typeof v==='string'&&v.length<=1_000_000)
    ||local.mobileNewTaskDrafts!==undefined&&!sendJSON(local.mobileNewTaskDrafts))return null;
  const cleanup=releaseStore(local.mobileNewTaskDrafts);if(!cleanup)return null;
  const raw={snapshotDrafts:local.snapshotDrafts,composerFiles:local.composerFiles??[],mobileAttachmentOrder:local.mobileAttachmentOrder??{},
    snapshotReleases:local.snapshotReleases,fileReleases:cleanup.fileReleases};
  // Optional absence is supported; explicit null is never normalized into an empty inventory.
  if(local.composerFiles===null||local.mobileAttachmentOrder===null||!sendJSON(raw))return null;
  const read=mobileComposerAttachmentInventoryRead(raw,{environmentId:target.environmentId,threadId:target.threadId,draftKey:target.key});
  if(!read.ok)return null;
  const completions=local.mobileOutboxTransferCompletions===undefined?{}:local.mobileOutboxTransferCompletions;
  if(!plainRaw(completions)||!sendJSON(completions))return null;
  const id=slot(scope),entry=registry.entries.get(id),text=local.drafts[target.key]??'';
  for(const value of registry.entries.values())if(!sendJSON({...value,...(value.context===undefined?{}:{context:value.context})}))return null;
  if(entry&&(entry.text!==text||!Number.isSafeInteger(entry.revision)||entry.revision<0))return null;
  const checked=batchContext(entry?.context);if(!checked.ok)return null;
  const history=entry?histories.get(entry)?.snapshot()??[]:[];
  if(!sendJSON(history)||!batchContext({version:1,records:history}).ok)return null;
  const context=checked.context??null,contextRow=context?{...scope,revision:entry!.revision,text,context:clone(context)}:null;
  const files=(raw.composerFiles as import('./shared/composer-editor-files').DraftFile[]).filter(file=>file.draftKey===target.key);
  const orders=raw.mobileAttachmentOrder as Record<string,string[]>;
  const snapshot:ComposerSendDraftSnapshot={text,context:context?clone(context):null,contextRevision:entry?.revision??0,contextRow,
    images:clone(local.snapshotDrafts[target.key]??[]),files:clone(files),attachmentIds:read.attachments.map(a=>a.id),
    attachmentOrder:Object.hasOwn(orders,target.key)?clone(orders[target.key]!):null};
  return {local,scope,registry,id,entry,cleanup,raw,completions,snapshot};
}
export function mobileComposerContextSendSnapshot(client:T3Client,target:MobileComposerTarget):ComposerSendDraftSnapshot|null {
  return sendRaw(client,target)?.snapshot??null;
}
/** Internal concrete transaction: wrapper proves actual queued claim and rejects active rich
 * admission. Pure DTO validity alone is not queue authenticity. No arbitrary installer/callback. */
export function mobileComposerContextCompleteSend(client:T3Client,current:EditorDocumentIntent,
  claim:import('./thread-send-transfer-model').ThreadSendTransferClaim):
  {ok:true;marker:import('./thread-send-transfer-model').ThreadSendTransferCompletion;alreadyApplied:boolean}|{ok:false} {
  const raw=sendRaw(client,current.target);
  if(!raw||!mobileEditorDocumentIntentCurrent(client,current)||!Number.isSafeInteger(client.revision)||client.revision<0||client.revision>=Number.MAX_SAFE_INTEGER
    ||claim.state!=='queued'||!claim.capture||!claim.record)return {ok:false};
  const captured=claim.capture.draft,{snapshot,local,scope,entry}=raw;
  if(captured.key!==current.target.key||captured.origin!==scope.origin||captured.environmentId!==scope.environmentId||captured.threadId!==current.target.threadId)return {ok:false};
  const document=sendDocument(client,current.key);
  if(!document||document.blocked||document.incarnation===captured.document.incarnation&&document.revision<captured.document.revision)return {ok:false};
  const existing=raw.completions[claim.transferId];
  if(existing!==undefined){
    try {const marker=sendCompletion(existing,claim);if(sendCanonical(marker.after)===sendCanonical(sendAfter(document,snapshot)))return {ok:true,marker,alreadyApplied:true}}
    catch{return {ok:false}}
  }
  // A prior accepted publication is never applied a second time. Newer input refreshes a preserve proof.
  const exact=existing===undefined&&document.incarnation===captured.document.incarnation&&document.revision===captured.document.revision
    &&sendCanonical(document.selection)===sendCanonical(captured.document.selection)&&snapshot.text===captured.text
    &&snapshot.contextRevision===captured.contextRevision&&sendCanonical(snapshot.context)===sendCanonical(captured.context)
    &&sendCanonical(snapshot.images)===sendCanonical(captured.images)&&sendCanonical(snapshot.files)===sendCanonical(captured.files)
    &&sendCanonical(snapshot.attachmentIds)===sendCanonical(captured.attachmentIds)&&sendCanonical(snapshot.attachmentOrder)===sendCanonical(captured.attachmentOrder);
  if(exact&&(current.revision>=Number.MAX_SAFE_INTEGER||snapshot.contextRevision>=Number.MAX_SAFE_INTEGER))return {ok:false};
  const after=exact?{document:{origin:document.origin,environmentId:document.environmentId,threadId:document.threadId,draftKey:document.draftKey,
    incarnation:document.incarnation,revision:document.revision+1,selection:{start:0,end:0}},text:'',context:null,images:[],files:[],attachmentIds:[],attachmentOrder:null}:sendAfter(document,snapshot);
  let marker:import('./thread-send-transfer-model').ThreadSendTransferCompletion;
  try {marker=sendCompletion({version:2,kind:'ordinary',draftKey:claim.draftKey,fingerprint:claim.fingerprint,
    disposition:exact?'cleared':'preserved',before:{incarnation:captured.document.incarnation,revision:captured.document.revision},after},claim)}catch{return {ok:false}}
  const completions={...raw.completions,[claim.transferId]:marker};
  const result={ok:true as const,marker:clone(marker),alreadyApplied:false};
  if(!exact){local.mobileOutboxTransferCompletions=completions;client.revision++;return result}
  const images={...clone(local.snapshotDrafts),[current.target.key]:[]};
  const files=clone(raw.raw.composerFiles as import('./shared/composer-editor-files').DraftFile[]).filter(file=>file.draftKey!==current.target.key);
  const orders={...clone(raw.raw.mobileAttachmentOrder as Record<string,string[]>),[current.target.key]:[]};
  const snapshotReleases=[...local.snapshotReleases,...snapshot.images.map(image=>String(image.id)).filter(id=>!local.snapshotReleases.includes(id))];
  const oldFileReleases=raw.cleanup.fileReleases as string[];
  const cleanup={...raw.cleanup,fileReleases:[...oldFileReleases,...snapshot.files.map(file=>file.id).filter(id=>!oldFileReleases.includes(id))]};
  const history=mobileCreateContextHistory(),next={value:'',selection:{start:0,end:0}};
  if(!mobileEditorDocumentCommit(client,current,next))return {ok:false};
  // Every allocation/validation above precedes the single concrete text commit.
  if(entry){delete entry.context;entry.text='';entry.revision++;histories.set(entry,history)}
  local.snapshotDrafts=images;local.composerFiles=files;local.mobileAttachmentOrder=orders;
  local.snapshotReleases=snapshotReleases;local.mobileNewTaskDrafts=cleanup;local.mobileOutboxTransferCompletions=completions;
  return result;
}
function sendAfter(document:import('./composer-editor-persistence').EditorDurableDocument,snapshot:ComposerSendDraftSnapshot) {
  return {document:{origin:document.origin,environmentId:document.environmentId,threadId:document.threadId,draftKey:document.draftKey,
    incarnation:document.incarnation,revision:document.revision,selection:document.selection?{...document.selection}:null},
    // Completion describes actual serialized order. Raw capture order still fences pre-clear changes.
    text:snapshot.text,context:snapshot.contextRow,images:snapshot.images,files:snapshot.files,attachmentIds:snapshot.attachmentIds,
    attachmentOrder:snapshot.attachmentIds.length?snapshot.attachmentIds:null};
}

/** Internal mounted Send publication. The owner supplies an authenticated queued claim and,
 * for a terminal, the exact retained native command proof. This DTO is not write authority.
 * A null observation installs only a conservative durable fence; cold recovery never clears it.
 * Native text/metadata are prepared together before the one concrete document commit. */
export function mobileComposerContextMountedSend(client:T3Client,current:EditorDocumentIntent,
  claim:import('./thread-send-transfer-model').ThreadSendTransferClaim,
  terminal:{expectedMarker:string;next:ComposerEditorDocument;clear:boolean;proof:Parameters<typeof mobileEditorDocumentCommitMounted>[2]}|null) {
  const raw=sendRaw(client,current.target),document=sendDocument(client,current.key);
  const refuse=()=>({ok:false as const});
  if(!raw||!document||document.blocked||!mobileEditorDocumentIntentCurrent(client,current)
    ||!Number.isSafeInteger(client.revision)||client.revision<0||client.revision>=Number.MAX_SAFE_INTEGER
    ||claim.state!=='queued'||!claim.capture||!claim.record)return refuse();
  const captured=claim.capture.draft,{snapshot,local,scope,entry}=raw;
  if(captured.key!==current.target.key||captured.origin!==scope.origin||captured.environmentId!==scope.environmentId
    ||captured.threadId!==current.target.threadId||document.incarnation===captured.document.incarnation&&document.revision<captured.document.revision)return refuse();
  const existing=raw.completions[claim.transferId];
  if(existing!==undefined){try{sendCompletion(existing,claim)}catch{return refuse()}}
  if(terminal&&sendCanonical(existing)!==terminal.expectedMarker)return refuse();
  const exact=document.incarnation===captured.document.incarnation&&document.revision===captured.document.revision
    &&sendCanonical(document.selection)===sendCanonical(captured.document.selection)&&snapshot.text===captured.text
    &&snapshot.contextRevision===captured.contextRevision&&sendCanonical(snapshot.context)===sendCanonical(captured.context)
    &&sendCanonical(snapshot.images)===sendCanonical(captured.images)&&sendCanonical(snapshot.files)===sendCanonical(captured.files)
    &&sendCanonical(snapshot.attachmentIds)===sendCanonical(captured.attachmentIds)&&sendCanonical(snapshot.attachmentOrder)===sendCanonical(captured.attachmentOrder);
  const clear=!!terminal?.clear&&exact&&terminal.next.value===''&&terminal.next.selection.start===0&&terminal.next.selection.end===0;
  if(terminal?.clear&&!clear)return refuse();
  const next=terminal?.next??{value:document.value,selection:document.selection??{start:document.value.length,end:document.value.length}};
  const write=!!terminal&&(clear||next.value!==document.value||sendCanonical(next.selection)!==sendCanonical(document.selection));
  if(write&&(document.revision>=Number.MAX_SAFE_INTEGER||snapshot.contextRevision>=Number.MAX_SAFE_INTEGER))return refuse();
  const history=mobileCreateContextHistory();
  let context=snapshot.context??undefined;
  if(write){
    if(clear)context=undefined;
    else if(entry) {
      const records=histories.get(entry)?.snapshot()??[];
      history('',{version:1,records});context=history(next.value,snapshot.context??undefined);
    }else context=undefined; // Observe does not create a context row for ordinary text alone.
  }
  const projectedDocument={...document,value:next.value,selection:write?{...next.selection}:document.selection,revision:document.revision+(write?1:0)};
  const projected:ComposerSendDraftSnapshot={...snapshot,text:next.value,context:context??null,
    contextRevision:snapshot.contextRevision+(write&&entry?1:0),
    contextRow:context?{...scope,text:next.value,revision:snapshot.contextRevision+(write&&entry?1:0),context:clone(context)}:null,
    ...(clear?{images:[],files:[],attachmentIds:[],attachmentOrder:null}:{})};
  let marker:import('./thread-send-transfer-model').ThreadSendTransferCompletion;
  try {marker=sendCompletion({version:2,kind:'ordinary',draftKey:claim.draftKey,fingerprint:claim.fingerprint,
    disposition:clear?'cleared':'preserved',before:{incarnation:captured.document.incarnation,revision:captured.document.revision},
    after:sendAfter(projectedDocument,projected)},claim)}catch{return refuse()}
  const completions={...raw.completions,[claim.transferId]:marker};
  const images=clear?{...clone(local.snapshotDrafts),[current.target.key]:[]}:local.snapshotDrafts;
  const files=clear?clone(raw.raw.composerFiles as import('./shared/composer-editor-files').DraftFile[]).filter(file=>file.draftKey!==current.target.key):raw.raw.composerFiles;
  const orders=clear?{...clone(raw.raw.mobileAttachmentOrder as Record<string,string[]>),[current.target.key]:[]}:raw.raw.mobileAttachmentOrder;
  const snapshotReleases=clear?[...local.snapshotReleases,...snapshot.images.map(image=>String(image.id)).filter(id=>!local.snapshotReleases.includes(id))]:local.snapshotReleases;
  const releases=raw.cleanup.fileReleases as string[];
  const cleanup=clear?{...raw.cleanup,fileReleases:[...releases,...snapshot.files.map(file=>file.id).filter(id=>!releases.includes(id))]}:raw.cleanup;
  const result={ok:true as const,marker:clone(marker),exact,write,
    ledger:{value:next.value,selection:projectedDocument.selection?{...projectedDocument.selection}:null,revision:projectedDocument.revision,incarnation:projectedDocument.incarnation}};
  if(write&&(!terminal||!mobileEditorDocumentCommitMounted(client,current,terminal.proof,next)))return refuse();
  if(write&&entry){entry.text=next.value;entry.revision++;if(context)entry.context=context;else delete entry.context;histories.set(entry,history)}
  if(clear){local.snapshotDrafts=images;local.composerFiles=files;local.mobileAttachmentOrder=orders;
    local.snapshotReleases=snapshotReleases;local.mobileNewTaskDrafts=cleanup}
  local.mobileOutboxTransferCompletions=completions;if(!write)client.revision++;
  return result;
}

/** Internal local content clear. The owner verifies this exact native command/terminal.
 * No inventory is removed: attachment or metadata changes revoke producer-specific clearing.
 * All allocations precede the concrete mounted document write; no callback authority. */
export function mobileComposerContextMountedLocalClear(client:T3Client,current:EditorDocumentIntent,
  captured:{snapshot:ComposerSendDraftSnapshot;incarnation:string;revision:number},
  proof:Parameters<typeof mobileEditorDocumentCommitMounted>[2]) {
  const raw=sendRaw(client,current.target),document=sendDocument(client,current.key),refuse=()=>({ok:false as const});
  if(!raw||!document||document.blocked||!mobileEditorDocumentIntentCurrent(client,current)
    ||!Number.isSafeInteger(client.revision)||client.revision<0||client.revision>=Number.MAX_SAFE_INTEGER
    ||document.revision>=Number.MAX_SAFE_INTEGER||raw.snapshot.contextRevision>=Number.MAX_SAFE_INTEGER)return refuse();
  const {entry,snapshot}=raw,next={value:proof.latest.value,selection:{...proof.latest.selection}};
  const exact=document.incarnation===captured.incarnation&&document.revision===captured.revision
    &&sendCanonical(snapshot)===sendCanonical(captured.snapshot)&&!snapshot.images.length&&!snapshot.files.length;
  const cleared=exact&&proof.terminal.kind==='commandApplied'&&proof.latest.eventCount===proof.terminal.eventCount
    &&next.value===''&&next.selection.start===0&&next.selection.end===0;
  const history=mobileCreateContextHistory();
  let context=snapshot.context??undefined;
  if(entry){
    history('',{version:1,records:histories.get(entry)?.snapshot()??[]});
    // Keep history metadata available to native Undo; content clear removes the live envelope.
    const projected=history(next.value,snapshot.context??undefined);
    context=cleared?undefined:exact&&next.value!==document.value?projected:snapshot.context??undefined;
  }
  const result={ok:true as const,cleared,ledger:{value:next.value,selection:{...next.selection},
    revision:document.revision+1,incarnation:document.incarnation}};
  if(!mobileEditorDocumentCommitMounted(client,current,proof,next))return refuse();
  if(entry){entry.text=next.value;entry.revision++;if(context)entry.context=context;else delete entry.context;histories.set(entry,history)}
  return result;
}

/** Source-shaped file context for legacy saved picker rows. This creates metadata only from
 * actual canonical attached rows and their current token; native holds remain separate authority. */
function mountedContext(snapshot:ComposerSendDraftSnapshot):MobileMessageContext|undefined {
  const refs=new Set(collectComposerContextReferences(snapshot.text).filter(r=>r.kind==='file').map(r=>r.contextId));
  const derived=fileContextRecords(snapshot.files.filter(file=>file.source==='attached'&&refs.has(file.contextId)).map(file=>({...file,attachmentId:file.id})));
  const records=new Map(derived.map(record=>[str(record.contextId),record]));
  for(const record of snapshot.context?.records??[])records.set(str(record.contextId),record);
  return records.size?{version:1,records:[...records.values()]}:snapshot.context??undefined;
}
/** Read-only display seam; an invalid complete raw store still refuses before derivation. */
export function mobileComposerContextMountedRead(client:T3Client,target:MobileComposerTarget) {
  const raw=sendRaw(client,target);return raw?{ok:true as const,context:mountedContext(raw.snapshot)}:{ok:false as const};
}
/** Real mounted observation publication; caller proves current editor event and IO-usable holds.
 * Context, restored inventory, byte-release candidates and both histories are prepared together.
 * No incoming bytes, unmounted producer or synthetic native receipt is admitted here. */
export function mobileComposerContextPublishMounted(client:T3Client,current:EditorDocumentIntent,event:ComposerEditorEvent,
  files:ComposerFileHistory,holds:readonly HeldFile[],addedRecords:readonly Obj[]) {
  const raw=sendRaw(client,current.target),refuse=(message:string)=>({ok:false as const,message});
  if(!raw||!mobileEditorDocumentIntentCurrent(client,current)||!Number.isSafeInteger(client.revision)
    ||client.revision<0||client.revision>=Number.MAX_SAFE_INTEGER||current.revision>=Number.MAX_SAFE_INTEGER
    ||raw.snapshot.contextRevision>=Number.MAX_SAFE_INTEGER)return refuse('The saved draft is unavailable. Keep its files.');
  const added=batchContext({version:1,records:addedRecords},true);if(!added.ok)return refuse('The added context is invalid.');
  const {snapshot,entry}=raw,history=mobileCreateContextHistory(),beforeContext=mountedContext(snapshot);
  const prior=entry?histories.get(entry)?.snapshot()??[]:[];
  // The actual source helper refreshes duplicate recency with delete+set and bounds before restore.
  history('',{version:1,records:prior});
  const records=new Map((beforeContext?.records??[]).map(record=>[str(record.contextId),record]));
  for(const record of added.context!.records){records.delete(str(record.contextId));records.set(str(record.contextId),record)}
  const currentContext=beforeContext!==undefined||addedRecords.length?{version:1 as const,records:[...records.values()]}:undefined;
  const restored=entry||beforeContext!==undefined||addedRecords.length?history(event.value,currentContext):undefined,checked=batchContext(restored,true);
  if(!checked.ok)return refuse('The restored context exceeds the supported draft.');
  // Every accepted import remains in the detached history even if newer native text removed it.
  const target={environmentId:current.target.environmentId,threadId:current.target.threadId,draftKey:current.target.key};
  const read=mobileComposerAttachmentInventoryRead(raw.raw,target);if(!read.ok)return refuse('The saved file inventory is invalid.');
  const projected=mobileComposerFileHistoryProject(files,{identity:files.identity,target:files.target,context:checked.context,
    attachments:read.attachments.map(a=>a.type==='image'?{type:'image' as const,id:a.id}:{type:'file' as const,file:a.file,
      hold:holds.find(h=>h.receipt.id===a.id&&h.receipt.sizeBytes===a.file.sizeBytes)??null}),usableHolds:holds});
  if(projected.status!=='ready')return refuse('Protect the saved files before editing this draft.');
  if(projected.unavailable.length)return refuse('A file referenced by Undo is no longer available. Keep this draft.');
  const inventory={...raw.raw,composerFiles:[...clone(raw.raw.composerFiles as import('./shared/composer-editor-files').DraftFile[]),...projected.restoredFiles],
    mobileAttachmentOrder:{...clone(raw.raw.mobileAttachmentOrder as Record<string,string[]>),[current.target.key]:[...snapshot.attachmentIds,...projected.restoredFiles.map(f=>f.id)]}};
  const publication=mobileComposerAttachmentPublicationPrepare({inventory,target,previousContext:beforeContext,
    nextContext:checked.context,nextText:event.value});
  if(!publication.ok)return refuse('This file context cannot be published safely.');
  if(files.closed||files.revision!==projected.expectedRevision)return refuse('The file history changed.');
  const context=checked.context,needsEntry=!!entry||context!==undefined||records.size>0;
  if(!entry&&needsEntry&&incarnation>=Number.MAX_SAFE_INTEGER)return refuse('Reopen this draft before editing.');
  const nextEntry:Entry=entry??{...raw.scope,incarnation:incarnation+1,revision:0,text:snapshot.text};
  const cleanup={...raw.cleanup,fileReleases:publication.fileReleases};
  const result={ok:true as const,history:projected.history,release:projected.release,
    ledger:{value:event.value,selection:{...event.selection},revision:current.revision+1,incarnation:current.incarnation}};
  if(!mobileEditorDocumentCommitObservation(client,current,event))return refuse('The native document changed.');
  // Every object and possible refusal above precedes the concrete document write.
  if(needsEntry){nextEntry.text=event.value;nextEntry.revision++;if(context)nextEntry.context=context;else delete nextEntry.context;
    if(!entry){incarnation++;raw.registry.entries.set(raw.id,nextEntry)}histories.set(nextEntry,history)}
  raw.local.snapshotDrafts=publication.snapshotDrafts;raw.local.composerFiles=publication.composerFiles;
  raw.local.mobileAttachmentOrder=publication.mobileAttachmentOrder;raw.local.snapshotReleases=publication.snapshotReleases;raw.local.mobileNewTaskDrafts=cleanup;
  return result;
}
