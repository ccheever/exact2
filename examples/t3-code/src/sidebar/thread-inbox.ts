// The Working shelf's placement over time (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3):
// packages/client-runtime/src/state/threadInbox.ts createInboxReturnTracker and WORKING_EXIT_GRACE_MS. A thread that
// stops working keeps its place under Working for the grace, so a server that settles it meanwhile sends it straight
// to Settled; then it heads the inbox at its return. Port changes: the data module has no clock or timers. Its time
// is the window's, the freshest it has heard (`at`); a change starts its grace no earlier than the shell's own
// `updatedAt`, so a snapshot clock that lags the event does not end the grace at once. The window ends a grace when
// its live clock passes the deadline (sidebar.contract's grace marker reports the time, `expire`); a later
// observation past the deadline ends it too.
import { str, type Obj } from '../client/domain';
import { effectiveSnoozed, isWorkingThread, latestRun, pendingApproval, pendingInput, shellRuntime } from './sidebar-model';
import { pendingSubagentRequest } from './subagent-requests';

/**
 * How long a thread that stopped working keeps reading as working. An orchestrator run can end moments before its
 * continuation starts, so clients wait this long before treating the thread as done.
 */
export const WORKING_EXIT_GRACE_MS = 2_000;

type State = { working: boolean; deadline: number | null; placement: string };
export interface InboxObservation {
  isWorking(thread: Obj): boolean;
  returnedAt(thread: Obj): number | undefined;
}
const parse = (value: unknown): number => { const time = Date.parse(str(value)); return Number.isFinite(time) ? time : 0; };

/** Needs the user, or failed: such a thread leaves Working at once (only placement is held). */
function actionable(thread: Obj): boolean {
  return pendingApproval(thread) || pendingInput(thread) || pendingSubagentRequest(thread) !== null || Number(thread.pendingGuestMessageCount ?? 0) > 0
    || (thread.interactionMode === 'plan' && thread.hasActionableProposedPlan === true)
    || (shellRuntime(thread) !== null && thread.lastError != null) || latestRun(thread)?.status === 'failed';
}

export class InboxReturnTracker {
  private states = new Map<string, State>();
  private returns = new Map<string, number>();
  /** The freshest window time heard: a snapshot's clock, a sidebar command's or the grace marker's report. */
  private latest = 0;

  constructor(private readonly keyOf: (thread: Obj) => string) {}

  /** `observe` with every thread shell when they or the clock change, or null while the shelf is off. */
  observe(threads: Obj[] | null, now: number): InboxObservation {
    if (threads === null) {
      this.states.clear(); this.returns.clear();
      return { isWorking: isWorkingThread, returnedAt: () => undefined };
    }
    const at = this.note(now), present = new Set<string>();
    for (const thread of threads) {
      const key = this.keyOf(thread);
      present.add(key);
      const placement = JSON.stringify([thread.archivedAt, thread.deletedAt, thread.pinnedAt, thread.settledOverride, thread.settledAt,
        thread.snoozedAt, thread.snoozedUntil, thread.activeOrderKey].map(value => value ?? null));
      const previous = this.states.get(key);
      const moved = previous !== undefined && previous.placement !== placement;
      const excluded = thread.archivedAt != null || thread.deletedAt != null || thread.pinnedAt != null
        || thread.settledOverride === 'settled' || effectiveSnoozed(thread, at);
      const actual = isWorkingThread(thread);
      const deadline = !actual && previous?.working && !actionable(thread) && !moved && !excluded
        ? previous.deadline ?? Math.max(at, parse(thread.updatedAt)) + WORKING_EXIT_GRACE_MS : null;
      const working = actual || (deadline !== null && deadline > at);
      if (previous?.working && !working) this.returns.set(key, deadline ?? at);
      this.states.set(key, { working, deadline: working ? deadline : null, placement });
    }
    for (const key of this.states.keys()) {
      if (!present.has(key)) { this.states.delete(key); this.returns.delete(key); }
    }
    const observedWorking = new Map([...this.states].map(([key, state]) => [key, state.working]));
    const observedReturns = new Map(this.returns);
    return {
      isWorking: thread => observedWorking.get(this.keyOf(thread)) ?? isWorkingThread(thread),
      returnedAt: thread => observedReturns.get(this.keyOf(thread)),
    };
  }
  /** The window's clock reached `now`: every grace due by then ends, its return at its deadline (the timer's time). */
  expire(now: number): boolean {
    const at = this.note(now);
    let ended = false;
    for (const [key, state] of this.states) {
      if (state.deadline !== null && state.deadline <= at) {
        this.returns.set(key, state.deadline); state.working = false; state.deadline = null; ended = true;
      }
    }
    return ended;
  }
  /** The soonest grace's end, 0 for none. */
  nextDeadline(): number {
    let next = Infinity;
    for (const state of this.states.values()) if (state.deadline !== null) next = Math.min(next, state.deadline);
    return Number.isFinite(next) ? next : 0;
  }
  isWorking(thread: Obj): boolean { return this.states.get(this.keyOf(thread))?.working ?? isWorkingThread(thread); }
  returnedAt(thread: Obj): number | undefined { return this.returns.get(this.keyOf(thread)); }
  private note(now: number): number {
    if (Number.isFinite(now) && now > this.latest) this.latest = now;
    return this.latest;
  }
}

const trackers = new WeakMap<object, InboxReturnTracker>();
/** One tracker per window client, kept at module scope so placement survives the list's remounts. */
export function inboxReturns(client: object, keyOf: (thread: Obj) => string): InboxReturnTracker {
  let tracker = trackers.get(client);
  if (!tracker) { tracker = new InboxReturnTracker(keyOf); trackers.set(client, tracker); }
  return tracker;
}
