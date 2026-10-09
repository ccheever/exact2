// Invocation-owned native image-lease adapter. This does not publish composer content.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { EditorPasteProof } from './composer-editor-persistence';
import { ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';

export interface ComposerPasteStageRequest {
  op:'composerEditorPasteFiles'; action:'stage'; generation:number; operationId:string;
  identity:EditorPasteProof['identity']; richEventId:string; target:EditorPasteProof['target'];
  sources:Array<{kind:'editorImage';leaseId:string}>; remaining:number;
}
export interface ComposerPasteTransitionRequest {
  op:'composerEditorPasteFiles'; action:'adopt'|'discard'|'retire'; generation:number; operationId:string; proof:EditorPasteProof;
}
export type ComposerPasteRequest=ComposerPasteStageRequest|ComposerPasteTransitionRequest;
export interface ComposerPasteSkipped {leaseId:string;reason:'excess'|'unreadable'|'too-large'}
export type ComposerPasteReply={status:'staged'|'adopted'|'discarded';proof:EditorPasteProof;skipped:ComposerPasteSkipped[]}
  |{status:'retired';proof:EditorPasteProof};
export interface ComposerPasteResult {
  state:'answered'|'stale'|'uncertain'|'unavailable'; request:ComposerPasteRequest; reply:ComposerPasteReply|null; message:string;
}
type Obj=Record<string,unknown>;
const fail=(message:string)=>new ClientError(message,'ComposerPaste');
const plain=(v:unknown):v is Obj=>v!==null&&typeof v==='object'&&!Array.isArray(v)
  &&[Object.prototype,null].includes(Object.getPrototypeOf(v));
const keys=(v:Obj,names:string[])=>Object.keys(v).length===names.length&&names.every(k=>Object.hasOwn(v,k));
const integer=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0;
const text=(v:unknown):v is string=>typeof v==='string'&&v.length>0;
const uuid=(v:unknown):v is string=>typeof v==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
const lowerUUID=(v:unknown):v is string=>uuid(v)&&v===v.toLowerCase();
const identityKeys=['owner','editorId','routeVisit','renderEpoch','mountId'];
const targetKeys=['origin','environmentId','draftKey','incarnation','capturedRevision'];
/** Reject accessors, extra array properties and non-JSON values before reading any fields. */
function json(v:unknown,ancestors=new Set<object>(),depth=0):boolean {
  if(v===null||typeof v==='string'||typeof v==='boolean')return true;
  if(typeof v==='number')return Number.isFinite(v);
  if(typeof v!=='object'||depth>64||ancestors.has(v)||!Array.isArray(v)&&!plain(v))return false;
  const descriptors=Object.getOwnPropertyDescriptors(v),names=Reflect.ownKeys(descriptors);
  if(names.some(k=>typeof k!=='string'))return false;
  if(Array.isArray(v)&& (names.length!==v.length+1||!Array.from({length:v.length},(_,i)=>String(i)).every(k=>Object.hasOwn(descriptors,k))))return false;
  ancestors.add(v);
  const valid=names.every(k=>{const d=descriptors[k as string];return 'value'in d&&(k==='length'&&Array.isArray(v)||d.enumerable===true&&json(d.value,ancestors,depth+1))});
  ancestors.delete(v);return valid;
}
function copy<T>(v:T):T {
  const out=JSON.parse(JSON.stringify(v)) as T;
  const freeze=(x:unknown):void=>{if(x&&typeof x==='object'){Object.values(x).forEach(freeze);Object.freeze(x)}};
  freeze(out);return out;
}
function canonical(v:unknown):string {
  if(Array.isArray(v))return '['+v.map(canonical).join(',')+']';
  if(plain(v))return '{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+canonical(v[k])).join(',')+'}';
  return JSON.stringify(v)??'';
}
const same=(a:unknown,b:unknown)=>canonical(a)===canonical(b);
const identity=(v:unknown)=>plain(v)&&keys(v,identityKeys)&&identityKeys.every(k=>text(v[k]));
const target=(v:unknown)=>plain(v)&&keys(v,targetKeys)&&targetKeys.slice(0,4).every(k=>text(v[k]))&&integer(v.capturedRevision);
function proof(v:unknown):v is EditorPasteProof {
  if(!plain(v)||!keys(v,['version','operationId','identity','richEventId','target','files'])||v.version!==1||!lowerUUID(v.operationId)
    ||!identity(v.identity)||!uuid(v.richEventId)||!target(v.target)||!Array.isArray(v.files)||v.files.length>100)return false;
  const ids=new Set<string>(),leases=new Set<string>();
  return v.files.every(f=>{
    if(!plain(f)||!keys(f,['leaseId','id','kind','name','mimeType','sizeBytes','sha256'])||!uuid(f.leaseId)||!lowerUUID(f.id)
      ||ids.has(f.id)||leases.has(f.leaseId.toLowerCase())||f.kind!=='image'||f.name!=='pasted-image.png'||f.mimeType!=='image/png'
      ||!integer(f.sizeBytes)||f.sizeBytes===0||f.sizeBytes>10*1024*1024||typeof f.sha256!=='string'||!/^[0-9a-f]{64}$/.test(f.sha256))return false;
    ids.add(f.id);leases.add(f.leaseId.toLowerCase());return true;
  });
}
/** Call once with caller-generated operationId; persist this exact intent before dispatch if restart recovery is required. */
export function mobileComposerPasteRequest(raw:unknown):ComposerPasteRequest {
  if(!json(raw)||!plain(raw)||raw.op!=='composerEditorPasteFiles'||!integer(raw.generation)||!lowerUUID(raw.operationId))throw fail('Invalid native paste request.');
  if(raw.action==='stage'){
    if(!keys(raw,['op','action','generation','operationId','identity','richEventId','target','sources','remaining'])
      ||!identity(raw.identity)||!uuid(raw.richEventId)||!target(raw.target)||!integer(raw.remaining)||raw.remaining>100
      ||!Array.isArray(raw.sources)||raw.sources.length>1024)throw fail('Invalid pasted image admission.');
    const leases=new Set<string>();
    for(const source of raw.sources){
      if(!plain(source)||!keys(source,['kind','leaseId'])||source.kind!=='editorImage'||!uuid(source.leaseId)||leases.has(source.leaseId.toLowerCase()))throw fail('Invalid native image lease.');
      leases.add(source.leaseId.toLowerCase());
    }
  }else if(typeof raw.action!=='string'||!['adopt','discard','retire'].includes(raw.action)
    ||!keys(raw,['op','action','generation','operationId','proof'])||!proof(raw.proof)||raw.proof.operationId!==raw.operationId)throw fail('Invalid pasted image transition.');
  return copy(raw as unknown as ComposerPasteRequest);
}
function decode(raw:unknown,request:ComposerPasteRequest):ComposerPasteReply {
  if(!json(raw)||!plain(raw)||typeof raw.ok!=='boolean'||!integer(raw.generation)||raw.generation!==request.generation)throw fail('The native paste generation does not match.');
  if(!raw.ok){
    if(!plain(raw.error)||!text(raw.error.kind)||typeof raw.error.message!=='string')throw fail('Invalid native paste error.');
    throw new ClientError(raw.error.message,raw.error.kind,raw.error.uncertain===true);
  }
  const v=raw.value;
  if(!plain(v)||!proof(v.proof)||v.proof.operationId!==request.operationId)throw fail('Invalid native paste proof.');
  if(request.action==='stage'){
    if(!same(v.proof.identity,request.identity)||!same(v.proof.target,request.target)||v.proof.richEventId!==request.richEventId)throw fail('The native paste proof belongs to another editor.');
  }else if(!same(v.proof,request.proof))throw fail('The native paste proof changed.');
  if(request.action==='retire'){
    if(!keys(v,['status','proof'])||v.status!=='retired')throw fail('Invalid native paste retirement.');
    return copy(v as unknown as ComposerPasteReply);
  }
  if(!keys(v,['status','proof','skipped'])||typeof v.status!=='string'||!['staged','adopted','discarded'].includes(v.status)
    ||request.action!=='stage'&&v.status==='staged'||!Array.isArray(v.skipped)||v.skipped.length>1024)throw fail('Invalid native paste outcome.');
  const seen=new Set(v.proof.files.map(f=>f.leaseId.toLowerCase()));
  for(const skip of v.skipped){
    if(!plain(skip)||!keys(skip,['leaseId','reason'])||!uuid(skip.leaseId)||seen.has(skip.leaseId.toLowerCase())
      ||typeof skip.reason!=='string'||!['excess','unreadable','too-large'].includes(skip.reason))throw fail('Invalid skipped image receipt.');
    seen.add(skip.leaseId.toLowerCase());
  }
  if(request.action==='stage'){
    const indices=new Map(request.sources.map((s,i)=>[s.leaseId,i]));let acceptedIndex=-1,skippedIndex=-1;
    for(const f of v.proof.files){const i=indices.get(f.leaseId);if(i===undefined||i>=request.remaining||i<=acceptedIndex)throw fail('Invalid accepted image order.');acceptedIndex=i}
    for(const s of v.skipped as unknown as ComposerPasteSkipped[]){const i=indices.get(s.leaseId);
      if(i===undefined||i<=skippedIndex||(s.reason==='excess')!==(i>=request.remaining))throw fail('Invalid skipped image order.');skippedIndex=i}
    if(seen.size!==request.sources.length)throw fail('The native image result is incomplete.');
  }
  return copy(v as unknown as ComposerPasteReply);
}
/** One invocation, no retained handles or automatic cleanup/retry. Caller serializes lifecycle actions.
 * current checks exact route/catalog/owner/generation for stage/publication, or captured cleanup authority
 * for transitions. An authenticated reply alone does not authorize document publication. */
export async function mobileComposerPasteInvoke(raw:ComposerPasteRequest,native:Native,current:()=>boolean):Promise<ComposerPasteResult> {
  const request=mobileComposerPasteRequest(raw);
  const result=(state:ComposerPasteResult['state'],message='',reply:ComposerPasteReply|null=null):ComposerPasteResult=>({state,request,reply,message});
  if(!current())return result('stale');
  if(!native.available)return result('unavailable','Native pasted image storage is unavailable.');
  let decoded:ComposerPasteReply|null=null;
  try {
    decoded=decode(await native.later(request),request);
    return current()?result('answered','',decoded):result('stale','',decoded);
  }catch(error){
    if(letGo(error))throw error;
    return result('uncertain',error instanceof Error?error.message:'The pasted image outcome is uncertain.',decoded);
  }
}
