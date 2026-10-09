// Source365aa87982 ThreadComposer, ThreadDetailScreen and threadFeedback (MIT; LICENSE-T3).
// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import { collectProviderUsageLimits, type UsageLimitsReport } from './thread-local-usage-model';
import { codexFeedbackNotice, type CodexFeedbackSubmission } from './thread-local-feedback-model';
import { mobileThreadSendPlan, type ThreadSendSnapshot, type ThreadSendFacts } from './thread-send-admission';
import { mobileOutboxCanonicalOrigin } from './mobile-outbox-model';
import { arr, type Obj } from './shared/domain';
import { letGo } from './shared/let-go';

export interface ThreadLocalScope { origin:string; environmentId:string; threadId:string; draftKey:string }
export interface ThreadLocalCommandContext {
  snapshot:ThreadSendSnapshot; facts:ThreadSendFacts; now:number;
  /** Source ThreadDetail key: scoped thread, thread model instance, run and pending request. */
  usageKey:string;
  /** Invocation-only exact route/draft/catalog/generation admission. Never retained. */
  current():boolean;
  /** Starts the concrete clear synchronously, before returning its persistence promise.
   * Refusal must reject; a nonempty message reports an accepted edit whose save failed.
   * Text clear follows onChangeDraftMessage; content clear also discards draft context.
   * No attachment is admitted to this API. The parent supplies the actual reducer. */
  clearDraft(mode:'text'|'content'):Promise<{message:string}>;
  /** Captured environment/generation transport must reject replacement replies and preserve
   * letGo. A later route change alone must not redirect or reject the captured result. */
  uploadFeedback(payload:{threadId:string;reason?:string}):Promise<{feedbackId:string}>;
}
export type ThreadLocalCommandResult =
  | {kind:'not-local'|'refused'|'busy';message:string}
  | {kind:'usage-limits';message:string;opened:boolean}
  | {kind:'feedback';message:string;noticeId:string;status:'sent'|'failed'|'interrupted'};
export interface ThreadLocalPresentationInput {
  scope:ThreadLocalScope;usageKey:string;instanceId:string;config:Obj|null;userInputActive:boolean;
}
export interface ThreadLocalFeedbackNotice {
  id:string;status:'uploading'|'sent'|'failed';title:string;description:string;
  feedbackId:string;dismissible:boolean;createdAt:string;
}
interface Panel {scope:string;key:string;now:number}
interface State {sequence:number;revision:number;panel:Panel|null;feedback:Map<string,CodexFeedbackSubmission[]>;busy:Set<string>}
const states=new WeakMap<object,State>();
function state(owner:object):State {
  let value=states.get(owner);
  if(!value){value={sequence:0,revision:0,panel:null,feedback:new Map(),busy:new Set()};states.set(owner,value)}
  return value;
}
function scopeKey(scope:ThreadLocalScope):string {
  if(!mobileOutboxCanonicalOrigin(scope.origin)||typeof scope.environmentId!=='string'||!scope.environmentId.trim()
    ||typeof scope.threadId!=='string'||!scope.threadId.trim()||scope.threadId.startsWith('new:')
    ||scope.draftKey!==`${scope.environmentId}:${scope.threadId}`||scope.draftKey.includes('~queued-edit~'))return '';
  return JSON.stringify([scope.origin,scope.environmentId,scope.threadId,scope.draftKey]);
}
const report=(config:Obj|null,instanceId:string,now:number)=>collectProviderUsageLimits(instanceId,arr(config?.providers),arr(config?.usageLimitSources),now);
const errorMessage=(error:unknown)=>error instanceof Error?error.message:'An error occurred.';
function update(s:State,key:string,submission:CodexFeedbackSubmission) {
  const rows=s.feedback.get(key)??[],index=rows.findIndex(row=>row.id===submission.id);
  s.feedback.set(key,index<0?[...rows,submission]:rows.map((row,i)=>i===index?submission:row));s.revision++;
}
/** Reading a different source panel key retires it permanently. Feedback is per originating
 * thread and survives route changes; this projection cannot authorize a send or draft clear. */
export function mobileThreadLocalCommandsSnapshot(owner:object,input:ThreadLocalPresentationInput):{
  revision:number;usage:UsageLimitsReport|null;feedback:ThreadLocalFeedbackNotice[];busy:boolean;
} {
  const s=state(owner),key=scopeKey(input.scope);
  if(s.panel&&(s.panel.scope!==key||s.panel.key!==input.usageKey)){s.panel=null;s.revision++}
  const usage=s.panel&&!input.userInputActive?report(input.config,input.instanceId,s.panel.now):null;
  const feedback:ThreadLocalFeedbackNotice[]=[];
  for(const row of s.feedback.get(key)??[]){const notice=codexFeedbackNotice(row);if(!notice||row.status==='interrupted')continue;
    feedback.push({id:row.id,status:row.status,title:notice.title,description:notice.description??'',
      feedbackId:row.status==='sent'?row.feedbackId:'',dismissible:row.status!=='uploading',createdAt:row.createdAt});}
  return {revision:s.revision,usage:usage?JSON.parse(JSON.stringify(usage)) as UsageLimitsReport:null,feedback,busy:s.busy.has(key)};
}
export function mobileThreadLocalFeedbackDismiss(owner:object,scope:ThreadLocalScope,id:string):boolean {
  const s=state(owner),key=scopeKey(scope),rows=s.feedback.get(key),row=rows?.find(row=>row.id===id);
  if(!row||row.status==='uploading')return false;
  const next=rows!.filter(row=>row.id!==id);if(next.length)s.feedback.set(key,next);else s.feedback.delete(key);
  s.revision++;return true;
}
/** Parent invokes on explicit close, or a successful ordinary message while its captured
 * thread is still selected. Feedback/refused Send must not invoke this. Key prevents late close. */
export function mobileThreadLocalUsageDismiss(owner:object,scope:ThreadLocalScope,usageKey:string):boolean {
  const s=state(owner);if(!s.panel||s.panel.scope!==scopeKey(scope)||s.panel.key!==usageKey)return false;
  s.panel=null;s.revision++;return true;
}
/** Typed Send only. Command-menu /usage-limits has a different range replacement/clear order.
 * No outbox completion proof, Native, callback, or Promise enters the retained plain state. */
export async function mobileThreadSendLocalCommand(owner:object,ctx:ThreadLocalCommandContext):Promise<ThreadLocalCommandResult> {
  const plan=mobileThreadSendPlan(ctx.snapshot,ctx.facts);
  if(plan.kind==='message')return {kind:'not-local',message:''};
  if(plan.kind==='refused')return {kind:'refused',message:plan.reason};
  const scope:ThreadLocalScope={origin:ctx.snapshot.origin,environmentId:ctx.snapshot.environmentId,
    threadId:ctx.snapshot.threadId,draftKey:ctx.snapshot.draftKey},key=scopeKey(scope);
  if(!key||!Number.isFinite(ctx.now)||Math.abs(ctx.now)>8.64e15||typeof ctx.usageKey!=='string'||!ctx.usageKey
    ||ctx.snapshot.attachments.length!==0||!ctx.current())return {kind:'refused',message:'The selected composer changed. Try again.'};
  const s=state(owner);
  if(plan.kind==='usage-limits'){
    const collected=report(ctx.facts.config,ctx.snapshot.modelSelection.instanceId,ctx.now);
    s.panel=collected?{scope:key,key:ctx.usageKey,now:ctx.now}:null;s.revision++;
    if(!collected)return {kind:'usage-limits',opened:false,message:'This provider does not currently report limits.'};
    try {
      // Invocation starts clear in this turn; persistence cannot retarget a later draft.
      const saved=await ctx.clearDraft('text');return {kind:'usage-limits',opened:true,message:saved.message};
    }catch(error){if(letGo(error))throw error;return {kind:'usage-limits',opened:true,message:errorMessage(error)}}
  }
  if(s.busy.has(key))return {kind:'busy',message:''};
  if(!Number.isSafeInteger(s.sequence)||s.sequence>=Number.MAX_SAFE_INTEGER)return {kind:'refused',message:'Reopen the app before sending more feedback.'};
  const id=`feedback:${++s.sequence}`,submission={id,command:ctx.snapshot.rawText.trim(),createdAt:new Date(ctx.now).toISOString()};
  const payload={threadId:scope.threadId,...(plan.reason===undefined?{}:{reason:plan.reason})};
  s.busy.add(key);update(s,key,{...submission,status:'uploading'});
  try {
    const saved=await ctx.clearDraft('content');
    // Save failure is a truthful local failure, never an upload retry or draft rollback.
    if(saved.message){update(s,key,{...submission,status:'failed',errorMessage:saved.message});
      return {kind:'feedback',noticeId:id,status:'failed',message:saved.message};}
    if(!ctx.current()){update(s,key,{...submission,status:'interrupted'});
      return {kind:'feedback',noticeId:id,status:'interrupted',message:''};}
    const reply=await ctx.uploadFeedback(payload);
    if(typeof reply?.feedbackId!=='string'||!reply.feedbackId.trim())throw new Error('The feedback server returned an invalid response.');
    update(s,key,{...submission,status:'sent',feedbackId:reply.feedbackId});
    return {kind:'feedback',noticeId:id,status:'sent',message:''};
  }catch(error){
    if(letGo(error)){update(s,key,{...submission,status:'interrupted'});throw error;}
    const message=errorMessage(error);update(s,key,{...submission,status:'failed',errorMessage:message});
    return {kind:'feedback',noticeId:id,status:'failed',message};
  }finally{s.busy.delete(key);s.revision++;}
}
