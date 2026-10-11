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
import type { Native } from './protocol';
import { providerBadge } from './presentation';
import { triggerModelName } from './r3-composer-controls-model';
import { emptyDraft, matchesScheduledTaskScope, resolveTaskScope, scheduledTaskDefaultModel, taskStatus, taskToDraft, type TaskScope } from './scheduled-tasks';
import { liveEnvironments, watchLive, type LiveEnvironment } from './live-streams';
import { letGo } from './let-go';
import { sanitizeNewRefName } from './composer-controls-branch';
import { REF_PAGE, firstPage, morePages, pageWanted, refsStatus, scrollEnds, type RefPages } from './r5-composer-paging';
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
type TaskModelMark = { value: string; name: string; driver: string; badge: string; accent: string };

/**
 * The Model trigger's label and mark for any `instance:model` the editor holds (model-picker-parity S2-4, ProviderModelPicker's
 * trigger over getCustomModelOptionsByInstance): every instance's models, the draft's own key when no instance lists it (its
 * slug), and "" for no model: the first instance's first model, or "Choose model" (activeInstanceId falls back to the first).
 */
export function taskModelMarks(providers: Obj[], draftKey: string): TaskModelMark[] {
  const mark = (provider: Obj | undefined, value: string, name: string): TaskModelMark => {
    const badge = provider ? providerBadge(provider, providers) : { providerBadge: '', providerBadgeColor: '' };
    return { value, name, driver: str(provider?.driver), badge: badge.providerBadge, accent: badge.providerBadgeColor };
  };
  const marks = providers.flatMap(provider => arr(provider.models).map(model => mark(provider, `${str(provider.instanceId)}:${str(model.slug)}`, triggerModelName(model) || str(model.slug))));
  if (draftKey && !marks.some(entry => entry.value === draftKey)) {
    const at = draftKey.indexOf(':');
    marks.push(mark(providers.find(provider => provider.instanceId === draftKey.slice(0, at)), draftKey, draftKey.slice(at + 1)));
  }
  const first = providers[0], firstModel = arr(first?.models)[0];
  marks.push(mark(first, '', firstModel ? triggerModelName(firstModel) || str(firstModel.slug) : 'Choose model'));
  return marks;
}
type BranchRef = { value: string; label: string; search: string; badge: string; remote: boolean };
/**
 * BranchPickerRefItem: the name and its tag (current, worktree, remote, default). `remote` is the ref's own isRemote, which
 * the trigger reads (settings-scheduled.contract taskBaseLabel: "origin/" only for a listed local branch).
 */
export function branchRef(ref: Obj, projectCwd: string): BranchRef {
  const name = str(ref.name), worktree = str(ref.worktreePath);
  const badge = ref.current === true ? 'current' : worktree && projectCwd && worktree !== projectCwd ? 'worktree' : ref.isRemote === true ? 'remote' : ref.isDefault === true ? 'default' : '';
  return { value: name, label: name, search: name.toLowerCase(), badge, remote: ref.isRemote === true };
}

/** resolveSettingsScope for this page over every live environment (the groups are the focused environment's). */
export function taskScope(client: T3Client, environments: { environmentId: string }[], machine: string, projectKey: string, checkout: string, projectId = ''): TaskScope {
  const member = (project: Obj) => ({ environmentId: client.environmentId, id: str(project.id), physicalProjectKey: str(project.id) });
  const groups = client.projectGroups().map(group => ({ projectKey: group.key, memberProjects: group.members.map(member) }));
  // A bare project id (Project settings' target) is its project on every environment (settingsScopeOf); one in no group is that project's checkout.
  const group = !projectKey && projectId ? groups.find(entry => entry.memberProjects.some(project => project.id === projectId)) : undefined;
  if (group) return resolveTaskScope({ machine, project: group.projectKey, checkout: '' }, groups, environments);
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

// WorktreeBaseBranchPicker's refs (usePaginatedBranches, vcs.listRefs): pages of VCS_REF_LIST_LIMIT refs per project and
// search, the picker's search sent to the server (sanitizeNewRefName), a scroll toward the list's end loading the next
// page (r5-composer-paging.ts), and the selectedRefQuery lookups by name. Read when the editor opens and kept while it
// stays open, as the picker's query atoms are, never again on a wake or the minute tick; Refresh, a new connection and
// another environment, task or project list read again. (audit-wave-followups-2 FV-2)
// The list the picker shows (its project and search, usePaginatedBranches' targetKey) starts again from its first page
// whenever it changes (`cursors = INITIAL_BRANCH_CURSORS`): a kept list shows its first page, and scrolls of another list
// are not its own. A search not read yet shows "Loading refs..." (its first page pending, `data === null`) over no refs,
// and the answer asks itself again (`t3.notify`, R10Connect.swift's wake) for the read.
type ProjectRefs = RefPages & { error: string; first: Obj | null; loading?: boolean };
type Refs = { projectId: string; error: string; refs: BranchRef[]; selected: BranchRef[]; status: string };
type Visit = { key: string; pages: Map<string, ProjectRefs>; lookups: Map<string, BranchRef[]>; picker: string; looks: number };
/** The topic the page watches while a search's first page is pending: its own wake asks it again (r10-connect-timing.ts). */
export const TASK_REFS_WAKE = 't3.notify';
const editorRefs = new WeakMap<T3Client, Visit>();
export const TASK_REFS = 'task-refs'; // settings-scheduled.contract's list: `data-anchor="scroll:task-refs"`
const failed = (error: unknown) => (error instanceof Error && error.message.trim() ? error.message : 'Failed to load refs.');

/** The picker's list asked for its next page and has not had it yet (the shell clock keeps asking, r6-polish-refs.ts). */
export function scheduledRefsWanted(client: T3Client): boolean {
  const visit = editorRefs.get(client), pages = visit?.pages.get(visit.picker);
  return !!pages && !pages.error && pageWanted(pages, obj(client.presentation), TASK_REFS);
}

/**
 * The editor's picker as the root holds it (`taskBase`, settings-scheduled.contract `taskPicker`): its project, its base
 * and its search, for the editor `key` only.
 */
export function pickerOf(taskBase: string, key: string): { project: string; ref: string; query: string } | null {
  const params = new URLSearchParams(taskBase);
  return params.get('key') === key ? { project: str(params.get('project')), ref: str(params.get('ref')).trim(), query: str(params.get('query')) } : null;
}

/**
 * BranchPicker's status line: the error, "Loading refs..." while the first page is out, "Loading more refs..." while the
 * next one is, or "Showing N of M refs" while more remain.
 */
export function taskRefsStatus(pages: ProjectRefs): string {
  return pages.error || refsStatus(pages, pages.loading === true);
}

export async function scheduledPage(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, editor: string, editingId: string, active: boolean, now = 0, machine = '', projectKey = '', checkout = '', taskBase = '', refresh = 0) {
  const empty = { available: false, writable: false, error: '', loading: false, environment: '', scope: `${environmentId}:${projectId}`, missing: false,
    tasks: [] as TaskRow[], sections: [] as TaskSection[],
    editors: [] as (ReturnType<typeof editorDraft> & { key: string; missing: boolean })[], projects: [] as Choice[], workspaces: [] as Choice[], environments: [] as Choice[],
    branches: [] as Refs[], marks: [] as TaskModelMark[] };
  if (!active || editor !== 'task') editorRefs.delete(client);
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
    const marks = taskModelMarks(arr(config.providers), draft.modelKey);
    // Base-branch refs for each selectable project (WorktreeBaseBranchPicker's vcs.listRefs), the picker's search and
    // pages for the project it shows, and the base when the shown list does not have it: the picker's selectedRefQuery
    // asks for it by name (limit 10), so a local branch past the first page still reads "From origin/<ref>". The base is
    // the draft's in its project, and the editor's (`taskBase`, settings-scheduled.contract taskPicker). A failed lookup
    // leaves it unknown.
    const key = `${editing.environmentId}:${target?.taskId || 'new'}`;
    const picker = pickerOf(taskBase, key);
    const shown = picker?.project || draft.projectId;
    const search = sanitizeNewRefName(picker?.query ?? '').slice(0, 256);
    const wanted = [{ projectId: draft.projectId, ref: draft.baseRef.trim() }, ...(picker ? [{ projectId: shown, ref: picker.ref }] : [])];
    // Once per editor visit: another connection, task or project list starts a new one.
    const visitKey = [editing.environmentId, editing.key, editing.connected, editing.focused ? client.generation : '', target?.taskId ?? 'new', refresh,
      ...projects.map(project => `${str(project.id)}=${str(project.workspaceRoot)}`)].join('|');
    let visit = editorRefs.get(client);
    if (!visit || visit.key !== visitKey) editorRefs.set(client, visit = { key: visitKey, pages: new Map(), lookups: new Map(), picker: '', looks: 0 });
    // Another list shown (a project, a search): it starts from its first page (usePaginatedBranches' targetKey).
    const moved = visit.picker !== `${shown}\n${search}`, opening = visit.looks++ === 0;
    visit.picker = `${shown}\n${search}`;
    // VcsListRefsInput's query is optional and non-empty: an empty search omits it (usePaginatedBranches). The focused
    // environment's read is shared (T3Transport `share`): an answer asked again before its reply joins it.
    const listRefs = async (payload: Obj) => (editing.focused ? client.restAccess(native).read('vcs.listRefs', payload) : editing.request('vcs.listRefs', payload));
    const kept = visit, presentation = obj(client.presentation);
    let pending = false;
    const branches: Refs[] = await Promise.all(projects.map(async project => {
      const cwd = str(project.workspaceRoot), projectId = str(project.id), query = projectId === shown ? search : '';
      const list = (cursor?: number) => listRefs({ cwd, limit: REF_PAGE, ...(query ? { query } : {}), ...(cursor === undefined ? {} : { cursor }) });
      const pagesKey = `${projectId}\n${query}`, ends = scrollEnds(presentation, TASK_REFS);
      let pages = kept.pages.get(pagesKey);
      if (pages && projectId === shown && moved) {
        // Shown again: its first page, and only a scroll from now on loads the next (a failed read is read again).
        if (pages.error || !pages.first) { kept.pages.delete(pagesKey); pages = undefined; }
        else Object.assign(pages, firstPage(pages.first, ends), { loadingMore: false });
      }
      if (!pages && projectId === shown && !opening) {
        pages = { refs: [], total: 0, nextCursor: null, ends, error: '', first: null, loading: true };
        kept.pages.set(pagesKey, pages);
        pending = true;
      } else if (!pages || pages.loading) {
        let read: ProjectRefs;
        try { const first = await list(); read = { ...firstPage(first, ends), error: '', first }; }
        catch (error) { if (letGo(error)) throw error; read = { refs: [], total: 0, nextCursor: null, ends, error: failed(error), first: null }; }
        if (pages) Object.assign(pages, read, { loading: false }); else kept.pages.set(pagesKey, pages = read);
      } else if (projectId === shown && !pages.error) {
        // A scroll toward the list's end loads the next page (BranchPicker maybeFetchNextBranchPage).
        try { Object.assign(pages, await morePages(pages, presentation, TASK_REFS, list)); }
        catch (error) { if (letGo(error)) throw error; Object.assign(pages, { error: failed(error), loadingMore: false }); }
      }
      const shownPages = pages;
      const listed = shownPages.refs.map(ref => branchRef(ref, cwd));
      // A base another of this project's kept lists has is known (no lookup), so a pending search keeps its label at once.
      const keptRef = (name: string): BranchRef[] | undefined => {
        for (const [key, entry] of kept.pages) {
          const ref = key.startsWith(`${projectId}\n`) ? entry.refs.find(item => str(item.name) === name) : undefined;
          if (ref) return [branchRef(ref, cwd)];
        }
        return undefined;
      };
      const unlisted = [...new Set(wanted.filter(entry => cwd && entry.ref && entry.projectId === projectId && !listed.some(ref => ref.value === entry.ref)).map(entry => entry.ref))];
      const selected = (await Promise.all(unlisted.map(async name => {
        const found = kept.lookups.get(`${projectId}\n${name}`) ?? keptRef(name);
        if (found) return found;
        const refs = await listRefs({ cwd, query: name, limit: 10 }).then(answer => arr(answer.refs)).catch((error: unknown): Obj[] => { if (letGo(error)) throw error; return []; });
        const named = refs.filter(ref => str(ref.name) === name).map(ref => branchRef(ref, cwd));
        kept.lookups.set(`${projectId}\n${name}`, named);
        return named;
      }))).flat();
      return { projectId, error: shownPages.error, refs: listed, selected, status: taskRefsStatus(shownPages) };
    }));
    // The status is drawn first; the page asks again now for the search's first page (watched, so it is asked again
    // after this answer lands, LLP 1016.002 D4).
    if (pending) {
      native.watch(TASK_REFS_WAKE);
      try { await native.later({ op: 'r10Wake', topic: TASK_REFS_WAKE }); } catch (error) { if (letGo(error)) throw error; }
    }
    // editingTaskMissing: the task went away while its editor was open.
    const missing = !!target && !!all && !task;
    return { ...base, missing,
      editors: [{ ...draft, key, missing }],
      projects: projects.map(project => ({ value: str(project.id), label: str(project.title), selected: false })),
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
