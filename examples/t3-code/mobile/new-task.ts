// Pinned mobile NewTask{Route,Draft,ContextPicker} screens at365aa87982; shared draft and launch ownership.
// @ref llp/1107.005-composer-and-transcript.decision.md#new-task-ownership
import { mobileClient, mobileCommand, mobileNative } from './client';
import { mobileHomeProjects, mobileHomeSources } from './home';
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
import { mobileMoveScratch, mobileOpenScratch, mobileScratchTarget } from './new-task-scratch';
import { threadOps } from './shared/client-ops-threads';

export interface NewTaskProject { id: string; environmentId: string; projectId: string; title: string; subtitle: string; path: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskEnvironment { id: string; label: string; machine: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskBranch { id: string; label: string; badge: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskSnapshot { revision: number; environmentId: string; projectId: string; threadId: string; projectTitle: string; environmentLabel: string;
  projects: NewTaskProject[]; environments: NewTaskEnvironment[]; branches: NewTaskBranch[]; query: string; branchQuery: string;
  emptyTitle: string; emptyDetail: string; branchEmpty: string; error: string; busy: boolean; branchLoaded: boolean; branchHasMore: boolean;
  canSelect: boolean; canStartScratch: boolean; scratchTarget: string; hasProjects: boolean; draft: boolean; scratch: boolean; workspaceMode: string; workspaceLabel: string; branchLabel: string; originOn: boolean;
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
      subtitle: str(project.workspaceRoot), path: str(project.workspaceRoot), selected: source.environmentId === client.environmentId && project.id === client.projectId,
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
  composer.canSend &&= !client.threadId && canSelect && (scratch || context.envMode !== 'worktree' || !!context.branch);
  return { revision: client.revision, environmentId: client.environmentId, projectId: client.projectId, threadId: client.threadId,
    projectTitle: str(project?.title), environmentLabel: environments.find(environment => environment.selected)?.label ?? str(obj(client.config.environment).label),
    projects, environments, branches: state.branches, query, branchQuery: state.branchQuery, emptyTitle, emptyDetail,
    branchEmpty: state.branchLoaded ? state.error || (state.branchQuery ? 'No matching branches' : 'No branches available') : 'Loading branches…',
    error: state.error, busy: state.busy, branchLoaded: state.branchLoaded, branchHasMore: state.branchHasMore, canSelect,
    canStartScratch: !!mobileScratchTarget(client, background), scratchTarget: mobileScratchTarget(client, background), hasProjects: projects.length > 0,
    draft: !client.threadId && !!project, scratch, workspaceMode: scratch ? 'local' : context.envMode,
    workspaceLabel: context.envMode === 'worktree' ? 'New worktree' : context.worktreePath ? 'Current worktree' : 'Current checkout',
    branchLabel: context.branch || 'Select branch', originOn: startFromOrigin(client), composer };
}

/** Choose-project scopes reuse the same repository grouping and updated-at order
 * as Home. Selection still carries one actual environment/project pair. */
export function mobileNewTaskChooser(query = '', groupingMode = 'repository', client: T3Client = mobileClient, background: EnvironmentFleet = fleet): NewTaskSnapshot {
  const data = mobileNewTask('', client, background), needle = query.trim().toLowerCase();
  const sources = mobileHomeSources(client, background).map(source => ({ ...source, shell: { ...source.shell,
    projects: source.shell.projects.filter(project => project.archivedAt == null && !isScratch(project, scratchRootOf(true, source.config))) } }));
  const scopes = mobileHomeProjects(sources, { groupingMode, projectSortOrder: 'updated_at' });
  const projects = scopes.flatMap(scope => {
    const members = sources.flatMap(source => source.shell.projects.filter(project => scope.projectKeys.includes(`${source.environmentId}:${str(project.id)}`))
      .map(project => ({ source, project })));
    if (needle && ![scope.title, ...members.flatMap(({ project }) => [str(project.title), str(project.workspaceRoot)])].some(text => text.toLowerCase().includes(needle))) return [];
    const target = members.find(member => member.source.environmentId === client.environmentId) ?? members[0];
    if (!target) return [];
    const row = data.projects.find(project => project.environmentId === target.source.environmentId && project.projectId === target.project.id);
    return row ? [{ ...row, title: scope.title, subtitle: members.length > 1 ? `${members.length} workspaces` : str(target.project.workspaceRoot), last: false }] : [];
  });
  projects.forEach((row, index) => { row.last = index === projects.length - 1; });
  return { ...data, projects, query, hasProjects: scopes.length > 0,
    emptyTitle: needle && scopes.length ? 'No matching projects' : data.emptyTitle,
    emptyDetail: needle && scopes.length ? 'Try a different project name or workspace path.' : data.emptyDetail };
}

/** Awaited root resource, real repository reads; only projected rows/permission booleans survive the answer. */
export async function mobileNewTaskPrepare(branchQuery: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const state = stateFor(client), expected = owner(client);
  if (!nativeInput?.available || !client.ready || client.threadId || !client.projectId || mobileNewTask('', client).scratch) return { revision: client.revision, loaded: false };
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
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet, current: () => boolean = () => true): Promise<NewTaskResult> {
  const state = stateFor(client), result = (message = '', submitted = false) => ({ revision: client.revision, message, submitted,
    environmentId: client.environmentId, projectId: client.projectId, threadId: client.threadId });
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to create a task.');
  const assertCurrent = () => { if (!current()) throw new ClientError('The new task route changed.', 'superseded'); };
  const base = letGoAware(mobileNative(nativeInput));
  const native: Native = { available: base.available, watch: topic => base.watch(topic), later: async request => {
    assertCurrent(); const reply = await base.later(request); assertCurrent(); return reply;
  } };
  const storage = client === mobileClient ? nativeFiles(native) : suppliedStorage;
  // Keystrokes have their own owner-safe shared reducer path, outside task transitions.
  if (kind === 'draft') {
    assertCurrent();
    const saved = await mobileDraftChanged(client, value, native, storage, id);
    return { ...result(saved.message), revision: saved.revision };
  }
  if (state.busy) return result('Wait for the current task change to finish.');
  const command = (op: string, id = '', value = '') => client === mobileClient ? mobileCommand([op, id, value], native, suppliedStorage)
    : op === 'send-alternate' ? mobileSend(client, true, native, storage) : client.command(op, id, value, 0, native, storage);
  const run = async (op: string, id = '', value = '') => { const response = await command(op, id, value); if (response.message) throw new ClientError(response.message); };
  state.busy = true; state.error = '';
  try {
    assertCurrent();
    if (client.busy || client.pending) throw new ClientError('Wait for the current submission to finish before changing tasks.');
    if (kind === 'project' || kind === 'scratch') {
      const target = kind === 'scratch' ? await mobileOpenScratch(id, native, client, background, current)
        : mobileHomeSources(client, background).flatMap(source => source.shell.projects.filter(project => project.archivedAt == null)
          .map(project => ({ environmentId: source.environmentId, projectId: str(project.id) }))).find(project => projectKey(project.environmentId, project.projectId) === id);
      if (!target) throw new ClientError('That project is no longer available.');
      if (target.environmentId !== client.environmentId) {
        const entry = [...background.entries.values()].find(entry => entry.environmentId === target.environmentId && entry.phase === 'connected' && entry.synchronized === entry.generation);
        if (!entry) throw new ClientError('That environment is no longer connected.');
        await run('connect', entry.origin);
        await client.synchronize(native);
        if (client.environmentId !== target.environmentId) throw new ClientError('The connected environment did not match the selected project.');
      }
      if (!client.shell.projects.some(project => project.id === target.projectId)) throw new ClientError('That project is no longer available.');
      assertCurrent();
      // The shared command preamble awaits preferences before reducing selection.
      // Apply its actual selection reducer now so a later project switch cannot
      // be overwritten when an earlier preamble resumes. This does not create a
      // thread or alter either project's existing draft contents.
      const out = { message: '', id: target.projectId, value: '' };
      const reducing = threadOps.call(client, 'new-thread', target.projectId, '', 0, native, storage, out);
      const selected = JSON.stringify([client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch]);
      const assertSelected = () => { if (selected !== JSON.stringify([client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch]))
        throw new ClientError('The selected workspace changed.', 'superseded'); };
      await reducing; assertSelected();
      assertCurrent();
      if (mobileNewTask('', client, background).scratch) patchDraftContext(client, { envMode: 'local', branch: '', worktreePath: '' });
      await client.persist(storage); assertSelected();
    } else if (kind === 'environment') {
      if (client.threadId) throw new ClientError('A started thread keeps its environment.');
      if (id !== client.environmentId && mobileNewTask('', client, background).scratch) {
        // The shared desktop move owns text/context only. Preserve files and an
        // existing destination slot until mobile's independent flow drafts exist.
        if (client.snapshotDrafts.length) throw new ClientError('Remove this draft’s attachments before changing environments.');
        const target = await mobileOpenScratch(mobileScratchTarget(client, background, id), native, client, background, current);
        const key = `${target.environmentId}:new:${target.projectId}`;
        if (client.local.drafts[key] || client.local.snapshotDrafts[key]?.length) throw new ClientError('That environment already has a saved draft without a project. Open it separately to keep both drafts.');
        const moved = await mobileMoveScratch(id, native, client, background, current);
        assertCurrent();
        if (moved.status) client.adoptStatus(moved.status, moved.generation);
        await client.persist(storage);
      } else await run('environment-run-on', '', id);
    } else if (['workspace', 'branch', 'branch-more', 'origin'].includes(kind) && mobileNewTask('', client, background).scratch) {
      throw new ClientError('Tasks without a project run locally without a branch or worktree.');
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
      if (mobileNewTask('', client, background).scratch) patchDraftContext(client, { envMode: 'local', branch: '', worktreePath: '' });
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
