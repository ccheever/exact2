// Lineage's parent row for a member of a provider-native coordinator (MIT reference, see
// LICENSE-T3: apps/web/src/components/chat/ThreadRelationshipsControl.tsx parentRef and
// parentNativeStatus, upstream 74541bab17). The coordinator runs no app run, so its shell
// status stays idle; the row reads the latest runless root turn of the coordinator's own
// projection (deriveProviderSubagentStatus). The reference subscribes to that projection; here it
// is read again when the coordinator's shell changes, and every few seconds while its turn is live.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { isProviderSubagent, subagentStatus } from '../composer/composer-controls-subagent';
import { parentOf } from './shell-lineage';

export type NativeParent = { threadId: string; status: string };
type Entry = { key: string; at: number; status: string };
const LIVE_REFRESH_MS = 5_000;
const cache = new WeakMap<object, Entry>();
const live = (status: string) => status === 'pending' || status === 'running' || status === 'waiting';

/** The open thread's parent when it is a provider-native subagent thread, with its root turn's status ('' until known). */
export async function nativeParentStatus(client: T3Client, native: Native, threadId: string, now: number): Promise<NativeParent | null> {
  const threads = client.shell.threads;
  const current = threads.find(thread => thread.id === threadId);
  const parentId = current ? parentOf(current) : '';
  const parent = parentId ? threads.find(thread => thread.id === parentId) : undefined;
  if (!parent || !isProviderSubagent(parent)) return null;
  const key = `${client.environmentId}\u001f${parentId}\u001f${str(parent.updatedAt)}`;
  const cached = cache.get(client);
  if (cached?.key === key && !(live(cached.status) && now - cached.at >= LIVE_REFRESH_MS)) return { threadId: parentId, status: cached.status };
  try {
    const projection: Obj = parentId === client.threadId ? client.projection
      : obj(await client.restAccess(native).read('orchestration.getThreadProjection', { threadId: parentId }));
    const status = subagentStatus(projection)?.status ?? '';
    cache.set(client, { key, at: now, status });
    return { threadId: parentId, status };
  } catch (error) {
    if (letGo(error)) throw error;
    return { threadId: parentId, status: cached?.key === key ? cached.status : '' };
  }
}
