// @ref llp/1107.009-mobile-settings.decision.md#root-and-native-lifetime
// @ref llp/1107.003-pairing-and-transport.decision.md#decision
// Mobile Scheduled Tasks over the shared live stream; no second list subscription/client.
import { mobileClient } from './client';
import { arr, obj, str, type Obj } from './shared/domain';
import { letGo } from './shared/let-go';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { currentTasks, liveEnvironments, type LiveEnvironment } from './shared/live-streams';
import { scheduledTaskDefaultModel } from './shared/scheduled-tasks';
import { settingsCall, settingsEndpoint, settingsSources, settingsNative, type MobileSettingsSource } from './settings-server-source';
import { decodeMobileServerScope, type MobileServerScope } from './settings-server';
import { formatScheduledTaskInterval, formatNextScheduledTaskRun } from './settings-scheduled-format';
import { mobileTaskDraft, mobileTaskInput, mobileTaskSignature, mobileWebhookAddress, WEBHOOK_PROMPT, type MobileTaskDraft } from './settings-scheduled-draft';
import { resolvedCurrent } from './shared/r3-composer-controls-model';
import { runtimeModes } from './shared/composer-presentation';

export const SCHEDULED_ROUTES = ['SettingsScheduledTasks','SettingsScheduledTaskNew','SettingsScheduledTaskEdit','SettingsScheduledTaskModel','SettingsScheduledTaskBranch'];
export function scheduledGrant(session: Obj): boolean { return session.authenticated === true && (Array.isArray(session.permissions) ? session.permissions.includes('orchestration:operate') : session.permissions === undefined && Array.isArray(session.scopes) && session.scopes.includes('orchestration:operate')); }
const result = (message = '', closed = false) => ({ revision: ++mobileClient.revision, message, closed });
const inScope = (scope: MobileServerScope, environmentId: string, projectId: string) => scope.environmentIds.includes(environmentId) && (scope.members === null || scope.members.some(member => member.environmentId === environmentId && member.id === projectId));
function visible(scope: MobileServerScope, source: MobileSettingsSource) { return source.enabled && source.phase === 'connected' && scope.environmentIds.includes(source.environmentId) && (scope.members === null || scope.members.some(member => member.environmentId === source.environmentId)); }
const days = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];
function scheduleLabel(task: Obj) {
  const schedule = obj(task.schedule);
  if (schedule.type === 'webhook') return 'On webhook';
  if (schedule.type === 'interval') return formatScheduledTaskInterval(Number(schedule.everyMs));
  const weekdays = Array.isArray(schedule.weekdays) ? schedule.weekdays.map(Number) : [];
  const [hour,minute] = str(schedule.timeOfDay).split(':').map(Number), time = new Date(0); time.setHours(hour ?? 9, minute ?? 0, 0, 0);
  const repeat = !weekdays.length || weekdays.length === 7 ? 'Every day' : weekdays.length === 5 && weekdays.every(day => day >= 1 && day <= 5) ? 'Weekdays' : [1,2,3,4,5,6,0].filter(day=>weekdays.includes(day)).map(day=>days[day]).join(', ');
  return `${repeat} at ${Number.isInteger(hour)&&Number.isInteger(minute)?time.toLocaleTimeString([], {hour:'numeric',minute:'2-digit'}):str(schedule.timeOfDay)}`;
}
export async function mobileScheduledTasks(scopeJSON: string, now: number, nativeInput: Native | null | undefined) {
  const empty = { ready: false, error: '', emptyMessage: '', canCreate: false, sections: [] as { id: string; title: string; error: string; loading: boolean; emptyMessage: string; rows: { id: string; title: string; subtitle: string; error: string; canOperate: boolean; enabled: boolean; webhook: boolean; first: boolean }[] }[] };
  if (!nativeInput?.available) return empty;
  const scope = decodeMobileServerScope(scopeJSON), native = settingsNative(nativeInput);
  try {
    const all = await settingsSources(native), sources = all.filter(source => visible(scope, source)), live = liveEnvironments(mobileClient, native);
    const sections = await Promise.all(sources.map(async source => {
      const environment = live.find(entry => entry.environmentId === source.environmentId), endpoint = settingsEndpoint(source, native);
      let canOperate = false, error = environment?.tasks.error ?? '';
      try { canOperate = scheduledGrant(await settingsCall(endpoint, { op: 'http', path: '/api/auth/session' })); }
      catch (cause) { if (letGo(cause)) throw cause; error = cause instanceof Error ? cause.message : 'Could not verify permissions.'; }
      const tasks = environment?.tasks.value?.filter(task => inScope(scope, source.environmentId, str(task.projectId))) ?? null;
      return { id: source.environmentId, title: source.label, error, loading: !error && tasks === null,
        emptyMessage: tasks?.length === 0 ? scope.members === null ? 'No scheduled tasks yet.' : 'No tasks in this project.' : '',
        rows: (tasks ?? []).map((task,index) => ({ id: str(task.id), title: str(task.title), subtitle: `${scheduleLabel(task)}${task.enabled !== true ? ' · Paused' : task.nextRunAt ? ` · ${formatNextScheduledTaskRun(str(task.nextRunAt), now)}` : ''}`,
          error: task.lastRunError ? `Last run failed: ${str(task.lastRunError)}` : '', canOperate, enabled: task.enabled === true, webhook: obj(task.schedule).type === 'webhook', first: index === 0 })) };
    }));
    return { ...empty, ready: true, canCreate: sources.length > 0, sections, emptyMessage: sources.length ? '' : all.some(source => source.enabled && source.phase === 'connected')
      ? 'No environments match these filters. Change the filter above.' : 'Connect an environment to view and create scheduled tasks.' };
  } catch (error) { if (letGo(error)) throw error; return { ...empty, error: error instanceof Error ? error.message : 'Could not load scheduled tasks.' }; }
}
async function target(environmentId: string, native: Native, write = false) {
  const source = (await settingsSources(native)).find(source => source.environmentId === environmentId && source.enabled && source.phase === 'connected');
  const live = liveEnvironments(mobileClient, native).find(entry => entry.environmentId === environmentId);
  if (!source || !live?.connected) throw new ClientError('This environment is disconnected. Reconnect before saving.');
  const endpoint = settingsEndpoint(source, native), session = await settingsCall(endpoint, { op: 'http', path: '/api/auth/session' });
  const canOperate = scheduledGrant(session);
  if (write && !canOperate) throw new ClientError('This connection cannot change scheduled tasks.');
  return { source, live, endpoint, canOperate };
}
export async function mobileScheduledCommand(op: string, scopeJSON: string, environmentId: string, taskId: string, nativeInput: Native | null | undefined) {
  try {
    if (!nativeInput?.available) throw new ClientError('Scheduled tasks require the native app.');
    const scope = decodeMobileServerScope(scopeJSON), { live, endpoint } = await target(environmentId, settingsNative(nativeInput), true);
    const task = (await currentTasks(live)).find(task => task.id === taskId);
    if (!task || !inScope(scope, environmentId, str(task.projectId))) throw new ClientError('This scheduled task no longer exists in the selected scope.');
    const methods: Record<string,string> = { toggle: 'scheduledTasks.setEnabled', run: 'scheduledTasks.runNow', 'delete-confirmed': 'scheduledTasks.delete', 'rotate-confirmed': 'scheduledTasks.rotateWebhookToken' };
    if (!methods[op] || op === 'run' && obj(task.schedule).type === 'webhook' || op === 'rotate-confirmed' && obj(task.schedule).type !== 'webhook') throw new ClientError('That scheduled task action is unavailable or requires confirmation.');
    await settingsCall(endpoint, { op: 'request', method: methods[op], payload: { id: taskId, ...(op === 'toggle' ? { enabled: task.enabled !== true } : {}) } });
    return result();
  } catch (error) { if (letGo(error)) throw error; return result(error instanceof Error ? error.message : 'Could not update task.'); }
}
interface Editor { id: number; environmentId: string; label: string; initial: string; draft: MobileTaskDraft; busy: boolean; saved: boolean; error: string;
  config: Obj; projects: Obj[]; environments: { id: string; label: string }[]; canOperate: boolean; missing: boolean; connected: boolean; webhook: { address: string; copyable: boolean; note: string }; branches: Obj[]; branchQuery: string; nextCursor: number | null; branchError: string }
let editor: Editor | null = null, editorSerial = 0, branchSerial = 0, branchLoadedSerial = -1, branchLoadedEditor = -1;
function defaultDraft(live: LiveEnvironment, scope: MobileServerScope) {
  const project = live.shell.projects.find(project => scope.members?.some(member => member.environmentId === live.environmentId && member.id === project.id)) ?? live.shell.projects[0] ?? null;
  return mobileTaskDraft(str(project?.id), scheduledTaskDefaultModel(obj(live.config.settings), project, arr(live.config.providers)));
}
export async function mobileScheduledBegin(scopeJSON: string, environmentId: string, taskId: string, nativeInput: Native | null | undefined) {
  try {
    if (!nativeInput?.available) throw new ClientError('Scheduled tasks require the native app.');
    const scope = decodeMobileServerScope(scopeJSON), native = settingsNative(nativeInput), sources = await settingsSources(native);
    const id = environmentId || sources.find(source => visible(scope, source))?.environmentId;
    if (!id || !sources.some(source=>source.environmentId===id&&visible(scope,source))) throw new ClientError('Connect an environment to create a scheduled task.');
    const { source, live, canOperate } = await target(id, native), tasks = taskId ? await currentTasks(live) : [];
    const task = tasks.find(task => task.id === taskId) ?? null;
    if (taskId && (!task || !inScope(scope, id, str(task.projectId)))) throw new ClientError('This scheduled task no longer exists in the selected scope.');
    const draft = task ? mobileTaskDraft('', null, task) : defaultDraft(live, scope);
    editor = { id: ++editorSerial, environmentId: id, label: source.label, initial: JSON.stringify([id,mobileTaskSignature(draft)]), draft, busy: false, saved: false, error: '', config: live.config,
      projects: live.shell.projects, environments: sources.filter(source => source.enabled && source.phase === 'connected').map(source => ({id:source.environmentId,label:source.label})), canOperate, missing: false, connected: true,
      webhook: task?.webhook ? mobileWebhookAddress(obj(task.webhook), source.origin) : { address:'',copyable:false,note:'' }, branches: [], branchQuery:'',nextCursor:null,branchError:'' };
    return result();
  } catch(error) { if (letGo(error)) throw error; return result(error instanceof Error ? error.message : 'Could not open scheduled task.'); }
}
/** Lifecycle/data refresh only, never on a field edit. Retain draft while checking live task/connection facts. */
export async function mobileScheduledPrepare(nativeInput: Native | null | undefined) {
  const state = editor; if (!state || !nativeInput?.available) return mobileScheduledEditor();
  try {
    const { live, source, canOperate } = await target(state.environmentId, settingsNative(nativeInput));
    if (editor !== state) return mobileScheduledEditor();
    state.config = live.config; state.projects = live.shell.projects; state.label = source.label; state.canOperate = canOperate; state.connected = true;
    const task = state.draft.task ? live.tasks.value === null ? state.draft.task : live.tasks.value.find(task => task.id === state.draft.task?.id) : null;
    state.missing = !!state.draft.task && live.tasks.value !== null && !task;
    state.webhook = task?.webhook ? mobileWebhookAddress(obj(task.webhook), source.origin) : {address:'',copyable:false,note:''};
  } catch(error) { if (letGo(error)) throw error; state.connected = false; state.error = error instanceof Error ? error.message : 'Could not refresh task.'; }
  return mobileScheduledEditor();
}
export function mobileScheduledEditor() {
  const state = editor, draft = state?.draft, config = state?.config ?? {}, selection = obj(draft?.modelSelection);
  const modelRows = arr(config.providers).filter(provider => provider.enabled === true && provider.installed === true && obj(provider.auth).status !== 'unauthenticated').flatMap(provider =>
    arr(provider.models).map(model => ({ id: JSON.stringify([provider.instanceId,model.slug]), instanceId:str(provider.instanceId), model:str(model.slug), label:str(model.name)||str(model.slug),
      provider: str(provider.displayName)||str(provider.driver), selected:selection.instanceId === provider.instanceId && selection.model === model.slug, disabled:provider.availability === 'unavailable'||model.isUnavailable===true,
      legacy:model.isLegacy===true, driver:str(provider.driver), capabilities:obj(model.capabilities) })));
  if(str(selection.instanceId)&&str(selection.model)&&!modelRows.some(model=>model.selected)) {
    const provider=arr(config.providers).find(provider=>provider.instanceId===selection.instanceId);
    modelRows.push({id:JSON.stringify([selection.instanceId,selection.model]),instanceId:str(selection.instanceId),model:str(selection.model),label:str(selection.model),provider:str(provider?.displayName)||str(provider?.driver)||str(selection.instanceId),selected:true,disabled:true,legacy:false,driver:str(provider?.driver),capabilities:{}});
  }
  const current = modelRows.find(model => model.selected), descriptors = arr(current?.capabilities.optionDescriptors).filter(descriptor => ['select','boolean'].includes(str(descriptor.type)));
  const options = descriptors.map(descriptor => { const value = resolvedCurrent(descriptor, arr(selection.options)); return { id:str(descriptor.id),label:str(descriptor.label),boolean:descriptor.type==='boolean',on:value===true,
    choices:arr(descriptor.options).filter(option => option.id !== 'ultracode' && !(Array.isArray(descriptor.promptInjectedValues)&&descriptor.promptInjectedValues.includes(option.id ?? null))).map(option=>({id:str(option.id),label:str(option.label),selected:option.id===value})) }; });
  const provider = arr(config.providers).find(provider => provider.instanceId === selection.instanceId), supported = provider?.supportedRuntimeModes;
  return { present:!!state, id:state?.id??0, title:draft?.task?'Edit scheduled task':'New scheduled task', error:state?.error??'', busy:state?.busy??false, saved:state?.saved??false,
    dirty:!!state&&state.initial!==JSON.stringify([state.environmentId,mobileTaskSignature(state.draft)]), canSave:!!state&&state.canOperate&&state.connected&&!state.missing&&!state.busy&&!state.saved,
    connected:state?.connected??false, missing:state?.missing??false, taskId:str(draft?.task?.id), environmentId:state?.environmentId??'', environmentLabel:state?.label??'', editing:!!draft?.task,
    name:draft?.title??'',prompt:draft?.prompt??'',projectId:draft?.projectId??'',modelLabel:current?.label||str(selection.model)||'Choose model',
    projects:(state?.projects??[]).map(project=>({id:str(project.id),label:str(project.title),selected:project.id===draft?.projectId})), environments:(state?.environments??[]).map(source=>({...source,selected:source.id===state?.environmentId})),
    workspace:draft?.workspace??'worktree',baseRef:draft?.baseRef??'main',checkoutPath:draft?.checkoutPath??'',startFromOrigin:draft?.startFromOrigin??true,
    repeatLabel:draft?.schedule.weekdays.length===7?'Every day':draft?.schedule.weekdays.length===5&&[1,2,3,4,5].every(day=>draft.schedule.weekdays.includes(day))?'Weekdays':draft?.schedule.weekdays.length?[1,2,3,4,5,6,0].filter(day=>draft.schedule.weekdays.includes(day)).map(day=>days[day]).join(', '):'Choose days',
    mode:draft?.schedule.mode??'fixed_time',time:draft?.schedule.timeOfDay??'09:00',interval:draft?.schedule.intervalMinutes??'15',age:draft?.schedule.maxDeliveryAgeMinutes??'',enabled:draft?.enabled??true,
    days:[1,2,3,4,5,6,0].map(day=>({id:String(day),label:days[day]!,selected:draft?.schedule.weekdays.includes(day)??false})),signatureConfigured:!!draft?.schedule.signature,
    webhookAddress:state?.webhook.address??'',webhookCopyable:state?.webhook.copyable??false,webhookNote:state?.webhook.note??'',
    models:modelRows.map(({capabilities,...row})=>row),options,runtimes:runtimeModes.filter(mode=>!Array.isArray(supported)||!supported.length||supported.includes(mode.mode)).map(mode=>({id:mode.mode,label:mode.label,description:mode.description,selected:mode.mode===draft?.runtimeMode})),
    branches:(state?.branches??[]).map(branch=>({id:str(branch.name),label:str(branch.name),selected:branch.name===draft?.baseRef})),branchQuery:state?.branchQuery??'',branchError:state?.branchError??'',hasMore:state?.nextCursor!=null };
}
/** Synchronous edits have no I/O and never mutate the active chat composer. */
export function mobileScheduledEdit(key:string,value:string) {
  const state=editor;if(!state||state.busy||state.saved)return result('Open an editable scheduled task form first.');
  const draft=state.draft;
  try {
    if (['title','prompt','baseRef','checkoutPath','runtimeMode','workspace'].includes(key)) {
      if(key==='workspace'&&!['root','worktree','existing_worktree'].includes(value)||key==='runtimeMode'&&!runtimeModes.some(mode=>mode.mode===value))throw new ClientError('Unsupported task option.');
      state.draft={...draft,[key]:value};
    } else if(key==='projectId') { branchSerial++; const project=state.projects.find(project=>project.id===value);if(!project)throw new ClientError('Choose a project in this environment.');state.draft={...draft,projectId:value,modelSelection:draft.modelSelectionIsExplicit?draft.modelSelection:scheduledTaskDefaultModel(obj(state.config.settings),project,arr(state.config.providers))}; }
    else if(key==='model') {const [instanceId,model]=JSON.parse(value) as string[];const choice=mobileScheduledEditor().models.find(row=>row.instanceId===instanceId&&row.model===model&&!row.disabled);if(!choice)throw new ClientError('Choose an available model.');state.draft={...draft,modelSelection:{instanceId:instanceId!,model:model!},modelSelectionIsExplicit:true};}
    else if(key.startsWith('option:')) {if(!draft.modelSelection)throw new ClientError('Choose a model first.');const id=key.slice(7),descriptor=mobileScheduledEditor().options.find(option=>option.id===id);if(!descriptor)throw new ClientError('That model option is unavailable.');if(descriptor.boolean&&!['true','false'].includes(value))throw new ClientError('Choose On or Off.');const parsed=descriptor.boolean?value==='true':value;if(!descriptor.boolean&&!descriptor.choices.some(choice=>choice.id===value))throw new ClientError('Unsupported model option.');state.draft={...draft,modelSelection:{...draft.modelSelection,options:mobileScheduledEditor().options.flatMap(option=>{const choice=option.id===id?parsed:option.boolean?option.on:option.choices.find(choice=>choice.selected)?.id;return choice===undefined?[]:[{id:option.id,value:choice}];})},modelSelectionIsExplicit:true};}
    else if(key==='enabled'||key==='startFromOrigin') {if(!['true','false'].includes(value))throw new ClientError('Choose On or Off.');state.draft={...draft,[key]:value==='true'};}
    else if(key==='day') {const day=Number(value);if(!Number.isInteger(day)||day<0||day>6)throw new ClientError('Choose a weekday.');state.draft={...draft,schedule:{...draft.schedule,weekdays:draft.schedule.weekdays.includes(day)?draft.schedule.weekdays.filter(item=>item!==day):[...draft.schedule.weekdays,day]}};}
    else if(['mode','timeOfDay','intervalMinutes','maxDeliveryAgeMinutes','repeat'].includes(key)) {
      if(key==='repeat'&&!['every_day','weekdays'].includes(value))throw new ClientError('Choose a repeat option.');
      if(key==='mode'&&!['fixed_time','interval','webhook'].includes(value))throw new ClientError('Unsupported schedule.');
      state.draft={...draft,prompt:key==='mode'&&value==='webhook'&&!draft.prompt.trim()?WEBHOOK_PROMPT:draft.prompt,schedule:{...draft.schedule,...(key==='repeat'?{weekdays:value==='every_day'?[0,1,2,3,4,5,6]:[1,2,3,4,5]}:{[key]:value})}};
    } else throw new ClientError('Unknown scheduled task field.');
    state.error='';return result();
  } catch(error){state.error=error instanceof Error?error.message:'Could not edit task.';return result(state.error);}
}
export async function mobileScheduledEnvironment(environmentId:string,scopeJSON:string,nativeInput:Native|null|undefined){
  const state=editor;if(!state||state.draft.task||state.busy||!nativeInput?.available)return result('The task environment cannot be changed.');
  try{const {source,live,canOperate}=await target(environmentId,settingsNative(nativeInput));if(editor!==state)return result('The task editor changed.');const draft={...defaultDraft(live,decodeMobileServerScope(scopeJSON)),title:state.draft.title,prompt:state.draft.prompt,schedule:state.draft.schedule,enabled:state.draft.enabled};branchSerial++;Object.assign(state,{environmentId,label:source.label,draft,config:live.config,projects:live.shell.projects,canOperate,connected:true,branches:[],nextCursor:null});return result();}catch(error){if(letGo(error))throw error;state.error=error instanceof Error?error.message:'Could not change environment.';return result(state.error);}
}
export async function mobileScheduledSave(nativeInput:Native|null|undefined){
  const state=editor;if(!state||state.busy||state.saved)return result('A task save is already in progress.');state.busy=true;state.error='';
  try{if(!nativeInput?.available)throw new ClientError('Scheduled tasks require the native app.');const {live,endpoint}=await target(state.environmentId,settingsNative(nativeInput),true);if(editor!==state)throw new ClientError('The task editor changed.');
    const task=state.draft.task?(await currentTasks(live)).find(task=>task.id===state.draft.task?.id)??null:null,payload=mobileTaskInput(state.draft,task);
    if(!live.shell.projects.some(project=>project.id===payload.projectId))throw new ClientError('Choose a project in this environment.');
    await settingsCall(endpoint,{op:'request',method:'scheduledTasks.upsert',payload});state.saved=true;return result('',true);
  }catch(error){if(letGo(error))throw error;state.error=error instanceof Error?error.message:'Could not save task.';return result(state.error);}finally{state.busy=false;}
}
export function mobileScheduledClose(confirmed=false){if(editor?.busy)return result('Wait for the task to finish saving before leaving.');if(mobileScheduledEditor().dirty&&!editor?.saved&&!confirmed)return result('Discard changes?');editor=null;return result('',true);}
export async function mobileScheduledBranches(query:string,more:boolean,nativeInput:Native|null|undefined){
  const state=editor,requestSerial=++branchSerial,projectId=state?.draft.projectId;if(!state||!nativeInput?.available)return mobileScheduledEditor();
  try{const {live,endpoint}=await target(state.environmentId,settingsNative(nativeInput));const project=live.shell.projects.find(project=>project.id===state.draft.projectId);if(!project)throw new ClientError('This project is no longer available.');
    const cursor=more&&query===state.branchQuery?state.nextCursor:null;if(more&&cursor===null)return mobileScheduledEditor();const reply=await settingsCall(endpoint,{op:'request',method:'vcs.listRefs',payload:{cwd:str(project.workspaceRoot),limit:100,...(query.trim()?{query:query.trim()}:{}),...(cursor===null?{}:{cursor})}});
    if(editor!==state||requestSerial!==branchSerial||projectId!==state.draft.projectId)return mobileScheduledEditor();state.branches=[...(cursor===null?[]:state.branches),...arr(reply.refs)].filter(branch=>branch.isRemote!==true&&str(branch.name).toLowerCase().includes(query.trim().toLowerCase()));state.branchQuery=query;branchLoadedSerial=requestSerial;branchLoadedEditor=state.id;state.nextCursor=typeof reply.nextCursor==='number'?reply.nextCursor:null;state.branchError='';
  }catch(error){if(letGo(error))throw error;if(editor!==state||requestSerial!==branchSerial)return mobileScheduledEditor();state.branchError=error instanceof Error?error.message:'Could not load branches.';}return mobileScheduledEditor();
}
export async function mobileScheduledCopyWebhook(nativeInput:Native|null|undefined){try{if(!nativeInput?.available||!editor?.webhook.copyable)throw new ClientError('No complete webhook URL is available.');const reply=await bridgeReply(nativeInput,{op:'copyText',text:editor.webhook.address});if(!reply.ok||obj(reply.value).copied!==true)throw new ClientError('The webhook URL was not copied.');return result();}catch(error){if(letGo(error))throw error;return result(error instanceof Error?error.message:'Could not copy URL.');}}
/** Native menu descriptors; root dispatches environment asynchronously, other keys through edit. */
export function mobileScheduledMenu(key:string) {
  const data=mobileScheduledEditor(),disabled=!data.present||data.busy;
  const rows=key==='environment'?data.environments:key==='project'?data.projects:key==='workspace'?
    [{id:'worktree',label:'New worktree',selected:data.workspace==='worktree'},{id:'root',label:'Project checkout',selected:data.workspace==='root'},{id:'existing_worktree',label:'Specific checkout',selected:data.workspace==='existing_worktree'}]:key==='repeat'?
    [{id:'every_day',label:'Every day',selected:data.days.every(day=>day.selected)},{id:'weekdays',label:'Weekdays',selected:data.repeatLabel==='Weekdays'},...data.days.map(day=>({...day,id:`day:${day.id}`}))]:key.startsWith('option:')?
    (()=>{const option=data.options.find(option=>option.id===key.slice(7));return option?.boolean?[{id:'true',label:'On',selected:option.on},{id:'false',label:'Off',selected:!option.on}]:option?.choices??[];})():[];
  return rows.map(row=>({id:row.id,title:row.label,selected:row.selected,disabled:disabled||key==='environment'&&data.editing}));
}
export function mobileScheduledBranchQuery(query:string){if(editor){if(editor.branchQuery!==query)branchSerial++;editor.branchQuery=query;editor.branchError='';}return result();}

/** True only after a response for this editor/query survived every invalidation. */
export function mobileScheduledBranchLoaded(query:string,editorId:number){return editor?.id===editorId&&editor.branchQuery===query&&branchLoadedEditor===editorId&&branchLoadedSerial===branchSerial;}
