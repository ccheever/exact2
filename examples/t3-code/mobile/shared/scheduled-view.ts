// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/scheduled-view.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Settings → Scheduled tasks view model (MIT reference, see LICENSE-T3; T3 Code 1e2ecbd975:
// components/settings/ScheduledTasksSettings.tsx ScheduledTasksSettings,
// ScheduledTaskEnvironmentSection, ScheduledTaskRow, ScheduledTaskEditorDialog): one section
// per environment in scope (a heading only when there is more than one), each fed by its live
// `scheduledTasks.subscribe` list (live-streams.ts) with its loading, error and "Environment
// disconnected" states, the `taskId` deep link that opens the editor once, and the editor's
// draft, choices and branch refs. The editor never enables or runs a task by itself; Enabled
// is the user's own switch. Labels and the scope rule are scheduled-tasks.ts ports.
import { arr, obj, str, num, type Obj } from './domain';
import type { T3Client } from './client';
import { providerAvailable, type Native } from './protocol';
import { providerBadge } from './presentation';
import { emptyDraft, matchesScheduledTaskScope, resolveTaskScope, scheduledTaskDefaultModel, taskStatus, taskToDraft, type TaskScope } from './scheduled-tasks';
import { liveEnvironments, watchLive, type LiveEnvironment } from './live-streams';
export const WORKSPACE_LABELS: Record<string, string> = { worktree: 'Create a new worktree', root: 'Use the project checkout', existing_worktree: 'Use a specific checkout' };
export { scheduleLabel, relativeLabel as runLabel, taskStatus } from './scheduled-tasks';

/** The editor's Contract draft from taskToDraft / EMPTY_DRAFT. */
export function editorDraft(task: Obj | undefined, projectId: string, modelKey: string, environmentId = '') {
  const draft = task ? taskToDraft(task) : emptyDraft(projectId, modelKey);
  const schedule = obj(task?.schedule);
  return { id: draft.editingId, environmentId, title: draft.title, prompt: draft.prompt, enabled: draft.enabled, scheduleMode: draft.scheduleMode, intervalMinutes: draft.intervalMinutes,
    timeOfDay: draft.timeOfDay, sunday: draft.sunday, monday: draft.monday, tuesday: draft.tuesday, wednesday: draft.wednesday, thursday: draft.thursday, friday: draft.friday,
    saturday: draft.saturday, projectId: draft.projectId, threadId: draft.threadId, workspaceMode: draft.workspaceMode, baseRef: draft.baseRef, startFromOrigin: draft.startFromOrigin,
    path: draft.existingWorktreePath, modelKey: draft.modelKey, legacyInterval: schedule.type === 'interval' && num(schedule.everyMs) < 60000 };
}

/** scheduledTaskDefaultModel as the picker's `instanceId:model` key. */
export function defaultModelKey(config: Obj, projectId: string, project: Obj | null = null): string {
  const selection = scheduledTaskDefaultModel(obj(config.settings), project ?? (projectId ? { id: projectId } : null), arr(config.providers));
  return selection ? `${str(selection.instanceId)}:${str(selection.model)}` : '';
}

type Choice = { value: string; label: string; selected: boolean };
type BranchRef = { value: string; label: string; search: string; badge: string };
/** BranchPickerRefItem: the name and its tag (current, worktree, remote, default). */
export function branchRef(ref: Obj, projectCwd: string): BranchRef {
  const name = str(ref.name), worktree = str(ref.worktreePath);
  const badge = ref.current === true ? 'current' : worktree && projectCwd && worktree !== projectCwd ? 'worktree' : ref.isRemote === true ? 'remote' : ref.isDefault === true ? 'default' : '';
  return { value: name, label: name, search: name.toLowerCase(), badge };
}

/** resolveSettingsScope for this page over every live environment (the groups are the focused environment's). */
export function taskScope(client: T3Client, environments: { environmentId: string }[], machine: string, projectKey: string, checkout: string, projectId = ''): TaskScope {
  const member = (project: Obj) => ({ environmentId: client.environmentId, id: str(project.id), physicalProjectKey: str(project.id) });
  const groups = client.projectGroups().map(group => ({ projectKey: group.key, memberProjects: group.members.map(member) }));
  // A bare project id (the Projects route's legacy target) is that project's checkout.
  if (!projectKey && projectId) groups.push({ projectKey: `project:${projectId}`, memberProjects: client.shell.projects.filter(project => project.id === projectId).map(member) });
  return resolveTaskScope({ machine, project: projectKey || (projectId ? `project:${projectId}` : ''), checkout: checkout || (!projectKey && projectId ? projectId : '') }, groups, environments);
}

/** Editing targets: `<environmentId>|<taskId>` from a row, `link|<environmentId>|<taskId>` from a deep link. */
export function editTarget(value: string, fallbackEnvironment: string): { environmentId: string; taskId: string; link: boolean } {
  const parts = value.split('|');
  if (parts[0] === 'link' && parts.length >= 3) return { environmentId: parts[1] || fallbackEnvironment, taskId: parts.slice(2).join('|'), link: true };
  if (parts.length >= 2) return { environmentId: parts[0] || fallbackEnvironment, taskId: parts.slice(1).join('|'), link: false };
  return { environmentId: fallbackEnvironment, taskId: value, link: false };
}

export type TaskRow = { id: string; environmentId: string; title: string; prompt: string; status: string; runStatus: string; runError: string; enabled: boolean; first: boolean };
export type TaskSection = { id: string; label: string; heading: boolean; state: string; title: string; description: string; linkMissing: boolean; tasks: TaskRow[] };

/** ScheduledTaskEnvironmentSection: disconnected, error, loading, then the scoped rows. */
export function taskSection(environment: LiveEnvironment, scope: TaskScope, heading: boolean, linkTaskId: string, now: number): TaskSection {
  const base = { id: environment.environmentId, label: environment.label, heading, linkMissing: false, tasks: [] as TaskRow[] };
  if (!environment.connected) return { ...base, state: 'disconnected', title: 'Environment disconnected', description: `Reconnect ${environment.label} to view its scheduled tasks.` };
  if (environment.tasks.error) return { ...base, state: 'error', title: 'Could not load scheduled tasks', description: environment.tasks.error };
  if (!environment.tasks.value) return { ...base, state: 'loading', title: 'Loading scheduled tasks…', description: '' };
  const tasks = environment.tasks.value.filter(task => matchesScheduledTaskScope(scope, environment.environmentId, str(task.projectId)));
  return { ...base, state: 'ready', title: tasks.length ? '' : 'No scheduled tasks', description: tasks.length ? '' : 'No tasks match this environment and project selection.',
    linkMissing: !!linkTaskId && !tasks.some(task => task.id === linkTaskId),
    tasks: tasks.map((task, index) => ({ id: str(task.id), environmentId: environment.environmentId, title: str(task.title), prompt: str(task.prompt), status: taskStatus(task, now),
      runStatus: str(task.lastRunStatus) === 'never' ? '' : str(task.lastRunStatus), runError: str(task.lastRunError), enabled: task.enabled === true, first: index === 0 })) };
}

export async function scheduledPage(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, editor: string, editingId: string, active: boolean, now = 0, machine = '', projectKey = '', checkout = '') {
  const empty = { available: false, writable: false, error: '', loading: false, environment: '', scope: `${environmentId}:${projectId}`, missing: false,
    tasks: [] as TaskRow[], sections: [] as TaskSection[],
    editors: [] as (ReturnType<typeof editorDraft> & { key: string; missing: boolean })[], projects: [] as Choice[], models: [] as Choice[], workspaces: [] as Choice[], environments: [] as Choice[],
    branches: [] as { projectId: string; error: string; refs: BranchRef[] }[], marks: [] as { value: string; name: string; driver: string; badge: string; accent: string }[] };
  if (!active) return empty;
  try {
    if (!native?.available) throw new Error('Open this app on macOS to connect to T3 Code.');
    await watchLive(client, native);
    const environments = liveEnvironments(client, native);
    if (!environments.length) throw new Error('Connect an environment to manage scheduled tasks.');
    const scope = taskScope(client, environments, machine, projectKey, checkout, projectKey ? '' : projectId);
    if (scope.kind === 'unavailable') throw new Error(scope.message);
    const inScope = environments.filter(environment => scope.environmentIds.includes(environment.environmentId));
    const fallback = (inScope.find(environment => environment.focused && environment.connected) ?? inScope.find(environment => environment.connected) ?? inScope[0])?.environmentId ?? '';
    const target = editor === 'task' && editingId ? editTarget(editingId, fallback) : null;
    const sections = inScope.map(environment => taskSection(environment, scope, inScope.length > 1, target?.link && target.environmentId === environment.environmentId ? target.taskId : '', now));
    const editing = environments.find(environment => environment.environmentId === (target?.environmentId || fallback));
    const base = { ...empty, available: !!editing?.connected, writable: !!editing?.connected && (!editing.focused || client.writable),
      environment: editing?.label ?? '', sections, tasks: sections.flatMap(section => section.tasks) };
    if (editor !== 'task' || !editing) return base;
    const all = editing.tasks.value;
    // A deep link opens the editor once its task is known; a missing one is the section's "Task unavailable".
    if (target?.link && (!all || !all.some(task => task.id === target.taskId))) return base;
    const task = target && all ? all.find(entry => entry.id === target.taskId) : undefined;
    const config = editing.config;
    const projects = editing.shell.projects.filter(project => matchesScheduledTaskScope(scope, editing.environmentId, str(project.id)));
    const firstProject = projects[0] ?? null;
    const draft = editorDraft(task, str(firstProject?.id), defaultModelKey(config, str(firstProject?.id), firstProject), editing.environmentId);
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
        const refs = arr((await editing.request('vcs.listRefs', { cwd: str(project.workspaceRoot), limit: 100 })).refs);
        return { projectId: str(project.id), error: '', refs: refs.map(ref => branchRef(ref, str(project.workspaceRoot))) };
      } catch (error) { return { projectId: str(project.id), error: error instanceof Error ? error.message : 'Could not load refs.', refs: [] as BranchRef[] }; }
    }));
    // editingTaskMissing: the task went away while its editor was open.
    const missing = !!target && !!all && !task;
    return { ...base, missing,
      editors: [{ ...draft, key: `${editing.environmentId}:${target?.taskId || 'new'}`, missing }],
      projects: projects.map(project => ({ value: str(project.id), label: str(project.title), selected: false })), models,
      workspaces: ['worktree', 'root', 'existing_worktree'].map(value => ({ value, label: WORKSPACE_LABELS[value], selected: false })),
      environments: environments.filter(environment => environment.connected).map(environment => ({ value: environment.environmentId, label: environment.label, selected: environment.environmentId === editing.environmentId })),
      branches, marks };
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
