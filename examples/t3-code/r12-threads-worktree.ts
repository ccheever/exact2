// Lane r12-threads: the worktree setup card leaves a thread once its turn is live, also after the
// run settled (T3 Code, MIT, see LICENSE-T3: client-runtime threadExecution.ts
// deriveThreadActivityRun, shared orchestrationV2ThreadError.ts latestUnheldRun, ChatView.tsx
// resolveVisibleWorktreeSetup's `turnStarted: activeActivityRun?.startedAt != null`). The activity
// run is the newest run doing live work, else the newest run that is not a held queued one, so a
// completed run (a retried preparation's included) still counts as started and a clean "Worktree
// ready" card does not stay above its reply.
import { arr, num, str, type Obj } from './domain';

const ACTIVITY = new Set(['preparing', 'starting', 'running', 'waiting']);
const ordinal = (run: Obj, index: number) => (typeof run.ordinal === 'number' ? num(run.ordinal) : index);
function latest(runs: Obj[], keep: (run: Obj) => boolean): Obj | null {
  let best: Obj | null = null, bestOrdinal = -Infinity;
  for (let index = 0; index < runs.length; index++) {
    const run = runs[index]!;
    if (keep(run) && ordinal(run, index) > bestOrdinal) { best = run; bestOrdinal = ordinal(run, index); }
  }
  return best;
}

/** deriveThreadActivityRun without the usage-limit presentation: the live run, else the latest unheld one. */
export function activityRun(projection: Obj): Obj | null {
  const runs = arr(projection.runs);
  return latest(runs, run => ACTIVITY.has(str(run.status))) ?? latest(runs, run => !(run.status === 'queued' && run.queueHeld === true));
}

/** resolveVisibleWorktreeSetup's turnStarted. */
export const setupTurnStarted = (projection: Obj): boolean => !!activityRun(projection)?.startedAt;
