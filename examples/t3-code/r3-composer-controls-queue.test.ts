// Lane composer-controls, round 3: dragging a queued message's grip
// (QueuedRunsControl completeDrag) reorders it with queued-run.reorder.
import { describe, expect, test } from 'bun:test';
import { arr } from './domain';
import { snapshot } from './presentation';
import { opened, running } from './composer-controls-fixture';
import type { T3Client } from './client';

const queued = (client: T3Client) => {
  running(client, { activeAttemptId: 'a1', providerThreadId: 'pt' });
  Object.assign(client.thread!.projection, {
    runs: [...arr(client.thread!.projection.runs), { id: 'q1', ordinal: 2, status: 'queued', queuePosition: 1, userMessageId: 'm1' },
      { id: 'q2', ordinal: 3, status: 'queued', queuePosition: 2, userMessageId: 'm2' }, { id: 'q3', ordinal: 4, status: 'queued', queuePosition: 3, userMessageId: 'm3' }],
    messages: [{ id: 'm1', text: 'One' }, { id: 'm2', text: 'Two' }, { id: 'm3', text: 'Three' }],
    providerThreads: [{ id: 'pt', providerSessionId: 's' }], providerSessions: [{ id: 's', capabilities: { turns: { supportsQueuedMessages: true } } }],
    providerTurns: [{ runAttemptId: 'a1', status: 'running' }] });
};

describe('queued drag reorder', () => {
  test('a drop moves the run before the run at the insertion index, or to the end', async () => {
    const { client, native, command } = await opened();
    queued(client);
    const reorders = () => native.committed.filter(entry => entry.type === 'queued-run.reorder').map(entry => [entry.runId, entry.beforeRunId]);
    await command('cc:queued-drop', 'q3|0');
    await command('cc:queued-drop', 'q1|3');
    await command('cc:queued-drop', 'q1|2');
    expect(reorders()).toEqual([['q3', 'q1'], ['q1', null], ['q1', 'q3']]);
  });
  test('dropping just before or after itself does nothing, but still settles the lifted row', async () => {
    const { client, native, command } = await opened();
    queued(client);
    const before = snapshot(client).composer.queueDropSeq;
    await command('cc:queued-drop', 'q2|1');
    await command('cc:queued-drop', 'q2|2');
    await command('cc:queued-drop', 'gone|0');
    await command('cc:queued-drop', 'q2|9');
    expect(native.committed.some(entry => entry.type === 'queued-run.reorder')).toBe(false);
    expect(snapshot(client).composer.queueDropSeq).toBe(before + 4);
  });
});
