// Stop without a named run (T3 Code 74541bab17, MIT, see LICENSE-T3: packages/client-runtime/src/operations/
// commands.ts interruptThreadTurn): a server advertising capabilities.threadStop gets `thread.stop`, which resolves
// the run itself; an older server still gets `run.interrupt` on the run the projection shows.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { composerWrites } from '../../src/client/client-ops-composer';
import type { OpOut } from '../../src/client/client-ops';
import type { Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { composerCommand } from '../../src/composer/composer-controls-commands';
import { shellCommand } from '../../src/shell/shell-commands';

const native = { available: true } as unknown as Native, files = {} as Files;
const running = { runs: [{ id: 'r1', status: 'completed' }, { id: 'r2', status: 'running' }], thread: {} };

function fixture(threadStop: boolean, projection: Obj = running) {
  const dispatched: Obj[] = [], requests: string[] = [];
  let ids = 0;
  const access = {
    ids: async (count: number) => Array.from({ length: count }, () => `cmd-${++ids}`),
    dispatch: async (_storage: Files, payload: Obj) => { dispatched.push(payload); return {}; },
    request: async (method: string) => { requests.push(method); return projection; },
  };
  const client = {
    threadId: 'thread', thread: { projection }, projection, threadStopOffered: threadStop, writable: true, error: '',
    shell: { projects: [], threads: [{ id: 'thread' }, { id: 'kid' }] },
    ids: async (_native: Native, count: number) => access.ids(count),
    dispatch: async (_native: Native, storage: Files, payload: Obj) => access.dispatch(storage, payload),
    restAccess: () => access,
  };
  return { client: client as unknown as T3Client, dispatched, requests };
}

const stop = (client: T3Client) => composerWrites.call(client, 'stop', '', '', 0, native, files, { message: '', id: '', value: '' } as OpOut);

describe('Stop', () => {
  test('sends thread.stop when the server advertises threadStop', async () => {
    const { client, dispatched } = fixture(true);
    expect(await stop(client)).toBe(true);
    expect(dispatched).toEqual([{ type: 'thread.stop', commandId: 'cmd-1', threadId: 'thread' }]);
  });

  test('a thread whose run the projection does not show still stops', async () => {
    const { client, dispatched } = fixture(true, { runs: [], thread: {} });
    await stop(client);
    expect(dispatched.map(payload => payload.type)).toEqual(['thread.stop']);
  });

  test('an older server gets run.interrupt on the active run', async () => {
    const { client, dispatched } = fixture(false);
    await stop(client);
    expect(dispatched).toEqual([{ type: 'run.interrupt', commandId: 'cmd-1', threadId: 'thread', runId: 'r2', holdQueue: true }]);
  });

  test('Stop background work follows the same capability', async () => {
    const current = fixture(true, { runs: [], thread: { pullRequests: [{ host: 'github.com', repository: 'o/r', number: 1, watch: {} }] } });
    await composerCommand(current.client, native, files, 'stop-background', '', '');
    expect(current.dispatched).toEqual([{ type: 'thread.stop', commandId: 'cmd-1', threadId: 'thread' }]);
    const older = fixture(false);
    await composerCommand(older.client, native, files, 'stop-background', '', '');
    expect(older.dispatched.map(payload => payload.type)).toEqual(['run.interrupt']);
  });

  test('Lineage Stop reads only the owner\'s projection, then sends thread.stop to the child after the parent\'s stop', async () => {
    const task = { id: 'task', threadId: 'thread', childThreadId: 'kid', runId: 'r2', origin: 'app_owned', status: 'running' };
    const { client, dispatched, requests } = fixture(true, { ...running, subagents: [task] });
    await shellCommand(client, native, files, 'stop-subagent', 'kid', '');
    expect(requests).toEqual(['orchestration.getThreadProjection']);
    expect(dispatched).toEqual([{ type: 'subagent.stop', commandId: 'cmd-1', threadId: 'thread', runId: 'r2', subagentId: 'task' },
      { type: 'thread.stop', commandId: 'cmd-2', threadId: 'kid' }]);
  });
});

describe('an uncertain thread.stop', () => {
  // T3Client.reconcilePending on a stand-in client: what counts as the stop having reached the server.
  function reconciles(runs: Obj[], shellThread: Obj): boolean {
    const pending = { method: 'orchestration.dispatchCommand', threadId: 'thread', payload: { type: 'thread.stop', commandId: 'c', threadId: 'thread' } };
    let finished = false;
    const client = { pending, threadId: 'thread', threadLive: true, projection: { runs }, shell: { projects: [], threads: [{ id: 'thread', ...shellThread }] }, finishPending: () => { finished = true; } };
    return T3Client.prototype.reconcilePending.call(client as never) && finished;
  }
  test('is done once no run is active and no background work is left', () => {
    expect(reconciles([{ id: 'r', status: 'completed' }], {})).toBe(true);
    expect(reconciles([{ id: 'r', status: 'running' }], {})).toBe(false);
  });
  test('with background work still pending it is retried, unless the server marked the stop', () => {
    expect(reconciles([{ id: 'r', status: 'completed' }], { pendingBackgroundTasks: [{ id: 't', kind: 'subagent' }] })).toBe(false);
    expect(reconciles([{ id: 'r', status: 'completed' }], { pendingBackgroundTasks: [{ id: 't', kind: 'subagent' }], latestRunStopRequested: true })).toBe(true);
  });
});
