// Mobile365aa87982 NewTaskFlowProvider, NewTaskDraftRouteScreen and DraftScreen.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import { mobileNewTaskCloneObserve, mobileNewTaskCloneAction } from './new-task-clone';
import { mobileClient, mobileNative } from './client';
import { mobileHomeSources } from './home';
import { mobileNewTask, mobileNewTaskAction } from './new-task';
import { mobileSessionGrants } from './environment-detail';
import { mobileComposerSettings, mobileComposerSettingsAction } from './composer-settings';
import { mobileBindNewTaskDraft } from './new-task-draft-binding';
import { mobileNewTaskDraftLookup, mobileNewTaskDraftCurrent, mobileNewTaskDraftUnbind, mobileNewTaskDraftRetarget } from './mobile-new-task-drafts';
import { mobileComposerTarget } from './composer-target';
import { patchDraftContext } from './shared/composer-controls-branch';
import { ClientError, nativeFiles, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { obj, str } from './shared/domain';
import { fleet, type EnvironmentFleet } from './shared/settings-b-fleet';
import type { T3Client } from './shared/client';

export interface NewTaskFlowSnapshot {
  owner: string; requestRoute: string; status: string; title: string; message: string;
  draftOwner: string; ready: boolean; fileReady: boolean; chooser: boolean; needsPrepare: boolean; busy: boolean; nextLocation: string;
}
export interface NewTaskFlowResult {
  revision: number; requestRoute: string; nextLocation: string; message: string; alertTitle: string;
  submitted: boolean; environmentId: string; projectId: string; threadId: string;
}
interface Selection { environmentId: string; projectId: string; draftKey: string; generation: number; threadEpoch: number }
interface Flow {
  session: string; owner: string; visit: string; location: string; active: boolean; serial: number;
  draftKey: string; busy: boolean; selected: Selection | null; applied: Set<string>; requests: Map<string, string>; error: string; readyVisit: string;
}
const flows = new WeakMap<T3Client, Flow>();
// A checkout already dispatched cannot be cancelled by ignoring its result.
// Block another preparation until its actual answer settles, without retaining
// that answer's native handle or awaiting its promise from a later answer.
const checkouts = new WeakSet<T3Client>();
let sequence = 0;
const selection = (client: T3Client): Selection => ({ environmentId: client.environmentId, projectId: client.projectId,
  draftKey: client.draftKey, generation: client.generation, threadEpoch: client.threadEpoch });
const sameSelection = (value: Selection | null, client: T3Client) => !!value && !client.threadId
  && value.environmentId === client.environmentId && value.projectId === client.projectId
  && value.draftKey === client.draftKey && value.generation === client.generation && value.threadEpoch === client.threadEpoch;
const projectExists = (environmentId: string, projectId: string, client: T3Client, background: EnvironmentFleet) =>
  mobileHomeSources(client, background).some(source => source.environmentId === environmentId
    && source.shell.projects.some(project => project.id === projectId && project.archivedAt == null));

/** Query identity is explicit. Titles never substitute for project IDs. */
export function mobileNewTaskRoute(location: string) {
  const queryAt = location.indexOf('?'), path = queryAt < 0 ? location : location.slice(0, queryAt);
  const query = new URLSearchParams(queryAt < 0 ? '' : location.slice(queryAt + 1));
  const chooser = path === '/new' || path === '/new/projects';
  const context = ({ '/new/draft': 'draft', '/new/draft/environment': 'environment', '/new/draft/branch': 'branch',
    '/new/draft/settings': 'settings' } as Record<string, string>)[path]
    ?? (/^\/new\/add-project(?:\/(?:repository|destination|local|new))?$/.test(path) ? 'add-project'
      : /^\/new\/draft\/attachments\/[^/]+$/.test(path) ? 'attachment'
      : /^\/new\/draft\/files\/.+$/.test(path) ? 'file'
        : /^\/new\/draft\/settings\/(?:runtime|providers|options\/[^/]+)$/.test(path) ? 'settings-child' : '');
  const unsupported = ['pendingTaskId', 'incomingShareId'].find(key => !!query.get(key))
    ?? (query.get('cloning') && query.get('cloning') !== '1' ? 'cloning' : '');
  const param = (key: string) => { const value = query.get(key); return value?.trim() ? value : ''; };
  const environmentId = param('environmentId'), projectId = param('projectId'), cwd = param('cwd');
  const standaloneFile = context === 'file' && !!environmentId && !!cwd && !projectId && !param('draftId') && !unsupported;
  return { chooser, context, environmentId, projectId, cwd, standaloneFile, draftId: param('draftId'),
    branch: query.get('branch') ?? '', worktreePath: query.get('worktreePath') ?? '', unsupported };
}

/** Observe the containing /new visit even while a child is selected. No shared
 * draft/selection mutation or native handle survives this synchronous snapshot.
 * An inactive observation invalidates every outstanding action for that session. */
export function mobileNewTaskFlowView(session: string, visit: string, location: string, active: boolean, catalogReady: boolean,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet): NewTaskFlowSnapshot {
  let flow = flows.get(client);
  if (!flow || flow.session !== session || active && !flow.active) {
    if (flow) mobileNewTaskDraftUnbind(client, flow.owner);
    flow = { session, owner: JSON.stringify([session, ++sequence]), visit, location, active, serial: 0,
      draftKey: '', busy: false, selected: null, applied: new Set(), requests: new Map(), error: '', readyVisit: '' }; flows.set(client, flow);
  }
  if (flow.requests.has(visit) && flow.requests.get(visit) !== location) flow.applied.delete(visit);
  flow.requests.set(visit, location);
  if (flow.visit !== visit || flow.location !== location || flow.active !== active) {
    flow.serial++; flow.error = ''; flow.visit = visit; flow.location = location; flow.active = active;
  }
  const route = mobileNewTaskRoute(location), explicit = !!route.environmentId && !!route.projectId;
  flow.readyVisit = '';
  const selected = !!flow.draftKey && mobileNewTaskDraftCurrent(client)?.key === flow.draftKey && sameSelection(flow.selected, client) && projectExists(client.environmentId, client.projectId, client, background);
  const base: NewTaskFlowSnapshot = { owner: flow.owner, requestRoute: visit, status: 'inactive', title: 'New task', message: flow.error,
    draftOwner: active && client.preferencesLoaded && selected ? mobileComposerTarget(client).owner : '',
    ready: false, fileReady: false, chooser: route.chooser, needsPrepare: false, busy: flow.busy || checkouts.has(client), nextLocation: '' };
  if (!active) { mobileNewTaskDraftUnbind(client, flow.owner); return base; }
  if (route.context === 'add-project') return { ...base, status: 'add-project', title: 'Add project' };
  if (route.chooser) return { ...base, status: 'choose', title: 'Choose project' };
  if (route.standaloneFile) return { ...base, status: 'file', title: 'Files', draftOwner: '', fileReady: true, busy: false };
  if (!client.preferencesLoaded) return { ...base, status: 'loading' };
  if (!route.context) return { ...base, status: 'pick', nextLocation: '/new' };
  if (route.unsupported) return { ...base, status: 'pick', nextLocation: '/new', message: 'This saved task or shared-content link is unavailable in this build.' };
  if (base.busy) return { ...base, status: 'preparing', title: route.branch ? 'Switching branch...' : 'New task' };
  if (flow.error) return { ...base, status: 'error' };
  if (!flow.applied.has(visit) && route.draftId) {
    if (!catalogReady) return { ...base, status: 'loading' };
    const saved = mobileNewTaskDraftLookup(client, route.draftId);
    if (!saved || !projectExists(saved.environmentId, saved.projectId, client, background)) return { ...base, status: 'pick', nextLocation: '/new' };
    return { ...base, status: 'prepare', needsPrepare: true };
  }
  if (!flow.applied.has(visit) && explicit) {
    if (!catalogReady) return { ...base, status: 'loading' };
    if (!projectExists(route.environmentId, route.projectId, client, background)) return { ...base, status: 'pick', nextLocation: '/new' };
    return { ...base, status: 'prepare', needsPrepare: true };
  }
  if (!selected) return { ...base, status: catalogReady ? 'pick' : 'loading', nextLocation: catalogReady ? '/new' : '' };
  if (route.context === 'branch' && mobileNewTask('', client, background).scratch) return { ...base, status: 'pick', nextLocation: '/new/draft' };
  // A settings URL needs the same staged session that an explicit button creates.
  if ((route.context === 'settings' && !flow.applied.has(visit)) || route.context === 'settings-child' && !mobileComposerSettings('', '', false, client).open) return { ...base, status: 'prepare', needsPrepare: true };
  flow.readyVisit = visit;
  return { ...base, status: 'ready', ready: true, fileReady: route.context === 'file', title: ({ draft: 'New task', environment: 'Environment', branch: 'Branch', settings: 'Model' })[route.context] ?? 'New task' };
}

export function mobileNewTaskFlowCurrent(owner: string, visit: string, location: string, client: T3Client = mobileClient) {
  const flow = flows.get(client);
  return !!flow && flow.active && flow.owner === owner && flow.visit === visit && flow.location === location;
}

/** File access is route-owned; explicit standalone workspaces grant no draft authority. */
export function mobileNewTaskFileRouteCurrent(owner: string, visit: string, location: string, client: T3Client = mobileClient) {
  if (!mobileNewTaskFlowCurrent(owner, visit, location, client)) return false;
  const route = mobileNewTaskRoute(location);
  return route.context === 'file' && !route.unsupported && (route.standaloneFile || mobileNewTaskFlowOwns(owner, visit, client));
}

/** Root must apply ready/owner to draft, attachment, model and voice actions as
 * well as rendering. A route change alone never adopts the client's old draft. */
export function mobileNewTaskFlowOwns(owner: string, visit: string, client: T3Client = mobileClient) {
  const flow = flows.get(client);
  return !!flow && flow.active && flow.owner === owner && flow.visit === visit && flow.readyVisit === visit && !flow.busy && !checkouts.has(client) && sameSelection(flow.selected, client) && mobileNewTaskDraftCurrent(client)?.key === flow.draftKey;
}

export async function mobileNewTaskFlowAction(owner: string, visit: string, kind: string, id: string, value: string,
  nativeInput: Native | null | undefined, suppliedStorage: Files, client: T3Client = mobileClient, background: EnvironmentFleet = fleet): Promise<NewTaskFlowResult> {
  const flow = flows.get(client), result = (message = '', nextLocation = '', submitted = false): NewTaskFlowResult => ({
    revision: client.revision, requestRoute: visit, nextLocation, message, alertTitle: '', submitted,
    environmentId: client.environmentId, projectId: client.projectId, threadId: client.threadId });
  if (!flow || !flow.active || flow.owner !== owner || flow.visit !== visit) return result('The new task route changed.');
  if (mobileNewTaskRoute(flow.location).standaloneFile) return result('Return to the new task before changing its draft.');
  if (mobileNewTaskRoute(flow.location).context === 'add-project') return result('Return to the new task before changing its draft.');
  if (flow.busy || checkouts.has(client)) return result('Wait for the current task change to finish.');
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to create a task.');
  if (!client.preferencesLoaded) return result('Wait for saved drafts to load.');
  if (kind !== 'project' && kind !== 'scratch' && kind !== 'prepare' && !mobileNewTaskFlowOwns(owner, visit, client)) return result('Wait for the current draft to be ready.');
  if (kind.startsWith('clone-')) {
    mobileNewTaskCloneObserve(owner, visit, flow.location, true, client);
    if (id !== client.projectId || value !== client.environmentId) return result('The project clone changed.');
    const generation = client.generation, environmentId = client.environmentId, origin = client.origin, location = flow.location;
    // Keep the draft route while its acknowledged deletion reaches the shell stream.
    // That shared refresh may select a fallback before this answer returns.
    flow.busy = true;
    try {
      const changed = await mobileNewTaskCloneAction(owner, visit, kind.slice(6), nativeInput, client);
      const stillHere = flows.get(client) === flow && flow.active && flow.visit === visit && flow.location === location
        && client.generation === generation && client.environmentId === environmentId && client.origin === origin;
      const removedHere = changed.removed && stillHere && !client.threadId
        && !client.shell.projects.some(project => project.id === id);
      return { ...result(changed.message, !stillHere ? '' : removedHere ? '/' : changed.nextLocation),
        revision: changed.revision, alertTitle: changed.alertTitle };
    } finally { flow.busy = false; client.revision++; }
  }
  if (kind === 'send' || kind === 'send-alternate') {
    const clone = mobileNewTaskCloneObserve(owner, visit, flow.location, true, client);
    if (clone.blocked) return result(clone.pending || clone.phase === 'running' ? 'Cloning repository' : 'Repository not cloned');
  }
  if (kind === 'draft') {
    const target = flow.selected, current = () => flows.get(client) === flow && flow.active && flow.visit === visit && sameSelection(target, client);
    if (!current()) return result('Choose a project before changing this draft.');
    const edited = await mobileNewTaskAction(kind, id, value, nativeInput, suppliedStorage, client, background, current);
    return { ...result(edited.message), revision: edited.revision };
  }
  const serial = ++flow.serial, current = () => flows.get(client) === flow && flow.active && flow.visit === visit && flow.serial === serial;
  const assertCurrent = () => { if (!current()) throw new ClientError('The new task route changed.', 'superseded'); };
  const base = letGoAware(mobileNative(nativeInput));
  const native: Native = { available: base.available, watch: topic => base.watch(topic), later: async input => {
    assertCurrent(); const answer = await base.later(input); assertCurrent(); return answer;
  } };
  const storage = client === mobileClient ? nativeFiles(native) : suppliedStorage;
  const route = mobileNewTaskRoute(flow.location);
  if (kind === 'scratch' && (!route.chooser || route.unsupported)) return result('Choose a project before starting this task.');
  flow.busy = true; flow.error = '';
  try {
    // Root's existing snapshot owns preference hydration. Refuse before that
    // completes rather than creating a competing read of the shared draft store.
    assertCurrent();
    if (kind === 'prepare') {
      if (!route.context || route.unsupported) return result('', '/new');
      const saved = route.draftId ? mobileNewTaskDraftLookup(client, route.draftId) : null;
      const requestedEnvironment = saved?.environmentId || route.environmentId, requestedProject = saved?.projectId || route.projectId;
      if (route.draftId && !saved) return result('', '/new');
      if (!flow.applied.has(visit) && requestedEnvironment && requestedProject) {
        if (!projectExists(requestedEnvironment, requestedProject, client, background)) return result('', '/new');
        if (saved && saved.key !== flow.draftKey) mobileNewTaskDraftUnbind(client, flow.owner);
        else if (flow.draftKey) {
          const held = mobileNewTaskDraftLookup(client, flow.draftKey);
          if (!held) return result('', '/new');
          mobileNewTaskDraftRetarget(client, flow.draftKey, held);
        }
        const chosen = await mobileNewTaskAction('project', JSON.stringify([requestedEnvironment, requestedProject]), '', native, storage, client, background, current);
        assertCurrent(); if (chosen.message) throw new ClientError(chosen.message);
        flow.draftKey = await mobileBindNewTaskDraft(client, flow.owner, flow.draftKey, route.draftId, native, storage, current);
        assertCurrent();
        if (route.branch && !mobileNewTask('', client, background).scratch) {
          const target = selection(client), expected = () => current() && sameSelection(target, client);
          const scoped: Native = { available: native.available, watch: topic => native.watch(topic), later: async input => {
            if (!expected()) throw new ClientError('The selected workspace changed.', 'superseded');
            const reply = await native.later(input);
            if (!expected()) throw new ClientError('The selected workspace changed.', 'superseded'); return reply;
          } };
          let branch = route.branch;
          if (!route.worktreePath) {
            const session = await client.http(scoped, '/api/auth/session');
            if (!mobileSessionGrants(session, 'source-control:write')) throw new ClientError('This connection cannot check out that branch.');
            const cwd = str(client.shell.projects.find(project => project.id === target.projectId)?.workspaceRoot);
            checkouts.add(client);
            let switched;
            try { switched = await client.restAccess(scoped).request('vcs.switchRef', { cwd, refName: branch }); }
            finally { checkouts.delete(client); }
            branch = str(obj(switched).refName) || branch;
          }
          if (!expected()) throw new ClientError('The selected workspace changed.', 'superseded');
          patchDraftContext(client, { envMode: 'local', branch, worktreePath: route.worktreePath });
          await client.persist(storage); assertCurrent();
        }
        flow.selected = selection(client);
      }
      if (!sameSelection(flow.selected, client)) return result('', '/new');
      if (route.context === 'settings' || route.context === 'settings-child' && !mobileComposerSettings('', '', false, client).open) {
        const opened = await mobileComposerSettingsAction('open', '', '', native, storage, client);
        assertCurrent(); if (opened.message) throw new ClientError(opened.message);
      }
      flow.applied.add(visit);
      return result();
    }
    if (kind !== 'project' && kind !== 'scratch' && !sameSelection(flow.selected, client)) throw new ClientError('Choose a project before changing this draft.');
    if (flow.draftKey && ['project', 'scratch', 'environment'].includes(kind)) {
      const held = mobileNewTaskDraftLookup(client, flow.draftKey);
      if (!held) throw new ClientError('That saved draft is no longer available.');
      // Validate the existing stamp without changing it, before reducing selection
      // or making a remote scratch request for a captured pending draft.
      mobileNewTaskDraftRetarget(client, flow.draftKey, held);
    }
    const response = await mobileNewTaskAction(kind, id, value, native, storage, client, background, current);
    assertCurrent(); if (response.message) throw new ClientError(response.message);
    if (response.submitted) { flow.selected = null; mobileNewTaskDraftUnbind(client, flow.owner); flow.draftKey = ''; return result('', '', true); }
    if (kind === 'project' || kind === 'scratch' || kind === 'environment') {
      flow.draftKey = await mobileBindNewTaskDraft(client, flow.owner, flow.draftKey, '', native, storage, current);
      assertCurrent();
    }
    flow.selected = selection(client); flow.applied.add(visit);
    return result('', kind === 'project' || kind === 'scratch' ? '/new/draft' : '');
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) flow.error = error instanceof Error ? error.message : 'Could not open this task.';
    return result(current() ? flow.error : '');
  } finally { flow.busy = false; client.revision++; }
}
