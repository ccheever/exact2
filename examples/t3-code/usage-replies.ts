// Detached requests on background environments (lane usage-pooled-view). The Usage page reads and
// refreshes every selected environment at once, as the reference's per-environment atoms do
// (client-runtime state/usage.ts refreshUsage, UsagePage.tsx refreshLimits, UsageLimits.tsx
// useResetCredit). The native data executor answers one request at a time, so no answer waits on
// another environment's reply: like composer-replies.ts for the focused connection, the request
// carries a `deliver` key (T3Transport.swift), the background transport files the reply in its own
// inbox and wakes `t3.fleet`, and EnvironmentFleet's drain hands it here (`usageFleetEvent`). A
// connection change or a stopped environment settles the waiter as interrupted (`settleFleetWaiters`).
// This module imports no value from settings-b-fleet.ts, which imports it.
import { bridgeReply, ClientError, type Native } from './protocol';
import { obj, str, type Obj } from './domain';
import type { DetachedReply } from './composer-replies';

type FleetTarget = { readonly key: string; readonly generation: number };
type Waiter = { fleet: string; generation: number; resolve: (reply: DetachedReply) => void };
const KEY = 'usage-reply:';
const waiters = new Map<string, Waiter>();
let serial = 0;
let changed: (() => void) | null = null;
let seenRevision = -1;

/** The Usage page's redraw while it is open: a reply or a background change bumps the focused client's revision. */
export function onUsageFleetChange(listener: (() => void) | null): void { changed = listener; }

/** Sends `method` on a background environment's transport and returns at once with a promise of its reply. */
export async function startFleetDetached(native: Native, target: FleetTarget, method: string, payload: Obj): Promise<{ reply: Promise<DetachedReply> }> {
  let resolveReply: (reply: DetachedReply) => void = () => {};
  const reply = new Promise<DetachedReply>(resolve => { resolveReply = resolve; });
  const remote: Native = { available: native.available, watch: topic => native.watch(topic), later: request => native.later({ ...obj(request), fleet: target.key }) };
  let key = '';
  try {
    // A fresh id per request (the transport's `ids`): a reply filed for a request of a reloaded module never matches.
    const ids = await bridgeReply(native, { op: 'ids', count: 1 });
    const id = Array.isArray(ids.value) && typeof ids.value[0] === 'string' ? ids.value[0] : '';
    key = `${KEY}${id || `${++serial}`}`;
    waiters.set(key, { fleet: target.key, generation: target.generation, resolve: resolveReply });
    const response = await bridgeReply(remote, { op: 'request', method, payload, deliver: key, timeout: 300, generation: target.generation });
    if (!response.ok) settle(key, { ok: false, error: new ClientError(response.error!.message, response.error!.kind, response.error!.uncertain), interrupted: false });
    else if (response.generation !== target.generation) settle(key, { ok: false, error: new ClientError('The connection changed. Refresh before continuing.', 'stale'), interrupted: true });
  } catch (error) {
    const failure = error instanceof ClientError ? error : new ClientError(error instanceof Error ? error.message : String(error));
    if (key) settle(key, { ok: false, error: failure, interrupted: failure.kind === 'superseded' || failure.kind === 'stale' });
    else resolveReply({ ok: false, error: failure, interrupted: failure.kind === 'superseded' || failure.kind === 'stale' });
  }
  return { reply };
}
function settle(key: string, reply: DetachedReply): void {
  const waiter = waiters.get(key);
  if (!waiter) return;
  waiters.delete(key);
  waiter.resolve(reply);
}

/** EnvironmentFleet's drain: a `usage-reply:` inbox entry settles its waiter. Other entries are not ours. */
export function usageFleetEvent(entry: FleetTarget, event: Obj): boolean {
  const key = str(event.key);
  if (!key.startsWith(KEY)) return false;
  const waiter = waiters.get(key);
  if (!waiter || waiter.fleet !== entry.key) return true;
  const item = obj(event.value);
  if (item._reply !== undefined) settle(key, { ok: true, value: obj(item._reply) });
  else {
    const error = obj(item._replyError), kind = str(error.kind, 'RPC');
    settle(key, { ok: false, error: new ClientError(str(error.message, 'The server request failed.'), kind, error.uncertain === true, { reason: str(error.reason), detail: str(error.detail) }),
      interrupted: kind === 'Interrupt' });
  }
  return true;
}

/** EnvironmentFleet.sync's end: a background change redraws the open Usage page (its revision is the focused client's). */
export function usageFleetSynced(revision: number): void {
  if (revision === seenRevision) return;
  seenRevision = revision;
  changed?.();
}

/** Waiters whose environment stopped or reconnected: their replies never come, so each ends as interrupted. */
export function settleFleetWaiters(live: (fleet: string, generation: number) => boolean): void {
  for (const [key, waiter] of [...waiters]) {
    if (live(waiter.fleet, waiter.generation)) continue;
    settle(key, { ok: false, error: new ClientError('The environment disconnected.', 'Disconnected'), interrupted: true });
  }
}
