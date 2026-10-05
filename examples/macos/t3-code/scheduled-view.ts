// Settings → Scheduled tasks view model (reference ScheduledTasksSettings.tsx and
// scheduledTasksSettings.logic.ts): rows with schedule/status labels, and the
// editor's draft, choices and branch refs. The editor never enables or runs a
// task by itself; Enabled is the user's own switch.
import { arr, obj, str, num, type Obj } from './domain';
import type { T3Client } from './client';
import { providerAvailable, type Native } from './protocol';
import { providerBadge } from './presentation';
import { relativeTimeLabel } from './settings-data';

const WEEKDAY_LABELS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
export const WORKSPACE_LABELS: Record<string, string> = { worktree: 'Create a new worktree', root: 'Use the project checkout', existing_worktree: 'Use a specific checkout' };

/** scheduleLabel */
export function scheduleLabel(schedule: Obj): string {
  if (schedule.type === 'interval') {
    const minutes = num(schedule.everyMs) / 60000;
    return Number.isInteger(minutes) ? `Every ${minutes} min` : `Every ${Math.round(num(schedule.everyMs) / 1000)} sec`;
  }
  const weekdays = Array.isArray(schedule.weekdays) ? schedule.weekdays.map(Number) : [];
  const days = weekdays.length === 0 ? 'Daily' : weekdays.length === 5 && weekdays.every(day => day >= 1 && day <= 5) ? 'Weekdays' : weekdays.map(day => WEEKDAY_LABELS[day]).join(', ');
  return `${days} at ${str(schedule.timeOfDay)}`;
}
/** relativeLabel: future instants read "in 5m". */
export function runLabel(value: unknown, now: number): string {
  if (!value) return 'Not scheduled';
  const diff = Date.parse(str(value)) - now;
  if (!Number.isFinite(diff)) return 'Not scheduled';
  if (diff <= 0) return relativeTimeLabel(str(value), now) || 'Not scheduled';
  const minutes = Math.ceil(diff / 60000);
  if (minutes < 2) return 'in under a minute';
  if (minutes < 60) return `in ${minutes}m`;
  const hours = Math.round(minutes / 60);
  return hours < 24 ? `in ${hours}h` : `in ${Math.round(hours / 24)}d`;
}
export function taskStatus(task: Obj, now: number): string {
  return `${scheduleLabel(obj(task.schedule))} · ${task.enabled === true ? (task.nextRunAt ? `Next run ${runLabel(task.nextRunAt, now)}` : 'Not scheduled') : 'Paused'}`;
}

/** EMPTY_DRAFT / taskToDraft */
export function editorDraft(task: Obj | undefined, projectId: string, modelKey: string) {
  if (!task) return { id: '', title: '', prompt: '', enabled: true, scheduleMode: 'fixed', intervalMinutes: '15', timeOfDay: '09:00',
    sunday: false, monday: true, tuesday: true, wednesday: true, thursday: true, friday: true, saturday: false,
    projectId, threadId: '', workspaceMode: 'worktree', baseRef: 'main', startFromOrigin: true, path: '', modelKey, legacyInterval: false };
  const schedule = obj(task.schedule), workspace = obj(task.workspaceStrategy), model = obj(task.modelSelection);
  const days = schedule.type === 'fixed_time' && Array.isArray(schedule.weekdays) && schedule.weekdays.length ? schedule.weekdays.map(Number) : [0, 1, 2, 3, 4, 5, 6];
  return { id: str(task.id), title: str(task.title), prompt: str(task.prompt), enabled: task.enabled === true, scheduleMode: schedule.type === 'interval' ? 'interval' : 'fixed',
    intervalMinutes: schedule.type === 'interval' ? String(Math.max(1, num(schedule.everyMs) / 60000)) : '15', timeOfDay: schedule.type === 'fixed_time' ? str(schedule.timeOfDay, '09:00') : '09:00',
    sunday: days.includes(0), monday: days.includes(1), tuesday: days.includes(2), wednesday: days.includes(3), thursday: days.includes(4), friday: days.includes(5), saturday: days.includes(6),
    projectId: str(task.projectId), threadId: str(task.threadId), workspaceMode: str(workspace.type, 'worktree'), baseRef: workspace.type === 'worktree' ? str(workspace.baseRef, 'main') : 'main',
    startFromOrigin: workspace.type === 'worktree' ? workspace.startFromOrigin === true : true, path: workspace.type === 'existing_worktree' ? str(workspace.worktreePath) : '',
    modelKey: `${str(model.instanceId)}:${str(model.model)}`, legacyInterval: schedule.type === 'interval' && num(schedule.everyMs) < 60000 };
}

/** scheduledTaskDefaultModel over the advertised, authenticated catalog. */
export function defaultModelKey(config: Obj, projectId: string): string {
  const available = arr(config.providers).filter(provider => providerAvailable(provider) && obj(provider.auth).status !== 'unauthenticated');
  const settings = obj(config.settings), override = obj(obj(settings.projectSettingsOverrides)[projectId]);
  for (const selection of [obj(override.defaultModelSelection), obj(settings.defaultModelSelection)]) {
    if (available.some(provider => provider.instanceId === selection.instanceId && arr(provider.models).some(model => model.slug === selection.model && model.isLegacy !== true))) return `${selection.instanceId}:${selection.model}`;
  }
  const models = available.flatMap(provider => arr(provider.models).filter(model => model.isLegacy !== true).map(model => ({ provider, model })));
  const fallback = models.find(entry => entry.model.isDefault === true) || models[0];
  return fallback ? `${fallback.provider.instanceId}:${fallback.model.slug}` : '';
}

type Choice = { value: string; label: string; selected: boolean };
type BranchRef = { value: string; label: string; search: string; badge: string };
/** BranchPickerRefItem: the name and its tag (current, worktree, remote, default). */
export function branchRef(ref: Obj, projectCwd: string): BranchRef {
  const name = str(ref.name), worktree = str(ref.worktreePath);
  const badge = ref.current === true ? 'current' : worktree && projectCwd && worktree !== projectCwd ? 'worktree' : ref.isRemote === true ? 'remote' : ref.isDefault === true ? 'default' : '';
  return { value: name, label: name, search: name.toLowerCase(), badge };
}
export async function scheduledPage(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, editor: string, editingId: string, active: boolean, now = 0) {
  const empty = { available: false, writable: false, error: '', loading: false, environment: '', scope: `${environmentId}:${projectId}`, missing: false,
    tasks: [] as { id: string; title: string; prompt: string; status: string; runStatus: string; runError: string; enabled: boolean; first: boolean }[],
    editors: [] as (ReturnType<typeof editorDraft> & { key: string; missing: boolean })[], projects: [] as Choice[], models: [] as Choice[], workspaces: [] as Choice[], environments: [] as Choice[],
    branches: [] as { projectId: string; error: string; refs: BranchRef[] }[], marks: [] as { value: string; name: string; driver: string; badge: string; accent: string }[] };
  if (!active) return empty;
  try {
    if (!native?.available || !client.ready || (environmentId !== "" && environmentId !== client.environmentId) || (projectId && !client.shell.projects.some(project => project.id === projectId))) throw new Error('This scope is unavailable. Choose a connected environment and an existing checkout.');
    const access = client.restAccess(native);
    const all = arr((await access.request('scheduledTasks.list', {})).tasks);
    const tasks = all.filter(task => !projectId || task.projectId === projectId);
    const config = client.config;
    const projects = client.shell.projects.filter(project => !projectId || project.id === projectId);
    const environment = str(obj(config.environment).label, str(obj(config.environment).environmentId, 'This environment'));
    const base = { ...empty, available: true, writable: client.writable, environment,
      tasks: tasks.map((task, index) => ({ id: str(task.id), title: str(task.title), prompt: str(task.prompt), status: taskStatus(task, now), runStatus: str(task.lastRunStatus) === 'never' ? '' : str(task.lastRunStatus), runError: str(task.lastRunError), enabled: task.enabled === true, first: index === 0 })) };
    if (editor !== 'task') return base;
    const task = editingId ? all.find(entry => entry.id === editingId) : undefined;
    const firstProject = str(projects[0]?.id);
    const draft = editorDraft(task, firstProject, defaultModelKey(config, firstProject));
    const providers = arr(config.providers).filter(provider => providerAvailable(provider) && obj(provider.auth).status !== 'unauthenticated');
    const models = providers.flatMap(provider => arr(provider.models).filter(model => model.isUnavailable !== true && model.isLegacy !== true).map(model => ({ value: `${provider.instanceId}:${model.slug}`, label: `${str(model.name, str(model.slug))} · ${str(provider.displayName, str(provider.instanceId))}`, selected: false })));
    // The trigger shows the model's name with its provider mark (ProviderModelPicker).
    const marks = providers.flatMap(provider => arr(provider.models).filter(model => model.isUnavailable !== true && model.isLegacy !== true).map(model => {
      const badge = providerBadge(provider, arr(config.providers));
      return { value: `${provider.instanceId}:${model.slug}`, name: str(model.name, str(model.slug)), driver: str(provider.driver), badge: badge.providerBadge, accent: badge.providerBadgeColor };
    }));
    if (draft.modelKey && !models.some(model => model.value === draft.modelKey)) models.unshift({ value: draft.modelKey, label: draft.modelKey.slice(draft.modelKey.indexOf(':') + 1), selected: false });
    // Base-branch refs for each selectable project (WorktreeBaseBranchPicker's vcs.listRefs).
    const branches = await Promise.all(projects.map(async project => {
      try {
        // VcsListRefsInput's query is optional and non-empty: an empty search omits it (usePaginatedBranches).
        const refs = arr((await access.request('vcs.listRefs', { cwd: str(project.workspaceRoot), limit: 100 })).refs);
        return { projectId: str(project.id), error: '', refs: refs.map(ref => branchRef(ref, str(project.workspaceRoot))) };
      } catch (error) { return { projectId: str(project.id), error: error instanceof Error ? error.message : 'Could not load refs.', refs: [] as BranchRef[] }; }
    }));
    return { ...base, missing: Boolean(editingId) && !task,
      editors: [{ ...draft, key: `${editingId || 'new'}`, missing: Boolean(editingId) && !task }],
      projects: projects.map(project => ({ value: str(project.id), label: str(project.title), selected: false })), models,
      workspaces: ['worktree', 'root', 'existing_worktree'].map(value => ({ value, label: WORKSPACE_LABELS[value], selected: false })),
      environments: [{ value: environmentId, label: environment, selected: true }], branches, marks };
  } catch (error) { return { ...empty, error: error instanceof Error ? error.message : 'Could not load scheduled tasks.' }; }
}

/** The editor's submit(): field names from Contract, validation messages from the reference. */
export function taskInput(input: Record<string, string>, existing: Obj | undefined): Obj {
  const fail = (title: string, detail: string) => { throw new Error(`${title}: ${detail}`); };
  const title = str(input.title).trim(), prompt = str(input.prompt).trim(), projectId = str(input.projectId), modelKey = str(input.modelKey);
  const split = modelKey.indexOf(':');
  if (!title || !prompt || !projectId || split <= 0 || split === modelKey.length - 1) fail('Scheduled task is incomplete', 'Add a title, prompt, project, and model.');
  let schedule: Obj;
  if (input.scheduleMode === 'interval') {
    const everyMs = Math.round(Number(input.intervalMinutes) * 60000);
    if (!Number.isSafeInteger(everyMs) || everyMs < 60000) fail('Invalid interval', 'Enter an interval of at least one minute.');
    schedule = { type: 'interval', everyMs };
  } else {
    const days = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'].flatMap((day, index) => input[day] === 'true' ? [index] : []);
    const time = str(input.timeOfDay) || '09:00';
    if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(time)) fail('Invalid time', 'Choose a time of day.');
    schedule = { type: 'fixed_time', timeOfDay: time, ...(days.length === 0 || days.length === 7 ? {} : { weekdays: days }) };
  }
  const mode = str(input.workspaceMode, 'worktree');
  if (mode === 'existing_worktree' && !str(input.path).trim()) fail('Checkout path is required', 'Enter the path of the checkout to run in.');
  const workspaceStrategy = mode === 'root' ? { type: 'root' } : mode === 'existing_worktree' ? { type: 'existing_worktree', worktreePath: str(input.path).trim() } : { type: 'worktree', baseRef: str(input.baseRef).trim() || 'main', startFromOrigin: input.startFromOrigin === 'true' };
  const selection = { instanceId: modelKey.slice(0, split), model: modelKey.slice(split + 1) };
  const original = obj(existing?.modelSelection);
  return { ...(input.id ? { id: input.id, requireExisting: true } : {}), title, prompt, enabled: input.enabled === 'true', schedule, projectId,
    threadId: input.threadId ? input.threadId : null, workspaceStrategy,
    modelSelection: original.instanceId === selection.instanceId && original.model === selection.model ? original : selection,
    runtimeMode: str(existing?.runtimeMode, 'full-access'), interactionMode: str(existing?.interactionMode, 'default'), creationSource: 'web' };
}
