// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
// Explicit UIKit actions over the parent's existing native module bridge.
import { mobileClient } from './client';
import { obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileScheduledBegin, mobileScheduledClose, mobileScheduledCommand, mobileScheduledEdit, mobileScheduledEditor, mobileScheduledEnvironment, mobileScheduledMenu, mobileScheduledTasks } from './settings-scheduled';
import { mobileUsageCommand, mobileUsageRedemption, mobileUsageEnvironmentChoices, mobileUsageToggleEnvironment } from './settings-usage';
const result=(message='',route='',closed=false)=>({revision:mobileClient.revision,message,route,closed});
async function nativeControl(native:Native|null|undefined,input:Obj){
  if(!native?.available)throw new ClientError('This control requires the native app.');
  const reply=await bridgeReply(native,input);if(!reply.ok)throw new ClientError(reply.error?.message||'Could not present this control.');return obj(reply.value);
}
const errorMessage=(error:unknown)=>error instanceof Error?error.message:'Could not complete this action.';
/** Called with the key emitted by SettingsScheduledEditor/Model. */
export async function mobileScheduledNativeMenu(key:string,scopeJSON:string,native:Native|null|undefined){
  const opened=mobileScheduledEditor();
  try{
    if(!opened.present||opened.busy)throw new ClientError('Open an editable task form first.');
    if(key==='time'){
      const response=await nativeControl(native,{op:'mobileScheduledTime',value:opened.time});
      if(response.cancelled===true)return result();if(mobileScheduledEditor().id!==opened.id)throw new ClientError('The task editor changed.');
      return {...mobileScheduledEdit('timeOfDay',str(response.value)),route:''};
    }
    const items=mobileScheduledMenu(key);if(!items.length)throw new ClientError('This menu has no available choices.');
    const response=await nativeControl(native,{op:'mobileScheduledMenu',title:({environment:'Runs on',project:'Project',workspace:'Workspace',repeat:'Repeat'} as Record<string,string>)[key]||'Model option',items});
    if(response.cancelled===true)return result();if(mobileScheduledEditor().id!==opened.id)throw new ClientError('The task editor changed.');
    const choice=str(response.choice);if(!items.some(item=>item.id===choice&&!item.disabled))throw new ClientError('That choice is unavailable.');
    if(key==='environment')return {...await mobileScheduledEnvironment(choice,scopeJSON,native),route:''};
    const edited=key==='repeat'?mobileScheduledEdit(choice.startsWith('day:')?'day':'repeat',choice.startsWith('day:')?choice.slice(4):choice):mobileScheduledEdit(key==='project'?'projectId':key,choice);
    return {...edited,route:''};
  }catch(error){if(letGo(error))throw error;return result(errorMessage(error));}
}
export async function mobileScheduledTaskMenu(scopeJSON:string,environmentId:string,taskId:string,now:number,native:Native|null|undefined){
  try{
    const data=await mobileScheduledTasks(scopeJSON,now,native),task=data.sections.find(section=>section.id===environmentId)?.rows.find(row=>row.id===taskId);
    if(!task)throw new ClientError('This task is no longer available in the selected scope.');
    const items=[{id:'edit',title:'Edit',disabled:false},{id:'toggle',title:task.enabled?'Pause':'Resume',disabled:!task.canOperate},
      ...(task.webhook?[]:[{id:'run',title:'Run now',disabled:!task.canOperate}]),{id:'delete',title:'Delete',disabled:!task.canOperate,destructive:true}];
    const response=await nativeControl(native,{op:'mobileScheduledMenu',title:task.title,items});if(response.cancelled===true)return result();
    const choice=str(response.choice);if(!items.some(item=>item.id===choice&&!item.disabled))throw new ClientError('That task action is unavailable.');
    if(choice==='edit'){const opened=await mobileScheduledBegin(scopeJSON,environmentId,taskId,native);return {...opened,route:opened.message?'':'SettingsScheduledTaskEdit'};}
    if(choice==='delete'){const confirmation=await nativeControl(native,{op:'mobileScheduledConfirm',kind:'delete',taskTitle:task.title});if(confirmation.confirmed!==true)return result();}
    return {...await mobileScheduledCommand(choice==='delete'?'delete-confirmed':choice,scopeJSON,environmentId,taskId,native),route:''};
  }catch(error){if(letGo(error))throw error;return result(errorMessage(error));}
}
export async function mobileScheduledNativeClose(native:Native|null|undefined){
  const state=mobileScheduledEditor(),closed=mobileScheduledClose();if(closed.closed||state.busy)return {...closed,route:''};
  try{const response=await nativeControl(native,{op:'mobileScheduledConfirm',kind:'discard'});if(response.confirmed!==true)return result();
    if(mobileScheduledEditor().id!==state.id)throw new ClientError('The task editor changed.');return {...mobileScheduledClose(true),route:''};
  }catch(error){if(letGo(error))throw error;return result(errorMessage(error));}
}
export async function mobileScheduledNativeRotate(scopeJSON:string,native:Native|null|undefined){
  const state=mobileScheduledEditor();
  try{if(!state.canSave||!state.taskId||state.mode!=='webhook')throw new ClientError('No editable webhook task is open.');
    const response=await nativeControl(native,{op:'mobileScheduledConfirm',kind:'rotate'});if(response.confirmed!==true)return result();
    if(mobileScheduledEditor().id!==state.id)throw new ClientError('The task editor changed.');
    return {...await mobileScheduledCommand('rotate-confirmed',scopeJSON,state.environmentId,state.taskId,native),route:''};
  }catch(error){if(letGo(error))throw error;return result(errorMessage(error));}
}
export async function mobileUsageNativeRedeem(environmentId:string,accountKey:string,now:number,native:Native|null|undefined){
  try{const shown=mobileUsageRedemption(accountKey);if(!shown)throw new ClientError('No reset credit is available for this account.');
    const response=await nativeControl(native,{op:'mobileScheduledConfirm',kind:'reset-credit'});if(response.confirmed!==true)return result();
    return {...await mobileUsageCommand('redeem-confirmed',environmentId,accountKey,now,native,shown),route:''};
  }catch(error){if(letGo(error))throw error;return result(errorMessage(error));}
}

export async function mobileUsageNativeSelection(selectionJSON:string,native:Native|null|undefined){
  try{const choices=mobileUsageEnvironmentChoices(selectionJSON),items=[{id:'all',title:'All environments',selected:selectionJSON==='null'},...choices.map(choice=>({id:JSON.stringify(['environment',choice.id]),title:choice.label,selected:choice.selected}))];
    const response=await nativeControl(native,{op:'mobileScheduledMenu',title:'Environments',items});if(response.cancelled===true)return {...result(),selection:selectionJSON};
    const chosen=str(response.choice);if(chosen==='all')return {...result(),selection:'null'};
    const item=choices.find(choice=>JSON.stringify(['environment',choice.id])===chosen);if(!item)throw new ClientError('That environment is unavailable.');
    return {...result(),selection:mobileUsageToggleEnvironment(selectionJSON,item.id)};
  }catch(error){if(letGo(error))throw error;return {...result(errorMessage(error)),selection:selectionJSON};}
}
