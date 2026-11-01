// A side chat's own chat view (T3 Code 2359643cff, MIT, see LICENSE-T3: SideChatPane embeds ChatView for its thread):
// the window's presentation and commands over the client seen through the side chat (thread-scope.ts).
import { describe, expect, test } from 'bun:test';
import { arr, obj, str } from '../../src/client/domain';
import { threadScope, windowClient } from '../../src/client/thread-scope';
import { snapshot } from '../../src/client/presentation';
import { pinThreads } from '../../src/details/lineage-parent';
import { scopedCommand } from '../../src/panel/side-chat-thread';
import { opened, thread } from '../support/client-fixture';
import { pushToast, toasts } from '../../src/shell/toast';
import { layoutFrames, noteLayoutFrames } from '../../src/shell/layout-frames';
import { slashRows } from '../../src/composer/composer-editor-menu';
import { childThreadInputs, childThreadInputsTitle, childThreadInputsWaiting, pendingSubagentRequestsByThread } from '../../src/sidebar/subagent-requests';
import { subagentLead } from '../../src/sidebar/sidebar-lineage';
import { keptThread } from '../../src/client/keep-alive';
import { contractComponent } from '../support/contract-action-fixture';

async function withSideChat() {
  const context = await opened();
  const side = { ...thread('s1'), title: 'Side question', lineage: { parentThreadId: 't1', relationshipToParent: 'fork', sideChat: true } };
  context.native.shell.threads = [...arr(context.native.shell.threads), side];
  context.native.details.s1 = { snapshotSequence: 10, historyCursor: null, hasMoreHistory: false, latestLocalTurnOrdinal: 0,
    projection: { ...obj(context.native.details.t1!.projection), thread: side, messages: [] } };
  context.native.emit('shell', { kind: 'thread.updated', sequence: ++context.native.sequence, location: 'active', thread: side });
  await context.client.refresh(context.native, context.disk.files);
  await pinThreads(context.client, context.native, 'sideChat', 's1');
  await context.client.refresh(context.native, context.disk.files);
  return context;
}

describe('a thread scope', () => {
  test('reads its own thread and selection, and the window keeps its own', async () => {
    const { client } = await withSideChat();
    const scope = threadScope(client, 's1');
    expect(scope).not.toBe(client);
    expect(threadScope(client, 's1')).toBe(scope);
    expect(threadScope(client, 't1')).toBe(client);
    expect(windowClient(scope)).toBe(client);
    expect(scope.threadId).toBe('s1');
    expect(str(obj(scope.projection.thread).id)).toBe('s1');
    expect(scope.draftKey).toBe('env1:s1');
    expect(scope.modelId).toBe('model-a');
    scope.error = 'side failed';
    expect(client.error).toBe('');
    expect(client.threadId).toBe('t1');
  });
  test('its snapshot is the side chat\'s chat view', async () => {
    const { client } = await withSideChat();
    const data = snapshot(threadScope(client, 's1'), 1);
    expect(data.threadId).toBe('s1');
    expect(data.threadTitle).toBe('Side question');
    expect(snapshot(client, 1).threadId).toBe('t1');
  });
  test('a scoped send goes to the side chat, and the window draft stays', async () => {
    const context = await withSideChat();
    context.client.local.drafts['env1:t1'] = 'main draft';
    await scopedCommand(context.client, 'scope:s1|send', '', 'Hello side', 0, context.native, context.disk.files);
    const dispatched = context.native.committed.filter(payload => payload.type === 'message.dispatch');
    expect(dispatched.map(payload => [payload.threadId, payload.text])).toEqual([['s1', 'Hello side']]);
    expect(context.client.local.drafts['env1:t1']).toBe('main draft');
    expect(context.client.threadId).toBe('t1');
  });
});

describe('/side and /btw from the composer (useComposerSend)', () => {
  async function main(extra: { caps?: Record<string, unknown> } = {}) {
    const context = await opened();
    context.client.config = { ...context.client.config, environment: { ...obj(context.client.config.environment), capabilities: { ...obj(obj(context.client.config.environment).capabilities), sideChats: true, ...extra.caps } } };
    return context;
  }
  test('starts a side chat with the rest of the message and clears the draft', async () => {
    const context = await main();
    // The fork's thread has no detail in the stand-in; its message is recorded as committed.
    const commit = context.native.commit.bind(context.native);
    context.native.commit = (method, payload) => { if (payload.type !== 'message.dispatch' || context.native.details[String(payload.threadId)]) commit(method, payload); };
    await context.command('draft', '', '/btw what is this?');
    await context.command('send', '', '/btw what is this?');
    const types = context.native.committed.map(payload => payload.type);
    expect(types).toContain('thread.fork');
    expect(context.native.committed.find(payload => payload.type === 'thread.fork')).toMatchObject({ sourceThreadId: 't1', sideChat: true });
    expect(context.native.committed.find(payload => payload.type === 'message.dispatch')).toMatchObject({ text: 'what is this?' });
    expect(context.client.local.drafts['env1:t1']).toBe('');
  });
  test('refuses where side chats are unavailable, and with attachments', async () => {
    const old = await main({ caps: { sideChats: false } });
    await old.command('send', '', '/side hi');
    expect(toasts(old.client).at(-1)).toMatchObject({ kind: 'warning', title: 'Side chat not started', description: "Update this environment's T3 Code server to use side chats." });
    expect(old.native.committed.filter(payload => payload.type === 'message.dispatch')).toEqual([]);
    const attached = await main();
    attached.client.local.snapshotDrafts['env1:t1'] = [{ id: 'img', name: 'a.png' }];
    await attached.command('send', '', '/side hi');
    expect(toasts(attached.client).at(-1)).toMatchObject({ title: 'Side chats take text only', description: 'Remove the attachments and context, then send it again.' });
  });
  test('the slash menu offers them at the message start', () => {
    const rows = slashRows({ query: '', atPromptStart: true, planModeUiEnabled: false, compactAvailable: false, driver: 'codex', slashCommands: [], skills: [], showSkillsInSlashMenu: false, sideChatAvailable: true, sideChatDisabledReason: null });
    expect(rows.map(row => [row.label, row.description])).toEqual([['/model', 'Switch response model for this thread'], ['/side', 'Ask a side question in a new side chat'], ['/btw', 'Same as /side']]);
    const later = slashRows({ query: '', atPromptStart: false, planModeUiEnabled: false, compactAvailable: false, driver: 'codex', slashCommands: [], skills: [], showSkillsInSlashMenu: false, sideChatAvailable: true, sideChatDisabledReason: 'Why' });
    expect(later.map(row => row.label)).toEqual(['/model']);
  });
});

describe('a side chat on its parent (ThreadActivityBanners, MessagesTimeline)', () => {
  const sideShell = (extra: Record<string, unknown> = {}) => ({ id: 's1', title: 'Side question', status: 'running', activityRunStatus: 'running', lineage: { parentThreadId: 't1', relationshipToParent: 'fork', sideChat: true }, ...extra });
  test('its approval or question waits on the parent: "Side chat needs approval", "Side chat waiting"', () => {
    const threads = [{ id: 't1', title: 'Main', lineage: {} }, sideShell({ pendingRuntimeRequest: { kind: 'approval' } })];
    const children = childThreadInputs(threads, 't1');
    expect(children).toEqual([{ id: 's1', title: 'Side question', approval: true, sideChat: true }]);
    expect(childThreadInputsTitle(children)).toBe('Side chat needs approval');
    expect(childThreadInputsWaiting(children)).toBe('Side chat waiting');
    expect(pendingSubagentRequestsByThread(threads).get('t1')).toBe('approval');
    // A side chat asks its user directly, so its question rolls up too.
    const asking = [threads[0]!, sideShell({ pendingRuntimeRequest: { kind: 'user_input' }, status: 'waiting', activityRunStatus: 'waiting' })];
    expect(childThreadInputsTitle(childThreadInputs(asking, 't1'))).toBe('Side chat needs input');
    expect(pendingSubagentRequestsByThread(asking).get('t1')).toBe('user_input');
  });
  test('its timeline leads with "Context from" its parent, which opens nothing in the pane', async () => {
    const { client } = await withSideChat();
    const scoped = subagentLead(threadScope(client, 's1'));
    expect(scoped).toEqual([expect.objectContaining({ kind: 'fork', icon: 'messages-square', title: 'Context from', detail: 'Thread t1', targetId: '', actionLabel: '' })]);
  });
});

describe('what a scope shares with its window', () => {
  test('its toasts show in the window; its layout boxes and anchors are its "side-" ones', async () => {
    const { client } = await withSideChat();
    const scope = threadScope(client, 's1');
    pushToast(scope, { kind: 'warning', title: 'From the side chat' });
    expect(toasts(client).at(-1)?.title).toBe('From the side chat');
    noteLayoutFrames(client, 'toolbar:1,2,3,4;side-toolbar:5,6,7,8');
    expect(layoutFrames(client).toolbar).toEqual([1, 2, 3, 4]);
    expect(layoutFrames(scope)).toEqual({ toolbar: [5, 6, 7, 8] });
    client.presentation = { anchors: { controls: [0, 100], 'side-chat-controls': [0, 40], probe: [0, 9] } };
    expect(obj(scope.presentation.anchors)).toMatchObject({ controls: [0, 40], probe: [0, 9] });
  });
  test('a window op pressed in the pane acts on the window', async () => {
    const context = await withSideChat();
    await scopedCommand(context.client, 'scope:s1|select-thread', 't2', '', 0, context.native, context.disk.files);
    expect(context.client.threadId).toBe('t2');
  });
});

describe('what stays the window\'s, and what the side chat keeps', () => {
  test('a thread the side chat opens opens in the window; the side chat keeps its own (onOpenThread)', async () => {
    const context = await withSideChat();
    const scope = threadScope(context.client, 's1');
    await scope.openSelected(context.native, 't2');
    expect(context.client.threadId).toBe('t2');
    expect(scope.threadId).toBe('s1');
  });
  test('a new thread state written through the scope is its kept thread\'s (Load earlier, a stop\'s repair)', async () => {
    const { client } = await withSideChat();
    const scope = threadScope(client, 's1');
    const before = scope.thread!;
    scope.thread = { ...before, hasMore: true };
    expect(keptThread(client, 's1')?.hasMore).toBe(true);
    expect(scope.thread?.hasMore).toBe(true);
    expect(client.thread).not.toBe(scope.thread);
  });
  test('the model picker and the menus serve the window\'s composer again once they close', () => {
    const members = contractComponent('side-chat-members.contract', 'T3Code');
    const state = { composerTarget: 's1' };
    members.call('composerTargetReleased', state, { modelsOpen: true, optionsOpen: '' }, []);
    expect(state.composerTarget).toBe('s1');
    members.call('composerTargetReleased', state, { modelsOpen: false, optionsOpen: '' }, []);
    expect(state.composerTarget).toBe('');
  });
  test('an over-limit prompt arms the limit notice and does not send, as the window\'s submit does', () => {
    const members = contractComponent('side-chat-members.contract', 'T3Code');
    const sent: unknown[][] = [];
    members.call('sideSend', {}, { commandPending: false, sideChatThread: 's1', sideEditor: { overLimit: true }, sideText: 'long', sideRichOwner: 'o', sideDraftOwner: 'o',
      sideChat: { requestMode: '', canSend: true, snapshotDrafts: [], composer: { uploadBlock: '', queueEditAttachments: [] } },
      command: (...args: unknown[]) => args, sent: (name: string, args: unknown) => sent.push([name, args]) }, ['']);
    expect(sent).toEqual([['localChanged', ['scope:s1|editorlocal:arm-limit', '', '', 0]]]);
  });
});

test('a side chat\'s answers offer no Fork, which would promote the side chat (ChatView `embedded`)', async () => {
  const { forkable } = await import('../../src/timeline/timeline-presentation');
  const { client } = await withSideChat();
  const answer = { type: 'assistant_message', runId: 'run', status: 'completed' };
  expect(forkable(client, {}, answer)).toBe(true);
  expect(forkable(threadScope(client, 's1'), {}, answer)).toBe(false);
});

test('a side chat field\'s edit is answered from its own source session (composerIntent\'s last argument is its thread)', async () => {
  const { composerIntentAnswer } = await import('../../src/composer/composer-intent-source');
  const { composerSourceSession } = await import('../../src/composer/composer-rich-state');
  const { client } = await withSideChat();
  const scope = threadScope(client, 's1');
  composerSourceSession(scope).observe('side-owner', 'hi');
  const args = [scope.snapshotOwner, 'side-owner', 'hi', 1, 2, 2, 'key', '', 'Enter', false, false, false, 'rich', 's1'];
  expect(composerIntentAnswer(client, args).kind).toBe('stale');
  expect(composerIntentAnswer(scope, args).kind).not.toBe('stale');
  const members = (await import('../support/app-files')).appText('src/composer/composer-members.contract');
  expect(members).toContain('false, fieldRich.mode, fieldTarget)');
});

test('the module\'s field reads and edits reach the field of the client that asks (app.ts fieldNative)', async () => {
  const { composerFieldNative, composerFieldObserved } = await import('../../src/composer/composer-field-bridge');
  const { composerFieldOwner } = await import('../../src/composer/auto-balance-owner');
  const { client } = await withSideChat();
  const scope = threadScope(client, 's1');
  const owner = composerFieldOwner(scope);
  composerFieldObserved(scope, [{ generation: scope.generation, snapshotOwner: scope.snapshotOwner, owner, value: 'side text', start: 9, end: 9, composing: false, live: true, focused: true, inputSerial: 1 }, true], owner);
  const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as unknown as Parameters<typeof composerFieldNative>[1];
  const state = async (host: typeof client) => (await composerFieldNative(host, module, () => composerFieldOwner(host)).later({ op: 'editorState' })) as { value: Record<string, unknown> };
  expect((await state(scope)).value.text).toBe('side text');
  expect((await state(client)).value.text).not.toBe('side text');
});

test('an edit asked of the side chat\'s field while the engine serves the window\'s says it waits (composerFieldWaiting)', async () => {
  const { composerFieldNative, composerFieldObserved, composerFieldResource, composerFieldWaiting } = await import('../../src/composer/composer-field-bridge');
  const { composerFieldOwner } = await import('../../src/composer/auto-balance-owner');
  const { client } = await withSideChat();
  const scope = threadScope(client, 's1');
  const owner = composerFieldOwner(scope);
  const snapshot = { generation: scope.generation, snapshotOwner: scope.snapshotOwner, owner, value: 'side text', start: 9, end: 9, composing: false, live: true, focused: false, inputSerial: 1 };
  composerFieldObserved(scope, [snapshot, true], owner);
  const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as unknown as Parameters<typeof composerFieldNative>[1];
  expect(composerFieldWaiting(scope, [true, 0], owner)).toBe(0);
  const admitted = (await composerFieldNative(scope, module, () => owner).later({ op: 'editorInsert', text: '!', start: 9, end: 9 })) as { value: { seq: number } };
  const seq = admitted.value.seq;
  expect(seq).toBeGreaterThan(0);
  expect([composerFieldWaiting(scope, [true, 0], owner), composerFieldWaiting(scope, [false, 0], owner), composerFieldWaiting(scope, [true, seq], owner)]).toEqual([seq, 0, 0]);
  // Asking does not deliver it: the engine's own read, once it serves the field, does.
  expect(composerFieldResource(scope, [snapshot, 0], owner)).toMatchObject({ ready: true, seq, text: ' ! ' });
});
