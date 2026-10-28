// The end of the timeline and the queued-message edit, as T3 Code 7c3b3b3605ef (MIT; see LICENSE-T3) has them:
// ThreadNow.tsx, QueuedRunsControl.tsx, queuedRunEdit.ts (makeQueuedRunEditStore) and useQueuedRunEdit.ts,
// against the composer-controls fake backend.
import { describe, expect, test } from 'bun:test';
import { arr, obj, type Obj } from '../../src/client/domain';
import { snapshot } from '../../src/client/presentation';
import { toasts } from '../../src/shell/toast';
import { endSummary, queuedTurnRows, threadEndRows, threadNowRows } from '../../src/composer/thread-end';
import { checkMissingEditThreads, queueState, queuedEdit } from '../../src/composer/composer-controls-queue';
import { opened } from '../support/composer-controls-fixture';

/** A running, steerable turn on t1 with `count` queued messages, as the server holds them. */
async function queue(count: number, steerable = true) {
  const context = await opened();
  const projection = obj(context.native.details.t1!.projection);
  Object.assign(projection, {
    runs: [{ id: 'r1', ordinal: 1, status: 'running', activeAttemptId: 'a1', providerThreadId: 'pt' },
      ...Array.from({ length: count }, (_, i) => ({ id: `q${i + 1}`, ordinal: i + 2, status: 'queued', queuePosition: i + 1, userMessageId: `m${i + 1}` }))],
    messages: Array.from({ length: count }, (_, i) => ({ id: `m${i + 1}`, role: 'user', text: `Follow-up ${i + 1}\nsecond line` })),
    providerThreads: [{ id: 'pt', providerSessionId: 's' }],
    providerSessions: [{ id: 's', capabilities: { turns: { supportsActiveSteering: steerable } } }],
    providerTurns: [{ id: 'turn-1', runAttemptId: 'a1', status: steerable ? 'running' : 'completed' }] });
  await go(context, 't2');
  await go(context, 't1');
  return context;
}
/** Opens a thread as the window does: the selection, then the refresh that loads it. */
async function go(context: Awaited<ReturnType<typeof opened>>, id: string) {
  await context.command('select-thread', id);
  await context.client.refresh(context.native, context.disk);
}
const bubbles = (rows: ReturnType<typeof queuedTurnRows>) => rows.filter(row => row.kind === 'queued');
const lost = (client: Awaited<ReturnType<typeof opened>>['client']) => toasts(client).filter(toast => toast.title === 'Queued message is no longer queued');

describe('queued turns at the end of the timeline (QueuedRunsControl)', () => {
  test('a long queue shows its first two bubbles and "Show all N queued"; the button shows the rest', async () => {
    const { client, command } = await queue(4);
    const rows = queuedTurnRows(client);
    expect(bubbles(rows).map(row => [row.runId, row.body, row.title])).toEqual([
      ['q1', 'Follow-up 1\nsecond line', 'Queued message 1 of 4, not sent'], ['q2', 'Follow-up 2\nsecond line', 'Queued message 2 of 4, not sent']]);
    expect(rows.at(-1)).toMatchObject({ kind: 'queued-more', title: 'Show all 4 queued', expanded: false });
    expect(rows.map(row => [row.continues === true, row.collapsible === true])).toEqual([[true, false], [false, true], [false, false]]);
    expect(rows[0]!.attribution).toBe('4 queued');
    expect(endSummary(client)).toBe('4 queued');
    expect(snapshot(client).composer.endSummary).toBe('4 queued');
    await command('cclocal:queued-show-all');
    expect(bubbles(queuedTurnRows(client))).toHaveLength(4);
    expect(queuedTurnRows(client).at(-1)).toMatchObject({ title: 'Show fewer', expanded: true });
    await command('cclocal:queued-show-all');
    expect(bubbles(queuedTurnRows(client))).toHaveLength(2);
  });
  test('Send now steers, the first naming ⇧⌘Enter; the pencil on the last names ⌥Up; nothing reorders', async () => {
    const { client, native, command } = await queue(3);
    const rows = bubbles(queuedTurnRows(client));
    expect(rows.map(row => [row.actionLabel, row.intentTip, row.detail])).toEqual([
      ['Send now', 'Send into the current turn (⇧⌘Enter)', 'Edit in the composer'],
      ['Send now', 'Send into the current turn', 'Edit in the composer'],
      ['Send now', 'Send into the current turn', 'Edit in the composer (⌥Up)']]);
    await command('cc:queued-steer', 'q1');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-message.promote-to-steer', queuedRunId: 'q1', targetRunId: 'r1' });
    const before = native.committed.length;
    expect((await command('cc:queued-reorder', 'q2', 'q1')).message).toBe('Unknown composer action: queued-reorder');
    expect(native.committed.length).toBe(before);
  });
  test('with no active run Send now starts a turn (queued-run.start); during a compaction it skips the compaction', async () => {
    const { client, native, command } = await queue(2);
    // Stopped: the queue waits with no active run.
    client.thread!.projection.runs = arr(client.thread!.projection.runs).map(run => run.id === 'r1' ? { ...run, status: 'interrupted' } : run);
    expect(bubbles(queuedTurnRows(client)).map(row => [row.actionLabel, row.intentTip])).toEqual([
      ['Send now', 'Start a turn with this message (⇧⌘Enter)'], ['Send now', 'Start a turn with this message']]);
    await command('cc:queued-steer', 'q2');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-run.start', runId: 'q2' });
    // A running /compact: Send now stops it and sends, whether or not the provider steers.
    client.thread!.projection.runs = arr(client.thread!.projection.runs).map(run => run.id === 'r1' ? { ...run, status: 'running', userMessageId: 'mc' } : run);
    client.thread!.projection.messages = [...arr(client.thread!.projection.messages), { id: 'mc', role: 'user', text: '/compact' }];
    client.thread!.projection.providerSessions = [{ id: 's', capabilities: { turns: {} } }];
    expect(bubbles(queuedTurnRows(client))[0]).toMatchObject({ actionLabel: 'Send now', intentTip: 'Skip compaction and send this message (⇧⌘Enter)' });
    await command('cc:queued-steer', 'q1');
    expect(native.committed.at(-1)).toMatchObject({ type: 'queued-message.promote-to-steer', queuedRunId: 'q1', targetRunId: 'r1' });
  });
  test("Send now's rules: no turn starts on an archived or deleted thread; only a user's bare /compact is skipped", async () => {
    const { client } = await queue(1, false);
    const projection = client.projection;
    const active = (status: string, userMessageId = '') => { projection.runs = arr(projection.runs).map(run => run.id === 'r1' ? { ...run, status, userMessageId } : run); };
    active('interrupted');
    expect(queueState(projection)).toMatchObject({ canSendNow: true, canSteer: false });
    for (const gone of [{ archivedAt: '2026-10-28T00:00:00.000Z' }, { deletedAt: '2026-10-28T00:00:00.000Z' }]) {
      projection.thread = { ...obj(projection.thread), archivedAt: null, deletedAt: null, ...gone };
      expect(queueState(projection).canSendNow).toBe(false);
      expect(bubbles(queuedTurnRows(client))[0]!.actionLabel).toBe('');
    }
    projection.thread = { ...obj(projection.thread), archivedAt: null, deletedAt: null };
    // isCompactionCommandMessage: the user's `/compact` (any case, padded) with no attachments, nothing else.
    active('running', 'mc');
    const compacting = (fields: Obj) => {
      projection.messages = [...arr(projection.messages).filter(message => message.id !== 'mc'), { id: 'mc', role: 'user', text: ' /COMPACT ', ...fields }];
      return queueState(projection);
    };
    expect(compacting({})).toMatchObject({ canSendNow: false, promoteSkipsCompaction: true, canSteer: true });
    expect(compacting({ attachments: [{ type: 'image', id: 'i1' }] })).toMatchObject({ promoteSkipsCompaction: false, canSteer: false });
    expect(compacting({ role: 'assistant' })).toMatchObject({ promoteSkipsCompaction: false, canSteer: false });
    expect(compacting({ text: '/compact the history' })).toMatchObject({ promoteSkipsCompaction: false, canSteer: false });
  });
  test('a turn that cannot be steered offers no Send now', async () => {
    const { client } = await queue(1, false);
    expect(bubbles(queuedTurnRows(client))[0]).toMatchObject({ actionLabel: '', status: '' });
  });
  test('an open question disables the pencil with its reason and refuses the edit', async () => {
    const { client, command } = await queue(2);
    client.thread!.projection.runtimeRequests = [{ id: 'ask', kind: 'user_input', status: 'pending', createdAt: '2026-10-03T01:00:00.000Z' }];
    client.thread!.projection.turnItems = [{ type: 'user_input_request', requestId: 'ask', questions: [{ id: 'q', header: 'Pick', question: 'Which?', options: [{ label: 'A' }] }] }];
    expect(bubbles(queuedTurnRows(client)).map(row => [row.detail, row.failed])).toEqual([
      ['Answer the question to edit queued messages', true], ['Answer the question to edit queued messages', true]]);
    await command('cclocal:queued-edit', 'q2');
    expect(queuedEdit(client)).toBeUndefined();
  });
  test('while one is edited its row says so and offers Cancel instead of Send now', async () => {
    const { client, command } = await queue(2);
    await command('cclocal:queued-edit', 'q2');
    expect(bubbles(queuedTurnRows(client)).map(row => [row.runId, row.status, row.actionLabel])).toEqual([['q1', '', 'Send now'], ['q2', 'editing', '']]);
  });
});

describe('the queued edit (makeQueuedRunEditStore)', () => {
  test('one edit per thread, kept across navigation with the thread\'s own draft', async () => {
    const context = await queue(2), { client, command } = context;
    await command('draft', '', 'my own draft');
    await command('cclocal:queued-edit', 'q1');
    await command('draft', '', 'Follow-up 1, revised');
    await go(context, 't2');
    expect(queuedEdit(client)).toBeUndefined();
    await go(context, 't1');
    expect(queuedEdit(client)?.runId).toBe('q1');
    expect(client.draft).toBe('Follow-up 1, revised');
    await command('cclocal:queued-cancel');
    expect(client.draft).toBe('my own draft');
  });
  test('a run that starts while the user is elsewhere settles its edit there, once, naming the thread', async () => {
    const context = await queue(1), { client, command } = context;
    await command('cclocal:queued-edit', 'q1');
    await command('draft', '', 'changed while queued');
    await go(context, 't2');
    Object.assign(client.shell.threads.find(thread => thread.id === 't1')!, { activeRunId: 'q1' });
    snapshot(client);
    snapshot(client);
    expect(toasts(client).filter(toast => toast.title === 'Queued message is no longer queued').map(toast => toast.description))
      .toEqual(['Your unsaved edit in “Thread t1” was kept in the composer.']);
    expect(client.local.drafts[`${client.environmentId}:t1`]).toBe('changed while queued');
  });
  // The pinned server reports an archive on the active shell as it reports a deletion (ShellStream.ts thread.removed,
  // location "active"); the archived shell tells them apart (queuedRunEdit.ts makeQueuedRunsAtom).
  test('a thread that leaves the active shell keeps its edit while archived; deleted, the edit ends once, unnamed', async () => {
    const context = await queue(1), { client, native, disk, command } = context, now = Date.parse('2026-10-28T00:00:00Z');
    await command('cclocal:queued-edit', 'q1');
    await command('draft', '', 'changed while queued');
    await go(context, 't2');
    const row = { ...client.shell.threads.find(thread => thread.id === 't1')! };
    const leave = async () => {
      native.emit('shell', { kind: 'thread.removed', sequence: client.shell.sequence + 1, location: 'active', threadId: 't1' });
      await client.refresh(native, disk);
      snapshot(client);
      await checkMissingEditThreads(client, native, now);
      snapshot(client);
    };
    native.archived = [{ ...row, archivedAt: '2026-10-28T00:00:00.000Z' }];
    await leave();
    expect(lost(client)).toHaveLength(0);
    // Unarchived, the row is back and so is the edit.
    native.archived = [];
    native.emit('shell', { kind: 'thread.updated', sequence: client.shell.sequence + 1, location: 'active', thread: row });
    await client.refresh(native, disk);
    snapshot(client);
    await go(context, 't1');
    expect(queuedEdit(client)?.runId).toBe('q1');
    await go(context, 't2');
    // Deleted: not in the archive either. Its row is gone, so the toast names no thread (useQueuedRunEdit.ts).
    await leave();
    expect(lost(client).map(toast => toast.description)).toEqual(['Your unsaved edit was kept in the composer.']);
    snapshot(client);
    expect(lost(client)).toHaveLength(1);
  });
  test('a deletion seen only in a fresh snapshot, after a reconnect, ends the edit too', async () => {
    const context = await queue(1), { client, native, command } = context;
    await command('cclocal:queued-edit', 'q1');
    await command('draft', '', 'changed while queued');
    await go(context, 't2');
    // The reconnect's snapshot no longer lists t1, and the archive does not hold it.
    client.shell = { ...client.shell, threads: client.shell.threads.filter(thread => thread.id !== 't1') };
    snapshot(client);
    await checkMissingEditThreads(client, native, Date.parse('2026-10-28T00:00:00Z'));
    snapshot(client);
    expect(lost(client)).toHaveLength(1);
  });
  test('a save keeps its claim until it finishes, and a stale save never clears a newer edit', async () => {
    const { client, native, command } = await queue(2);
    let release: () => void = () => {};
    const later = native.later.bind(native);
    native.later = async (input: unknown) => {
      const request = obj(input);
      if (request.op === 'request' && obj(request.payload).type === 'queued-run.edit') await new Promise<void>(resolve => { release = resolve; });
      return later(input);
    };
    await command('draft', '', 'my own draft');
    await command('cclocal:queued-edit', 'q1');
    const saving = command('send', '', 'Follow-up 1, saved');
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(snapshot(client).composer.queueEditSaving).toBe(true);
    // The edit is begun again while its save is in flight: the save still decides nothing for the newer edit.
    await command('cclocal:queued-cancel');
    await command('cclocal:queued-edit', 'q2');
    await command('send', '', 'Follow-up 2, too early');
    expect(arr(native.committed).filter((entry: Obj) => entry.type === 'queued-run.edit')).toHaveLength(0);
    release();
    await saving;
    expect(queuedEdit(client)?.runId).toBe('q2');
    expect(client.draft).toBe('Follow-up 2\nsecond line');
    expect(arr(native.committed).filter((entry: Obj) => entry.type === 'queued-run.edit').map(entry => entry.text)).toEqual(['Follow-up 1, saved']);
  });
});

describe('what the thread is doing (ThreadNow)', () => {
  test('while a thread loads, the footer is its sync line from the next snapshot on', async () => {
    const context = await opened(), { client } = context;
    await go(context, 't2');
    // The timeline asks for the footer before it has the thread's detail (thread-status-line.ts: no status, no handoff).
    client.thread = null;
    expect(threadEndRows(client, [], null)).toEqual([]);
    expect(threadEndRows(client, [], null)).toMatchObject([{ id: 'thread-sync:t2', kind: 'thread-now', icon: 'refresh-cw', status: 'status', title: 'Loading messages...' }]);
  });
  test('the footer: the pending handoff, what the thread is doing, the status line, then the queue', async () => {
    const { client } = await queue(1);
    Object.assign(client.shell.threads.find(thread => thread.id === 't1')!, { goal: { objective: 'Fix parser', status: 'active' } });
    const handoff = { id: 'pending-handoff:t1', kind: 'handoff', title: 'Context handoff pending', body: '' };
    const status = { id: 'status:t1', kind: 'status', title: 'Working', body: '' };
    expect(threadEndRows(client, [handoff], status).map(row => row.kind)).toEqual(['handoff', 'thread-now', 'status', 'queued']);
  });
  test('a cut line offers its details, measured from its probes against its laid-out width', async () => {
    const { client } = await opened();
    Object.assign(client.shell.threads[0]!, { pendingBackgroundTasks: [{ taskId: 'a', kind: 'command', description: 'npm run dev' }] });
    const line = threadNowRows(client)[0]!;
    expect(line.collapsible).toBe(false);
    client.presentation.anchors = { [`tn:${line.id}`]: [0, 60], 't:12:400:Running: npm run dev': [0, 110] };
    expect(threadNowRows(client)[0]!.collapsible).toBe(true);
    client.presentation.anchors = { [`tn:${line.id}`]: [0, 140], 't:12:400:Running: npm run dev': [0, 110] };
    expect(threadNowRows(client)[0]!.collapsible).toBe(false);
  });
});

describe('notices above the composer (ComposerBannerStack)', () => {
  test("the machine's notice comes before a usage limit; an open question leaves only the machine's", async () => {
    const { client } = await opened();
    const now = Date.parse('2026-10-20T12:00:00.000Z'), reset = new Date(now + 30 * 60_000).toISOString();
    snapshot(client, now);
    Object.assign(client.shell.threads[0]!, { status: 'failed', lastErrorClass: 'usage_limit', usageLimitResetAt: reset, latestRunId: 'r9', updatedAt: new Date(now - 60_000).toISOString(),
      limitRecovery: { runId: 'r9', resetAt: reset, snooze: false } });
    client.connection = 'offline';
    expect(snapshot(client, now).composer.notices.map(notice => [notice.group, notice.title, notice.stacked])).toEqual([
      ['environment', expect.any(String), false], ['attention', 'Usage limit reached', true]]);
    client.thread!.projection.runtimeRequests = [{ id: 'ask', kind: 'user_input', status: 'pending', createdAt: '2026-10-03T01:00:00.000Z' }];
    client.thread!.projection.turnItems = [{ type: 'user_input_request', requestId: 'ask', questions: [{ id: 'q', header: 'Pick', question: 'Which?', options: [{ label: 'A' }] }] }];
    expect(snapshot(client, now).composer.notices.map(notice => notice.group)).toEqual(['environment']);
  });
  test('machine notices come first, every card after the first joins it, and an open question leaves only machine problems', async () => {
    const { client, native, disk } = await opened();
    client.connection = 'offline';
    obj(obj(native.config.environment)).label = 'Studio';
    const notices = snapshot(client).composer.notices;
    expect(notices[0]).toMatchObject({ group: 'environment', stacked: false });
    expect(notices.slice(1).every(notice => notice.stacked)).toBe(true);
    client.thread!.projection.runtimeRequests = [{ id: 'ask', kind: 'user_input', status: 'pending', createdAt: '2026-10-03T01:00:00.000Z' }];
    client.thread!.projection.turnItems = [{ type: 'user_input_request', requestId: 'ask', questions: [{ id: 'q', header: 'Pick', question: 'Which?', options: [{ label: 'A' }] }] }];
    expect(snapshot(client).composer.notices.every(notice => notice.group === 'environment' && (notice.variant === 'error' || notice.variant === 'warning'))).toBe(true);
    void disk;
  });
});
