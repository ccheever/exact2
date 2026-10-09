// File half of T3 Code365aa87982 createComposerDraftContextHistory (MIT, LICENSE-T3).
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { DraftFile } from './shared/composer-editor-files';
import type { ComposerMountedIdentity } from './composer-editor-state';
import type { FileHoldTarget, HeldFile } from './composer-file-holds-io';
import type { MobileMessageContext } from './mobile-new-task-context';
import { mobileContextRecordValid } from './mobile-context-record';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import type { Obj } from './shared/domain';

export interface ComposerFileHistoryEntry { readonly file:DraftFile; readonly hold:HeldFile }
export interface ComposerFileHistory {
  readonly identity:ComposerMountedIdentity; readonly target:FileHoldTarget;
  readonly revision:number; readonly closed:boolean; readonly entries:readonly ComposerFileHistoryEntry[];
}
export type ComposerFileHistoryAttachment = {readonly type:'image';readonly id:string}
  | {readonly type:'file';readonly file:DraftFile;readonly hold:HeldFile|null};
export interface ComposerFileHistoryInput {
  readonly identity:ComposerMountedIdentity; readonly target:FileHoldTarget;
  readonly attachments:readonly ComposerFileHistoryAttachment[];
  readonly context:MobileMessageContext|undefined;
  /** Current IO-usable bindings, including historical entries. Shape cannot authenticate native authority. */
  readonly usableHolds:readonly HeldFile[];
}
export type ComposerFileHistoryProjection = {status:'ready';expectedRevision:number;history:ComposerFileHistory;
  restoredFiles:DraftFile[];release:HeldFile[];unavailable:string[]}
  | {status:'needs-holds';files:DraftFile[]}
  | {status:'refused';reason:'owner'|'invalid'|'unsupported'|'limit'|'unusable-hold'};
export type ComposerFileHistoryDisposal = {status:'ready';history:ComposerFileHistory;release:HeldFile[]}
  | {status:'refused';reason:'invalid'};
const MAX=Number.MAX_SAFE_INTEGER;
const plain=(value:unknown):value is Record<string,unknown>=>value!==null&&typeof value==='object'&&!Array.isArray(value)
  &&[Object.prototype,null].includes(Object.getPrototypeOf(value));
const text=(value:unknown):value is string=>typeof value==='string'&&value.length>0;
const integer=(value:unknown):value is number=>typeof value==='number'&&Number.isSafeInteger(value)&&value>=0;
const uuid=(value:unknown):value is string=>typeof value==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value);
const keys=(value:Record<string,unknown>,fields:readonly string[])=>Object.keys(value).length===fields.length&&fields.every(key=>Object.hasOwn(value,key));
const copy=<T>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
const equal=(a:unknown,b:unknown)=>canonical(a)===canonical(b);
/** Reject non-JSON/cyclic raw state rather than cloning away invalid unused data. */
function json(value:unknown,parents=new Set<object>()):boolean {
  if(value===null||typeof value==='string'||typeof value==='boolean')return true;
  if(typeof value==='number')return Number.isFinite(value);
  if(!Array.isArray(value)&&!plain(value)||parents.has(value as object))return false;
  parents.add(value as object);
  const valid=Object.values(value as object).every(child=>json(child,parents));parents.delete(value as object);return valid;
}
const identityFields=['owner','editorId','routeVisit','renderEpoch','mountId'];
function identity(value:unknown):value is ComposerMountedIdentity {
  return plain(value)&&keys(value,identityFields)&&identityFields.every(key=>text(value[key]));
}
function scope(i:unknown,t:unknown):boolean {
  if(!identity(i)||!plain(t)||!keys(t,['origin','environmentId','threadId','draftKey'])||!Object.values(t).every(text)
    ||String(t.threadId).startsWith('new:')||String(t.draftKey).includes('~queued-edit~')||t.draftKey!==`${t.environmentId}:${t.threadId}`)return false;
  try {const owner:unknown=JSON.parse(i.owner);return Array.isArray(owner)&&(owner.length===5||owner.length===6)
    &&owner[0]==='ordinary'&&owner[1]===t.origin&&owner[2]===t.environmentId&&integer(owner[3])&&owner[4]===t.draftKey}
  catch {return false}
}
function file(value:unknown,target:FileHoldTarget):value is DraftFile {
  if(!plain(value)||!uuid(value.id)||!integer(value.sizeBytes)||value.sizeBytes>50*1024*1024
    ||!['contextId','draftKey','environmentId','name','mimeType','source'].every(key=>text(value[key]))
    ||value.draftKey!==target.draftKey||value.environmentId!==target.environmentId||typeof value.attachmentId!=='string'
    ||typeof value.status!=='string'||!['staged','ready'].includes(value.status))return false;
  const fields=['id','contextId','draftKey','environmentId','name','mimeType','source','sizeBytes','attachmentId','status','videoWidth','videoHeight'];
  return Object.keys(value).every(key=>fields.includes(key))&&['videoWidth','videoHeight'].every(key=>!Object.hasOwn(value,key)
    ||typeof value[key]==='number'&&Number.isFinite(value[key])&&Number(value[key])>0);
}
function held(value:unknown,i:ComposerMountedIdentity,t:FileHoldTarget):value is HeldFile {
  if(!plain(value)||!keys(value,['request','receipt'])||!plain(value.request)||!plain(value.receipt))return false;
  const q=value.request,r=value.receipt;
  return keys(q,['op','action','generation','identity','requestId','target','file'])&&q.op==='composerFileHold'&&q.action==='acquire'
    &&integer(q.generation)&&q.generation===(JSON.parse(i.owner) as unknown[])[3]&&uuid(q.requestId)
    &&equal(q.identity,i)&&equal(q.target,t)&&file(q.file,t)&&q.file.source==='attached'
    &&keys(r,['identity','requestId','holdId','fileIdentity','id','sizeBytes'])&&equal(r.identity,i)
    &&r.requestId===q.requestId&&uuid(r.holdId)&&uuid(r.fileIdentity)&&r.id===q.file.id&&r.sizeBytes===q.file.sizeBytes;
}
function historyValid(value:unknown):value is ComposerFileHistory {
  if(!plain(value)||!json(value)||!keys(value,['identity','target','revision','closed','entries'])||!scope(value.identity,value.target)
    ||!integer(value.revision)||typeof value.closed!=='boolean'||!Array.isArray(value.entries)||value.closed&&value.entries.length>0)return false;
  const ids=new Set<string>(),requests=new Set<string>();
  for(const entry of value.entries) {
    if(!plain(entry)||!keys(entry,['file','hold'])||!file(entry.file,value.target as unknown as FileHoldTarget)||entry.file.source!=='attached'
      ||!held(entry.hold,value.identity as ComposerMountedIdentity,value.target as unknown as FileHoldTarget)
      ||entry.hold.receipt.id!==entry.file.id||entry.hold.receipt.sizeBytes!==entry.file.sizeBytes||ids.has(entry.file.id)
      ||requests.has(entry.hold.request.requestId))return false;
    ids.add(entry.file.id);requests.add(entry.hold.request.requestId);
  }
  return true;
}
function contextValid(value:unknown):boolean {
  if(value===undefined)return true;
  if(!plain(value)||!json(value)||value.version!==1||!Array.isArray(value.records))return false;
  const ids=new Set<string>();
  for(const record of value.records) {
    if(!plain(record)||!mobileContextRecordValid(record as Obj)||ids.has(String(record.contextId)))return false;
    ids.add(String(record.contextId));
  }
  return true;
}
/** Inputs are JSON-normalized rows (omit absent optional fields). Caller supplies IO-usable holds and
 * already-restored context, and rechecks current mount/catalog/target.
 * Install only while !current.closed and identity/target/expectedRevision still match. Release obligations
 * run AFTER accepted publication, never while merely computing this detached candidate. No byte IO occurs. */
export function mobileComposerFileHistoryProject(history:ComposerFileHistory,input:ComposerFileHistoryInput):ComposerFileHistoryProjection {
  if(!historyValid(history)||!plain(input)||!keys(input,['identity','target','attachments','context','usableHolds'])||!scope(input.identity,input.target)||!Array.isArray(input.attachments)
    ||!json(input.attachments)||!Array.isArray(input.usableHolds)||!json(input.usableHolds)||!contextValid(input.context))return {status:'refused',reason:'invalid'};
  if(history.closed||!equal(history.identity,input.identity)||!equal(history.target,input.target))return {status:'refused',reason:'owner'};
  if(history.revision>=MAX)return {status:'refused',reason:'limit'};
  const usable=new Map<string,HeldFile>();
  for(const binding of input.usableHolds) {
    if(!held(binding,history.identity,history.target)||usable.has(binding.request.requestId))return {status:'refused',reason:'invalid'};
    usable.set(binding.request.requestId,binding);
  }
  const isUsable=(binding:HeldFile)=>equal(usable.get(binding.request.requestId),binding);
  if(history.entries.some(entry=>!isUsable(entry.hold)))return {status:'refused',reason:'unusable-hold'};
  const liveIds=new Set<string>(),missing:DraftFile[]=[];
  for(const attachment of input.attachments) {
    if(!plain(attachment))return {status:'refused',reason:'invalid'};
    if(attachment.type==='image') {
      if(!keys(attachment,['type','id'])||!text(attachment.id)||liveIds.has(attachment.id))return {status:'refused',reason:'invalid'};
      liveIds.add(attachment.id);continue;
    }
    if(attachment.type!=='file'||!keys(attachment,['type','file','hold'])||!file(attachment.file,history.target)
      ||liveIds.has(attachment.file.id))return {status:'refused',reason:'invalid'};
    if(attachment.file.source!=='attached')return {status:'refused',reason:'unsupported'};
    liveIds.add(attachment.file.id);
    if(attachment.hold===null){missing.push(attachment.file);continue}
    if(!held(attachment.hold,history.identity,history.target)||attachment.hold.receipt.id!==attachment.file.id
      ||attachment.hold.receipt.sizeBytes!==attachment.file.sizeBytes)return {status:'refused',reason:'invalid'};
    if(!isUsable(attachment.hold))return {status:'refused',reason:'unusable-hold'};
  }
  if(missing.length)return {status:'needs-holds',files:copy(missing)};
  const files=new Map(history.entries.map(entry=>[entry.file.id,entry])),release=new Map<string,HeldFile>();
  for(const attachment of input.attachments) {
    if(attachment.type!=='file')continue;
    const previous=files.get(attachment.file.id);files.delete(attachment.file.id);
    if(previous&&!equal(previous.hold,attachment.hold))release.set(previous.hold.request.requestId,previous.hold);
    files.set(attachment.file.id,{file:attachment.file,hold:attachment.hold!});
  }
  const limit=Math.max(200,input.attachments.length);
  while(files.size>limit) {
    const oldest=files.keys().next().value!,entry=files.get(oldest)!;
    release.set(entry.hold.request.requestId,entry.hold);files.delete(oldest);
  }
  const restoredFiles:DraftFile[]=[],unavailable:string[]=[];
  for(const record of input.context?.records??[]) {
    if(record.kind!=='file'||!('attachmentId' in record)||liveIds.has(String(record.attachmentId)))continue;
    const id=String(record.attachmentId),saved=files.get(id)?.file;liveIds.add(id);
    if(saved)restoredFiles.push({...saved,attachmentId:'',status:'staged'});else unavailable.push(id);
  }
  return copy({status:'ready',expectedRevision:history.revision,history:{...history,revision:history.revision+1,entries:[...files.values()]},
    restoredFiles,release:[...release.values()],unavailable});
}
/** Only returns release obligations. Native owner retirement/guarded cleanup owns real byte disposal. */
export function mobileComposerFileHistoryDispose(history:ComposerFileHistory):ComposerFileHistoryDisposal {
  if(!historyValid(history))return {status:'refused',reason:'invalid'};
  if(history.closed)return {status:'ready',history:copy(history),release:[]};
  return copy({status:'ready',history:{...history,closed:true,revision:Math.min(MAX,history.revision+1),entries:[]},release:history.entries.map(entry=>entry.hold)});
}
