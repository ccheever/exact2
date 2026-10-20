// @ref examples/t3-code/mobile/docs/1130.011-responsive-workspace.decision.md#navigation-and-data-ownership
// @ref examples/t3-code/mobile/docs/1130.009-mobile-settings.decision.md#offline-cache-storage
import type { T3Client } from './shared/client';
import { ClientError, bridgeReply, type Files, type Native } from './shared/protocol';
import { fleet, parseFleetThreadId, trimOrigin } from './shared/settings-b-fleet';
import { obj, str } from './shared/domain';
import { letGo, letGoAware } from './shared/let-go';
import { mobileCacheSync, mobileCacheAdoptThreadPresentation } from './mobile-client-cache-sync';
import { mobileCacheFleetDisplays, mobileCacheFleetSync } from './mobile-client-cache-fleet';
import { mobileCacheCatalogCurrent } from './mobile-client-cache-catalog';
import { mobileCacheReadRevision } from './mobile-client-cache';
import { mobileOutboxDriveSnapshot } from './mobile-outbox-drive';

const selections = new WeakMap<T3Client, number>();
const stamp = (client: T3Client) => JSON.stringify([client.environmentId, client.origin, client.generation,
  client.threadId, client.threadEpoch]);
const identity = (environmentId: string) => {
  const rows = fleet.saved.filter(row => row.environmentId === environmentId);
  return rows.length === 1 && str(rows[0]!.origin) && rows[0]!.enabled !== false
    ? JSON.stringify([environmentId, trimOrigin(str(rows[0]!.origin))]) : '';
};
const stale = () => new ClientError('The selected thread changed.', 'superseded');

/** Offline rows use the real native saved-environment selector. Cached content
 * remains display-only; reconnect and synchronization keep their normal owners. */
async function offlineSelection(client: T3Client, environmentId: string, threadId: string,
  native: Native, storage: Files, serial: number) {
  const catalog = identity(environmentId), clearRevision = mobileCacheReadRevision(environmentId);
  if (!catalog || !mobileCacheCatalogCurrent(client, fleet.saved, environmentId)) return { revision: client.revision, message: 'This thread is unavailable offline.' };
  let owner = stamp(client);
  const selected = () => selections.get(client) === serial
    && identity(environmentId) === catalog && mobileCacheReadRevision(environmentId) === clearRevision;
  const current = () => selected() && stamp(client) === owner;
  const check = () => { if (!current()) throw stale(); };
  const live = letGoAware(native);
  const guarded = (valid: () => boolean): Native => {
    const assert = () => { if (!valid()) throw stale(); };
    return { available: live.available, watch(topic) { assert(); live.watch(topic); }, async later(input) {
      assert(); const result = await live.later(input); assert(); return result;
    } };
  };
  const scoped = guarded(current);
  const selectLive = async () => {
    const origin = client.origin, generation = client.generation;
    const valid = () => selected() && client.environmentId === environmentId && client.origin === origin && client.generation === generation;
    // Shared selection owns its own thread/command epochs after this handoff.
    const result = await client.command('select-thread', threadId, '', 0, guarded(valid), storage);
    if (!valid()) throw stale();
    return result;
  };
  try {
    if (client.environmentId !== environmentId) {
      if (client.busy || client.pending || mobileOutboxDriveSnapshot(client, 0).busy)
        return { revision: client.revision, message: 'Wait for the current send before switching threads.' };
      await mobileCacheFleetSync(fleet, scoped, client); check();
      const cached = mobileCacheFleetDisplays(fleet, client).find(row => row.enabled && row.environmentId === environmentId);
      if (!cached?.shell.threads.some(row => row.id === threadId))
        return { revision: client.revision, message: 'This thread is unavailable offline.' };
      const generation = client.generation, threadEpoch = client.threadEpoch;
      const restoredThread = client.local.selections[environmentId]?.threadId || '';
      check();
      // Native publishes t3.status before answering this call. That notification
      // may already have adopted precisely this focus, but no other selection.
      const reply = await bridgeReply(guarded(selected), { op: 'mobileSelectSavedEnvironment', origin: cached.origin,
        environmentId, generation });
      if (!reply.ok) { check(); throw new ClientError(reply.error?.message || 'Could not open the saved environment.'); }
      const status = obj(reply.value);
      if (reply.generation !== generation + 1 || status.environmentId !== environmentId
        || trimOrigin(str(status.homeOrigin, str(status.origin))) !== trimOrigin(cached.origin)) throw stale();
      const observed = client.environmentId === environmentId && client.generation === reply.generation
        && client.threadEpoch === threadEpoch + 1 && client.threadId === restoredThread
        && trimOrigin(client.origin) === trimOrigin(str(status.origin));
      if (!current() && !observed) throw stale();
      // A same-generation status notification may be newer than the reply.
      if (!observed) client.adoptStatus(status, reply.generation);
      owner = stamp(client);
      // The old background transport is no longer a source of this focused row.
      for (const entry of fleet.entries.values()) if (entry.environmentId === environmentId) {
        fleet.forget(entry.key);
        await scoped.later({ op: 'fleetStop', fleet: entry.key }); check();
      }
    }
    // Reconcile a clear/forget/replacement before trusting a previously rendered row.
    await mobileCacheSync(client, scoped, () => fleet.saved); check();
    if (client.connection === 'connected') {
      // A reconnect won the race. Its live reader owns all subsequent adoption.
      return selectLive();
    }
    const thread = client.shell.threads.find(row => row.id === threadId);
    if (!thread) return { revision: client.revision, message: 'This thread is unavailable offline.' };
    if (client.threadId !== threadId) {
      client.threadId = threadId; client.thread = null; client.threadEpoch++;
      client.threadSubscription = ''; delete client.subscriptions.thread;
      client.threadLive = false; client.answers = {}; client.diffOpen = false;
      client.projectId = str(thread.projectId); client.revision++;
      mobileCacheAdoptThreadPresentation(client, null);
      owner = stamp(client);
    }
    await mobileCacheSync(client, scoped, () => fleet.saved); check();
    if (client.connection === 'connected') return selectLive();
    await client.persist(storage); check();
    return { revision: client.revision, message: '' };
  } catch (error) {
    if (letGo(error)) throw error;
    return { revision: client.revision, message: error instanceof Error ? error.message : 'Could not open the cached thread.' };
  }
}

/** The selection that is changing the focused environment. A focus forgets the
 * target's background entry first, so every route waits instead of superseding it. */
const switching = new WeakMap<T3Client, number>();
/** Selections another command interrupted: their count, whether the interrupting
 * command may still be busy, and the target whose opening was cut short. */
const interruptions = new WeakMap<T3Client, { count: number; waiting: boolean; target: string }>();
const unopened = (client: T3Client, environmentId: string, threadId: string) => client.connection === 'connected'
  && client.environmentId === environmentId && client.threadId === threadId && client.threadSubscription !== threadId
  && client.shell.threads.some(row => row.id === threadId);

/** The route target's own selection facts (app.contract selectRouteThread). They
 * never read the previous thread's liveness or reply, and nothing a same-environment
 * selection changes is in the key, so a refusal alerts once and does not repeat. */
export function mobileThreadSelectionFacts(client: T3Client, environmentId: string, threadId: string, route: string) {
  const focused = environmentId === client.environmentId;
  const background = focused ? undefined : [...fleet.entries.values()].find(row => row.environmentId === environmentId);
  const phase = focused ? client.connection : background?.phase ?? '';
  const live = phase === 'connected', transient = phase === 'connecting' || phase === 'reconnecting';
  const cached = !!identity(environmentId) && mobileCacheCatalogCurrent(client, fleet.saved, environmentId);
  // offlineSelection refuses a switch for these; their end is a new attempt.
  const blocked = !focused && !live && (client.busy || !!client.pending || mobileOutboxDriveSnapshot(client, 0).busy);
  const retry = interruptions.get(client) ?? { count: 0, waiting: false, target: '' };
  const target = live ? !focused || (client.configLive && client.shellLive) : transient ? cached : true;
  return { selectionRoute: route,
    selectionReady: !!environmentId && !!threadId && target && !switching.has(client) && !(retry.waiting && client.busy),
    selectionReopen: retry.target === JSON.stringify([environmentId, threadId]) && unopened(client, environmentId, threadId),
    selectionKey: JSON.stringify([environmentId, focused, live, phase, cached, blocked,
      focused ? client.generation : background?.generation ?? -1, retry.count]) };
}

/** A route projection can still name the background connection after a completed
 * focus. Resolve against the live client before looking up that removed entry. */
export async function mobileThreadSelection(client: T3Client, id: string, native: Native, storage: Files) {
  const serial = (selections.get(client) ?? 0) + 1; selections.set(client, serial);
  const target = parseFleetThreadId(id), environmentId = target?.environmentId ?? client.environmentId;
  const threadId = target?.threadId ?? id, retry = interruptions.get(client);
  if (retry) retry.waiting = false;
  if (environmentId !== client.environmentId) switching.set(client, serial);
  let requests = 0;
  const counted: Native = { available: native.available, watch: topic => native.watch(topic),
    later: request => { requests++; return native.later(request); } };
  try {
    const result = await select(client, id, environmentId, threadId, counted, storage, serial);
    // The shared command answers '' when a newer command superseded it, even after
    // it set threadId. Only the newest selection decides whether to try again.
    if (selections.get(client) === serial && !result.message) {
      const previous = interruptions.get(client) ?? { count: 0, waiting: false, target: '' };
      if (client.environmentId !== environmentId || client.threadId !== threadId
        || (requests > 0 || client.busy) && unopened(client, environmentId, threadId)) {
        interruptions.set(client, { count: previous.count + 1, waiting: true, target: JSON.stringify([environmentId, threadId]) });
        client.revision++;
      } else interruptions.set(client, { ...previous, target: '' });
    }
    return result;
  } finally { if (switching.get(client) === serial) switching.delete(client); }
}
async function select(client: T3Client, id: string, environmentId: string, threadId: string, native: Native,
  storage: Files, serial: number) {
  const target = parseFleetThreadId(id);
  const background = target && target.environmentId !== client.environmentId
    ? [...fleet.entries.values()].find(row => row.environmentId === target.environmentId) : null;
  if (environmentId === client.environmentId ? client.connection !== 'connected'
    : background?.phase !== 'connected' && !!identity(environmentId))
    return offlineSelection(client, environmentId, threadId, native, storage, serial);
  // Focus restores selection before its shell arrives; do not replace it with a draft.
  if (target?.environmentId === client.environmentId && target.threadId === client.threadId)
    return { revision: client.revision, message: '' };
  return client.command('select-thread', target?.environmentId === client.environmentId ? target.threadId : id,
    '', 0, native, storage);
}
