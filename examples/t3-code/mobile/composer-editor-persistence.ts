// App-owned durable ordinary documents and Send receipts; rich editor activation remains separate.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { T3Client, Pending } from './shared/client';
import type { MobileComposerTarget } from './composer-target';
import { obj, str, type Obj, type Json } from './shared/domain';
import { ClientError } from './shared/protocol';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';

export interface EditorDurableDocument {
  origin:string; environmentId:string; threadId:string; draftKey:string; incarnation:string;
  revision:number; value:string; blocked?:true; selection:{start:number;end:number}|null;
}
export interface EditorDocumentIntent {
  key:string; incarnation:string; revision:number; before:string; selection:{start:number;end:number};
  target:MobileComposerTarget; producer:string;
}
export interface EditorSendReceipt {
  id:string; documentKey:string; incarnation:string; revision:number; before:string; outgoing:string;
  environmentId:string; threadId:string; method:string; commandId:string; fingerprint:string;
  authority:'preserve'|'complete-v1'; phase:'pending'|'confirmed'|'retired'|'preserved'; failure:boolean;
}
export interface EditorSendCapture { id:string; created:boolean; fingerprint:string }
export interface EditorPersistSnapshot { invalid:boolean; raw:unknown; paste:unknown }
interface Saved { version:1; documents:Record<string,EditorDurableDocument>; sends:Record<string,EditorSendReceipt>; pasteRetirements:Record<string,unknown> }
interface Store { saved:Saved; invalid?:unknown; paste:unknown; revision:number; hydrated:boolean }
const stores=new WeakMap<object,Store>();
const LIMIT=256, MAX_TEXT=1_000_000;
// Deliberately incomplete until all ordinary external producers use explicit writes.
const COMPLETE_PRODUCER_COVERAGE=false;
const copy=<T>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
const plain=(v:unknown):v is Obj=>v!==null&&typeof v==='object'&&!Array.isArray(v);
const integer=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0;
const uuid=(v:unknown):v is string=>typeof v==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
const text=(v:unknown,max=4096):v is string=>typeof v==='string'&&v.length>0&&v.length<=max;
const normal=(v:string)=>v.trim().replace(/\/+$/,'');
const home=(client:T3Client)=>normal(mobileQueuedEditOrigin(client));
export const mobileEditorDocumentKey=(d:Pick<EditorDurableDocument,'origin'|'environmentId'|'draftKey'>)=>JSON.stringify([d.origin,d.environmentId,d.draftKey]);
const refusal=()=>new ClientError('The saved editor ownership is unavailable. Keep this draft.','retained');
const fresh=():Saved=>({version:1,documents:{},sends:{},pasteRetirements:{}});
function store(client:T3Client):Store {
  let s=stores.get(client.local);if(!s){s={saved:fresh(),paste:{},revision:0,hydrated:false};stores.set(client.local,s)}return s;
}
function range(value:unknown,length:number):boolean {
  const r=obj(value);return integer(r.start)&&integer(r.end)&&r.start<=r.end&&r.end<=length;
}
function validDocument(raw:unknown,key:string):raw is EditorDurableDocument {
  if(!plain(raw)||!text(raw.origin)||raw.origin!==normal(raw.origin)||!text(raw.environmentId)||!text(raw.threadId)||!text(raw.incarnation,128)
    ||raw.draftKey!==`${raw.environmentId}:${raw.threadId}`||str(raw.threadId).startsWith('new:')||!integer(raw.revision)
    ||typeof raw.value!=='string'||raw.value.length>MAX_TEXT||raw.blocked!==undefined&&raw.blocked!==true||raw.selection!==null&&!range(raw.selection,raw.value.length))return false;
  return key===mobileEditorDocumentKey(raw as unknown as EditorDurableDocument);
}
function validReceipt(raw:unknown,id:string,docs:Record<string,EditorDurableDocument>):raw is EditorSendReceipt {
  if(!plain(raw)||raw.id!==id||!text(raw.documentKey)||!text(raw.incarnation,128)||!integer(raw.revision)||typeof raw.before!=='string'||raw.before.length>MAX_TEXT
    ||typeof raw.outgoing!=='string'||!text(raw.commandId)||!text(raw.environmentId)||!text(raw.threadId)
    ||raw.method!=='orchestration.dispatchCommand'||typeof raw.fingerprint!=='string'||typeof raw.failure!=='boolean'
    ||!['preserve','complete-v1'].includes(str(raw.authority))||!['pending','confirmed','retired','preserved'].includes(str(raw.phase)))return false;
  const d=docs[raw.documentKey];return !!d&&d.incarnation===raw.incarnation&&d.revision>=raw.revision&&d.environmentId===raw.environmentId&&d.threadId===raw.threadId
    &&id===JSON.stringify([d.origin,raw.environmentId,raw.method,raw.commandId]);
}
function decode(raw:unknown,shared?:Obj):Saved|null {
  if(!plain(raw)||raw.version!==1||!plain(raw.documents)||!plain(raw.sends)||!plain(raw.pasteRetirements)||Object.keys(raw.pasteRetirements).length>LIMIT||Object.keys(raw.documents).length>LIMIT||Object.keys(raw.sends).length>LIMIT)return null;
  for(const [id,proof]of Object.entries(raw.pasteRetirements))if(!validPasteProof(proof)||proof.operationId!==id)return null;
  const decoded=copy(raw);
  if(shared){
    for(const value of Object.values(obj(decoded.documents))){
      if(!plain(value)||'value' in value)return null;
      value.value=Object.hasOwn(obj(shared.drafts),str(value.draftKey))?obj(shared.drafts)[str(value.draftKey)]:'';
    }
    for(const value of Object.values(obj(decoded.sends))){
      if(!plain(value))return null;
      if(value.request==='pending'){
        const p=obj(obj(shared.pending)[str(value.environmentId)]);
        if(p.method!==value.method||obj(p.payload).commandId!==value.commandId||p.threadId!==value.threadId)return null;
        value.outgoing=str(obj(p.payload).text);value.fingerprint=payloadFingerprint(p as unknown as Pending);
      }else if(value.request==='terminal'&&['retired','preserved'].includes(str(value.phase))){value.outgoing='';value.fingerprint=''}
      else if(value.request!=='inline')return null;
      delete value.request;
    }
  }
  for(const [key,value]of Object.entries(obj(decoded.documents)))if(!validDocument(value,key))return null;
  const docs=decoded.documents as unknown as Saved['documents'];
  for(const [key,value]of Object.entries(obj(decoded.sends)))if(!validReceipt(value,key,docs))return null;
  return decoded as unknown as Saved;
}
/** No decoded membership means no authority to fall back to legacy equality clear. */
export function mobileEditorDocumentsHydrate(client:T3Client,saved:Obj):void {
  const s=store(client);if(s.hydrated)return;s.hydrated=true;
  if(saved.mobileComposerEditor!==undefined){
    const value=decode(saved.mobileComposerEditor,saved);
    if(!value)s.invalid=copy(saved.mobileComposerEditor);else s.saved=value;
  }
  s.paste=saved.composerPasteAdoptions===undefined?{}:copy(saved.composerPasteAdoptions);s.revision++;
}
function ordinary(target:MobileComposerTarget):boolean {
  return target.kind==='ordinary'&&!!target.threadId&&!target.threadId.startsWith('new:')&&target.key===`${target.environmentId}:${target.threadId}`;
}
function connection(client:T3Client,target:MobileComposerTarget):boolean {
  return ordinary(target)&&target.origin===client.origin&&target.environmentId===client.environmentId&&target.generation===client.generation;
}
function scope(client:T3Client,target:MobileComposerTarget) {
  return {origin:home(client),environmentId:target.environmentId,threadId:target.threadId,draftKey:target.key};
}
/** Called only by explicit rich admission, never by legacy Send or background reads. */
export function mobileEditorDocumentEnroll(client:T3Client,target:MobileComposerTarget):EditorDurableDocument|null {
  const s=store(client);if(s.invalid!==undefined||!connection(client,target)||!home(client))return null;
  const captured=scope(client,target),key=mobileEditorDocumentKey(captured),old=s.saved.documents[key],value=client.local.drafts[target.key]??'';
  if(old)return !old.blocked&&old.value===value?old:null;
  if(Object.keys(s.saved.documents).length>=LIMIT||value.length>MAX_TEXT)throw refusal();
  const d:EditorDurableDocument={...captured,incarnation:`document-${Object.keys(s.saved.documents).length+1}`,revision:0,value,selection:null};
  s.saved.documents[key]=d;s.revision++;return d;
}
export function mobileEditorDocument(client:T3Client,key:string):EditorDurableDocument|null {
  const s=store(client);return s.invalid===undefined?s.saved.documents[key]??null:null;
}
/** Read-only producer admission. Invalid saved membership never grants legacy fallback. */
export function mobileEditorDocumentMembership(client:T3Client,target:MobileComposerTarget):'unenrolled'|'enrolled'|'unavailable' {
  const s=store(client);
  if(!connection(client,target)||!home(client)||s.invalid!==undefined)return 'unavailable';
  const d=s.saved.documents[mobileEditorDocumentKey(scope(client,target))];
  if(!d)return 'unenrolled';
  return d.blocked||d.value!==(client.local.drafts[target.key]??'')?'unavailable':'enrolled';
}
export function mobileEditorDocumentCapture(client:T3Client,target:MobileComposerTarget,producer:string,selection?:{start:number;end:number}):EditorDocumentIntent|null {
  if(!connection(client,target)||!producer)return null;
  const key=mobileEditorDocumentKey(scope(client,target)),d=mobileEditorDocument(client,key),value=client.local.drafts[target.key]??'';
  const r=selection??d?.selection??{start:value.length,end:value.length};
  if(!d||d.blocked||d.value!==value||!range(r,value.length))return null;
  return {key,incarnation:d.incarnation,revision:d.revision,before:d.value,selection:{...r},target:copy(target),producer};
}
export function mobileEditorDocumentIntentCurrent(client:T3Client,c:EditorDocumentIntent):boolean {
  const d=mobileEditorDocument(client,c.key);
  return connection(client,c.target)&&c.key===mobileEditorDocumentKey(scope(client,c.target))&&!!d&&!d.blocked&&d.incarnation===c.incarnation
    &&d.revision===c.revision&&d.value===c.before&&(client.local.drafts[d.draftKey]??'')===c.before;
}
/** At an actual reducer write: after is already in the named slot. No guessed writes. */
export function mobileEditorDocumentWritten(client:T3Client,target:MobileComposerTarget,before:string,after:string,selection?:{start:number;end:number}):boolean {
  if(!connection(client,target)||(client.local.drafts[target.key]??'')!==after||after.length>MAX_TEXT||selection&&!range(selection,after.length))return false;
  const d=mobileEditorDocument(client,mobileEditorDocumentKey(scope(client,target)));if(!d||d.blocked||d.value!==before||d.revision===Number.MAX_SAFE_INTEGER)return false;
  if(before!==after){d.revision++;d.value=after;d.selection=selection?{...selection}:null;store(client).revision++}
  else if(selection){d.selection={...selection};store(client).revision++}return true;
}
/** Caller proves no mounted owner immediately before this synchronous named commit. */
export function mobileEditorDocumentCommit(client:T3Client,c:EditorDocumentIntent,next:{value:string;selection:{start:number;end:number}}):boolean {
  if(!mobileEditorDocumentIntentCurrent(client,c)||next.value.length>MAX_TEXT||!range(next.selection,next.value.length))return false;
  const d=mobileEditorDocument(client,c.key)!;if(d.revision===Number.MAX_SAFE_INTEGER)return false;
  client.local.drafts[d.draftKey]=next.value;d.value=next.value;d.selection={...next.selection};d.revision++;store(client).revision++;client.revision++;return true;
}
/** Internal mounted publication primitive. The concrete owner must additionally prove current
 * mount/queued claim/fence. Native's retained terminal proves the replacement; latest is the
 * admitted observation, which may include text typed after that terminal. No IO or callbacks. */
export function mobileEditorDocumentCommitMounted(client:T3Client,c:EditorDocumentIntent,
  proof:{command:import('./composer-editor-state').ComposerEditorCommand;terminal:import('./composer-editor-state').ComposerEditorEvent;
    latest:import('./composer-editor-state').ComposerEditorEvent},next:{value:string;selection:{start:number;end:number}}):boolean {
  const {command,terminal,latest}=proof;
  const identity=(v:import('./composer-editor-state').ComposerMountedIdentity)=>canonical([v.owner,v.editorId,v.routeVisit,v.renderEpoch,v.mountId]);
  if(identity(command)!==identity(terminal)||identity(command)!==identity(latest)
    ||terminal.commandId!==command.commandId||terminal.commandRevision!==command.commandRevision
    ||!['commandApplied','commandRejected'].includes(terminal.kind)||terminal.eventCount<=command.expected.eventCount
    ||latest.eventCount<terminal.eventCount||next.value!==latest.value||canonical(next.selection)!==canonical(latest.selection)
    ||terminal.kind==='commandApplied'&&(terminal.value!==command.next.value||canonical(terminal.selection)!==canonical(command.next.selection)||terminal.composing)
    ||!mobileEditorDocumentIntentCurrent(client,c)||next.value.length>MAX_TEXT||!range(next.selection,next.value.length))return false;
  const d=mobileEditorDocument(client,c.key)!;if(d.revision===Number.MAX_SAFE_INTEGER)return false;
  client.local.drafts[d.draftKey]=next.value;d.value=next.value;d.selection={...next.selection};d.revision++;store(client).revision++;client.revision++;return true;
}
const messageSend=(p:Pending)=>p.method==='orchestration.dispatchCommand'&&p.payload.type==='message.dispatch'&&!!p.threadId&&p.payload.threadId===p.threadId;
const payloadFingerprint=(p:Pending)=>canonical({method:p.method,payload:p.payload,threadId:p.threadId,text:p.text});
function receiptID(client:T3Client,p:Pending,environmentId=client.environmentId){return JSON.stringify([home(client),environmentId,p.method,str(p.payload.commandId)])}
function managed(client:T3Client,p:Pending,environmentId:string):boolean {
  const s=store(client);return messageSend(p)&&(s.invalid!==undefined||Object.values(s.saved.documents).some(d=>d.environmentId===environmentId&&d.draftKey===`${environmentId}:${p.threadId}`));
}
export function mobileEditorSendCapture(client:T3Client,p:Pending,retry:boolean):EditorSendCapture|null {
  if(!messageSend(p))return null;const s=store(client);if(s.invalid!==undefined)throw refusal();
  const key=mobileEditorDocumentKey({origin:home(client),environmentId:client.environmentId,draftKey:`${client.environmentId}:${p.threadId}`}),d=s.saved.documents[key];
  if(!d)return null;const id=receiptID(client,p),fingerprint=payloadFingerprint(p),old=s.saved.sends[id];
  if(old){if(['retired','preserved'].includes(old.phase)||old.fingerprint!==fingerprint)throw refusal();return {id,created:false,fingerprint}}
  // A legacy retry may complete, but cannot acquire ownership of a later document.
  if(retry)return null;
  if(Object.keys(s.saved.sends).length>=LIMIT||!text(p.payload.commandId)||typeof p.payload.text!=='string')throw refusal();
  if(Object.values(s.saved.sends).some(r=>r.documentKey===key&&r.phase==='confirmed'))throw new ClientError('Finish the confirmed draft cleanup before sending again.','retained');
  const before=client.local.drafts[d.draftKey]??'';if(before!==d.value)d.blocked=true;
  s.saved.sends[id]={id,documentKey:key,incarnation:d.incarnation,revision:d.revision,before,outgoing:str(p.payload.text),
    environmentId:d.environmentId,threadId:d.threadId,method:p.method,commandId:str(p.payload.commandId),fingerprint,
    authority:COMPLETE_PRODUCER_COVERAGE&&!d.blocked?'complete-v1':'preserve',phase:'pending',failure:false};s.revision++;
  return {id,created:true,fingerprint};
}
export function mobileEditorSendFailed(client:T3Client,c:EditorSendCapture|null):void {
  if(!c)return;const r=store(client).saved.sends[c.id];if(r?.fingerprint===c.fingerprint&&r.phase==='pending'){r.failure=true;store(client).revision++}
}
export function mobileEditorSendConfirmed(client:T3Client,p:Pending,environmentId:string):{managed:boolean;receiptKey:string} {
  if(!managed(client,p,environmentId))return {managed:false,receiptKey:''};
  const s=store(client);if(s.invalid!==undefined)return {managed:true,receiptKey:''};
  // Captured environment may differ after focus changed; resolve receipt from its payload, not current focus origin.
  const r=Object.values(s.saved.sends).find(r=>r.environmentId===environmentId&&r.threadId===p.threadId&&r.commandId===p.payload.commandId&&r.fingerprint===payloadFingerprint(p));
  if(r&&r.phase==='pending'){r.phase=COMPLETE_PRODUCER_COVERAGE&&r.authority==='complete-v1'?'confirmed':'preserved';s.revision++}
  return {managed:true,receiptKey:r?.id??''};
}
export function mobileEditorRetirementReceipt(client:T3Client,id:string):EditorSendReceipt|null {const s=store(client);return s.invalid===undefined&&s.saved.sends[id]?copy(s.saved.sends[id]):null}
/** Pure policy fixture seam; production passes its private incomplete coverage constant. */
export function editorRetirementDecision(r:EditorSendReceipt,d:EditorDurableDocument|null,completeCoverage:boolean):'preserve'|'clear' {
  return completeCoverage&&r.authority==='complete-v1'&&r.phase==='confirmed'&&!!d&&!d.blocked&&d.incarnation===r.incarnation&&d.revision===r.revision&&d.value===r.before?'clear':'preserve';
}
export function mobileEditorRetirementAllowed(client:T3Client,id:string):boolean {
  const r=mobileEditorRetirementReceipt(client,id);return !!r&&editorRetirementDecision(r,mobileEditorDocument(client,r.documentKey),COMPLETE_PRODUCER_COVERAGE)==='clear';
}
export function mobileEditorRetirementComplete(client:T3Client,id:string,outcome:'retired'|'preserved'):void {
  const s=store(client),r=s.saved.sends[id];if(!r||!['pending','confirmed'].includes(r.phase))return;
  r.phase=outcome;s.revision++;
}
export function mobileEditorRetirementSnapshot(client:T3Client){const s=store(client);return {revision:s.revision,items:Object.values(s.saved.sends).map(r=>({key:r.id,phase:r.phase,documentKey:r.documentKey,
  message:r.phase==='preserved'?'Message sent. The draft text was kept.':''}))}}
export function mobileEditorPersistSnapshot(client:T3Client):EditorPersistSnapshot {
  const s=store(client);
  if(s.invalid===undefined)for(const d of Object.values(s.saved.documents)){if((client.local.drafts[d.draftKey]??'')!==d.value)d.blocked=true}
  return {invalid:s.invalid!==undefined,raw:copy(s.invalid===undefined?s.saved:s.invalid),paste:copy(s.paste)};
}
/** Captured records are checked against these exact shared bytes, never later live state. */
export function mobileEditorPersistDocument(snapshot:EditorPersistSnapshot,shared:Obj):Obj {
  const output={...shared},valid=snapshot.invalid?null:decode(snapshot.raw);
  if(valid){
    for(const d of Object.values(valid.documents)){
      const actual=Object.hasOwn(obj(shared.drafts),d.draftKey)?obj(shared.drafts)[d.draftKey]:'';if(typeof actual!=='string'||actual.length>MAX_TEXT||!d.blocked&&actual!==d.value)throw refusal();
    }
    for(const r of Object.values(valid.sends)){
      const pending=obj(obj(shared.pending)[r.environmentId]);
      if(pending.payload&&obj(pending.payload).commandId===r.commandId&&payloadFingerprint(pending as unknown as Pending)!==r.fingerprint)throw refusal();
    }
    if(Object.keys(valid.documents).length||Object.keys(valid.sends).length||Object.keys(valid.pasteRetirements).length){
      const documents:Obj={},sends:Obj={};
      for(const [key,d]of Object.entries(valid.documents)){const {value,...row}=d;documents[key]={...row,...(row.blocked?{selection:null}:{})}}
      for(const [key,r]of Object.entries(valid.sends)){
        const pending=obj(obj(shared.pending)[r.environmentId]),terminal=['retired','preserved'].includes(r.phase),matching=pending.method===r.method&&obj(pending.payload).commandId===r.commandId&&pending.threadId===r.threadId;
        const {outgoing,fingerprint,...rest}=r;
        sends[key]=terminal?{...rest,request:'terminal'}:matching?{...rest,request:'pending'}:{...r,request:'inline'};
      }
      output.mobileComposerEditor={version:1,documents,sends,pasteRetirements:copy(valid.pasteRetirements) as Json};
    }
  }else output.mobileComposerEditor=copy(snapshot.raw) as Json;
  if(plain(snapshot.paste)&&Object.keys(snapshot.paste).length===0)delete output.composerPasteAdoptions;
  else output.composerPasteAdoptions=copy(snapshot.paste) as Json;
  return output;
}

export interface EditorPasteProof {
  version:1; operationId:string; identity:{owner:string;editorId:string;routeVisit:string;renderEpoch:string;mountId:string};richEventId:string;
  target:{origin:string;environmentId:string;draftKey:string;incarnation:string;capturedRevision:number};
  files:Array<{leaseId:string;id:string;kind:'image';name:string;mimeType:string;sizeBytes:number;sha256:string}>;
}
export interface EditorPasteMarker {proof:EditorPasteProof;publication:{origin:string;environmentId:string;draftKey:string;incarnation:string;revision:number}}
function validPasteProof(v:unknown):v is EditorPasteProof {
  if(!plain(v)||v.version!==1||!uuid(v.operationId)||v.operationId!==v.operationId.toLowerCase()||!uuid(v.richEventId)||!plain(v.identity)||!plain(v.target)
    ||!['owner','editorId','routeVisit','renderEpoch','mountId'].every(k=>text(obj(v.identity)[k]))
    ||!['origin','environmentId','draftKey','incarnation'].every(k=>text(obj(v.target)[k]))||!integer(v.target.capturedRevision)
    ||!Array.isArray(v.files)||v.files.length>100)return false;
  const ids=new Set<string>(),leases=new Set<string>();
  return v.files.every(f=>{if(!plain(f)||!uuid(f.leaseId)||!uuid(f.id)||f.id!==f.id.toLowerCase()||ids.has(f.id)||leases.has(f.leaseId)||f.kind!=='image'
      ||f.name!=='pasted-image.png'||f.mimeType!=='image/png'||!integer(f.sizeBytes)||f.sizeBytes===0||f.sizeBytes>10*1024*1024
      ||typeof f.sha256!=='string'||!/^[0-9a-f]{64}$/.test(f.sha256))return false;
    ids.add(f.id);leases.add(f.leaseId);return true});
}
function validMarker(v:unknown):v is EditorPasteMarker {
  if(!plain(v)||!validPasteProof(v.proof)||!plain(v.publication)||!integer(v.publication.revision))return false;
  const t=v.proof.target,p=v.publication;return p.origin===t.origin&&p.environmentId===t.environmentId&&p.draftKey===t.draftKey&&p.incarnation===t.incarnation&&Number(p.revision)>=t.capturedRevision;
}
/** Future paste producer calls only after actual CAS and metadata publication; no lease IO. */
export function mobileEditorPastePublish(client:T3Client,marker:EditorPasteMarker):boolean {
  const s=store(client);if(s.invalid!==undefined||!plain(s.paste)||!validMarker(marker)||Object.entries(s.paste).some(([id,v])=>!validMarker(v)||v.proof.operationId!==id))return false;
  const id=marker.proof.operationId,old=s.paste[id];if(old!==undefined)return canonical(old)===canonical(marker);
  if(Object.keys(s.paste).length>=LIMIT||s.saved.pasteRetirements[id])return false;
  const p=marker.publication,d=s.saved.documents[mobileEditorDocumentKey(p)];
  if(!d||d.blocked||d.incarnation!==p.incarnation||d.revision!==p.revision||d.value!==(client.local.drafts[d.draftKey]??''))return false;
  const images=client.local.snapshotDrafts[d.draftKey]??[];
  if(!marker.proof.files.every(f=>images.some(i=>i.id===f.id&&i.name===f.name&&i.mimeType===f.mimeType&&i.sizeBytes===f.sizeBytes)))return false;
  s.paste[id]=copy(marker) as unknown as Obj;s.revision++;return true;
}
/** Only an exact native adopted outcome admits marker removal; receipt survives the save. */
export function mobileEditorPasteRemoveMarker(client:T3Client,proof:EditorPasteProof,adopted:{status:string;proof:unknown}):boolean {
  const s=store(client);if(s.invalid!==undefined||!plain(s.paste)||!validPasteProof(proof)||adopted.status!=='adopted'||canonical(adopted.proof)!==canonical(proof))return false;
  const old=s.paste[proof.operationId];if(!validMarker(old)||canonical(old.proof)!==canonical(proof))return false;
  if(Object.keys(s.saved.pasteRetirements).length>=LIMIT)return false;
  s.saved.pasteRetirements[proof.operationId]=copy(proof);delete s.paste[proof.operationId];s.revision++;return true;
}
export function mobileEditorPasteRetirements(client:T3Client):EditorPasteProof[] {
  const s=store(client);return s.invalid===undefined?copy(Object.values(s.saved.pasteRetirements)) as EditorPasteProof[]:[];
}
/** Caller supplies actual native retire reply after durable marker removal; never deletes bytes. */
export function mobileEditorPasteRetired(client:T3Client,proof:EditorPasteProof,reply:{status:string;proof:unknown}):boolean {
  const s=store(client),old=s.saved.pasteRetirements[proof.operationId];
  if(s.invalid!==undefined||!old||reply.status!=='retired'||canonical(reply.proof)!==canonical(old)||canonical(proof)!==canonical(old))return false;
  delete s.saved.pasteRetirements[proof.operationId];s.revision++;return true;
}
