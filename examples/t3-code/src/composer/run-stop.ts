// A Stop the agent has not finished, and Force stop for one that ignores it. Adapted from T3 Code 2359643cff (MIT; see
// LICENSE-T3): client-runtime state/threadExecution.ts (presentRunStop), chat/ThreadActivityBanners.tsx
// (runStopNowItem), chat/useThreadLifecycle.tsx and chat/useForceStopThread.ts.
import { arr, obj, str, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { pushToast } from '../shell/toast';

const INTERRUPTIBLE_RUN_STATUSES = new Set(['preparing', 'starting', 'running']);

/** `unresponsive`: the provider did not confirm the stop in time, so Force stop is offered. `background`: the run had
 *  already ended; the Stop targeted its background work. */
export type RunStop = { state: 'stopping' | 'unresponsive'; runId: string; requestedAt: string; background: boolean };

/**
 * presentRunStop: a Stop the provider has not finished yet. It stays `stopping` while the agent winds down, since some
 * providers confirm the request before the turn actually ends. On a run that already ended, only an unanswered Stop of
 * its background work counts.
 */
export function presentRunStop(projection: Obj): RunStop | null {
  let active: Obj | null = null, latest: Obj | null = null;
  for (const candidate of arr(projection.runs)) {
    const ordinal = Number(candidate.ordinal);
    // Queued follow-ups never ran, so they hold no Stop of their own.
    if (candidate.status !== 'queued' && ordinal > Number(latest?.ordinal ?? -1)) latest = candidate;
    if (INTERRUPTIBLE_RUN_STATUSES.has(str(candidate.status)) && ordinal > Number(active?.ordinal ?? -1)) active = candidate;
  }
  const run = active ?? latest;
  if (run === null) return null;
  let request: Obj | undefined;
  for (const item of arr(projection.turnItems)) {
    if (item.runId !== run.id) continue;
    if (item.type === 'run_interrupt_result') return null;
    if (item.type === 'run_interrupt_request') request = item;
  }
  const unresponsive = typeof request?.unresponsiveAt === 'string';
  if (request === undefined || (active === null && !unresponsive)) return null;
  return { state: unresponsive ? 'unresponsive' : 'stopping', runId: str(run.id), requestedAt: str(request.startedAt) || str(request.updatedAt), background: active === null };
}

/** useThreadLifecycle: the open thread's Stop, a background one only while that work is still pending. */
export function threadRunStop(client: T3Client): RunStop | null {
  const stop = client.threadId ? presentRunStop(client.projection) : null;
  if (!stop?.background) return stop;
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  return arr(shell?.pendingBackgroundTasks).length > 0 ? stop : null;
}

/** useForceStopThread's canForceStop: the session may operate threads and the server offers threadForceStop. */
export function canForceStop(client: T3Client): boolean {
  return client.scopes.includes('orchestration:operate') && obj(obj(client.config.environment).capabilities).threadForceStop === true;
}

const stopping = new WeakMap<T3Client, string>();
/** The thread a Force stop is under way for ('' for none). */
export const forceStoppingThread = (client: T3Client) => stopping.get(client) ?? '';

/** forceStopThread: the run that ignored Stop goes with it, so a Force stop left open cannot hit a newer run. */
export async function forceStop(client: T3Client, native: Native, runId: string): Promise<void> {
  const threadId = client.threadId;
  if (!threadId || !canForceStop(client) || stopping.has(client)) return;
  stopping.set(client, threadId);
  try {
    const result = await client.forceStopThread(native, threadId, runId || undefined);
    const others = result.sharedWithThreadIds.length;
    if (result.outcome === 'releasing') pushToast(client, { kind: 'info', title: 'Turn stopped; the agent is still shutting down' });
    if (result.outcome === 'shared') pushToast(client, { kind: 'info', title: 'Turn stopped; the agent process kept running',
      description: `It is shared with ${others === 0 ? 'other' : others} working ${others === 1 ? 'thread' : 'threads'}. Stop those to free it.` });
  } catch (error) {
    pushToast(client, { kind: 'error', title: 'Could not force stop the agent', ...(error instanceof Error && error.message ? { description: error.message } : {}) });
  } finally {
    stopping.delete(client);
  }
}
