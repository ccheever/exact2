// Source365aa87982 ThreadComposer.handleSend / use-thread-composer-state.onSendMessage.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import type { OrdinaryInventoryAttachment } from './composer-attachment-publication';
import type { MobileOutboxRecord, MobileOutboxModelSelection, MobileOutboxRuntimeMode } from './mobile-outbox-model';
import { mobileOutboxCanonicalOrigin } from './mobile-outbox-model';
import { mobileContextRecordValid } from './mobile-context-record';
import { mobileModelSelectionUnavailable } from './model-availability';
import { MAX_FILE_BYTES } from './shared/composer-editor-files';
import { isUsageLimitsCommand } from './shared/composer-controls-usage';
import { obj, type Obj } from './shared/domain';

export interface ThreadSendSnapshot {
  origin:string; environmentId:string; projectId:string; threadId:string; draftKey:string;
  rawText:string; context?:unknown;
  /** Complete ordered output of mobileComposerAttachmentInventoryRead. The caller must
   * validate the full owning stores first; this named snapshot cannot validate foreign rows. */
  attachments:OrdinaryInventoryAttachment[];
  modelSelection:MobileOutboxModelSelection; runtimeMode:MobileOutboxRuntimeMode;
  interactionMode:'default'|'plan'; providerDriver:string;
  activeProviderThreadId:string|null; showInteractionModeToggle:boolean;
}
export interface ThreadSendFacts {
  connected:boolean; canOperate:boolean; pendingThreadCreation:boolean; queuedEdit:boolean;
  contextImporting:boolean; voiceBlocked:boolean; pendingPastedText:boolean;
  usageLimitsOffered:boolean;
  /** Captured UI preference only: ordinary source preserves an existing Plan selection. */
  planModeEnabled:boolean;
  activeThreadBusy:boolean; canSteerActiveTurn:boolean;
  followUpBehavior:'queue'|'steer'|'restart'; followUpOverride?:'queue'|'steer'|'restart';
  config:Obj|null;
  /** States are scoped to the captured environment and keyed by canonical local ID. */
  uploadStates:Record<string,'uploading'|'ready'|'failed'>;
  /** An existing upload ID must have explicit saved origin/environment provenance. */
  uploadOwners:Record<string,{origin:string;environmentId:string}>;
}
export type ThreadSendRecord = Omit<MobileOutboxRecord,'messageId'|'commandId'|'createdAt'>;
export type ThreadSendPlan = {kind:'refused';reason:string} | {kind:'usage-limits'} |
  {kind:'feedback';reason?:string} | {kind:'message';record:ThreadSendRecord};
const copy=<T>(value:T):T=>JSON.parse(JSON.stringify(value)) as T;
const plain=(v:unknown):v is Record<string,unknown>=>v!==null&&typeof v==='object'&&!Array.isArray(v)
  &&[Object.prototype,null].includes(Object.getPrototypeOf(v));
const nonempty=(v:unknown,max=Infinity):v is string=>typeof v==='string'&&v.length>0&&v.trim()===v&&v.length<=max;
const integer=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0;
const uuid=(v:unknown):v is string=>typeof v==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
const hasKeys=(value:Record<string,unknown>,keys:string[])=>Object.keys(value).every(key=>keys.includes(key));
function json(v:unknown,parents=new Set<object>()):boolean {
  if(v===null||typeof v==='string'||typeof v==='boolean')return true;
  if(typeof v==='number')return Number.isFinite(v);
  if(!Array.isArray(v)&&!plain(v)||parents.has(v as object))return false;
  parents.add(v as object);
  const valid=(!Array.isArray(v)||Object.keys(v).length===v.length&&Array.from({length:v.length},(_,i)=>Object.hasOwn(v,i)).every(Boolean))
    &&Object.values(v as object).every(child=>json(child,parents));
  parents.delete(v as object);return valid;
}
const refuse=(reason:string):ThreadSendPlan=>({kind:'refused',reason});
function fail(reason:string):never {throw new Error(reason)}
function validate(snapshot:ThreadSendSnapshot,facts:ThreadSendFacts):void {
  if(!plain(snapshot)||!hasKeys(snapshot,['origin','environmentId','projectId','threadId','draftKey','rawText','context','attachments',
    'modelSelection','runtimeMode','interactionMode','providerDriver','activeProviderThreadId','showInteractionModeToggle'])
    ||!mobileOutboxCanonicalOrigin(snapshot.origin)||!['environmentId','projectId','threadId'].every(k=>nonempty(snapshot[k]))
    ||snapshot.threadId.startsWith('new:')||snapshot.draftKey!==`${snapshot.environmentId}:${snapshot.threadId}`
    ||snapshot.draftKey.includes('~queued-edit~')||typeof snapshot.rawText!=='string'||snapshot.rawText.length>1_000_000
    ||!Array.isArray(snapshot.attachments)||!json(snapshot.attachments))fail('The ordinary draft snapshot is invalid.');
  const model=snapshot.modelSelection;
  if(!plain(model)||!hasKeys(model,['instanceId','model','options'])||!nonempty(model.instanceId)||!nonempty(model.model)
    ||model.options!==undefined&&(!Array.isArray(model.options)||!json(model.options)||!model.options.every(option=>plain(option)
      &&hasKeys(option,['id','value'])&&nonempty(option.id)&&(typeof option.value==='boolean'||nonempty(option.value))))
    ||!['approval-required','auto-accept-edits','auto','full-access'].includes(snapshot.runtimeMode)
    ||!['default','plan'].includes(snapshot.interactionMode)||!nonempty(snapshot.providerDriver)
    ||snapshot.activeProviderThreadId!==null&&!nonempty(snapshot.activeProviderThreadId)
    ||typeof snapshot.showInteractionModeToggle!=='boolean')fail('The captured model or mode is invalid.');
  if(!plain(facts)||!hasKeys(facts,['connected','canOperate','pendingThreadCreation','queuedEdit','contextImporting','voiceBlocked',
    'pendingPastedText','usageLimitsOffered','planModeEnabled','activeThreadBusy','canSteerActiveTurn','followUpBehavior','followUpOverride',
    'config','uploadStates','uploadOwners'])||!['connected','canOperate','pendingThreadCreation','queuedEdit','contextImporting','voiceBlocked',
      'pendingPastedText','usageLimitsOffered','planModeEnabled','activeThreadBusy','canSteerActiveTurn'].every(k=>typeof facts[k]==='boolean')
    ||!['queue','steer','restart'].includes(facts.followUpBehavior)
    ||facts.followUpOverride!==undefined&&!['queue','steer','restart'].includes(facts.followUpOverride)
    ||facts.config!==null&&(!plain(facts.config)||!json(facts.config))||!plain(facts.uploadStates)||!json(facts.uploadStates)
    ||!Object.values(facts.uploadStates).every(v=>['uploading','ready','failed'].includes(v))
    ||!plain(facts.uploadOwners)||!json(facts.uploadOwners)||!Object.values(facts.uploadOwners).every(v=>plain(v)
      &&hasKeys(v,['origin','environmentId'])&&mobileOutboxCanonicalOrigin(v.origin)&&nonempty(v.environmentId)))fail('The send admission facts are invalid.');
}
function attachments(snapshot:ThreadSendSnapshot,facts:ThreadSendFacts):MobileOutboxRecord['attachments'] {
  const ids=new Set<string>();
  return snapshot.attachments.map(item=>{
    if(!plain(item)||!['image','file'].includes(item.type)||!hasKeys(item,['type','id',item.type])||!uuid(item.id)
      ||ids.has(item.id.toLowerCase()))fail('The attachment inventory is invalid.');
    ids.add(item.id.toLowerCase());
    const candidate:unknown=item.type==='image'?item.image:item.file;
    if(!plain(candidate))fail('The attachment row is invalid.');
    const row=candidate;
    if(!plain(row)||row.id!==item.id||!nonempty(row.name,255)||!nonempty(row.mimeType,100)||!integer(row.sizeBytes))
      fail('The attachment metadata is invalid.');
    const uploadId=item.type==='image'?row.uploadId??'':row.attachmentId;
    if(typeof uploadId!=='string'||uploadId!==''&&!/^[a-z0-9_-]{1,128}$/i.test(uploadId))fail('The attachment upload identity is invalid.');
    if(uploadId&&(facts.uploadOwners[item.id]?.origin!==snapshot.origin||facts.uploadOwners[item.id]?.environmentId!==snapshot.environmentId))
      fail('The attachment upload owner is unresolved.');
    const upload=uploadId?{uploadEnvironmentId:snapshot.environmentId}:{};
    if(item.type==='image') {
      if(row.source!==undefined&&!plain(row.source))fail('The image source is invalid.');
      return {id:item.id,kind:'image' as const,name:row.name,mimeType:row.mimeType,sizeBytes:row.sizeBytes,
        uploadId,status:uploadId?'ready' as const:'staged' as const,...upload,...(row.source===undefined?{}:{source:copy(row.source) as Obj})};
    }
    if(row.draftKey!==snapshot.draftKey||row.environmentId!==snapshot.environmentId||!nonempty(row.contextId,128)
      ||typeof row.status!=='string'||!['staged','ready'].includes(row.status)||row.status==='ready'&&!uploadId
      ||['videoWidth','videoHeight'].some(k=>row[k]!==undefined&&(typeof row[k]!=='number'||!Number.isFinite(row[k])||Number(row[k])<=0)))
      fail('The file belongs to an invalid draft or upload state.');
    if(row.source!=='attached')fail('Save the pasted file bytes before queuing this message.');
    return {id:item.id,kind:'file' as const,name:row.name,mimeType:row.mimeType,sizeBytes:row.sizeBytes,
      uploadId,status:uploadId?'ready' as const:'staged' as const,...upload,contextId:row.contextId,source:row.source,
      ...(row.videoWidth===undefined?{}:{videoWidth:row.videoWidth as number}),...(row.videoHeight===undefined?{}:{videoHeight:row.videoHeight as number})};
  });
}
function context(snapshot:ThreadSendSnapshot,files:MobileOutboxRecord['attachments'],localUsage=false):Obj|undefined {
  const raw=snapshot.context;if(raw===undefined)return undefined;
  if(!plain(raw)||!hasKeys(raw,['version','records'])||!json(raw)||raw.version!==1||!Array.isArray(raw.records))fail('The draft context is invalid.');
  if(!localUsage&&raw.records.length>200)fail('Remove context items until there are at most 200.');
  if(!localUsage&&JSON.stringify(raw.records).length>16_000_000)fail('This draft has too much context to send.');
  const ids=new Set<string>();
  for(const record of raw.records) {
    if(!plain(record)||!mobileContextRecordValid(record as Obj)||ids.has(String(record.contextId)))fail('The draft context is invalid.');
    ids.add(String(record.contextId));
    if(!localUsage&&'attachmentId'in record) {
      const file=files.find(file=>file.id===record.attachmentId);
      if(!file||record.kind!==file.kind||['name','mimeType','sizeBytes'].some(k=>record[k]!==file[k as keyof typeof file]))
        fail('A context attachment does not match its canonical local file.');
    }
  }
  // Source admission keeps its supplied context, including unreferenced records.
  // No desktop reference pruning, contextId-to-file matching, or upload-ID aliasing.
  return copy(raw) as Obj;
}
/** Pure preparation only: no durable enrollment, native byte ownership, permission grant,
 * enqueue, upload, or draft-clear authority. Caller captures/rechecks those exact owners.
 * Message records preserve all live attachments once, in input order, and keep local IDs
 * in every context record; existing outbox upload/wire owners perform remote remapping. */
export function mobileThreadSendPlan(snapshot:ThreadSendSnapshot,facts:ThreadSendFacts):ThreadSendPlan {
  try {
    validate(snapshot,facts);
    const files=attachments(snapshot,facts),text=snapshot.rawText.trim();
    const localUsage=facts.usageLimitsOffered&&files.length===0&&isUsageLimitsCommand(text);
    const messageContext=context(snapshot,files,localUsage);
    if(files.length>100)return refuse('Remove attachments until there are at most 100.');
    if(facts.voiceBlocked||facts.pendingPastedText)return refuse('Finish voice input or attaching pasted text before sending.');
    // ThreadComposer handles this before onSendMessage, including its permission check.
    if(localUsage)return {kind:'usage-limits'};
    if(facts.connected&&!facts.canOperate)return refuse('This connection cannot send messages.');
    if(facts.pendingThreadCreation||facts.queuedEdit||facts.contextImporting)return refuse('Finish the current thread creation, queued edit, or context import before sending.');
    if(!text&&files.length===0)return refuse('Write a message or attach a file first.');
    const caps=obj(obj(facts.config?.environment).capabilities),limit=Number(obj(caps.fileAttachments).maxUploadBytes);
    const accepts=(file:MobileOutboxRecord['attachments'][number])=>caps.attachmentUploads===true
      &&(file.kind==='image'||caps.fileAttachments!==undefined&&file.sizeBytes<=Math.min(limit,MAX_FILE_BYTES));
    if(facts.connected&&files.some(file=>accepts(file)&&facts.uploadStates[file.id]==='failed'))return refuse('Retry or remove the failed attachment');
    if(facts.connected&&mobileModelSelectionUnavailable(facts.config,snapshot.modelSelection as unknown as Obj))
      return refuse('Antigravity model unavailable. Set it up on web or desktop, or choose another model.');
    if(files.length===0&&snapshot.providerDriver==='codex') {
      const match=/^\/feedback(?:\s+([\s\S]*))?$/iu.exec(text);
      if(match) {
        if(!facts.canOperate)return refuse('This connection cannot upload feedback.');
        if(snapshot.activeProviderThreadId===null)return refuse('Start a Codex thread before submitting feedback.');
        const reason=match[1]?.trim();return reason?{kind:'feedback',reason}:{kind:'feedback'};
      }
    }
    const running=facts.activeThreadBusy&&facts.canSteerActiveTurn;
    const alternate=facts.followUpOverride!==undefined&&facts.followUpOverride!==facts.followUpBehavior;
    // Exact resolveComposerDispatchMode source policy (restart is a supported preference).
    const action=!running?'auto':alternate?facts.followUpBehavior==='queue'?'steer':'queue':facts.followUpBehavior;
    const record:ThreadSendRecord={schemaVersion:1,origin:snapshot.origin,environmentId:snapshot.environmentId,threadId:snapshot.threadId,
      text,attachments:files,...(messageContext===undefined?{}:{context:messageContext}),modelSelection:copy(snapshot.modelSelection),
      runtimeMode:snapshot.runtimeMode,interactionMode:!snapshot.showInteractionModeToggle?'default':snapshot.interactionMode,
      ...(action==='auto'?{}:{dispatchMode:action==='queue'?'queue' as const:'auto' as const})};
    return {kind:'message',record};
  } catch(error) {return refuse(error instanceof Error?error.message:'The draft could not be prepared.');}
}
