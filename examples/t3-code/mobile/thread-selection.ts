// @ref llp/1109.011-responsive-workspace.decision.md#navigation-and-data-ownership
import type { T3Client } from './shared/client';
import type { Files, Native } from './shared/protocol';
import { parseFleetThreadId } from './shared/settings-b-fleet';

/** A route projection can still name the background connection after a completed
 * focus. Resolve against the live client before the shared command looks up that
 * now-removed fleet entry. A different environment must retain its full identity. */
export async function mobileThreadSelection(client: T3Client, id: string, native: Native, storage: Files) {
  const target = parseFleetThreadId(id);
  // Focus restores the requested selection before the new shell snapshot arrives.
  // Reopening it now would treat the empty shell as a missing thread and replace
  // that selection with a draft. The existing refresh owns its snapshot load.
  if (target?.environmentId === client.environmentId && target.threadId === client.threadId)
    return { revision: client.revision, message: '' };
  return client.command('select-thread', target?.environmentId === client.environmentId ? target.threadId : id,
    '', 0, native, storage);
}
