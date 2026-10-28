// ThreadNotificationCoordinator.logic.test.ts at 7c3b3b3605ef (MIT, see LICENSE-T3), "createThreadNotificationGate",
// on the shell's clock (shell-notify.ts threadTransitions): a completion waits WORKING_EXIT_GRACE_MS from the clock's
// first tick after it was seen, so each observation below is followed by that tick.
import { describe, expect, test } from 'bun:test';
import type { Obj, Shell } from '../../src/client/domain';
import { notifyGate, threadTransitions, WORKING_EXIT_GRACE_MS } from '../../src/shell/shell-notify';

const at = (ms: number) => new Date(Date.UTC(2026, 9, 27) + ms).toISOString();
const working: Obj = { status: 'running', activeRunId: 'r1', activityRunStatus: 'running' };
const completed = (ms: number): Obj => ({ status: 'completed', activeRunId: null, activityRunStatus: null, latestRunCompletedAt: at(ms) });
const approval: Obj = { ...working, pendingRuntimeRequest: { kind: 'approval' } };

function harness() {
  const gate = notifyGate(), kinds: string[] = [];
  let previous: ReturnType<typeof threadTransitions>['next'] | null = null, now = 1_000;
  const pass = (fields: Obj, extra: Obj = {}) => {
    const thread = { id: 't', title: 'Thread', latestRunId: 'r1', pendingRuntimeRequest: null, archivedAt: null, lineage: {}, ...fields, ...extra };
    const result = threadTransitions({ shell: { threads: [thread], projects: [] } as unknown as Shell }, previous, gate, now);
    previous = result.next;
    kinds.push(...result.transitions.map(transition => transition.kind));
  };
  let last: Obj = working, lastExtra: Obj = {};
  /** An observation, then the clock's next tick. */
  const observe = (fields: Obj, extra: Obj = {}) => { last = fields; lastExtra = extra; pass(fields, extra); now += 1; pass(fields, extra); };
  /** The clock runs on, ticking with the thread unchanged. */
  const advance = (ms: number) => { now += ms; pass(last, lastExtra); };
  return { gate, kinds, observe, advance };
}

describe('the completion grace', () => {
  test('drops a completion when the thread resumes within the grace', () => {
    const h = harness();
    h.observe(working); h.observe(completed(1));
    h.advance(WORKING_EXIT_GRACE_MS - 1);
    h.observe(working);
    h.advance(WORKING_EXIT_GRACE_MS * 2);
    expect(h.kinds).toEqual([]);
  });
  test('notifies a real completion once after the grace', () => {
    const h = harness();
    h.observe(working); h.observe(completed(1));
    h.advance(WORKING_EXIT_GRACE_MS - 1);
    expect(h.kinds).toEqual([]);
    h.advance(1);
    expect(h.kinds).toEqual(['completion']);
    h.observe({ ...completed(1) });
    h.advance(WORKING_EXIT_GRACE_MS * 2);
    expect(h.kinds).toEqual(['completion']);
  });
  test('notifies attention at once and replaces a pending completion', () => {
    const h = harness();
    h.observe(working); h.observe(completed(1));
    h.advance(500);
    h.observe(approval);
    expect(h.kinds).toEqual(['input']);
    h.advance(60_000);
    h.observe(completed(1));
    h.advance(WORKING_EXIT_GRACE_MS * 2);
    expect(h.kinds).toEqual(['input']);
  });
  test('notifies one completion across continuation gaps', () => {
    const h = harness();
    h.observe(working); h.observe(completed(1));
    h.advance(300); h.observe(working);
    h.advance(5_000); h.observe(completed(2));
    h.advance(300); h.observe(working);
    h.advance(5_000); h.observe(completed(3));
    h.advance(WORKING_EXIT_GRACE_MS);
    expect(h.kinds).toEqual(['completion']);
  });
  test('restarts the grace when a newer completion replaces a pending one', () => {
    const h = harness();
    h.observe(working); h.observe(completed(1));
    h.advance(1_500); h.observe(completed(2));
    h.advance(1_500);
    expect(h.kinds).toEqual([]);
    h.advance(500);
    expect(h.kinds).toEqual(['completion']);
  });
  test('drops a pending completion on archive or a stop request, and never notifies the first observation', () => {
    for (const extra of [{ archivedAt: at(0) }, { latestRunStopRequested: true }]) {
      const h = harness();
      h.observe(working); h.observe(completed(1), extra);
      h.advance(WORKING_EXIT_GRACE_MS * 2);
      expect(h.kinds).toEqual([]);
    }
    const first = harness();
    first.observe(completed(1));
    first.advance(WORKING_EXIT_GRACE_MS * 2);
    expect(first.kinds).toEqual([]);
  });
});
