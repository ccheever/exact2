// Keeping the pull request surfaces current (lane "pages"; MIT reference, see LICENSE-T3, T3 Code
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
//   activity reporter's, T3ActivityReporter.swift, asked through `activityLastInteraction`).
// - pullRequestDetail.logic.ts read/writePullRequestDetailSnapshot: the snapshots live in the
//   app's versioned preference file (t3-code.json, `prDetailSnapshots`), bounded to the 24 newest.
// The stream rides T3Client's transport (one dispatch line in client.ts drain, as shell-vcs.ts);
// the newest subscription since the latest subscribe is the one counted (shell-vcs.ts explains
// the request-id order). A new connection carries no streams, so the next read subscribes again.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import { subscriptionSerial } from './shell-vcs';
import { isEpoch } from './r8-pointer-clock';
import type { SnapshotStorage } from './pages-pr-logic';
import { letGo } from './let-go';

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
 * held; when the last closes it is let go, as an atom family entry nobody reads goes idle.
 */
export async function holdPullRequestRefreshes(client: T3Client, native: Native, surface: string, open: boolean): Promise<void> {
  const state = refreshState(client);
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

/**
 * When the reader last pointed, typed or scrolled, from the activity reporter, or null when it
 * cannot be compared with `now`: the window's time is an instant only where the host told the
 * date (agent mode counts from zero), so the idle rule then stands aside rather than guess.
 */
export async function lastInteraction(native: Native, now: number): Promise<number | null> {
  if (!isEpoch(now)) return null;
  const reply = await bridgeReply(native, { op: 'activityLastInteraction' }).catch((error: unknown) => { if (letGo(error)) throw error; return null; });
  const at = Number(obj(reply?.value).at);
  return reply?.ok && Number.isFinite(at) && at > 0 ? at : null;
}

// ── Snapshot storage (t3-code.json) ─────────────────────────────────────────

const SNAPSHOT_LIMIT = 24;
/** `prDetailSnapshots` on the preference record, as Storage's getItem/setItem; the newest are kept. */
export function snapshotStorage(owner: { local: object }): SnapshotStorage & { changed: boolean } {
  const local = owner.local as { prDetailSnapshots?: Record<string, string> };
  const store = { changed: false,
    getItem: (key: string) => { const held = local.prDetailSnapshots?.[key]; return typeof held === 'string' ? held : null; },
    setItem: (key: string, value: string) => {
      const held = { ...(local.prDetailSnapshots ?? {}) };
      if (held[key] === value) return;
      delete held[key]; held[key] = value;
      const keys = Object.keys(held);
      for (const old of keys.slice(0, Math.max(0, keys.length - SNAPSHOT_LIMIT))) delete held[old];
      local.prDetailSnapshots = held; store.changed = true;
    } };
  return store;
}
/** load(): carry the saved snapshots into a fresh preference record (strings only, the newest 24). */
export function adoptPrSnapshots(next: object, saved: Obj): void {
  const held = Object.entries(obj(saved.prDetailSnapshots)).filter((entry): entry is [string, string] => entry[0].startsWith('t3.pullRequests.detail:') && typeof entry[1] === 'string' && entry[1].length <= 200_000);
  (next as { prDetailSnapshots?: Record<string, string> }).prDetailSnapshots = Object.fromEntries(held.slice(-SNAPSHOT_LIMIT));
}
