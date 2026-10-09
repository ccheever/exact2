// @ref llp/1109.004-home-projection.decision.md#decision
// T3 Code 365aa87982 archivedThreadList.ts, archivedThreads.ts, useThreadListActions.ts.
// Presentation over shared validated snapshots and per-environment RPC ownership.
import { mobileClient, mobileNative } from './client';
import { mobileSessionGrants } from './environment-detail';
import { mobileRelativeTime } from './home';
import { arr, obj, str, applyShell, initialShell, type Obj, type Shell } from './shared/domain';
import { savedList } from './shared/connection-routes-ops';
import { machineKind } from './shared/connections';
import { letGo, letGoAware } from './shared/let-go';
import { liveEnvironments, type LiveEnvironment } from './shared/live-streams';
import { bridgeReply, ClientError, type Native, type Files } from './shared/protocol';
import { EnvironmentFleet, fleet } from './shared/settings-b-fleet';
import { mobileProjectFaviconTarget, type MobileProjectFaviconTarget } from './mobile-project-favicon';
import { mobileCacheCatalogIdentity } from './mobile-client-cache-catalog';
import { mobileCacheDisplayRevision } from './mobile-client-cache';
import { ICON_COLORS, projectIdentity } from './shared/settings-b-icons';

export interface ArchiveSnapshot { environmentId: string; label: string; machine: string; shell: Shell; canOperate: boolean }
export interface ArchiveEnvironment { id: string; label: string }
export interface ArchiveItem {
  key: string; kind: string; environmentId: string; projectId: string; threadId: string; title: string;
  subtitle: string; environmentLabel: string; machineSymbol: string; time: string;
  initial: boolean; first: boolean; last: boolean; final: boolean; canOperate: boolean;
  faviconTarget: MobileProjectFaviconTarget; favicon: string; iconKind: string; iconText: string; iconColor: string; iconSurface: string; iconSize: number;
}
export interface ArchiveView { items: ArchiveItem[]; environments: ArchiveEnvironment[]; error: string; loading: boolean; emptyTitle: string; emptyDetail: string }
const machines: Record<string, string> = { server: 'server.rack', cloud: 'cloud', linux: 'terminal', desktop: 'desktopcomputer', laptop: 'laptopcomputer', 'mac-mini': 'macmini', 'mac-studio': 'macstudio' };
const compare = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
const stamp = (thread: Obj) => { const ms = Date.parse(str(thread.archivedAt ?? thread.updatedAt)); return Number.isNaN(ms) ? 0 : ms; };
const blank = (key: string): ArchiveItem => ({ key, kind: 'thread', environmentId: '', projectId: '', threadId: '', title: '', subtitle: '', environmentLabel: '', machineSymbol: '', time: '', initial: false, first: false, last: false, final: false, canOperate: false, faviconTarget: mobileProjectFaviconTarget(''), favicon: '', iconKind: '', iconText: '', iconColor: '', iconSurface: '', iconSize: 0 });

/** Exact source grouping: filter groups by environment, then project/path/environment or thread/branch query. */
export function projectMobileArchive(snapshots: ArchiveSnapshot[], now: number, queryInput = '', environmentId = '', sortOrder = 'newest'): ArchiveItem[] {
  const query = queryInput.trim().toLocaleLowerCase(), direction = sortOrder === 'oldest' ? 1 : -1;
  const matches = (value: unknown) => str(value).toLocaleLowerCase().includes(query);
  const groups: { source: ArchiveSnapshot; project: Obj; threads: Obj[]; key: string }[] = [];
  for (const source of snapshots) {
    if (environmentId && source.environmentId !== environmentId) continue;
    for (const project of source.shell.projects) {
      const groupMatches = !query || [project.title, project.workspaceRoot, source.label].some(matches);
      const threads = source.shell.threads.filter(thread => thread.projectId === project.id && thread.archivedAt != null
        && (groupMatches || matches(thread.title) || matches(thread.branch)))
        .sort((a, b) => direction * (stamp(a) - stamp(b)) || compare(str(a.title), str(b.title)) || compare(str(a.id), str(b.id)));
      if (threads.length) groups.push({ source, project, threads, key: `${source.environmentId}:${str(project.id)}` });
    }
  }
  groups.sort((a, b) => direction * (stamp(a.threads[0]!) - stamp(b.threads[0]!)) || compare(str(a.project.title), str(b.project.title)) || compare(a.key, b.key));
  const items: ArchiveItem[] = [];
  for (const { source, project, threads, key } of groups) {
    const icon = obj(project.projectIcon), iconKind = icon.kind === 'lucide' ? 'monogram' : str(icon.kind);
    const iconText = icon.kind === 'lucide' ? projectIdentity(str(project.title).normalize('NFKC')).monogram : str(icon.emoji ?? icon.text);
    const iconColor = ICON_COLORS.find(color => color.value === icon.color)?.swatch ?? '';
    items.push({ ...blank(`${key}:project`), kind: 'project', environmentId: source.environmentId, projectId: str(project.id),
      title: str(project.title), environmentLabel: source.label, machineSymbol: machines[source.machine] ?? machines.server!,
      faviconTarget: mobileProjectFaviconTarget(source.environmentId, project), iconKind, iconText, iconColor, iconSurface: iconColor ? `${iconColor}26` : '',
      iconSize: 18 * (Array.from(iconText.replace(/\p{M}/gu, '')).length === 1 ? .6 : .515625) });
    threads.forEach((thread, index) => items.push({ ...blank(`${source.environmentId}:${str(thread.id)}`),
      environmentId: source.environmentId, projectId: str(project.id), threadId: str(thread.id), title: str(thread.title),
      subtitle: [source.label, str(thread.branch)].filter(Boolean).join(' · '), environmentLabel: source.label,
      time: mobileRelativeTime(thread.archivedAt ?? thread.updatedAt, now), first: index === 0, last: index === threads.length - 1, canOperate: source.canOperate }));
  }
  if (items.length) { items[0]!.initial = true; items[items.length - 1]!.final = true; }
  return items;
}

// View cache only: shared applyShell validates the server snapshot; no second live reducer.
interface RetainedArchive { identity: string; revision: string; value: ArchiveSnapshot }
const snapshots = new Map<string, RetainedArchive>();
let readSerial = 0;
const archiveRevision = (id: string) => mobileCacheDisplayRevision(id, 'shell');
const catalogCurrent = (id: string, identity: string) => !!identity && mobileCacheCatalogIdentity(fleet.saved, id) === identity;
const catalogEnabled = (id: string) => fleet.saved.filter(row => row.environmentId === id).length === 1
  && fleet.saved.find(row => row.environmentId === id)?.enabled !== false;
function retainedArchive(id: string, identity: string, revision: string): ArchiveSnapshot | null {
  const previous = snapshots.get(id);
  if (!previous) return null;
  if (!catalogCurrent(id, identity) || previous.identity !== identity || previous.revision !== revision || archiveRevision(id) !== revision) {
    snapshots.delete(id); return null;
  }
  return { ...previous.value, canOperate: false };
}
const inFlight = new Set<string>();
const errorText = (error: unknown) => error instanceof Error ? error.message : 'The action could not be completed. Try again.';

/** Existing liveEnvironments supplies request and command IDs. Lock its native seam to this generation. */
function archiveTransport(environmentId: string, native: Native, scopeCurrent: () => boolean = () => true): { environment: LiveEnvironment; native: Native; generation: number; current(): boolean } {
  const focused = mobileClient.environmentId === environmentId;
  const entry = [...fleet.entries.values()].find(item => item.environmentId === environmentId);
  const generation = focused ? mobileClient.generation : entry?.generation ?? -1, origin = focused ? mobileClient.origin : entry?.origin;
  const current = () => scopeCurrent() && catalogEnabled(environmentId) && (focused
    ? mobileClient.environmentId === environmentId && mobileClient.origin === origin && mobileClient.generation === generation && mobileClient.ready
    : mobileClient.environmentId !== environmentId && fleet.entries.get(entry?.key ?? '') === entry && entry?.origin === origin
      && entry?.generation === generation && entry?.phase === 'connected' && entry?.synchronized === generation);
  const guarded: Native = { available: native.available, watch: topic => native.watch(topic), later: async request => {
    const writing = obj(request).method === 'orchestration.dispatchCommand';
    if (!current()) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
    let reply;
    try { reply = await bridgeReply(native, request); }
    catch (error) { if (letGo(error)) throw error; throw error instanceof ClientError ? error : new ClientError(errorText(error), 'transport', writing); }
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain, reply.error!);
    if (!current() || reply.generation !== generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale', writing);
    return reply;
  } };
  const environment = liveEnvironments(mobileClient, guarded).find(item => item.environmentId === environmentId);
  if (!environment?.connected || !current()) throw new ClientError('Connect this environment to load its archived threads.');
  return { environment, native: focused ? guarded : EnvironmentFleet.native(guarded, environment.key), generation, current };
}
async function sessionPermission(native: Native, generation: number) {
  const reply = await bridgeReply(native, { op: 'http', path: '/api/auth/session', generation });
  return reply.ok && mobileSessionGrants(obj(reply.value), 'orchestration:operate');
}
async function readArchive(environment: LiveEnvironment) {
  return applyShell(initialShell(), await environment.request('orchestration.getArchivedShellSnapshot', {}));
}

/** Root owns refresh/revision/query/filter/clock dependencies. Each saved environment keeps its own request. */
export async function mobileArchive(now: number, query = '', selectedEnvironment = '', sortOrder = 'newest', nativeInput?: Native | null): Promise<ArchiveView> {
  const request = ++readSerial, readCurrent = () => request === readSerial;
  const filtered = !!query.trim() || !!selectedEnvironment;
  const empty = { items: [] as ArchiveItem[], environments: [] as ArchiveEnvironment[], error: '', loading: false,
    emptyTitle: filtered ? 'No matching threads' : 'No archived threads', emptyDetail: filtered ? 'Try another search or environment.' : 'Threads you archive will appear here.' };
  if (!nativeInput?.available) return empty;
  const native = letGoAware(mobileNative(nativeInput)); native.watch('t3.status'); native.watch('t3.fleet');
  let saved: Obj[];
  try { saved = await savedList(native); }
  catch (error) { if (letGo(error)) throw error; return { ...empty, error: 'Failed to load archived threads.' }; }
  if (!readCurrent()) throw new ClientError('The archive request was superseded.', 'superseded');
  const environments = [...new Map(saved.filter(item => mobileCacheCatalogIdentity(saved, str(item.environmentId))).map(item => [str(item.environmentId), {
    id: str(item.environmentId), label: str(item.mobileLabel) || str(item.label) || str(item.environmentId) }])).values()]
    .sort((a, b) => compare(a.label.toLocaleLowerCase(), b.label.toLocaleLowerCase()));
  const ids = new Set(environments.map(item => item.id));
  for (const id of snapshots.keys()) if (!ids.has(id)) snapshots.delete(id);
  const results = await Promise.all(environments.map(async info => {
    const identity = mobileCacheCatalogIdentity(saved, info.id), revision = archiveRevision(info.id);
    const owned = () => {
      const current = readCurrent() && catalogCurrent(info.id, identity) && archiveRevision(info.id) === revision;
      const retained = snapshots.get(info.id);
      if (!current && readCurrent() && retained?.identity === identity && retained.revision === revision) snapshots.delete(info.id);
      return current;
    };
    // This warm view has no durable archive record. Explicit cache-clear
    // eviction is an app ownership adaptation, not upstream atom invalidation.
    const previous = retainedArchive(info.id, identity, revision);
    try {
      if (!owned() || saved.find(row => row.environmentId === info.id)?.enabled === false) throw new ClientError('Connect this environment to load its archived threads.');
      const selected = archiveTransport(info.id, native, owned), shell = await readArchive(selected.environment);
      let canOperate = false;
      try { canOperate = await sessionPermission(selected.native, selected.generation); } catch (error) { if (letGo(error)) throw error; }
      if (!owned() || !selected.current()) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
      const value: ArchiveSnapshot = { environmentId: info.id, label: info.label, machine: machineKind(selected.environment.config), shell, canOperate };
      // Persistent view memory never keeps a grant. Only this live invocation
      // carries the fresh session decision, rechecked after all peers settle.
      snapshots.set(info.id, { identity, revision, value: { ...value, canOperate: false } });
      return { value, failed: false, owned, live: selected.current };
    } catch (error) {
      if (letGo(error)) throw error;
      return { value: owned() && previous ? { ...previous, label: info.label, canOperate: false } : null, failed: true, owned, live: () => false };
    }
  }));
  if (!readCurrent()) throw new ClientError('The archive request was superseded.', 'superseded');
  const current = results.filter(result => result.owned());
  const items = projectMobileArchive(current.flatMap(result => result.value ? [{ ...result.value, canOperate: result.value.canOperate && result.live() }] : []), now, query, selectedEnvironment, sortOrder);
  return { ...empty, items, environments: environments.filter(info => catalogCurrent(info.id, mobileCacheCatalogIdentity(saved, info.id))),
    error: results.some(result => result.failed || !result.owned() || !result.live()) ? 'Failed to load archived threads.' : '' };

}

/** Root confirms deletion first; permission, generation and archive membership are rechecked here. */
export async function mobileArchiveCommand(kind: string, environmentId: string, projectId: string, threadId: string, nativeInput?: Native | null, _storage?: Files) {
  const result = (message: string) => ({ revision: ++mobileClient.revision, message });
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad.');
  if (!['unarchive', 'delete'].includes(kind)) return result('Unknown archived thread action.');
  const key = JSON.stringify([environmentId, threadId]);
  if (inFlight.has(key)) return result('This thread action is already running.');
  inFlight.add(key);
  try {
    const native = letGoAware(mobileNative(nativeInput)), saved = await savedList(native);
    const identity = mobileCacheCatalogIdentity(saved, environmentId);
    if (!catalogCurrent(environmentId, identity) || !saved.some(item => item.environmentId === environmentId && item.enabled !== false)) throw new ClientError('Connect this environment before changing archived threads.');
    const selected = archiveTransport(environmentId, native, () => catalogCurrent(environmentId, identity));
    if (!await sessionPermission(selected.native, selected.generation)) throw new ClientError('This connection does not have permission to change threads.');
    const shell = await readArchive(selected.environment);
    if (!shell.projects.some(project => project.id === projectId) || !shell.threads.some(thread => thread.id === threadId && thread.projectId === projectId && thread.archivedAt != null))
      throw new ClientError('That archived thread is no longer available.');
    const [commandId] = await selected.environment.ids(1);
    await selected.environment.request('orchestration.dispatchCommand', { type: `thread.${kind}`, commandId, threadId }, true);
    snapshots.delete(environmentId);
    return result('');
  } catch (error) {
    if (letGo(error)) throw error;
    return result(errorText(error) + (error instanceof ClientError && error.uncertain ? ' The request may have reached the server. Refresh the archive before retrying.' : ''));
  } finally { inFlight.delete(key); }
}
