// Lineage's Stop (ThreadRelationshipsPanel stopSubagent, T3 Code 9c161462c9, MIT, see LICENSE-T3):
// interruptThreadTurn({ threadId: child }) reads the child thread's projection and interrupts its active run with
// the queue held; a refusal is the toast "Could not stop subagent", not the transcript banner.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { ClientError, type Files, type Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { shellCommand } from '../../src/shell/shell-commands';
import { contractComponent } from '../support/contract-action-fixture';
import { toasts } from '../../src/shell/toast';

const native = { available: true } as unknown as Native;
const files = {} as Files;

function fixture(projection: Obj, options: { refuse?: boolean; background?: boolean } = {}) {
  const requests: [string, Obj][] = [], dispatched: Obj[] = [];
  const client = {
    threadId: 'parent', projection: { runs: [] }, error: '', generation: 1,
    shell: { projects: [], threads: [{ id: 'parent' }, { id: 'kid', pendingBackgroundTasks: options.background ? [{ id: 'task' }] : [] }] },
    restAccess: () => ({
      request: async (method: string, payload: Obj) => { requests.push([method, payload]); return projection; },
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `cmd-${index + 1}`),
      dispatch: async (_storage: Files, payload: Obj) => {
        dispatched.push(payload);
        if (options.refuse) { client.error = 'Run is not active.'; throw new ClientError('Run is not active.', 'server'); }
        return {};
      },
    }),
  };
  return { client: client as unknown as T3Client & { error: string }, requests, dispatched };
}

describe('Stop subagent', () => {
  test('interrupts the child thread\'s active run, holding its queue', async () => {
    const { client, requests, dispatched } = fixture({ runs: [{ id: 'r1', status: 'completed' }, { id: 'r2', status: 'running' }, { id: 'r3', status: 'queued' }] });
    expect(await shellCommand(client, native, files, 'stop-subagent', 'kid', '')).toBe('');
    expect(requests).toEqual([['orchestration.getThreadProjection', { threadId: 'kid' }]]);
    expect(dispatched).toEqual([{ type: 'run.interrupt', commandId: 'cmd-1', threadId: 'kid', runId: 'r2', holdQueue: true }]);
    expect(toasts(client)).toEqual([]);
  });

  test('with no active run, the latest run while it owns background work; otherwise nothing', async () => {
    const settled = { runs: [{ id: 'r1', status: 'completed' }, { id: 'r2', status: 'completed' }] };
    const background = fixture(settled, { background: true });
    await shellCommand(background.client, native, files, 'stop-subagent', 'kid', '');
    expect(background.dispatched.map(payload => payload.runId)).toEqual(['r2']);
    const idle = fixture(settled);
    await shellCommand(idle.client, native, files, 'stop-subagent', 'kid', '');
    expect(idle.dispatched).toEqual([]);
  });

  test('a refusal is the toast, without a description or the transcript banner', async () => {
    const { client } = fixture({ runs: [{ id: 'r2', status: 'running' }] }, { refuse: true });
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
  groups.call('stop', state, { busy: false, act }, ['kid']);
  expect(calls).toEqual([['shell:stop-subagent', 'kid', '']]);
  groups.call('stop', state, { busy: true, act }, ['other']);
  expect(calls).toHaveLength(1);
});
