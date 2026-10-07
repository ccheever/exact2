// Running threads stay live (20261005-local-primary-environment item 8), after T3 Code (MIT, see
// LICENSE-T3; reference 1e2ecbd975) apps/web/src/state/threads.ts createRunningThreadKeepAliveAtom
// and RunningThreadKeepAlive: every running thread of every enabled environment keeps its detail
// stream open; a thread the shell reports stopped stays until its own detail is live and shows the
// stop (or cannot progress: deleted or failed); an environment that leaves drops its streams.
//
// The clone's environments are the focused one (T3Client, whose open thread already has its own
// stream) and the fleet's background ones (settings-b-fleet.ts), so each keeps its streams on its
// own transport for its generation, under the keys `keep:<thread id>`. Opening a kept thread, in the
// focused environment or by focusing a background one, starts from the kept detail at once
// (`keptThread`, `handoffKeptThread`) and resumes its stream after it, so nothing replays. The
// primary's lifecycle stream (local-lifecycle.ts) rides the same fleet pass.
import { arr, obj, str, num, applyThread, threadSnapshot, type Obj, type Shell, type ThreadState } from './domain';
import type { Native } from './protocol';
import type { FleetEntry } from './settings-b-fleet';
import { lifecycleEvent, lifecycleFleetEvent, lifecycleFleetPass, watchLifecycle } from './local-lifecycle';
import { letGo } from './let-go';

export const KEEP_PREFIX = 'keep:';
/** Streams kept per environment: the transport holds 16, and the app's own take about eight. */
export const KEEP_LIMIT = 6;

// ── The rule (createRunningThreadKeepAliveAtom) ─────────────────────────────
export type KeptThreads = ReadonlyMap<string, ReadonlySet<string>>;
/** A kept detail stream's state: its thread, whether it is in sync, and whether it can still progress. */
export type KeptDetail = { status: 'loading' | 'live' | 'deleted'; thread: ThreadState | null; error: string };

export const isRunning = (status: string) => status === 'preparing' || status === 'starting' || status === 'running';

/** isDetailDone: in sync and settled, or unable to progress (deleted or failed). A detail not yet loaded waits. */
export function isDetailDone(detail: KeptDetail | undefined): boolean {
  if (!detail) return false;
  if (detail.status === 'deleted' || detail.error) return true;
  return detail.status === 'live' && !arr(detail.thread?.projection.runs).some(run => isRunning(str(run.status)));
}

const sameSet = (a: ReadonlySet<string> | undefined, b: ReadonlySet<string>) => !!a && a.size === b.size && [...b].every(id => a.has(id));

/**
 * One pass of the keep-alive: for each listed environment, its running threads plus every thread
 * kept before whose own detail is not done yet. An environment that is not listed is dropped. The
 * previous map comes back unchanged when nothing started or stopped.
 */
export function runningThreadKeepAlive(input: {
  environmentIds: readonly string[];
  threads: (environmentId: string) => readonly { id: string; status: string }[];
  detail: (environmentId: string, threadId: string) => KeptDetail | undefined;
  previous?: KeptThreads;
}): KeptThreads {
  const kept = new Map<string, ReadonlySet<string>>();
  for (const environmentId of input.environmentIds) {
    const ids = new Set(input.threads(environmentId).flatMap(thread => isRunning(thread.status) ? [thread.id] : []));
    for (const threadId of input.previous?.get(environmentId) ?? []) {
      if (ids.has(threadId) || isDetailDone(input.detail(environmentId, threadId))) continue;
      ids.add(threadId);
    }
    kept.set(environmentId, ids);
  }
  const previous = input.previous;
  if (previous && previous.size === kept.size && [...kept].every(([environmentId, ids]) => sameSet(previous.get(environmentId), ids))) return previous;
  return kept;
}

// ── One environment's streams ─────────────────────────────────────────────
type Owner = { generation: number; kept: KeptThreads; subscriptions: Map<string, string>; details: Map<string, KeptDetail> };
const owners = new WeakMap<object, Owner>();
function ownerOf(owner: object, generation: number): Owner {
  let value = owners.get(owner);
  if (!value || value.generation !== generation) owners.set(owner, value = { generation, kept: new Map(), subscriptions: new Map(), details: new Map() });
  return value;
}

/** The stream items of one kept thread (`orchestration.subscribeThread` with a bounded snapshot first). */
function applyKept(detail: KeptDetail, item: Obj): KeptDetail {
  if (item._transportError) return { ...detail, error: str(obj(item._transportError).message, 'The live stream ended.') };
  if (item._streamEnded) return { ...detail, error: 'The live stream ended.' };
  if (item.kind === 'synchronized') return { ...detail, status: detail.thread ? 'live' : detail.status };
  try {
    if (item.kind === 'snapshot') return { status: 'live', thread: threadSnapshot(item), error: '' };
    return detail.thread ? { ...detail, thread: applyThread(detail.thread, item) } : detail;
  } catch (error) { return { ...detail, error: error instanceof Error ? error.message : 'The thread stream failed.' }; }
}

/** An inbox entry of a `keep:` stream: true when it was one (applied when it belongs to this generation's subscription). */
function event(owner: Owner | undefined, entry: Obj): boolean {
  const key = str(entry.key);
  if (!key.startsWith(KEEP_PREFIX)) return false;
  const threadId = key.slice(KEEP_PREFIX.length);
  if (!owner || num(entry.generation, -1) !== owner.generation || owner.subscriptions.get(threadId) !== str(entry.subscriptionId)) return true;
  const item = obj(entry.value);
  if (item._retryDue) { owner.subscriptions.delete(threadId); return true; }
  owner.details.set(threadId, applyKept(owner.details.get(threadId) ?? { status: 'loading', thread: null, error: '' }, item));
  return true;
}

/**
 * Reconciles one environment's kept streams: subscribe each newly kept thread (except `open`, which
 * has its own stream), unsubscribe each released one. `call` is the environment's own transport.
 */
async function pass(owner: Owner, environmentId: string, shell: Shell, open: string, call: (request: Obj) => Promise<Obj>): Promise<boolean> {
  const threads = shell.threads.map(thread => ({ id: str(thread.id), status: str(thread.status) }));
  const present = new Set(threads.map(thread => thread.id));
  for (const [threadId, detail] of owner.details) if (!present.has(threadId) && detail.status !== 'deleted') owner.details.set(threadId, { ...detail, status: 'deleted' });
  const next = runningThreadKeepAlive({ environmentIds: [environmentId], threads: () => threads, detail: (_environment, threadId) => owner.details.get(threadId), previous: owner.kept });
  const changed = next !== owner.kept;
  owner.kept = next;
  const wanted = [...(next.get(environmentId) ?? [])].filter(id => id !== open).slice(0, KEEP_LIMIT);
  for (const threadId of [...owner.subscriptions.keys()]) {
    if (wanted.includes(threadId)) continue;
    owner.subscriptions.delete(threadId); owner.details.delete(threadId);
    await call({ op: 'unsubscribe', key: `${KEEP_PREFIX}${threadId}` }).catch(() => ({}));
  }
  for (const threadId of wanted) {
    if (owner.subscriptions.has(threadId)) continue;
    try {
      // As the reference's thread state (client-runtime threads.ts): the bounded snapshot over HTTP first, then the
      // stream after its sequence. A stream from sequence 0 replays events onto no projection, so it never goes live.
      const kept = owner.details.get(threadId);
      const thread = kept?.status === 'live' && !kept.error && kept.thread ? kept.thread
        : threadSnapshot(await call({ op: 'http', path: `/api/orchestration/threads/${encodeURIComponent(threadId)}/bounded` }));
      owner.details.set(threadId, { status: 'live', thread, error: '' });
      const reply = await call({ op: 'subscribe', key: `${KEEP_PREFIX}${threadId}`, method: 'orchestration.subscribeThread', payload: { threadId, afterSequence: thread.sequence, acceptBoundedSnapshot: true } });
      owner.subscriptions.set(threadId, str(reply.id));
    } catch (error) { if (letGo(error)) throw error; }
  }
  return changed;
}

// ── The focused environment (T3Client) ─────────────────────────────────────
type Focused = { environmentId: string; origin: string; generation: number; ready: boolean; shell: Shell; threadId: string; thread: ThreadState | null;
  restAccess(native: Native): { call(request: Obj): Promise<Obj> } };

/** client.ts drain: `keep:` and lifecycle entries of the focused connection. */
export function keepAliveEvent(client: Focused, entry: Obj): boolean {
  if (lifecycleEvent(client, entry)) return true;
  return event(owners.get(client), entry);
}

/** After each refresh (app.ts): the focused environment's kept streams and the primary's lifecycle. */
export async function keepAlivePrepare(client: Focused, native: Native | null | undefined): Promise<void> {
  if (!native?.available || !client.ready || !client.environmentId) return;
  const call = (request: Obj) => client.restAccess(native).call(request);
  await watchLifecycle(client, call);
  await pass(ownerOf(client, client.generation), client.environmentId, client.shell, client.threadId, call);
}

/** The kept detail of a thread in the focused environment, to open it without a refetch (null: none live). */
export function keptThread(client: { generation: number }, threadId: string): ThreadState | null {
  const detail = owners.get(client)?.details.get(threadId);
  return detail && detail.status === 'live' && !detail.error && detail.thread ? detail.thread : null;
}

// ── Background environments (settings-b-fleet.ts) ──────────────────────────
export function keepAliveFleetEvent(entry: FleetEntry, value: Obj): boolean {
  if (lifecycleFleetEvent(entry, value)) return true;
  return event(owners.get(entry), value);
}
export async function keepAliveFleetPass(call: (request: Obj) => Promise<Obj>, entry: FleetEntry): Promise<void> {
  await lifecycleFleetPass(call, entry);
  await pass(ownerOf(entry, entry.generation), entry.environmentId, entry.shell, '', call);
}

// ── Opening a background environment's kept thread ────────────────────────
const handoffs = new WeakMap<object, { environmentId: string; threadId: string; thread: ThreadState }>();
/** focusFleetThread: carry the kept detail across the focus change. */
export function handoffKeptThread(client: object, entry: FleetEntry, threadId: string): void {
  const detail = owners.get(entry)?.details.get(threadId);
  if (detail && detail.status === 'live' && !detail.error && detail.thread) handoffs.set(client, { environmentId: entry.environmentId, threadId, thread: detail.thread });
}
/** The client moved to that environment: show the carried detail at once (true when it did). */
export function adoptHandoff(client: { environmentId: string; threadId: string; thread: ThreadState | null }): boolean {
  const handoff = handoffs.get(client);
  if (!handoff || handoff.environmentId !== client.environmentId || handoff.threadId !== client.threadId) return false;
  handoffs.delete(client);
  if (!client.thread) client.thread = handoff.thread;
  return true;
}
