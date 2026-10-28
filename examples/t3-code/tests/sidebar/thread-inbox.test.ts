// Ported from T3 Code 7c3b3b3605ef packages/client-runtime/src/state/threadInbox.test.ts createInboxReturnTracker
// (MIT, see LICENSE-T3). exact2: the inputs are raw V2 thread shells. The window's report (`advance`) stands where the
// reference reads `now()`: it starts an owed grace, stamps an owed return, and ends a grace as the timer would.
import { describe, expect, it } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import type { T3Client } from '../../src/client/client';
import { InboxReturnTracker } from '../../src/sidebar/thread-inbox';
import { WORKING_EXIT_GRACE_MS } from '../../src/shell/shell-notify';
import { sortByReturn } from '../../src/sidebar/sidebar-model';
import { partition, sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import { sidebarLocal } from '../../src/sidebar/sidebar-commands';
import type { Native } from '../../src/client/protocol';

const T0 = 10_000;
const stamp = (ms: number) => new Date(ms).toISOString();
function thread(id: string, working: boolean, extra: Obj = {}): Obj {
  return { id, environmentId: 'environment-1', projectId: 'p1', createdAt: '1970-01-01T00:00:00.000Z', updatedAt: stamp(T0), unsettledAt: null,
    archivedAt: null, lineage: { relationshipToParent: null }, interactionMode: 'default',
    ...(working ? { latestRunId: 'run-1', status: 'running', activityRunStatus: 'running', activeRunId: 'run-1' } : { latestRunId: null, status: 'completed' }),
    ...extra };
}
const tracker = () => new InboxReturnTracker(thread => `${String(thread.environmentId)}:${String(thread.id)}`);

describe('InboxReturnTracker (createInboxReturnTracker)', () => {
  it('stamps a thread when it stops working, but never on the first observation', () => {
    const inbox = tracker();
    const first = inbox.observe([thread('a', true), thread('b', false)], T0);
    expect(first.returnedAt(thread('a', true))).toBeUndefined();
    expect(first.returnedAt(thread('b', false))).toBeUndefined();
    const held = inbox.observe([thread('a', false), thread('b', false)], T0);
    inbox.advance(T0);
    inbox.advance(T0 + 2_000);
    const second = inbox.observe([thread('a', false), thread('b', false)], T0 + 2_000);
    expect(second.returnedAt(thread('a', false))).toBe(T0 + WORKING_EXIT_GRACE_MS);
    expect(second.returnedAt(thread('b', false))).toBeUndefined();
    // An earlier observation keeps its answer, so lists built from it stay valid.
    expect(held.isWorking(thread('a', false))).toBe(true);
    expect(held.returnedAt(thread('a', false))).toBeUndefined();
  });

  it('starts the grace when the window\'s clock next reads, and holds only placement until it passes', () => {
    const inbox = tracker();
    inbox.observe([thread('a', true)], T0);
    inbox.observe([thread('a', false)], T0);
    expect(inbox.isWorking(thread('a', false))).toBe(true);
    expect([inbox.owed() > 0, inbox.nextDeadline()]).toEqual([true, 0]);
    expect(inbox.advance(T0 + 400)).toBe(true);
    expect([inbox.owed(), inbox.nextDeadline()]).toEqual([0, T0 + 2_400]);
    expect(inbox.advance(T0 + 2_399)).toBe(false);
    expect(inbox.isWorking(thread('a', false))).toBe(true);
    expect(inbox.advance(T0 + 2_800)).toBe(true);
    expect(inbox.isWorking(thread('a', false))).toBe(false);
    // Its return is the grace's end, where the reference's timer fires.
    expect(inbox.returnedAt(thread('a', false))).toBe(T0 + 2_400);
    expect(inbox.nextDeadline()).toBe(0);
  });

  it('times the grace on the window\'s clock alone: a server clock a minute ahead does not lengthen it', () => {
    const inbox = tracker();
    inbox.observe([thread('a', true)], T0);
    inbox.observe([thread('a', false, { updatedAt: stamp(T0 + 60_000), latestRunId: 'run-1', latestRunCompletedAt: stamp(T0 + 60_000) })], T0);
    inbox.advance(T0 + 300);
    expect(inbox.nextDeadline()).toBe(T0 + 2_300);
    expect(inbox.advance(T0 + 2_300)).toBe(true);
    expect(inbox.isWorking(thread('a', false))).toBe(false);
  });

  it('a snapshot clock that moved on starts an owed grace when no window reports one', () => {
    const inbox = tracker();
    inbox.observe([thread('a', true)], T0);
    inbox.observe([thread('a', false)], T0);
    inbox.observe([thread('a', false)], T0 + 60_000);
    expect(inbox.nextDeadline()).toBe(T0 + 62_000);
    inbox.observe([thread('a', false)], T0 + 120_000);
    expect(inbox.isWorking(thread('a', false))).toBe(false);
    expect(inbox.returnedAt(thread('a', false))).toBe(T0 + 62_000);
  });

  it('cancels a transient return on resume and grants a fresh grace after the next finish', () => {
    const inbox = tracker();
    inbox.observe([thread('a', false)], T0);
    expect(inbox.isWorking(thread('a', false))).toBe(false);
    inbox.observe([thread('a', true)], T0);
    inbox.observe([thread('a', false)], T0);
    inbox.advance(T0);
    inbox.observe([thread('a', true)], T0 + 1_500);
    expect(inbox.advance(T0 + 2_500)).toBe(false);
    expect(inbox.returnedAt(thread('a', true))).toBeUndefined();
    inbox.observe([thread('a', false)], T0 + 2_500);
    inbox.advance(T0 + 2_500);
    expect(inbox.advance(T0 + 4_499)).toBe(false);
    expect(inbox.advance(T0 + 4_500)).toBe(true);
    expect(inbox.returnedAt(thread('a', false))).toBe(T0 + 4_500);
  });

  it.each([
    ['an approval', { pendingRuntimeRequest: { kind: 'approval' } }],
    ['a question', { pendingRuntimeRequest: { kind: 'user_input' } }],
    ['a guest message', { pendingGuestMessageCount: 1 }],
    ['a plan to act on', { interactionMode: 'plan', hasActionableProposedPlan: true }],
    ['a failed run', { latestRunId: 'run-1', status: 'failed', lastError: 'failed' }],
    ['a pin', { pinnedAt: '1970-01-01T00:00:01Z' }],
    ['settling', { settledOverride: 'settled' }],
    ['a snooze', { snoozedUntil: '1970-01-02T00:00:00Z' }],
    ['a manual move', { activeOrderKey: 'manual-order' }],
  ])('ends a grace at once for %s, its return stamped by the window\'s clock', (_label, change) => {
    const inbox = tracker();
    inbox.observe([thread('a', true)], T0);
    inbox.observe([thread('a', false)], T0);
    inbox.observe([thread('a', false, change as Obj)], T0);
    expect(inbox.isWorking(thread('a', false))).toBe(false);
    expect(inbox.nextDeadline()).toBe(0);
    inbox.advance(T0 + 700);
    expect(inbox.returnedAt(thread('a', false))).toBe(T0 + 700);
  });

  it('an approval seen on a lagging snapshot clock heads the inbox at once, then takes the window\'s time', () => {
    const inbox = tracker();
    const lagging = T0 - 50_000; // the snapshot clock, read once a minute
    const fresh = thread('fresh', false, { createdAt: stamp(T0 + 3_000), updatedAt: stamp(T0 + 3_000) });
    inbox.observe([thread('b', true), fresh], lagging);
    const asks = thread('b', true, { status: 'waiting', activityRunStatus: 'waiting', pendingRuntimeRequest: { kind: 'approval' }, updatedAt: stamp(T0 + 10_000) });
    const seen = inbox.observe([asks, fresh], lagging);
    expect(sortByReturn([fresh, asks], seen.returnedAt).map(item => item.id)).toEqual(['b', 'fresh']);
    inbox.advance(T0 + 10_400);
    expect(inbox.returnedAt(asks)).toBe(T0 + 10_400);
    expect(sortByReturn([fresh, asks], item => inbox.returnedAt(item)).map(item => item.id)).toEqual(['b', 'fresh']);
  });

  it('grants grace after a retained expired snooze, not to a future one', () => {
    const inbox = tracker();
    const expired = { snoozedAt: '1970-01-01T00:00:01.000Z', snoozedUntil: '1970-01-01T00:00:05.000Z' };
    inbox.observe([thread('a', true, expired)], T0);
    inbox.observe([thread('a', false, expired)], T0);
    expect(inbox.isWorking(thread('a', false, expired))).toBe(true);
    const future = { snoozedAt: '1970-01-01T00:00:01.000Z', snoozedUntil: '1970-01-01T00:00:30.000Z' };
    const other = tracker();
    other.observe([thread('b', true, future)], T0);
    other.observe([thread('b', false, future)], T0);
    expect(other.isWorking(thread('b', false, future))).toBe(false);
    expect(other.nextDeadline()).toBe(0);
  });

  it('stamps only stable returns and sorts them in their actual return order', () => {
    const inbox = tracker();
    inbox.observe([thread('a', true), thread('b', true)], T0);
    inbox.observe([thread('a', false), thread('b', true)], T0);
    inbox.advance(T0);
    inbox.observe([thread('a', false), thread('b', false)], T0 + 500);
    inbox.advance(T0 + 500);
    inbox.advance(T0 + 2_000);
    expect(inbox.returnedAt(thread('a', false))).toBe(T0 + 2_000);
    expect(inbox.returnedAt(thread('b', false))).toBeUndefined();
    inbox.advance(T0 + 2_500);
    expect(sortByReturn([thread('a', false), thread('b', false)], item => inbox.returnedAt(item)).map(item => item.id)).toEqual(['b', 'a']);
  });

  it('forgets removed threads and every grace while the shelf is off', () => {
    const inbox = tracker();
    inbox.observe([thread('a', true)], T0);
    inbox.observe([thread('a', false)], T0);
    inbox.observe([], T0);
    expect([inbox.nextDeadline(), inbox.owed()]).toEqual([0, 0]);
    inbox.observe([thread('a', true)], T0);
    inbox.observe([thread('a', false)], T0);
    inbox.advance(T0);
    inbox.observe(null, T0);
    expect([inbox.nextDeadline(), inbox.owed()]).toEqual([0, 0]);
    expect(inbox.returnedAt(thread('a', false))).toBeUndefined();
  });
});

describe('the Working shelf over a grace', () => {
  const window = (threads: Obj[]) => ({
    shell: { projects: [{ id: 'p1', title: 'P' }], threads, sequence: 1 },
    config: { environment: { capabilities: { threadSettlement: true } }, providers: [], keybindings: [] },
    environmentId: 'environment-1', threadId: '', query: '', presentation: {}, local: { drafts: {}, clientSettings: { sidebarWorkingShelfEnabled: true }, deviceSettings: {} },
    projectGroups: () => [{ key: 'g1', name: 'P', members: [{ id: 'p1' }] }],
  }) as unknown as T3Client;
  const helpers = { projectIdentity: () => ({ projectMark: '', projectInk: '', projectSurface: '' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
  const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;

  it('keeps a finished thread under Working for 2 s, then it heads the inbox; one settled meanwhile goes straight to Settled', async () => {
    const older = thread('older', false, { latestRunId: 'r0', status: 'completed', latestRunCompletedAt: stamp(T0 - 1_000) });
    const client = window([thread('a', true), thread('b', true), older]);
    partition(client, T0);
    client.shell.threads = [thread('a', false), thread('b', false), older];
    let parts = partition(client, T0);
    expect(parts.working.map(item => item.id)).toEqual(['a', 'b']);
    expect(parts.active.map(item => item.id)).toEqual(['older']);
    // The root's timer reads the window's clock for both graces at once (app.contract workingGraceOwed); the grace
    // itself never keeps the window's clock ticking.
    expect(sidebarSnapshot(client, T0, helpers).sidebar).toMatchObject({ graceUntil: 0, ticking: false, workingCount: 2 });
    expect(sidebarSnapshot(client, T0, helpers).sidebar.graceOwed).toBeGreaterThan(0);
    await sidebarLocal(client, native, 'working-grace', '', String(T0));
    expect(sidebarSnapshot(client, T0, helpers).sidebar).toMatchObject({ graceUntil: T0 + 2_000, graceOwed: 0, ticking: false, workingCount: 2 });
    // b settles within the grace: straight to Settled, never in the inbox.
    client.shell.threads = [thread('a', false), thread('b', false, { settledOverride: 'settled', settledAt: stamp(T0 + 1_000) }), older];
    parts = partition(client, T0 + 1_000);
    expect(parts.settled.map(item => item.id)).toEqual(['b']);
    expect(parts.working.map(item => item.id)).toEqual(['a']);
    expect(parts.active.map(item => item.id)).toEqual(['older']);
    // The window's clock passes the grace: a heads the inbox.
    await sidebarLocal(client, native, 'working-grace', '', String(T0 + 2_000));
    parts = partition(client, T0);
    expect(parts.working).toEqual([]);
    expect(parts.active.map(item => item.id)).toEqual(['a', 'older']);
    expect(sidebarSnapshot(client, T0, helpers).sidebar).toMatchObject({ graceUntil: 0, ticking: false, workingCount: 0 });
  });
});
