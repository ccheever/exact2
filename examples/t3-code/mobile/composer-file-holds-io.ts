// Invocation-owned adapter for saved ordinary canonical file holds. No runtime activation.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { ComposerMountedIdentity } from './composer-editor-state';
import type { DraftFile } from './shared/composer-editor-files';
import { ClientError, reply, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

export interface FileHoldTarget { origin:string; environmentId:string; threadId:string; draftKey:string }
export interface FileHoldRequest {
  op:'composerFileHold'; action:'acquire'; generation:number; identity:ComposerMountedIdentity;
  requestId:string; target:FileHoldTarget; file:DraftFile;
}
export interface FileHoldReceipt {
  identity:ComposerMountedIdentity; requestId:string; holdId:string; fileIdentity:string; id:string; sizeBytes:number;
}
export interface HeldFile { request:FileHoldRequest; receipt:FileHoldReceipt }
export type FileHoldPhase='reserved'|'acquiring'|'acquire-uncertain'|'held'|'release-needed'|'releasing'|'release-uncertain'|'released';
export interface FileHoldEntry {
  request:FileHoldRequest; receipt:FileHoldReceipt|null; phase:FileHoldPhase;
  desired:'retain'|'release'; dispatched:boolean; attempt:number; error:string;
}
/** Plain volatile owner state. Never hydrate this ledger or its receipts as native authority. */
export interface FileHoldLedger {
  identity:ComposerMountedIdentity; generation:number; target:FileHoldTarget;
  retired:boolean; revision:number; serial:number; entries:Record<string,FileHoldEntry>;
}
export interface FileHoldResult { phase:FileHoldPhase; revision:number; held:HeldFile|null; message:string }
const MAX=Number.MAX_SAFE_INTEGER,LIMIT=4096;
const integer=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0;
const text=(v:unknown):v is string=>typeof v==='string'&&v.length>0;
const uuid=(v:unknown):v is string=>typeof v==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v);
const plain=(v:unknown):v is Record<string,unknown>=>v!==null&&typeof v==='object'&&!Array.isArray(v)
  &&[Object.prototype,null].includes(Object.getPrototypeOf(v));
const fail=(message:string)=>new ClientError(message,'FileHold');
const keys=(v:Record<string,unknown>,names:readonly string[])=>Object.keys(v).length===names.length&&names.every(k=>Object.hasOwn(v,k));
const identityKeys=['owner','editorId','routeVisit','renderEpoch','mountId'];
function identity(v:unknown):v is ComposerMountedIdentity {return plain(v)&&keys(v,identityKeys)&&identityKeys.every(k=>text(v[k]))}
function equalIdentity(a:ComposerMountedIdentity,b:ComposerMountedIdentity):boolean {return identityKeys.every(k=>a[k as keyof ComposerMountedIdentity]===b[k as keyof ComposerMountedIdentity])}
function copy<T>(value:T):T {
  const cloned=JSON.parse(JSON.stringify(value)) as T;
  const freeze=(v:unknown):void=>{if(v&&typeof v==='object'){Object.values(v).forEach(freeze);Object.freeze(v)}};
  freeze(cloned);return cloned;
}
function canonical(value:unknown):string {
  if(Array.isArray(value))return '['+value.map(canonical).join(',')+']';
  if(plain(value))return '{'+Object.keys(value).sort().map(k=>JSON.stringify(k)+':'+canonical(value[k])).join(',')+'}';
  return JSON.stringify(value)??'';
}
function fileValid(value:unknown):value is DraftFile {
  if(!plain(value)||!uuid(value.id)||!integer(value.sizeBytes)||value.sizeBytes>50*1024*1024||value.source!=='attached'
    ||!['contextId','draftKey','environmentId','name','mimeType'].every(k=>text(value[k]))
    ||typeof value.attachmentId!=='string'||typeof value.status!=='string'||!['staged','ready'].includes(value.status))return false;
  const allowed=['id','contextId','draftKey','environmentId','name','mimeType','sizeBytes','source','attachmentId','status','videoWidth','videoHeight'];
  if(Object.keys(value).some(k=>!allowed.includes(k)))return false;
  return ['videoWidth','videoHeight'].every(k=>value[k]===undefined||typeof value[k]==='number'&&Number.isFinite(value[k])&&Number(value[k])>0);
}
function scopeValid(i:unknown,generation:unknown,target:unknown):boolean {
  if(!identity(i)||!integer(generation)||!plain(target)||!keys(target,['origin','environmentId','threadId','draftKey'])
    ||!Object.values(target).every(text)||String(target.threadId).startsWith('new:')||String(target.draftKey).includes('~queued-edit~')
    ||target.draftKey!==`${target.environmentId}:${target.threadId}`)return false;
  try {const owner:unknown=JSON.parse(i.owner);return Array.isArray(owner)&&(owner.length===5||owner.length===6)
    &&owner[0]==='ordinary'&&owner[1]===target.origin&&owner[2]===target.environmentId&&owner[3]===generation&&owner[4]===target.draftKey;
  }catch{return false}
}
const releasing=(ledger:FileHoldLedger,e:FileHoldEntry)=>ledger.retired||e.desired==='release';
function changed(ledger:FileHoldLedger):void {
  // Cleanup still invalidates usability at counter exhaustion; never wrap an observable revision.
  if(ledger.revision<MAX)ledger.revision++;
}
function entry(ledger:FileHoldLedger,id:string):FileHoldEntry {
  const found=ledger.entries[id];if(!uuid(id)||!found)throw fail('That file hold request is unavailable.');return found;
}
function begin(ledger:FileHoldLedger,e:FileHoldEntry,phase:FileHoldPhase):number {
  if(!integer(ledger.serial)||ledger.serial>=MAX||!integer(ledger.revision)||ledger.revision>=MAX)throw fail('This file hold owner has exhausted its request counter.');
  e.attempt=++ledger.serial;e.phase=phase;e.dispatched=true;e.error='';changed(ledger);return e.attempt;
}
function result(ledger:FileHoldLedger,e:FileHoldEntry):FileHoldResult {
  return {phase:e.phase,revision:ledger.revision,held:mobileFileHoldHeld(ledger,e.request.requestId),message:e.error};
}
function envelope(raw:unknown,generation:number):unknown {
  const decoded=reply(raw);
  if(!integer(decoded.generation)||decoded.generation!==generation)throw fail('The native file hold generation does not match.');
  if(!decoded.ok)throw new ClientError(decoded.error!.message,decoded.error!.kind,decoded.error!.uncertain);
  return decoded.value;
}
function receipt(raw:unknown,request:FileHoldRequest):FileHoldReceipt {
  if(!plain(raw)||!keys(raw,['status','receipt'])||raw.status!=='held'||!plain(raw.receipt))throw fail('The native file hold receipt is invalid.');
  const r=raw.receipt;
  if(!keys(r,['identity','requestId','holdId','fileIdentity','id','sizeBytes'])||!identity(r.identity)||!equalIdentity(r.identity,request.identity)
    ||r.requestId!==request.requestId||!uuid(r.holdId)||!uuid(r.fileIdentity)||r.id!==request.file.id
    ||!integer(r.sizeBytes)||r.sizeBytes!==request.file.sizeBytes)throw fail('The native file hold receipt does not match its request.');
  return copy(r as unknown as FileHoldReceipt);
}
export function mobileFileHoldsCreate(identity:ComposerMountedIdentity,generation:number,target:FileHoldTarget):FileHoldLedger {
  if(!scopeValid(identity,generation,target))throw fail('Only a captured ordinary native editor can retain saved files.');
  return {identity:copy(identity),generation,target:copy(target),retired:false,revision:0,serial:0,entries:{}};
}
/** Caller uses existing localId() once, retains this plain reservation, then invokes acquire. */
export function mobileFileHoldReserve(ledger:FileHoldLedger,file:DraftFile,requestId:string):FileHoldRequest {
  if(ledger.retired||!uuid(requestId)||!fileValid(file)||file.draftKey!==ledger.target.draftKey||file.environmentId!==ledger.target.environmentId)
    throw fail('This saved file cannot be retained by the captured editor.');
  const request:FileHoldRequest=copy({op:'composerFileHold',action:'acquire',generation:ledger.generation,
    identity:ledger.identity,requestId,target:ledger.target,file});
  const prior=ledger.entries[requestId];
  if(prior){if(prior.desired==='release'||prior.phase==='released'||canonical(prior.request)!==canonical(request))throw fail('A file hold request cannot change or be revived.');return prior.request}
  if(Object.keys(ledger.entries).length>=LIMIT||!integer(ledger.revision)||ledger.revision>=MAX)throw fail('This editor cannot reserve more file hold requests.');
  ledger.entries[requestId]={request,receipt:null,phase:'reserved',desired:'retain',dispatched:false,attempt:0,error:''};changed(ledger);return request;
}
/** Authenticated local receipt only. Caller MUST synchronously recheck complete current admission,
 * target/catalog/mount before putting this or any historical held entry into a live projection. */
export function mobileFileHoldHeld(ledger:FileHoldLedger,requestId:string):HeldFile|null {
  const e=entry(ledger,requestId);
  return !ledger.retired&&e.desired==='retain'&&e.phase==='held'&&e.receipt?copy({request:e.request,receipt:e.receipt}):null;
}
export function mobileFileHoldReleaseNeeded(ledger:FileHoldLedger,requestId:string):void {
  const e=entry(ledger,requestId);if(e.desired==='release')return;
  e.desired='release';
  if(!e.dispatched)e.phase='released';
  else if(e.phase!=='acquiring'&&e.phase!=='releasing')e.phase='release-needed';
  changed(ledger);
}
export function mobileFileHoldsRetire(ledger:FileHoldLedger):void {
  if(ledger.retired)return;ledger.retired=true;changed(ledger);
  for(const id of Object.keys(ledger.entries))mobileFileHoldReleaseNeeded(ledger,id);
}
export async function mobileFileHoldAcquire(ledger:FileHoldLedger,requestId:string,native:Native):Promise<FileHoldResult> {
  const e=entry(ledger,requestId);
  if(ledger.retired||e.desired==='release')throw fail('A retiring file request cannot acquire again.');
  if(e.phase==='acquiring'||e.phase==='releasing')return result(ledger,e);
  if(!native.available){e.error='Native file retention is unavailable.';changed(ledger);return result(ledger,e)}
  const ticket=begin(ledger,e,'acquiring'),request=e.request;
  try {
    const validated=receipt(envelope(await native.later(request),request.generation),request);
    if(e.attempt!==ticket)return result(ledger,e);
    if(e.receipt&&canonical(e.receipt)!==canonical(validated))throw fail('The native file hold replay returned a different receipt.');
    e.receipt=validated;e.phase=releasing(ledger,e)?'release-needed':'held';e.error='';changed(ledger);
  }catch(error){
    if(e.attempt===ticket){e.phase=releasing(ledger,e)?'release-needed':'acquire-uncertain';
      e.error=letGo(error)?'':error instanceof Error?error.message:'The file hold outcome is uncertain.';changed(ledger)}
    if(letGo(error))throw error;
  }
  return result(ledger,e);
}
export async function mobileFileHoldRelease(ledger:FileHoldLedger,requestId:string,native:Native):Promise<FileHoldResult> {
  const e=entry(ledger,requestId);mobileFileHoldReleaseNeeded(ledger,requestId);
  if(e.phase==='released'||e.phase==='acquiring'||e.phase==='releasing')return result(ledger,e);
  if(!native.available){e.error='Native file release is unavailable.';changed(ledger);return result(ledger,e)}
  const ticket=begin(ledger,e,'releasing'),request=e.request;
  const release={op:'composerFileHold',action:'release',generation:request.generation,identity:request.identity,requestId,
    ...(e.receipt?{holdId:e.receipt.holdId}:{})};
  try {
    const value=envelope(await native.later(release),request.generation);
    if(!plain(value)||!keys(value,['status','requestId'])||value.status!=='released'||value.requestId!==requestId)throw fail('The native release reply does not match its request.');
    if(e.attempt===ticket){e.phase='released';e.error='';changed(ledger)}
  }catch(error){
    if(e.attempt===ticket){e.phase='release-uncertain';e.error=letGo(error)?'':error instanceof Error?error.message:'The file release outcome is uncertain.';changed(ledger)}
    if(letGo(error))throw error;
  }
  return result(ledger,e);
}
