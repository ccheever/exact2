// Codex `/feedback` in the composer (composer-feedback.ts) against the fake T3 backend: the
// reference's ChatView onSend branch (:8653-8706), feedbackBannerItem, anchoredTimelineMessages
// and the "Sending feedback" send status, with scripted `provider.uploadFeedback` replies.
import { describe, expect, test } from 'bun:test';
import { obj, str, type Obj } from './domain';
import { snapshot, transcriptPresentation } from './presentation';
import { toasts } from './toast';
import { Fake, connected, opened } from './composer-controls-fixture';
import { composerBranches } from './composer-controls-branch';
import { settleLostReplies, settleStaleReplies } from './composer-replies';
import { feedbackCommandFor } from './composer-feedback';
import { fileChipLink, stageFold } from './composer-editor-files';
import { contextLink, contextId } from './composer-editor-menu';

type Reply = { ok: true; value: Obj } | { ok: false; error: Obj };
/**
 * The fake backend with `provider.uploadFeedback` answered by the test: the request is detached
 * (T3Transport files its reply in the inbox under `deliver`), so `reply` emits it and the next
 * refresh drains it.
 */
class FeedbackFake extends Fake {
  uploads: Obj[] = [];
  keys: string[] = [];
  async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'request' && request.method === 'provider.uploadFeedback') {
      expect(typeof request.deliver).toBe('string');
      this.uploads.push(obj(request.payload)); this.keys.push(String(request.deliver));
      return this.ok({ id: `rpc-${this.uploads.length}` });
    }
    return super.later(input);
  }
  reply(result: Reply) {
    const key = this.keys.shift()!;
    this.emit(key, result.ok ? { _reply: result.value } : { _replyError: { uncertain: false, ...result.error } });
  }
}
async function codexThread() {
  const context = await opened();
  const native = new FeedbackFake();
  Object.assign(native, { shell: context.native.shell, details: context.native.details, config: context.native.config, seq: context.native.seq, subs: context.native.subs, generation: context.native.generation });
  const command = (op: string, id = '', value = '', n = 0) => context.client.command(op, id, value, n, native, context.disk);
  Object.assign(context.client.shell.threads.find(thread => thread.id === 't1')!, { activeProviderThreadId: 'pt1' });
  snapshot(context.client, Date.parse('2026-10-08T03:00:00.000Z'));
  return { ...context, native, command };
}
const notices = (client: Parameters<typeof snapshot>[0]) => snapshot(client, Date.parse('2026-10-08T03:00:00.000Z')).composer.notices.filter(notice => notice.id.startsWith('feedback:'));
const localRows = (client: Parameters<typeof snapshot>[0]) => transcriptPresentation(client).filter(row => row.local).map(row => [row.kind, row.body]);
const settle = () => new Promise(resolve => setTimeout(resolve, 0));

describe('/feedback in a Codex thread', () => {
  test('Send clears the draft at once, shows the banner, two local rows and "Sending feedback"; the reply shows the thread id with Copy ID', async () => {
    const { client, native, command, disk } = await codexThread();
    await command('draft', '', '/feedback broken diff');
    const sent = await command('send', '', '/feedback broken diff');
    expect(sent.message).toBe('');
    expect(native.uploads).toEqual([{ threadId: 't1', reason: 'broken diff' }]);
    expect(client.draft).toBe('');
    expect(native.committed.some(entry => entry.type === 'message.dispatch')).toBe(false);
    let view = snapshot(client, Date.parse('2026-10-08T03:00:00.000Z'));
    expect(view.composer).toMatchObject({ sendStatus: 'Sending feedback' });
    expect(view.canSend).toBe(false);
    expect(notices(client)).toMatchObject([{ title: 'Sending feedback to OpenAI...', description: '', variant: 'info', priority: 0, icon: 'message-square', dismiss: '', action: '' }]);
    expect(localRows(client)).toEqual([['user', '/feedback broken diff'], ['assistant', 'Sending feedback to OpenAI...']]);
    // A second send while the upload runs is held, as onSend returns for an in-flight thread.
    await command('send', '', 'another message');
    expect(native.committed.some(entry => entry.type === 'message.dispatch')).toBe(false);

    native.reply({ ok: true, value: { feedbackId: 'th_feedback_1' } });
    await client.refresh(native, disk); await settle();
    const [notice] = notices(client);
    expect(notice).toMatchObject({ title: 'Feedback sent to OpenAI', description: 'Thread ID: th_feedback_1', variant: 'success', priority: 2, actionLabel: 'Copy ID',
      dismiss: 'cclocal:feedback-dismiss', dismissLabel: 'Dismiss feedback notice' });
    expect(localRows(client)).toEqual([['user', '/feedback broken diff'], ['assistant', 'Feedback sent to OpenAI.\n\nThread ID: `th_feedback_1`']]);
    view = snapshot(client, Date.parse('2026-10-08T03:00:00.000Z'));
    expect(view.composer.sendStatus).toBe('');
    expect(view.messages.filter(row => row.kind === 'assistant').every(row => !row.canFork)).toBe(true);
    await command(notice!.action);
    expect(native.copied).toEqual(['th_feedback_1']);
    // Dismissing removes the banner and its rows.
    expect(native.uploads).toHaveLength(1);
    await command('cclocal:feedback-dismiss', notice!.dismissId);
    expect(notices(client)).toEqual([]);
    expect(localRows(client)).toEqual([]);
  });

  test('a failed upload says the error; an interrupted one shows nothing; a second send then works', async () => {
    const { client, native, command, disk } = await codexThread();
    await command('send', '', '/feedback');
    expect(native.uploads).toEqual([{ threadId: 't1' }]);
    native.reply({ ok: false, error: { kind: 'ProviderUploadFeedbackError', message: 'The server refused the request (ProviderUploadFeedbackError).' } });
    await client.refresh(native, disk); await settle();
    expect(notices(client)).toMatchObject([{ title: 'Could not send feedback to OpenAI', description: 'Failed to upload feedback for thread t1.', variant: 'error' }]);
    expect(localRows(client).at(-1)).toEqual(['assistant', 'Could not send feedback to OpenAI.\n\nFailed to upload feedback for thread t1.']);

    await command('send', '', '/feedback again');
    native.reply({ ok: false, error: { kind: 'Interrupt', message: 'The server interrupted the request.' } });
    await client.refresh(native, disk); await settle();
    expect(notices(client).map(notice => notice.title)).toEqual(['Could not send feedback to OpenAI']);
    expect(localRows(client)).toHaveLength(2);
    expect(snapshot(client, Date.parse('2026-10-08T03:00:00.000Z')).composer.sendStatus).toBe('');
    // A transport failure keeps its own message.
    await command('send', '', '/feedback third');
    native.reply({ ok: false, error: { kind: 'transport', message: 'The socket closed.' } });
    await client.refresh(native, disk); await settle();
    expect(notices(client).at(-1)).toMatchObject({ description: 'The socket closed.' });
    // A reply that never comes because the connection changed ends as an interruption (no banner, no rows).
    const before = notices(client).length;
    await command('send', '', '/feedback fourth');
    expect(notices(client)[0]).toMatchObject({ title: 'Sending feedback to OpenAI...', priority: 0 });
    client.generation += 1; settleStaleReplies(client); // adoptStatus does this when the connection changes
    await settle();
    expect(notices(client)).toHaveLength(before);
    expect(snapshot(client, Date.parse('2026-10-08T03:00:00.000Z')).composer.sendStatus).toBe('');
  });

  test('a reply the inbox dropped, or a disconnected socket, ends as a failure the user sees', async () => {
    const { client, native, command, disk } = await codexThread();
    await command('send', '', '/feedback lost');
    settleLostReplies(client); await settle(); // client.ts drain: the inbox reset within this connection
    expect(notices(client)).toMatchObject([{ title: 'Could not send feedback to OpenAI', description: 'The server reply was lost. The request may already have been applied.' }]);
    expect(snapshot(client, Date.parse('2026-10-08T03:00:00.000Z')).composer.sendStatus).toBe('');
    native.keys.shift();
    await command('send', '', '/feedback again');
    native.reply({ ok: false, error: { kind: 'Disconnected', message: 'Disconnected from the server.' } });
    await client.refresh(native, disk); await settle();
    expect(notices(client).map(notice => notice.description)).toContain('Disconnected from the server.');
  });

  test('without a started Codex thread it warns and keeps the draft', async () => {
    const { client, native, command } = await codexThread();
    Object.assign(client.shell.threads.find(thread => thread.id === 't1')!, { activeProviderThreadId: null });
    await command('draft', '', '/feedback early');
    expect((await command('send', '', '/feedback early')).message).toBe('');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'warning', title: 'Start a Codex thread first', description: 'Send a message before you submit feedback.', stacked: true });
    expect(client.draft).toBe('/feedback early');
    expect(native.uploads).toEqual([]);
    // A new draft has no thread at all.
    const draft = await connected();
    await draft.command('draft', '', '/feedback');
    await draft.command('send', '', '/feedback');
    expect(toasts(draft.client).at(-1)?.title).toBe('Start a Codex thread first');
    expect(draft.native.committed.some(entry => entry.method === 'orchestration.launchThread')).toBe(false);
  });
});

describe('/feedback is an ordinary message elsewhere', () => {
  test('a non-Codex provider sends it as a turn', async () => {
    const { native, command } = await codexThread();
    await command('model', 'model-a', 'claude');
    await command('send', '', '/feedback broken diff');
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', text: '/feedback broken diff' });
    expect(native.uploads).toEqual([]);
  });
  test('an attached image, a staged file or folded paste, or any context chip makes it an ordinary prompt', async () => {
    const { client } = await codexThread();
    expect(feedbackCommandFor(client, '/feedback broken diff')).toEqual({ reason: 'broken diff' });
    // A folded paste stays staged until Send (composer-editor-files.ts); its chip still counts, as composerFiles does.
    const fold = stageFold(client.local, client.draftKey, client.environmentId, 'pasted', 'fold-1');
    expect(fold.status).toBe('staged');
    expect(feedbackCommandFor(client, `/feedback see ${fileChipLink(fold)}`)).toBeNull();
    expect(feedbackCommandFor(client, `/feedback ${contextLink('thread', contextId('thread', 't2'), 'Thread t2')}`)).toBeNull();
    expect(feedbackCommandFor(client, `/feedback ${contextLink('terminal', contextId('terminal', 'term-1'), 'Terminal 1')}`)).toBeNull();
    client.local.snapshotDrafts[client.draftKey] = [{ id: '00000000-0000-0000-0000-000000000001', mimeType: 'image/png', sizeBytes: 1, name: 'shot.png' }];
    expect(feedbackCommandFor(client, '/feedback with image')).toBeNull();
  });
  test('a several-model draft sends it to every model', async () => {
    const context = await connected();
    obj(obj(context.native.config.environment).capabilities).requiredWorktreeBootstrap = true;
    await context.client.refresh(context.native, context.disk);
    context.native.gesture = { modifiers: 'shift', source: 'pointer', ageMs: 12 };
    await context.command('model', 'model-b', 'codex');
    expect(snapshot(context.client).composer.fanout).toBe(true);
    await composerBranches(context.client, context.native, false, '');
    await context.command('send', '', '/feedback');
    expect(toasts(context.client).some(toast => toast.title === 'Start a Codex thread first')).toBe(false);
    expect(context.native.committed.filter(entry => entry.method === 'orchestration.launchThread').map(entry => str(obj(entry.initialMessage).text))).toEqual(['/feedback', '/feedback']);
  });
});
