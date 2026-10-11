// @ref llp/1109.004-home-projection.decision.md#decision
// T3 Code 365aa87982 archivedThreadList.ts, archivedThreads.ts, useThreadListActions.ts.
// Presentation over shared validated snapshots and per-environment RPC ownership.
// Mobile adaptation: stable Archive IO admission and a separate pure projection;
// warm snapshots retain no grant, handle or promise (LLP1109.009).
import { mobileClient, mobileNative } from './client';
import { mobileSessionGrants } from './environment-detail';
import { mobileRelativeTime } from './home';
import { arr, obj, str, applyShell, initialShell, type Obj, type Shell } from './shared/domain';
import { savedList } from './shared/connection-routes-ops';
import { machineKind } from './shared/connections';
import { letGo, letGoAware } from './shared/let-go';
import { liveEnvironments, type LiveEnvironment } from './shared/live-streams';
import { bridgeReply, ClientError, type Native, type Files } from './shared/protocol';
import { subagentTitle } from './shared/timeline-events';
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
      // Pinned models.ts scopes the subagent display title before matching or sorting.
      const threads = source.shell.threads.filter(thread => thread.projectId === project.id && thread.archivedAt != null)
        .map(thread => {
          const title = obj(thread.lineage).relationshipToParent === 'subagent' ? subagentTitle(str(thread.title)) : str(thread.title);
          return title === thread.title ? thread : { ...thread, title };
        })
        .filter(thread => groupMatches || matches(thread.title) || matches(thread.branch))
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
interface RetainedArchive { identity: string; revision: string; serial: number; value: ArchiveSnapshot }
const snapshots = new Map<string, RetainedArchive>();
let readSerial = 0;
let idle: { since: number; expired: boolean } | null = null;
// Scalar producer identity only: weak membership never retains a fleet entry.
const producers = new WeakMap<object, number>();
let producerSerial = 0;
function producerId(owner: object) {
  let id = producers.get(owner);
  if (id === undefined) {
    if (producerSerial >= Number.MAX_SAFE_INTEGER) throw new ClientError('The Archive owner cannot be identified safely. Restart the app.');
    id = ++producerSerial; producers.set(owner, id);
  }
  return id;
}
export interface ArchiveScope { key: string; presentation: string }
export interface ArchiveReceipt {
  environmentId: string; identity: string; revision: string; origin: string; generation: number;
  focused: boolean; producer: number; granted: boolean;
}
export interface ArchiveRead { serial: number; receipts: ArchiveReceipt[]; error: string }
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

function archiveOwner(environmentId: string) {
  const focused = mobileClient.environmentId === environmentId;
  const entry = [...fleet.entries.values()].find(item => item.environmentId === environmentId);
  return { focused, origin: focused ? mobileClient.origin : entry?.origin ?? '',
    generation: focused ? mobileClient.generation : entry?.generation ?? -1,
    producer: focused ? producerId(mobileClient) : entry ? producerId(entry) : 0,
    connected: catalogEnabled(environmentId) && (focused ? mobileClient.ready
      : entry?.phase === 'connected' && entry.synchronized === entry.generation) };
}
const archiveEnvironments = (saved: Obj[]) => [...new Map(saved.filter(item => mobileCacheCatalogIdentity(saved, str(item.environmentId))).map(item => [str(item.environmentId), {
  id: str(item.environmentId), label: str(item.mobileLabel) || str(item.label) || str(item.environmentId) }])).values()]
  .sort((a, b) => compare(a.label.toLocaleLowerCase(), b.label.toLocaleLowerCase()));

/** No network dependency on unrelated root revisions, stream events or presentation. */
export function mobileArchiveScope(): ArchiveScope {
  const environments = archiveEnvironments(fleet.saved), ids = new Set(environments.map(item => item.id));
  for (const id of snapshots.keys()) if (!ids.has(id)) snapshots.delete(id);
  const key = environments.map(info => {
    const identity = mobileCacheCatalogIdentity(fleet.saved, info.id), revision = archiveRevision(info.id);
    retainedArchive(info.id, identity, revision);
    const owner = archiveOwner(info.id);
    return [info.id, identity, revision, catalogEnabled(info.id), owner.focused, owner.origin, owner.generation, owner.producer, owner.connected];
  }).sort((a, b) => compare(String(a[0]), String(b[0])));
  const presentation = environments.map(info => {
    const config = info.id === mobileClient.environmentId ? mobileClient.config : [...fleet.entries.values()].find(entry => entry.environmentId === info.id)?.config;
    return [info.id, info.label, machineKind(config ?? {})];
  });
  return { key: JSON.stringify(key), presentation: JSON.stringify(presentation) };
}
function receiptCurrent(receipt: ArchiveReceipt) {
  const owner = archiveOwner(receipt.environmentId);
  return catalogCurrent(receipt.environmentId, receipt.identity) && archiveRevision(receipt.environmentId) === receipt.revision
    && owner.connected && owner.focused === receipt.focused && owner.origin === receipt.origin
    && owner.generation === receipt.generation && owner.producer === receipt.producer;
}

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

/** One visit/admission/explicit refresh owns its native calls; no broad inbox watches. */
export async function mobileArchiveRead(visit: string, admission: string, nativeInput?: Native | null): Promise<ArchiveRead> {
  if (readSerial >= Number.MAX_SAFE_INTEGER) throw new ClientError('The Archive read cannot be identified safely. Restart the app.');
  const request = ++readSerial, readCurrent = () => request === readSerial;
  const empty = { serial: request, receipts: [] as ArchiveReceipt[], error: '' };
  if (!visit || !nativeInput?.available) return empty;
  if (admission !== mobileArchiveScope().key) throw new ClientError('The archive request was superseded.', 'superseded');
  const admittedOwners = new Map(archiveEnvironments(fleet.saved).map(info => [info.id, { ...archiveOwner(info.id),
    identity: mobileCacheCatalogIdentity(fleet.saved, info.id), revision: archiveRevision(info.id) }]));
  const native = letGoAware(mobileNative(nativeInput));
  let saved: Obj[];
  try { saved = await savedList(native); }
  catch (error) { if (letGo(error)) throw error; return { ...empty, error: 'Failed to load archived threads.' }; }
  if (!readCurrent()) throw new ClientError('The archive request was superseded.', 'superseded');
  const environments = archiveEnvironments(saved);
  const ids = new Set(environments.map(item => item.id));
  for (const id of snapshots.keys()) if (!ids.has(id)) snapshots.delete(id);
  const results = await Promise.all(environments.map(async info => {
    const identity = mobileCacheCatalogIdentity(saved, info.id), revision = archiveRevision(info.id);
    const owned = () => {
      const admitted = admittedOwners.get(info.id);
      const current = readCurrent() && admitted?.identity === identity && admitted.revision === revision
        && catalogCurrent(info.id, identity) && archiveRevision(info.id) === revision;
      const retained = snapshots.get(info.id);
      if (!current && readCurrent() && retained?.identity === identity && retained.revision === revision) snapshots.delete(info.id);
      return current;
    };
    // This warm view has no durable archive record. Explicit cache-clear
    // eviction is an app ownership adaptation, not upstream atom invalidation.
    retainedArchive(info.id, identity, revision);
    const owner = admittedOwners.get(info.id) ?? { origin: '', generation: -1, focused: false, producer: 0 };
    const receipt: ArchiveReceipt = { environmentId: info.id, identity, revision,
      origin: owner.origin, generation: owner.generation, focused: owner.focused, producer: owner.producer, granted: false };
    try {
      if (!owned() || !receiptCurrent(receipt) || saved.find(row => row.environmentId === info.id)?.enabled === false) throw new ClientError('Connect this environment to load its archived threads.');
      const selected = archiveTransport(info.id, native, () => owned() && receiptCurrent(receipt)), shell = await readArchive(selected.environment);
      let canOperate = false;
      try { canOperate = await sessionPermission(selected.native, selected.generation); } catch (error) { if (letGo(error)) throw error; }
      if (!owned() || !selected.current()) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
      const value: ArchiveSnapshot = { environmentId: info.id, label: info.label, machine: machineKind(selected.environment.config), shell, canOperate };
      // Persistent view memory never keeps a grant. Only this live invocation
      // carries the fresh session decision, rechecked after all peers settle.
      snapshots.set(info.id, { identity, revision, serial: request, value: { ...value, canOperate: false } });
      return { receipt: { ...receipt, granted: canOperate }, failed: false, owned, live: selected.current };
    } catch (error) {
      if (letGo(error)) throw error;
      return { receipt, failed: true, owned, live: () => false };
    }
  }));
  if (!readCurrent()) throw new ClientError('The archive request was superseded.', 'superseded');
  return { ...empty, receipts: results.filter(result => result.owned()).map(result => ({ ...result.receipt, granted: result.receipt.granted && result.live() })),
    error: results.some(result => result.failed || !result.owned() || !result.live()) ? 'Failed to load archived threads.' : '' };
}

/** The resource's live receipt is separate from nongranted retained snapshots. */
export function mobileArchiveView(now: number, query = '', selectedEnvironment = '', sortOrder = 'newest', active = false,
  prepared: ArchiveRead = { serial: 0, receipts: [], error: '' }, observed = active): ArchiveView {
  // A covered Archive route is still an observer. The root's one-shot task
  // wakes this projection after its last mounted route has been absent for 5m.
  if (observed) idle = null;
  else {
    idle ??= { since: now, expired: false };
    if (!idle.expired && now - idle.since >= 300_000) {
      if (readSerial >= Number.MAX_SAFE_INTEGER) throw new ClientError('The Archive read cannot be identified safely. Restart the app.');
      readSerial++; // An already-issued read cannot repopulate an expired cache.
      snapshots.clear(); idle.expired = true;
    }
  }
  const filtered = !!query.trim() || !!selectedEnvironment;
  const empty = { items: [] as ArchiveItem[], environments: [] as ArchiveEnvironment[], error: '', loading: false,
    emptyTitle: filtered ? 'No matching threads' : 'No archived threads', emptyDetail: filtered ? 'Try another search or environment.' : 'Threads you archive will appear here.' };
  if (!active) return empty;
  mobileArchiveScope();
  const environments = archiveEnvironments(fleet.saved), values: ArchiveSnapshot[] = [];
  for (const info of environments) {
    const identity = mobileCacheCatalogIdentity(fleet.saved, info.id), revision = archiveRevision(info.id);
    const retained = retainedArchive(info.id, identity, revision), receipt = prepared.receipts.find(row => row.environmentId === info.id);
    if (!retained) continue;
    const record = snapshots.get(info.id)!;
    if (record.serial === prepared.serial && receipt && !receiptCurrent(receipt)) record.serial = 0;
    const config = info.id === mobileClient.environmentId ? mobileClient.config : [...fleet.entries.values()].find(entry => entry.environmentId === info.id)?.config;
    values.push({ ...retained, label: info.label, machine: machineKind(config ?? {}), canOperate: prepared.serial === readSerial && record.serial === prepared.serial
      && receipt?.granted === true && receiptCurrent(receipt) });
  }
  return { ...empty, environments, error: prepared.error, items: projectMobileArchive(values, now, query, selectedEnvironment, sortOrder) };
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
