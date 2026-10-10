// Saved canonical file admission/Undo ownership for the real ordinary native editor.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type {T3Client} from './shared/client';
import type {Native,Files} from './shared/protocol';
import {ClientError} from './shared/protocol';
import {letGo} from './shared/let-go';
import {localId} from './shared/composer-editor';
import {fleet} from './shared/settings-b-fleet';
import {queuedEditNative} from './queued-edit-state';
import {mobileCacheCatalogIdentity} from './mobile-client-cache-catalog';
import {mobileComposerContextSendSnapshot} from './composer-command-context';
import {mobileEditorOwner,mobileEditorContextTargetCurrent,mobileEditorPublishMounted,type EditorOwner} from './composer-editor-owner';
import {mobileOutboxTransferCanonical as canonical} from './mobile-outbox-transfer-model';
import {mobileComposerFileHistoryProject,mobileComposerFileHistoryDispose,type ComposerFileHistory} from './composer-file-history';
import {mobileFileHoldsCreate,mobileFileHoldReserve,mobileFileHoldAcquire,mobileFileHoldHeld,mobileFileHoldReleaseNeeded,
  mobileFileHoldRelease,mobileFileHoldsRetire,type FileHoldLedger,type HeldFile} from './composer-file-holds-io';
export interface EditorFileProjection {ready:boolean;prepareKey:string;needsPrepare:boolean;message:string;cleanupKey:string;retiredAdmissions:string[];revision:number}
interface State {incoming:Map<string,Set<string>>;admission:string;owner:EditorOwner;catalog:string;ledger:FileHoldLedger;history:ComposerFileHistory;seeded:boolean;
  attempt:number;attempted:string;message:string;releaseRevision:number}
interface Registry {active:State|null;retired:Map<string,State>;revision:number;cleanupAttempted:string;retry:number;message:string}
const states=new WeakMap<T3Client,Registry>();
function registry(client:T3Client):Registry {let r=states.get(client);if(!r){r={active:null,retired:new Map(),revision:0,cleanupAttempted:'',retry:0,message:''};states.set(client,r)}return r}
function changed(client:T3Client):void {const r=registry(client);if(r.revision<Number.MAX_SAFE_INTEGER)r.revision++;if(client.revision<Number.MAX_SAFE_INTEGER)client.revision++}
const fail=()=>new ClientError('The file-owning editor changed.','superseded');
const identity=(o:EditorOwner)=>({...o.state.identity,mountId:o.state.mountId});
function current(client:T3Client,s:State):boolean {
  return registry(client).active===s&&!s.ledger.retired&&mobileEditorOwner(client)===s.owner&&s.owner.route.active
    &&mobileEditorContextTargetCurrent(client,s.owner.target)&&canonical(identity(s.owner))===canonical(s.ledger.identity)
    &&s.catalog===mobileCacheCatalogIdentity(fleet.saved,s.owner.target.environmentId);
}
function usable(s:State):HeldFile[]{return Object.keys(s.ledger.entries).flatMap(id=>{const held=mobileFileHoldHeld(s.ledger,id);return held?[held]:[]})}
function retire(client:T3Client,s:State):void {
  if(s.ledger.retired)return;
  const disposed=mobileComposerFileHistoryDispose(s.history);if(disposed.status==='ready')s.history=disposed.history;
  mobileFileHoldsRetire(s.ledger);registry(client).retired.set(s.admission,s);changed(client);
}
/** Projection calls this before exposing configuration. A real ready mount is mandatory. */
function observe(client:T3Client):State|null {
  const r=registry(client),owner=mobileEditorOwner(client);
  if(r.active&&(!owner||r.active.owner!==owner||!current(client,r.active))){retire(client,r.active);r.active=null}
  if(!owner||!owner.state.mountId||!owner.route.active||!mobileEditorContextTargetCurrent(client,owner.target))return null;
  if(r.active)return r.active;
  const catalog=mobileCacheCatalogIdentity(fleet.saved,owner.target.environmentId);
  if(!catalog||owner.signature!==JSON.stringify([owner.target.owner,owner.route.routeVisit,owner.route.editorId,catalog]))return null;
  const target={origin:owner.target.origin,environmentId:owner.target.environmentId,threadId:owner.target.threadId,draftKey:owner.target.key};
  const ledger=mobileFileHoldsCreate(identity(owner),owner.target.generation,target);
  const s:State={incoming:new Map(),admission:owner.admission,owner,catalog,ledger,history:{identity:ledger.identity,target:ledger.target,revision:0,closed:false,entries:[]},
    seeded:false,attempt:0,attempted:'',message:'',releaseRevision:0};
  r.active=s;r.message='';changed(client);return s;
}
function inspect(client:T3Client,s:State) {
  const snapshot=current(client,s)?mobileComposerContextSendSnapshot(client,s.owner.target):null;
  if(!snapshot)return {ready:false,key:'',message:'This saved draft or file inventory is invalid.',snapshot:null,holds:[] as HeldFile[]};
  const holds=usable(s),byId=new Map(holds.map(h=>[h.receipt.id,h]));
  const key=JSON.stringify([s.admission,s.ledger.identity.mountId,snapshot.files,s.attempt]);
  if(snapshot.files.some(f=>f.source!=='attached'))return {ready:false,key:'',message:'This file has not entered saved-file ownership yet.',snapshot,holds};
  const missing=snapshot.files.filter(f=>!byId.has(f.id)||byId.get(f.id)!.receipt.sizeBytes!==f.sizeBytes);
  if(missing.length)return {ready:false,key,message:s.message||'Protecting saved files before editing…',snapshot,holds};
  if(s.history.entries.some(e=>!holds.some(h=>h.request.requestId===e.hold.request.requestId)))return {ready:false,key:'',message:'A file history hold is no longer usable.',snapshot,holds};
  if(!s.seeded){
    const seeded=mobileComposerFileHistoryProject(s.history,{identity:s.ledger.identity,target:s.ledger.target,context:snapshot.context??undefined,
      attachments:snapshot.attachmentIds.map(id=>{const file=snapshot.files.find(f=>f.id===id);return file?{type:'file' as const,file,hold:byId.get(id)??null}:{type:'image' as const,id}}),usableHolds:holds});
    if(seeded.status!=='ready'||seeded.unavailable.length)return {ready:false,key:'',message:'A referenced file is unavailable for Undo.',snapshot,holds};
    s.history=seeded.history;s.seeded=true;changed(client);
  }
  return {ready:true,key:'',message:'',snapshot,holds};
}
function cleanupKey(client:T3Client):string {
  const r=registry(client),rows=[...(r.active?[r.active]:[]),...r.retired.values()];
  const pending=rows.flatMap(s=>Object.values(s.ledger.entries).filter(e=>e.desired==='release'&&e.phase!=='released').map(e=>[s.admission,e.request.requestId,e.phase==='acquiring'?'acquiring':'settled']));
  return pending.length?JSON.stringify([pending,r.retry]):'';
}
export function mobileEditorFilesSnapshot(client:T3Client):EditorFileProjection {
  const s=observe(client),r=registry(client),read=s?inspect(client,s):null,key=cleanupKey(client);
  return {ready:!!read?.ready,prepareKey:read?.key??'',needsPrepare:!!read?.key&&s!.attempted!==read!.key&&!s!.owner.route.readOnly&&!s!.owner.route.voiceBusy,
    message:read?.message||r.message,cleanupKey:key&&key!==r.cleanupAttempted?key:'',retiredAdmissions:[...r.retired.keys()],revision:r.revision};
}
/** Actual control admission rechecks native hold usability; projected booleans are not authority. */
export function mobileEditorFilesReady(client:T3Client,admission:string):boolean {
  const s=observe(client);return !!s&&s.admission===admission&&inspect(client,s).ready;
}
/** A user retry creates new invocation work; uncertain acquisition reuses its frozen request. */
export function mobileEditorFilesRetry(client:T3Client,admission:string):void {
  const r=registry(client),s=r.active;if(s&&s.admission===admission&&s.attempt<Number.MAX_SAFE_INTEGER&&r.retry<Number.MAX_SAFE_INTEGER){s.attempt++;s.message='';r.retry++;r.message='';changed(client)}
}
export async function mobileEditorFilesPrepare(client:T3Client,admission:string,key:string,native:Native):Promise<void> {
  const s=observe(client);if(!s||s.admission!==admission||!current(client,s)||s.owner.route.readOnly||s.owner.route.voiceBusy)throw fail();
  const before=inspect(client,s);if(!key||key!==before.key||!before.snapshot)throw fail();
  if(s.attempted===key)return;s.attempted=key;changed(client);
  try {
    for(const file of before.snapshot.files){
      if(!current(client,s))throw fail();
      if(usable(s).some(h=>h.receipt.id===file.id&&h.receipt.sizeBytes===file.sizeBytes))continue;
      const old=Object.values(s.ledger.entries).find(e=>e.desired==='retain'&&e.request.file.id===file.id);
      const request=old?.request??mobileFileHoldReserve(s.ledger,file,localId());
      const answer=await mobileFileHoldAcquire(s.ledger,request.requestId,native);changed(client);
      if(!current(client,s))throw fail();
      if(!answer.held){s.message=answer.message||'The file hold is still pending. Retry file preparation.';return}
    }
    s.message='';inspect(client,s);
  }catch(error){if(letGo(error))throw error;s.message=error instanceof Error?error.message:'Could not protect this saved file.'}
  finally {changed(client)}
}
/** Called only from the actual native-event reducer, before stage/ACK or persistence. */
export function mobileEditorFilesPublish(client:T3Client):{ok:true}|{ok:false;message:string} {
  const s=observe(client);if(!s)return {ok:false,message:'The native file owner is unavailable.'};
  const read=inspect(client,s);if(!read.ready)return {ok:false,message:read.message};
  const expected=s.history,answer=mobileEditorPublishMounted(client,expected,read.holds);
  if(!answer.ok)return answer;
  // No await or callback can change these identities between the transaction and installation.
  s.history=answer.history;
  for(const held of answer.release)if(![...s.incoming.values()].some(ids=>ids.has(held.request.requestId)))mobileFileHoldReleaseNeeded(s.ledger,held.request.requestId);
  if(answer.release.length&&s.releaseRevision<Number.MAX_SAFE_INTEGER)s.releaseRevision++;
  changed(client);return {ok:true};
}
/** Fresh persistence precedes dropping live-owner evictions. Release never unlinks bytes;
 * the existing guarded coordinator drain remains the sole deletion mechanism. */
export async function mobileEditorFilesCleanup(client:T3Client,key:string,native:Native,storage:Files):Promise<void> {
  const r=registry(client);if(!key||cleanupKey(client)!==key||r.cleanupAttempted===key)return;
  r.cleanupAttempted=key;const rows=[...(r.active?[r.active]:[]),...r.retired.values()];
  try {
    await client.persist(storage);let released=false;
    for(const s of rows){
      if(r.active!==s&&r.retired.get(s.admission)!==s)continue;
      for(const e of Object.values(s.ledger.entries))if(e.desired==='release'&&e.phase!=='released'){
        const result=await mobileFileHoldRelease(s.ledger,e.request.requestId,native);changed(client);
        if(result.phase==='released')released=true;else r.message=result.message||'A file release is still pending.';
      }
    }
    if(released)await queuedEditNative(native,{action:'release'});
  }catch(error){if(letGo(error))throw error;r.message=error instanceof Error?error.message:'Could not finish file cleanup.'}
  finally {changed(client)}
}
/** Root calls ONLY once the exact old native component is no longer root-owned (removed or
 * guaranteed native remount). This delegates residual cleanup to native claim.end; it does
 * not assert a release reply or grant a new owner any historical byte authority. */
export function mobileEditorFilesDelegateRetired(client:T3Client,admission:string):void {
  const r=registry(client),s=r.retired.get(admission);if(!s||!s.ledger.retired)return;
  r.retired.delete(admission);changed(client);
}

/** Issued native picker members enter the same real IO ledger before CAS. Pending incoming
 * ownership is independent of live rows and history, so concurrent typing cannot release it. */
export async function mobileEditorFilesIntake(client:T3Client,admission:string,operationId:string,
  files:readonly import('./shared/composer-editor-files').DraftFile[],native:Native):Promise<void> {
  const s=observe(client);if(!s||s.admission!==admission||!current(client,s))throw fail();
  let requests=s.incoming.get(operationId);if(!requests){requests=new Set();s.incoming.set(operationId,requests)}
  for(const file of files){
    const existing=[...requests].map(id=>s.ledger.entries[id]!).find(e=>e.request.file.id===file.id);
    const request=mobileFileHoldReserve(s.ledger,file,existing?.request.requestId??localId(),{operationId});
    requests.add(request.requestId);
  }
  changed(client);
  for(const id of requests){
    if(!current(client,s))throw fail();
    if(mobileFileHoldHeld(s.ledger,id))continue;
    const answer=await mobileFileHoldAcquire(s.ledger,id,native);changed(client);
    if(!current(client,s))throw fail();
    if(!answer.held)throw new ClientError(answer.message||'Imported file protection is pending. Retry this attachment action.','retained');
  }
}
/** After a settled native intake partition, transfer real holds to live/history ownership.
 * Unaccepted files become guarded cleanup obligations; native lifetime handles retired owners. */
export function mobileEditorFilesIntakeEnd(client:T3Client,admission:string,operationId:string):void {
  const r=registry(client),s=r.active?.admission===admission?r.active:r.retired.get(admission);
  const requests=s?.incoming.get(operationId);if(!s||!requests)return;
  const snapshot=current(client,s)?mobileComposerContextSendSnapshot(client,s.owner.target):null;
  for(const id of requests){
    const e=s.ledger.entries[id]!;
    if(!s.history.entries.some(h=>h.hold.request.requestId===id)&&!snapshot?.files.some(f=>f.id===e.request.file.id))mobileFileHoldReleaseNeeded(s.ledger,id);
  }
  s.incoming.delete(operationId);changed(client);
}
