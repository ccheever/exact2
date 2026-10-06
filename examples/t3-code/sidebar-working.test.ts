// Ported from T3 Code 1e2ecbd975 packages/client-runtime/src/state/threadInbox.test.ts
// (MIT, see LICENSE-T3). exact2: the inputs are raw V2 thread shells
// (latestRunId / latestRunRequestedAt) instead of presented shells.
import { describe, expect, it } from 'bun:test';
import type { Obj } from './domain';
import { sortWorkingThreadsBySend } from './sidebar-model';
import { partition } from './sidebar-view';
import type { T3Client } from './client';

const environmentId = 'environment-1';
function thread(id: string, extra: Obj = {}): Obj {
  return { id, environmentId, projectId: 'p1', createdAt: '2026-06-01T00:00:00.000Z', updatedAt: '2026-06-01T00:00:00.000Z',
    unsettledAt: null, latestRunId: null, status: 'idle', archivedAt: null, lineage: { relationshipToParent: null }, ...extra };
}
const running = (requestedAt: string): Obj => ({ latestRunId: 'run:wake', status: 'running', activityRunStatus: 'running', activeRunId: 'run:wake',
  latestRunRequestedAt: requestedAt, latestRunStartedAt: requestedAt, latestRunCompletedAt: null });

describe('sortWorkingThreadsBySend', () => {
  it('orders by the last message the user sent, not by later runs', () => {
    const sentFirst = thread('sent-first', {
      latestUserAuthoredMessageAt: '2026-06-01T01:00:00.000Z',
      // A wake run requested after the other thread's send.
      ...running('2026-06-01T04:00:00.000Z'),
    });
    const sentLast = thread('sent-last', { latestUserAuthoredMessageAt: '2026-06-01T02:00:00.000Z' });
    // Launched by an agent: no user message, so creation time is the send.
    const launched = thread('launched', { latestUserAuthoredMessageAt: null });
    expect(sortWorkingThreadsBySend([launched, sentFirst, sentLast]).map(item => item.id)).toEqual(['sent-last', 'sent-first', 'launched']);
  });

  // exact2 additions: the older-server fallback and the tie the reference's sortNewestFirst takes.
  it('falls back to the latest run request when the server omits the authored stamp', () => {
    const older = thread('older', running('2026-06-01T03:00:00.000Z'));
    const sent = thread('sent', { latestUserAuthoredMessageAt: '2026-06-01T02:00:00.000Z', ...running('2026-06-01T05:00:00.000Z') });
    const launched = thread('launched', { latestUserAuthoredMessageAt: null, ...running('2026-06-01T06:00:00.000Z') });
    expect(sortWorkingThreadsBySend([launched, sent, older]).map(item => item.id)).toEqual(['older', 'sent', 'launched']);
  });

  it('breaks a tie by thread id, then environment id', () => {
    const b = thread('b'), a2 = thread('a', { environmentId: 'environment-2' }), a1 = thread('a');
    expect(sortWorkingThreadsBySend([b, a2, a1]).map(item => `${item.id}@${item.environmentId}`))
      .toEqual(['a@environment-1', 'a@environment-2', 'b@environment-1']);
  });

  it('orders only the Working shelf; the active shelf keeps the return order', () => {
    const working = (id: string, sent: string, requested: string) => thread(id, { latestUserAuthoredMessageAt: sent, ...running(requested) });
    const threads = [
      working('w1', '2026-06-01T01:00:00.000Z', '2026-06-01T09:00:00.000Z'),
      working('w2', '2026-06-01T02:00:00.000Z', '2026-06-01T02:00:00.000Z'),
      working('w3', '2026-06-01T03:00:00.000Z', '2026-06-01T03:00:00.000Z'),
      thread('i1', { latestUserAuthoredMessageAt: '2026-06-01T08:00:00.000Z', latestRunId: 'r', status: 'completed', latestRunRequestedAt: '2026-06-01T01:00:00.000Z', latestRunCompletedAt: '2026-06-01T01:30:00.000Z' }),
      thread('i2', { latestUserAuthoredMessageAt: '2026-06-01T01:00:00.000Z', latestRunId: 'r', status: 'completed', latestRunRequestedAt: '2026-06-01T05:00:00.000Z', latestRunCompletedAt: '2026-06-01T05:30:00.000Z' }),
    ];
    const client = {
      shell: { projects: [{ id: 'p1', title: 'P' }], threads, sequence: 1 },
      config: { environment: { capabilities: {} }, providers: [], keybindings: [] },
      environmentId: 'env', local: { drafts: {}, clientSettings: { sidebarWorkingShelfEnabled: true } },
      projectGroups: () => [{ key: 'g1', name: 'P', members: [{ id: 'p1' }] }],
    } as unknown as T3Client;
    const parts = partition(client, Date.parse('2026-06-01T10:00:00.000Z'));
    expect(parts.working.map(item => item.id)).toEqual(['w3', 'w2', 'w1']);
    expect(parts.active.map(item => item.id)).toEqual(['i2', 'i1']);
  });
});
