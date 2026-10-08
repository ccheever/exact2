// Pinned T3 Code 365aa87982 NewTaskDraftScreen, state/projectClones and ProjectCloneBanner.
// @ref llp/1107.005-composer-and-transcript.decision.md#new-task-ownership
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { obj, str, applyShell, initialShell, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { liveEnvironment, cloneTracking } from './shared/live-streams';
import { projectCloneDisplayName, projectCloneProgressSummary } from './shared/project-clones';
import { mobileSessionGrants } from './mobile-grants';

export interface NewTaskCloneSnapshot {
  owner: string; requestRoute: string; revision: number; environmentId: string; projectId: string;
  visible: boolean; blocked: boolean; pending: boolean; phase: string; title: string; description: string;
  error: string; cancel: boolean; retry: boolean; remove: boolean; busy: boolean;
}
export interface NewTaskCloneResult {
  revision: number; requestRoute: string; nextLocation: string; removed: boolean; message: string; alertTitle: string;
}
interface CloneFlow {
  owner: string; visit: string; location: string; ready: boolean; initialRequest: string;
  initialEnvironmentId: string; initialProjectId: string; initialCloning: boolean;
}
const flows = new WeakMap<T3Client, CloneFlow>();
const locks = new WeakMap<T3Client, Set<string>>();
function lockSet(client: T3Client) { let value = locks.get(client); if (!value) { value = new Set(); locks.set(client, value); } return value; }
function selection(client: T3Client) { return JSON.stringify([client.origin, client.environmentId, client.generation, client.projectId, client.threadId, client.threadEpoch]); }
function read(client: T3Client) {
  const flow = flows.get(client), environment = liveEnvironment(client, null, client.environmentId);
  // The shared stream keeps its last list on error. Mobile's source treats the failed
  // subscription as empty, including before the first value, and lets server admission decide.
  const tracked = !!environment && cloneTracking(environment.config), failed = !!environment?.clones.error;
  const list = tracked && !failed ? environment?.clones.value ?? null : [];
  const clone = list?.find(item => item.projectId === client.projectId) ?? null;
  const pending = !!flow?.ready && list === null && flow.initialCloning
    && flow.initialEnvironmentId === client.environmentId && flow.initialProjectId === client.projectId && !client.threadId;
  const valid = !client.threadId && !!client.projectId;
  return { flow, clone: valid ? clone : null, pending: valid && pending, blocked: valid && (pending || !!clone && clone.phase !== 'done') };
}

/** Root observes the current ready draft visit; child routes preserve its initial clone identity. */
export function mobileNewTaskCloneObserve(owner: string, visit: string, location: string, ready: boolean, client: T3Client = mobileClient): NewTaskCloneSnapshot {
  let flow = flows.get(client);
  if (!flow || flow.owner !== owner) {
    flow = { owner, visit, location, ready, initialRequest: '', initialEnvironmentId: '', initialProjectId: '', initialCloning: false };
    flows.set(client, flow);
  }
  flow.visit = visit; flow.location = location; flow.ready = ready && !!owner;
  const at = location.indexOf('?'), path = at < 0 ? location : location.slice(0, at), request = `${visit}:${location}`;
  if (path === '/new/draft' && flow.initialRequest !== request) {
    const query = new URLSearchParams(at < 0 ? '' : location.slice(at + 1));
    flow.initialRequest = request; flow.initialEnvironmentId = query.get('environmentId') ?? '';
    flow.initialProjectId = query.get('projectId') ?? ''; flow.initialCloning = query.get('cloning') === '1';
  }
  return mobileNewTaskCloneSnapshot(client);
}
export function mobileNewTaskCloneBlocks(client: T3Client = mobileClient): boolean { return read(client).blocked; }
export function mobileNewTaskCloneSnapshot(client: T3Client = mobileClient): NewTaskCloneSnapshot {
  const { flow, clone, pending, blocked } = read(client), phase = str(clone?.phase), running = phase === 'running', cancelled = phase === 'cancelled';
  const visible = !!clone && phase !== 'done', name = clone ? projectCloneDisplayName(clone) : '';
  return { owner: flow?.owner ?? '', requestRoute: flow?.visit ?? '', revision: client.revision, environmentId: client.environmentId, projectId: client.projectId,
    visible, blocked, pending, phase, title: !visible ? '' : running ? `Cloning ${name}` : cancelled ? `Cancelled cloning ${name}` : `Failed to clone ${name}`,
    description: running && clone ? projectCloneProgressSummary(clone) : '', error: !running && visible ? str(clone?.error) : '',
    cancel: visible && running, retry: visible && !running, remove: visible && !running,
    busy: lockSet(client).has(client.environmentId) };
}

/** Root owns this answer and displays its source-native error alert. No second clone stream or optimistic clone phase. */
export async function mobileNewTaskCloneAction(owner: string, visit: string, kind: string, nativeInput?: Native | null,
  client: T3Client = mobileClient): Promise<NewTaskCloneResult> {
  const flow = flows.get(client), captured = selection(client), environmentId = client.environmentId, projectId = client.projectId;
  const generation = client.generation, origin = client.origin, locked = lockSet(client);
  const title = kind === 'cancel' ? 'Failed to cancel clone' : kind === 'retry' ? 'Failed to retry clone' : 'Failed to remove project';
  const result = (message = '', removed = false, nextLocation = ''): NewTaskCloneResult => ({ revision: client.revision, requestRoute: visit,
    nextLocation, removed, message, alertTitle: message ? title : '' });
  const current = () => !!flow && flows.get(client) === flow && flow.owner === owner && flow.visit === visit && flow.ready
    && selection(client) === captured;
  const endpointCurrent = () => client.origin === origin && client.environmentId === environmentId && client.generation === generation;
  const check = () => { if (!current()) throw new ClientError('The new task draft changed.', 'superseded'); };
  if (!owner || !current()) return result();
  if (locked.has(environmentId)) return result();
  if (!['cancel', 'retry', 'remove'].includes(kind)) return result('This clone action is unavailable.');
  locked.add(environmentId); client.revision++; let sent = false, removed = false;
  try {
    if (!nativeInput?.available || !client.ready) throw new ClientError('This environment is not connected.');
    const base = letGoAware(mobileNative(nativeInput));
    const native: Native = { available: base.available, watch: topic => { check(); base.watch(topic); }, later: async input => {
      check(); const reply = await base.later(input);
      if (!endpointCurrent()) throw new ClientError('The connection changed. Reopen this draft.', 'stale');
      // A successful deletion belongs to its captured project even if the picker changed
      // during the RPC. Only navigation depends on the currently selected project.
      if (!sent) check(); return reply;
    } };
    const session = await client.http(native, '/api/auth/session', generation);
    const permission = kind === 'remove' ? 'orchestration:operate' : 'source-control:write';
    if (!mobileSessionGrants(session, permission)) throw new ClientError(kind === 'remove'
      ? 'This connection cannot remove projects.' : 'This connection cannot change project clones.');
    check();
    const state = mobileNewTaskCloneSnapshot(client);
    if (kind === 'cancel' ? !state.cancel : kind === 'retry' ? !state.retry : !state.remove) throw new ClientError('The project clone changed.');
    if (kind === 'remove') {
      const [commandId] = await client.ids(native, 1); check(); sent = true;
      await client.request(native, 'projects.mutate', { type: 'project.delete', commandId: commandId!, projectId }, generation, true); removed = true;
      if (!current()) return result('', true);
      try { client.shell = applyShell(initialShell(), await client.http(native, '/api/orchestration/shell', generation)); client.shellLoaded = true; }
      catch (error) { if (letGo(error)) throw error; /* The existing shell stream also adopts the acknowledged deletion. */ }
      if (current()) return result('', true, '/');
    } else {
      sent = true; await client.request(native, kind === 'cancel' ? 'projectClone.cancel' : 'projectClone.retry', { projectId }, generation, true);
    }
    return result('', removed);
  } catch (error) {
    if (letGo(error)) throw error;
    return current() ? result(error instanceof Error ? error.message : 'An error occurred.', removed) : result('', removed);
  } finally { locked.delete(environmentId); client.revision++; }
}
