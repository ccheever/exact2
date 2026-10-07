// Mobile365aa87982 NewTaskFlowProvider, NewTaskDraftRouteScreen and DraftScreen.
// @ref llp/1107.005-composer-and-transcript.decision.md#new-task-ownership
import { mobileClient, mobileNative } from './client';
import { mobileHomeSources } from './home';
import { mobileNewTask, mobileNewTaskAction } from './new-task';
import { mobileSessionGrants } from './environment-detail';
import { mobileComposerSettings, mobileComposerSettingsAction } from './composer-settings';
import { mobileComposerTarget } from './composer-target';
import { patchDraftContext } from './shared/composer-controls-branch';
import { ClientError, nativeFiles, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { obj, str } from './shared/domain';
import { fleet, type EnvironmentFleet } from './shared/settings-b-fleet';
import type { T3Client } from './shared/client';

export interface NewTaskFlowSnapshot {
  owner: string; requestRoute: string; status: string; title: string; message: string;
  draftOwner: string; ready: boolean; chooser: boolean; needsPrepare: boolean; busy: boolean; nextLocation: string;
}
export interface NewTaskFlowResult {
  revision: number; requestRoute: string; nextLocation: string; message: string;
  submitted: boolean; environmentId: string; projectId: string; threadId: string;
}
interface Selection { environmentId: string; projectId: string; draftKey: string; generation: number; threadEpoch: number }
interface Flow {
  session: string; owner: string; visit: string; location: string; active: boolean; serial: number;
  busy: boolean; selected: Selection | null; applied: Set<string>; requests: Map<string, string>; error: string; readyVisit: string;
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
    ?? (/^\/new\/draft\/attachments\/[^/]+$/.test(path) ? 'attachment'
      : /^\/new\/draft\/files\/.+$/.test(path) ? 'file'
        : /^\/new\/draft\/settings\/(?:runtime|providers|options\/[^/]+)$/.test(path) ? 'settings-child' : '');
  const unsupported = ['pendingTaskId', 'draftId', 'incomingShareId', 'cloning'].find(key => !!query.get(key)) ?? '';
  return { chooser, context, environmentId: query.get('environmentId') ?? '', projectId: query.get('projectId') ?? '',
    branch: query.get('branch') ?? '', worktreePath: query.get('worktreePath') ?? '', unsupported };
}

/** Observe the containing /new visit even while a child is selected. No shared
 * draft/selection mutation or native handle survives this synchronous snapshot.
 * An inactive observation invalidates every outstanding action for that session. */
export function mobileNewTaskFlowView(session: string, visit: string, location: string, active: boolean, catalogReady: boolean,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet): NewTaskFlowSnapshot {
  let flow = flows.get(client);
  if (!flow || flow.session !== session || active && !flow.active) {
    flow = { session, owner: JSON.stringify([session, ++sequence]), visit, location, active, serial: 0,
      busy: false, selected: null, applied: new Set(), requests: new Map(), error: '', readyVisit: '' }; flows.set(client, flow);
  }
  if (flow.requests.has(visit) && flow.requests.get(visit) !== location) flow.applied.delete(visit);
  flow.requests.set(visit, location);
  if (flow.visit !== visit || flow.location !== location || flow.active !== active) {
    flow.serial++; flow.error = ''; flow.visit = visit; flow.location = location; flow.active = active;
  }
  const route = mobileNewTaskRoute(location), explicit = !!route.environmentId && !!route.projectId;
  flow.readyVisit = '';
  const selected = sameSelection(flow.selected, client) && projectExists(client.environmentId, client.projectId, client, background);
  const base: NewTaskFlowSnapshot = { owner: flow.owner, requestRoute: visit, status: 'inactive', title: 'New task', message: flow.error,
    draftOwner: active && client.preferencesLoaded && selected ? mobileComposerTarget(client).owner : '',
    ready: false, chooser: route.chooser, needsPrepare: false, busy: flow.busy || checkouts.has(client), nextLocation: '' };
  if (!active) return base;
  if (route.chooser) return { ...base, status: 'choose', title: 'Choose project' };
  if (!client.preferencesLoaded) return { ...base, status: 'loading' };
  if (!route.context) return { ...base, status: 'pick', nextLocation: '/new' };
  if (route.unsupported) return { ...base, status: 'pick', nextLocation: '/new', message: 'This saved task or shared-content link is unavailable in this build.' };
  if (base.busy) return { ...base, status: 'preparing', title: route.branch ? 'Switching branch...' : 'New task' };
  if (flow.error) return { ...base, status: 'error' };
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
  return { ...base, status: 'ready', ready: true, title: ({ draft: 'New task', environment: 'Environment', branch: 'Branch', settings: 'Model' })[route.context] ?? 'New task' };
}

export function mobileNewTaskFlowCurrent(owner: string, visit: string, location: string, client: T3Client = mobileClient) {
  const flow = flows.get(client);
  return !!flow && flow.active && flow.owner === owner && flow.visit === visit && flow.location === location;
}

/** Root must apply ready/owner to draft, attachment, model and voice actions as
 * well as rendering. A route change alone never adopts the client's old draft. */
export function mobileNewTaskFlowOwns(owner: string, visit: string, client: T3Client = mobileClient) {
  const flow = flows.get(client);
  return !!flow && flow.active && flow.owner === owner && flow.visit === visit && flow.readyVisit === visit && !flow.busy && !checkouts.has(client) && sameSelection(flow.selected, client);
}

export async function mobileNewTaskFlowAction(owner: string, visit: string, kind: string, id: string, value: string,
  nativeInput: Native | null | undefined, suppliedStorage: Files, client: T3Client = mobileClient, background: EnvironmentFleet = fleet): Promise<NewTaskFlowResult> {
  const flow = flows.get(client), result = (message = '', nextLocation = '', submitted = false): NewTaskFlowResult => ({
    revision: client.revision, requestRoute: visit, nextLocation, message, submitted,
    environmentId: client.environmentId, projectId: client.projectId, threadId: client.threadId });
  if (!flow || !flow.active || flow.owner !== owner || flow.visit !== visit) return result('The new task route changed.');
  if (flow.busy || checkouts.has(client)) return result('Wait for the current task change to finish.');
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to create a task.');
  if (!client.preferencesLoaded) return result('Wait for saved drafts to load.');
  if (kind !== 'project' && kind !== 'scratch' && kind !== 'prepare' && !mobileNewTaskFlowOwns(owner, visit, client)) return result('Wait for the current draft to be ready.');
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
      if (!flow.applied.has(visit) && route.environmentId && route.projectId) {
        if (!projectExists(route.environmentId, route.projectId, client, background)) return result('', '/new');
        const chosen = await mobileNewTaskAction('project', JSON.stringify([route.environmentId, route.projectId]), '', native, storage, client, background, current);
        assertCurrent(); if (chosen.message) throw new ClientError(chosen.message);
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
    const response = await mobileNewTaskAction(kind, id, value, native, storage, client, background, current);
    assertCurrent(); if (response.message) throw new ClientError(response.message);
    if (response.submitted) { flow.selected = null; return result('', '', true); }
    flow.selected = selection(client); flow.applied.add(visit);
    return result('', kind === 'project' || kind === 'scratch' ? '/new/draft' : '');
  } catch (error) {
    if (letGo(error)) throw error;
    if (current()) flow.error = error instanceof Error ? error.message : 'Could not open this task.';
    return result(current() ? flow.error : '');
  } finally { flow.busy = false; client.revision++; }
}
