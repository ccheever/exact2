// Stop settles background rows the server already finished (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3:
// packages/client-runtime/src/state/threadCommands.ts interruptTurn, state/threads.ts reconcilePendingWork): after
// each Stop action, however many commands it sends, the open thread's active background items are read with
// getTurnItem, four at a time with 10 s each, and those the server settled replace their local copies; an event
// applied meanwhile wins.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { arr, obj, str, type Obj } from '../../src/client/domain';
import { Backend, storage } from '../support/client-fixture';

const item = (id: string, type: string, status: string, extra: Obj = {}): Obj => ({ id, threadId: 't1', runId: 'r1', nodeId: 'n', ordinal: Number(id.replace(/\D/g, '')) || 1, type, status,
  ...(type === 'command_execution' ? { input: `sleep ${id}` } : {}), ...(type === 'dynamic_tool' ? { toolName: 'watch' } : {}), ...extra });

/** getTurnItem replies join the inbox under their `deliver` key, as the transport files them. */
class RepairBackend extends Backend {
  server: Record<string, Obj> = {};
  /** Items whose lookup fails (a timeout, say) instead of answering. */
  failing = new Set<string>();
  /** orchestration.getThreadProjection's answers, by thread (a subagent's Stop reads its owner fresh). */
  projections: Record<string, Obj> = {};
  lookups: Obj[] = [];
  /** Lookups held back until the test releases them. */
  held = false;
  private pending: Obj[] = [];
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'request' && request.method === 'orchestration.getThreadProjection') {
      this.calls.push(request);
      return this.good(this.projections[str(obj(request.payload).threadId)] ?? {});
    }
    if (request.op !== 'request' || request.method !== 'orchestration.getTurnItem') return super.later(input);
    this.calls.push(request); this.lookups.push(request);
    if (this.held) this.pending.push(request); else this.answer(request);
    return this.good({ id: `rpc-${this.lookups.length}` });
  }
  answer(request: Obj) {
    const itemId = str(obj(request.payload).itemId);
    const value = this.failing.has(itemId) ? { _replyError: { kind: 'Timeout', message: 'The request timed out after 10 s.', uncertain: false } } : { _reply: { item: this.server[itemId] ?? null } };
    this.events.push({ seq: ++this.eventSequence, generation: this.generation, key: str(request.deliver), value });
  }
  release() { this.held = false; for (const request of this.pending.splice(0)) this.answer(request); }
}

async function open(local: Obj[], runs: Obj[] = [{ id: 'r1', status: 'completed' }], options: { threadStop?: boolean; subagents?: Obj[] } = {}) {
  const native = new RepairBackend(), disk = storage(), client = new T3Client();
  const projection = obj(native.details.t1!.projection);
  Object.assign(projection, { runs, turnItems: local, subagents: options.subagents ?? [],
    visibleTurnItems: local.map((entry, position) => ({ position, visibility: 'local', sourceThreadId: 't1', sourceItemId: entry.id, item: entry })) });
  obj(obj(native.config.environment).capabilities).threadStop = options.threadStop !== false;
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

  test('an event applied during the lookup wins over the reply; the reply settles the rest', async () => {
    const { native, command, refresh, status } = await open([item('i1', 'command_execution', 'running'), item('i2', 'command_execution', 'running')]);
    native.server.i1 = item('i1', 'command_execution', 'completed', { output: 'from the lookup' });
    native.server.i2 = item('i2', 'command_execution', 'completed', { output: 'only the lookup knows' });
    native.held = true;
    await command('stop');
    native.emitDomain('turn-item.updated', item('i1', 'command_execution', 'failed', { output: 'from the event' }));
    await refresh();
    native.release();
    await refresh();
    expect([status('i1'), status('i2')]).toEqual(['failed', 'completed']);
  });

  test('a Stop with nothing to interrupt still repairs, as the reference\'s does', async () => {
    const { native, command, refresh, status } = await open([item('i1', 'command_execution', 'running')], [{ id: 'r1', status: 'completed' }], { threadStop: false });
    native.server.i1 = item('i1', 'command_execution', 'completed');
    const sent = native.committed.length;
    await command('cc:stop-background');
    expect(native.committed.length).toBe(sent);
    expect(native.lookups.map(lookup => obj(lookup.payload).itemId)).toEqual(['i1']);
    await refresh();
    expect(status('i1')).toBe('completed');
  });

  test('a Stop that interrupts the run (no threadStop) repairs too', async () => {
    const { native, command, refresh, status } = await open([item('i1', 'command_execution', 'running')], [{ id: 'r1', status: 'running' }], { threadStop: false });
    native.server.i1 = item('i1', 'command_execution', 'completed');
    await command('stop');
    expect(native.committed.at(-1)).toMatchObject({ type: 'run.interrupt', runId: 'r1' });
    expect(native.lookups.map(lookup => obj(lookup.payload).itemId)).toEqual(['i1']);
    await refresh();
    expect(status('i1')).toBe('completed');
  });

  test("a subagent's Stop repairs its thread once, though an app-owned task sends two commands", async () => {
    const started = '2026-10-27T12:00:00.000Z';
    const nativeTask = { id: 'native-task', threadId: 't1', childThreadId: 'native-child', runId: 'r1', origin: 'provider_native', driver: 'claudeAgent',
      nativeTaskRef: { nativeId: 'claude-task', strength: 'strong' }, status: 'running', startedAt: started };
    const appOwned = { id: 'task', threadId: 't1', childThreadId: 'kid', runId: 'r1', origin: 'app_owned', status: 'running', startedAt: started };
    const local = [item('i1', 'command_execution', 'running'), item('i2', 'dynamic_tool', 'running')];
    for (const [agent, commands] of [[nativeTask, ['subagent.stop']], [appOwned, ['subagent.stop', 'thread.stop']]] as const) {
      const opened = await open(local, [{ id: 'r1', status: 'running' }], { subagents: [agent] });
      opened.native.projections.t1 = { subagents: [agent], runs: [{ id: 'r1', status: 'running' }] };
      opened.native.server.i1 = item('i1', 'command_execution', 'completed');
      await opened.command('shell:stop-subagent', agent.childThreadId);
      expect(opened.native.committed.slice(-commands.length).map(payload => payload.type)).toEqual([...commands]);
      expect(opened.native.lookups.map(lookup => obj(lookup.payload).itemId)).toEqual(['i1', 'i2']);
      await opened.refresh();
      expect([opened.status('i1'), opened.status('i2')]).toEqual(['completed', 'running']);
    }
  });

  test('another thread opened during the lookups drops the repair', async () => {
    const local = ['i1', 'i2', 'i3', 'i4', 'i5'].map(id => item(id, 'command_execution', 'running'));
    const { client, native, command, refresh } = await open(local);
    for (const entry of local) native.server[str(entry.id)] = { ...entry, status: 'completed' };
    native.held = true;
    await command('stop');
    expect(native.lookups).toHaveLength(4);
    await command('select-thread', 't2');
    await refresh();
    native.release();
    await refresh();
    // The fifth lookup is never sent, and the other thread's rows are left alone.
    expect([native.lookups.length, client.threadId]).toEqual([4, 't2']);
    expect(arr(client.projection.turnItems).map(entry => entry.status)).toEqual([]);
  });

  test('a lookup that fails or times out leaves its row; the others settle', async () => {
    const { native, command, refresh, status } = await open([item('i1', 'command_execution', 'running'), item('i2', 'command_execution', 'running')]);
    native.server.i1 = item('i1', 'command_execution', 'completed');
    native.server.i2 = item('i2', 'command_execution', 'completed');
    native.failing.add('i2');
    await command('stop');
    await refresh();
    expect([status('i1'), status('i2')]).toEqual(['completed', 'running']);
    // The repair is over: the next Stop reads i2 again.
    native.failing.clear();
    await command('stop');
    await refresh();
    expect(native.lookups.map(lookup => obj(lookup.payload).itemId)).toEqual(['i1', 'i2', 'i2']);
    expect(status('i2')).toBe('completed');
  });

  test('nothing pending sends no lookup; a persistent tool and a rolled-back run are not pending work', async () => {
    const { native, command } = await open([item('i1', 'dynamic_tool', 'running', { input: { persistent: true } }), item('i2', 'command_execution', 'completed'),
      item('i3', 'command_execution', 'running', { runId: 'r0' })], [{ id: 'r0', status: 'rolled_back' }, { id: 'r1', status: 'completed' }]);
    await command('stop');
    expect(native.committed.at(-1)).toMatchObject({ type: 'thread.stop' });
    expect(native.lookups).toEqual([]);
  });
});
