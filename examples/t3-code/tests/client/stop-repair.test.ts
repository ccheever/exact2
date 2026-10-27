// Stop settles background rows the server already finished (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3:
// packages/client-runtime/src/state/threadCommands.ts interruptTurn, state/threads.ts reconcilePendingWork): after
// any Stop the open thread's active background items are read with getTurnItem, four at a time with 10 s each,
// and those the server settled replace their local copies; an event applied meanwhile wins.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { arr, obj, str, type Obj } from '../../src/client/domain';
import { Backend, storage } from '../support/client-fixture';

const item = (id: string, type: string, status: string, extra: Obj = {}): Obj => ({ id, threadId: 't1', runId: 'r1', nodeId: 'n', ordinal: Number(id.replace(/\D/g, '')) || 1, type, status,
  ...(type === 'command_execution' ? { input: `sleep ${id}` } : {}), ...(type === 'dynamic_tool' ? { toolName: 'watch' } : {}), ...extra });

/** getTurnItem replies join the inbox under their `deliver` key, as the transport files them. */
class RepairBackend extends Backend {
  server: Record<string, Obj> = {};
  lookups: Obj[] = [];
  /** Lookups held back until the test releases them. */
  held = false;
  private pending: Obj[] = [];
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op !== 'request' || request.method !== 'orchestration.getTurnItem') return super.later(input);
    this.calls.push(request); this.lookups.push(request);
    if (this.held) this.pending.push(request); else this.answer(request);
    return this.good({ id: `rpc-${this.lookups.length}` });
  }
  answer(request: Obj) {
    const itemId = str(obj(request.payload).itemId);
    this.events.push({ seq: ++this.eventSequence, generation: this.generation, key: str(request.deliver), value: { _reply: { item: this.server[itemId] ?? null } } });
  }
  release() { this.held = false; for (const request of this.pending.splice(0)) this.answer(request); }
}

async function open(local: Obj[], runs: Obj[] = [{ id: 'r1', status: 'completed' }]) {
  const native = new RepairBackend(), disk = storage(), client = new T3Client();
  const projection = obj(native.details.t1!.projection);
  Object.assign(projection, { runs, turnItems: local,
    visibleTurnItems: local.map((entry, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: entry.id, item: entry })) });
  obj(obj(native.config.environment).capabilities).threadStop = true;
  await client.refresh(native, disk.files);
  await client.command('select-thread', 't1', '', 0, native, disk.files);
  await client.refresh(native, disk.files);
  const command = (op: string, id = '', value = '') => client.command(op, id, value, 0, native, disk.files);
  const refresh = () => client.refresh(native, disk.files);
  const status = (id: string) => str(arr(client.projection.turnItems).find(entry => entry.id === id)?.status);
  const rowStatus = (id: string) => str(obj(arr(client.projection.visibleTurnItems).find(row => row.sourceItemId === id)?.item).status);
  return { client, native, command, refresh, status, rowStatus };
}

describe('Stop repairs rows the server already settled', () => {
  test('a command that finished while the socket was down settles after Stop, one lookup per pending item', async () => {
    const { native, command, refresh, status, rowStatus } = await open([item('i1', 'command_execution', 'running'), item('i2', 'assistant_message', 'running', { text: '', streaming: true, messageId: 'm' })]);
    native.server.i1 = item('i1', 'command_execution', 'completed', { output: 'done' });
    await command('stop');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.stop', threadId: 't1' });
    // Only background work is looked up: a command, a tool or a subagent, not a streaming answer.
    expect(native.lookups.map(lookup => [lookup.payload, lookup.timeout])).toEqual([[{ threadId: 't1', itemId: 'i1' }, 10]]);
    await refresh();
    expect([status('i1'), rowStatus('i1'), status('i2')]).toEqual(['completed', 'completed', 'running']);
  });

  test('items the server still runs stay running, and lookups go four at a time', async () => {
    const local = ['i1', 'i2', 'i3', 'i4', 'i5', 'i6'].map(id => item(id, id === 'i6' ? 'dynamic_tool' : 'command_execution', 'running'));
    const { native, command, refresh, status } = await open(local);
    for (const id of ['i1', 'i2', 'i3', 'i5', 'i6']) native.server[id] = { ...local.find(entry => entry.id === id)!, status: 'completed' };
    native.server.i4 = local[3]!;
    native.held = true;
    await command('stop');
    expect(native.lookups).toHaveLength(4);
    native.release();
    await refresh();
    expect(native.lookups).toHaveLength(6);
    native.release();
    await refresh();
    expect(['i1', 'i2', 'i3', 'i4', 'i5', 'i6'].map(status)).toEqual(['completed', 'completed', 'completed', 'running', 'completed', 'completed']);
  });

  test('an event applied during the lookup wins over the reply', async () => {
    const { native, command, refresh, status } = await open([item('i1', 'command_execution', 'running')]);
    native.server.i1 = item('i1', 'command_execution', 'completed', { output: 'from the lookup' });
    native.held = true;
    await command('stop');
    native.emitDomain('turn-item.updated', item('i1', 'command_execution', 'failed', { output: 'from the event' }));
    await refresh();
    native.release();
    await refresh();
    expect(status('i1')).toBe('failed');
  });

  test('nothing pending sends no lookup; a persistent tool and a rolled-back run are not pending work', async () => {
    const { native, command } = await open([item('i1', 'dynamic_tool', 'running', { input: { persistent: true } }), item('i2', 'command_execution', 'completed'),
      item('i3', 'command_execution', 'running', { runId: 'r0' })], [{ id: 'r0', status: 'rolled_back' }, { id: 'r1', status: 'completed' }]);
    await command('stop');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.stop' });
    expect(native.lookups).toEqual([]);
  });
});
