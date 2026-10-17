// Ordinary Send's explicit local recovery; native owns the interrupted mutation.
// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import type { MobileDraftClient } from './mobile-draft-recovery';
import { mobileThreadTransferBusy, mobileThreadTransferLease, mobileThreadTransferResume, type ThreadTransferResult } from './thread-send-transfer';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxThreadTransferLookup, mobileOutboxThreadTransferStatus,
  mobileOutboxReleaseFailedThreadTransfer, type MobileOutboxThreadTarget, type MobileOutboxOutcome } from './mobile-outbox';
import { threadSendTransferDecodeClaim, type ThreadSendTransferClaim } from './thread-send-transfer-model';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { fleet } from './shared/settings-b-fleet';
import { mobileQueuedEditCurrent } from './queued-edit-state';
import { obj } from './shared/domain';
import { ClientError, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';

export interface ThreadSendRecoveryInput { target:MobileOutboxThreadTarget; routeVisit:string; current():boolean }
export type ThreadSendRecoveryOperation='commit'|'rollback'|'retry'|'finish'|'release';
export interface ThreadSendRecoveryItem {
  transferId:string; state:ThreadSendTransferClaim['state']|'unknown'; message:string;
  actions:Array<{kind:ThreadSendRecoveryOperation;label:string;key:string}>;
}
export interface ThreadSendRecoveryView {
  revision:number; scope:string; routeVisit:string; complete:boolean; blocksSend:boolean; busy:boolean;
  message:string; items:ThreadSendRecoveryItem[];
}
export interface ThreadSendRecoveryResult {revision:number;message:string;released:boolean;transfer:ThreadTransferResult|null}
interface Evidence {claim:ThreadSendTransferClaim;outcome:MobileOutboxOutcome|null;epoch:string;mode:'pending'|'durability'|'settled'|'unknown'}
const labels:Record<ThreadSendRecoveryOperation,string>={commit:'Keep queued message',rollback:'Keep draft instead',retry:'Retry saving',finish:'Finish saving message',release:'Keep draft'};
const stale=()=>new ClientError('This message recovery changed. Refresh its saved status.','superseded');
const terminal=(claim:ThreadSendTransferClaim)=>['completed','released'].includes(claim.state);
function scope(client:MobileDraftClient,input:ThreadSendRecoveryInput,handle:Native|null|undefined) {
  const target={...input.target},visit=input.routeVisit,generation=client.generation;
  const check=()=>{
    if(!visit||input.routeVisit!==visit||canonical(input.target)!==canonical(target)||!input.current()
      ||client.generation!==generation||client.environmentId!==target.environmentId||client.threadId!==target.threadId
      ||client.draftKey!==target.draftKey||mobileQueuedEditCurrent(client)!==null||target.draftKey!==`${target.environmentId}:${target.threadId}`
      ||target.threadId.startsWith('new:')||target.draftKey.includes('~queued-edit~')||mobileQueuedEditOrigin(client)!==target.origin
      ||mobileCacheCatalogIdentity(fleet.saved,target.environmentId)!==JSON.stringify([target.environmentId,target.origin]))throw stale();
  };
  check();if(!handle?.available)throw new ClientError('Open T3 Code on your iPhone or iPad to recover this message.');
  const base=letGoAware(handle);
  const native:Native={available:true,watch(topic){check();base.watch(topic)},async later(request){check();const reply=await base.later(request);check();return reply}};
  return {native,check,target,visit};
}
function immutable(claim:ThreadSendTransferClaim) {
  const {state:_state,...value}=claim;return value;
}
function evidence(client:MobileDraftClient,claim:ThreadSendTransferClaim,outcome:MobileOutboxOutcome|null):Evidence {
  const saved=mobileOutboxSnapshot(client),same=saved.recovery.filter(raw=>obj(raw).messageId===claim.messageId);
  const pending=same.length===1&&same.some(raw=>{
    const envelope=obj(raw),mutation=obj(envelope.mutation),rawClaim=obj(mutation.transfer),{outcome:_outcome,...candidate}=rawClaim;
    try {return envelope.state==='pending'&&mutation.operation==='enqueue'&&mutation.mutationId===claim.mutationId
      &&canonical(mutation.record)===canonical(claim.record)
      &&canonical(immutable(threadSendTransferDecodeClaim(candidate)))===canonical(immutable(claim));}catch{return false}
  });
  const uncertain=outcome&&['uncertain','unknown'].includes(outcome.status)&&outcome.messageId===claim.messageId
    &&outcome.mutationId===claim.mutationId&&!!saved.ownerEpoch&&outcome.ownerEpoch===saved.ownerEpoch;
  return {claim,outcome,epoch:saved.ownerEpoch??'',mode:claim.state==='prepared'&&pending?'pending'
    :same.length?'unknown':uncertain&&!terminal(claim)?'durability':claim.state==='prepared'?'unknown':'settled'};
}
function proof(value:Evidence,input:ThreadSendRecoveryInput) {
  const c=value.claim,o=value.outcome;
  return {target:input.target,routeVisit:input.routeVisit,transferId:c.transferId,fingerprint:c.fingerprint,
    messageId:c.messageId,commandId:c.commandId,mutationId:c.mutationId,state:c.state,epoch:value.epoch,mode:value.mode,
    outcome:o?{messageId:o.messageId,mutationId:o.mutationId,status:o.status,revision:o.revision??null,ownerEpoch:o.ownerEpoch??'',
      sequenceFloor:o.sequenceFloor??null,current:o.current?{revision:o.current.revision,token:o.current.token,pending:o.current.pending}:null}:null};
}
function operations(value:Evidence,complete:boolean):ThreadSendRecoveryOperation[] {
  if(!value.epoch)return [];
  if(value.mode==='pending')return ['commit','rollback'];
  if(value.mode==='durability')return ['retry'];
  if(value.mode!=='settled'||!complete)return [];
  return value.claim.state==='queued'?['finish']:value.claim.state==='failed'?['release']:[];
}
function message(value:Evidence):string {
  if(value.mode==='pending')return 'Saving this message was interrupted. Keep it queued or keep the draft.';
  if(value.mode==='durability')return 'Confirm the interrupted local save before continuing.';
  if(value.mode==='unknown')return 'The interrupted write does not match a known recovery choice. Refresh its status.';
  return value.claim.state==='queued'?'The message is queued. Finish saving it; newer draft edits will be kept.'
    :value.claim.state==='failed'?'The message was not queued. Keep the draft to continue editing.'
      :value.claim.state==='completed'?'This captured message was already queued.':'The failed capture was released.';
}
async function inspect(client:MobileDraftClient,native:Native,target:MobileOutboxThreadTarget) {
  const complete=await mobileOutboxRead(client,native),lookup=await mobileOutboxThreadTransferLookup(client,native,target);
  const values:Evidence[]=[],unknown:ThreadSendTransferClaim[]=[];
  for(const claim of lookup.claims){
    const fresh=await mobileOutboxThreadTransferStatus(client,native,target,claim.transferId);
    if(!fresh.claim){unknown.push(claim);continue}
    values.push(evidence(client,fresh.claim,fresh.outcome));
  }
  return {values,unknown,complete:complete&&lookup.complete&&mobileOutboxSnapshot(client).complete,
    conflicts:values.filter(value=>!terminal(value.claim)).length>1};
}
export async function mobileThreadSendRecoveryRead(client:MobileDraftClient,handle:Native|null|undefined,input:ThreadSendRecoveryInput):Promise<ThreadSendRecoveryView> {
  const view=(message:string,complete=false,items:ThreadSendRecoveryItem[]=[],blocksSend=true):ThreadSendRecoveryView=>({revision:client.revision,
    scope:canonical(input.target),routeVisit:input.routeVisit,complete,blocksSend,busy:mobileThreadTransferBusy(client,input.target),message,items});
  if(!client.preferencesLoaded)return view('Wait for saved drafts to load.');
  try {
    const {native,target,check}=scope(client,input,handle),found=await inspect(client,native,target);check();
    const enabled=!found.conflicts&&!found.unknown.length&&!mobileThreadTransferBusy(client,target);
    const items=found.values.map(value=>({transferId:value.claim.transferId,state:value.claim.state,message:message(value),
      actions:enabled?operations(value,found.complete).map(kind=>({kind,label:labels[kind],key:canonical({...proof(value,input),kind})})):[]}));
    const rows:ThreadSendRecoveryItem[]=[...items,...found.unknown.map(claim=>({transferId:claim.transferId,state:'unknown' as const,
      message:'The saved transfer outcome is unavailable. Refresh before changing this draft.',actions:[]}))];
    return view(found.conflicts?'More than one transfer owns this draft. Resolve its saved storage first.'
      :!found.complete||found.unknown.length?'Saved message ownership is incomplete. Only verified interrupted writes can be recovered.':'',
      found.complete&&!found.unknown.length,rows,!enabled||!found.complete||found.values.some(value=>!terminal(value.claim)));
  }catch(error){if(letGo(error))throw error;return view(error instanceof Error?error.message:String(error))}
}
export function mobileThreadSendRecoveryPresentation(view:ThreadSendRecoveryView|null) {
  const active=view?.items.filter(item=>!['completed','released'].includes(item.state))??[];
  return {visible:!!view&&(!view.complete||active.length>0||!!view.message),message:view?.message||active.map(item=>item.message).join('\n'),
    actions:view?.busy?[]:active.flatMap(item=>item.actions.map(({key,label})=>({key,label})))};
}
export async function mobileThreadSendRecoveryAction(client:MobileDraftClient,handle:Native|null|undefined,storage:Files,
  input:ThreadSendRecoveryInput,key:string):Promise<ThreadSendRecoveryResult> {
  const result=(message:string,transfer:ThreadTransferResult|null=null,released=false):ThreadSendRecoveryResult=>({revision:client.revision,message,released,transfer});
  if(!client.preferencesLoaded)return result('Wait for saved drafts to load.');
  try {
    const {native,check,target}=scope(client,input,handle);
    let requested:Record<string,unknown>;try{requested=JSON.parse(key)}catch{return result('Refresh this recovery before choosing an action.')}
    if(!requested||typeof requested!=='object'||Array.isArray(requested)||canonical(requested.target)!==canonical(target)||requested.routeVisit!==input.routeVisit)
      return result('This recovery action belongs to another thread visit.');
    if(mobileThreadTransferBusy(client,target))return result('This message transfer is already being recovered.');
    const found=await inspect(client,native,target);check();
    const chosen=found.values.find(value=>value.claim.transferId===requested.transferId);
    if(found.conflicts||found.unknown.length||!chosen||mobileThreadTransferBusy(client,target))return result('Refresh the saved transfer status before recovering.');
    const kind=requested.kind as ThreadSendRecoveryOperation;
    if(!operations(chosen,found.complete).includes(kind)||key!==canonical({...proof(chosen,input),kind}))
      return result('The saved recovery state changed. Refresh before choosing again.');
    if(kind==='release'){
      const release=mobileThreadTransferLease(client,target);if(!release)return result('This message transfer is already being recovered.');
      try {
        check();const released=await mobileOutboxReleaseFailedThreadTransfer(client,native,chosen.claim);check();
        if(!released)return result('The failed capture is still retained.');
        const fresh=await inspect(client,native,target);check();
        return fresh.complete&&fresh.values.some(value=>value.claim.transferId===chosen.claim.transferId&&value.claim.state==='released')
          ?result('',null,true):result('The capture was released. Refresh the remaining saved status.');
      }finally{release()}
    }
    let mutating=false;
    const admitted:Native={available:true,watch:topic=>native.watch(topic),async later(request){
      const r=obj(request);
      if(r.op==='mobileOutbox'&&['recover','completeTransfer'].includes(String(r.action)))mutating=true;
      const reply=await native.later(request);
      if(!mutating&&r.op==='mobileOutbox'&&r.action==='transferStatus'&&obj(reply).ok===true){
        const raw=obj(obj(reply).value);let claim=threadSendTransferDecodeClaim(raw.claim);
        // The shared owner retains a known queued/failed state over native's temporary
        // prepared projection. Its exact uncertain outcome, not that retained state,
        // authorizes only Retry; normalize that same allowed projection for the CAS.
        if(chosen.mode==='durability'&&claim.state==='prepared'&&['queued','failed'].includes(chosen.claim.state)
          &&canonical(immutable(claim))===canonical(immutable(chosen.claim)))claim={...claim,state:chosen.claim.state};
        if(canonical(proof(evidence(client,claim,raw.outcome as MobileOutboxOutcome|null),input))!==canonical(proof(chosen,input)))throw stale();
      }
      return reply;
    }};
    const guarded:Files={fs:{async mkdir(path){check();const value=await storage.fs.mkdir(path);check();return value},
      async readFile(path){check();const value=await storage.fs.readFile(path);check();return value},
      async atomicWriteFile(path,bytes){check();const value=await storage.fs.atomicWriteFile(path,bytes);check();return value}}};
    const transfer=await mobileThreadTransferResume(client,admitted,guarded,target,chosen.claim.transferId,kind==='finish'?undefined:kind);
    return result(kind==='rollback'&&transfer.status==='failed'&&transfer.claim?.state==='failed'?'':transfer.message,transfer);
  }catch(error){if(letGo(error))throw error;return result(error instanceof Error?error.message:String(error))}
}
