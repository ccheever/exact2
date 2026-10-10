// Inactive app-owned Thread rich editor runtime. No producer or root activation.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import { mobileComposerMenuSelection } from './composer-command-selection';
import { mobileThreadLocalUsageOpen } from './thread-send-local-commands';
import type { EditorUsageCommand, EditorLocalClear, EditorCommandReceipt } from './composer-editor-owner';
import type { T3Client } from './shared/client';
import { arr,obj,str,type Obj } from './shared/domain';
import { ClientError,reply,type Native,type Files } from './shared/protocol';
import { letGo } from './shared/let-go';
import {mobileEditorFilesSnapshot,mobileEditorFilesReady,mobileEditorFilesPrepare,mobileEditorFilesPublish,mobileEditorFilesCleanup,type EditorFileProjection} from './composer-file-runtime';
export {mobileEditorFilesReady,mobileEditorFilesRetry,mobileEditorFilesDelegateRetired} from './composer-file-runtime';
import {mobileThreadMountedSendConsume} from './thread-send-mounted';
import { mobileEditorRetirementAllowed, mobileEditorRetirementReceipt, mobileEditorRetirementComplete, mobileEditorDocumentWritten } from './composer-editor-persistence';
import { fleet } from './shared/settings-b-fleet';
import { mobileDraftAttachmentsOrdered } from './draft-attachment-order';
import { draftFiles } from './shared/composer-editor-files';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { stage } from './shared/composer-controls';
import { messageContext,workspaceCwd } from './shared/composer-editor';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { mobileComposerTarget,mobileComposerTargetCurrent } from './composer-target';
import { mobileComposerContextRead,mobileComposerContextMountedRead } from './composer-command-context';
import { mobileNewTaskContextProject,type MobileMessageContext } from './mobile-new-task-context';
import { mobileComposerDocument,formatComposerContextReference,mobileComposerFileIcon,type ComposerInlineToken } from './composer-editor-document';
import { mobileComposerTrigger,mobileComposerCommandRows,replaceTextRange,
  resolveProviderSkillsForCwd,mobileComposerThreadRecord,mobileComposerPullRequestRecord,
  type ComposerCommandItem,type ComposerTrigger } from './composer-command-model';
import { mobileComposerCommandPresentation,type ComposerCommandPopover } from './composer-command-presentation';
import { mobileComposerProvider,mobileComposerQueryDemand,mobileComposerQuerySnapshot,mobileComposerQueryPrepare,
  type ComposerQueryDemand,type ComposerQueryInput,type ComposerQueryLane } from './composer-command-query';
import { mobileComposerEditorAccept,mobileComposerEditorDecodeEvent,mobileComposerEditorStageEffect,mobileComposerEditorCommitted,
  mobileComposerEditorControlled,mobileComposerEditorCommand,type ComposerEditorDocument,type ComposerEditorEffect } from './composer-editor-state';
import { editorCopy,mobileEditorOwner,mobileEditorOwnerAdmit,mobileEditorOwnerRevision,mobileEditorOwnerChanged,mobileEditorCompleteMountedLocalClear,mobileEditorCaptureIntent,
  mobileEditorIntentCurrent,mobileEditorClaimEffect as claimOwnerEffect,type EditorOwner,type EditorRouteInput,type EditorRootEffect,type EditorIntentCapture } from './composer-editor-owner';
export { mobileEditorCaptureIntent,mobileEditorPublishCommitted,mobileEditorCaptureDocumentIntent,mobileEditorCommitDocumentIntent } from './composer-editor-owner';
export type { EditorRouteInput,EditorIntentCapture } from './composer-editor-owner';

export interface EditorPresentation {
  themeJson:string; placeholder:string; fontSize:number; lineHeight:number;
  enterBehavior:'send'|'newline'; fontFamily?:string; contentInsetVertical?:number; scrollEnabled?:boolean; autoCorrect?:boolean; spellCheck?:boolean;
  textPasteThresholdBytes?:number; maxInputChars?:number; submitTitle?:string; alternateSubmitTitle?:string; iconUris:Record<string,string>;
  usageKey?:string; hasCompactableConversation:boolean; offersUsageLimits:boolean; allowInteractionMode:boolean;
  repository:string; permissionRevision:string; session?:Obj;
}
export interface EditorWake { key:string; dueAt:number; delayMs:number }
export interface EditorProjection {
  files:EditorFileProjection; enabled:boolean; admission:string; revision:number; pendingCommandKey:string; configuration:string; menu:ComposerCommandPopover; menuRevision:string;
  queries:{immediateKey:string;immediateAt:number;immediateClockOffset:number;path:EditorWake;pullRequests:EditorWake;discovery:EditorWake};
  effect:EditorRootEffect|null; message:string;
}
export interface EditorResult { revision:number; message:string; effect:EditorRootEffect|null; admission:string; command?:{id:string;applied:boolean;contentCleared?:boolean} }
interface Runtime {
  owner:EditorOwner; provider:ComposerQueryInput['provider']; presentation:EditorPresentation; confirmed:readonly ComposerInlineToken[];
  query:ComposerQueryDemand; trigger:ComposerTrigger|null; items:ComposerCommandItem[]; menuRevision:string;
  wakes:Record<ComposerQueryLane,EditorWake>; immediateKey:string; immediateAt:number;immediateClockOffset:number; catalog:string;
}
const runtimes=new WeakMap<T3Client,Runtime>();
const emptyWake=():EditorWake=>({key:'',dueAt:0,delayMs:0});
const closed=():ComposerCommandPopover=>({admission:'',visible:false,title:'',empty:'',rows:[]});
const superseded=()=>new ClientError('The composer changed. Try again in the current draft.','superseded');
const mounted=(owner:EditorOwner)=>({...owner.state.identity,mountId:owner.state.mountId});
function provider(client:T3Client):ComposerQueryInput['provider'] {
  const value=arr(client.config.providers).find(p=>p.instanceId===client.providerId);
  return mobileComposerProvider(value);
}
function current(client:T3Client,runtime:Runtime):boolean {
  return runtimes.get(client)===runtime && mobileEditorOwner(client)===runtime.owner && runtime.owner.route.active
    && mobileComposerTargetCurrent(client,runtime.owner.target)
    && runtime.catalog===mobileCacheCatalogIdentity(fleet.saved,client.environmentId);
}
function requireCurrent(client:T3Client,runtime:Runtime):void {if(!current(client,runtime))throw superseded()}
function context(client:T3Client,owner:EditorOwner,value=owner.state.value,added?:Obj|Obj[]):MobileMessageContext|undefined {
  const saved=mobileComposerContextMountedRead(client,owner.target);if(!saved.ok)throw new ClientError('This draft contains unsupported or invalid context. Keep the original draft.','retained');
  const records=new Map<string,Obj>();
  for(const record of [...arr(messageContext(client,value)?.records),...(saved.context?.records??[]),...(added?Array.isArray(added)?added:[added]:[])])records.set(str(record.contextId),record);
  return records.size?{version:1,records:[...records.values()]}:undefined;
}
function clipboardAttachments(client:T3Client,key:string) {
  return mobileDraftAttachmentsOrdered(client,key,[...(client.local.snapshotDrafts[key]??[]).map(image=>({id:str(image.id),
    uploadedAttachmentId:str(image.uploadId)||undefined,uploadEnvironmentId:str(image.uploadId)?client.environmentId:undefined})),
    ...draftFiles(client.local).filter(file=>file.draftKey===key&&file.environmentId===client.environmentId).map(file=>({id:file.id,
      uploadedAttachmentId:file.attachmentId,uploadEnvironmentId:file.attachmentId?file.environmentId:undefined}))]);
}
function document(client:T3Client,runtime:Runtime,value=runtime.owner.state.value,added?:Obj|Obj[],displayOnly=false) {
  const p=runtime.presentation,source=runtime.provider??provider(client),cwd=workspaceCwd(client);
  let records:MobileMessageContext|undefined;
  try{records=context(client,runtime.owner,value,added)}catch(error){
    if(!displayOnly)throw error;runtime.owner.error=error instanceof Error?error.message:'This draft context is unavailable.';
  }
  return mobileComposerDocument({value,context:records,environmentId:client.environmentId,
    attachments:clipboardAttachments(client,runtime.owner.target.key),skills:source?resolveProviderSkillsForCwd(source,cwd):[],confirmedTokens:runtime.confirmed,
    iconUri:path=>p.iconUris[mobileComposerFileIcon(path)]??null});
}
function reconcile(client:T3Client,runtime:Runtime,now:number,wallTime=now):void {
  requireCurrent(client,runtime);
  const owner=runtime.owner,p=runtime.presentation,state=owner.state,cwd=workspaceCwd(client);
  const trigger=mobileComposerTrigger(state.value,state.selection,!owner.route.readOnly&&!owner.route.voiceBusy&&state.focused,state.composing);
  runtime.trigger=trigger;
  const selected=provider(client);
  const input:ComposerQueryInput={...mounted(owner),documentRevision:state.eventCount,prompt:state.value,
    active:!!state.mountId&&!owner.route.readOnly&&!owner.route.voiceBusy,environmentId:client.environmentId,cwd,projectId:client.projectId,
    repository:p.repository,provider:selected,trigger,permissionRevision:p.permissionRevision,...(p.session?{session:p.session}:{})};
  runtime.query=mobileComposerQueryDemand(client,input,now);
  const query=mobileComposerQuerySnapshot(client,runtime.query.admission,now);
  runtime.provider=query.provider??selected;
  runtime.items=mobileComposerCommandRows({trigger,selectedProviderStatus:query.provider??selected,projectCwd:cwd||null,hasThread:true,
    hasCompactableConversation:p.hasCompactableConversation,offersUsageLimits:p.offersUsageLimits,allowInteractionMode:p.allowInteractionMode,
    environmentId:client.environmentId,currentThreadId:client.threadId,threadShells:client.shell.threads.map(t=>({environmentId:client.environmentId,
      id:str(t.id),title:str(t.title),updatedAt:str(t.updatedAt),archivedAt:typeof t.archivedAt==='string'?t.archivedAt:null})),
    pathEntries:query.pathEntries,pullRequestEntries:query.pullRequests});
  runtime.menuRevision=JSON.stringify([owner.admission,runtime.query.admission,state.eventCount,trigger,runtime.items,p.allowInteractionMode]);
  const identity=JSON.stringify(mounted(owner));
  const identities:Record<ComposerQueryLane,string>={
    path:JSON.stringify([identity,'path',client.environmentId,cwd,trigger?.kind==='path'?trigger.query.trim():'',p.permissionRevision]),
    pullRequests:JSON.stringify([identity,'pullRequests',trigger?.kind==='pull-request'?trigger.query:null]),
    discovery:JSON.stringify([identity,'discovery',runtime.query.discovery.key])};
  for(const lane of ['path','pullRequests','discovery'] as const){const due=runtime.query[lane];
    runtime.wakes[lane]=due.key&&due.delayMs>0?{key:identities[lane],dueAt:due.dueAt,delayMs:due.delayMs}:emptyWake()}
  const immediate=(['path','pullRequests','discovery'] as const).flatMap(lane=>{
    const due=runtime.query[lane];return due.key&&due.delayMs===0?[[lane,due.key]]:[]});
  const nextImmediate=immediate.length?JSON.stringify([runtime.query.admission,immediate]):'';
  if(nextImmediate!==runtime.immediateKey){runtime.immediateKey=nextImmediate;runtime.immediateAt=now;runtime.immediateClockOffset=now-wallTime}
}
function empty(client:T3Client):EditorProjection {
  return {files:mobileEditorFilesSnapshot(client),enabled:false,admission:'',revision:mobileEditorOwnerRevision(client),pendingCommandKey:'',configuration:'',menu:closed(),menuRevision:'',
    queries:{immediateKey:'',immediateAt:0,immediateClockOffset:0,path:emptyWake(),pullRequests:emptyWake(),discovery:emptyWake()},effect:null,message:''};
}
/** Pure IO-free projection may observe an event; only Action claims its effects.
 * Clocks are caller supplied. One clock domain needs no offset; an explicit second
 * clock can still describe conversion for a caller that actually owns both. */
export function mobileEditorSnapshot(client:T3Client,route:EditorRouteInput,rawLatch:string,presentation:EditorPresentation,now:number,wallTime=now):EditorProjection {
  const target=mobileComposerTarget(client),catalog=mobileCacheCatalogIdentity(fleet.saved,client.environmentId);
  const prior=runtimes.get(client),owner=mobileEditorOwnerAdmit(client,target,route,catalog);
  if(!owner){
    if(prior){mobileComposerQueryDemand(client,{...mounted(prior.owner),documentRevision:prior.owner.state.eventCount,prompt:prior.owner.state.value,
      active:false,environmentId:prior.owner.target.environmentId,cwd:'',projectId:'',repository:'',provider:null,trigger:null,permissionRevision:''},now)}
    runtimes.delete(client);return empty(client);
  }
  let runtime=prior;
  if(!runtime || runtime.owner!==owner){runtime={owner,provider:null,presentation:editorCopy(presentation),confirmed:[],query:{revision:0,admission:'',path:emptyWake(),pullRequests:emptyWake(),discovery:emptyWake()},
    trigger:null,items:[],menuRevision:'',wakes:{path:emptyWake(),pullRequests:emptyWake(),discovery:emptyWake()},immediateKey:'',immediateAt:now,immediateClockOffset:now-wallTime,catalog};runtimes.set(client,runtime)}
  runtime.presentation=editorCopy(presentation);
  if(rawLatch){const accepted=mobileComposerEditorAccept(owner.state,rawLatch);if(accepted.accepted)owner.state=accepted.state}
  reconcile(client,runtime,now,wallTime);
  let tokensJson='[]',clipboardFragment='';
  try {const doc=document(client,runtime,owner.state.value,undefined,true);runtime.confirmed=doc.confirmedTokens;tokensJson=doc.tokensJson;clipboardFragment=doc.clipboardFragment}
  catch(error){owner.error=error instanceof Error?error.message:'This draft context is unavailable.'}
  const files=mobileEditorFilesSnapshot(client);
  const query=mobileComposerQuerySnapshot(client,runtime.query.admission,now),control=mobileComposerEditorControlled(owner.state,tokensJson);
  // This increment dispatches commands through invocation-owned CAS, not render effects.
  const configuration=JSON.stringify({...control,command:null,active:route.active,editable:files.ready&&!route.readOnly&&!route.voiceBusy,readOnly:route.readOnly||route.voiceBusy,
    focusIntent:route.focusIntent,voiceOwner:target.editorOwner,presentation:{themeJson:presentation.themeJson,placeholder:presentation.placeholder,
      fontSize:presentation.fontSize,lineHeight:presentation.lineHeight,enterBehavior:presentation.enterBehavior,clipboardFragment,
      fontFamily:presentation.fontFamily??'DMSans-Regular',contentInsetVertical:presentation.contentInsetVertical??0,scrollEnabled:presentation.scrollEnabled??true,
      autoCorrect:presentation.autoCorrect??true,spellCheck:presentation.spellCheck??true,textPasteThresholdBytes:presentation.textPasteThresholdBytes??0,maxInputChars:presentation.maxInputChars??1_000_000,
      ...(presentation.submitTitle!==undefined?{submitTitle:presentation.submitTitle}:{}),...(presentation.alternateSubmitTitle!==undefined?{alternateSubmitTitle:presentation.alternateSubmitTitle}:{})}});
  const menu=mobileComposerCommandPresentation({admission:owner.admission,trigger:runtime.trigger?.kind??null,items:runtime.items,
    loading:runtime.trigger?.kind==='path'?query.pathPending:query.pullRequestsPending,error:runtime.trigger?.kind==='pull-request'?query.pullRequestsError||null:null,voiceBusy:route.voiceBusy});
  if(owner.dismissed===runtime.menuRevision)menu.visible=false;
  return {files,enabled:true,admission:owner.admission,revision:files.revision+client.revision+owner.state.revision+mobileEditorOwnerRevision(client)+query.revision,
    pendingCommandKey:owner.pending?.id??'',configuration,menu,menuRevision:runtime.menuRevision,queries:{immediateKey:runtime.immediateKey,immediateAt:runtime.immediateAt,immediateClockOffset:runtime.immediateClockOffset,...editorCopy(runtime.wakes)},effect:editorCopy(owner.effects[0]??null),message:owner.error};
}
function result(client:T3Client,runtime:Runtime,message=''):EditorResult {
  return {revision:client.revision+runtime.owner.state.revision+mobileEditorOwnerRevision(client),message,
    effect:editorCopy(runtime.owner.effects[0]??null),admission:runtime.owner.admission};
}
function rootEffect(owner:EditorOwner,staged:ComposerEditorEffect):void {
  if(!['focus','blur','submit'].includes(staged.event.kind))return;
  const effect:EditorRootEffect={id:staged.id,kind:staged.event.kind as EditorRootEffect['kind'],payload:JSON.stringify({...mounted(owner),eventCount:staged.event.eventCount,documentRevision:owner.document.revision,value:owner.state.value,alternate:staged.event.alternate??false})};
  if(effect.kind==='submit'){
    if(owner.effects.some(item=>item.kind==='submit'))owner.error='A previous submit action is still waiting. Keep this draft.';
    else owner.effects.push(effect);
  }else{owner.effects=owner.effects.filter(item=>item.kind==='submit');owner.effects.push(effect)}
}
/** Synchronous prefix reduces newest text and command effects before persistence awaits. */
async function consume(client:T3Client,runtime:Runtime,native:Native,storage:Files):Promise<EditorResult> {
  requireCurrent(client,runtime);const owner=runtime.owner,effect=owner.state.latestEffect,publication=owner.pending;
  const settledCurrent=()=>requireCommandCurrent(client,runtime,publication);
  if(owner.pending?.queuedSend&&mobileThreadMountedSendConsume(client)){
    if(effect)rootEffect(owner,effect);
    await client.persist(storage);requireCurrent(client,runtime);
    if(effect)owner.state=mobileComposerEditorCommitted(owner.state,effect);
    return result(client,runtime);
  }
  if(owner.pending?.localClear?.mode==='content'&&owner.state.commandEffect){
    const terminal=owner.state.commandEffect,pending=owner.pending;
    if(!mobileEditorCompleteMountedLocalClear(client).ok)throw new ClientError('The command context could not be safely cleared. Keep this draft.','retained');
    pending.outcome={id:pending.id,applied:terminal.event.kind==='commandApplied',terminal:editorCopy(terminal.event)};
    if(pending.receipt){pending.receipt.applied=pending.outcome.applied;pending.receipt.contentCleared=pending.localClear?.cleared??null}
    if(effect){owner.state=mobileComposerEditorStageEffect(owner.state,effect.id).state;rootEffect(owner,effect)}
    owner.state=mobileComposerEditorStageEffect(owner.state,terminal.id).state;owner.pending=null;
    await client.persist(storage);settledCurrent();
    if(effect)owner.state=mobileComposerEditorCommitted(owner.state,effect);
    return result(client,runtime);
  }
  let needsSave=false,staged:ComposerEditorEffect|null=null,prefixError:unknown,commandMessage='';
  try {
  // Durable terminal disposition is included in the same synchronous draft-save snapshot.
  const pendingRetirement=owner.pending, retirement=pendingRetirement?.retirementKey, outcome=owner.state.commandEffect;
  if(pendingRetirement&&retirement&&outcome&&outcome.event.commandId===pendingRetirement.id&&outcome.event.commandRevision===pendingRetirement.revision){
    mobileEditorRetirementComplete(client,retirement,outcome.event.kind==='commandApplied'?'retired':'preserved');needsSave=true;
  }
  const needsPublication=!!effect&&owner.state.stagedEventCount<effect.event.eventCount&&(effect.writeText||effect.event.kind==='text')
    ||!!outcome&&outcome.event.kind==='commandApplied'&&(!!owner.pending?.added||!!owner.pending?.picker&&!owner.pending.picker.published);
  if(needsPublication){
    const publication=mobileEditorFilesPublish(client);
    if(!publication.ok)throw new ClientError(publication.message,'retained');
    needsSave=true;
  }
  if(effect && owner.state.stagedEventCount<effect.event.eventCount){
    const claimed=mobileComposerEditorStageEffect(owner.state,effect.id);owner.state=claimed.state;staged=claimed.effect;
    if(staged&&!needsPublication){owner.document.selection={...owner.state.selection};mobileEditorDocumentWritten(client,owner.target,owner.document.value,owner.document.value,owner.state.selection)}
    if(staged)rootEffect(owner,staged);
  }
  const terminal=owner.state.commandEffect;
  if(terminal){
    const pending=owner.pending;
    if(!pending || pending.id!==terminal.event.commandId || pending.revision!==terminal.event.commandRevision)throw superseded();
    if(terminal.event.kind==='commandApplied'){
      if(pending.localCommand){
        const command=pending.localCommand;
        if(pending.settings===settings(client,runtime)&&command.usageKey===runtime.presentation.usageKey&&!owner.route.readOnly&&!owner.route.voiceBusy){
          const opened=mobileThreadLocalUsageOpen(client,{...command,scope:{origin:mobileQueuedEditOrigin(client).trim().replace(/\/+$/,''),
            environmentId:owner.target.environmentId,threadId:owner.target.threadId,draftKey:owner.target.key}});
          if(opened.message){owner.error=opened.message;commandMessage=opened.message;}
        }
      }
      if(pending.mode){
        if(pending.settings===settings(client,runtime)){stage(client,{interactionMode:pending.mode});client.revision++;needsSave=true}
        else owner.error='The model or mode changed before this selection finished. Keep the current choice.';
      }
    }
    pending.outcome={id:pending.id,applied:terminal.event.kind==='commandApplied',terminal:editorCopy(terminal.event)};
    if(pending.receipt){pending.receipt.applied=pending.outcome.applied;pending.receipt.contentCleared=pending.localClear?.cleared??null}
    const claimed=mobileComposerEditorStageEffect(owner.state,terminal.id);owner.state=claimed.state;if(claimed.effect)owner.pending=null;
  }
  }catch(error){prefixError=error}
  if(needsSave){await client.persist(storage);settledCurrent()}
  if(prefixError!==undefined)throw prefixError;
  if(staged)owner.state=mobileComposerEditorCommitted(owner.state,staged);
  return result(client,runtime,commandMessage);
}
function settings(client:T3Client,runtime:Runtime):string{return JSON.stringify([client.providerId,client.modelId,client.interactionMode,runtime.presentation.allowInteractionMode])}
function actionRuntime(client:T3Client,route:EditorRouteInput):Runtime {
  const runtime=runtimes.get(client),owner=runtime?.owner;
  if(!runtime || !owner || !route.active || route.routeVisit!==owner.route.routeVisit || route.editorId!==owner.route.editorId
    || route.environmentId!==owner.target.environmentId || route.threadId!==owner.target.threadId)throw superseded();
  requireCurrent(client,runtime);owner.route=editorCopy(route);return runtime;
}
/** After a local terminal has published, route-only navigation cannot redirect its
 * originating thread callback. Transport/catalog replacement still ends the invocation. */
function requireCommandCurrent(client:T3Client,runtime:Runtime,pending:EditorOwner['pending']):void {
  if(!pending?.localClear||!pending.outcome){requireCurrent(client,runtime);return}
  const target=runtime.owner.target;
  if(client.origin!==target.origin||client.environmentId!==target.environmentId||client.generation!==target.generation
    ||runtime.catalog!==mobileCacheCatalogIdentity(fleet.saved,target.environmentId))throw superseded();
}
async function invokeCommand(client:T3Client,runtime:Runtime,native:Native,storage:Files):Promise<EditorResult> {
  requireCurrent(client,runtime);const command=runtime.owner.state.pendingCommand;
  if(!command || !runtime.owner.pending)throw superseded();
  const pending=runtime.owner.pending;
  const response=reply(await native.later({op:'composerEditorApply',generation:runtime.owner.target.generation,identity:mounted(runtime.owner),command}));
  requireCommandCurrent(client,runtime,pending);
  if(!response.ok)throw new ClientError(response.error?.message||'The editor could not apply the change.',response.error?.kind||'protocol');
  if(response.generation!==runtime.owner.target.generation)throw superseded();
  const terminal=mobileComposerEditorDecodeEvent(obj(response.value).event);
  if(!terminal || !['commandApplied','commandRejected'].includes(terminal.kind) || terminal.commandId!==command.commandId || terminal.commandRevision!==command.commandRevision
    || terminal.owner!==command.owner || terminal.editorId!==command.editorId || terminal.routeVisit!==command.routeVisit
    || terminal.renderEpoch!==command.renderEpoch || terminal.mountId!==command.mountId)throw superseded();
  if(pending.outcome){
    const fields=(e:typeof terminal)=>[e.kind,e.commandId,e.commandRevision,e.eventCount,e.value,e.selection,e.composing,e.focused,e.reason??''];
    if(JSON.stringify(fields(pending.outcome.terminal))!==JSON.stringify(fields(terminal)))throw superseded();
  }
  const receipt=()=>({id:command.commandId,applied:terminal.kind==='commandApplied',
    ...(pending.localClear?.mode==='content'?{contentCleared:pending.localClear.cleared===true}:{})});
  if(pending.outcome)return {...result(client,runtime),command:receipt()};
  const accepted=mobileComposerEditorAccept(runtime.owner.state,terminal);
  if(accepted.accepted)runtime.owner.state=accepted.state;
  else {
    // A later reservation is possible only after this command's terminal was ACKed.
    const state=runtime.owner.state;
    if(state.ackCommandId===command.commandId || state.lastCommandRevision>command.commandRevision && state.issuedCommandIds.includes(command.commandId))return {...result(client,runtime),command:receipt()};
    throw superseded();
  }
  const answer=await consume(client,runtime,native,storage);return {...answer,command:receipt()};
}
/** Reserve all plain effects before native dispatch. No settings or context change
 * occurs merely because a caller requested a replacement. */
export async function mobileEditorRequestIntent(client:T3Client,capture:EditorIntentCapture,next:ComposerEditorDocument,
  added:Obj|undefined,native:Native,storage:Files,mode:'plan'|'default'|null=null,retirementKey='',localCommand?:EditorUsageCommand,localClear?:EditorLocalClear,receipt?:EditorCommandReceipt,picker?:import('./composer-command-context').ComposerPickerPublication):Promise<EditorResult> {
  const runtime=runtimes.get(client);if(!runtime || !mobileEditorIntentCurrent(client,capture))throw superseded();
  requireCurrent(client,runtime);const owner=runtime.owner;
  if(picker&&(added||mode||retirementKey||localCommand||localClear||picker.published))throw superseded();
  if(localClear&&(next.value!==''||next.selection.start!==0||next.selection.end!==0||added||mode||retirementKey||localCommand))throw superseded();
  if(retirementKey&&!mobileEditorRetirementAllowed(client,retirementKey))throw superseded();
  if(owner.route.readOnly || owner.route.voiceBusy || !mobileEditorFilesReady(client,owner.admission) || !native.available || owner.pending)throw new ClientError('The composer is not ready for this edit.','busy');
  const prospective=mobileNewTaskContextProject(next.value,context(client,owner,next.value,picker?.added??added));
  if(!prospective.ok)throw new ClientError(prospective.error,'retained');
  const doc=document(client,runtime,next.value,picker?.added??added),id=`${owner.state.identity.renderEpoch}-command-${++owner.serial}`,revision=owner.state.lastCommandRevision+1;
  const reserved=mobileComposerEditorCommand(owner.state,id,revision,{...next,tokensJson:doc.tokensJson});
  if(!reserved.command)throw superseded();
  if(receipt){receipt.id=id;receipt.applied=null;receipt.contentCleared=null}
  owner.pending={id,revision,...(picker?{picker}:{}),...(receipt?{receipt}:{}),...(localClear?{localClear:editorCopy(localClear)}:{}),...(localCommand?{localCommand:editorCopy(localCommand)}:{}),...(added?{added:editorCopy(added)}:{}),mode,settings:settings(client,runtime),intent:editorCopy(capture),...(retirementKey?{retirementKey}:{})};
  owner.state=reserved.state;
  return invokeCommand(client,runtime,native,storage);
}
export async function mobileEditorAction(client:T3Client,route:EditorRouteInput,
  action:'event'|'rich'|'pick'|'dismiss'|'retry',payload:string,native:Native,storage:Files,clock:()=>number):Promise<EditorResult> {
  const runtime=actionRuntime(client,route),owner=runtime.owner;
  try {
    if(action==='rich'){
      let callback:Obj;try{callback=obj(JSON.parse(payload))}catch{callback={}}
      if(callback.kind==='contentSize'){
        const size=obj(callback.payload),identity=mounted(owner);
        if(!identity.mountId||Object.entries(identity).some(([key,value])=>callback[key]!==value)
          ||!str(callback.richEventId)||!Number.isSafeInteger(callback.eventCount)||Number(callback.eventCount)<0
          ||typeof size.width!=='number'||!Number.isFinite(size.width)||size.width<0
          ||typeof size.height!=='number'||!Number.isFinite(size.height)||size.height<0)throw superseded();
        // Native layout observations are not paste/context requests. They never
        // publish a document, clear an existing notice or claim a pending effect.
        return {...result(client,runtime),effect:null};
      }
      throw new ClientError('Rich paste and context actions are not integrated yet.','unsupported');
    }
    if(action==='event'){
      const accepted=mobileComposerEditorAccept(owner.state,payload);if(!accepted.accepted)throw superseded();owner.state=accepted.state;
      return await consume(client,runtime,native,storage);
    }
    if(action==='retry'){
      if(payload!==owner.pending?.id)throw superseded();return await invokeCommand(client,runtime,native,storage);
    }
    reconcile(client,runtime,clock());
    if(action==='dismiss'){if(payload!==runtime.menuRevision)throw superseded();owner.dismissed=runtime.menuRevision;return result(client,runtime)}
    let requested:Obj;try{requested=obj(JSON.parse(payload))}catch{throw superseded()}
    if(requested.admission!==owner.admission || requested.menuRevision!==runtime.menuRevision || owner.dismissed===runtime.menuRevision)throw superseded();
    const item=runtime.items.find(row=>row.id===requested.id),trigger=runtime.trigger;
    if(!item || !trigger || owner.route.readOnly || owner.route.voiceBusy)throw superseded();
    let added:Obj|undefined,text='',cursor=0,mode:'plan'|'default'|null=null,localCommand:EditorUsageCommand|undefined;
    if(item.type==='thread' || item.type==='pull-request'){
      added=obj(item.type==='thread'?mobileComposerThreadRecord(item.thread,item.label)
        :mobileComposerPullRequestRecord(item.pullRequest,`pr_${owner.state.identity.renderEpoch.replace(/-/g,'_')}_${++owner.serial}`));
      const insertion=formatComposerContextReference({kind:str(added.kind),contextId:str(added.contextId),label:str(added.label)})+' ';
      ({text,cursor}=replaceTextRange(owner.state.value,trigger.rangeStart,trigger.rangeEnd,insertion));
    }else{
      const replacement=mobileComposerMenuSelection({draftMessage:owner.state.value,trigger,item,allowInteractionMode:runtime.presentation.allowInteractionMode,
        openUsageLimits:runtime.presentation.offersUsageLimits&&!!runtime.presentation.usageKey&&clipboardAttachments(client,owner.target.key).length===0});
      if(!replacement)throw superseded();({text,cursor}=replacement);mode=replacement.interactionMode;
      if(replacement.localCommand)localCommand={kind:'usage-limits',instanceId:client.providerId,usageKey:runtime.presentation.usageKey!,config:editorCopy(client.config),now:clock()};
    }
    const capture=mobileEditorCaptureIntent(client,owner.target,'suggestion');if(!capture)throw superseded();
    return await mobileEditorRequestIntent(client,capture,{value:text,selection:{start:cursor,end:cursor}},added,native,storage,mode,'',localCommand);
  }catch(error){if(letGo(error))throw error;requireCurrent(client,runtime);owner.error=error instanceof Error?error.message:'Could not update the composer.';mobileEditorOwnerChanged(client);return result(client,runtime,owner.error)}
}
/** Timer identity is source debounce identity; request scope is resolved only when
 * this still-current timer fires. PR project changes do not restart its query timer. */
export async function mobileEditorQueryWake(client:T3Client,key:string,native:Native,clock:()=>number):Promise<{revision:number}> {
  const runtime=runtimes.get(client);if(!runtime)throw superseded();requireCurrent(client,runtime);
  const lane=(['path','pullRequests','discovery'] as const).find(name=>runtime.wakes[name].key===key && !!key);
  if(!lane || clock()<runtime.wakes[lane].dueAt)return {revision:runtime.query.revision};
  reconcile(client,runtime,clock());
  // The wake can turn immediate at its deadline; its admitted request remains current.
  const answer=await mobileComposerQueryPrepare(client,runtime.query.admission,lane,runtime.query[lane].key,native,clock);
  requireCurrent(client,runtime);return answer;
}
export async function mobileEditorPrepareImmediate(client:T3Client,admission:string,key:string,native:Native,clock:()=>number):Promise<{revision:number}> {
  const runtime=runtimes.get(client);if(!runtime || runtime.owner.admission!==admission)throw superseded();requireCurrent(client,runtime);
  if(!key || key!==runtime.immediateKey)return {revision:runtime.query.revision};
  const expected=runtime.query.admission;const due=(['path','pullRequests','discovery'] as const).filter(lane=>runtime.query[lane].key&&runtime.query[lane].delayMs===0);
  // Independent reads share this invocation only. Await every result before returning.
  let abandoned:unknown;
  const check=()=>{if(abandoned!==undefined)throw abandoned;requireCurrent(client,runtime)};
  const guarded:Native={available:native.available,watch(topic){check();native.watch(topic)},async later(input){
    check();try{const value=await native.later(input);check();return value}catch(error){if(letGo(error)&&abandoned===undefined)abandoned=error;throw error}}};
  const results=await Promise.allSettled(due.map(lane=>mobileComposerQueryPrepare(client,expected,lane,runtime.query[lane].key,guarded,clock)));
  if(abandoned!==undefined)throw abandoned;
  for(const answer of results)if(answer.status==='rejected')throw answer.reason;
  requireCurrent(client,runtime);return {revision:mobileComposerQuerySnapshot(client,expected,clock()).revision};
}

/** Public root-effect claim rechecks actual target/catalog in addition to the leaf receipt. */
export function mobileEditorClaimEffect(client:T3Client,admission:string,id:string):EditorRootEffect|null {
  const runtime=runtimes.get(client);return runtime&&current(client,runtime)?claimOwnerEffect(client,admission,id):null;
}

/** New durable capability only; producer coverage currently refuses clear authority. */
export async function mobileEditorRequestRetirement(client:T3Client,receiptKey:string,route:EditorRouteInput,native:Native,storage:Files):Promise<EditorResult> {
  const runtime=actionRuntime(client,route),owner=runtime.owner,receipt=mobileEditorRetirementReceipt(client,receiptKey);
  if(!receipt||receipt.environmentId!==owner.target.environmentId||receipt.threadId!==owner.target.threadId)throw superseded();
  if(owner.pending?.retirementKey===receiptKey)return invokeCommand(client,runtime,native,storage);
  if(!mobileEditorRetirementAllowed(client,receiptKey))throw superseded();
  const capture=mobileEditorCaptureIntent(client,owner.target,'send-retirement');if(!capture)throw superseded();
  return mobileEditorRequestIntent(client,capture,{value:'',selection:{start:0,end:0}},undefined,native,storage,null,receiptKey);
}

/** Root schedules once per prepareKey, or explicitly retries. No connection-ready gate. */
export async function mobileEditorPrepareFiles(client:T3Client,admission:string,key:string,native:Native,storage:Files):Promise<EditorResult> {
  const runtime=runtimes.get(client);if(!runtime||runtime.owner.admission!==admission)throw superseded();requireCurrent(client,runtime);
  await mobileEditorFilesPrepare(client,admission,key,native);requireCurrent(client,runtime);
  if(mobileEditorFilesReady(client,admission)&&runtime.owner.state.latestEffect
    &&runtime.owner.state.stagedEventCount<runtime.owner.state.latestEffect.event.eventCount)return consume(client,runtime,native,storage);
  return result(client,runtime,mobileEditorFilesSnapshot(client).message);
}
export async function mobileEditorCleanupFiles(client:T3Client,key:string,native:Native,storage:Files):Promise<{revision:number}> {
  await mobileEditorFilesCleanup(client,key,native,storage);return {revision:mobileEditorFilesSnapshot(client).revision};
}
