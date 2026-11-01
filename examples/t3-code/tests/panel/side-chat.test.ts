// Side chats beside the main thread (T3 Code 2359643cff, MIT, see LICENSE-T3: SideChatPane.tsx, useSideChat.ts,
// sideChatPane.ts, rightPanelStore openSideChat, RightPanelTabs' side chat row and tab, useRightPanelSurfaceClose,
// reopenClosedView, ChatView sideChatDisabledReason, composer-logic parseSideChatCommand).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { initialShell } from '../../src/client/domain';
import {
  bringBackBlockedReason, parseSideChatCommand, sideChatDisabledReason, sideChatSurfaceRows, sideChatTabState, sideChatView, sideChatsOffered, worktreePending,
} from '../../src/panel/side-chat';
import { openNewSideChat, panelState, panelView, surfaceCommand, surfaceLocal } from '../../src/panel/panel-surfaces';
import { savedPanel } from '../../src/panel/saved-panel';
import { planNextReopen, type ClosedView, type ReopenOwner } from '../../src/panel/closed-views';
import { toasts } from '../../src/shell/toast';
import { contractComponent } from '../support/contract-action-fixture';

const native: Native = { available: true, watch() {}, async later() { return {}; } };
const parent = (extra: Obj = {}): Obj => ({ id: 'main', title: 'Main thread', status: 'idle', projectId: 'p1', lineage: { parentThreadId: null, relationshipToParent: null }, ...extra });
const side = (extra: Obj = {}): Obj => ({ id: 'side', title: 'Side question', status: 'idle', projectId: 'p1', lineage: { parentThreadId: 'main', relationshipToParent: 'fork', sideChat: true }, ...extra });

/** A focused client whose writes are recorded; `fail` names the command types the server refuses. */
function client(threads: Obj[] = [parent()], options: { caps?: Obj; scopes?: string[]; fail?: string[] } = {}) {
  const requests: Obj[] = [];
  let id = 0;
  const access = {
    ids: async (count: number) => Array.from({ length: count }, () => `id-${++id}`),
    request: async (method: string, payload: Obj) => {
      requests.push({ method, ...payload });
      if (options.fail?.includes(String(payload.type))) throw new Error(`${payload.type} refused`);
      return {};
    },
    call: async () => ({}),
  };
  const value = { local: {}, environmentId: 'env', threadId: 'main', projectId: 'p1', diffOpen: false, ready: true, writable: true, error: '', generation: 1,
    scopes: options.scopes ?? ['orchestration:operate', 'filesystem:read'], projection: { runs: [] },
    config: { environment: { capabilities: options.caps ?? { sideChats: true } }, providers: [] },
    shell: { ...initialShell(), threads, projects: [{ id: 'p1', title: 'P', workspaceRoot: '/repo' }] },
    get draftKey() { return `env:${this.threadId || 'new:p1'}`; }, restAccess: () => access };
  return { client: value as unknown as T3Client & { error: string; threadId: string; shell: { threads: Obj[] } }, requests };
}

describe('/side and /btw (parseSideChatCommand)', () => {
  test('the whole message, with an optional prompt', () => {
    expect(parseSideChatCommand('/side')).toEqual({ prompt: '' });
    expect(parseSideChatCommand('  /BTW what is this?\nsecond line ')).toEqual({ prompt: 'what is this?\nsecond line' });
    expect(parseSideChatCommand('/sidebar')).toBeNull();
    expect(parseSideChatCommand('ask /side later')).toBeNull();
  });
});

describe('where side chats are offered (ChatView sideChatsOffered, sideChatDisabledReason)', () => {
  test('a server thread offers the "+" menu and launcher row, S', () => {
    const { client: c } = client();
    expect(sideChatsOffered(c)).toBe(true);
    expect(sideChatSurfaceRows(c)).toEqual([{ id: 'side-chat', label: 'Side chat', icon: 'messages-square', shortcut: 'S', letter: 's', available: true, reason: '' }]);
  });
  test('a side chat, a subagent, an old server or a read-only connection offers none', () => {
    expect(sideChatSurfaceRows(client([parent({ lineage: { sideChat: true, parentThreadId: 'x' } })]).client)).toEqual([]);
    expect(sideChatSurfaceRows(client([parent({ lineage: { relationshipToParent: 'subagent' } })]).client)).toEqual([]);
    expect(sideChatSurfaceRows(client([parent()], { caps: {} }).client)).toEqual([]);
    expect(sideChatSurfaceRows(client([parent()], { scopes: [] }).client)).toEqual([]);
  });
  test('the reasons, in the reference order', () => {
    expect(sideChatDisabledReason(client([parent({ lineage: { sideChat: true } })]).client)).toBe('Side chats cannot start side chats.');
    expect(sideChatDisabledReason(client([parent({ lineage: { relationshipToParent: 'subagent' } })]).client)).toBe('Subagent threads cannot start side chats.');
    expect(sideChatDisabledReason(client([parent()], { caps: {} }).client)).toBe("Update this environment's T3 Code server to use side chats.");
    const draft = client([]); draft.client.threadId = '';
    expect(sideChatDisabledReason(draft.client)).toBe('Side chats start after the first message.');
    expect(sideChatSurfaceRows(draft.client)[0]).toMatchObject({ available: false, reason: 'Side chats start after the first message.' });
    const preparing = client();
    (preparing.client as unknown as { projection: Obj }).projection = { runs: [{ status: 'preparing', workspacePreparation: { type: 'worktree' } }] };
    expect(sideChatDisabledReason(preparing.client)).toBe("Available once the thread's worktree is set up.");
    expect(worktreePending('/wt', [{ status: 'preparing', workspacePreparation: { type: 'worktree' } }])).toBe(false);
  });
});

describe('starting one (useSideChat.open)', () => {
  test('forks the latest stable point as a hidden side chat, sends the prompt, then shows its tab', async () => {
    const { client: c, requests } = client();
    expect(await openNewSideChat(c, native, '  why is this slow? ')).toBe(true);
    expect(requests.map(request => request.type)).toEqual(['thread.fork', 'message.dispatch']);
    expect(requests[0]).toMatchObject({ sourceThreadId: 'main', sourcePoint: { type: 'latest_stable' }, sideChat: true, title: 'Side chat', createdBy: 'user', creationSource: 'web' });
    const target = String(requests[0]!.targetThreadId);
    expect(requests[1]).toMatchObject({ threadId: target, text: 'why is this slow?', titleSeed: 'why is this slow?', dispatchMode: { type: 'start_immediately' } });
    const state = panelState(c);
    expect([state.active, state.visible, state.surfaces.map(surface => [surface.kind, surface.path])]).toEqual([`side-chat:${target}`, true, [['side-chat', target]]]);
    // Not yet in this client's shell: the pane waits instead of calling it deleted.
    expect(sideChatView(c, 'main', target)).toMatchObject({ deleted: false, title: 'Side chat' });
  });
  test('the "+" menu row starts one without a message; a refused fork goes on the thread\'s banner and opens nothing', async () => {
    const plain = client();
    await surfaceLocal(plain.client, native, 'open', '', 'side-chat');
    expect(plain.requests.map(request => request.type)).toEqual(['thread.fork']);
    const refused = client([parent()], { fail: ['thread.fork'] });
    expect(await openNewSideChat(refused.client, native, 'hi')).toBe(false);
    expect([refused.client.error, panelState(refused.client).surfaces]).toEqual(['thread.fork refused', []]);
  });
  test('a message the server refuses drops the empty side chat (onlyIfUnused)', async () => {
    const { client: c, requests } = client([parent()], { fail: ['message.dispatch'] });
    expect(await openNewSideChat(c, native, 'hi')).toBe(false);
    await Promise.resolve();
    expect(requests.map(request => [request.type, request.onlyIfUnused ?? null])).toEqual([['thread.fork', null], ['message.dispatch', null], ['thread.delete', true]]);
    expect([c.error, panelState(c).surfaces]).toEqual(['message.dispatch refused', []]);
  });
});

describe('the pane (SideChatPane)', () => {
  const finished = { runs: [{ id: 'r1', ordinal: 1, status: 'completed' }] };
  test('Bring back says why it is unavailable', () => {
    expect(bringBackBlockedReason({ canOperate: false, parentActive: false, mergeRun: null })).toBe('This connection cannot change threads.');
    expect(bringBackBlockedReason({ canOperate: true, parentActive: true, mergeRun: {} })).toBe('Available when the main thread is idle.');
    expect(bringBackBlockedReason({ canOperate: true, parentActive: false, mergeRun: null })).toBe('Available after a finished reply.');
    expect(bringBackBlockedReason({ canOperate: true, parentActive: false, mergeRun: finished.runs[0]! })).toBeNull();
    const { client: c } = client([parent(), side()]);
    expect(sideChatView(c, 'main', 'side')).toMatchObject({ title: 'Side question', bringBackDisabled: true, bringBackTip: 'Available after a finished reply.',
      deleteDisabled: false, confirmDelete: true, deleteTitle: 'Delete side chat "Side question"?', deleteBody: 'This permanently clears its conversation history.',
      contextFrom: 'Main thread', loading: true });
  });
  test('a side chat the live shell no longer has reads as deleted', () => {
    expect(sideChatView(client([parent()]).client, 'main', 'gone')).toMatchObject({ deleted: true, deleteDisabled: true, canSend: false });
  });
  test('the tab: its title, and a dot with its state while it works or asks', async () => {
    const { client: c } = client([parent(), side({ status: 'running' })]);
    await surfaceLocal(c, native, 'side-chat-open', 'side', '');
    const view = await panelView(c, null, 0);
    expect(view.tabs.map(tab => [tab.kind, tab.title, tab.icon, tab.tone])).toEqual([['side-chat', 'Side question', 'messages-square', 'running']]);
    expect(view.kind).toBe('side-chat');
    expect(sideChatTabState(side({ pendingRuntimeRequest: { kind: 'approval' } }))).toBe('needs approval');
    expect(sideChatTabState(side({ pendingRuntimeRequest: { kind: 'user_input' } }))).toBe('needs input');
    expect(sideChatTabState(side())).toBe('');
  });
  test('a Lineage row opens only this thread\'s own side chat', async () => {
    const { client: c } = client([parent(), side(), side({ id: 'other', lineage: { parentThreadId: 'elsewhere', sideChat: true } })]);
    await surfaceLocal(c, native, 'side-chat-open', 'other', '');
    expect(panelState(c).surfaces).toEqual([]);
    await surfaceLocal(c, native, 'side-chat-open', 'side', '');
    expect(panelState(c).active).toBe('side-chat:side');
  });
  test('closing a tab deletes its side chat when unused; Delete closes the tab with no second delete', async () => {
    const closing = client([parent(), side()]);
    await surfaceLocal(closing.client, native, 'side-chat-open', 'side', '');
    await surfaceLocal(closing.client, native, 'close', 'side-chat:side', '');
    expect(closing.requests).toMatchObject([{ type: 'thread.delete', threadId: 'side', onlyIfUnused: true }]);
    const deleting = client([parent(), side()]);
    await surfaceLocal(deleting.client, native, 'side-chat-open', 'side', '');
    await surfaceCommand(deleting.client, native, {} as never, 'side-chat-delete', 'side', '');
    expect(deleting.requests.map(request => [request.type, request.onlyIfUnused ?? null])).toEqual([['thread.delete', null]]);
    expect(panelState(deleting.client).surfaces).toEqual([]);
    const refused = client([parent(), side()], { fail: ['thread.delete'] });
    await surfaceLocal(refused.client, native, 'side-chat-open', 'side', '');
    await surfaceCommand(refused.client, native, {} as never, 'side-chat-delete', 'side', '');
    expect(toasts(refused.client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Could not delete the side chat', 'thread.delete refused']]);
    expect(panelState(refused.client).surfaces).toHaveLength(1);
  });
  test('Delete asks first while Confirm thread deletion is on; the field sends to the side chat', () => {
    const body = contractComponent('side-chat.contract', 'SideChatBody'), local = body.state(), calls: unknown[][] = [];
    const view = { threadId: 'side', canSend: true, confirmDelete: true, deleteTitle: 'Delete side chat "S"?', deleteBody: 'Body', bringBackDisabled: false };
    const scope = { view, command: (...args: unknown[]) => calls.push(['command', ...args]), confirm: (...args: unknown[]) => calls.push(['confirm', ...args]) };
    body.call('remove', local, scope, []);
    body.call('edit', local, scope, ['  hello ']);
    body.call('send', local, scope, []);
    body.call('bringBack', local, scope, []);
    expect(calls).toEqual([
      ['confirm', 'ui:confirm:side-chat-delete', 'side', 'shell:surface-side-chat-delete', 'Delete side chat "S"?', 'Body', true],
      ['command', 'shell:surface-side-chat-send', 'side', '  hello ', 0],
      ['command', 'shell:surface-side-chat-bring-back', 'side', '', 0],
    ]);
    expect(local.text).toBe('');
  });
});

describe('a side chat tab outlives a relaunch and ⇧⌘T (rightPanelStore, reopenClosedView)', () => {
  test('saved with its thread; a malformed one is dropped', () => {
    const surface = { id: 'side-chat:side', kind: 'side-chat' as const, path: 'side', line: 0, reveal: 0 };
    expect(savedPanel({ surfaces: [surface], active: surface.id, visible: true })?.surfaces).toEqual([{ id: 'side-chat:side', kind: 'side-chat', path: 'side', line: 0 }]);
    expect(savedPanel({ surfaces: [{ ...surface, id: 'side-chat:other' }], active: '', visible: false })).toBeNull();
  });
  test('reopen skips one whose side chat is gone', () => {
    const entry: ClosedView = { id: 'closed', key: 'env:main', surface: { id: 'side-chat:side', kind: 'side-chat', path: 'side', line: 0, reveal: 0 } };
    const owner = (sideChatExists: boolean): ReopenOwner => ({ environmentKnown: true, catalogReady: true, ownerExists: true, shellLive: true, sideChatExists, panel: { visible: false, surfaces: [] } });
    expect(planNextReopen([entry], () => owner(false)).restore).toBeNull();
    expect(planNextReopen([entry], () => owner(true)).restore).toBe(entry);
  });
});
