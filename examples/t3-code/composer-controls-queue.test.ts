// Lane composer-controls: the queued-messages grip's keyboard reorder (QueuedRunsControl onKeyDown).
import { describe, expect, test } from 'bun:test';
import { arr } from './domain';
import { opened, running } from './composer-controls-fixture';
import { snapshot } from './presentation';

describe('queued messages reorder', () => {
  test('↑ moves before the previous row, ↓ after the next one; the ends and other keys do nothing', async () => {
    const { client, native, command } = await opened();
    running(client, { activeAttemptId: 'a1' });
    client.thread!.projection.runs = [...arr(client.thread!.projection.runs),
      { id: 'q1', ordinal: 2, status: 'queued', queuePosition: 1, userMessageId: 'm1' },
      { id: 'q2', ordinal: 3, status: 'queued', queuePosition: 2, userMessageId: 'm2' },
      { id: 'q3', ordinal: 4, status: 'queued', queuePosition: 3, userMessageId: 'm3' }];
    client.thread!.projection.messages = [{ id: 'm1', text: 'one' }, { id: 'm2', text: 'two' }, { id: 'm3', text: 'three' }];
    const before = native.committed.length;
    await command('cc:queued-key', 'q2', 'ArrowUp');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-run.reorder', runId: 'q2', beforeRunId: 'q1' });
    await command('cc:queued-key', 'q1', 'ArrowDown');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-run.reorder', runId: 'q1', beforeRunId: 'q3' });
    await command('cc:queued-key', 'q2', 'ArrowDown');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-run.reorder', runId: 'q2', beforeRunId: null });
    const after = native.committed.length;
    expect(after - before).toBe(3);
    await command('cc:queued-key', 'q1', 'ArrowUp');
    await command('cc:queued-key', 'q3', 'ArrowDown');
    await command('cc:queued-key', 'q2', 'Tab');
    expect(native.committed.length).toBe(after);
  });
  test('composer commands run without Date.now() (data sources have none): the snapshot clock stands in', async () => {
    const { client, native, command } = await opened();
    snapshot(client, Date.parse('2026-10-03T02:00:00.000Z'));
    running(client);
    const real = Date.now;
    Date.now = () => { throw new Error('Date.now() is unavailable in data sources; pass time or a random seed as an argument'); };
    try {
      const result = await command('send', '', 'a follow-up while running');
      expect(result.message).toBe('');
      expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', dispatchMode: { type: 'queue_after_active' } });
      expect((await command('cclocal:dismiss-woke')).message).toBe('');
      expect((await command('cclocal:usage-limits')).message).toBe('');
      expect(client.error).toBe('');
    } finally { Date.now = real; }
  });
});
