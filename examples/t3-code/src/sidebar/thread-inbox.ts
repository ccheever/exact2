// The Working shelf's placement over time (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3):
// packages/client-runtime/src/state/threadInbox.ts createInboxReturnTracker, its grace shell-notify.ts's
// WORKING_EXIT_GRACE_MS. A thread that stops working keeps its place under Working for the grace, so a server that
// settles it meanwhile sends it straight to Settled; then it heads the inbox at its return. The reference stamps a
// grace's start and a return with the client's own `now()`. A data module has no clock: its snapshot time lags by up to a minute, and a shell's
// `updatedAt` is the server's clock. So an observation only owes those stamps, and the root's timers pay them with
// the window's clock (`advance`): at once for an owed stamp (app.contract workingGraceOwed), and 2 s after the soonest
// grace started (workingGraceEnds). Until a return is stamped it sorts as the newest thread, where `now()` puts it.
import type { Obj } from '../client/domain';
import { effectiveSnoozed, inboxTime, isWorkingThread, latestRun, pendingApproval, pendingInput, shellRuntime } from './sidebar-model';
import { pendingSubagentRequest } from './subagent-requests';
import { WORKING_EXIT_GRACE_MS } from '../shell/shell-notify';

type State = { working: boolean; deadline: number | null; placement: string };
export interface InboxObservation {
  isWorking(thread: Obj): boolean;
  returnedAt(thread: Obj): number | undefined;
}

/** Needs the user, or failed: such a thread leaves Working at once (only placement is held). */
function actionable(thread: Obj): boolean {
  return pendingApproval(thread) || pendingInput(thread) || pendingSubagentRequest(thread) !== null || Number(thread.pendingGuestMessageCount ?? 0) > 0
    || (thread.interactionMode === 'plan' && thread.hasActionableProposedPlan === true)
    || (shellRuntime(thread) !== null && thread.lastError != null) || latestRun(thread)?.status === 'failed';
}

export class InboxReturnTracker {
  private states = new Map<string, State>();
  private returns = new Map<string, number>();
  /** Graces whose start waits for the window's clock, with the time they were seen at. */
  private owedStarts = new Map<string, number>();
  /** Returns whose stamp waits for the window's clock; each sorts as the newest thread meanwhile. */
  private owedReturns = new Set<string>();
  private serial = 0;
  /** The freshest window time heard: a snapshot's clock or the window's report. */
  private latest = 0;

  constructor(private readonly keyOf: (thread: Obj) => string) {}

  /** `observe` with every thread shell when they or the clock change, or null while the shelf is off. */
  observe(threads: Obj[] | null, now: number): InboxObservation {
    if (threads === null) {
      this.states.clear(); this.returns.clear(); this.owedStarts.clear(); this.owedReturns.clear();
      return { isWorking: isWorkingThread, returnedAt: () => undefined };
    }
    const at = this.note(now), present = new Set<string>();
    let newest = at;
    for (const time of this.returns.values()) newest = Math.max(newest, time);
    for (const thread of threads) newest = Math.max(newest, inboxTime(thread));
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
      // Only shelf placement is held: execution state and attention requests stay authoritative.
      const held = !actual && previous?.working === true && !actionable(thread) && !moved && !excluded;
      let deadline = held ? previous!.deadline : null;
      if (held && deadline === null) {
        const since = this.owedStarts.get(key);
        if (since === undefined) { this.owedStarts.set(key, at); this.serial++; }
        // The clock moved on since (a minute tick, or no sidebar to report): that reading starts it.
        else if (at > since) { deadline = at + WORKING_EXIT_GRACE_MS; this.owedStarts.delete(key); }
      } else this.owedStarts.delete(key);
      const working = actual || (held && (deadline === null || deadline > at));
      if (previous?.working && !working) {
        if (deadline !== null) this.returns.set(key, deadline); // a grace that ran out: the reference's timer time
        else { this.returns.set(key, newest + 1); this.owedReturns.add(key); this.serial++; }
      }
      this.states.set(key, { working, deadline: working ? deadline : null, placement });
    }
    for (const key of this.states.keys()) {
      if (!present.has(key)) { this.states.delete(key); this.returns.delete(key); this.owedStarts.delete(key); this.owedReturns.delete(key); }
    }
    const observedWorking = new Map([...this.states].map(([key, state]) => [key, state.working]));
    const observedReturns = new Map(this.returns);
    return {
      isWorking: thread => observedWorking.get(this.keyOf(thread)) ?? isWorkingThread(thread),
      returnedAt: thread => observedReturns.get(this.keyOf(thread)),
    };
  }
  /**
   * The window's clock read `now`: each owed grace starts and each owed return is stamped there, and every grace due
   * by then ends, its return at its deadline (where the reference's timer fires).
   */
  advance(now: number): boolean {
    const at = this.note(now);
    let changed = false;
    for (const key of this.owedStarts.keys()) {
      const state = this.states.get(key);
      if (state?.working && state.deadline === null) { state.deadline = at + WORKING_EXIT_GRACE_MS; changed = true; }
    }
    for (const key of this.owedReturns) { this.returns.set(key, at); changed = true; }
    this.owedStarts.clear(); this.owedReturns.clear();
    for (const [key, state] of this.states) {
      if (state.deadline !== null && state.deadline <= at) {
        this.returns.set(key, state.deadline); state.working = false; state.deadline = null; changed = true;
      }
    }
    return changed;
  }
  /** Changes whenever a start or a stamp becomes owed, and is 0 while none is: the window then reports its clock. */
  owed(): number { return this.owedStarts.size > 0 || this.owedReturns.size > 0 ? this.serial : 0; }
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
