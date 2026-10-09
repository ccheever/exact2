// Pinned365aa87982 ComposerFeedback, ComposerUsageLimits and UsageLimitsSection.
// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {elapsedShare,formatDuration,formatResetsIn,limitsNotice,paceOf,remainingPercent,
  type UsageLimitsReport,type ResetCreditInput} from './thread-local-usage-model';
import type {ThreadLocalFeedbackNotice} from './thread-send-local-commands';
import {arr,obj,str} from './shared/domain';

export interface ThreadLocalResetState {key:string;busy:boolean;status:string}
export interface ThreadLocalDockInput {
  usageKey:string;viewportHeight:number;dark:boolean;canManageProviders:boolean;
  resetStates?:readonly ThreadLocalResetState[];
}
/** UI identity only. Root must revalidate the originating scope, report and exact input
 * before native confirmation and again before spending a credit. */
export const mobileThreadLocalResetKey=(usageKey:string,accountId:string,input:ResetCreditInput)=>
  JSON.stringify([usageKey,accountId,input]);
const DRIVER_LABEL:Record<string,string>={codex:'Codex',claudeAgent:'Claude'};
const driverLabel=(driver:string)=>DRIVER_LABEL[driver]??driver;
const paceLabel={ahead:'ahead of pace',on:'on pace',under:'under pace'};
export function mobileThreadLocalDock(snapshot:{usage:UsageLimitsReport|null;feedback:readonly ThreadLocalFeedbackNotice[]},input:ThreadLocalDockInput) {
  const report=snapshot.usage,now=report?Date.parse(report.createdAt):0;
  const accounts=(report?.accounts??[]).map((account,index)=>{
    const label=driverLabel(account.driver),instanceLabel=account.instanceId
      ?account.displayName?.trim()||(account.instanceId!==account.driver?account.instanceId:label):account.label;
    const notice=limitsNotice(account.limits)??'',credits=obj(account.limits.resetCredits);
    const resetInput=account.resetCreditInput??(account.instanceId?{instanceId:account.instanceId}:null);
    const resetKey=resetInput?mobileThreadLocalResetKey(input.usageKey,account.id,resetInput):'';
    const reset=input.resetStates?.find(state=>state.key===resetKey),count=typeof credits.availableCount==='number'?credits.availableCount:0;
    const expires=typeof credits.nextExpiresAt==='string'?formatDuration(Date.parse(credits.nextExpiresAt)-now):'';
    const hasCredits=!!resetInput&&account.limits.resetCredits!==undefined;
    const summary=count===0?'No reset credits banked':`${count} ${count===1?'reset credit':'reset credits'} banked${expires?` · next expires in ${expires}`:''}`;
    return {id:account.id,key:JSON.stringify([input.usageKey,account.id,instanceLabel]),first:index===0,
      driver:account.driver,label,instanceLabel:instanceLabel!==label?instanceLabel:'',detail:account.plan??'',notice,
      externalURL:str(obj(account.limits.externalUsage).url),
      windows:notice?[]:arr(account.limits.windows).map(window=>{
        const remaining=remainingPercent(window),elapsed=elapsedShare(window,now),pace=paceOf(window,now);
        // Source useBarColor only recognizes Codex/Claude. Other drivers use foreground.
        const color=remaining<=10?'oklch(63.7% 0.237 25.331)':remaining<=30?'oklch(76.9% 0.188 70.08)':account.driver==='codex'
          ?input.dark?'#e6e6e6':'#3c3c43':account.driver==='claudeAgent'?'#d97757':'';
        return {id:str(window.id),label:str(window.label),remaining,color,hasTime:elapsed!==null,
          timeLeft:elapsed===null?0:Math.round((1-elapsed)*100),pace:pace?paceLabel[pace]:'',resets:formatResetsIn(window,now)??''};
      }),
      creditsVisible:hasCredits&&(count>0||!!reset?.status),creditsSummary:summary,
      resetKey,resetInput:resetInput?JSON.stringify(resetInput):'',resetVisible:hasCredits&&count>0,
      resetDisabled:!!reset?.busy||!input.canManageProviders,resetLabel:reset?.busy?'Using…':'Use reset',
      permissionMessage:input.canManageProviders?'':'This connection cannot manage provider accounts.',resetStatus:reset?.status??''};
  });
  return {hasUsage:report!==null,usageKey:input.usageKey,
    maxHeight:Number.isFinite(input.viewportHeight)?Math.max(0,Math.round(input.viewportHeight*.4)):0,
    feedback:snapshot.feedback.map(row=>({...row})),accounts,
    notices:(report?.notices??[]).map((text,index)=>({id:String(index),text,border:accounts.length>0}))};
}
export type MobileThreadLocalDockData=ReturnType<typeof mobileThreadLocalDock>;
