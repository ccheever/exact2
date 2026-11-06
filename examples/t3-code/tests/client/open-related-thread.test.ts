// ChatView onOpenRelatedThread at T3 Code 74541bab17 (#20747): a thread the timeline links to that the live shell does not
// hold, or holds deleted, stays a toast and the current thread stays open.
import { describe, expect, test } from 'bun:test';
import { opened } from '../support/client-fixture';
import { toasts } from '../../src/shell/toast';

describe('opening a related thread from the timeline', () => {
  test('a missing thread, after its short wait, and a deleted one leave the current thread open with a toast', async () => {
    const { client, native, command } = await opened();
    await command('open-related-thread', 'missing');
    expect(client.threadId).toBe('t1');
    expect(toasts(client).map(toast => [toast.kind, toast.title])).toEqual([['info', 'This thread is no longer available']]);
    (native.shell.threads as Record<string, unknown>[])[1]!.deletedAt = '2026-10-21T00:00:00Z';
    client.shell.threads.find(thread => thread.id === 't2')!.deletedAt = '2026-10-21T00:00:00Z';
    await command('open-related-thread', 't2');
    expect(client.threadId).toBe('t1');
    expect(toasts(client)).toHaveLength(2);
  });
  test('a thread the shell holds opens', async () => {
    const { client, command } = await opened();
    await command('open-related-thread', 't2');
    expect(client.threadId).toBe('t2');
    expect(toasts(client)).toHaveLength(0);
  });
});
