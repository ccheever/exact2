// Lineage statuses at T3 Code 74541bab17 (MIT, see LICENSE-T3): a supervisor waiting on its worker
// stays running (#18603), and a member's provider-native coordinator shows its root turn (#18149).
import { describe, expect, test } from 'bun:test';
import { currentSubagent, lineageView } from '../../src/details/shell-lineage';
import { nativeParentStatus } from '../../src/details/lineage-parent';
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
  const coordinator = shell('coordinator', { title: 'Workflow coordinator', creationSource: 'provider', updatedAt: at(1),
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

  function client(nodes: Obj[], threads: Obj[] = [coordinator, member]) {
    const requests: Obj[] = [];
    const value = { environmentId: 'env', threadId: 'member', projection: memberProjection, shell: { threads },
      restAccess: () => ({ read: async (method: string, payload: Obj) => { requests.push({ method, ...payload });
        return { thread: coordinator, nodes: value.nodes }; } }), nodes };
    return { client: value as unknown as T3Client & { nodes: Obj[]; shell: { threads: Obj[] } }, requests };
  }
  const native = { available: true } as unknown as Native;
  const rootTurn = (status: string) => ({ kind: 'root_turn', runId: null, status, startedAt: at(1), completedAt: status === 'running' ? null : at(2) });

  test('the coordinator\'s projection is read once per shell change, and again every 5 s while its turn is live', async () => {
    const { client: c, requests } = client([rootTurn('running')]);
    expect(await nativeParentStatus(c, native, 'member', 0)).toEqual({ threadId: 'coordinator', status: 'running' });
    expect(await nativeParentStatus(c, native, 'member', 4_000)).toEqual({ threadId: 'coordinator', status: 'running' });
    expect(requests).toEqual([{ method: 'orchestration.getThreadProjection', threadId: 'coordinator' }]);
    c.nodes = [rootTurn('completed')];
    expect((await nativeParentStatus(c, native, 'member', 5_000))?.status).toBe('completed');
    expect((await nativeParentStatus(c, native, 'member', 60_000))?.status).toBe('completed');
    expect(requests).toHaveLength(2);
    c.nodes = [rootTurn('failed')];
    c.shell.threads = [{ ...coordinator, updatedAt: at(5) }, member];
    expect((await nativeParentStatus(c, native, 'member', 61_000))?.status).toBe('failed');
    expect(requests).toHaveLength(3);
  });

  test('only a provider-native parent counts', async () => {
    const appParent = { ...coordinator, creationSource: 'mcp' };
    const { client: c, requests } = client([rootTurn('running')], [appParent, member]);
    expect(await nativeParentStatus(c, native, 'member', 0)).toBeNull();
    expect(await nativeParentStatus(c, native, 'coordinator', 0)).toBeNull();
    expect(requests).toEqual([]);
  });
});
