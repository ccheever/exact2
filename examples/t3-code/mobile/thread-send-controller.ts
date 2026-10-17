// Pinned365aa87982 ThreadComposer.handleSend / use-thread-composer-state.onSendMessage.
// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import type { MobileDraftClient } from './mobile-draft-recovery';
import type { MobileComposerTarget } from './composer-target';
import { mobileComposerContextSendSnapshot } from './composer-command-context';
import { mobileEditorContextTargetCurrent, mobileEditorOwner } from './composer-editor-owner';
import { mobileQueuedEditCurrent } from './queued-edit-state';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileVoiceBlocksSubmission } from './voice-data';
import { mobileComposerAttachmentPicking } from './composer-attachments';
import { mobileOutboxSnapshot } from './mobile-outbox';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { mobileThreadSendPlan, type ThreadSendSnapshot, type ThreadSendFacts, type ThreadSendPlan } from './thread-send-admission';
import { mobileThreadTransferSubmit, type ThreadTransferResult } from './thread-send-transfer';
import {mobileThreadMountedSendCurrent,type MountedSendAdmission} from './thread-send-mounted';
import { providersWithLimits } from './settings-usage-limits';
import { mobileUsageConfig } from './settings-usage-types';
import { fleet } from './shared/settings-b-fleet';
import { pendingRequests } from './shared/requests';
import { queueState } from './shared/composer-controls-queue';
import { activeRun, ClientError, type Native, type Files } from './shared/protocol';
import { arr, obj, str, type Obj } from './shared/domain';

export interface ThreadSendControllerInput {
  editor?:MountedSendAdmission;
  expectedOwner:string; alternate:boolean;
  /** Already-loaded mobile preferences, captured at tap. Never reread after an await. */
  preferences:{followUpBehavior:'queue'|'steer';planModeEnabled:boolean};
  /** Required producer facts: absence is not proof that an import/upload is idle. */
  activity:Pick<ThreadSendFacts,'contextImporting'|'pendingPastedText'|'uploadStates'|'uploadOwners'>;
}
export interface ThreadSendControllerReady {
  kind:'ready';target:MobileComposerTarget;snapshot:ThreadSendSnapshot;facts:ThreadSendFacts;
  plan:Exclude<ThreadSendPlan,{kind:'refused'}>;hasContent:boolean;
}
export type ThreadSendControllerRefusal={kind:'blocked';reason:string}|
  {kind:'separate';lane:'new-task'|'queued-edit'|'question'|'pending-creation'};
export type ThreadSendControllerRead=ThreadSendControllerReady|ThreadSendControllerRefusal;
export interface ThreadSendCommandContext extends ThreadSendControllerReady {now:number;current():boolean}
export interface ThreadSendLocalResult {accepted:boolean;message:string}
export interface ThreadSendLocalCommands {
  /** Invocation-owned callbacks must implement exact captured clear/card and duplicate policy.
   * They must not reinterpret the current draft after awaiting. No callback is retained here. */
  usageLimits(context:ThreadSendCommandContext,native:Native,storage:Files):Promise<ThreadSendLocalResult>;
  feedback(context:ThreadSendCommandContext,native:Native,storage:Files,reason?:string):Promise<ThreadSendLocalResult>;
}
export type ThreadSendControllerResult=ThreadSendControllerRefusal|
  {kind:'transfer';scope:MobileComposerTarget;result:ThreadTransferResult}|
  {kind:'local';scope:MobileComposerTarget;command:'usage-limits'|'feedback';result:ThreadSendLocalResult};
const blocked=(reason:string):ThreadSendControllerRefusal=>({kind:'blocked',reason});
const copy=<T>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
const plain=(value:unknown):value is Record<string,unknown>=>value!==null&&typeof value==='object'&&!Array.isArray(value)
  &&[Object.prototype,null].includes(Object.getPrototypeOf(value));
function json(value:unknown,parents=new Set<object>()):boolean {
  if(value===null||typeof value==='string'||typeof value==='boolean')return true;
  if(typeof value==='number')return Number.isFinite(value);
  if(!Array.isArray(value)&&!plain(value)||parents.has(value as object))return false;
  parents.add(value as object);
  const array=Array.isArray(value),keys=Reflect.ownKeys(value as object).filter(k=>!array||k!=='length');
  const valid=(!array||keys.length===value.length&&Array.from({length:value.length},(_,i)=>Object.hasOwn(value,i)).every(Boolean))
    &&keys.every(k=>{const d=Object.getOwnPropertyDescriptor(value,k)!;return typeof k==='string'&&d.enumerable&&'value'in d&&json(d.value,parents)});
  parents.delete(value as object);return valid;
}
function settings(client:MobileDraftClient,key:string) {
  const raw:unknown=client.local.composerControls;
  if(raw!==undefined&&(!plain(raw)||!json(raw)||raw.staged!==undefined&&!plain(raw.staged)))return null;
  const staged=plain(raw)&&plain(raw.staged)?raw.staged[key]:undefined;
  if(staged!==undefined&&(!plain(staged)||Object.keys(staged).some(k=>!['providerId','modelId','options','runtimeMode','interactionMode'].includes(k))))return null;
  const value=staged===undefined?{providerId:client.providerId,modelId:client.modelId,options:client.modelOptions,
    runtimeMode:client.runtimeMode,interactionMode:client.interactionMode}:staged as Record<string,unknown>;
  if(!json(value)||!['providerId','modelId','runtimeMode','interactionMode'].every(k=>typeof value[k]==='string')||!Array.isArray(value.options))return null;
  return {modelSelection:{instanceId:value.providerId as string,model:value.modelId as string,options:copy(value.options) as ThreadSendSnapshot['modelSelection']['options']},
    runtimeMode:value.runtimeMode as ThreadSendSnapshot['runtimeMode'],interactionMode:value.interactionMode as ThreadSendSnapshot['interactionMode']};
}
/** No enrollment, normalization, context observation, persistence or IO. Strict whole-store
 * snapshot precedes any target helper that could initialize malformed legacy metadata. */
export function mobileThreadSendRead(client:MobileDraftClient,input:ThreadSendControllerInput):ThreadSendControllerRead {
  try {
    if(!client.threadId||client.threadId.startsWith('new:'))return {kind:'separate',lane:'new-task'};
    if(mobileQueuedEditCurrent(client))return {kind:'separate',lane:'queued-edit'};
    if(pendingRequests(client.projection).inputs.length)return {kind:'separate',lane:'question'};
    if(!client.preferencesLoaded)return blocked('Wait for saved drafts and preferences to load.');
    if(!plain(input)||typeof input.expectedOwner!=='string'||typeof input.alternate!=='boolean'
      ||!plain(input.preferences)||!json(input.preferences)||!['queue','steer'].includes(input.preferences.followUpBehavior)
      ||typeof input.preferences.planModeEnabled!=='boolean'||!plain(input.activity)||!json(input.activity)
      ||Object.keys(input.activity).some(k=>!['contextImporting','pendingPastedText','uploadStates','uploadOwners'].includes(k)))return blocked('The current Send preferences or activity are unavailable.');
    const key=`${client.environmentId}:${client.threadId}`;
    const target:MobileComposerTarget={kind:'ordinary',origin:client.origin,environmentId:client.environmentId,generation:client.generation,
      projectId:client.projectId,threadId:client.threadId,key,editorOwner:key,editOwner:'',incarnation:'',
      owner:JSON.stringify(['ordinary',client.origin,client.environmentId,client.generation,key])};
    const saved=mobileComposerContextSendSnapshot(client,target);
    if(!saved||!mobileEditorContextTargetCurrent(client,target)||input.expectedOwner!==target.owner)return blocked('The ordinary draft changed or its saved inventory is invalid.');
    const origin=mobileQueuedEditOrigin(client).trim().replace(/\/+$/,'');
    if(mobileCacheCatalogIdentity(fleet.saved,target.environmentId)!==JSON.stringify([target.environmentId,origin]))return blocked('The saved environment identity changed.');
    const rich=mobileEditorOwner(client);
    if(input.editor?!mobileThreadMountedSendCurrent(client,target,input.editor)
      :rich&&rich.target.origin===target.origin&&rich.target.environmentId===target.environmentId&&rich.target.key===target.key)
      return blocked('Finish the active rich editor before sending.');
    const shell=client.shell.threads.find(t=>t.id===target.threadId);
    if(!shell||shell.projectId!==target.projectId)return blocked('The selected thread is unavailable.');
    const outbox=mobileOutboxSnapshot(client);
    const creation=(r:typeof outbox.rows[number]['record']|null)=>!!r?.creation&&r.origin===origin&&r.environmentId===target.environmentId&&r.threadId===target.threadId;
    if(outbox.rows.some(row=>creation(row.record))||outbox.transfers.some(claim=>creation(claim.record)))return {kind:'separate',lane:'pending-creation'};
    if(mobileComposerAttachmentPicking(client))return blocked('Finish attaching files before sending.');
    const selection=settings(client,key);
    if(!selection||!json(client.config))return blocked('The captured model or mode is invalid.');
    const provider=arr(client.config.providers).find(p=>p.instanceId===selection.modelSelection.instanceId);
    // Classification-only absence sentinel, never a wire provider or installed claim.
    // Source enables local feedback only from an actual provider row, not a saved
    // instance driver. Model unavailability still reads config.settings in admission.
    const driver=str(provider?.driver)||'unknown';
    const attachments:ThreadSendSnapshot['attachments']=saved.attachmentIds.map(id=>{
      const image=saved.images.find(row=>row.id===id);if(image)return {type:'image',id,image};
      const file=saved.files.find(row=>row.id===id);if(!file)throw Error('The ordered file is unavailable.');return {type:'file',id,file};
    });
    const snapshot:ThreadSendSnapshot={origin,environmentId:target.environmentId,projectId:target.projectId,threadId:target.threadId,draftKey:key,
      rawText:saved.text,...(saved.context===null?{}:{context:saved.context}),attachments,...selection,providerDriver:driver,
      activeProviderThreadId:str(shell.activeProviderThreadId)||null,showInteractionModeToggle:provider?.showInteractionModeToggle!==false};
    // Pinned hasProviderUsageLimits, reusing its exact provider filter and the
    // existing structural config adapter rather than the older shared notice filter.
    const usage=mobileUsageConfig(client.config);
    const offered=!!provider&&(providersWithLimits(usage.providers).some(p=>p.driver===driver)
      ||usage.usageLimitSources.some(source=>source.accounts.some(account=>account.driver===driver)
        ||source.error!==undefined&&source.accounts.length===0));
    const queue=queueState(client.projection);
    const facts:ThreadSendFacts={connected:client.connection==='connected',canOperate:client.scopes.includes('orchestration:operate'),
      pendingThreadCreation:false,queuedEdit:false,contextImporting:input.activity.contextImporting,pendingPastedText:input.activity.pendingPastedText,
      uploadStates:copy(input.activity.uploadStates),uploadOwners:copy(input.activity.uploadOwners),voiceBlocked:mobileVoiceBlocksSubmission(client),
      usageLimitsOffered:offered,planModeEnabled:input.preferences.planModeEnabled,activeThreadBusy:!!activeRun(client.projection),
      canSteerActiveTurn:!!queue.canSteer,followUpBehavior:input.preferences.followUpBehavior,
      ...(input.alternate?{followUpOverride:input.preferences.followUpBehavior==='queue'?'steer' as const:'queue' as const}:{}),config:copy(client.config)};
    const plan=mobileThreadSendPlan(snapshot,facts);
    if(plan.kind==='refused')return blocked(plan.reason);
    return {kind:'ready',target,snapshot:copy(snapshot),facts,plan,hasContent:!!saved.text.trim()||attachments.length>0};
  }catch{return blocked('The ordinary draft or its Send settings are invalid. Keep the draft.');}
}
/** Capture and invoke the accepted facade in the same synchronous turn. Handles/callbacks live
 * only in this invocation. Root route effects must still check their captured visit on return. */
export async function mobileThreadSendSubmit(client:MobileDraftClient,native:Native,storage:Files,
  input:ThreadSendControllerInput&{now:number;current():boolean},commands:ThreadSendLocalCommands):Promise<ThreadSendControllerResult> {
  if(!input.current())throw new ClientError('The thread visit changed before Send.','superseded');
  if(!Number.isFinite(input.now)||input.now<=0)return blocked('The submission time is unavailable.');
  const ready=mobileThreadSendRead(client,input);if(ready.kind!=='ready')return ready;
  if(!input.current())throw new ClientError('The thread visit changed before Send.','superseded');
  const {target,plan}=ready;
  if(plan.kind==='message')return {kind:'transfer',scope:target,result:await mobileThreadTransferSubmit(client,native,storage,
    {target,record:plan.record,now:input.now,current:input.current,...(input.editor?{editor:input.editor}:{})})};
  const context:ThreadSendCommandContext={...ready,now:input.now,current:input.current};
  const result=plan.kind==='usage-limits'?await commands.usageLimits(context,native,storage)
    :await commands.feedback(context,native,storage,plan.reason);
  return {kind:'local',scope:target,command:plan.kind,result};
}
