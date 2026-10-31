// Stopping a run is not finishing it (ThreadNotificationCoordinator, T3 Code 74541bab17): a run that completes
// after an interrupt request (the shell's latestRunStopRequested) posts no "Thread completed", and the next run
// that completes on its own does.
import { describe, expect, test } from 'bun:test';
import type { Obj, Shell } from '../../src/client/domain';
import { threadTransitions } from '../../src/shell/shell-notify';

const NOW = Date.parse('2026-10-21T12:00:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const thread = (extra: Obj = {}): Obj => ({ id: 't', projectId: 'p1', title: 'Watch the deploy', status: 'running', latestRunId: 'r1',
  activeProviderThreadId: null, pendingRuntimeRequest: null, createdAt: iso(-3_600_000), updatedAt: iso(-60_000), archivedAt: null,
  settledOverride: null, settledAt: null, modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });
const environment = (threads: Obj[]) => ({ shell: { projects: [], threads, sequence: 1 } as unknown as Shell });

describe('a stopped run (latestRunStopRequested)', () => {
  test('completes without a notification; a later run that finishes on its own notifies', () => {
    const first = threadTransitions(environment([thread()]), null);
    const stopped = threadTransitions(environment([thread({ status: 'completed', latestRunCompletedAt: iso(0), latestRunStopRequested: true })]), first.next);
    expect(stopped.transitions).toEqual([]);
    const running = threadTransitions(environment([thread({ latestRunId: 'r2', latestRunStopRequested: false })]), stopped.next);
    const finished = threadTransitions(environment([thread({ latestRunId: 'r2', status: 'completed', latestRunCompletedAt: iso(60_000), latestRunStopRequested: false })]), running.next);
    expect(finished.transitions).toEqual([expect.objectContaining({ threadId: 't', kind: 'completion', title: 'Thread completed' })]);
  });
  test('an older server that sends no flag still notifies', () => {
    const first = threadTransitions(environment([thread()]), null);
    const done = threadTransitions(environment([thread({ status: 'completed', latestRunCompletedAt: iso(0) })]), first.next);
    expect(done.transitions.map(transition => transition.kind)).toEqual(['completion']);
  });
});
