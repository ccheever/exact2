// @ref llp/1106.009-mobile-settings.decision.md#root-and-native-lifetime
// @ref llp/1106.003-pairing-and-transport.decision.md#mobile-adaptations
// Compact root integration: one async lifecycle resource, one pure projection, one command.
import type { Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileScheduledTasks, mobileScheduledBegin, mobileScheduledPrepare, mobileScheduledEditor, mobileScheduledEdit, mobileScheduledSave, mobileScheduledBranches, mobileScheduledBranchQuery, mobileScheduledBranchLoaded, mobileScheduledCopyWebhook } from './settings-scheduled';
import { mobileScheduledNativeMenu, mobileScheduledTaskMenu, mobileScheduledNativeClose, mobileScheduledNativeRotate, mobileUsageNativeRedeem, mobileUsageNativeSelection } from './settings-scheduled-native';
import { mobileUsage, mobileUsageSnapshot, mobileUsageWindow, mobileUsageAccount, mobileUsageReveal, mobileUsageCommand } from './settings-usage';
import { mobileUsagePresentation } from './settings-usage-view';
export const MOBILE_AUTOMATION_ROUTES=['settingsScheduledTasks','settingsScheduledNew','settingsScheduledEdit','settingsScheduledModel','settingsScheduledBranch','settingsUsage','settingsUsageAccount'];
const scheduledRoute=(route:string)=>route.startsWith('settingsScheduled');
const usageRoute=(route:string)=>route==='settingsUsage'||route==='settingsUsageAccount';
let route='',scope='',error='',selection='null',tab='limits',days=30,metric='cost',anchor=0,revision=0,branchTypedAt=0;
let branchFetched:string|null=null;
let account={key:'',window:'',kind:'',now:0};
let tasks:Awaited<ReturnType<typeof mobileScheduledTasks>>={ready:false,error:'',emptyMessage:'',canCreate:false,sections:[]};
let preparedEpoch=0;
/** Native source revisions/route changes call this; local fields call only the snapshot below. */
export async function mobileAutomationPrepare(name:string,scopeJSON:string,accountKey:string,windowId:string,windowKind:string,now:number,native:Native|null|undefined){
  const current=++preparedEpoch,previous=route;route=name;scope=scopeJSON;error='';
  try{
    if(name==='settingsScheduledTasks'){const loaded=await mobileScheduledTasks(scope,now,native);if(current===preparedEpoch)tasks=loaded;}
    else if(scheduledRoute(name)){
      await mobileScheduledPrepare(native);
      if(name==='settingsScheduledBranch'&&previous!==name){
        branchFetched=null;const editor=mobileScheduledEditor(),query=editor.branchQuery;
        await mobileScheduledBranches(query,false,native);
        if(current===preparedEpoch&&route===name&&mobileScheduledBranchLoaded(query,editor.id))branchFetched=query;
      }
    }
    if(usageRoute(name)){
      if(!usageRoute(previous)){selection='null';tab='limits';days=30;metric='cost';anchor=now;}
      if(name==='settingsUsageAccount'&&previous!==name)account={key:accountKey,window:windowId,kind:windowKind,now:anchor};
      await mobileUsage(selection,tab,JSON.stringify(mobileUsageWindow(days,anchor)),metric,anchor,native,native?.available===true);
    }else if(usageRoute(previous))await mobileUsage(selection,tab,'{}',metric,anchor,native,false,false);
  }catch(cause){if(letGo(cause))throw cause;if(current===preparedEpoch)error=cause instanceof Error?cause.message:'Could not load settings.';}
  return {revision:++revision};
}
export function mobileAutomationSnapshot(){
  return {revision,route,error,days,tab,selection,now:anchor,scheduled:tasks,editor:mobileScheduledEditor(),usage:usageRoute(route)?mobileUsageSnapshot(selection,tab,metric):mobileUsagePresentation(null),account:mobileUsageAccount(account.key,account.window,account.kind,account.now)};
}
/** The root only navigates/reloads when this result explicitly says so. */
export async function mobileAutomationCommand(op:string,id:string,value:string,extra:string,now:number,native:Native|null|undefined){
  let message='',next='',closed=false,refresh=false,debounce=false;
  try{
    if(op==='new'||op==='edit'){
      const result=await mobileScheduledBegin(scope,op==='new'?'':id,op==='new'?'':value,native);message=result.message;next=message?'':op==='new'?'settingsScheduledNew':'settingsScheduledEdit';
    }else if(op==='task-menu'){
      const result=await mobileScheduledTaskMenu(scope,id,value,now,native);message=result.message;next=result.route?'settingsScheduledEdit':'';refresh=true;
    }else if(op==='field')message=mobileScheduledEdit(id,value).message;
    else if(op==='menu')message=(await mobileScheduledNativeMenu(id,scope,native)).message;
    else if(op==='navigate')next=id==='SettingsScheduledTaskModel'?'settingsScheduledModel':id==='SettingsScheduledTaskBranch'?'settingsScheduledBranch':'';
    else if(op==='save'){const result=await mobileScheduledSave(native);message=result.message;closed=result.closed;refresh=result.closed;}
    else if(op==='close'){const result=await mobileScheduledNativeClose(native);message=result.message;closed=result.closed;}
    else if(op==='copy')message=(await mobileScheduledCopyWebhook(native)).message;
    else if(op==='rotate'){message=(await mobileScheduledNativeRotate(scope,native)).message;refresh=true;}
    else if(op==='branch-query'){mobileScheduledBranchQuery(value);branchTypedAt=now;debounce=true;}
    else if(op==='branch-fetch'){
      const editor=mobileScheduledEditor(),query=editor.branchQuery,epoch=preparedEpoch;if(route==='settingsScheduledBranch'&&now-branchTypedAt>=150&&query!==branchFetched){await mobileScheduledBranches(query,false,native);if(epoch===preparedEpoch&&route==='settingsScheduledBranch'&&mobileScheduledBranchLoaded(query,editor.id))branchFetched=query;}
    }else if(op==='branch-more')await mobileScheduledBranches(mobileScheduledEditor().branchQuery,true,native);
    else if(op==='branch-select'){message=mobileScheduledEdit('baseRef',value).message;closed=!message;}
    else if(op==='usage-local'){
      if(id==='metric'){metric=value==='tokens'?'tokens':'cost';}
      else if(id==='tab'){tab=value==='usage'?'usage':'limits';refresh=true;}
      else if(id==='days'&&[1,7,30,90].includes(Number(value))){days=Number(value);anchor=now;refresh=true;}
    }else if(op==='usage-environments'){const result=await mobileUsageNativeSelection(selection,native);selection=result.selection;message=result.message;refresh=!message;}
    else if(op==='usage-command'){message=(await mobileUsageCommand(id,value,extra,now,native)).message;if(id!=='open-link'){anchor=now;refresh=true;}}
    else if(op==='usage-refresh'){message=(await mobileUsageCommand(tab==='limits'?'refresh-limits':'refresh-usage','','',now,native)).message;anchor=now;refresh=true;}
    else if(op==='usage-account'){account={key:id,window:value,kind:extra,now:anchor};next='settingsUsageAccount';}
    else if(op==='usage-reveal')mobileUsageReveal(id);
    else if(op==='usage-redeem'){message=(await mobileUsageNativeRedeem(id,value,now,native)).message;refresh=true;}
    else throw new Error('This settings action is unavailable.');
  }catch(cause){if(letGo(cause))throw cause;message=cause instanceof Error?cause.message:'Could not update settings.';}
  return {revision:++revision,message,route:next,closed,refresh,debounce,accountKey:account.key,windowId:account.window,windowKind:account.kind};
}
