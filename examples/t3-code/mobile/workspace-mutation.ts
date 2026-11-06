import { arr, obj, str } from './shared/domain';
import type { T3Client } from './shared/client';

/**
 * The latest settled work item or checkpoint that may have changed workspace files.
 * Assistant text and in-flight work do not invalidate the media URLs already shown.
 */
export function workspaceMutationId(client: T3Client): string | null {
  const projection = obj(client.projection);
  const items = arr(projection.visibleTurnItems).map(row => obj(row.item));
  const settled = items.findLast(item =>
    ['command_execution', 'file_change', 'dynamic_tool', 'subagent'].includes(str(item.type))
    && !['pending', 'running', 'waiting'].includes(str(item.status)));
  const checkpoint = arr(projection.checkpoints).findLast(entry => entry.appRunOrdinal != null && entry.runId != null);
  const itemId = settled ? str(settled.id) || null : null;
  const capturedAt = checkpoint ? str(checkpoint.capturedAt) || null : null;
  return itemId === null && capturedAt === null ? null : JSON.stringify([itemId, capturedAt]);
}
