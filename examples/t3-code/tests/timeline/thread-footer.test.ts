// The timeline's footer (ChatView timelineFooter at T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3) before the thread's
// detail arrives: ThreadNow's sync line and the status line, which the shell gives (useThreadLifecycle.tsx).
import { describe, expect, test } from 'bun:test';
import { threadFooterRows } from '../../src/timeline/thread-status-line';
import { opened } from '../support/composer-controls-fixture';

describe('the timeline footer while a thread loads', () => {
  test('a snoozed thread still says so, under "Loading messages..."', async () => {
    const context = await opened(), { client } = context, now = Date.parse('2026-10-28T00:00:00Z');
    await context.command('select-thread', 't2');
    await client.refresh(context.native, context.disk);
    Object.assign(client.shell.threads.find(thread => thread.id === 't2')!, { snoozedUntil: new Date(now + 2 * 3_600_000).toISOString() });
    client.thread = null;
    // The sync line shows from the snapshot after the one that found the thread loading (syncStatus).
    expect(threadFooterRows(client, now).map(row => row.kind)).toEqual(['status']);
    expect(threadFooterRows(client, now).map(row => [row.kind, row.title])).toEqual([['thread-now', 'Loading messages...'], ['status', 'Snoozed, 2h left']]);
  });
});
