// Pinned mobile NewTask{Route,Draft,ContextPicker} screens at365aa87982; shared draft and launch ownership.
// @ref llp/1106.005-composer-and-transcript.decision.md#new-task-ownership
import { mobileClient, mobileCommand, mobileNative } from './client';
import { mobileHomeSources } from './home';
import { mobileSessionGrants } from './environment-detail';
import { mobileThreadComposer, type ThreadComposerState } from './thread';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, nativeFiles, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { fleet, type EnvironmentFleet } from './shared/settings-b-fleet';
import { environmentOptions } from './shared/r4-git-env';
import { draftContext, composerBranches, patchDraftContext } from './shared/composer-controls-branch';
import { branchState, cardBranchView, startFromOrigin } from './shared/r4-git-branch';
import { mobileSend } from './composer-behavior';
import { mobileDraftChanged } from './draft';
import { isScratch, scratchRootOf } from './shared/r12-threads-scratch';

export interface NewTaskProject { id: string; environmentId: string; projectId: string; title: string; subtitle: string; path: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskEnvironment { id: string; label: string; machine: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskBranch { id: string; label: string; badge: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskSnapshot { revision: number; environmentId: string; projectId: string; threadId: string; projectTitle: string; environmentLabel: string;
  projects: NewTaskProject[]; environments: NewTaskEnvironment[]; branches: NewTaskBranch[]; query: string; branchQuery: string;
  emptyTitle: string; emptyDetail: string; branchEmpty: string; error: string; busy: boolean; branchLoaded: boolean; branchHasMore: boolean;
  canSelect: boolean; draft: boolean; scratch: boolean; workspaceMode: string; workspaceLabel: string; branchLabel: string; originOn: boolean;
  composer: ThreadComposerState; }
export interface NewTaskResult { revision: number; message: string; submitted: boolean; environmentId: string; projectId: string; threadId: string }
interface TaskState { owner: string; busy: boolean; error: string; branchLoaded: boolean; branchHasMore: boolean; canWriteGit: boolean; branches: NewTaskBranch[]; branchQuery: string; }
const states = new WeakMap<T3Client, TaskState>();
const owner = (client: T3Client) => JSON.stringify([client.generation, client.draftKey]);
function stateFor(client: T3Client) {
  let state = states.get(client);
  if (!state || state.owner !== owner(client)) {
    state = { owner: owner(client), busy: false, error: '', branchLoaded: false, branchHasMore: false, canWriteGit: false, branches: [], branchQuery: '' };
    states.set(client, state);
  }
  return state;
}
function projectKey(environmentId: string, projectId: string) { return JSON.stringify([environmentId, projectId]); }
const machineSymbols: Record<string, string> = { server: 'server.rack', cloud: 'cloud', linux: 'terminal', desktop: 'desktopcomputer', laptop: 'laptopcomputer', 'mac-mini': 'macmini', 'mac-studio': 'macstudio' };
/** Source-derived project chooser. Each action key contains both identities, never a bare cross-environment project id. */
export function mobileNewTask(query = '', client: T3Client = mobileClient, background: EnvironmentFleet = fleet): NewTaskSnapshot {
  const state = stateFor(client), sources = mobileHomeSources(client, background), needle = query.trim().toLocaleLowerCase();
  const context = draftContext(client), canSelect = !state.busy && !client.busy && !client.pending;
  const projects = sources.flatMap(source => source.shell.projects.filter(project => project.archivedAt == null && !isScratch(project, scratchRootOf(true, source.config)))
    .filter(project => !needle || [str(project.title), str(project.workspaceRoot)].some(value => value.toLocaleLowerCase().includes(needle)))
    .map(project => ({ id: projectKey(source.environmentId, str(project.id)), environmentId: source.environmentId, projectId: str(project.id), title: str(project.title),
      subtitle: source.label, path: str(project.workspaceRoot), selected: source.environmentId === client.environmentId && project.id === client.projectId,
      disabled: !canSelect || source.focused && !client.ready, last: false })));
  projects.forEach((row, index) => { row.last = index === projects.length - 1; });
  const environments = environmentOptions(client, background).map((environment, index, all) => ({ id: environment.id, label: environment.label,
    machine: machineSymbols[environment.machine] ?? 'server.rack', selected: environment.selected, disabled: !canSelect || !!client.threadId, last: index === all.length - 1 }));
  const project = client.shell.projects.find(project => project.id === client.projectId), scratch = !!project && isScratch(project, scratchRootOf(client.ready, client.config));
  const connecting = ['connecting', 'reconnecting'].includes(client.connection), hasConnections = !!client.environmentId || background.saved.length > 0;
  const emptyTitle = needle ? 'No matching projects' : !hasConnections ? 'No environments connected' : connecting && !client.shellLoaded ? 'Connecting to environment' : sources.length ? 'No projects found' : 'Environment unavailable';
  const emptyDetail = needle ? 'Try a different project name or workspace path.' : !hasConnections ? 'Add an environment before creating a task.' : connecting && !client.shellLoaded
    ? 'Loading projects from the saved environment.' : sources.length ? 'The connected environment did not report any projects.' : client.error || 'The saved environment is offline. Check the URL or start the environment, then retry.';
  const composer = { ...mobileThreadComposer(client), placeholder: 'Ask anything…' };
  composer.canSend &&= !client.threadId && canSelect && (context.envMode !== 'worktree' || !!context.branch);
  return { revision: client.revision, environmentId: client.environmentId, projectId: client.projectId, threadId: client.threadId,
    projectTitle: str(project?.title), environmentLabel: environments.find(environment => environment.selected)?.label ?? str(obj(client.config.environment).label),
    projects, environments, branches: state.branches, query, branchQuery: state.branchQuery, emptyTitle, emptyDetail,
    branchEmpty: state.branchLoaded ? state.error || (state.branchQuery ? 'No matching branches' : 'No branches available') : 'Loading branches…',
    error: state.error, busy: state.busy, branchLoaded: state.branchLoaded, branchHasMore: state.branchHasMore, canSelect,
    draft: !client.threadId && !!project, scratch, workspaceMode: context.envMode,
    workspaceLabel: context.envMode === 'worktree' ? 'New worktree' : context.worktreePath ? 'Current worktree' : 'Current checkout',
    branchLabel: context.branch || 'Select branch', originOn: startFromOrigin(client), composer };
}

/** Awaited root resource, real repository reads; only projected rows/permission booleans survive the answer. */
export async function mobileNewTaskPrepare(branchQuery: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const state = stateFor(client), expected = owner(client);
  if (!nativeInput?.available || !client.ready || client.threadId || !client.projectId) return { revision: client.revision, loaded: false };
  const native = letGoAware(mobileNative(nativeInput)), context = draftContext(client);
  const root = str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot), cwd = context.worktreePath || root;
  try {
    const session = await client.http(native, '/api/auth/session');
    if (owner(client) !== expected) return { revision: client.revision, loaded: false };
    state.canWriteGit = mobileSessionGrants(session, 'source-control:write');
    const strip = await composerBranches(client, native, false, '', true);
    const branch = branchState(client); branch.open = 'branch'; branch.query = branchQuery;
    let view = await cardBranchView(client, native, cwd, root, strip.show);
    // The shared pager first marks loading; this awaited mobile resource owns the ensuing read.
    if (branch.refs?.loadingMore) view = await cardBranchView(client, native, cwd, root, strip.show);
    if (owner(client) !== expected) return { revision: client.revision, loaded: false };
    state.branchQuery = branchQuery; state.branchLoaded = !view.disabled; state.error = view.disabled && strip.show ? 'Could not load branches.' : '';
    state.branchHasMore = branch.refs?.nextCursor != null;
    const isBase = context.envMode === 'worktree' && !context.worktreePath;
    state.branches = view.refs.map((ref, index, all) => ({ id: ref.name, label: ref.name, badge: ref.badge, selected: ref.selected,
      disabled: state.busy || (!isBase && !state.canWriteGit && !['current', 'worktree'].includes(ref.badge)), last: index === all.length - 1 }));
    // Shared draft context owns the launch's base; adopt the actual repository fallback only after a real read.
    if (!context.branch && view.value && !client.threadId) client.local.composerControls.contexts[client.draftKey] = { ...draftContext(client), branch: view.value };
  } catch (error) {
    if (letGo(error)) throw error;
    if (owner(client) === expected) { state.error = error instanceof Error ? error.message : 'Could not load branches.'; state.branchLoaded = true; }
  }
  return { revision: client.revision, loaded: state.branchLoaded };
}

/** Mobile route adapter; all server writes and draft dispatch remain the shared client's. */
export async function mobileNewTaskAction(kind: string, id: string, value: string, nativeInput: Native | null | undefined, suppliedStorage: Files,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet): Promise<NewTaskResult> {
  const state = stateFor(client), result = (message = '', submitted = false) => ({ revision: client.revision, message, submitted,
    environmentId: client.environmentId, projectId: client.projectId, threadId: client.threadId });
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to create a task.');
  const native = letGoAware(mobileNative(nativeInput)), storage = client === mobileClient ? nativeFiles(native) : suppliedStorage;
  // Keystrokes have their own owner-safe shared reducer path, outside task transitions.
  if (kind === 'draft') {
    const saved = await mobileDraftChanged(client, value, native, storage);
    return { ...result(saved.message), revision: saved.revision };
  }
  if (state.busy) return result('Wait for the current task change to finish.');
  const command = (op: string, id = '', value = '') => client === mobileClient ? mobileCommand([op, id, value], nativeInput, suppliedStorage)
    : op === 'send-alternate' ? mobileSend(client, true, native, storage) : client.command(op, id, value, 0, native, storage);
  const run = async (op: string, id = '', value = '') => { const response = await command(op, id, value); if (response.message) throw new ClientError(response.message); };
  state.busy = true; state.error = '';
  try {
    if (client.busy || client.pending) throw new ClientError('Wait for the current submission to finish before changing tasks.');
    if (kind === 'project') {
      const target = mobileNewTask('', client, background).projects.find(project => project.id === id);
      if (!target) throw new ClientError('That project is no longer available.');
      if (target.environmentId !== client.environmentId) {
        const entry = [...background.entries.values()].find(entry => entry.environmentId === target.environmentId && entry.phase === 'connected' && entry.synchronized === entry.generation);
        if (!entry) throw new ClientError('That environment is no longer connected.');
        await run('connect', entry.origin);
        await client.synchronize(native);
        if (client.environmentId !== target.environmentId) throw new ClientError('The connected environment did not match the selected project.');
      }
      if (!client.shell.projects.some(project => project.id === target.projectId)) throw new ClientError('That project is no longer available.');
      await run('new-thread', target.projectId);
    } else if (kind === 'environment') {
      if (client.threadId) throw new ClientError('A started thread keeps its environment.');
      await run('environment-run-on', '', id);
    } else if (kind === 'workspace') {
      await run('cclocal:env-mode', '', value);
      const next = stateFor(client); next.branchLoaded = false; next.branches = [];
    } else if (kind === 'origin') await run('chatlocal:git-origin');
    else if (kind === 'branch') {
      if (client.threadId || !state.branchLoaded) throw new ClientError('Reload branches before selecting one.');
      const branch = state.branches.find(branch => branch.id === id);
      if (!branch || branch.disabled) throw new ClientError('This connection cannot check out that branch.');
      const known = branchState(client).refs?.refs.find(ref => ref.name === id);
      const context = draftContext(client);
      if (known?.current === true && context.envMode === 'local' && !context.worktreePath) patchDraftContext(client, { branch: id });
      else {
        await run('shell:git-branch', '', id);
        const expected = known?.isRemote === true && context.envMode === 'local' ? id.replace(/^[^/]+\//, '') : id;
        if (draftContext(client).branch !== expected) throw new ClientError('The branch could not be checked out.');
      }
      state.branchLoaded = false;
    } else if (kind === 'branch-more') {
      const ends = obj(client.presentation.scrollEnds);
      client.presentation.scrollEnds = { ...ends, 'scroll:details-refs': Number(ends['scroll:details-refs'] ?? 0) + 1 };
    }
    else if ((kind === 'send' || kind === 'send-alternate')) {
      if (client.threadId) throw new ClientError('Open a new task draft before sending.');
      const environmentId = client.environmentId, projectId = client.projectId, generation = client.generation;
      await run(kind);
      const submitted = !!client.threadId && environmentId === client.environmentId && projectId === client.projectId && generation === client.generation;
      return result('', submitted);
    } else throw new ClientError('Unknown new task action.');
    return result();
  } catch (error) {
    if (letGo(error)) throw error;
    state.error = error instanceof Error ? error.message : 'Could not change the task.'; return result(state.error);
  } finally { state.busy = false; client.revision++; }
}
