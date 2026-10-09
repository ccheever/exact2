// Ordinary Thread context ownership, adapted from T3 Code (MIT); see LICENSE-T3.
// Source365aa87982 composerContext.ts/use-composer-drafts.ts; mobile-only persistence seam.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { draftFiles } from './shared/composer-editor-files';
import { ClientError } from './shared/protocol';
import type { MobileComposerTarget } from './composer-target';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileContextRecordValid } from './mobile-context-record';
import { mobileCreateContextHistory, mobileNewTaskContextProject, mobileReferencedComposerContext, type MobileMessageContext } from './mobile-new-task-context';

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
