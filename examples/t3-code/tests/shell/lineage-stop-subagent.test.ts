// Lineage's Stop (ThreadRelationshipsPanel stopSubagent, T3 Code 74541bab17, MIT, see LICENSE-T3):
// interruptThreadTurn({ threadId: owner, subagentId }) reads the owner's fresh projection; an app-owned task gets the
// parent's best-effort subagent.stop, then its child thread's active run is interrupted with the queue held (or
// thread.stop when the server offers it); a refusal is the toast "Could not stop subagent", not the transcript banner.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { ClientError, type Files, type Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { shellCommand } from '../../src/shell/shell-commands';
import { contractComponent } from '../support/contract-action-fixture';
import { toasts } from '../../src/shell/toast';

const native = { available: true } as unknown as Native;
const files = {} as Files;
const task: Obj = { id: 'task', threadId: 'parent', childThreadId: 'kid', runId: 'parent-run', origin: 'app_owned', status: 'running', startedAt: '2026-10-21T12:00:00.000Z' };

function fixture(child: Obj, options: { refuse?: 'all' | 'subagent'; background?: boolean; threadStop?: boolean; fresh?: Obj | null } = {}) {
  const requests: [string, Obj][] = [], dispatched: Obj[] = [];
  const owner = { subagents: options.fresh === null ? [] : [options.fresh ?? task], runs: [{ id: 'parent-run', status: 'running' }] };
  const client = {
    threadId: 'parent', projection: { runs: [], subagents: [task] }, error: '', generation: 1, threadStopOffered: options.threadStop === true,
    shell: { projects: [], threads: [{ id: 'parent' }, { id: 'kid', pendingBackgroundTasks: options.background ? [{ id: 'bg' }] : [] }] },
    restAccess: () => ({
      request: async (method: string, payload: Obj) => { requests.push([method, payload]); return payload.threadId === 'parent' ? owner : child; },
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `cmd-${dispatched.length + index + 1}`),
      dispatch: async (_storage: Files, payload: Obj) => {
        dispatched.push(payload);
        if (options.refuse === 'all' || (options.refuse === 'subagent' && payload.type === 'subagent.stop')) {
          client.error = 'Run is not active.'; throw new ClientError('Run is not active.', 'server');
        }
        return {};
      },
    }),
  };
  return { client: client as unknown as T3Client & { error: string }, requests, dispatched };
}

const parentStop = { type: 'subagent.stop', commandId: 'cmd-1', threadId: 'parent', runId: 'parent-run', subagentId: 'task' };

describe('Stop subagent', () => {
  test('the parent stops the task, then the child thread\'s active run is interrupted with its queue held', async () => {
    const { client, requests, dispatched } = fixture({ runs: [{ id: 'r1', status: 'completed' }, { id: 'r2', status: 'running' }, { id: 'r3', status: 'queued' }] });
    expect(await shellCommand(client, native, files, 'stop-subagent', 'kid', '')).toBe('');
    expect(requests).toEqual([['orchestration.getThreadProjection', { threadId: 'parent' }], ['orchestration.getThreadProjection', { threadId: 'kid' }]]);
    expect(dispatched).toEqual([parentStop, { type: 'run.interrupt', commandId: 'cmd-2', threadId: 'kid', runId: 'r2', holdQueue: true }]);
    expect(toasts(client)).toEqual([]);
  });

  test('an idle orphaned follow-up: the parent\'s refusal is ignored and the child still stops', async () => {
    const { client, dispatched } = fixture({ runs: [{ id: 'r2', status: 'queued' }, { id: 'r3', status: 'running' }] }, { refuse: 'subagent' });
    await shellCommand(client, native, files, 'stop-subagent', 'kid', '');
    expect(dispatched.map(payload => [payload.type, payload.threadId])).toEqual([['subagent.stop', 'parent'], ['run.interrupt', 'kid']]);
    expect(toasts(client)).toEqual([]);
    expect(client.error).toBe('');
  });

  test('with threadStop the child gets thread.stop; a task without a parent run skips the parent stop', async () => {
    const offered = fixture({ runs: [] }, { threadStop: true });
    await shellCommand(offered.client, native, files, 'stop-subagent', 'kid', '');
    expect(offered.dispatched).toEqual([parentStop, { type: 'thread.stop', commandId: 'cmd-2', threadId: 'kid' }]);
    const runless = fixture({ runs: [] }, { threadStop: true, fresh: { ...task, runId: null } });
    await shellCommand(runless.client, native, files, 'stop-subagent', 'kid', '');
    expect(runless.dispatched).toEqual([{ type: 'thread.stop', commandId: 'cmd-1', threadId: 'kid' }]);
    const gone = fixture({ runs: [{ id: 'r2', status: 'running' }] }, { fresh: null });
    await shellCommand(gone.client, native, files, 'stop-subagent', 'kid', '');
    expect(gone.dispatched).toEqual([]);
  });

  test('with no active run, the latest run while it owns background work; otherwise only the parent stop', async () => {
    const settled = { runs: [{ id: 'r1', status: 'completed' }, { id: 'r2', status: 'completed' }] };
    const background = fixture(settled, { background: true });
    await shellCommand(background.client, native, files, 'stop-subagent', 'kid', '');
    expect(background.dispatched.map(payload => payload.runId)).toEqual(['parent-run', 'r2']);
    const idle = fixture(settled);
    await shellCommand(idle.client, native, files, 'stop-subagent', 'kid', '');
    expect(idle.dispatched).toEqual([parentStop]);
  });

  test('with no run to interrupt, the child\'s pull request watches are turned off (a dismissed or unwatched link is left)', async () => {
    const link = (number: number, extra: Obj = {}) => ({ host: 'github.com', repository: 'o/r', number, watch: { since: 'x' }, ...extra });
    const { client, dispatched } = fixture({ runs: [{ id: 'r1', status: 'completed' }], thread: { pullRequests: [link(1), link(2, { watch: undefined }), link(3, { source: 'stack-dismissed' })] } });
    await shellCommand(client, native, files, 'stop-subagent', 'kid', '');
    expect(dispatched).toEqual([parentStop,
      { type: 'thread.pull-request.watch', commandId: 'cmd-2', threadId: 'kid', host: 'github.com', repository: 'o/r', number: 1, watching: false }]);
  });

  test('a refusal of the child stop is the toast, without a description or the transcript banner', async () => {
    const { client } = fixture({ runs: [{ id: 'r2', status: 'running' }] }, { refuse: 'all' });
    expect(await shellCommand(client, native, files, 'stop-subagent', 'kid', '')).toBe('');
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Could not stop subagent', '']]);
    expect(client.error).toBe('');
  });
});

test('Lineage Stop appears for a hovered or focused owned agent and gates repeated commands', () => {
  const row = contractComponent('shell-details.contract', 'LineageRow');
  const scope = { row: { id: 'kid', canStop: true }, hovered: '', focused: '' };
  expect(row.derives({}, scope).stopShown).toBe(false);
  expect(row.derives({}, { ...scope, hovered: 'kid' }).stopShown).toBe(true);
  expect(row.derives({}, { ...scope, hovered: 'stop-kid' }).stopShown).toBe(true);
  expect(row.derives({}, { ...scope, focused: 'kid' }).stopShown).toBe(true);
  expect(row.derives({}, { ...scope, row: { id: 'kid', canStop: false }, hovered: 'kid' }).stopShown).toBe(false);
  const groups = contractComponent('shell-details.contract', 'LineageGroups'), state = groups.state();
  const calls: unknown[][] = [], act = (...args: unknown[]) => calls.push(args);
  groups.call('stop', state, { busy: false, act }, ['shell:stop-subagent', 'kid']);
  expect(calls).toEqual([['shell:stop-subagent', 'kid', '']]);
  groups.call('stop', state, { busy: true, act }, ['shell:stop-workflow', 'other']);
  expect(calls).toHaveLength(1);
});
