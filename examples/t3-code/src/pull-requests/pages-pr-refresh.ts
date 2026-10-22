// Keeping the pull request surfaces current (MIT reference, see LICENSE-T3, T3 Code
// 1e2ecbd975):
// - packages/client-runtime/src/state/pullRequests.ts createPullRequestRefreshAtomFamily: one
//   `pullRequests.subscribeRefreshes` stream (payload `{}`) per environment, held while any pull
//   request surface reads; each value is a server-announced change (a mutation, a turn, another
//   client's `pullRequests.invalidate`), and the detail, the activity and the list read again (their
//   `refreshTrigger`). The server sends its current epoch on subscribe only when one exists
//   (`changes` filtered to > 0), so a first value cannot be told from a change; a read in flight
//   when it lands absorbs it (the readers note the epoch when their read completes).
// - apps/web/src/hooks/useLiveRefresh.ts: when each view last read (kept outside the panel, so a
//   view reopened later is an arrival, not a first read), and the reader's last interaction (the
//   window's, app.contract `interacted`, to the second); the first after six idle minutes resumes
//   the views as an arrival (`watchInteraction`).
// - pullRequestDetail.logic.ts read/writePullRequestDetailSnapshot (74541bab17): the snapshots live in
//   the app's versioned preference file (t3-code.json, `prDetailSnapshots`), bounded to the 20 most
//   recently written.
// The stream rides T3Client's transport (one dispatch line in client.ts drain, as shell-vcs.ts);
// the newest subscription since the latest subscribe is the one counted (shell-vcs.ts explains
// the request-id order). A new connection carries no streams, so the next read subscribes again.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { subscriptionSerial } from '../details/shell-vcs';
import { LIVE_REFRESH_IDLE_AFTER_MS, LIVE_REFRESH_INTERVAL_MS, shouldRefreshOnArrival, shouldRefreshOnInterval, type SnapshotStorage } from './pages-pr-logic';
import { letGo } from '../client/let-go';

export const PR_REFRESH_KEY = 'pull-request-refreshes';

type RefreshState = {
  generation: number; id: string; floor: number; maxSeen: number; tried: boolean;
  /** The last value the current stream sent. */
  last: number | null; epoch: number; subscribes: number; unsubscribes: number;
  /** Which surfaces read pull requests now: "detail", "list". */
  surfaces: Set<string>;
};
const states = new WeakMap<object, RefreshState>();
function refreshState(client: object): RefreshState {
  let state = states.get(client);
  if (!state) { state = { generation: -1, id: '', floor: 0, maxSeen: 0, tried: false, last: null, epoch: 0, subscribes: 0, unsubscribes: 0, surfaces: new Set() }; states.set(client, state); }
  return state;
}

/** How many server-announced changes this client has seen; a surface reads again when it moves. */
export function pullRequestRefreshEpoch(client: object): number { return refreshState(client).epoch; }
/** The stream's bookkeeping, for tests and the trace record. */
export function pullRequestRefreshStats(client: object) { const state = refreshState(client); return { subscribed: !!state.id, subscribes: state.subscribes, unsubscribes: state.unsubscribes, epoch: state.epoch }; }

/** client.ts drain: one `pull-request-refreshes` entry. */
export function prRefreshEvent(client: object, entry: Obj): void {
  const state = refreshState(client);
  const id = str(entry.subscriptionId), serial = subscriptionSerial(id);
  state.maxSeen = Math.max(state.maxSeen, serial);
  if (serial <= state.floor || (state.id && serial < subscriptionSerial(state.id))) return;
  if (id !== state.id) { state.id = id; state.last = null; }
  const value = entry.value, item = obj(value);
  if (item._retryDue || item._streamEnded) { state.id = ''; state.tried = false; state.last = null; return; }
  if (item._transportError || typeof value !== 'number') return;
  if (value !== state.last) { state.last = value; state.epoch++; }
}

/**
 * A surface opened or closed (`surface` is "detail" or "list"). While any is open the stream is
 * held; when the last closes it is let go, as an atom family entry nobody reads goes idle. The first
 * to open is useLiveRefresh's first watcher, which counts as an interaction at `now` (watchInteraction).
 */
export async function holdPullRequestRefreshes(client: T3Client, native: Native, surface: string, open: boolean, now = 0): Promise<void> {
  const state = refreshState(client);
  if (open && !state.surfaces.size && now > 0) noteInteraction(client, now, false);
  if (open) state.surfaces.add(surface); else state.surfaces.delete(surface);
  if (state.generation !== client.generation) { state.generation = client.generation; state.id = ''; state.tried = false; state.last = null; }
  if (!state.surfaces.size) {
    if (!state.id && !state.tried) return;
    state.id = ''; state.tried = false; state.last = null; state.floor = state.maxSeen; state.unsubscribes++;
    try { await client.restAccess(native).call({ op: 'unsubscribe', key: PR_REFRESH_KEY }); } catch (error) { if (letGo(error)) throw error; }
    return;
  }
  if (state.id || state.tried) return;
  state.tried = true; state.floor = state.maxSeen;
  try {
    const reply = await client.restAccess(native).call({ op: 'subscribe', key: PR_REFRESH_KEY, method: 'pullRequests.subscribeRefreshes', payload: {} });
    state.subscribes++;
    const serial = subscriptionSerial(str(reply.id));
    state.maxSeen = Math.max(state.maxSeen, serial);
    if (serial > state.floor && (!state.id || serial > subscriptionSerial(state.id))) state.id = str(reply.id);
  } catch (error) { state.tried = false; if (letGo(error)) throw error; }
}

// ── useLiveRefresh's module state ───────────────────────────────────────────

const lastRefreshed = new WeakMap<object, Map<string, number>>();
/** lastRefreshedAtByView: when a view last read, by the view's key. */
export function viewRefreshedAt(client: object, view: string): number | undefined { return lastRefreshed.get(client)?.get(view); }
export function noteViewRefreshed(client: object, view: string, now: number): void {
  let views = lastRefreshed.get(client);
  if (!views) { views = new Map(); lastRefreshed.set(client, views); }
  views.set(view, now);
}

const interactions = new WeakMap<object, { at: number; resumes: number }>();
/**
 * The window's interaction (app.ts, from app.contract `interacted`), on the views' clock. `resumed`: the first after six
 * idle minutes, which useLiveRefresh's watchers take as an arrival.
 */
export function noteInteraction(client: object, at: number, resumed: boolean): void {
  const entry = interactions.get(client) ?? { at: 0, resumes: 0 };
  if (at > entry.at) entry.at = at;
  if (resumed) entry.resumes += 1;
  interactions.set(client, entry);
}
/** How many times the reader came back from six idle minutes: a view that sees it change has an arrival. */
export function interactionResumes(client: object): number { return interactions.get(client)?.resumes ?? 0; }
/** When the reader last pointed, typed or scrolled; before any, the first view's ask, as the hook's first watcher notes it. */
function lastInteraction(client: object, now: number): number {
  const entry = interactions.get(client);
  if (entry && entry.at > 0) return entry.at;
  noteInteraction(client, now, false);
  return now;
}

/**
 * useLiveRefresh's onArrival and onInterval for one view (the detail panel's, the Pull Requests list's): whether
 * it reads again now, noted as read when it does. `arrival`: the view shown again (opened, the window shown or
 * focused); otherwise the clock's minute tick asks whether the five-minute interval is due. Both wait out the
 * 10 s minimum and a hidden window; both stop once the reader has been idle for six minutes.
 */
export function liveRefreshDue(client: object, view: string, input: { visible: boolean; now: number; arrival: boolean }): boolean {
  const { visible, now, arrival } = input, lastRefreshedAt = viewRefreshedAt(client, view);
  if (!liveRefreshAsked(client, view, input)) return false;
  const interacted = lastInteraction(client, now);
  const read = arrival ? now - interacted < LIVE_REFRESH_IDLE_AFTER_MS && shouldRefreshOnArrival({ visible, now, lastRefreshedAt })
    : shouldRefreshOnInterval({ visible, now, lastRefreshedAt: lastRefreshedAt!, lastInteractedAt: interacted });
  if (read) noteViewRefreshed(client, view, now);
  return read;
}

/** Whether a view has anything to ask liveRefreshDue (an arrival, or the five-minute interval come round), without waiting. */
export function liveRefreshAsked(client: object, view: string, input: { now: number; arrival: boolean }): boolean {
  const lastRefreshedAt = viewRefreshedAt(client, view);
  return input.arrival || (lastRefreshedAt !== undefined && input.now - lastRefreshedAt >= LIVE_REFRESH_INTERVAL_MS);
}

// ── Snapshot storage (t3-code.json) ─────────────────────────────────────────

const SNAPSHOT_LIMIT = 20;
/** `prDetailSnapshots` on the preference record, as Storage's getItem/setItem; the most recently written are kept. */
export function snapshotStorage(owner: { local: object }): SnapshotStorage & { changed: boolean } {
  const local = owner.local as { prDetailSnapshots?: Record<string, string> };
  const store = { changed: false,
    getItem: (key: string) => { const held = local.prDetailSnapshots?.[key]; return typeof held === 'string' ? held : null; },
    setItem: (key: string, value: string) => {
      const held = { ...(local.prDetailSnapshots ?? {}) };
      // The key order is the write order, oldest first: an unchanged write still counts as the most recent.
      if (held[key] === value && Object.keys(held).at(-1) === key) return;
      delete held[key]; held[key] = value;
      const keys = Object.keys(held);
      for (const old of keys.slice(0, Math.max(0, keys.length - SNAPSHOT_LIMIT))) delete held[old];
      local.prDetailSnapshots = held; store.changed = true;
    } };
  return store;
}
/** load(): carry the saved snapshots into a fresh preference record (strings only, the 20 most recent). */
export function adoptPrSnapshots(next: object, saved: Obj): void {
  const held = Object.entries(obj(saved.prDetailSnapshots)).filter((entry): entry is [string, string] => entry[0].startsWith('t3.pullRequests.detail:') && typeof entry[1] === 'string' && entry[1].length <= 200_000);
  (next as { prDetailSnapshots?: Record<string, string> }).prDetailSnapshots = Object.fromEntries(held.slice(-SNAPSHOT_LIMIT));
}
