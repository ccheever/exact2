// Workspace SVG shares the root's live route, file authority and existing transports.
// @ref llp/1109.006-review-and-files.decision.md#workspace-svg-preview-gap
import { mobileClient, mobileNative, mobileFocusedConnectionFacts } from './client';
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { fleet, type EnvironmentFleet } from './shared/settings-b-fleet';
import { environmentSources } from './shared/connections';
import { workspaceOf } from './shared/r4-surfaces-panel';
import { reviewOwner } from './review-owner';
import { mobileSessionGrants } from './mobile-grants';
import { mobileMediaURL } from './media-preview';
import { mediaFileReference } from './shared/media-reference';
import { draftContext } from './shared/composer-controls-branch';
import { mobileNewTaskFileRouteCurrent } from './new-task-flow';
import { mobileWorkspaceEntries } from './mobile-workspace';
import { newTaskFileEndpointIdentity, withStandaloneNewTaskFile } from './new-task-file-source';
import { mobileFilePresentation, type FileSnapshot } from './file-data';

export type SvgVisit = { path: string; location: string; visit: string; flowOwner: string };
export type SvgAdmission = { owner: string; visit: string; location: string };
interface Scope extends SvgVisit { owner: string; environment: string; cwd: string; projectName: string; resource: Obj; draft: boolean }
interface State { scope: Scope; epoch: number; load: number; retry: number; share: number; report: number; prepared: boolean; loading: boolean;
  error: string; url: string; revision: number; authorized: boolean; sharing: boolean;
  attempt?: { identifier: string; dispatched: boolean }; shareMessage: string; shareReceipt: string }
interface Owner { live: Omit<SvgVisit, 'flowOwner'> | null; state?: State }
// The app holds identifiers only. Native sharing outlives any individual Exact answer.
const owners = new WeakMap<T3Client, Owner>();
const ownerOf = (client: T3Client) => { let value = owners.get(client); if (!value) { value = { live: null }; owners.set(client, value); } return value; };
export const workspaceSvgPath = (path: string) => /\.svg$/i.test(path.split(/[?#]/, 1)[0] ?? '');
const absolute = (path: string) => path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path) || path.startsWith('\\\\');
const sameVisit = (left: Omit<SvgVisit, 'flowOwner'> | null, right: SvgVisit | SvgAdmission) => !!left && left.visit === right.visit && left.location === right.location && (!('path' in right) || left.path === right.path);
function revoke(state: State) {
  state.epoch++; state.load++; state.retry++; state.share++;
  state.url = ''; state.authorized = false; state.loading = false;
  if (!state.attempt?.dispatched) { state.attempt = undefined; state.sharing = false; }
}
/** The always-consumed workspace source invokes this before projecting layout. */
export function workspaceSvgObserve(entries: unknown, client: T3Client = mobileClient) {
  const top = mobileWorkspaceEntries(entries).at(-1), owner = ownerOf(client);
  owner.live = top && (top.name === 'threadFile' || top.name === 'newTaskFile')
    ? { path: top.params.filePath ?? '', location: top.url, visit: top.id } : null;
  if (owner.state && !sameVisit(owner.live, owner.state.scope)) { revoke(owner.state); owner.state = undefined; }
}
function scopeOf(visit: SvgVisit, client: T3Client, background: EnvironmentFleet): Scope | null {
  const draft = visit.location.startsWith('/new/draft/files/');
  if (!sameVisit(ownerOf(client).live, visit) || !workspaceSvgPath(visit.path)
    || draft && !mobileNewTaskFileRouteCurrent(visit.flowOwner, visit.visit, visit.location, client)) return null;
  const query = new URLSearchParams(visit.location.split('?').slice(1).join('?'));
  const param = (key: string) => { const value = query.get(key); return value?.trim() ? value : ''; };
  const workspace = workspaceOf(client), environment = draft ? param('environmentId') || client.environmentId : client.environmentId;
  const cwd = draft ? param('cwd') || draftContext(client).worktreePath || workspace.cwd : workspace.cwd;
  const project = client.shell.projects.find(value => value.id === client.projectId);
  const projectName = draft ? param('projectName') || str(project?.title, 'Files') : workspace.projectName;
  const resolved = absolute(visit.path) ? visit.path : /^[A-Za-z]:[\\/]/.test(cwd) || cwd.startsWith('\\\\')
    ? `${cwd.replace(/[\\/]+$/, '')}\\${visit.path.replaceAll('/', '\\')}` : `${cwd.replace(/[\\/]+$/, '')}/${visit.path.replace(/^\/+/, '')}`;
  const resource = draft ? { _tag: 'draft-workspace-file', cwd, path: visit.path }
    : { _tag: absolute(visit.path) ? 'media-file' : 'workspace-file', threadId: client.threadId, path: resolved };
  const owner = JSON.stringify([visit.path, visit.location, visit.visit, visit.flowOwner,
    draft ? newTaskFileEndpointIdentity(environment, client, background) : reviewOwner(client), cwd]);
  return { ...visit, owner, environment, cwd, projectName, resource, draft };
}
function sourceOf(scope: Scope, client: T3Client, background: EnvironmentFleet) {
  return environmentSources(client, background.saved, background.entries, mobileFocusedConnectionFacts(client)).find(source => source.environmentId === scope.environment);
}
function subject(scope: Scope, client: T3Client, background: EnvironmentFleet) {
  return scope.environment === client.environmentId ? client : [...background.entries.values()].find(entry => entry.environmentId === scope.environment);
}
export function workspaceSvgConnection(phase: string, label: string, error = '', trace = '') {
  const retrying = phase === 'connecting' || phase === 'reconnecting';
  const title = phase === 'offline' ? 'You are offline' : phase === 'connecting' ? `Connecting to ${label}...`
    : phase === 'reconnecting' ? `Reconnecting to ${label}...` : phase === 'unsupported' ? 'Client not supported'
    : phase === 'error' ? `${label} is unavailable` : phase === 'connected' ? '' : `${label} is disconnected`;
  const detail = error ? (phase === 'reconnecting' ? `The app will keep retrying automatically. ${error}` : error)
    : phase === 'offline' ? 'Cached data remains available. The preview will load when your connection returns.'
    : retrying ? 'The preview will load as soon as the environment is ready.' : phase === 'unsupported'
      ? 'Use compatible versions of the app and server to connect.' : phase === 'connected' ? '' : 'Reconnect the environment to load the preview.';
  return { title, detail, trace, retrying, retry: phase !== 'offline' && phase !== 'unsupported', symbol: phase === 'offline' ? 'wifi.slash' : 'bolt.horizontal.circle' };
}
function phaseOf(scope: Scope, client: T3Client, background: EnvironmentFleet) {
  const source = sourceOf(scope, client, background);
  const phase = scope.environment === client.environmentId && client.connection === 'offline' ? 'offline' : source?.phase ?? 'available';
  return { phase, ...workspaceSvgConnection(phase, source?.label || 'Environment', source?.error || '', source?.traceId || '') };
}
function current(state: State, client: T3Client, background: EnvironmentFleet) {
  return ownerOf(client).state === state && scopeOf(state.scope, client, background)?.owner === state.scope.owner;
}
const superseded = () => new ClientError('The file route changed.', 'superseded');
function ownsInvocation(state: State, client: T3Client, background: EnvironmentFleet, epoch: number, lane: 'load' | 'share', id: number) {
  return current(state, client, background) && state.epoch === epoch && state[lane] === id;
}
function fence(state: State, client: T3Client, background: EnvironmentFleet, epoch: number, lane: 'load' | 'share', id: number) {
  if (!ownsInvocation(state, client, background, epoch, lane, id)) throw superseded();
  if (state.authorized && !subject(state.scope, client, background)?.scopes.includes('filesystem:read')) {
    revoke(state); state.prepared = true; state.error = 'This connection cannot read host files.';
    throw new ClientError(state.error);
  }
}
export function workspaceSvgSnapshot(visit: SvgVisit, dark = false, client: T3Client = mobileClient, background: EnvironmentFleet = fleet): FileSnapshot | null {
  const scope = scopeOf(visit, client, background), owner = ownerOf(client);
  if (!scope) { if (workspaceSvgPath(visit.path)) throw superseded(); return null; }
  if (owner.state?.scope.owner !== scope.owner) {
    if (owner.state) revoke(owner.state);
    owner.state = { scope, epoch: 0, load: 0, retry: 0, share: 0, report: 0, prepared: false, loading: false, error: '', url: '', revision: 0, authorized: false, sharing: false, shareMessage: '', shareReceipt: '' };
  }
  const state = owner.state;
  if (state.authorized && !subject(scope, client, background)?.scopes.includes('filesystem:read')) {
    revoke(state); state.prepared = true; state.error = 'This connection cannot read host files.';
  }
  const connection = phaseOf(scope, client, background), reference = mediaFileReference(str(scope.resource.path), scope.cwd);
  const failure = ['offline', 'reconnecting', 'error', 'unsupported'].includes(connection.phase) || connection.phase === 'available' && state.prepared
    ? 'disconnected' : connection.phase === 'connecting' ? '' : state.error ? 'failed' : '';
  const url = state.url && state.url + (state.revision ? `${state.url.includes('?') ? '&' : '?'}revision=${state.revision}` : '');
  const source = url && !failure ? JSON.stringify({ identifier: JSON.stringify([scope.owner, state.revision]), name: visit.path.split(/[\\/]/).at(-1) || 'File',
    kind: 'file', source: 'remote', id: scope.owner, url, workspaceSvg: true }) : '';
  return { ...mobileFilePresentation({ owner: scope.owner, revision: client.revision, projectName: scope.projectName,
    loading: state.loading || !state.prepared, error: state.error }, visit.path, dark), svg: true, svgSource: source,
    svgRetry: JSON.stringify({ identifier: scope.owner, operation: 'retry' }), svgSharing: state.sharing, svgCanRead: state.authorized,
    svgShareAction: JSON.stringify({ identifier: scope.owner, operation: 'share' }), svgShareMessage: state.shareMessage, svgShareReceipt: state.shareReceipt,
    svgFailure: failure, svgPhase: connection.phase, svgTitle: connection.title, svgDetail: connection.detail, svgTrace: connection.trace,
    svgTraceCopy: JSON.stringify({ identifier: scope.owner, operation: 'copy-trace', trace: connection.trace }),
    svgRetrying: connection.retrying, svgRetryAllowed: connection.retry, svgSymbol: connection.symbol,
    fullPath: reference.path, relativePath: reference.relativePath ?? '' };
}
function applyShareStatus(state: State, status: Obj) {
  const attempt = state.attempt;
  if (!attempt?.dispatched || !['idle', 'preparing', 'presented', 'cancelling', 'terminal'].includes(str(status.phase))) return;
  const matching = status.identifier === attempt.identifier;
  if (matching && status.phase !== 'terminal') { state.sharing = true; return; }
  state.sharing = false; state.attempt = undefined;
  if (matching) { state.shareMessage = state.authorized ? str(status.message) : ''; state.shareReceipt = attempt.identifier; }
}
/** A fresh resource answer watches the native attempt, including a lost start acknowledgement. */
export async function workspaceSvgRead(visit: SvgVisit, native: Native | null | undefined, dark = false,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet): Promise<FileSnapshot> {
  const initial = workspaceSvgSnapshot(visit, dark, client, background);
  if (!initial) throw superseded();
  if (!native?.available) return initial;
  const state = ownerOf(client).state!, share = state.share;
  native.watch('t3.mobile-media');
  const response = await bridgeReply(native, { op: 'mobileMediaShareStatus' });
  if (!current(state, client, background)) throw superseded();
  if (response.ok && share === state.share) {
    let status = obj(response.value);
    const active = ['preparing', 'presented'].includes(str(status.phase));
    if (active && status.routeKey === visit.visit && (status.identifier !== state.attempt?.identifier || !state.authorized)) {
      const cancelled = await bridgeReply(native, { op: 'mobileMediaShareCancel', identifier: str(status.identifier) });
      if (!current(state, client, background)) throw superseded();
      if (share !== state.share) return workspaceSvgSnapshot(visit, dark, client, background)!;
      if (cancelled.ok) status = obj(cancelled.value);
    }
    applyShareStatus(state, status);
  }
  return workspaceSvgSnapshot(visit, dark, client, background)!;
}
async function signed(state: State, native: Native, client: T3Client, background: EnvironmentFleet, lane: 'load' | 'share', id: number) {
  const epoch = state.epoch, guard = () => fence(state, client, background, epoch, lane, id);
  const base = letGoAware(mobileNative(native, client, background));
  const resolve = async (call: (input: Obj) => Promise<Obj>, origin: string) => {
    state.authorized = true; guard();
    const result = await call({ op: 'request', method: 'assets.createUrl', payload: { resource: state.scope.resource } });
    guard(); return mobileMediaURL(origin, str(result.relativeUrl));
  };
  if (state.scope.draft) return withStandaloneNewTaskFile(state.scope.environment, base, client, background, guard, resolve);
  guard();
  if (!client.ready || !state.scope.cwd) throw new ClientError('Connect and select a workspace to view its files.');
  const call = async (input: Obj) => { guard(); const result = await client.call(base, input); guard(); return obj(result); };
  const session = await call({ op: 'http', path: '/api/auth/session' });
  if (!mobileSessionGrants(session, 'filesystem:read')) { revoke(state); state.prepared = true; state.error = 'This connection cannot read host files.'; throw new ClientError(state.error); }
  return resolve(call, client.origin);
}
export async function workspaceSvgPrepare(visit: SvgVisit, native: Native | null | undefined, dark = false, refresh = false,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet): Promise<FileSnapshot | null> {
  const snapshot = workspaceSvgSnapshot(visit, dark, client, background); if (!snapshot) return null;
  const state = ownerOf(client).state!;
  if (!native?.available) {
    state.prepared = true; state.loading = false; state.error = 'Open T3 Code on your iPhone or iPad to preview files.';
    return workspaceSvgSnapshot(visit, dark, client, background);
  }
  if (refresh && state.url) { state.revision++; client.revision++; return workspaceSvgSnapshot(visit, dark, client, background); }
  if (state.prepared) return snapshot;
  const id = ++state.load, epoch = state.epoch; state.loading = true; state.error = '';
  let completed = false;
  try { const url = await signed(state, native, client, background, 'load', id); fence(state, client, background, epoch, 'load', id); state.url = url; completed = true; }
  catch (error) {
    if (!ownsInvocation(state, client, background, epoch, 'load', id)) throw superseded();
    if (letGo(error)) throw error;
    state.error = error instanceof Error ? error.message : 'Preview unavailable'; completed = true;
  } finally { if (ownsInvocation(state, client, background, epoch, 'load', id)) { state.prepared = completed; state.loading = false; client.revision++; } }
  return workspaceSvgSnapshot(visit, dark, client, background);
}
export async function workspaceSvgAction(event: string, admission: SvgAdmission, native: Native | null | undefined,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet) {
  let value: Obj; try { value = obj(JSON.parse(event)); } catch { value = {}; }
  const state = ownerOf(client).state, result = { owner: str(value.identifier), message: '', kind: '', uncertain: false };
  if (!state || value.identifier !== admission.owner || admission.owner !== state.scope.owner || !sameVisit(ownerOf(client).live, admission)
    || !native?.available || !current(state, client, background)) return result;
  const report = ++state.report, reportEpoch = state.epoch;
  const canReport = () => current(state, client, background) && state.epoch === reportEpoch && state.report === report;
  try {
    if (value.operation === 'copy-trace') {
      const trace = phaseOf(state.scope, client, background).trace;
      if (!trace || value.trace !== trace) return result;
      const guard = () => { if (!canReport()) throw superseded(); };
      guard(); const copy = await bridgeReply(native, { op: 'copyText', text: trace }); guard();
      if (!copy.ok) throw new ClientError(copy.error!.message, copy.error!.kind, copy.error!.uncertain);
      if (obj(copy.value).copied !== false) { const haptic = await bridgeReply(native, { op: 'mobileHomeHaptic', kind: 'selection' }); guard(); if (!haptic.ok) throw new ClientError(haptic.error!.message); }
    } else if (value.operation === 'refresh') { if (state.url) { state.revision++; client.revision++; } }
    else if (value.operation === 'retry' && !state.sharing) {
      const retry = ++state.retry;
      if (phaseOf(state.scope, client, background).phase !== 'connected') {
        const source = sourceOf(state.scope, client, background);
        const focused = state.scope.environment === client.environmentId && !!client.origin;
        if (source?.enabled === false || !focused && !source) throw new ClientError('Connect to this workspace to view its files.');
        const origin = client.origin, key = source?.key;
        const reply = await bridgeReply(mobileNative(native, client, background), { op: 'retry', ...(focused ? {} : { fleet: source!.key }) });
        if (!sameVisit(ownerOf(client).live, admission) || ownerOf(client).state !== state || state.retry !== retry
          || (focused ? client.environmentId !== state.scope.environment || client.origin !== origin : sourceOf(state.scope, client, background)?.key !== key))
          throw new ClientError('The file route changed.', 'superseded');
        if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
      }
      if (!sameVisit(ownerOf(client).live, admission)) throw new ClientError('The file route changed.', 'superseded');
      const next = workspaceSvgSnapshot(state.scope, false, client, background); if (!next) throw new ClientError('The file route changed.', 'superseded');
      const fresh = ownerOf(client).state!; fresh.load++; fresh.loading = false; fresh.prepared = false; fresh.error = ''; fresh.url = ''; fresh.authorized = false; fresh.revision++;
      client.revision++; result.owner = fresh.scope.owner;
      if (phaseOf(fresh.scope, client, background).phase === 'connected') await workspaceSvgPrepare(fresh.scope, native, false, false, client, background);
    } else if (value.operation === 'share' && !state.sharing) {
      const id = ++state.share, epoch = state.epoch, guard = () => fence(state, client, background, epoch, 'share', id);
      state.sharing = true; client.revision++;
      const identifier = JSON.stringify([state.scope.owner, 'share', id]);
      const attempt = { identifier, dispatched: false }; state.attempt = attempt; state.shareMessage = ''; state.shareReceipt = '';
      try {
        const url = await signed(state, native, client, background, 'share', id); guard();
        attempt.dispatched = true;
        const reply = await bridgeReply(native, { op: 'mobileMediaShare', routeKey: state.scope.visit, sourceJSON: JSON.stringify({ identifier,
          name: state.scope.path.split(/[\\/]/).at(-1) || 'File', kind: 'file', source: 'remote', id: identifier, url }) });
        guard();
        if (!reply.ok) {
          if (!reply.error?.uncertain) { state.attempt = undefined; state.sharing = false; }
          throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain);
        }
        applyShareStatus(state, obj(reply.value));
      } finally {
        if (state.attempt === attempt && !attempt.dispatched) { state.attempt = undefined; state.sharing = false; }
        client.revision++;
      }
    }
  } catch (error) {
    if (!letGo(error) && canReport()) {
      result.message = error instanceof Error ? error.message : 'Could not complete file action';
      result.kind = error instanceof ClientError ? error.kind : ''; result.uncertain = error instanceof ClientError && error.uncertain;
    }
  }
  return result;
}
