// App-owned ordinary Thread adapter over the accepted native editor runtime.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type {MobileDraftClient} from './mobile-draft-recovery';
import {mobileEditorSnapshot,mobileEditorAction,mobileEditorClaimEffect,mobileEditorPrepareImmediate,mobileEditorQueryWake,
  mobileEditorPrepareFiles,mobileEditorCleanupFiles,type EditorPresentation,type EditorProjection,type EditorRouteInput} from './composer-editor-runtime';
import {mobileEditorFilesRetry,mobileEditorFilesDelegateRetired,mobileEditorFilesSnapshot} from './composer-file-runtime';
import {mobileEditorOwner,mobileEditorOwnerRevision} from './composer-editor-owner';
import {mobileComposerEditorDecodeEvent} from './composer-editor-state';
import {collectComposerInlineTokens,composerContextEditorTokens,mobileComposerFileIcon} from './composer-editor-document';
import {mobileComposerContextRead} from './composer-command-context';
import {mobileQueuedEditCurrent} from './queued-edit-state';
import {mobileQueuedEditOrigin} from './queued-edit-origin';
import {mobileVoiceBlocksSubmission} from './voice-data';
import {mobileThreadOutbox} from './thread-outbox';
import {mobileCacheCatalogIdentity} from './mobile-client-cache-catalog';
import {mobileCommandColors,mobileNativeComposerTheme} from './design';
import {providersWithLimits} from './settings-usage-limits';
import {mobileUsageConfig} from './settings-usage-types';
import {repositorySelector} from './shared/composer-editor-menu';
import {pendingRequests} from './shared/requests';
import {activeRun,ClientError,type Native,type Files} from './shared/protocol';
import {queueState} from './shared/composer-controls-queue';
import {arr,obj,str,visibleTurnItems,type Obj} from './shared/domain';
import {fleet} from './shared/settings-b-fleet';

export interface ComposerRootInput {
  active:boolean;visit:string;url:string;environmentId:string;threadId:string;editorId:string;
  preferencesReady:boolean;preferencesJSON:string;scheme:string;themeId:string;enterBehavior:string;
  focusSerial:string;focusAttempt:number;focusOperation:string;now:number;
}
export type ComposerRootOperation='event'|'rich'|'pick'|'dismiss'|'retry'|'prepare-files'|'cleanup-files'|'immediate'|'wake'|'claim-effect';
export interface ComposerRootAction {op:string;admission:string;key:string;payload:string}
export type ComposerRootProjection=Omit<EditorProjection,'effect'|'files'>&{
  files:Omit<EditorProjection['files'],'retiredAdmissions'|'revision'>;
  colors:ReturnType<typeof mobileCommandColors>;routeVisit:string;editorId:string;owner:string;effectId:string;effectKind:string;
};
export interface ComposerRootResult {
  revision:number;message:string;visit:string;owner:string;editorId:string;admission:string;effectId:string;effectKind:string;alternate:boolean;
}
interface State {input:ComposerRootInput;signature:string;admission:string}
const states=new WeakMap<MobileDraftClient,State>();
const superseded=()=>new ClientError('The composer visit changed.','superseded');
const catalog=(client:MobileDraftClient)=>mobileCacheCatalogIdentity(fleet.saved,client.environmentId);
const scope=(client:MobileDraftClient)=>JSON.stringify([client.origin,client.environmentId,client.threadId,client.generation,catalog(client)]);
const permission=(client:MobileDraftClient)=>JSON.stringify([client.connection,client.scopes]);
function preferences(input:ComposerRootInput):Obj|null {
  if(!input.preferencesReady)return null;
  try {const value=JSON.parse(input.preferencesJSON);
    if(!value||typeof value!=='object'||Array.isArray(value)||typeof value.planModeEnabled!=='boolean'
      ||!['queue','steer'].includes(value.followUpBehavior)||!['send','newline'].includes(input.enterBehavior))return null;
    return value as Obj;
  }catch{return null}
}
function admitted(client:MobileDraftClient,input:ComposerRootInput,prefs:Obj|null):boolean {
  if(!input.active||!prefs||!client.preferencesLoaded||!input.visit||!input.editorId||!input.threadId||input.threadId.startsWith('new:')
    ||input.environmentId!==client.environmentId||input.threadId!==client.threadId||mobileQueuedEditCurrent(client)
    ||pendingRequests(client.projection).inputs.length||!Number.isFinite(input.now)||input.now<0
    ||catalog(client)!==JSON.stringify([client.environmentId,mobileQueuedEditOrigin(client).trim().replace(/\/+$/,'')]))return false;
  try {if(!input.url.startsWith('/threads/'))return false;const url=new URL(input.url,'https://route.invalid'),parts=url.pathname.slice(1).split('/').map(decodeURIComponent);
    return parts.length===3&&parts[0]==='threads'&&parts[1]===input.environmentId&&parts[2]===input.threadId;
  }catch{return false}
}
function route(client:MobileDraftClient,input:ComposerRootInput):EditorRouteInput {
  return {active:admitted(client,input,preferences(input)),routeVisit:input.visit,editorId:input.editorId,
    environmentId:input.environmentId,threadId:input.threadId,readOnly:false,voiceBusy:mobileVoiceBlocksSubmission(client),
    focusIntent:{serial:input.focusSerial,attempt:Number.isSafeInteger(input.focusAttempt)&&input.focusAttempt>=0?input.focusAttempt:0,
      operation:input.focusOperation==='focus'||input.focusOperation==='blur'?input.focusOperation:'none'}};
}
function icons(client:MobileDraftClient,rawLatch:string):Record<string,string> {
  const result:Record<string,string>={},owner=mobileEditorOwner(client),incoming=mobileComposerEditorDecodeEvent(rawLatch);
  // Asset resolution grants no draft authority. Include newly observed text so its first
  // native projection has icons; actual event identity is admitted by the runtime below.
  for(const value of new Set([client.draft,owner?.state.value??'',incoming?.value??''])){
    const saved=mobileComposerContextRead(client,client.draftKey,value);
    const records=saved.ok?saved.context?.records??[]:[];
    for(const token of composerContextEditorTokens(value,collectComposerInlineTokens(`${value} `))){
      const record=token.type==='context'?records.find(r=>r.contextId===token.contextId):undefined;
      const path=token.type==='mention'?token.value:record?.kind==='mention'?str(record.path):'';
      if(path){const key=mobileComposerFileIcon(path);result[key]=`t3-bundled-icon:${key}`;}
    }
  }
  return result;
}
function presentation(client:MobileDraftClient,input:ComposerRootInput,rawLatch:string):EditorPresentation {
  const prefs=preferences(input),provider=arr(client.config.providers).find(p=>p.instanceId===client.providerId);
  const shell=client.shell.threads.find(t=>t.id===input.threadId),project=client.shell.projects.find(p=>p.id===client.projectId);
  const requests=pendingRequests(client.projection),usage=mobileUsageConfig(client.config),driver=str(provider?.driver);
  const offersUsageLimits=!!provider&&(providersWithLimits(usage.providers).some(p=>p.driver===driver)
    ||usage.usageLimitSources.some(source=>source.accounts.some(account=>account.driver===driver)||source.error!==undefined&&source.accounts.length===0));
  const named=obj(client.projection.thread).id===input.threadId;
  const compactable=(named&&visibleTurnItems(client.thread).some(row=>{const m=obj(row.item);return m.type==='user_message'&&(arr(m.attachments).length>0||str(m.text).trim().toLowerCase()!=='/compact')}))
    ||mobileThreadOutbox(client,input.now).some(m=>m.record.attachments.length>0||m.record.text.trim().toLowerCase()!=='/compact')
    ||(named&&client.thread?.hasMore===true&&typeof shell?.latestUserMessageAt==='string');
  const running=!!activeRun(client.projection),follow=running&&queueState(client.projection).canSteer?str(prefs?.followUpBehavior):'queue';
  return {themeJson:JSON.stringify(mobileNativeComposerTheme(input.scheme,input.themeId)),placeholder:'Ask the repo agent, or run a command…',
    fontSize:16,lineHeight:23,fontFamily:'DMSans-Regular',enterBehavior:input.enterBehavior==='newline'?'newline':'send',
    iconUris:icons(client,rawLatch),hasCompactableConversation:compactable,offersUsageLimits,
    allowInteractionMode:provider?.showInteractionModeToggle!==false,
    repository:repositorySelector(obj(project?.repositoryIdentity)),
    permissionRevision:JSON.stringify([client.origin,client.generation,client.connection,client.scopes,catalog(client)]),
    usageKey:[`${input.environmentId}:${input.threadId}`,str(obj(shell?.modelSelection).instanceId),str(obj(shell?.latestRun).runId),
      requests.approvals[0]?.requestId??requests.inputs[0]?.requestId??''].join(':'),
    submitTitle:running?(follow==='steer'?'Steer':'Queue'):'Send',alternateSubmitTitle:running&&queueState(client.projection).canSteer?(follow==='steer'?'Queue':'Steer'):running?'Queue':'Send'};
}
function cleanupKey(view:EditorProjection):string {
  return view.files.retiredAdmissions.length?JSON.stringify([view.files.cleanupKey,view.files.retiredAdmissions]):view.files.cleanupKey;
}
/** Root must observe every route/catalog change before asynchronous resource work.
 * Native port identity is keyed by admission; retired native lifetime remains native-owned. */
export function mobileComposerRootSnapshot(client:MobileDraftClient,input:ComposerRootInput,rawLatch:string):ComposerRootProjection {
  input={...input};const prior=states.get(client),signature=scope(client);
  // Actions supply epochAtZero+now(); snapshots can still carry an older minute
  // tick. Keep the latest explicit observation only within the same live owner.
  if(prior?.admission&&prior.signature===signature&&prior.input.active&&input.active
    &&prior.input.visit===input.visit&&prior.input.editorId===input.editorId)input.now=Math.max(prior.input.now,input.now);
  const currentRoute=route(client,input),shown=mobileEditorSnapshot(client,currentRoute,rawLatch,presentation(client,input,rawLatch),input.now);
  states.set(client,{input,signature,admission:shown.admission});
  const {effect,files,...rest}=shown;
  return {...rest,files:{ready:files.ready,prepareKey:files.prepareKey,needsPrepare:files.needsPrepare,message:files.message,cleanupKey:cleanupKey(shown)},
    colors:mobileCommandColors(input.scheme,input.themeId),routeVisit:input.visit,editorId:input.editorId,
    owner:shown.enabled?mobileEditorOwner(client)?.target.owner??'':'',effectId:effect?.id??'',effectKind:effect?.kind??''};
}
/** Fresh native/storage handles belong to this call only. Root supplies its actual current
 * input plus the captured admission/key, never a callback or a synthetic native event. */
export async function mobileComposerRootAction(client:MobileDraftClient,input:ComposerRootInput,action:ComposerRootAction,nativeInput:Native|null|undefined,storage:Files):Promise<ComposerRootResult> {
  input={...input};action={...action};
  const prior=states.get(client);
  if(action.admission&&prior&&prior.admission!==action.admission)throw superseded();
  if(!['event','rich','pick','dismiss','retry','prepare-files','cleanup-files','immediate','wake','claim-effect'].includes(action.op))throw new ClientError('Unknown composer action.');
  const shown=mobileComposerRootSnapshot(client,input,''),saved=states.get(client)!,authorization=permission(client);
  const current=()=>{const latest=states.get(client);return !!latest&&latest.admission===saved.admission&&latest.signature===saved.signature
    &&latest.input.visit===input.visit&&latest.input.editorId===input.editorId&&scope(client)===saved.signature
    &&permission(client)===authorization&&route(client,latest.input).active};
  const answer=(message=''):ComposerRootResult=>({revision:client.revision+mobileEditorOwnerRevision(client)
      +(mobileEditorOwner(client)?.state.revision??0)+mobileEditorFilesSnapshot(client).revision,message:current()?message:'',visit:input.visit,
    owner:shown.owner,editorId:input.editorId,admission:shown.admission,effectId:'',effectKind:'',alternate:false});
  if(action.op==='cleanup-files'){
    if(!action.key||action.key!==shown.files.cleanupKey)throw superseded();
    // Root has committed to removing/replacing the retired keyed port. Native
    // lifetime keeps its holds until actual destruction; this drops only unusable
    // JS bookkeeping and never asserts that native release has happened.
    for(const retired of mobileEditorFilesSnapshot(client).retiredAdmissions)mobileEditorFilesDelegateRetired(client,retired);
    const key=mobileEditorFilesSnapshot(client).cleanupKey;
    if(key){if(!nativeInput?.available)return answer('Native file cleanup is unavailable.');
      await mobileEditorCleanupFiles(client,key,nativeInput,storage);}
    return answer();
  }
  if(!shown.enabled||!action.admission||action.admission!==shown.admission||!current())throw superseded();
  if(action.op==='claim-effect'){
    const effect=mobileEditorClaimEffect(client,action.admission,action.key),result=answer();
    if(effect){result.effectId=effect.id;result.effectKind=effect.kind;
      if(effect.kind==='submit')result.alternate=obj(JSON.parse(effect.payload)).alternate===true;}
    return result;
  }
  if(!nativeInput?.available)return answer('The native composer is unavailable.');
  const native=nativeInput,guarded:Native={available:native.available,watch(topic){if(!current())throw superseded();native.watch(topic)},async later(request){
    if(!current())throw superseded();const value=await native.later(request);if(!current())throw superseded();return value;}};
  // Invocation-only callback reads the latest admitted explicit root observation.
  // No ambient wall clock is available in captured source. Without another root
  // observation during slow IO, settlement time remains this known lower bound.
  const clock=()=>{if(!current())throw superseded();return states.get(client)!.input.now;};
  if(action.op==='prepare-files')return answer((await mobileEditorPrepareFiles(client,action.admission,action.key,guarded,storage)).message);
  if(action.op==='immediate')await mobileEditorPrepareImmediate(client,action.admission,action.key,guarded,clock);
  else if(action.op==='wake')await mobileEditorQueryWake(client,action.key,guarded,clock);
  else if(action.op==='retry'&&action.key==='files')mobileEditorFilesRetry(client,action.admission);
  else {
    const payload=action.op==='pick'?JSON.stringify({admission:action.admission,menuRevision:action.key,id:action.payload})
      :action.op==='dismiss'||action.op==='retry'?action.key:action.payload;
    const result=await mobileEditorAction(client,route(client,input),action.op as 'event'|'rich'|'pick'|'dismiss'|'retry',payload,guarded,storage,clock);
    return answer(result.message);
  }
  return answer();
}
