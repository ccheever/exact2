// Source365aa87982 use-composer-drafts.ts withReferencedContextFiles; app inventory adaptation.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { DraftFile } from './shared/composer-editor-files';
import type { Obj } from './shared/domain';
import { contextReferences } from './shared/composer-editor-menu';
import { collectComposerContextReferences, imageMimeType } from './composer-editor-document';
import { mobileContextRecordValid } from './mobile-context-record';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import type { MobileMessageContext } from './mobile-new-task-context';

export interface OrdinaryAttachmentTarget { readonly environmentId:string; readonly threadId:string; readonly draftKey:string }
/** Complete raw inventories, not prefiltered named rows. Caller validates the owning metadata store
 * before extracting fileReleases; an invalid store must not be converted to an empty queue. */
export interface OrdinaryAttachmentInventory {
  readonly snapshotDrafts:unknown; readonly composerFiles:unknown; readonly mobileAttachmentOrder:unknown;
  readonly snapshotReleases:unknown; readonly fileReleases:unknown;
}
export type OrdinaryInventoryAttachment = {type:'image';id:string;image:Obj} | {type:'file';id:string;file:DraftFile};
export type OrdinaryAttachmentRefusal = {ok:false;reason:'invalid-target'|'invalid-inventory'|'invalid-context'|'unsupported'};
export interface OrdinaryAttachmentPublicationInput {
  readonly inventory:OrdinaryAttachmentInventory; readonly target:OrdinaryAttachmentTarget;
  readonly previousContext:unknown; readonly nextContext:unknown; readonly nextText:string;
  /** Exact metadata from the current source context history; permits no byte restoration. */
  readonly unavailableImages?:readonly Obj[];
}
export interface OrdinaryAttachmentPublicationReady {
  ok:true; snapshotDrafts:Record<string,Obj[]>; composerFiles:DraftFile[]; mobileAttachmentOrder:Record<string,string[]>;
  snapshotReleases:string[]; fileReleases:string[]; removedFileIds:string[];
}
type Inventory = Omit<OrdinaryAttachmentPublicationReady,'ok'|'removedFileIds'>;
const plain=(v:unknown):v is Record<string,unknown>=>v!==null&&typeof v==='object'&&!Array.isArray(v)
  &&[Object.prototype,null].includes(Object.getPrototypeOf(v));
const text=(v:unknown):v is string=>typeof v==='string'&&v.length>0;
const integer=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0;
const uuid=(v:string)=>/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
const copy=<T>(v:T):T=>JSON.parse(JSON.stringify(v)) as T;
const refused=(reason:OrdinaryAttachmentRefusal['reason']):OrdinaryAttachmentRefusal=>({ok:false,reason});
/** Validate JSON without normalizing away unsupported payloads, cycles or sparse array entries. */
function json(v:unknown,parents=new Set<object>()):boolean {
  if(v===null||typeof v==='string'||typeof v==='boolean')return true;
  if(typeof v==='number')return Number.isFinite(v);
  if(!Array.isArray(v)&&!plain(v)||parents.has(v as object))return false;
  parents.add(v as object);
  const entries=Object.values(v as object),valid=(!Array.isArray(v)||Object.keys(v).length===v.length
    &&Array.from({length:v.length},(_,i)=>Object.hasOwn(v,i)).every(Boolean))&&entries.every(x=>json(x,parents));
  parents.delete(v as object);return valid;
}
function targetValid(t:OrdinaryAttachmentTarget):boolean {
  return plain(t)&&text(t.environmentId)&&text(t.threadId)&&!t.threadId.startsWith('new:')
    &&t.draftKey===`${t.environmentId}:${t.threadId}`&&!t.draftKey.includes('~queued-edit~');
}
function strings(v:unknown):v is string[]{return Array.isArray(v)&&v.every(text)}
function image(v:unknown):boolean {
  return plain(v)&&['id','name','mimeType'].every(k=>text(v[k]))&&integer(v.sizeBytes)
    &&(!Object.hasOwn(v,'uploadId')||typeof v.uploadId==='string');
}
function file(v:unknown):boolean {
  return plain(v)&&['id','contextId','draftKey','environmentId','name','mimeType','source'].every(k=>text(v[k]))
    &&integer(v.sizeBytes)&&typeof v.attachmentId==='string'&&typeof v.status==='string'&&['staged','ready'].includes(v.status)
    &&(v.status!=='ready'||text(v.attachmentId))
    &&['videoWidth','videoHeight'].every(k=>!Object.hasOwn(v,k)||typeof v[k]==='number'&&Number.isFinite(v[k])&&Number(v[k])>0);
}
function inventoryRead(raw:OrdinaryAttachmentInventory):Inventory|null {
  if(!plain(raw))return null;
  const files=raw.composerFiles===undefined?[]:raw.composerFiles,orders=raw.mobileAttachmentOrder===undefined?{}:raw.mobileAttachmentOrder;
  const value={snapshotDrafts:raw.snapshotDrafts,composerFiles:files,mobileAttachmentOrder:orders,
    snapshotReleases:raw.snapshotReleases,fileReleases:raw.fileReleases};
  if(!json(value)||!plain(value.snapshotDrafts)||!Array.isArray(files)||!plain(orders)
    ||!strings(value.snapshotReleases)||!strings(value.fileReleases))return null;
  const ids=new Map<string,Set<string>>(),contexts=new Map<string,Set<string>>();
  const unique=(key:string,id:string)=>{let set=ids.get(key);if(!set){set=new Set();ids.set(key,set)}if(set.has(id))return false;set.add(id);return true};
  for(const [key,rows]of Object.entries(value.snapshotDrafts)) {
    if(!text(key)||!Array.isArray(rows))return null;
    for(const row of rows)if(!image(row)||!unique(key,String((row as Obj).id)))return null;
  }
  for(const row of files) {
    if(!file(row))return null;
    const f=row as DraftFile;if(!unique(f.draftKey,f.id))return null;
    let set=contexts.get(f.draftKey);if(!set){set=new Set();contexts.set(f.draftKey,set)}
    if(set.has(f.contextId))return null;set.add(f.contextId);
  }
  for(const [key,order]of Object.entries(orders))if(!text(key)||!strings(order)||new Set(order).size!==order.length)return null;
  return copy(value) as Inventory;
}
function mixed(inventory:Inventory,target:OrdinaryAttachmentTarget):OrdinaryInventoryAttachment[] {
  const images=inventory.snapshotDrafts[target.draftKey]??[],files=inventory.composerFiles.filter(f=>f.draftKey===target.draftKey);
  const attachments:OrdinaryInventoryAttachment[]=[...images.map(image=>({type:'image' as const,id:String(image.id),image})),
    ...files.map(file=>({type:'file' as const,id:file.id,file}))];
  const byId=new Map(attachments.map(a=>[a.id,a])),ids=[...new Set([...(inventory.mobileAttachmentOrder[target.draftKey]??[]),...byId.keys()])];
  return ids.flatMap(id=>{const a=byId.get(id);return a?[a]:[]});
}
export function mobileComposerAttachmentInventoryRead(raw:OrdinaryAttachmentInventory,target:OrdinaryAttachmentTarget):
  {ok:true;attachments:OrdinaryInventoryAttachment[]}|OrdinaryAttachmentRefusal {
  if(!targetValid(target))return refused('invalid-target');
  const inventory=inventoryRead(raw);if(!inventory)return refused('invalid-inventory');
  if(inventory.composerFiles.some(f=>f.draftKey===target.draftKey&&f.environmentId!==target.environmentId))return refused('invalid-inventory');
  return {ok:true,attachments:mixed(inventory,target)};
}
function context(v:unknown,limit=false):v is MobileMessageContext|undefined {
  if(v===undefined)return true;
  if(!plain(v)||!json(v)||v.version!==1||!Array.isArray(v.records)||limit&&v.records.length>200)return false;
  const seen=new Set<string>();
  return v.records.every(r=>{if(!plain(r)||!mobileContextRecordValid(r as Obj)||seen.has(String(r.contextId)))return false;seen.add(String(r.contextId));return true});
}
function compatible(text:string,next:MobileMessageContext|undefined,attachments:OrdinaryInventoryAttachment[],unavailableImages:readonly Obj[]=[]):boolean {
  const files=attachments.flatMap(a=>a.type==='file'?[a.file]:[]),refs=contextReferences(text).filter(r=>r.kind==='file');
  const sourceIds=new Set(collectComposerContextReferences(text).filter(r=>r.kind==='file').map(r=>r.contextId));
  if(sourceIds.size!==refs.length||refs.some(r=>!sourceIds.has(r.id)))return false;
  for(const ref of refs) {
    const file=files.find(f=>f.contextId===ref.id);if(!file){if(!next?.records.some(r=>r.contextId===ref.id))continue;return false}
    const record=next?.records.find(r=>r.contextId===ref.id);
    if(record&&(record.kind!=='file'||!('attachmentId'in record)||record.attachmentId!==file.id))return false;
  }
  for(const record of next?.records??[])if('attachmentId'in record) {
    const attachment=attachments.find(a=>a.id===record.attachmentId);
    if(!attachment){if(record.kind==='image'&&unavailableImages.some(saved=>canonical(saved)===canonical(record)))continue;return false}
    if(record.kind==='file'&&(attachment.type!=='file'||attachment.file.contextId!==record.contextId||!sourceIds.has(String(record.contextId))))return false;
    if(record.kind==='image'&&attachment.type!=='image'&&imageMimeType(attachment.file)===null)return false;
  }
  return true;
}
/** Pure latest-state preparation. No historical restoration, native Undo, byte authority or IO.
 * Caller prepares future context first and installs these detached replacements only with the same
 * synchronous named document/context acceptance. Persist cleanup obligations before native deletion. */
export function mobileComposerAttachmentPublicationPrepare(input:OrdinaryAttachmentPublicationInput):
  OrdinaryAttachmentPublicationReady|OrdinaryAttachmentRefusal {
  const {inventory:raw,target,previousContext:before,nextContext:after,nextText}=input;
  const read=mobileComposerAttachmentInventoryRead(raw,target);if(!read.ok)return read;
  if(!context(before)||!context(after,true)||typeof nextText!=='string'||nextText.length>1_000_000)return refused('invalid-context');
  const previous=new Set(before?.records.flatMap(r=>'attachmentId'in r?[r.attachmentId]:[])??[]);
  const retained=new Set(after?.records.flatMap(r=>'attachmentId'in r?[r.attachmentId]:[])??[]);
  const removed=read.attachments.filter((a):a is Extract<OrdinaryInventoryAttachment,{type:'file'}>=>a.type==='file'&&previous.has(a.id)&&!retained.has(a.id));
  if(removed.some(a=>a.file.source!=='attached'||!uuid(a.id)))return refused('unsupported');
  const removedIds=new Set(removed.map(a=>a.id)),attachments=read.attachments.filter(a=>!removedIds.has(a.id));
  if(input.unavailableImages!==undefined&&(!Array.isArray(input.unavailableImages)||!context({version:1,records:input.unavailableImages})
    ||input.unavailableImages.some(r=>r.kind!=='image')))return refused('invalid-context');
  if(!compatible(nextText,after,attachments,input.unavailableImages))return refused('unsupported');
  const prepared=inventoryRead(raw)!;
  prepared.composerFiles=prepared.composerFiles.filter(f=>f.draftKey!==target.draftKey||!removedIds.has(f.id));
  prepared.mobileAttachmentOrder[target.draftKey]=attachments.map(a=>a.id);
  prepared.fileReleases=[...prepared.fileReleases,...[...removedIds].filter(id=>!prepared.fileReleases.includes(id))];
  return {ok:true,...prepared,removedFileIds:[...removedIds]};
}

/** Detached replacement for a captured native command. Its caller must authenticate incoming
 * rows through issued intake before dispatch. Matching JSON alone never grants byte ownership. */
export function mobileComposerAttachmentInventoryReplace(raw:OrdinaryAttachmentInventory,target:OrdinaryAttachmentTarget,
  expected:readonly OrdinaryInventoryAttachment[],next:readonly OrdinaryInventoryAttachment[]):
  {ok:true;inventory:OrdinaryAttachmentInventory}|OrdinaryAttachmentRefusal {
  const read=mobileComposerAttachmentInventoryRead(raw,target);if(!read.ok)return read;
  if(!json(expected)||!json(next)||canonical(read.attachments)!==canonical(expected))return refused('invalid-inventory');
  const prepared=inventoryRead(raw)!;
  if(next.some(a=>!plain(a)||!text(a.id)||(a.type==='image'?a.id!==a.image?.id:
    a.type!=='file'||a.id!==a.file?.id||a.file.draftKey!==target.draftKey||a.file.environmentId!==target.environmentId)))return refused('invalid-inventory');
  prepared.snapshotDrafts[target.draftKey]=next.flatMap(a=>a.type==='image'?[copy(a.image)]:[]);
  prepared.composerFiles=[...prepared.composerFiles.filter(f=>f.draftKey!==target.draftKey),...next.flatMap(a=>a.type==='file'?[copy(a.file)]:[])];
  prepared.mobileAttachmentOrder[target.draftKey]=next.map(a=>a.id);
  const removed=read.attachments.filter(a=>!next.some(n=>n.id===a.id));
  if(removed.some(a=>!uuid(a.id)||a.type==='file'&&a.file.source!=='attached'))return refused('unsupported');
  for(const a of removed){const queue=a.type==='image'?prepared.snapshotReleases:prepared.fileReleases;if(!queue.includes(a.id))queue.push(a.id)}
  const validated=mobileComposerAttachmentInventoryRead(prepared,target);if(!validated.ok)return validated;
  return {ok:true,inventory:prepared};
}
