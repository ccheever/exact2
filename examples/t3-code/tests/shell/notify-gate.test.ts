// ThreadNotificationCoordinator.logic.test.ts at 5d3d78ab93 (MIT, see LICENSE-T3), "createThreadNotificationGate", on
// shell-notify.ts threadTransitions: attention and completions notify at once, a completion once.
import { describe, expect, test } from 'bun:test';
import type { Obj, Shell } from '../../src/client/domain';
import { notifyGate, threadTransitions } from '../../src/shell/shell-notify';

const at = (ms: number) => new Date(Date.UTC(2026, 9, 27) + ms).toISOString();
const working: Obj = { status: 'running', activeRunId: 'r1', activityRunStatus: 'running' };
const completed = (ms: number): Obj => ({ status: 'completed', activeRunId: null, activityRunStatus: null, latestRunCompletedAt: at(ms) });
const approval: Obj = { ...working, pendingRuntimeRequest: { kind: 'approval' } };

function harness() {
  const gate = notifyGate(), kinds: string[] = [];
  let previous: ReturnType<typeof threadTransitions>['next'] | null = null;
  const observe = (fields: Obj, extra: Obj = {}) => {
    const thread = { id: 't', title: 'Thread', latestRunId: 'r1', pendingRuntimeRequest: null, archivedAt: null, lineage: {}, ...fields, ...extra };
    const result = threadTransitions({ shell: { threads: [thread], projects: [] } as unknown as Shell }, previous, gate);
    previous = result.next;
    kinds.push(...result.transitions.map(transition => transition.kind));
  };
  const forget = () => { previous = null; };
  return { gate, kinds, observe, forget };
}

describe('the notification gate', () => {
  test('notifies a completion immediately and only once', () => {
    const h = harness();
    h.observe(working); h.observe(completed(1));
    expect(h.kinds).toEqual(['completion']);
    h.observe({ ...completed(1) });
    h.observe(working); h.observe(completed(1));
    expect(h.kinds).toEqual(['completion']);
  });
  test('notifies each newer completion immediately', () => {
    const h = harness();
    h.observe(working); h.observe(completed(1));
    h.observe(working); h.observe(completed(2));
    expect(h.kinds).toEqual(['completion', 'completion']);
    h.observe(completed(1));
    expect(h.kinds).toHaveLength(2);
  });
  test('notifies attention once and still notifies the subsequent completion', () => {
    const h = harness();
    h.observe(working); h.observe(approval);
    expect(h.kinds).toEqual(['input']);
    h.observe({ ...approval });
    expect(h.kinds).toEqual(['input']);
    h.observe(working); h.observe(completed(1));
    expect(h.kinds).toEqual(['input', 'completion']);
  });
  test('does not replay a completion after attention resolves', () => {
    const h = harness();
    h.observe(working); h.observe(completed(1));
    h.observe(approval); h.observe(completed(1));
    expect(h.kinds).toEqual(['completion', 'input']);
  });
  test('never notifies existing, archived or stopped completions, including when unarchived', () => {
    const h = harness();
    h.observe(completed(1));
    h.observe(working);
    h.observe(completed(2), { latestRunStopRequested: true });
    h.observe(completed(2));
    h.observe(working);
    h.observe(completed(3), { archivedAt: at(0) });
    h.observe(completed(3));
    expect(h.kinds).toEqual([]);
    h.observe(working); h.observe(completed(4));
    expect(h.kinds).toEqual(['completion']);
  });
  test('forgets handled completions when a thread leaves the snapshot', () => {
    const h = harness();
    h.observe(completed(1));
    threadTransitions({ shell: { threads: [], projects: [] } as unknown as Shell }, null, h.gate);
    expect(h.gate.handled.size).toBe(0);
    h.forget();
    h.observe(working); h.observe(completed(1));
    expect(h.kinds).toEqual(['completion']);
  });
});
