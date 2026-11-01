// Lineage's writes (T3 Code 2359643cff, MIT, see LICENSE-T3: components/chat/ThreadRelationshipsControl.tsx merge and
// detach; client-runtime operations/commands.ts mergeThreadBack and stopThreadSession; state/threadWorkflows.ts
// canDetachThreadProviderSession). A failure is a toast, "Could not merge back" or "Could not disconnect agent
// session", which the next try closes first; the transcript's error banner stays clear of it.
import type { T3Client } from '../client/client';
import { arr, obj, str, type Obj } from '../client/domain';
import { activeRun, ClientError, type Files, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { dismissToast, pushToast } from '../shell/toast';

const toasts = new WeakMap<T3Client, { merge: number; detach: number }>();
const toastIds = (client: T3Client) => { let ids = toasts.get(client); if (!ids) toasts.set(client, ids = { merge: 0, detach: 0 }); return ids; };
const messageOf = (error: unknown) => error instanceof Error ? error.message : '';

/** resolveThreadProviderSession: the active run's provider session, else the attached provider thread's, else the newest live one. */
export function threadProviderSession(projection: Obj): Obj | null {
  const sessions = arr(projection.providerSessions), threads = arr(projection.providerThreads), thread = obj(projection.thread);
  const providerThreadId = str(activeRun(projection)?.providerThreadId) || str(thread.activeProviderThreadId);
  const active = providerThreadId ? threads.find(entry => entry.id === providerThreadId) : undefined;
  const attached = active ?? threads.find(entry => entry.appThreadId === thread.id && entry.providerSessionId != null);
  const sessionId = str(attached?.providerSessionId);
  if (sessionId) return sessions.find(session => session.id === sessionId) ?? null;
  return sessions.slice().reverse().find(session => session.status !== 'stopped' && session.status !== 'error') ?? null;
}
/** canDetachThreadProviderSession: the Lineage heading's "Disconnect agent session". */
export function canDetachSession(projection: Obj | null): boolean {
  const session = projection ? threadProviderSession(projection) : null;
  return session !== null && session.status !== 'stopped' && session.status !== 'error';
}

async function withToast(client: T3Client, which: 'merge' | 'detach', title: string, run: () => Promise<void>): Promise<boolean> {
  const ids = toastIds(client);
  if (ids[which]) { dismissToast(client, ids[which]); ids[which] = 0; }
  const before = client.error;
  try { await run(); return true; } catch (error) {
    if (letGo(error)) throw error;
    if (client.error !== before && client.error === messageOf(error)) client.error = before; // the toast stands in for the banner
    ids[which] = pushToast(client, { kind: 'error', title });
    return false;
  }
}

/** ThreadRelationshipsPanel merge(): thread.merge_back from the fork's latest finished run, then the target opens. */
export async function mergeBack(client: T3Client, native: Native, storage: Files, sourceThreadId: string, value: string): Promise<void> {
  const [targetThreadId = '', runId = ''] = value.split('|');
  if (!targetThreadId || !runId) throw new ClientError('Complete a run in this fork before merging it back');
  const title = str(client.shell.threads.find(thread => thread.id === sourceThreadId)?.title, 'thread');
  const access = client.restAccess(native);
  const merged = await withToast(client, 'merge', 'Could not merge back', async () => {
    const [commandId] = await access.ids(1);
    await access.dispatch(storage, { type: 'thread.merge_back', commandId, createdBy: 'user', creationSource: 'web', sourceThreadId, targetThreadId,
      sourcePoint: { type: 'run', runId } }, `Merge ${title} back`);
  });
  if (merged) await client.openSelected(native, targetThreadId);
}

/** ThreadRelationshipsPanel detach(): every provider session of the thread is detached (stopThreadSession). */
export async function detachSession(client: T3Client, native: Native, threadId: string): Promise<void> {
  if (threadId !== client.threadId || !canDetachSession(client.projection)) return;
  const access = client.restAccess(native);
  await withToast(client, 'detach', 'Could not disconnect agent session', async () => {
    const projection = obj(await access.request('orchestration.getThreadProjection', { threadId }));
    const [commandId] = await access.ids(1);
    for (const session of arr(projection.providerSessions)) {
      await access.request('orchestration.dispatchCommand', { type: 'provider-session.detach', commandId: `${commandId}:detach:${str(session.id)}`, threadId,
        providerSessionId: str(session.id), reason: 'client-requested' }, true);
    }
  });
}
