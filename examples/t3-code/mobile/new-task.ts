import { mobileNewTaskPendingContext } from './new-task-pending-context';
import { mobileModelSelectionReady } from './model-availability';
// Pinned mobile NewTask{Route,Draft,ContextPicker} screens at365aa87982; shared draft and launch ownership.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import { mobileNewTaskDraftIsPendingKey, mobileNewTaskDraftNoteBranch, mobileNewTaskDraftLookup, mobileNewTaskDraftPresentation, mobileNewTaskDraftSelectedBranch } from './mobile-new-task-drafts';
import { mobileNewTaskTransferBusy } from './new-task-transfer';
import { mobileNewTaskLaunchPendingOwned } from './mobile-new-task-launch';
import { mobileNewTaskCloneSnapshot } from './new-task-clone';
import { mobileClient, mobileCommand, mobileNative } from './client';
import { mobileHomeProjects, mobileHomeSources } from './home';
import { mobileSessionGrants } from './environment-detail';
import { mobileThreadComposer, type ThreadComposerState } from './thread';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, nativeFiles, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { fleet, type EnvironmentFleet } from './shared/settings-b-fleet';
import { mobileNewTaskEnvironmentMatch, mobileNewTaskEnvironmentSources } from './new-task-selection';
import { draftContext, composerBranches, patchDraftContext } from './shared/composer-controls-branch';
import { branchState, cardBranchView, startFromOrigin } from './shared/r4-git-branch';
import { mobileSend } from './composer-behavior';
import { mobileDraftChanged } from './draft';
import { isScratch, scratchRootOf } from './shared/r12-threads-scratch';
import { mobileOpenScratch, mobileScratchTarget } from './new-task-scratch';
import { threadOps } from './shared/client-ops-threads';

export interface NewTaskProject { id: string; environmentId: string; projectId: string; title: string; subtitle: string; path: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskEnvironment { id: string; label: string; machine: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskBranch { id: string; label: string; badge: string; selected: boolean; disabled: boolean; last: boolean }
export interface NewTaskSnapshot { pendingEditor: boolean; revision: number; environmentId: string; projectId: string; threadId: string; projectTitle: string; environmentLabel: string;
  projects: NewTaskProject[]; environments: NewTaskEnvironment[]; branches: NewTaskBranch[]; query: string; branchQuery: string;
  emptyTitle: string; emptyDetail: string; branchEmpty: string; error: string; busy: boolean; branchLoaded: boolean; branchHasMore: boolean;
  canSelect: boolean; canAddProject: boolean; canStartScratch: boolean; scratchTarget: string; hasProjects: boolean; draft: boolean; scratch: boolean; workspaceMode: string; workspaceLabel: string; branchLabel: string; originOn: boolean;
  composer: ThreadComposerState; }
export interface NewTaskResult { revision: number; message: string; submitted: boolean; environmentId: string; projectId: string; threadId: string }
interface TaskState { owner: string; prepare: number; busy: boolean; error: string; branchLoaded: boolean; branchHasMore: boolean; canWriteGit: boolean; branches: NewTaskBranch[]; branchQuery: string; }
const states = new WeakMap<T3Client, TaskState>();
const owner = (client: T3Client) => JSON.stringify([client.generation, client.origin, client.environmentId, client.projectId, client.threadId,
  client.draftKey, client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot]);
function stateFor(client: T3Client) {
  let state = states.get(client);
  if (!state || state.owner !== owner(client)) {
    state = { owner: owner(client), prepare: 0, busy: false, error: '', branchLoaded: false, branchHasMore: false, canWriteGit: false, branches: [], branchQuery: '' };
    states.set(client, state);
  }
  return state;
}
function projectKey(environmentId: string, projectId: string) { return JSON.stringify([environmentId, projectId]); }
const machineSymbols: Record<string, string> = { server: 'server.rack', cloud: 'cloud', linux: 'terminal', desktop: 'desktopcomputer', laptop: 'laptopcomputer', 'mac-mini': 'macmini', 'mac-studio': 'macstudio' };
/** Source-derived project chooser. Each action key contains both identities, never a bare cross-environment project id. */
export function mobileNewTask(query = '', client: T3Client = mobileClient, background: EnvironmentFleet = fleet): NewTaskSnapshot {
  const state = stateFor(client), sources = mobileHomeSources(client, background), needle = query.trim().toLocaleLowerCase();
  const context = draftContext(client), pendingDraft = mobileNewTaskLaunchPendingOwned(client), pendingEditor = mobileNewTaskPendingContext(client);
  const canSelect = !state.busy && ((!client.busy && !client.pending) || pendingDraft);
  const projects = sources.flatMap(source => source.shell.projects.filter(project => project.archivedAt == null && !isScratch(project, scratchRootOf(true, source.config)))
    .filter(project => !needle || [str(project.title), str(project.workspaceRoot)].some(value => value.toLocaleLowerCase().includes(needle)))
    .map(project => ({ id: projectKey(source.environmentId, str(project.id)), environmentId: source.environmentId, projectId: str(project.id), title: str(project.title),
      subtitle: str(project.workspaceRoot), path: str(project.workspaceRoot), selected: source.environmentId === client.environmentId && project.id === client.projectId,
      disabled: !canSelect || pendingDraft && !source.focused || source.focused && !client.ready, last: false })));
  projects.forEach((row, index) => { row.last = index === projects.length - 1; });
  const currentProject = client.shell.projects.find(project => project.id === client.projectId) ?? null;
  const scratchSelection = !!currentProject && isScratch(currentProject, scratchRootOf(client.ready, client.config));
  const choices = scratchSelection ? sources.filter(source => !!mobileScratchTarget(client, background, source.environmentId))
    : mobileNewTaskEnvironmentSources(sources, currentProject);
  const environments = choices.map((source, index, all) => ({ id: source.environmentId, label: source.label || source.environmentId,
    machine: machineSymbols[source.machine] ?? 'server.rack', selected: source.environmentId === client.environmentId,
    disabled: !canSelect || !!client.threadId || !!client.pending || source.connected === false, last: index === all.length - 1 }));
  const project = client.shell.projects.find(project => project.id === client.projectId), scratch = !!project && isScratch(project, scratchRootOf(client.ready, client.config));
  const connecting = ['connecting', 'reconnecting'].includes(client.connection), hasConnections = !!client.environmentId || background.saved.length > 0;
  const emptyTitle = needle ? 'No matching projects' : !hasConnections ? 'No environments connected' : connecting && !client.shellLoaded ? 'Connecting to environment' : sources.length ? 'No projects found' : 'Environment unavailable';
  const emptyDetail = needle ? 'Try a different project name or workspace path.' : !hasConnections ? 'Add an environment before creating a task.' : connecting && !client.shellLoaded
    ? 'Loading projects from the saved environment.' : sources.length ? 'The connected environment did not report any projects.' : client.error || 'The saved environment is offline. Check the URL or start the environment, then retry.';
  const canAddProject = !!client.environmentId && client.connection === 'connected'
    || [...background.entries.values()].some(entry => entry.phase === 'connected'
      && background.saved.some(saved => saved.environmentId === entry.environmentId && saved.enabled !== false));
  const hasProjects = sources.some(source => source.shell.projects.some(project => project.archivedAt == null
    && !isScratch(project, scratchRootOf(true, source.config))));
  const composer = { ...mobileThreadComposer(client), placeholder: 'Ask anything…' };
  const clone = mobileNewTaskCloneSnapshot(client);
  if (clone.blocked) {
    composer.canSend = false;
    composer.blockedReason = clone.pending || clone.phase === 'running' ? 'Cloning repository' : 'Repository not cloned';
    composer.sendLabel = composer.blockedReason;
  }
  if (!client.providerId || !client.modelId) composer.modelLabel = 'Choose model';
  // Local durable admission remains available offline; actual online grants,
  // workspace defaults and capture ownership are rechecked by the submit adapter.
  if (client.connection !== 'connected') composer.blockedReason = '';
  composer.canSend = !!client.draft.trim() && !!client.providerId && !!client.modelId && !composer.modelUnavailable
    && client.preferencesLoaded && !client.threadId && !client.pending && !client.busy && canSelect && !clone.blocked
    && !mobileNewTaskTransferBusy(client, client.draftKey) && (client.connection !== 'connected' || composer.canOperate
      && mobileModelSelectionReady(client.config, { instanceId: client.providerId, model: client.modelId }))
    && (scratch || context.envMode !== 'worktree' || !!context.branch);
  if (pendingEditor) {
    composer.canSend = !!client.draft.trim() && !!client.providerId && !!client.modelId && !client.busy;
    composer.sendLabel = 'Save changes'; composer.showReadOnlyNotice = false; composer.blockedReason = '';
  }
  return { pendingEditor: !!pendingEditor, revision: client.revision, environmentId: client.environmentId, projectId: client.projectId, threadId: client.threadId,
    projectTitle: pendingEditor?.title ?? str(project?.title), environmentLabel: environments.find(environment => environment.selected)?.label ?? str(obj(client.config.environment).label),
    projects, environments, branches: state.branches, query, branchQuery: state.branchQuery, emptyTitle, emptyDetail,
    branchEmpty: state.branchLoaded ? state.error || (state.branchQuery ? 'No matching branches' : 'No branches available') : 'Loading branches…',
    error: state.error, busy: state.busy, branchLoaded: state.branchLoaded, branchHasMore: state.branchHasMore, canSelect, canAddProject,
    canStartScratch: !!mobileScratchTarget(client, background), scratchTarget: mobileScratchTarget(client, background), hasProjects,
    draft: !client.threadId && (!!project || !!pendingEditor), scratch, workspaceMode: scratch ? 'local' : context.envMode,
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
  const state = stateFor(client), expected = owner(client), request = ++state.prepare;
  if (mobileNewTaskPendingContext(client) || !nativeInput?.available || !client.ready || client.threadId || !client.projectId || mobileNewTask('', client).scratch) return { revision: client.revision, loaded: false };
  const native = letGoAware(mobileNative(nativeInput)), context = draftContext(client);
  const stamp = () => JSON.stringify([draftContext(client), mobileNewTaskDraftLookup(client, client.draftKey)?.branchChoice]);
  const captured = stamp(), current = () => owner(client) === expected && state.prepare === request && stamp() === captured;
  const root = str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot), cwd = context.worktreePath || root;
  try {
    const session = await client.http(native, '/api/auth/session');
    if (!current()) return { revision: client.revision, loaded: false };
    state.canWriteGit = mobileSessionGrants(session, 'source-control:write');
    const strip = await composerBranches(client, native, false, '', true);
    if (!current()) return { revision: client.revision, loaded: false };
    const branch = branchState(client); branch.open = 'branch'; branch.query = branchQuery;
    let view = await cardBranchView(client, native, cwd, root, strip.show, current);
    // The shared pager first marks loading; this awaited mobile resource owns the ensuing read.
    if (branch.refs?.loadingMore) view = await cardBranchView(client, native, cwd, root, strip.show, current);
    if (!current()) return { revision: client.revision, loaded: false };
    state.branchQuery = branchQuery; state.branchLoaded = !view.disabled; state.error = view.disabled && strip.show ? 'Could not load branches.' : '';
    state.branchHasMore = branch.refs?.nextCursor != null;
    const isBase = context.envMode === 'worktree' && !context.worktreePath;
    state.branches = view.refs.map((ref, index, all) => ({ id: ref.name, label: ref.name, badge: ref.badge, selected: ref.selected,
      disabled: state.busy || (!isBase && !state.canWriteGit && !['current', 'worktree'].includes(ref.badge)), last: index === all.length - 1 }));
    // A guarded shared read never writes into the live draft. Adopt its automatic
    // fallback only after the request and captured workspace still match.
    const draft = mobileNewTaskDraftPresentation(client, client.draftKey);
    const automatic = !context.branch || draft?.branchChoice?.kind === 'automatic' && mobileNewTaskDraftSelectedBranch(draft) !== undefined;
    if (automatic && view.value) {
      patchDraftContext(client, { branch: view.value });
      mobileNewTaskDraftNoteBranch(client, draft?.branchChoice?.kind === 'explicit' ? 'explicit' : 'automatic');
    }
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) { state.error = error instanceof Error ? error.message : 'Could not load branches.'; state.branchLoaded = true; }
  }
  return { revision: client.revision, loaded: state.branchLoaded };
}

/** Mobile route adapter; all server writes and draft dispatch remain the shared client's. */
export async function mobileNewTaskAction(kind: string, id: string, value: string, nativeInput: Native | null | undefined, suppliedStorage: Files,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet, current: () => boolean = () => true): Promise<NewTaskResult> {
  const state = stateFor(client), result = (message = '', submitted = false) => ({ revision: client.revision, message, submitted,
    environmentId: client.environmentId, projectId: client.projectId, threadId: client.threadId });
  if (mobileNewTaskDraftIsPendingKey(client.draftKey) && kind !== 'draft') return result('Use the pending task editor to save these changes.');
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
    : op === 'send' || op === 'send-alternate' ? mobileSend(client, op === 'send-alternate', native, storage) : client.command(op, id, value, 0, native, storage);
  const run = async (op: string, id = '', value = '') => { const response = await command(op, id, value); if (response.message) throw new ClientError(response.message); };
  state.busy = true; state.error = '';
  try {
    assertCurrent();
    const localSelection = kind === 'project' && id.startsWith(`[${JSON.stringify(client.environmentId)},`) && mobileNewTaskLaunchPendingOwned(client);
    if ((client.busy || client.pending) && !localSelection) throw new ClientError('Wait for the current submission to finish before changing tasks.');
    if (kind === 'project' || kind === 'scratch' || kind === 'environment') {
      if (kind === 'environment' && client.threadId) throw new ClientError('A started thread keeps its environment.');
      if (kind === 'environment' && id === client.environmentId) return result();
      const movingScratch = kind === 'environment' && mobileNewTask('', client, background).scratch;
      const environmentSource = kind === 'environment' && !movingScratch
        ? mobileNewTaskEnvironmentSources(mobileHomeSources(client, background), client.shell.projects.find(project => project.id === client.projectId) ?? null)
          .find(source => source.environmentId === id && source.connected !== false) : null;
      const environmentProject = environmentSource ? mobileNewTaskEnvironmentMatch(environmentSource.shell.projects.filter(project => project.archivedAt == null),
        client.shell.projects.find(project => project.id === client.projectId) ?? null) : null;
      const target = movingScratch ? await mobileOpenScratch(mobileScratchTarget(client, background, id), native, client, background, current)
        : kind === 'environment' ? environmentProject ? { environmentId: id, projectId: str(environmentProject.id) } : null
        : kind === 'scratch' ? await mobileOpenScratch(id, native, client, background, current)
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
      await client.persist(storage); assertSelected();
    } else if (['workspace', 'branch', 'branch-more', 'origin'].includes(kind) && mobileNewTask('', client, background).scratch) {
      throw new ClientError('Tasks without a project run locally without a branch or worktree.');
    } else if (kind === 'workspace') {
      const draft = mobileNewTaskDraftPresentation(client, client.draftKey), expected = owner(client);
      if (draft && (value === 'local' || value === 'worktree')) {
        // Pinned setWorkspaceMode resolves local from the current ref, while a
        // new worktree retains the selected base. The desktop reducer differs.
        if (value === 'local') {
          const root = str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot);
          const refs = branchState(client).refs;
          const current = refs?.cwd === root && refs.generation === client.generation && !refs.stale
            ? refs.refs.find(ref => ref.current === true) : null;
          const path = str(current?.worktreePath);
          patchDraftContext(client, { envMode: 'local', branch: str(current?.name), worktreePath: path === root ? '' : path });
        } else patchDraftContext(client, { envMode: 'worktree', worktreePath: '' });
        mobileNewTaskDraftNoteBranch(client, 'explicit');
      } else {
        await run('cclocal:env-mode', '', value);
        if (owner(client) !== expected) throw new ClientError('The selected workspace changed.', 'superseded');
        if (value === 'previous') mobileNewTaskDraftNoteBranch(client, 'explicit');
      }
      await client.persist(storage); assertCurrent();
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
      mobileNewTaskDraftNoteBranch(client, 'explicit');
      await client.persist(storage); assertCurrent();
      state.branchLoaded = false;
    } else if (kind === 'branch-more') {
      const ends = obj(client.presentation.scrollEnds);
      client.presentation.scrollEnds = { ...ends, 'scroll:details-refs': Number(ends['scroll:details-refs'] ?? 0) + 1 };
    }
    else if ((kind === 'send' || kind === 'send-alternate')) {
      if (client.threadId) throw new ClientError('Open a new task draft before sending.');
      if (mobileNewTask('', client, background).scratch) patchDraftContext(client, { envMode: 'local', branch: '', worktreePath: '' });
      const environmentId = client.environmentId, projectId = client.projectId, generation = client.generation, draftKey = client.draftKey;
      await run(kind);
      if (!client.threadId && (draftKey !== client.draftKey || environmentId !== client.environmentId || generation !== client.generation))
        throw new ClientError('The draft or model changed before sending. Your original draft is preserved.');
      const submitted = !!client.threadId && environmentId === client.environmentId && projectId === client.projectId && generation === client.generation;
      return result('', submitted);
    } else throw new ClientError('Unknown new task action.');
    return result();
  } catch (error) {
    if (letGo(error)) throw error;
    state.error = error instanceof Error ? error.message : 'Could not change the task.'; return result(state.error);
  } finally { state.busy = false; client.revision++; }
}
