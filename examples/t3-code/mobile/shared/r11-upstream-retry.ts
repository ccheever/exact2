// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r11-upstream-retry.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r11-upstream: retry a failed workspace preparation (upstream 737993303d;
// T3 Code, MIT, see LICENSE-T3: client-runtime turnItemPresentation.ts
// workspacePreparationRetryRunIds / turnItemIsWorkspacePreparation,
// operations/commands.ts retryWorkspacePreparation, ChatView.tsx and
// MessagesTimeline.tsx WorkEntryLogRow). A failed preparation's error row offers
// Retry while its run still ends there; the retry dispatches prepared-run.retry,
// the server prepares the run's workspace again and cancels the old error item,
// which the timeline then hides.
import { arr, obj, str, type Activity, type Obj } from './domain';
import type { T3Client } from './client';
import { ClientError, type Files, type Native } from './protocol';

/** ORCHESTRATION_V2_WORKSPACE_PREPARATION_FAILURE_CODE. */
export const WORKSPACE_PREPARATION_FAILURE_CODE = 'workspace_preparation_failed';

/** turnItemIsWorkspacePreparation: the synthetic setup command, and a preparation failure a retry cancelled. */
export function turnItemIsWorkspacePreparation(item: Obj): boolean {
  return item.type === 'command_execution' && item.input === 'Preparing workspace'
    || item.type === 'error' && item.status === 'cancelled' && obj(item.failure).code === WORKSPACE_PREPARATION_FAILURE_CODE;
}

/**
 * Runs a Retry can prepare again: their workspace preparation failed and the run
 * still ended there. Older servers record no preparation on the run, so they never offer it.
 */
export function workspacePreparationRetryRunIds(runs: Obj[], items: Obj[]): Set<string> {
  const failed = new Set(runs.filter(run => run.status === 'failed' && run.workspacePreparation !== undefined && run.workspacePreparation !== null).map(run => str(run.id)));
  const retryable = new Set<string>();
  if (!failed.size) return retryable;
  for (const item of items) {
    if (item.type === 'error' && item.status === 'failed' && obj(item.failure).code === WORKSPACE_PREPARATION_FAILURE_CODE
      && item.runId != null && failed.has(str(item.runId))) retryable.add(str(item.runId));
  }
  return retryable;
}

/** The failure row's run when its item is a workspace preparation failure (WorkEntryLogRow's retryRunId candidate). */
export function preparationFailureRunId(item: Obj): string {
  return item.type === 'error' && item.status === 'failed' && obj(item.failure).code === WORKSPACE_PREPARATION_FAILURE_CODE && item.runId != null ? str(item.runId) : '';
}

/** Keeps Retry only on rows whose run is still retryable in the open thread's projection. */
export function retryableActivities<T extends Activity & { retryRunId: string }>(client: T3Client, activities: T[]): T[] {
  if (!activities.some(activity => activity.retryRunId)) return activities;
  const projection = client.projection;
  const retryable = workspacePreparationRetryRunIds(arr(projection.runs), arr(projection.turnItems));
  return activities.map(activity => activity.retryRunId && !retryable.has(activity.retryRunId) ? { ...activity, retryRunId: '' } : activity);
}

/** Runs with a retry in flight: a second press lands after the run is preparing again. */
const retrying = new WeakMap<T3Client, Set<string>>();

/** threadEnvironment.retryWorkspacePreparation: prepared-run.retry for the open thread's failed run. */
export async function retryWorkspacePreparation(client: T3Client, native: Native, storage: Files, runId: string): Promise<string> {
  const threadId = client.threadId;
  if (!threadId || !runId) throw new ClientError('That thread is no longer available.');
  const inFlight = retrying.get(client) ?? new Set<string>();
  retrying.set(client, inFlight);
  if (inFlight.has(runId)) return '';
  inFlight.add(runId);
  try {
    const access = client.restAccess(native);
    const [commandId] = await access.ids(1);
    await access.dispatch(storage, { type: 'prepared-run.retry', commandId, threadId, runId }, 'Retry workspace preparation');
  } finally { inFlight.delete(runId); }
  return '';
}
