// Stop on the background work line, from useThreadLifecycle.tsx and client-runtime state/threadExecution.ts
// (isBackgroundWorkStopPending) at T3 Code 5d3d78ab93 (MIT; see LICENSE-T3). The server accepting Stop does not end
// the tasks, so the line reads "Stopping..." until the tasks listed when it was pressed leave, for at most 10 seconds
// after the acceptance (the Claude adapter forces a stuck query closed after 10 seconds); a failure brings Stop back.
import { str, type Obj } from '../client/domain';

export const BACKGROUND_WORK_STOP_RETRY_AFTER_MS = 10_000;
type StopRequest = { taskIds: string[]; retryAt: number | null };
const requests = new WeakMap<object, Map<string, StopRequest>>();
const threadKey = (client: { environmentId: string; threadId: string }) => `${client.environmentId}:${client.threadId}`;

/** isBackgroundWorkStopPending for the open thread, its pending tasks as listed now. */
export function backgroundStopPending(client: { environmentId: string; threadId: string }, tasks: Obj[], now: number): boolean {
  const request = requests.get(client)?.get(threadKey(client));
  if (!request || (request.retryAt !== null && now >= request.retryAt)) return false;
  return request.retryAt === null || tasks.some(task => request.taskIds.includes(str(task.taskId)));
}

/**
 * Runs the stop for the open thread's tasks: nothing while one is pending; held while the request is out, then
 * until the tasks leave or the retry window ends (`settled` is told when that is), and dropped when it fails. `clock`
 * reads the app's time (epoch ms).
 */
export async function stopBackgroundWork(client: { environmentId: string; threadId: string }, tasks: Obj[], clock: () => number,
  stop: () => Promise<void>, settled: (retryAfterMs: number) => void): Promise<void> {
  if (backgroundStopPending(client, tasks, clock())) return;
  let byThread = requests.get(client);
  if (!byThread) requests.set(client, byThread = new Map());
  const key = threadKey(client), request: StopRequest = { taskIds: tasks.map(task => str(task.taskId)), retryAt: null };
  byThread.set(key, request);
  try { await stop(); }
  catch (error) {
    if (byThread.get(key) === request) byThread.delete(key);
    throw error;
  }
  if (byThread.get(key) !== request) return;
  byThread.set(key, { ...request, retryAt: clock() + BACKGROUND_WORK_STOP_RETRY_AFTER_MS });
  settled(BACKGROUND_WORK_STOP_RETRY_AFTER_MS);
}
