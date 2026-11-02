// A Stop the agent has not finished, and Force stop (T3 Code 2359643cff threadExecution.ts presentRunStop,
// ThreadActivityBanners.tsx runStopNowItem, useForceStopThread.ts; MIT, see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from '../../src/client/domain';
import { presentRunStop } from '../../src/composer/run-stop';
import { threadNowRows } from '../../src/composer/thread-end';
import { toasts } from '../../src/shell/toast';
import { opened } from '../support/composer-controls-fixture';

const request = (runId: string, extra: Obj = {}) => ({ id: `ir-${runId}`, type: 'run_interrupt_request', runId, startedAt: '2026-10-31T10:00:00.000Z', updatedAt: '2026-10-31T10:00:00.000Z', ...extra });

describe('presentRunStop', () => {
  test('an unanswered Stop of the running run is stopping, then unresponsive; its result ends it', () => {
    const runs = [{ id: 'r1', ordinal: 1, status: 'running' }];
    expect(presentRunStop({ runs, turnItems: [] })).toBeNull();
    expect(presentRunStop({ runs, turnItems: [request('r1')] })).toEqual({ state: 'stopping', runId: 'r1', requestedAt: '2026-10-31T10:00:00.000Z', background: false });
    expect(presentRunStop({ runs, turnItems: [request('r1', { unresponsiveAt: '2026-10-31T10:00:30.000Z' })] })?.state).toBe('unresponsive');
    expect(presentRunStop({ runs, turnItems: [request('r1'), { id: 'x', type: 'run_interrupt_result', runId: 'r1' }] })).toBeNull();
  });
  test('on an ended run only an unanswered Stop of its background work counts; queued runs hold none', () => {
    const runs = [{ id: 'r1', ordinal: 1, status: 'completed' }, { id: 'r2', ordinal: 2, status: 'queued' }];
    expect(presentRunStop({ runs, turnItems: [request('r1')] })).toBeNull();
    expect(presentRunStop({ runs, turnItems: [request('r1', { unresponsiveAt: '2026-10-31T10:00:30.000Z' })] })).toMatchObject({ state: 'unresponsive', runId: 'r1', background: true });
  });
});

describe('the thread end line and Force stop', () => {
  test('Stopping is the thread\'s work with its elapsed time; unanswered, it needs you and offers Force stop', async () => {
    const { client, native, disk, command } = await opened();
    obj(obj(native.config.environment).capabilities).threadForceStop = true;
    await client.refresh(native, disk);
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'running' }];
    client.thread!.projection.turnItems = [request('r1')];
    expect(threadNowRows(client)).toEqual([expect.objectContaining({ id: 'run-stop:t1', title: 'Stopping', icon: 'circle-stop', tone: '', detail: '',
      startedMs: Date.parse('2026-10-31T10:00:00.000Z'), activities: [] })]);
    client.thread!.projection.turnItems = [request('r1', { unresponsiveAt: '2026-10-31T10:00:30.000Z' })];
    const [line] = threadNowRows(client);
    expect(line).toMatchObject({ title: 'Agent not responding to Stop', icon: 'triangle-alert', tone: 'warning', detail: 'It may be short on memory. Force stop kills its processes.' });
    expect(line!.activities).toEqual([expect.objectContaining({ label: 'Force stop', detail: 'cc:force-stop', targetId: 'r1', ok: true })]);
    // The server's outcomes: a shared process stays, a releasing one still shuts down; a failure says so.
    const later = native.later.bind(native);
    let reply: Obj | Error = { outcome: 'shared', sharedWithThreadIds: ['t2'] };
    native.later = async (input: unknown) => {
      const call = obj(input);
      if (call.op === 'request' && call.method === 'orchestration.forceStopThread') {
        native.committed.push({ method: call.method, ...obj(call.payload) });
        return reply instanceof Error ? { ok: false, generation: native.generation, error: { kind: 'server', message: reply.message, uncertain: false } } : native.ok(reply);
      }
      return later(input);
    };
    await command('cc:force-stop', 'r1');
    expect(native.committed.at(-1)).toEqual({ method: 'orchestration.forceStopThread', threadId: 't1', runId: 'r1' });
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'info', title: 'Turn stopped; the agent process kept running', description: 'It is shared with 1 working thread. Stop those to free it.' });
    reply = { outcome: 'releasing', sharedWithThreadIds: [] };
    await command('cc:force-stop', 'r1');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'info', title: 'Turn stopped; the agent is still shutting down' });
    reply = new Error('The agent is gone.');
    await command('cc:force-stop', 'r1');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not force stop the agent' });
  });
  test('without the server\'s threadForceStop the warning stands alone', async () => {
    const { client } = await opened();
    client.thread!.projection.runs = [{ id: 'r1', ordinal: 1, status: 'running' }];
    client.thread!.projection.turnItems = [request('r1', { unresponsiveAt: '2026-10-31T10:00:30.000Z' })];
    expect(threadNowRows(client)[0]).toMatchObject({ title: 'Agent not responding to Stop', activities: [] });
  });
});
