// Lineage's parent row for a member of a provider-native coordinator (MIT reference, see
// LICENSE-T3: apps/web/src/components/chat/ThreadRelationshipsControl.tsx parentRef and
// parentNativeStatus, upstream 74541bab17). The coordinator runs no app run, so its shell
// status stays idle; the row reads the latest runless root turn of the coordinator's own
// projection (deriveProviderSubagentStatus). As the reference's useThreadProjection, the
// coordinator's thread stream stays subscribed while the details show its member: the keep-alive
// pins it (keep-alive.ts), and closing the details or leaving the member releases it.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { keepAlivePrepare, keptThread, pinKeptThreads } from '../client/keep-alive';
import { isProviderSubagent, subagentStatus } from '../composer/composer-controls-subagent';
import { parentOf } from './shell-lineage';

export type NativeParent = { threadId: string; status: string };

/** The open thread's parent when it is a provider-native subagent thread, with its root turn's status ('' until its stream is live). */
export async function nativeParentStatus(client: T3Client, native: Native, threadId: string): Promise<NativeParent | null> {
  const threads = client.shell.threads;
  const current = threads.find(thread => thread.id === threadId);
  const parentId = current ? parentOf(current) : '';
  const parent = parentId ? threads.find(thread => thread.id === parentId) : undefined;
  const wanted = parent && isProviderSubagent(parent) ? parentId : '';
  if (pinKeptThreads(client, wanted ? [wanted] : [])) await keepAlivePrepare(client, native);
  if (!wanted) return null;
  const projection = wanted === client.threadId ? client.projection : keptThread(client, wanted)?.projection;
  return { threadId: wanted, status: projection ? subagentStatus(projection)?.status ?? '' : '' };
}

/** The details closed (or show a draft): the coordinator's stream is released. */
export async function releaseNativeParent(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (pinKeptThreads(client, [])) await keepAlivePrepare(client, native);
}
