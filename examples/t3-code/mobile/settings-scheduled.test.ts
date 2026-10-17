import { afterEach, beforeEach, expect, test } from 'bun:test';
import { answer } from './app';
import { mobileClient } from './client';
import { MobileDraftClient } from './mobile-draft-recovery';
import { obj, type Obj } from './shared/domain';
import { liveFleetEvent } from './shared/live-streams';
import type { Native } from './shared/protocol';
import { environmentKey, fleet, type FleetEntry } from './shared/settings-b-fleet';
import { mobileSchedule, mobileScheduleInput, mobileTaskDraft, mobileTaskInput, mobileTaskSignature } from './settings-scheduled-draft';
import { mobileScheduledBegin, mobileScheduledClose, mobileScheduledCommand, mobileScheduledEdit, mobileScheduledEditor, mobileScheduledSave, mobileScheduledTasks, scheduledGrant } from './settings-scheduled';
const task:Obj={id:'task-a',title:'Audit',prompt:'Inspect this',projectId:'p',modelSelection:{instanceId:'codex',model:'gpt'},schedule:{type:'webhook',signature:{header:'signature',encoding:'hex',prefix:'sha256=',secret:'must-not-send'},maxDeliveryAgeMinutes:30},workspaceStrategy:{type:'root'},enabled:true,runtimeMode:'auto',interactionMode:'default',threadId:'thread',creationSource:'web'};
test('webhook edits preserve current signature metadata, never send a secret or stale configuration',()=>{
  const draft=mobileTaskDraft('',null,task),changed={...task,schedule:{...obj(task.schedule),signature:{header:'new-signature',encoding:'base64',prefix:'v2',secret:'also-private'}}};
  const payload=mobileTaskInput(draft,changed);expect(obj(payload.schedule).signature).toEqual({header:'new-signature',encoding:'base64',prefix:'v2'});expect(payload).toMatchObject({requireExisting:true,threadId:'thread',creationSource:'web',runtimeMode:'auto'});expect(JSON.stringify(payload)).not.toContain('private');
  expect(()=>mobileTaskInput(draft,null)).toThrow('no longer exists');
});
test('new mobile payload and schedule constraints cover every mode',()=>{
  const draft={...mobileTaskDraft('p',{instanceId:'codex',model:'gpt'}),title:'  Nightly  ',prompt:' inspect ',runtimeMode:'auto-accept-edits'};
  expect(mobileTaskInput(draft,null)).toMatchObject({title:'Nightly',prompt:'inspect',creationSource:'mobile',runtimeMode:'auto-accept-edits',schedule:{type:'fixed_time',weekdays:[1,2,3,4,5]}});
  expect(mobileScheduleInput({...draft.schedule,weekdays:[0,1,2,3,4,5,6]})).not.toHaveProperty('weekdays');
  expect(()=>mobileScheduleInput({...draft.schedule,weekdays:[]})).toThrow();expect(()=>mobileScheduleInput({...draft.schedule,mode:'interval',intervalMinutes:'0.5'})).toThrow();
  expect(mobileScheduleInput({...draft.schedule,mode:'interval',intervalMinutes:'1.5'})).toEqual({type:'interval',everyMs:90000});
  for(const value of ['0','1441','1.2','x'])expect(()=>mobileScheduleInput({...draft.schedule,mode:'webhook',maxDeliveryAgeMinutes:value})).toThrow();
  expect(mobileScheduleInput({...draft.schedule,mode:'webhook',maxDeliveryAgeMinutes:''})).toEqual({type:'webhook',signature:null,maxDeliveryAgeMinutes:null});
  expect(mobileSchedule({schedule:{type:'interval',everyMs:120000}}).weekdays).toEqual([1,2,3,4,5]);
});
test('dirty signature is stable for reordered days/options; explicit permissions override legacy',()=>{
  const draft=mobileTaskDraft('p',{instanceId:'codex',model:'gpt',options:[{id:'b',value:2},{id:'a',value:1}]});
  expect(mobileTaskSignature(draft)).toBe(mobileTaskSignature({...draft,schedule:{...draft.schedule,weekdays:[5,4,3,2,1]},modelSelection:{...draft.modelSelection,options:[{id:'a',value:1},{id:'b',value:2}]}}));
  expect(scheduledGrant({authenticated:true,scopes:['orchestration:operate']})).toBe(true);expect(scheduledGrant({authenticated:true,permissions:[],scopes:['orchestration:operate']})).toBe(false);
});
let original:{client:PropertyDescriptorMap;entries:typeof fleet.entries;saved:typeof fleet.saved;revision:number};
function clearClientFields(){for(const key of Reflect.ownKeys(mobileClient))Reflect.deleteProperty(mobileClient,key);}
beforeEach(async()=>{
  original={client:Object.getOwnPropertyDescriptors(mobileClient),entries:fleet.entries,saved:fleet.saved,revision:fleet.revision};
  const fresh=new MobileDraftClient();clearClientFields();
  // Copy fresh field values, not accessors that close over the temporary client.
  for(const key of Reflect.ownKeys(fresh))Object.defineProperty(mobileClient,key,{value:Reflect.get(fresh,key),writable:true,enumerable:true,configurable:true});
  fleet.entries=new Map();fleet.saved=[];fleet.revision=0;
  answer('scheduledClockTime',[false,'',0,0]);
  await answer('automationPrepare',['','', '', '', '',0,0,0]);
});
afterEach(async()=>{
  try{mobileScheduledClose(true);answer('scheduledClockTime',[false,'',0,0]);await answer('automationPrepare',['','', '', '', '',0,0,0]);}
  finally{clearClientFields();Object.defineProperties(mobileClient,original.client);fleet.entries=original.entries;fleet.saved=original.saved;fleet.revision=original.revision;}
});
const scope=JSON.stringify({environmentIds:['scheduled-test'],members:[{environmentId:'scheduled-test',id:'p'}],projectLabel:'Project'});
function transport(options:{tasks?:Obj[]|null;streamError?:string;auth?:'ok'|'error'|'abort';permissions?:string[]}={}){
  const calls:Obj[]=[],origin='http://scheduled.test',environmentId='scheduled-test',key=environmentKey(origin,environmentId);
  const tasks=options.tasks===undefined?[task]:options.tasks;let permissions=options.permissions??['orchestration:operate'],auth=options.auth??'ok';
  // The private stream and session owners are fresh for every fixture, never mobileClient.
  const entry:FleetEntry={key,origin,environmentId,phase:'connected',message:'',traceId:'',generation:101,synchronized:101,lastEvent:0,subscriptions:{},scopes:[],error:'',requested:true,
    config:{settings:{},providers:[{instanceId:'codex',driver:'codex',enabled:true,installed:true,auth:{status:'authenticated'},models:[{slug:'gpt',name:'GPT',isDefault:true}]}]},shell:{projects:[{id:'p',title:'Project',workspaceRoot:'/tmp/project'}],threads:[],sequence:0}};
  const saved=[{origin,environmentId}];fleet.entries.set(key,entry);fleet.saved=saved;
  if(tasks!==null)liveFleetEvent(entry,{key:'scheduled-tasks',generation:101,subscriptionId:'test-110',value:{tasks}});
  if(options.streamError)liveFleetEvent(entry,{key:'scheduled-tasks',generation:101,subscriptionId:'test-110',value:{_transportError:{message:options.streamError}}});
  const native:Native={available:true,watch(){},async later(input){
    const request=obj(input);calls.push(request);
    if(request.op==='http'&&request.path==='/api/auth/session'){
      if(auth==='abort')throw Object.assign(new Error('This answer was let go.'),{name:'FetchError',kind:'Aborted'});
      if(auth==='error')return {ok:false,generation:101,error:{kind:'transport',message:'Auth session read failed.',uncertain:false}};
    }
    const value=request.op==='environments'?{saved}:request.op==='http'?{authenticated:true,permissions}:request.method==='scheduledTasks.list'?{tasks:tasks??[]}:{};
    return {ok:true,generation:101,value};
  }};
  return {native,calls,entry,deny:()=>{permissions=[];},failAuth:()=>{auth='error';}};
}
test('shared live list drives filtered rows without opening another subscription; outside scope writes refuse',async()=>{
  const {native,calls}=transport();const list=await mobileScheduledTasks(scope,0,native);expect(list.sections[0]?.rows[0]?.id).toBe('task-a');expect(calls.some(call=>call.op==='subscribe'||call.method==='scheduledTasks.list')).toBe(false);
  const result=await mobileScheduledCommand('delete-confirmed',JSON.stringify({environmentIds:['scheduled-test'],members:[],projectLabel:'gone'}),'scheduled-test','task-a',native);expect(result.message).toContain('scope');expect(calls.some(call=>call.method==='scheduledTasks.delete')).toBe(false);
});
test('field edits make no requests; save rechecks revoked permissions and dirty close is guarded',async()=>{
  const {native,calls,deny}=transport();expect((await mobileScheduledBegin(scope,'scheduled-test','',native)).message).toBe('');const count=calls.length;
  mobileScheduledEdit('title','Nightly');mobileScheduledEdit('prompt','Audit');expect(calls).toHaveLength(count);expect(mobileScheduledEditor().dirty).toBe(true);expect(mobileScheduledClose().closed).toBe(false);
  deny();expect((await mobileScheduledSave(native)).message).toContain('cannot change');expect(calls.some(call=>call.method==='scheduledTasks.upsert')).toBe(false);
});
test('a task query error keeps its retained data separate from read-only permissions',async()=>{
  const {native,calls,entry}=transport({streamError:'Task stream failed.',permissions:[]});
  const list=await mobileScheduledTasks(scope,0,native);
  expect(list).toMatchObject({ready:true,error:''});expect(list.sections).toHaveLength(1);
  expect(list.sections[0]).toMatchObject({error:'Task stream failed.',loading:false,emptyMessage:'',rows:[{id:'task-a',canOperate:false}]});
  expect(calls.filter(call=>call.op==='http')).toEqual([{op:'http',path:'/api/auth/session',generation:101,fleet:entry.key}]);
  expect(calls.some(call=>call.op==='subscribe'||call.method==='scheduledTasks.list')).toBe(false);
});
test('an ordinary auth failure revokes operations while retaining healthy task rows',async()=>{
  const f=transport();expect((await mobileScheduledTasks(scope,0,f.native)).sections[0]?.rows[0]?.canOperate).toBe(true);
  f.failAuth();const list=await mobileScheduledTasks(scope,0,f.native);
  expect(list).toMatchObject({ready:true,error:''});expect(list.sections).toHaveLength(1);
  expect(list.sections[0]).toMatchObject({error:'',loading:false,emptyMessage:'',rows:[{id:'task-a',canOperate:false}]});expect(f.entry.scopes).toEqual([]);
  expect((await mobileScheduledCommand('delete-confirmed',scope,'scheduled-test','task-a',f.native)).message).toBe('Auth session read failed.');
  expect(f.calls.some(call=>call.op==='request'||call.op==='subscribe')).toBe(false);
});
test('an ordinary auth failure without task data stays loading rather than reporting a query error',async()=>{
  const {native,calls}=transport({tasks:null,auth:'error'});const list=await mobileScheduledTasks(scope,0,native);
  expect(list).toMatchObject({ready:true,error:''});expect(list.sections).toHaveLength(1);
  expect(list.sections[0]).toMatchObject({error:'',loading:true,emptyMessage:'',rows:[]});
  expect(calls.some(call=>call.op==='subscribe'||call.method==='scheduledTasks.list')).toBe(false);
});
test('actual let-go cancellation rejects the list answer instead of completing a section',async()=>{
  const {native,calls}=transport({auth:'abort'});let completed=false;
  const answer=mobileScheduledTasks(scope,0,native).then(value=>{completed=true;return value;});
  await expect(answer).rejects.toMatchObject({kind:'superseded'});expect(completed).toBe(false);
  expect(calls.map(call=>call.op)).toEqual(['environments','http']);
  expect(calls.some(call=>call.op==='request'||call.op==='subscribe')).toBe(false);
});

test('native Delete cancellation never writes, confirmation uses current scoped command',async()=>{
  const {mobileScheduledTaskMenu}=await import('./settings-scheduled-native');
  const {native,calls}=transport();let confirmed=false;
  const controls:Native={...native,async later(input){const request=obj(input);if(request.op==='mobileScheduledMenu')return {ok:true,value:{choice:'delete'},generation:0};if(request.op==='mobileScheduledConfirm')return {ok:true,value:{confirmed},generation:0};return native.later(input);}};
  expect((await mobileScheduledTaskMenu(scope,'scheduled-test','task-a',0,controls)).message).toBe('');expect(calls.some(call=>call.method==='scheduledTasks.delete')).toBe(false);
  confirmed=true;expect((await mobileScheduledTaskMenu(scope,'scheduled-test','task-a',0,controls)).message).toBe('');expect(calls.filter(call=>call.method==='scheduledTasks.delete')).toHaveLength(1);
});
test('native time cancel preserves draft; repeat choice uses actual menu descriptors',async()=>{
  const {mobileScheduledNativeMenu}=await import('./settings-scheduled-native');
  const {native}=transport();await mobileScheduledBegin(scope,'scheduled-test','',native);
  const controls:Native={...native,async later(input){const request=obj(input);return request.op==='mobileScheduledTime'?{ok:true,generation:0,value:{cancelled:true}}:request.op==='mobileScheduledMenu'?{ok:true,generation:0,value:{choice:'every_day'}}:native.later(input);}};
  await mobileScheduledNativeMenu('time',scope,controls);expect(mobileScheduledEditor().time).toBe('09:00');await mobileScheduledNativeMenu('repeat',scope,controls);expect(mobileScheduledEditor().repeatLabel).toBe('Every day');
});

const clockEpoch=Date.UTC(2026,9,14,12);
function clockFixture(){
  return transport({permissions:[],tasks:[75,100].map(seconds=>({...task,id:`due-${seconds}`,title:`Due ${seconds}`,
    schedule:{type:'interval',everyMs:60_000},nextRunAt:new Date(clockEpoch+seconds*1000).toISOString()}))});
}
function scheduledClock(entry:string,token:number,seconds:number,native?:Native){
  return answer('scheduledClockTime',[true,entry,token,clockEpoch+seconds*1000],undefined,undefined,native);
}
function prepareTasks(now:number,native:Native,selectedScope=scope,revision=0){
  return answer('automationPrepare',['settingsScheduledTasks',selectedScope,'','','',now,revision,0],undefined,undefined,native);
}
function taskRows(){return answer('automationSnapshot',[{},false]).scheduled.sections.flatMap(section=>section.rows);}
function holdEnvironments(native:Native){
  let enter!:()=>void,release!:()=>void,held=false;
  const entered=new Promise<void>(resolve=>{enter=resolve;}),released=new Promise<void>(resolve=>{release=resolve;});
  const gated:Native={...native,async later(input){
    const request=obj(input);
    if(!held&&request.op==='environments'){held=true;enter();await released;}
    return native.later(input);
  }};
  return {native:gated,entered,release};
}
test('Scheduled focus at30 samples synchronously; ordinary refresh at60 holds until its minute at90',async()=>{
  const f=clockFixture(),focus=scheduledClock('entry-a',0,30,f.native);
  expect(focus).toEqual({now:clockEpoch+30_000});expect(focus).not.toBeInstanceOf(Promise);expect(f.calls).toHaveLength(0);
  await prepareTasks(focus.now,f.native);
  expect(taskRows().map(row=>row.subtitle)).toEqual(['Every minute · Next run in less than a minute','Every minute · Next run in 1 minute']);
  const reads=f.calls.length,refresh=scheduledClock('entry-a',0,60,f.native);
  expect(refresh).toEqual(focus);expect(f.calls).toHaveLength(reads);
  await prepareTasks(refresh.now,f.native,scope,1);
  expect(taskRows()[1]!.subtitle).toBe('Every minute · Next run in 1 minute');
  const minute=scheduledClock('entry-a',90_000,90,f.native);expect(minute.now).toBe(clockEpoch+90_000);
  await prepareTasks(minute.now,f.native,scope,2);
  expect(taskRows().map(row=>row.subtitle)).toEqual(['Every minute · Next run due','Every minute · Next run in less than a minute']);
  const currentReads=f.calls.length;expect(scheduledClock('entry-a',90_000,95,f.native)).toEqual(minute);expect(f.calls).toHaveLength(currentReads);
  expect(f.calls.every(call=>call.op==='environments'||call.op==='http'&&call.path==='/api/auth/session')).toBe(true);
});
test('Scheduled cover/resume and distinct same-route entry sample anew; off-route clock leaves Usage anchors intact',async()=>{
  const f=clockFixture();await prepareTasks(scheduledClock('entry-a',0,30).now,f.native);
  expect(answer('scheduledClockTime',[false,'usage-entry',0,0],undefined,undefined,f.native)).toEqual({now:0});
  const reads=f.calls.length;expect(answer('scheduledClockTime',[false,'usage-entry',60_000,clockEpoch+300_000],undefined,undefined,f.native)).toEqual({now:0});expect(f.calls).toHaveLength(reads);
  await answer('automationPrepare',['settingsUsage',scope,'','','',clockEpoch+50_000,0,0]);
  expect(answer('automationSnapshot',[{},false]).now).toBe(clockEpoch+50_000);
  await answer('automationPrepare',['settingsUsage',scope,'','','',clockEpoch+80_000,1,0]);
  expect(answer('automationSnapshot',[{},false]).now).toBe(clockEpoch+50_000);
  const resumed=scheduledClock('entry-a',0,100);expect(resumed.now).toBe(clockEpoch+100_000);
  await prepareTasks(resumed.now,f.native);expect(taskRows()[1]!.subtitle).toBe('Every minute · Next run due');
  // Distinct physical ID, not an assertion that router replace creates one.
  const distinct=scheduledClock('entry-b',0,110);expect(distinct.now).toBe(clockEpoch+110_000);
  await prepareTasks(distinct.now,f.native);expect(answer('automationSnapshot',[{},false]).route).toBe('settingsScheduledTasks');
  expect(scheduledClock('entry-b',0,115)).toEqual(distinct);
});
test('a held ordinary Scheduled refresh cannot restore the pre-minute subtitles or clock',async()=>{
  const f=clockFixture();await prepareTasks(scheduledClock('entry-a',0,30).now,f.native);
  const gate=holdEnvironments(f.native),older=prepareTasks(scheduledClock('entry-a',0,40).now,gate.native,scope,1);
  await gate.entered;
  try{
    const reads=f.calls.length;expect(scheduledClock('entry-a',0,60,f.native).now).toBe(clockEpoch+30_000);expect(f.calls).toHaveLength(reads);
    await prepareTasks(scheduledClock('entry-a',90_000,90).now,f.native,scope,2);
    expect(taskRows()[1]!.subtitle).toBe('Every minute · Next run in less than a minute');
  }finally{gate.release();await older;}
  expect(taskRows()[1]!.subtitle).toBe('Every minute · Next run in less than a minute');
  expect(scheduledClock('entry-a',90_000,95).now).toBe(clockEpoch+90_000);
});
test('a held scope refresh neither resamples the focused clock nor replaces the successor project rows',async()=>{
  const f=clockFixture();f.entry.shell.projects.push({id:'q',title:'Other',workspaceRoot:'/tmp/other'});
  liveFleetEvent(f.entry,{key:'scheduled-tasks',generation:101,subscriptionId:'test-110',value:{tasks:[
    {...task,id:'old-project',schedule:{type:'interval',everyMs:60_000},nextRunAt:new Date(clockEpoch+75_000).toISOString()},
    {...task,id:'new-project',projectId:'q',schedule:{type:'interval',everyMs:60_000},nextRunAt:new Date(clockEpoch+100_000).toISOString()},
  ]}});
  await prepareTasks(scheduledClock('entry-a',0,30).now,f.native);expect(taskRows()[0]!.id).toBe('old-project');
  const gate=holdEnvironments(f.native),older=prepareTasks(scheduledClock('entry-a',0,40).now,gate.native,scope,1);
  await gate.entered;
  const nextScope=JSON.stringify({environmentIds:['scheduled-test'],members:[{environmentId:'scheduled-test',id:'q'}],projectLabel:'Other'});
  try{
    const held=scheduledClock('entry-a',0,60);expect(held.now).toBe(clockEpoch+30_000);
    await prepareTasks(held.now,f.native,nextScope,2);
    expect(taskRows()).toMatchObject([{id:'new-project',subtitle:'Every minute · Next run in 1 minute'}]);
  }finally{gate.release();await older;}
  expect(taskRows()).toMatchObject([{id:'new-project',subtitle:'Every minute · Next run in 1 minute'}]);
  expect(scheduledClock('entry-a',0,65).now).toBe(clockEpoch+30_000);
});
