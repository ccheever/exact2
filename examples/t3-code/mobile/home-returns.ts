// @ref llp/1109.004-home-projection.decision.md#decision
// Pinned threadInbox.ts createInboxReturnTracker; root supplies observation time.
import { str, type Obj } from './shared/domain';
import { isWorkingThread } from './shared/sidebar-model';

export interface HomeReturnState {
  lastWorkingKeys: string[] | null;
  returnedAt: Record<string, number>;
}
const keyOf = (thread: Obj) => `${str(thread.environmentId)}:${str(thread.id)}`;

/** Observe the complete navigation population before display filters, once per rebuild. */
export function homeObserveReturns(state: HomeReturnState, threads: Obj[] | null, now: number): void {
  if (threads === null) {
    state.lastWorkingKeys = null;
    state.returnedAt = {};
    return;
  }
  const present = new Set(threads.map(keyOf));
  const working = threads.filter(isWorkingThread).map(keyOf), workingKeys = new Set(working);
  for (const key of Object.keys(state.returnedAt)) if (!present.has(key)) delete state.returnedAt[key];
  for (const key of state.lastWorkingKeys ?? []) {
    if (present.has(key) && !workingKeys.has(key)) state.returnedAt[key] = now;
  }
  state.lastWorkingKeys = working;
}
