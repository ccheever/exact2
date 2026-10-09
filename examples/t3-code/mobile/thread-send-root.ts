// App-local invocation adapter for pinned ThreadComposer/ThreadDetailScreen Send.
// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import type {MobileDraftClient} from './mobile-draft-recovery';
import {mobileThreadSendRead,mobileThreadSendSubmit,type ThreadSendControllerInput,type ThreadSendCommandContext} from './thread-send-controller';
import {mobileThreadSendRecoveryRead,mobileThreadSendRecoveryAction,mobileThreadSendRecoveryPresentation,type ThreadSendRecoveryView} from './thread-send-recovery';
import {mobileThreadTransferBusy} from './thread-send-transfer';
import {mobileThreadSendLocalCommand,mobileThreadLocalCommandsSnapshot,mobileThreadLocalFeedbackDismiss,mobileThreadLocalUsageDismiss,type ThreadLocalScope} from './thread-send-local-commands';
import {mobileThreadLocalCommandClear} from './thread-send-local-clear';
import {mobileThreadLocalDock,mobileThreadLocalResetKey,type ThreadLocalResetState} from './thread-local-dock';
import {mobileComposerContextSendSnapshot} from './composer-command-context';
import {mobileQueuedEditCurrent} from './queued-edit-state';
import {mobileQueuedEditOrigin} from './queued-edit-origin';
import {mobileOutboxSnapshot,type MobileOutboxThreadTarget} from './mobile-outbox';
import {mobileCacheCatalogIdentity} from './mobile-client-cache-catalog';
import {fleet} from './shared/settings-b-fleet';
import {queueState} from './shared/composer-controls-queue';
import {activeRun} from './shared/protocol';
import {pendingRequests} from './shared/requests';
import {obj,str,type Obj} from './shared/domain';
import {bridgeReply,ClientError,type Native,type Files} from './shared/protocol';
import {letGo,letGoAware} from './shared/let-go';

export interface ThreadSendRootInput {
  visit:string;url:string;active:boolean;environmentId:string;threadId:string;
  preferencesReady:boolean;preferencesJSON:string;contextImporting:boolean;
  now:number;viewportHeight:number;dark:boolean;
}
export interface ThreadSendRootAction {
  op:'send'|'send-alternate'|'recovery-read'|'recovery-action'|'feedback-dismiss'|'feedback-copy'|'usage-close'|'open-link'|'reset-credit';
  key:string;value:string;expectedOwner:string;visit:string;
}
interface Recovery {dirty:number;read:number;attempt:number;pending:number|null;view:ThreadSendRecoveryView|null;signature:string}
interface State {epoch:number;revision:number;route:string;input:ThreadSendRootInput|null;scope:string;recoveries:Map<string,Recovery>;resets:Map<string,ThreadLocalResetState>;panel:string}
const states=new WeakMap<object,State>();
const state=(client:object)=>{let s=states.get(client);if(!s){s={epoch:0,revision:0,route:'',input:null,scope:'',recoveries:new Map(),resets:new Map(),panel:''};states.set(client,s)}return s};
const copy=<T>(v:T):T=>JSON.parse(JSON.stringify(v)) as T;
const encoded=(v:unknown)=>JSON.stringify(v);
const stale=()=>new ClientError('The selected thread changed.','superseded');
function routeActive(client:MobileDraftClient,input:ThreadSendRootInput):boolean {
  if(!input.active||!input.visit||!input.threadId||input.threadId.startsWith('new:')||input.environmentId!==client.environmentId||input.threadId!==client.threadId)return false;
  try {const parts=new URL(input.url,'https://route.invalid').pathname.split('/').filter(Boolean).map(decodeURIComponent);
    return (parts.length===3||parts.length===4&&parts[3]==='files')&&parts[0]==='threads'&&parts[1]===input.environmentId&&parts[2]===input.threadId;
  }catch{return false}
}
function scope(client:MobileDraftClient):MobileOutboxThreadTarget {
  return {origin:mobileQueuedEditOrigin(client).trim().replace(/\/+$/,''),environmentId:client.environmentId,threadId:client.threadId,draftKey:`${client.environmentId}:${client.threadId}`};
}
function signature(client:MobileDraftClient,target:MobileOutboxThreadTarget):string {
  const v=mobileOutboxSnapshot(client),claims=v.threadTransfers.filter(c=>c.origin===target.origin&&c.environmentId===target.environmentId&&c.draftKey===target.draftKey);
  const ids=new Set(claims.map(c=>c.messageId));
  return encoded([v.ownerEpoch,v.complete,claims,v.outcomes.filter(o=>ids.has(o.messageId)),v.recovery.filter(r=>ids.has(str(obj(r).messageId)))]);
}
function observed(client:MobileDraftClient,input:ThreadSendRootInput) {
  const s=state(client),target=scope(client),active=routeActive(client,input),catalog=mobileCacheCatalogIdentity(fleet.saved,target.environmentId);
  const identity=encoded([input.visit,input.url,input.active,input.environmentId,input.threadId,client.origin,client.generation,target.origin,catalog]);
  const changed=s.route!==identity;
  if(changed){if(s.epoch>=Number.MAX_SAFE_INTEGER)throw new ClientError('Reopen this view before sending.');s.route=identity;s.epoch++;s.revision++}
  s.input={...input};const key=encoded(target);s.scope=key;
  let recovery=s.recoveries.get(key);
  if(!recovery){recovery={dirty:1,read:0,attempt:0,pending:null,view:null,signature:signature(client,target)};s.recoveries.set(key,recovery)}
  else if(changed){recovery.dirty++;recovery.view=null;recovery.attempt++;recovery.pending=null;s.revision++}
  const latest=signature(client,target);
  if(recovery.pending===null&&recovery.signature!==latest){recovery.signature=latest;recovery.dirty++;s.revision++}
  return {s,target,key,recovery,active};
}
function target(client:MobileDraftClient) {
  const key=`${client.environmentId}:${client.threadId}`;
  return {kind:'ordinary' as const,origin:client.origin,environmentId:client.environmentId,generation:client.generation,projectId:client.projectId,
    threadId:client.threadId,key,editorOwner:key,editOwner:'',incarnation:'',owner:encoded(['ordinary',client.origin,client.environmentId,client.generation,key])};
}
function controls(input:ThreadSendRootInput,expectedOwner:string,alternate=false):ThreadSendControllerInput|null {
  if(!input.preferencesReady||typeof input.contextImporting!=='boolean')return null;
  try {const p=obj(JSON.parse(input.preferencesJSON));if(!['queue','steer'].includes(str(p.followUpBehavior))||typeof p.planModeEnabled!=='boolean')return null;
    return {expectedOwner,alternate,preferences:{followUpBehavior:p.followUpBehavior as 'queue'|'steer',planModeEnabled:p.planModeEnabled},
      // Current plain textarea has no asynchronous pasted-text/eager-upload producer.
      // Rich owners are refused by controller. Existing uploaded IDs still lack proof
      // and are refused; this is not a fabricated upload-provenance map.
      activity:{contextImporting:input.contextImporting,pendingPastedText:false,uploadStates:{},uploadOwners:{}}};
  }catch{return null}
}
function local(client:MobileDraftClient,input:ThreadSendRootInput,active:boolean) {
  const shell=client.shell.threads.find(t=>t.id===client.threadId),requests=pendingRequests(client.projection);
  const instanceId=str(obj(shell?.modelSelection).instanceId);
  const usageKey=[`${client.environmentId}:${client.threadId}`,instanceId,str(obj(shell?.latestRun).runId),
    requests.approvals[0]?.requestId??requests.inputs[0]?.requestId??''].join(':');
  const targetScope:ThreadLocalScope=active?scope(client):{origin:'',environmentId:'',threadId:'',draftKey:''};
  const value=mobileThreadLocalCommandsSnapshot(client,{scope:targetScope,usageKey:active?usageKey:'',instanceId,config:client.config,userInputActive:requests.inputs.length>0});
  const s=state(client),panel=active&&value.usage?encoded([targetScope,usageKey]):'';
  if(s.panel!==panel){s.panel=panel;s.resets.clear();s.revision++}
  return {value:active?value:{...value,usage:null,feedback:[],busy:false},usageKey,instanceId};
}
const readKey=(s:State,key:string,r:Recovery)=>encoded(['read',s.epoch,key,r.dirty]);
/** Runs synchronously on every root route observation, before asynchronous resources. */
export function mobileThreadSendRootSnapshot(client:MobileDraftClient,input:ThreadSendRootInput) {
  const {s,target:scoped,key,recovery,active}=observed(client,input),named=target(client),control=controls(input,named.owner);
  const read=active&&control?mobileThreadSendRead(client,control):null;
  const ordinary=active&&!mobileQueuedEditCurrent(client)&&!pendingRequests(client.projection).inputs.length
    &&!(read?.kind==='separate');
  const saved=ordinary?mobileComposerContextSendSnapshot(client,named):null;
  const hasContent=!!saved&&(!!saved.text.trim()||saved.attachmentIds.length>0);
  const presentation=local(client,input,active);
  const busy=recovery.pending!==null||mobileThreadTransferBusy(client,scoped)||presentation.value.busy;
  const needsRead=ordinary&&input.preferencesReady&&client.preferencesLoaded&&(recovery.pending!==null||recovery.read<recovery.dirty);
  const known=recovery.view!==null&&recovery.read===recovery.dirty;
  const running=ordinary&&!!activeRun(client.projection),followUp=running&&queueState(client.projection).canSteer?control?.preferences.followUpBehavior:'queue';
  const sendLabel=running?(followUp==='steer'?'Steer':'Queue'):'Send',sendSymbol=running?(followUp==='steer'?'arrow.turn.left.up':'list.number'):'arrow.up';
  const canSend=ordinary&&read?.kind==='ready'&&known&&!recovery.view!.blocksSend&&!busy;
  const reason=!ordinary?'':!control?'Wait for the current Send preferences.':read?.kind==='blocked'?read.reason
    :!known?'Read saved message ownership before sending.':recovery.view!.blocksSend?recovery.view!.message||'Resolve the saved message transfer before sending.':busy?'Saving…':'';
  const recoveryView=mobileThreadSendRecoveryPresentation(known?recovery.view:null);
  if(ordinary&&(!known||recovery.view?.blocksSend)){
    recoveryView.visible=true;if(!recoveryView.message)recoveryView.message='Refresh saved message ownership before sending.';
    if(recovery.pending===null)recoveryView.actions.push({key:readKey(s,key,recovery),label:'Refresh saved status'});
  }
  return {revision:s.revision+client.revision+presentation.value.revision,ordinary,expectedOwner:named.owner,sendLabel,sendSymbol,canSend,hasContent,reason,busy,
    needsRead,readKey:needsRead?readKey(s,key,recovery):'',recovery:ordinary?recoveryView:{visible:false,message:'',actions:[]},
    dock:mobileThreadLocalDock(presentation.value,{usageKey:presentation.usageKey,viewportHeight:input.viewportHeight,dark:input.dark,
      canManageProviders:client.scopes.includes('providers:manage'),resetStates:[...s.resets.values()]})};
}
export interface ThreadSendRootResult {revision:number;message:string;accepted:boolean;stale:boolean}
/** Uses explicit current root input: Runner can execute this source before its dependent
 * projection settles. No Native, storage, callback, Promise or timer enters retained state. */
export async function mobileThreadSendRootAction(client:MobileDraftClient,nativeInput:Native,storage:Files,input:ThreadSendRootInput,
  action:ThreadSendRootAction):Promise<ThreadSendRootResult> {
  input={...input};action={...action};
  const {s,target:scoped,key,recovery,active}=observed(client,input),epoch=s.epoch,generation=client.generation,origin=client.origin;
  const named=target(client),catalog=mobileCacheCatalogIdentity(fleet.saved,scoped.environmentId);
  const routeCurrent=()=>s.epoch===epoch&&s.route!==''&&active&&s.input?.visit===input.visit&&routeActive(client,s.input)
    &&client.origin===origin&&client.generation===generation&&encoded(scope(client))===key
    &&mobileCacheCatalogIdentity(fleet.saved,scoped.environmentId)===catalog;
  const result=(message='',accepted=false):ThreadSendRootResult=>{s.revision++;return {revision:s.revision+client.revision+local(client,s.input??input,routeActive(client,s.input??input)).value.revision,message:routeCurrent()?message:'',accepted,stale:!routeCurrent()}};
  const transportCurrent=(permission:string)=>client.origin===origin&&client.generation===generation&&client.environmentId===scoped.environmentId
    &&mobileQueuedEditOrigin(client).trim().replace(/\/+$/,'')===scoped.origin&&mobileCacheCatalogIdentity(fleet.saved,scoped.environmentId)===catalog
    &&catalog===encoded([scoped.environmentId,scoped.origin])&&client.connection==='connected'&&client.scopes.includes(permission);
  const rawNative=letGoAware(nativeInput);
  const guarded=(check:()=>boolean):Native=>({available:rawNative.available,watch(topic){if(!check())throw stale();rawNative.watch(topic)},async later(request){
    if(!check())throw stale();const reply=await rawNative.later(request);if(!check())throw stale();return reply;
  }});
  const dirty=()=>{recovery.dirty++;recovery.signature=signature(client,scoped);s.revision++};
  const read=async()=>{
    if(recovery.pending!==null)return result();
    const attempt=++recovery.attempt,admitted=recovery.dirty;recovery.pending=attempt;recovery.read=admitted;recovery.view=null;s.revision++;client.revision++;
    try {const view=await mobileThreadSendRecoveryRead(client,rawNative,{target:scoped,routeVisit:input.visit,current:routeCurrent});
      if(routeCurrent()&&recovery.attempt===attempt){recovery.view=copy(view);recovery.signature=signature(client,scoped);s.revision++}
      return result(view.message);
    }finally{if(recovery.pending===attempt){recovery.pending=null;s.revision++;client.revision++}}
  };
  let transferWork=false;
  try {
    if(!active||action.visit!==input.visit||action.expectedOwner!==named.owner||!routeCurrent())throw stale();
    if(!client.preferencesLoaded)return result('Finish loading this ordinary draft before continuing.');
    if(['send','send-alternate','recovery-read','recovery-action'].includes(action.op)
      &&(mobileQueuedEditCurrent(client)||pendingRequests(client.projection).inputs.length))return result('This composer belongs to another submission lane.');
    if(action.op==='recovery-read'){
      if(action.key!==readKey(s,key,recovery))throw stale();return await read();
    }
    if(action.op==='recovery-action'){
      if(action.key===readKey(s,key,recovery)){dirty();return await read()}
      const response=await mobileThreadSendRecoveryAction(client,rawNative,storage,{target:scoped,routeVisit:input.visit,current:routeCurrent},action.key);
      dirty();return result(response.message,response.released||response.transfer?.status==='completed');
    }
    const shown=()=>local(client,s.input??input,true);
    if(action.op==='feedback-dismiss')return result('',mobileThreadLocalFeedbackDismiss(client,scoped,action.key));
    if(action.op==='usage-close')return result('',mobileThreadLocalUsageDismiss(client,scoped,action.key));
    if(action.op==='feedback-copy'){
      const matches=()=>routeCurrent()&&shown().value.feedback.some(row=>row.id===action.key&&row.status==='sent'&&row.feedbackId===action.value);
      if(!action.value||!matches())throw stale();
      const copied=await bridgeReply(guarded(matches),{op:'copyText',text:action.value});
      if(!copied.ok||obj(copied.value).copied!==true)return result('The feedback ID could not be copied.');
      const haptic=await bridgeReply(guarded(matches),{op:'mobileHomeHaptic',kind:'light'});
      if(!haptic.ok)return result(haptic.error?.message??'The feedback ID was copied.');return result('',true);
    }
    if(action.op==='open-link'){
      const matches=()=>{const p=shown();return routeCurrent()&&mobileThreadLocalDock(p.value,{usageKey:p.usageKey,viewportHeight:input.viewportHeight,dark:input.dark,
        canManageProviders:client.scopes.includes('providers:manage')}).accounts.some(a=>a.key===action.key&&a.externalURL===action.value)};
      const url=new URL(action.value);if(!['http:','https:'].includes(url.protocol)||!matches())throw stale();
      const opened=await bridgeReply(guarded(matches),{op:'mobileOpenURL',url:url.href});return result(opened.ok&&obj(opened.value).opened===true?'':'The usage link could not be opened.',opened.ok&&obj(opened.value).opened===true);
    }
    if(action.op==='reset-credit'){
      const selected=()=>{const presentation=shown();return presentation.value.usage?.accounts.find(a=>{
        const reset=a.resetCreditInput??(a.instanceId?{instanceId:a.instanceId}:null);
        return reset&&mobileThreadLocalResetKey(presentation.usageKey,a.id,reset)===action.key&&encoded(reset)===action.value&&Number(obj(a.limits.resetCredits).availableCount)>0;
      })};
      const check=()=>routeCurrent()&&transportCurrent('providers:manage')&&!!selected();
      if(!check())return result('This reset credit or provider permission changed.');
      if(s.resets.get(action.key)?.busy)return result();
      const progress={key:action.key,busy:true,status:''};s.resets.set(action.key,progress);s.revision++;client.revision++;
      try {
        const confirm=await bridgeReply(guarded(check),{op:'mobileScheduledConfirm',kind:'reset-credit'});
        if(!confirm.ok)throw new ClientError(confirm.error?.message??'The reset confirmation could not open.');
        if(obj(confirm.value).confirmed!==true)return result();
        if(!check())throw stale();
        const reply=await client.rpc(guarded(()=>routeCurrent()&&transportCurrent('providers:manage')),'provider.consumeResetCredit',obj(JSON.parse(action.value)),true);
        const messages:Record<string,string>={reset:'Reset applied. Your windows have cleared.',nothingToReset:'Nothing to reset right now.',noCredit:'No reset credit left.',alreadyRedeemed:'That credit was already redeemed.'};
        progress.status=str(reply.warning)||messages[str(reply.outcome)]||'The server returned an unknown reset-credit outcome.';return result(progress.status,true);
      }catch(error){if(letGo(error))throw error;progress.status=error instanceof Error?error.message:'Could not use the reset credit.';return result(progress.status)}
      finally{progress.busy=false;s.revision++;client.revision++}
    }
    if(!['send','send-alternate'].includes(action.op))return result('This composer action is unavailable.');
    const projection=mobileThreadSendRootSnapshot(client,input);
    if(!projection.canSend)return result(projection.reason||'The ordinary composer is not ready to send.');
    const control=controls(input,action.expectedOwner,action.op==='send-alternate');if(!control)return result('The current Send preferences are unavailable.');
    const usageKey=shown().usageKey;
    const command=async(ctx:ThreadSendCommandContext)=>{
      let didClear=false;
      const answer=await mobileThreadSendLocalCommand(client,{snapshot:ctx.snapshot,facts:ctx.facts,now:ctx.now,usageKey,current:routeCurrent,
        clearDraft:async mode=>{if(!routeCurrent())throw stale();const cleared=await mobileThreadLocalCommandClear(client,ctx.target,ctx.snapshot,mode,rawNative,storage);
          if(!cleared.applied)throw new ClientError(cleared.message);didClear=true;return {message:cleared.message};},
        uploadFeedback:async payload=>{if(!routeCurrent()||!transportCurrent('orchestration:operate'))throw stale();
          const reply=await client.rpc(guarded(()=>transportCurrent('orchestration:operate')),'provider.uploadFeedback',payload,true);
          return {feedbackId:str(reply.feedbackId)};}});
      return {accepted:didClear&&(answer.kind==='feedback'||answer.kind==='usage-limits'&&answer.opened),message:answer.message};
    };
    const planned=mobileThreadSendRead(client,control);transferWork=planned.kind==='ready'&&planned.plan.kind==='message';
    const answer=await mobileThreadSendSubmit(client,rawNative,storage,{...control,now:input.now,current:routeCurrent},{usageLimits:command,feedback:command});
    if(answer.kind==='transfer'){
      const accepted=['completed','cleanup-pending'].includes(answer.result.status);
      if(accepted&&routeCurrent())mobileThreadLocalUsageDismiss(client,scoped,usageKey);return result(answer.result.message,accepted);
    }
    if(answer.kind==='local')return result(answer.result.message,answer.result.accepted);
    return result(answer.kind==='blocked'?answer.reason:'This composer belongs to another submission lane.');
  }catch(error){if(letGo(error))throw error;return result(error instanceof Error?error.message:'The composer action could not finish.')}
  finally{if(transferWork)dirty()}
}
