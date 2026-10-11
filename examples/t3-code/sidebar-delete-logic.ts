// Thread deletion order (task thread-commands-and-keys, G5), adapted from
// T3 Code 1e2ecbd975 (MIT); see LICENSE-T3. Source:
// apps/web/src/components/Sidebar.logic.ts (deleteSelectedThreadEntries,
// getFallbackThreadIdAfterDelete). Changes: results are plain objects instead
// of AsyncResult, and deleteSelectedThreadEntries can start from an earlier
// run's state and stop at an entry whose deletion paused on a question
// (`paused`), so the run resumes after the "Delete the worktree too?" answer.
import { str, type Obj } from './domain';
import { sortThreads, type ThreadSortOrder } from './legacy-sidebar-model';

export type DeleteResult = { ok: true } | { ok: false; error: unknown; interrupted?: boolean } | { paused: true };
export type DeleteOutcome = { deletedThreadKeys: Set<string>; firstFailure: { ok: false; error: unknown } | null; pausedAt: number };

/**
 * deleteSelectedThreadEntries: each entry waits for the one before it; a
 * worktree check excludes only earlier successes, ordinary failures continue
 * (the first is kept), an interruption stops the run, and a null result skips
 * an entry. `pausedAt` is the index of an entry whose deletion paused (-1 when
 * the run finished); `start` seeds a resumed run.
 */
export async function deleteSelectedThreadEntries<TEntry extends { readonly threadKey: string }>(input: {
  entries: readonly TEntry[];
  delete: (entry: TEntry, deletedThreadKeys: ReadonlySet<string>) => Promise<DeleteResult | null>;
  start?: { deletedThreadKeys: Set<string>; firstFailure: { ok: false; error: unknown } | null };
}): Promise<DeleteOutcome> {
  const deletedThreadKeys = input.start?.deletedThreadKeys ?? new Set<string>();
  let firstFailure = input.start?.firstFailure ?? null;
  for (const [index, entry] of input.entries.entries()) {
    const result = await input.delete(entry, deletedThreadKeys);
    if (result === null) continue;
    if ('paused' in result) return { deletedThreadKeys, firstFailure, pausedAt: index };
    if (!result.ok) {
      if (result.interrupted) break;
      firstFailure ??= result;
      continue;
    }
    deletedThreadKeys.add(entry.threadKey);
  }
  return { deletedThreadKeys, firstFailure, pausedAt: -1 };
}

/** getFallbackThreadIdAfterDelete: the top remaining thread of the deleted thread's project, in the sidebar's thread sort. */
export function getFallbackThreadIdAfterDelete(input: { threads: readonly Obj[]; deletedThreadId: string; sortOrder: ThreadSortOrder; deletedThreadIds?: ReadonlySet<string> }): string | null {
  const { deletedThreadId, deletedThreadIds, sortOrder, threads } = input;
  const deletedThread = threads.find(thread => thread.id === deletedThreadId);
  if (!deletedThread) return null;
  const candidates = threads.filter(thread => thread.projectId === deletedThread.projectId && thread.id !== deletedThreadId && !deletedThreadIds?.has(str(thread.id)));
  const first = sortThreads(candidates, sortOrder)[0];
  return first ? str(first.id) : null;
}
