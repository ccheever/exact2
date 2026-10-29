// Actual native picker intake, with invocation-only IO and immutable retry input.
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#composer-command-foundation
import { mobileComposerFileScope } from './composer-file-scope';
import type { ComposerMountedIdentity } from './composer-editor-state';
import type { FileHoldTarget } from './composer-file-holds-io';
import type { ThreadSendTransferCompletion } from './thread-send-transfer-model';
import type { MobileNewTaskDraft } from './mobile-new-task-drafts';
import { mobileContextRecordValid } from './mobile-context-record';
import type { Obj as JsonObj } from './shared/domain';
import { ClientError, reply, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

export interface PickerIntakeProvenance {operationId:string}
interface PickerIntakeBase {op:'composerPickerIntake';generation:number;identity:ComposerMountedIdentity;operationId:string}
export interface PickerIntakePick extends PickerIntakeBase {
  action:'pick';target:FileHoldTarget;document:{incarnation:string;revision:number};source:'photos'|'files';remaining:number;fileLimit:number;
}
export type PickerIntakeNewTaskAfter=Omit<ThreadSendTransferCompletion['after'],'document'|'context'>&{record:MobileNewTaskDraft};
export interface PickerIntakePublication {after:ThreadSendTransferCompletion['after']|PickerIntakeNewTaskAfter;acceptedIds:string[];discardedIds:string[]}
export interface PickerIntakeFinish extends PickerIntakeBase {action:'finish';publication:PickerIntakePublication}
export interface PickerIntakeObserve extends PickerIntakeBase {action:'status'|'cancel'}
export type PickerIntakeRequest=PickerIntakePick|PickerIntakeFinish|PickerIntakeObserve;
export interface PickerIntakeFile {kind:'image'|'file';id:string;name:string;mimeType:string;sizeBytes:number;videoWidth?:number;videoHeight?:number}
export interface PickerIntakeValue {operationId:string;identity:ComposerMountedIdentity;status:'picking'|'staged'|'finished'|'cancelled';files:PickerIntakeFile[];error:string}
export interface PickerIntakeResult {state:'answered'|'stale'|'uncertain'|'unavailable';request:PickerIntakeRequest;value:PickerIntakeValue|null;message:string}
type Obj=Record<string,unknown>;
const plain=(v:unknown):v is Obj=>v!==null&&typeof v==='object'&&!Array.isArray(v)&&[Object.prototype,null].includes(Object.getPrototypeOf(v));
const keys=(v:Obj,names:string[])=>Reflect.ownKeys(v).length===names.length&&names.every(k=>Object.hasOwn(v,k));
const integer=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0;
const text=(v:unknown):v is string=>typeof v==='string'&&v.length>0;
const uuid=(v:unknown):v is string=>typeof v==='string'&&/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(v);
const fail=(message='The picker intake is invalid.')=>new ClientError(message,'PickerIntake');
const identityKeys=['owner','editorId','routeVisit','renderEpoch','mountId'];
const identity=(v:unknown):v is ComposerMountedIdentity=>plain(v)&&keys(v,identityKeys)&&identityKeys.every(k=>text(v[k]));
const equalIdentity=(a:ComposerMountedIdentity,b:ComposerMountedIdentity)=>identityKeys.every(k=>a[k as keyof ComposerMountedIdentity]===b[k as keyof ComposerMountedIdentity]);
function json(v:unknown,parents=new Set<object>()):boolean {
  if(v===null||typeof v==='string'||typeof v==='boolean')return true;
  if(typeof v==='number')return Number.isFinite(v);
  if(!Array.isArray(v)&&!plain(v)||parents.has(v as object))return false;
  parents.add(v as object);const array=Array.isArray(v),names=Reflect.ownKeys(v as object).filter(k=>!array||k!=='length');
  const ok=(!array||names.length===v.length&&Array.from({length:v.length},(_,i)=>Object.hasOwn(v,i)).every(Boolean))&&names.every(k=>{
    const d=Object.getOwnPropertyDescriptor(v,k)!;return typeof k==='string'&&d.enumerable&&'value'in d&&json(d.value,parents);
  });parents.delete(v as object);return ok;
}
function frozen<T>(raw:T):T {
  const v=JSON.parse(JSON.stringify(raw)) as T;
  const freeze=(a:unknown):void=>{if(a&&typeof a==='object'){Object.values(a).forEach(freeze);Object.freeze(a)}};freeze(v);return v;
}
function scope(v:unknown,i:ComposerMountedIdentity,generation:number):v is FileHoldTarget {
  return mobileComposerFileScope(i.owner,v)?.generation===generation;
}
function publication(v:unknown,i:ComposerMountedIdentity,generation:number):boolean {
  if(!plain(v)||!keys(v,['after','acceptedIds','discardedIds'])||!plain(v.after)
    ||!Array.isArray(v.acceptedIds)||!Array.isArray(v.discardedIds)||!(v.acceptedIds as unknown[]).every(uuid)||!(v.discardedIds as unknown[]).every(uuid)
    ||new Set([...v.acceptedIds,...v.discardedIds]).size!==v.acceptedIds.length+v.discardedIds.length)return false;
  const a=v.after;
  if(typeof a.text!=='string'||a.text.length>1_000_000)return false;
  let owner:FileHoldTarget;
  if(keys(a,['document','text','context','images','files','attachmentIds','attachmentOrder'])){
    const d=a.document,c=a.context;
    if(!plain(d)||!keys(d,['origin','environmentId','threadId','draftKey','incarnation','revision','selection'])||!text(d.incarnation)||d.incarnation.length>128||!integer(d.revision))return false;
    const target={origin:d.origin,environmentId:d.environmentId,threadId:d.threadId,draftKey:d.draftKey};
    if(!scope(target,i,generation))return false;owner=target;
    if(d.selection!==null&&(!plain(d.selection)||!keys(d.selection,['start','end'])||!integer(d.selection.start)||!integer(d.selection.end)||d.selection.start>d.selection.end||d.selection.end>a.text.length))return false;
    if(c!==null&&(!plain(c)||!keys(c,['origin','environmentId','key','revision','text','context'])||c.origin!==d.origin||c.environmentId!==d.environmentId||c.key!==d.draftKey||!integer(c.revision)||c.text!==a.text))return false;
    if(c!==null){
      const context=(c as Obj).context;
      if(!plain(context)||!keys(context,['version','records'])||!contextRecords(context))return false;
    }
  }else if(keys(a,['record','text','images','files','attachmentIds','attachmentOrder'])){
    const r=a.record;
    if(!plain(r)||!integer(r.revision)||r.choices!==null&&!plain(r.choices))return false;
    const target={kind:'new-task',origin:r.origin,environmentId:r.environmentId,projectId:r.projectId,draftKey:r.key,createdAt:r.createdAt};
    if(!scope(target,i,generation))return false;owner=target;
    if(Object.hasOwn(r,'context')&&(!plain(r.context)||!contextRecords(r.context)||!Array.isArray(r.context.records)||r.context.records.length>200))return false;
  }else return false;
  if(!Array.isArray(a.images)||!Array.isArray(a.files)||!Array.isArray(a.attachmentIds)||!a.attachmentIds.every(text))return false;
  const attachmentIds=a.attachmentIds;
  if('kind'in owner&&(!v.acceptedIds.every(id=>attachmentIds.includes(id))||v.discardedIds.some(id=>attachmentIds.includes(id))))return false;
  const ids=new Set<string>();
  const descriptor=(row:unknown):row is Obj=>{
    if(!plain(row)||!text(row.id)||ids.has(row.id)||!text(row.name)||!text(row.mimeType)||!integer(row.sizeBytes))return false;ids.add(row.id);return true;
  };
  if(!a.images.every(row=>descriptor(row)&&(!Object.hasOwn(row,'uploadId')||typeof row.uploadId==='string')&&(!Object.hasOwn(row,'source')||plain(row.source))))return false;
  if(!a.files.every(row=>descriptor(row)&&row.draftKey===owner.draftKey&&row.environmentId===owner.environmentId&&text(row.contextId)
    &&typeof row.source==='string'&&['attached','pasted-text'].includes(row.source)&&typeof row.status==='string'&&['staged','ready'].includes(row.status)
    &&typeof row.attachmentId==='string'&&['videoWidth','videoHeight'].every(k=>!Object.hasOwn(row,k)||typeof row[k]==='number'&&Number.isFinite(row[k])&&row[k]>0)))return false;
  // Saved projection is not Send admission: unavailable historical image records, recovery
  // overflow and private files remain exact metadata and grant no new upload authority.
  return attachmentIds.length===ids.size&&attachmentIds.every(id=>ids.delete(id))
    &&(attachmentIds.length===0?a.attachmentOrder===null:JSON.stringify(attachmentIds)===JSON.stringify(a.attachmentOrder));
}
function contextRecords(context:Obj):boolean {
  if(context.version!==1||!Array.isArray(context.records))return false;
  const seen=new Set<string>();
  return context.records.every(r=>{
    if(!plain(r)||!mobileContextRecordValid(r as JsonObj)||seen.has(String(r.contextId)))return false;
    seen.add(String(r.contextId));return true;
  });
}
/** Plain, copied input only. No retained native/clock/current predicate. */
export function mobilePickerIntakeRequest(raw:unknown):PickerIntakeRequest {
  if(!json(raw)||!plain(raw)||raw.op!=='composerPickerIntake'||!integer(raw.generation)||!identity(raw.identity)||!uuid(raw.operationId)||typeof raw.action!=='string')throw fail();
  const base=['op','action','generation','identity','operationId'];
  if(raw.action==='pick'){
    if(!keys(raw,[...base,'target','document','source','remaining','fileLimit'])||!scope(raw.target,raw.identity,raw.generation)||!plain(raw.document)
      ||!keys(raw.document,['incarnation','revision'])||!text(raw.document.incarnation)||raw.document.incarnation.length>128||!integer(raw.document.revision)
      ||!['photos','files'].includes(String(raw.source))||typeof raw.source!=='string'||!integer(raw.remaining)||raw.remaining<1||raw.remaining>100
      ||!integer(raw.fileLimit)||raw.fileLimit>50*1024*1024||raw.source==='files'&&raw.fileLimit===0)throw fail();
    if('kind'in raw.target&&raw.document.incarnation!==raw.target.createdAt)throw fail();
  }else if(raw.action==='finish'){
    if(!keys(raw,[...base,'publication'])||!publication(raw.publication,raw.identity,raw.generation))throw fail('The picker publication snapshot is invalid.');
  }else if(!['status','cancel'].includes(raw.action)||!keys(raw,base))throw fail();
  return frozen(raw as unknown as PickerIntakeRequest);
}
function decode(raw:unknown,request:PickerIntakeRequest):PickerIntakeValue {
  const parsed=reply(raw);
  if(!integer(parsed.generation)||parsed.generation!==request.generation)throw fail('The picker reply generation changed.');
  if(!parsed.ok)throw new ClientError(parsed.error!.message,parsed.error!.kind,parsed.error!.uncertain);
  const v:unknown=parsed.value;
  if(!json(v)||!plain(v)||!keys(v,['operationId','identity','status','files','error'])||v.operationId!==request.operationId||!identity(v.identity)||!equalIdentity(v.identity,request.identity)
    ||typeof v.status!=='string'||!['picking','staged','finished','cancelled'].includes(v.status)||typeof v.error!=='string'||!Array.isArray(v.files)||v.files.length>100)throw fail('The picker reply does not match its owner.');
  const ids=new Set<string>();
  for(const f of v.files){
    if(!plain(f)||Object.keys(f).some(k=>!['kind','id','name','mimeType','sizeBytes','videoWidth','videoHeight'].includes(k))||!uuid(f.id)||ids.has(f.id)
      ||typeof f.kind!=='string'||!['image','file'].includes(f.kind)||!text(f.name)||!text(f.mimeType)||!integer(f.sizeBytes)||f.sizeBytes===0
      ||f.sizeBytes>(f.kind==='image'?10:50)*1024*1024||!['videoWidth','videoHeight'].every(k=>!Object.hasOwn(f,k)||typeof f[k]==='number'&&Number.isFinite(f[k])&&f[k]>0))throw fail('The picker returned invalid attachment metadata.');
    ids.add(f.id);
  }
  if(request.action==='pick'&&v.files.length>request.remaining||request.action==='finish'&&v.status!=='finished'||request.action==='cancel'&&v.status!=='cancelled')throw fail('The picker returned an invalid operation state.');
  if(request.action==='finish'){
    const expected=[...request.publication.acceptedIds,...request.publication.discardedIds];
    if(expected.length!==ids.size||!expected.every(id=>ids.has(id)))throw fail('The finished picker partition changed.');
  }
  return frozen(v as unknown as PickerIntakeValue);
}
export async function mobilePickerIntakeInvoke(raw:PickerIntakeRequest,native:Native,current:()=>boolean):Promise<PickerIntakeResult> {
  const request=mobilePickerIntakeRequest(raw);
  const result=(state:PickerIntakeResult['state'],message='',value:PickerIntakeValue|null=null):PickerIntakeResult=>({state,request,value,message});
  if(!current())return result('stale');
  if(!native.available)return result('unavailable','Native attachment picking is unavailable.');
  let value:PickerIntakeValue|null=null;
  try{value=decode(await native.later(request),request);return current()?result('answered','',value):result('stale','',value)}
  catch(error){if(letGo(error))throw error;return result('uncertain',error instanceof Error?error.message:'The picker outcome is uncertain.',value)}
}
