// App-owned canonical environment identity for durable queued content.
// @ref llp/1106.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import type { T3Client } from './shared/client';
import { str } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
const origins = new WeakMap<T3Client, { home: string; origin: string; environmentId: string; generation: number }>();
export function mobileQueuedEditOrigin(client: T3Client): string {
  const saved = origins.get(client);
  return saved?.origin === client.origin && saved.environmentId === client.environmentId && saved.generation === client.generation ? saved.home : client.origin;
}
export async function queuedEditRefreshOrigin(native: Native, client: T3Client): Promise<void> {
  const origin = client.origin, environmentId = client.environmentId, generation = client.generation;
  const response = await client.call(native, { op: 'status' });
  if (origin !== client.origin || environmentId !== client.environmentId || generation !== client.generation) throw new ClientError('The connection changed.', 'superseded');
  if (str(response.environmentId) && response.environmentId !== environmentId || str(response.origin) && response.origin !== origin) throw new ClientError('The connection changed.', 'superseded');
  const home = str(response.homeOrigin) || origin;
  origins.set(client, { home, origin, environmentId, generation });
}
