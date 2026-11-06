// Lineage statuses at T3 Code 74541bab17 (MIT, see LICENSE-T3): a supervisor waiting on its worker
// stays running (#18603), and a member's provider-native coordinator shows its root turn from its
// thread stream, subscribed while the member is shown (#18149).
import { describe, expect, test } from 'bun:test';
import { currentSubagent, lineageView } from '../../src/details/shell-lineage';
import { nativeParentStatus, releaseNativeParent } from '../../src/details/lineage-parent';
import { keepAliveEvent, KEEP_PREFIX } from '../../src/client/keep-alive';
import { initialShell } from '../../src/client/domain';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';

const at = (minutes: number) => new Date(Date.UTC(2026, 9, 21, 10, minutes)).toISOString();
const shell = (id: string, extra: Obj = {}): Obj => ({ id, title: `T ${id}`, status: 'idle', createdAt: at(0), lineage: { parentThreadId: null, relationshipToParent: null }, ...extra });
const now = Date.parse(at(10));

describe('a supervisor waiting on its worker', () => {
  const task = { childThreadId: 'kid', status: 'waiting', title: 'supervise', startedAt: at(1), progress: 'waiting on worker' };
  const kid = shell('kid', { status: 'completed', latestRunId: 'r1', latestRunRequestedAt: at(2), latestRunStartedAt: at(2), latestRunCompletedAt: at(3),
    lineage: { parentThreadId: 'me', relationshipToParent: 'subagent' } });
  const projection = { thread: { id: 'me', lineage: {} }, runs: [], contextTransfers: [], subagents: [task] };

  test('keeps the task\'s own status, timer and progress while the task is at work', () => {
    expect(currentSubagent(task, kid)).toBe(task);
    const row = lineageView([shell('me'), kid], projection, 'me', [], now).lineage.find(entry => entry.id === 'kid');
    expect(row).toMatchObject({ status: 'Waiting', group: 'active', elapsed: '9m' });
  });

  test('a live follow-up run on the child still wins; a settled task follows its child\'s newer run', () => {
    const live = { ...kid, activityRunStatus: 'running', activityRunStartedAt: at(8) };
    expect(lineageView([shell('me'), live], projection, 'me', [], now).lineage.find(entry => entry.id === 'kid')).toMatchObject({ status: 'Running' });
    const settled = { ...projection, subagents: [{ ...task, status: 'completed', completedAt: at(1) }] };
    expect(lineageView([shell('me'), kid], settled, 'me', [], now).lineage.find(entry => entry.id === 'kid')).toMatchObject({ status: 'Done', group: 'previous' });
  });
});

describe('a member of a native coordinator', () => {
  const coordinator = shell('coordinator', { title: 'Workflow coordinator', creationSource: 'provider',
    lineage: { parentThreadId: 'parent', relationshipToParent: 'subagent' } });
  const member = shell('member', { title: 'Reviewer', creationSource: 'provider', lineage: { parentThreadId: 'coordinator', relationshipToParent: 'subagent' } });
  const memberProjection = { thread: member, runs: [], contextTransfers: [], subagents: [] };
  const parentRow = (threads: Obj[], status: string) =>
    lineageView(threads, memberProjection, 'member', [], now, [], [], [], true, { threadId: 'coordinator', status }).lineage.find(row => row.id === 'coordinator');

  test('the parent row reads the coordinator\'s root turn; a live follow-up run on it takes precedence', () => {
    expect(parentRow([coordinator, member], '')).toMatchObject({ status: 'Idle', icon: 'corner-left-up' });
    expect(parentRow([coordinator, member], 'running')).toMatchObject({ status: 'Running' });
    expect(parentRow([coordinator, member], 'completed')).toMatchObject({ status: 'Done' });
    expect(parentRow([{ ...coordinator, activityRunStatus: 'running' }, member], 'completed')).toMatchObject({ status: 'Running' });
  });

  const ARRAYS = ['runs', 'attempts', 'nodes', 'subagents', 'providerSessions', 'providerThreads', 'providerTurns', 'runtimeRequests', 'messages', 'plans', 'turnItems',
    'checkpointScopes', 'checkpoints', 'contextHandoffs', 'contextTransfers', 'visibleTurnItems'];
  const rootTurn = (status: string) => ({ id: 'root', kind: 'root_turn', runId: null, status, startedAt: at(1), completedAt: status === 'running' ? null : at(2) });
  const coordinatorProjection = (status: string) => ({ ...Object.fromEntries(ARRAYS.map(name => [name, []])), thread: coordinator, nodes: [rootTurn(status)], updatedAt: at(3) });
  const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;

  /** A focused client on a transport double: the bounded snapshot over HTTP, subscription ids, and every request. */
  function client(threads: Obj[] = [coordinator, member], status = 'running') {
    const calls: Obj[] = [];
    const call = async (request: Obj) => {
      calls.push(request);
      if (request.op === 'http') return { snapshotSequence: 7, projection: coordinatorProjection(status), hasMoreHistory: false };
      return request.op === 'subscribe' ? { id: `9-${calls.length}` } : {};
    };
    const value = { environmentId: 'env', origin: 'https://box.example.com', generation: 9, ready: true, threadId: 'member', thread: null, projection: memberProjection,
      shell: { ...initialShell(), threads }, restAccess: () => ({ call }) };
    return { client: value as unknown as T3Client & { shell: { threads: Obj[] }; threadId: string }, calls };
  }
  const keepCalls = (calls: Obj[]) => calls.filter(request => String(request.key).startsWith(KEEP_PREFIX)).map(request => [request.op, request.key]);

  test('the coordinator\'s thread stream is subscribed while its member is shown, and its root turn moves the row', async () => {
    const { client: c, calls } = client();
    expect(await nativeParentStatus(c, native, 'member')).toEqual({ threadId: 'coordinator', status: 'running' });
    expect(calls.find(request => request.op === 'subscribe')).toMatchObject({ key: `${KEEP_PREFIX}coordinator`, method: 'orchestration.subscribeThread',
      payload: { threadId: 'coordinator', afterSequence: 7, acceptBoundedSnapshot: true } });
    // A second read subscribes nothing new; the stream's next snapshot carries the settled turn.
    expect(await nativeParentStatus(c, native, 'member')).toEqual({ threadId: 'coordinator', status: 'running' });
    expect(keepCalls(calls)).toEqual([['subscribe', `${KEEP_PREFIX}coordinator`]]);
    const id = `9-${calls.findIndex(request => request.op === 'subscribe') + 1}`;
    keepAliveEvent(c, { key: `${KEEP_PREFIX}coordinator`, generation: 9, subscriptionId: id,
      value: { kind: 'snapshot', snapshotSequence: 8, projection: coordinatorProjection('completed'), hasMoreHistory: false } });
    expect((await nativeParentStatus(c, native, 'member'))?.status).toBe('completed');
  });

  test('closing the details, or showing another thread, releases the stream', async () => {
    const closed = client();
    await nativeParentStatus(closed.client, native, 'member');
    await releaseNativeParent(closed.client, native);
    expect(keepCalls(closed.calls)).toEqual([['subscribe', `${KEEP_PREFIX}coordinator`], ['unsubscribe', `${KEEP_PREFIX}coordinator`]]);
    await releaseNativeParent(closed.client, native);
    expect(keepCalls(closed.calls)).toHaveLength(2);
    const moved = client([coordinator, member, shell('other')]);
    await nativeParentStatus(moved.client, native, 'member');
    expect(await nativeParentStatus(moved.client, native, 'other')).toBeNull();
    expect(keepCalls(moved.calls)).toEqual([['subscribe', `${KEEP_PREFIX}coordinator`], ['unsubscribe', `${KEEP_PREFIX}coordinator`]]);
  });

  test('only a provider-native parent counts', async () => {
    const { client: c, calls } = client([{ ...coordinator, creationSource: 'mcp' }, member]);
    expect(await nativeParentStatus(c, native, 'member')).toBeNull();
    expect(await nativeParentStatus(c, native, 'coordinator')).toBeNull();
    expect(keepCalls(calls)).toEqual([]);
  });
});
