// Ordinary draft handoff DTOs; pure validation grants no native admission or durability.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import { mobileOutboxCanonicalOrigin, mobileOutboxDecode, type MobileOutboxRecord } from './mobile-outbox-model';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobileContextRecordValid } from './mobile-context-record';
import { imageMimeType } from './composer-editor-document';
import type { DraftFile } from './shared/composer-editor-files';
import type { Obj } from './shared/domain';

export interface ThreadSendSelection { start:number; end:number }
export interface ThreadSendSourceContext { version:1; records:Obj[] }
export interface ThreadSendCapturedDocument { incarnation:string; revision:number; selection:ThreadSendSelection|null }
export interface ThreadSendTransferCapture {
  version:2; kind:'ordinary'; draft:{
    key:string; origin:string; environmentId:string; threadId:string; document:ThreadSendCapturedDocument;
    text:string; context:ThreadSendSourceContext|null; contextRevision:number;
    images:Obj[]; files:DraftFile[]; attachmentIds:string[]; attachmentOrder:string[]|null;
  };
}
export type ThreadSendTransferDraftContent=Omit<ThreadSendTransferCapture['draft'],'document'>;
export interface ThreadSendTransferClaim {
  kind:'ordinary'; origin:string; environmentId:string; transferId:string; draftKey:string; fingerprint:string;
  messageId:string; threadId:string; commandId:string; mutationId:string;
  state:'prepared'|'queued'|'failed'|'completed'|'released';
  record:MobileOutboxRecord|null; capture:ThreadSendTransferCapture|null;
}
export interface ThreadSendPersistedDocument extends ThreadSendCapturedDocument {
  origin:string; environmentId:string; threadId:string; draftKey:string;
}
export interface ThreadSendPersistedContext {
  origin:string; environmentId:string; key:string; revision:number; text:string; context:ThreadSendSourceContext;
}
export interface ThreadSendTransferCompletion {
  version:2; kind:'ordinary'; draftKey:string; fingerprint:string; disposition:'cleared'|'preserved';
  before:{incarnation:string;revision:number};
  // Saved order is canonical effective IDs, or null when persistence omits the empty row.
  after:{document:ThreadSendPersistedDocument;text:string;context:ThreadSendPersistedContext|null;
    images:Obj[];files:DraftFile[];attachmentIds:string[];attachmentOrder:string[]|null};
}
type Plain=Record<string,unknown>;
const plain=(v:unknown):v is Plain=>v!==null&&typeof v==='object'&&!Array.isArray(v)
  &&[Object.prototype,null].includes(Object.getPrototypeOf(v));
const text=(v:unknown,max=4096):v is string=>typeof v==='string'&&v.length>0&&v.trim()===v&&v.length<=max;
const integer=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0;
const uuid=(v:unknown):v is string=>typeof v==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
const fields=(v:Plain,keys:string[])=>Object.keys(v).every(k=>keys.includes(k));
const copy=<T>(v:T):T=>JSON.parse(JSON.stringify(v)) as T;
const fail=():never=>{throw new Error('The ordinary draft transfer is invalid.');};
/** Refuse data JSON would silently erase, including getters, holes, symbols and extra array keys. */
function json(v:unknown,parents=new Set<object>()):boolean {
  if(v===null||typeof v==='string'||typeof v==='boolean')return true;
  if(typeof v==='number')return Number.isFinite(v);
  if(!Array.isArray(v)&&!plain(v)||parents.has(v as object))return false;
  parents.add(v as object);
  const array=Array.isArray(v),keys=Reflect.ownKeys(v as object).filter(k=>!array||k!=='length');
  const valid=(!array||keys.length===v.length&&Array.from({length:v.length},(_,i)=>Object.hasOwn(v,i)).every(Boolean))
    &&keys.every(k=>{const d=Object.getOwnPropertyDescriptor(v,k)!;return typeof k==='string'&&d.enumerable&&'value'in d&&json(d.value,parents)});
  parents.delete(v as object);return valid;
}
function named(v:Plain,key='key'):boolean {
  return mobileOutboxCanonicalOrigin(v.origin)&&text(v.environmentId)&&text(v.threadId)&&!v.threadId.startsWith('new:')
    &&v[key]===`${v.environmentId}:${v.threadId}`&&!(v[key] as string).includes('~queued-edit~');
}
function selection(v:unknown,length:number):boolean {
  return v===null||plain(v)&&fields(v,['start','end'])&&integer(v.start)&&integer(v.end)&&v.start<=v.end&&v.end<=length;
}
function document(v:unknown,value:string,persisted=false):v is ThreadSendCapturedDocument {
  return plain(v)&&fields(v,['incarnation','revision','selection',...(persisted?['origin','environmentId','threadId','draftKey']:[])])
    &&text(v.incarnation,128)&&integer(v.revision)&&selection(v.selection,value.length)&&(!persisted||named(v,'draftKey'));
}
function context(v:unknown,bounded=true):v is ThreadSendSourceContext|null {
  if(v===null)return true;
  if(!plain(v)||!fields(v,['version','records'])||v.version!==1||!Array.isArray(v.records)
    ||bounded&&(v.records.length>200||JSON.stringify(v.records).length>16_000_000))return false;
  const ids=new Set<string>();
  return v.records.every(r=>{
    if(!plain(r)||!mobileContextRecordValid(r as Obj)||ids.has(String(r.contextId)))return false;
    ids.add(String(r.contextId));return true;
  });
}
function inventory(v:Plain,target:Plain,bounded=true):boolean {
  if(!Array.isArray(v.images)||!Array.isArray(v.files)||!Array.isArray(v.attachmentIds)
    ||bounded&&v.images.length+v.files.length>100)return false;
  const ids=new Set<string>(),contexts=new Set<string>();
  const descriptor=(r:unknown):r is Plain=>{
    if(!plain(r)||!(bounded?uuid(r.id):text(r.id))||ids.has(String(r.id).toLowerCase())||!text(r.name,255)||!text(r.mimeType,100)||!integer(r.sizeBytes))return false;
    ids.add(String(r.id).toLowerCase());return true;
  };
  const upload=(v:unknown)=>typeof v==='string'&&(v===''||/^[a-z0-9_-]{1,128}$/i.test(v));
  if(!v.images.every(r=>descriptor(r)&&(!Object.hasOwn(r,'uploadId')||upload(r.uploadId))
    &&(!Object.hasOwn(r,'source')||plain(r.source))))return false;
  if(!v.files.every(r=>descriptor(r)&&r.draftKey===target.key&&r.environmentId===target.environmentId&&text(r.contextId,128)
    &&!contexts.has(r.contextId)&&!!contexts.add(r.contextId)&&(bounded?r.source==='attached':text(r.source))&&upload(r.attachmentId)
    &&typeof r.status==='string'&&['staged','ready'].includes(r.status)&&(r.status!=='ready'||r.attachmentId!=='')
    &&['videoWidth','videoHeight'].every(k=>!Object.hasOwn(r,k)||typeof r[k]==='number'&&Number.isFinite(r[k])&&Number(r[k])>0)))return false;
  const order=v.attachmentIds,rawOrder=v.attachmentOrder;
  if(rawOrder!==null&&(!Array.isArray(rawOrder)||!rawOrder.every(id=>text(id))||new Set(rawOrder).size!==rawOrder.length))return false;
  const rows=[...v.images as Plain[],...v.files as Plain[]],expected=[...new Set([...(rawOrder as string[]|null??[]),...rows.map(r=>r.id)])].filter(id=>rows.some(r=>r.id===id));
  return canonical(order)===canonical(expected)&&order.length===ids.size&&order.every(id=>bounded?uuid(id):text(id))&&new Set(order.map(id=>String(id).toLowerCase())).size===order.length
    &&order.every(id=>[...v.images as Plain[],...v.files as Plain[]].some(row=>row.id===id));
}
function contextBindings(source:ThreadSendSourceContext|null,attachments:MobileOutboxRecord['attachments']):boolean {
  return source===null||source.records.every(r=>{
    if(!Object.hasOwn(r,'attachmentId'))return true;
    const a=attachments.find(a=>a.id===r.attachmentId);
    return !!a&&(r.kind===a.kind||r.kind==='image'&&a.kind==='file'&&imageMimeType(a)!==null)
      &&['name','mimeType','sizeBytes'].every(k=>r[k]===a[k as keyof typeof a]);
  });
}
function recordMatches(raw:unknown,d:ThreadSendTransferCapture['draft']):raw is MobileOutboxRecord {
  if(!plain(raw)||!json(raw)||Object.hasOwn(raw,'creation')||!fields(raw,['schemaVersion','origin','environmentId','threadId','messageId',
    'commandId','text','context','attachments','modelSelection','runtimeMode','interactionMode','dispatchMode','createdAt']))return false;
  if(!['modelSelection','runtimeMode','interactionMode'].every(k=>Object.hasOwn(raw,k)))return false;
  const decoded=mobileOutboxDecode(raw);if(!decoded.ok)return false;
  const r=decoded.record;
  if(r.origin!==d.origin||r.environmentId!==d.environmentId||r.threadId!==d.threadId||r.text!==d.text.trim()
    ||canonical(r.context??null)!==canonical(d.context)||r.attachments.length!==d.attachmentIds.length
    ||!contextBindings(d.context,r.attachments))return false;
  return r.attachments.every((a,i)=>{
    if(a.id!==d.attachmentIds[i])return false;
    const original:Plain|undefined=a.kind==='image'?d.images.find(x=>x.id===a.id):d.files.find(x=>x.id===a.id) as unknown as Plain|undefined;
    if(!original||!['name','mimeType','sizeBytes'].every(k=>original[k]===a[k as keyof typeof a]))return false;
    const upload=a.kind==='image'?original.uploadId??'':original.attachmentId;
    if(a.uploadId!==upload||a.status!==(upload?'ready':'staged')||(upload?a.uploadEnvironmentId!==d.environmentId:a.uploadEnvironmentId!==undefined))return false;
    const expected={id:a.id,kind:a.kind,name:a.name,mimeType:a.mimeType,sizeBytes:a.sizeBytes,uploadId:upload,status:upload?'ready':'staged',
      ...(upload?{uploadEnvironmentId:d.environmentId}:{}),...(a.kind==='image'?Object.hasOwn(original,'source')?{source:original.source}:{}:
        {contextId:original.contextId,source:original.source,...(Object.hasOwn(original,'videoWidth')?{videoWidth:original.videoWidth}:{}),
          ...(Object.hasOwn(original,'videoHeight')?{videoHeight:original.videoHeight}:{})})};
    return canonical(a)===canonical(expected);
  });
}
/** Validate actual raw content before enrollment; never synthesize a provisional document owner. */
export function threadSendTransferDecodeDraftContent(raw:unknown):ThreadSendTransferDraftContent {
  if(!plain(raw)||!json(raw)||!fields(raw,['key','origin','environmentId','threadId','text','context','contextRevision','images','files','attachmentIds','attachmentOrder'])
    ||!named(raw)||typeof raw.text!=='string'||raw.text.length>1_000_000||!integer(raw.contextRevision)||!context(raw.context)||!inventory(raw,raw))return fail();
  const d=raw as unknown as ThreadSendTransferDraftContent;
  if(d.context?.records.some(r=>{
    if(!Object.hasOwn(r,'attachmentId'))return false;
    const image=d.images.find(a=>a.id===r.attachmentId),file=d.files.find(a=>a.id===r.attachmentId),row=image??file;
    return !row||!(r.kind===(image?'image':'file')||r.kind==='image'&&file&&imageMimeType(file)!==null)
      ||['name','mimeType','sizeBytes'].some(k=>r[k]!==row[k as keyof typeof row]);
  }))return fail();
  return copy(d);
}
export function threadSendTransferDecodeCapture(raw:unknown,record?:unknown):ThreadSendTransferCapture {
  if(!plain(raw)||!json(raw)||!fields(raw,['version','kind','draft'])||raw.version!==2||raw.kind!=='ordinary'||!plain(raw.draft))return fail();
  const {document:doc,...content}=raw.draft,d=threadSendTransferDecodeDraftContent(content);
  if(!document(doc,d.text)||record!==undefined&&!recordMatches(record,{...d,document:doc}))return fail();
  return copy(raw) as unknown as ThreadSendTransferCapture;
}
export function threadSendTransferDecodeClaim(raw:unknown):ThreadSendTransferClaim {
  if(!plain(raw)||!json(raw)||!fields(raw,['kind','origin','environmentId','transferId','draftKey','fingerprint','messageId','threadId','commandId','mutationId','state','record','capture'])
    ||raw.kind!=='ordinary'||!named(raw,'draftKey')||!['transferId','messageId','commandId','mutationId'].every(k=>text(raw[k]))
    ||raw.transferId!==raw.messageId||typeof raw.fingerprint!=='string'||!/^[0-9a-f]{64}$/.test(raw.fingerprint)
    ||typeof raw.state!=='string'||!['prepared','queued','failed','completed','released'].includes(raw.state))return fail();
  if(['completed','released'].includes(raw.state)){if(raw.record!==null||raw.capture!==null)return fail()}
  else {
    const c=threadSendTransferDecodeCapture(raw.capture,raw.record),r=raw.record as MobileOutboxRecord;
    if(c.draft.origin!==raw.origin||c.draft.environmentId!==raw.environmentId||c.draft.key!==raw.draftKey
      ||r.messageId!==raw.messageId||r.threadId!==raw.threadId||r.commandId!==raw.commandId)return fail();
  }
  return copy(raw) as unknown as ThreadSendTransferClaim;
}
export function threadSendTransferDecodeCompletion(raw:unknown,claim:unknown):ThreadSendTransferCompletion {
  const c=threadSendTransferDecodeClaim(claim);
  if(c.state!=='queued'||!c.capture||!plain(raw)||!json(raw)||!fields(raw,['version','kind','draftKey','fingerprint','disposition','before','after'])
    ||raw.version!==2||raw.kind!=='ordinary'||raw.draftKey!==c.draftKey||raw.fingerprint!==c.fingerprint
    ||!['cleared','preserved'].includes(typeof raw.disposition==='string'?raw.disposition:'')||!plain(raw.before)||!fields(raw.before,['incarnation','revision'])
    ||raw.before.incarnation!==c.capture.draft.document.incarnation||raw.before.revision!==c.capture.draft.document.revision||!plain(raw.after))return fail();
  const a=raw.after,d=c.capture.draft;
  if(!fields(a,['document','text','context','images','files','attachmentIds','attachmentOrder'])||typeof a.text!=='string'||a.text.length>1_000_000
    ||!document(a.document,a.text,true)||!inventory(a,d,false)
    ||canonical(a.attachmentOrder)!==canonical((a.attachmentIds as unknown[]).length?a.attachmentIds:null))return fail();
  const doc=a.document as unknown as ThreadSendPersistedDocument;
  if(doc.origin!==d.origin||doc.environmentId!==d.environmentId||doc.threadId!==d.threadId||doc.draftKey!==d.key)return fail();
  if(a.context!==null){
    const x=a.context;
    if(!plain(x)||!fields(x,['origin','environmentId','key','revision','text','context'])||x.origin!==d.origin||x.environmentId!==d.environmentId
      ||x.key!==d.key||x.text!==a.text||!integer(x.revision)||x.context===null||!context(x.context,false))return fail();
  }
  if(raw.disposition==='cleared'){
    if(d.document.revision===Number.MAX_SAFE_INTEGER||doc.incarnation!==d.document.incarnation||doc.revision!==d.document.revision+1
      ||a.text!==''||a.context!==null||a.attachmentOrder!==null||(a.images as unknown[]).length||(a.files as unknown[]).length||(a.attachmentIds as unknown[]).length
      ||canonical(doc.selection)!==canonical({start:0,end:0}))return fail();
  }else if(doc.incarnation===d.document.incarnation&&(doc.revision<d.document.revision
    ||doc.revision===d.document.revision&&a.text!==d.text))return fail();
  return copy(raw) as unknown as ThreadSendTransferCompletion;
}
