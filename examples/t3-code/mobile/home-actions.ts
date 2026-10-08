// Pinned T3 Code365aa87982 thread-list-v2-items, useThreadListActions and HomeRouteScreen.
// @ref llp/1107.004-home-projection.decision.md#decision
// @ref llp/1107.011-responsive-workspace.decision.md#navigation-and-data-ownership
import { mobileClient, mobileNative } from './client';
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { liveEnvironments } from './shared/live-streams';
import { EnvironmentFleet, fleet } from './shared/settings-b-fleet';
import { capabilities, canSnooze, orderKeyBetween, sectionOf, sidebarVisible, shellRuntime, pendingApproval, pendingInput } from './shared/sidebar-model';
import { snoozePresets } from './shared/sidebar-presentation';
import { resolveRenameCommit } from './shared/shell-commands';
import { mobileSessionGrants } from './mobile-grants';
import { mobileHomeSources } from './home';
import { homeCreatePending, homeReconcilePending, homeMovePlan, homeOrderKey, homeOrderState, mobileHomeOrder, type HomeOrderSnapshot, type HomeMoveDirection } from './home-order';

export interface HomeMenuItem {
  id: string; parentId: string; label: string; operation: string; value: string; symbol: string; subtitle: string;
  destructive: boolean; checked: boolean; disabled: boolean;
}
export interface HomeActionResult {
  revision: number; requestRoute: string; message: string; alertTitle: string; nextLocation: string;
  archiveChanged: boolean; uncertain: boolean;
}
interface Surface { requestRoute: string; homeVisible: boolean; sidebarVisible: boolean }
interface Uncertain { origin: string; environmentId: string; threadId: string; payload: Obj; before: Obj }
const surfaces = new WeakMap<T3Client, Surface>();
const locks = new WeakMap<T3Client, Set<string>>();
const uncertainWrites = new WeakMap<T3Client, Map<string, Uncertain>>();
function lockSet(client: T3Client) { let value = locks.get(client); if (!value) { value = new Set(); locks.set(client, value); } return value; }
function uncertainSet(client: T3Client) { let value = uncertainWrites.get(client); if (!value) { value = new Map(); uncertainWrites.set(client, value); } return value; }

/** Root observes both current surfaces with identical inputs; this performs no I/O. */
export function mobileHomeActionsObserve(requestRoute: string, homeVisible: boolean, sidebarVisible: boolean, client: T3Client = mobileClient): void {
  const prior = surfaces.get(client);
  if (!prior || prior.requestRoute !== requestRoute || prior.homeVisible !== homeVisible || prior.sidebarVisible !== sidebarVisible)
    surfaces.set(client, { requestRoute, homeVisible, sidebarVisible });
}
function row(environmentId: string, threadId: string, client: T3Client, background: EnvironmentFleet) {
  const environment = liveEnvironments(client, null, background).find(value => value.environmentId === environmentId);
  const thread = environment?.shell.threads.find(value => value.id === threadId && sidebarVisible(value));
  const scopes = environment?.focused ? client.scopes : background.entries.get(environment?.key ?? '')?.scopes ?? [];
  return { environment, thread, allowed: !!environment?.connected && scopes.includes('orchestration:operate') };
}
function item(id: string, label: string, symbol = '', extra: Partial<HomeMenuItem> = {}): HomeMenuItem {
  return { id, parentId: '', label, operation: id, value: '', symbol, subtitle: '', destructive: false, checked: false, disabled: false, ...extra };
}
/** Shared preset calendar arithmetic, with mobile's device-locale time labels. */
export function mobileHomeSnoozePresets(now: number) {
  return snoozePresets(now, 'auto').map(value => {
    const date = new Date(value.until), time = date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
    return { id: value.id, label: value.label, value: value.until,
      subtitle: value.id === 'next-week' ? `${date.toLocaleDateString(undefined, { weekday: 'short' })} ${time}` : time };
  });
}
/** Source re-resolves relative presets at tap time; a vanished calendar preset may retain its displayed future date. */
export function mobileHomeSnoozeSelection(operation: string, displayed: string, now: number): string {
  if (!operation.startsWith('snooze:') || operation === 'snooze:custom') return '';
  const id = operation.slice(7), current = mobileHomeSnoozePresets(now).find(value => value.id === id);
  if (current) return current.value;
  return ['hour', 'three-hours', 'evening', 'tomorrow', 'next-week'].includes(id) && Date.parse(displayed) > now ? displayed : '';
}

/** Ordinary rows only. Arrange and swipes are later slices. */
export function mobileHomeMenu(environmentId: string, threadId: string, now: number,
  client: T3Client = mobileClient, background: EnvironmentFleet = fleet, orderInput?: HomeOrderSnapshot): HomeMenuItem[] {
  const { environment, thread, allowed } = row(environmentId, threadId, client, background);
  if (!allowed || !environment || !thread) return [];
  const order = orderInput ?? mobileHomeOrder(client, mobileHomeSources(client, background), now);
  const caps = capabilities(environment.config), queued = order.queuedThreadKeys.has(`${environmentId}:${threadId}`);
  const section = sectionOf(queued ? { ...thread, settledOverride: null } : thread, caps, now, false), snoozed = section === 'snoozed';
  const items: HomeMenuItem[] = [];
  if (str(thread.branch)) items.push(item('new-thread-on-branch', 'New thread on branch', 'square.and.pencil'));
  items.push(item('copy-thread-id', 'Copy thread ID', 'doc.on.doc'));
  const lifecycle = snoozed ? 'unsnooze' : !caps.settlement ? 'archive' : section === 'settled' ? 'unsettle' : 'settle';
  items.push(item(lifecycle, { unsnooze: 'Wake thread', archive: 'Archive', unsettle: 'Un-settle', settle: 'Settle' }[lifecycle],
    { unsnooze: 'clock', archive: 'archivebox', unsettle: 'arrow.uturn.backward', settle: 'checkmark' }[lifecycle]));
  if (!snoozed && caps.settlement && section !== 'settled' && caps.snooze && canSnooze(thread, now)) {
    items.push(item('snooze', 'Snooze', 'clock', { operation: '' }));
    for (const preset of mobileHomeSnoozePresets(now)) items.push(item(`snooze:${preset.id}`, preset.label, '', { parentId: 'snooze', value: preset.value, subtitle: preset.subtitle }));
    items.push(item('snooze:custom', 'Custom…', '', { parentId: 'snooze' }));
  }
  const moveSection = thread.pinnedAt != null ? 'pinned' : 'active';
  const movesSupported = moveSection === 'pinned' ? caps.pinReorder : !order.workingEnabled && caps.activeReorder;
  if (!snoozed && section !== 'settled' && movesSupported) {
    const available = order.availability.get(`${environmentId}:${threadId}`);
    items.push(item('move-up', 'Move up', 'arrow.up', { disabled: available?.canMoveUp !== true }));
    items.push(item('move-down', 'Move down', 'arrow.down', { disabled: available?.canMoveDown !== true }));
  }
  if (!snoozed && caps.pinning) items.push(thread.pinnedAt != null ? item('unpin', 'Unpin', 'pin.slash') : item('pin', 'Pin', 'pin'));
  items.push(item('rename', 'Rename', 'square.and.pencil'));
  if (caps.titleRegeneration) items.push(item('regenerate-title', thread.titleRegeneration != null ? 'Regenerating…' : 'Regenerate title', 'arrow.clockwise', { disabled: thread.titleRegeneration != null }));
  if ((snoozed || caps.settlement) && caps.autoSettleOptOut) {
    items.push(item('auto-settle', 'Auto-settle behavior', 'timer', { operation: '' }));
    items.push(item('auto-settle:enabled', 'Enabled', '', { parentId: 'auto-settle', checked: thread.autoSettleDisabledAt == null }));
    items.push(item('auto-settle:disabled', 'Disabled', '', { parentId: 'auto-settle', checked: thread.autoSettleDisabledAt != null }));
  }
  items.push(item('delete', 'Delete', 'trash', { destructive: true }));
  return items;
}
const titles: Record<string, string> = { rename: 'Could not rename thread', delete: 'Could not delete thread',
  archive: 'Could not archive thread', settle: 'Could not settle thread', unsettle: 'Could not un-settle thread',
  unsnooze: 'Could not wake thread', pin: 'Could not pin thread', unpin: 'Could not unpin thread',
  'regenerate-title': 'Could not regenerate title', 'copy-thread-id': 'Could not copy thread ID', 'new-thread-on-branch': 'Could not start task' };
const titleFor = (kind: string) => kind.startsWith('snooze:') ? 'Could not snooze thread' : kind.startsWith('auto-settle:') ? 'Could not update auto-settle' : titles[kind] ?? 'Could not update thread';
/** Mobile checks the presented runtime, including background-work idle projection. */
export function mobileHomeCanArchive(thread: Obj): boolean {
  const runtime = shellRuntime(thread);
  return runtime?.status === 'queued' ? !runtime.activeRunId : !['preparing', 'starting', 'running'].includes(runtime?.status ?? '');
}
function reflected(pending: Uncertain, thread: Obj | undefined): boolean {
  const p = pending.payload;
  if (p.type === 'thread.delete') return !thread;
  if (!thread) return false;
  if (p.type === 'thread.archive') return thread.archivedAt != null;
  if (p.type === 'thread.pin') return thread.pinnedAt != null && (p.orderKey === undefined || thread.pinOrderKey === p.orderKey);
  if (p.type === 'thread.unpin') return thread.pinnedAt == null;
  if (p.type === 'thread.settle') return thread.settledOverride === 'settled';
  if (p.type === 'thread.unsettle') return thread.settledOverride === 'active';
  if (p.type === 'thread.snooze') return thread.snoozedUntil === p.snoozedUntil;
  if (p.type === 'thread.unsnooze') return thread.snoozedUntil == null;
  if (p.type === 'thread.auto-settle.set') return (thread.autoSettleDisabledAt == null) === p.enabled;
  if (p.type === 'thread.metadata.update' && typeof p.title === 'string') return thread.title === p.title;
  return p.regenerateTitle === true && (thread.titleRegeneration != null || thread.title !== pending.before.title);
}

/** A single root mutation owns prompt, permission check and actual command. */
export async function mobileHomeAction(requestRoute: string, environmentId: string, threadId: string, kind: string, value: string, now: number,
  nativeInput?: Native | null, client: T3Client = mobileClient, background: EnvironmentFleet = fleet): Promise<HomeActionResult> {
  if (kind === 'move-up' || kind === 'move-down') return mobileHomeMove(requestRoute, environmentId, threadId, kind === 'move-up' ? 'up' : 'down', now, nativeInput, client, background);
  const surface = surfaces.get(client), key = JSON.stringify([environmentId, threadId]), held = lockSet(client);
  const result = (message = '', nextLocation = '', archiveChanged = false, uncertain = false): HomeActionResult => ({
    revision: client.revision, requestRoute, message, alertTitle: message ? titleFor(kind) : '', nextLocation, archiveChanged, uncertain });
  const currentSurface = () => !!surface && surfaces.get(client) === surface && surface.requestRoute === requestRoute && (surface.homeVisible || surface.sidebarVisible);
  if (!currentSurface() || held.has(key)) return result();
  const initial = row(environmentId, threadId, client, background), focused = initial.environment?.focused;
  const entry = focused ? undefined : background.entries.get(initial.environment?.key ?? '');
  const generation = focused ? client.generation : entry?.generation ?? -1, origin = focused ? client.origin : entry?.origin ?? '';
  const endpointCurrent = () => focused ? client.environmentId === environmentId && client.origin === origin && client.generation === generation && client.ready
    : !!entry && background.entries.get(entry.key) === entry && entry.origin === origin && entry.environmentId === environmentId && entry.generation === generation && entry.phase === 'connected' && entry.synchronized === generation;
  const check = () => { if (!currentSurface() || !endpointCurrent()) throw new ClientError('The thread action changed.', 'superseded'); };
  const pendingKey = JSON.stringify([origin, environmentId, threadId]), pending = uncertainSet(client).get(pendingKey);
  if (pending) {
    const thread = initial.environment?.shell.threads.find(item => item.id === threadId);
    if (initial.environment?.connected && reflected(pending, thread)) uncertainSet(client).delete(pendingKey);
    else return result('The previous request may have reached the server. Wait for the thread to refresh before trying again.', '', false, true);
  }
  if (!initial.allowed || !initial.thread || !initial.environment || !nativeInput?.available) return result('This connection cannot change threads.');
  held.add(key); client.revision++;
  let sent = false, acknowledged = false, refused = false, payload: Obj = {};
  try {
    const base = letGoAware(mobileNative(nativeInput));
    const guarded: Native = { available: base.available, watch: topic => { check(); base.watch(topic); }, later: async input => {
      check(); const request = obj(input), writing = request.method === 'orchestration.dispatchCommand';
      if (writing) sent = true;
      const response = await base.later(input), raw = obj(response);
      if (writing) { acknowledged = raw.ok === true && raw.generation === generation; refused = raw.ok === false && raw.generation === generation && obj(raw.error).uncertain !== true; }
      if (['http', 'request', 'ids', 'mobileCustomSnooze'].includes(str(request.op)) && (!endpointCurrent() || raw.generation !== generation))
        throw new ClientError('The connection changed. Refresh this thread.', 'stale', writing && !acknowledged);
      if (!sent) check();
      if (raw.ok === false) throw new ClientError(str(obj(raw.error).message, 'The request failed.'), str(obj(raw.error).kind, 'transport'), obj(raw.error).uncertain === true);
      return response;
    } };
    const environment = liveEnvironments(client, guarded, background).find(candidate => candidate.environmentId === environmentId)!;
    const endpointNative = focused ? guarded : EnvironmentFleet.native(guarded, environment.key);
    const local = async (request: Obj) => {
      const reply = await bridgeReply(guarded, request); if (!reply.ok) throw new ClientError(reply.error!.message); return obj(reply.value);
    };
    const grant = async () => {
      const response = await bridgeReply(endpointNative, { op: 'http', path: '/api/auth/session', generation });
      if (!response.ok || !mobileSessionGrants(obj(response.value), 'orchestration:operate')) throw new ClientError('This connection cannot change threads.');
    };
    const freshThread = () => {
      check(); const state = row(environmentId, threadId, client, background);
      if (!state.allowed || !state.thread) throw new ClientError('That thread is no longer available.');
      return state.thread;
    };
    let eligibilityTime = now;
    const available = () => {
      const thread = freshThread(), caps = capabilities(row(environmentId, threadId, client, background).environment!.config);
      if (['settle', 'unsettle'].includes(kind) && !caps.settlement) throw new ClientError("This environment's server does not support settling yet. Update the server to use Settle.");
      if ((kind.startsWith('snooze:') || kind === 'unsnooze') && !caps.snooze) throw new ClientError(kind === 'unsnooze'
        ? "This environment's server does not support snoozing yet. Update the server to wake this thread."
        : "This environment's server does not support snoozing yet. Update the server to use Snooze.");
      if (kind.startsWith('snooze:') && !canSnooze(thread, eligibilityTime)) throw new ClientError(pendingApproval(thread) || pendingInput(thread)
        ? 'This thread is waiting on you. Respond to the pending request before snoozing it.'
        : "This thread is still starting a turn. Try again once it's running.");
      if (['pin', 'unpin'].includes(kind) && !caps.pinning) throw new ClientError("This environment's server does not support pinning yet. Update the server to use Pin.");
      if (kind.startsWith('auto-settle:') && !caps.autoSettleOptOut) throw new ClientError("This environment's server does not support turning auto-settle off per thread yet. Update the server to use it.");
      if (kind === 'regenerate-title' && thread.titleRegeneration != null) throw new ClientError('');
      if (kind === 'regenerate-title' && !caps.titleRegeneration) throw new ClientError("This environment's server does not support title regeneration yet. Update the server to regenerate thread titles.");
      const menu = mobileHomeMenu(environmentId, threadId, eligibilityTime, client, background);
      if (!menu.some(item => item.operation === kind && !item.disabled) && !(kind.startsWith('snooze:') && kind !== 'snooze:custom' && menu.some(item => item.id === 'snooze')))
        throw new ClientError('This thread action is no longer available.');
    };
    await grant(); available();
    const original = freshThread();
    if (kind === 'new-thread-on-branch') {
      const query = new URLSearchParams({ environmentId, projectId: str(original.projectId) });
      if (str(original.branch)) query.set('branch', str(original.branch));
      if (str(original.worktreePath)) query.set('worktreePath', str(original.worktreePath));
      return result('', `/new/draft?${query}`);
    }
    if (kind === 'copy-thread-id') {
      const reply = await local({ op: 'copyText', text: threadId, generation });
      if (reply.copied === false) return result('Clipboard access is unavailable in this session.');
      await local({ op: 'mobileHomeHaptic', kind: 'light' }); return result();
    }
    let renamed = '', customSnoozedUntil = '';
    if (kind === 'snooze:custom') {
      const answer = await local({ op: 'mobileCustomSnooze', requestRoute, environmentId, threadId, origin, generation });
      if (answer.choice !== 'snooze') return result();
      const until = str(answer.snoozedUntil), stamp = Date.parse(until), confirmedAt = answer.confirmedAt;
      if (!Number.isFinite(stamp) || new Date(stamp).toISOString() !== until
        || typeof confirmedAt !== 'number' || !Number.isInteger(confirmedAt) || Math.abs(confirmedAt) > 8.64e15 || stamp <= confirmedAt)
        throw new ClientError('The snooze picker returned an invalid date and time.');
      // Native validates future time at confirmation. Network latency must not move the selected date.
      customSnoozedUntil = until; eligibilityTime = confirmedAt;
      await grant(); available();
    }
    if (kind === 'rename') {
      const answer = await local({ op: 'mobilePrompt', title: 'Rename thread', initialValue: str(original.title), cancelLabel: 'Cancel', submitLabel: 'OK' });
      if (answer.choice !== 'submit') return result();
      const resolved = resolveRenameCommit(str(answer.text), str(original.title));
      if (resolved.action === 'reject-empty') return result('Thread title cannot be empty.');
      if (resolved.action === 'noop') return result();
      renamed = resolved.title; await grant(); available();
    }
    if (kind === 'delete') {
      const answer = await local({ op: 'mobileAlert', kind: 'delete', title: 'Delete thread?', message: `“${str(original.title)}” will be permanently deleted, including its terminal history.` });
      if (answer.choice !== 'delete') return result();
      await grant(); available();
    }
    const thread = freshThread(), caps = capabilities(row(environmentId, threadId, client, background).environment!.config);
    payload = { threadId };
    if (kind === 'rename') Object.assign(payload, { type: 'thread.metadata.update', title: renamed });
    else if (kind === 'regenerate-title') Object.assign(payload, { type: 'thread.metadata.update', regenerateTitle: true });
    else if (kind.startsWith('auto-settle:')) Object.assign(payload, { type: 'thread.auto-settle.set', enabled: kind === 'auto-settle:enabled' });
    else if (kind.startsWith('snooze:')) {
      const until = kind === 'snooze:custom' ? customSnoozedUntil : mobileHomeSnoozeSelection(kind, value, now);
      if (!until) throw new ClientError('That snooze time has passed. Choose another time.');
      Object.assign(payload, { type: 'thread.snooze', snoozedUntil: until });
    } else {
      payload.type = `thread.${kind}`;
      if (kind === 'unsettle' || kind === 'unsnooze') payload.reason = 'user';
      if (kind === 'archive' && !mobileHomeCanArchive(thread)) throw new ClientError('This thread is working. Interrupt it first, then try again.');
      if (kind === 'pin' && caps.pinReorder) {
        const keys = liveEnvironments(client, null, background).flatMap(env => env.shell.threads.filter(item => item.pinnedAt != null && typeof item.pinOrderKey === 'string').map(item => str(item.pinOrderKey))).sort();
        const orderKey = orderKeyBetween(null, keys[0] ?? null); if (orderKey !== null) payload.orderKey = orderKey;
      }
    }
    // Haptic failure is feedback-only, but supersession still cancels a not-yet-sent write.
    try { await local({ op: 'mobileHomeHaptic', kind: 'light' }); } catch (error) { if (letGo(error)) throw error; }
    const [commandId] = await environment.ids(1); available();
    if (kind === 'archive' && !mobileHomeCanArchive(freshThread())) throw new ClientError('This thread is working. Interrupt it first, then try again.');
    payload.commandId = commandId!;
    await environment.request('orchestration.dispatchCommand', payload, true);
    return currentSurface() ? result('', '', kind === 'archive' || kind === 'delete') : result();
  } catch (error) {
    const uncertain = sent && !acknowledged && !refused;
    if (uncertain) uncertainSet(client).set(pendingKey, { origin, environmentId, threadId, payload: { ...payload }, before: { ...initial.thread } });
    if (letGo(error)) throw error;
    return currentSurface() ? result((error instanceof Error ? error.message : 'The action could not be completed.')
      + (uncertain ? ' The request may have reached the server. Refresh the thread before retrying.' : ''), '', false, uncertain) : result();
  } finally { held.delete(key); client.revision++; }
}

/** Source moveThread: one shared order hold and sequential assignments across captured endpoints. */
async function mobileHomeMove(requestRoute: string, environmentId: string, threadId: string, direction: HomeMoveDirection, now: number,
  nativeInput: Native | null | undefined, client: T3Client, background: EnvironmentFleet): Promise<HomeActionResult> {
  const surface = surfaces.get(client), state = homeOrderState(client), held = lockSet(client);
  const currentSurface = () => !!surface && surfaces.get(client) === surface && surface.requestRoute === requestRoute && (surface.homeVisible || surface.sidebarVisible);
  const result = (message = '', uncertain = false): HomeActionResult => ({ revision: client.revision, requestRoute, message,
    alertTitle: message ? 'Could not move thread' : '', nextLocation: '', archiveChanged: false, uncertain });
  if (!currentSurface()) return result();
  const initial = row(environmentId, threadId, client, background);
  if (!initial.allowed || !initial.thread || !nativeInput?.available) return result('This connection cannot change threads.');
  const first = mobileHomeOrder(client, mobileHomeSources(client, background), now);
  if (state.unknown) return result('The previous move may have reached the server. Wait for the thread order to refresh before trying again.', true);
  if (first.blocked) return result();
  const section = initial.thread.pinnedAt != null ? 'pinned' : 'active', movedId = `${environmentId}:${threadId}`;
  if (section === 'active' && first.workingEnabled) return result();
  if (!first.reorderable[section].has(environmentId)) return result("This environment's server does not support reordering these threads. Update the server to arrange them.");
  const assignments = homeMovePlan({ ordered: first.sections[section], allThreads: first.threads, section,
    reorderableEnvironmentIds: first.reorderable[section] }, movedId, direction);
  if (!assignments) return result();
  const byId = new Map(first.threads.map(thread => [homeOrderKey(thread), thread]));
  const targets = assignments.map(assignment => ({ ...assignment, thread: byId.get(assignment.id)! }));
  const lockKeys = [...new Set([JSON.stringify([environmentId, threadId]), ...targets.map(target => JSON.stringify([target.thread.environmentId, target.thread.id]))])];
  if (lockKeys.some(key => held.has(key))) return result();
  for (const key of lockKeys) {
    const [targetEnvironmentId, targetThreadId] = JSON.parse(key) as [string, string];
    const current = row(targetEnvironmentId, targetThreadId, client, background);
    const sourceOrigin = current.environment?.focused ? client.origin : background.entries.get(current.environment?.key ?? '')?.origin ?? '';
    const pendingKey = JSON.stringify([sourceOrigin, targetEnvironmentId, targetThreadId]), pending = uncertainSet(client).get(pendingKey);
    if (!pending) continue;
    if (current.environment?.connected && reflected(pending, current.thread)) uncertainSet(client).delete(pendingKey);
    else return result('The previous request may have reached the server. Wait for the thread to refresh before trying again.', true);
  }
  for (const key of lockKeys) held.add(key);
  state.busy = true; client.revision++;
  const serial = ++state.serial, baseline = homeCreatePending(section, first.sections[section], movedId, direction, assignments, serial);
  let published = false;
  let inFlight: { target: typeof targets[number]; origin: string; sent: boolean; acknowledged: boolean; refused: boolean } | null = null;
  try {
    const base = letGoAware(mobileNative(nativeInput));
    const environmentIds = [...new Set([environmentId, ...targets.map(target => str(target.thread.environmentId))])];
    const endpoints = environmentIds.map(id => {
      const source = row(id, str(first.threads.find(thread => thread.environmentId === id)?.id), client, background).environment;
      if (!source?.connected) throw new ClientError('Reconnect before moving threads.');
      const entry = source.focused ? undefined : background.entries.get(source.key);
      return { id, focused: source.focused, key: source.key, entry, generation: source.focused ? client.generation : entry!.generation,
        origin: source.focused ? client.origin : entry!.origin,
        native: source.focused ? base : EnvironmentFleet.native(base, source.key) };
    });
    const check = () => {
      if (!currentSurface()) throw new ClientError('The thread action changed.', 'superseded');
      for (const endpoint of endpoints) {
        const valid = endpoint.focused ? client.environmentId === endpoint.id && client.origin === endpoint.origin && client.generation === endpoint.generation && client.ready
          : background.entries.get(endpoint.key) === endpoint.entry && endpoint.entry!.origin === endpoint.origin && endpoint.entry!.environmentId === endpoint.id
            && endpoint.entry!.generation === endpoint.generation && endpoint.entry!.phase === 'connected' && endpoint.entry!.synchronized === endpoint.generation;
        if (!valid) throw new ClientError('The connection changed. Refresh the thread order.', 'stale');
      }
      const snapshot = mobileHomeOrder(client, mobileHomeSources(client, background), now);
      if (section === 'active' && snapshot.workingEnabled) throw new ClientError('');
      if (published ? state.pending?.serial !== serial : homeReconcilePending(baseline, snapshot.sections[section]) === null)
        throw new ClientError('The thread order changed. Try the move again.');
      if (!row(environmentId, threadId, client, background).allowed) throw new ClientError('This connection cannot change threads.');
      for (const target of targets) {
        const current = row(str(target.thread.environmentId), str(target.thread.id), client, background);
        if (!current.allowed || !current.thread) throw new ClientError('This connection cannot change threads.');
        if (!snapshot.reorderable[section].has(str(target.thread.environmentId)))
          throw new ClientError("This environment's server does not support reordering these threads. Update the server to arrange them.");
      }
    };
    const call = async (endpoint: typeof endpoints[number], request: Obj) => {
      check();
      const writing = request.method === 'orchestration.dispatchCommand';
      if (writing && inFlight) inFlight.sent = true;
      const response = await bridgeReply(endpoint.native, { ...request, generation: endpoint.generation });
      if (writing && inFlight) { inFlight.acknowledged = response.ok && response.generation === endpoint.generation; inFlight.refused = !response.ok && response.generation === endpoint.generation && response.error?.uncertain !== true; }
      if (response.generation !== endpoint.generation) throw new ClientError('The connection changed. Refresh the thread order.', 'stale', writing && !response.ok);
      if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind, response.error!.uncertain);
      check(); return response.value;
    };
    // Every assignment must have a current grant before a rewrite sends its first command.
    for (const endpoint of endpoints) {
      const session = await call(endpoint, { op: 'http', path: '/api/auth/session' });
      if (!mobileSessionGrants(obj(session), 'orchestration:operate')) throw new ClientError('This connection cannot change threads.');
    }
    check();
    try { await bridgeReply(base, { op: 'mobileHomeHaptic', kind: 'light' }); } catch (error) { if (letGo(error)) throw error; }
    check(); state.pending = baseline; published = true; client.revision++;
    for (const target of targets) {
      const endpoint = endpoints.find(value => value.id === target.thread.environmentId)!;
      const ids = await call(endpoint, { op: 'ids', count: 1 });
      if (!Array.isArray(ids) || ids.length !== 1 || typeof ids[0] !== 'string' || !ids[0]) throw new ClientError('Could not allocate request identifiers.');
      check();
      inFlight = { target, origin: endpoint.origin, sent: false, acknowledged: false, refused: false };
      await call(endpoint, { op: 'request', method: 'orchestration.dispatchCommand', payload: {
        type: section === 'pinned' ? 'thread.pin.reorder' : 'thread.active.reorder', threadId: str(target.thread.id), orderKey: target.orderKey, commandId: ids[0] } });
      inFlight = null;
    }
    if (state.pending?.serial === serial) state.pending = { ...state.pending, commandsComplete: true };
    mobileHomeOrder(client, mobileHomeSources(client, background), now);
    return result();
  } catch (error) {
    const uncertain = !!inFlight?.sent && !inFlight.acknowledged && !inFlight.refused;
    if (uncertain && inFlight) state.unknown = { environmentId: str(inFlight.target.thread.environmentId), threadId: str(inFlight.target.thread.id),
      origin: inFlight.origin, section, orderKey: inFlight.target.orderKey };
    if (state.pending?.serial === serial) state.pending = null;
    if (letGo(error)) throw error;
    return currentSurface() ? result((error instanceof Error ? error.message : 'The thread could not be moved.')
      + (uncertain ? ' The request may have reached the server. Wait for the thread order to refresh before trying again.' : ''), uncertain) : result();
  } finally {
    state.busy = false;
    for (const key of lockKeys) held.delete(key);
    client.revision++;
  }
}
