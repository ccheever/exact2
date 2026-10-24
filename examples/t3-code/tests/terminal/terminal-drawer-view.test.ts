import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// The drawer's client side (terminal-drawer-view.ts) against a fake client and native bridge: what
// toggle, New, Close, the exit report and thread delete send to the server (T3 Code 1e2ecbd975
// ChatView.tsx toggleTerminalVisibility / closeTerminal, useThreadActions.ts), the metadata stream's
// reconcile and the messages of the panes' terminal pages (terminal-frames.ts).
import { describe, expect, test } from 'bun:test';

import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { closeThreadTerminals as cleanupCloseThreadTerminals } from '../../src/sidebar/worktree-cleanup';
import { closeThreadTerminals, terminalRows, terminalKeybindings, terminalDrawerView, terminalMetadataEvent, terminalOps, terminalAvailable, terminalOpen, TERMINAL_METADATA_KEY } from '../../src/terminal/terminal-drawer-view';
import { surfaceStore } from '../../src/panel/panel-surfaces';
import { focusedTerminal, recordTerminalFocus } from '../../src/terminal/terminal-focus';
import { adoptTerminalPrefs, terminalUiStore } from '../../src/terminal/terminal-ui-state';
import { contractComponent } from '../support/contract-action-fixture';
import { terminalFrameId } from '../../src/terminal/terminal-frames';

type Call = { method: string; payload: Obj };
function fixture(options: { fail?: string[]; connected?: boolean } = {}) {
  const calls: Call[] = [], native: Obj[] = [];
  const client = {
    scopes: [...AUTH_STANDARD_CLIENT_SCOPES],
    environmentId: 'env-a', threadId: 'thread-1', projectId: 'project-1', generation: 3, connection: options.connected === false ? 'disconnected' : 'connected',
    ready: options.connected !== false, config: { keybindings: [] }, local: {} as object,
    shell: { projects: [{ id: 'project-1', workspaceRoot: '/repo' }], threads: [{ id: 'thread-1', projectId: 'project-1', worktreePath: '/repo/.t3/wt-1' }, { id: 'thread-2', projectId: 'project-1' }] },
    async request(_native: Native, method: string, payload: Obj) {
      calls.push({ method, payload });
      if (options.fail?.includes(method)) throw new Error(`${method} refused`);
      return {};
    },
    async call(_native: Native, request: Obj) {
      native.push(request);
      if (request.op === 'terminalSocket') return { url: `ws://${String(request.environmentId)}/ws?wsTicket=t` };
      return request.op === 'subscribe' ? { id: '3-7' } : {};
    },
  };
  return { client: client as unknown as T3Client, raw: client, calls, native };
}
const out = () => ({ message: '', id: '', value: '' });
const session = (terminalId: string, extra: Obj = {}) => ({ threadId: 'thread-1', terminalId, cwd: '/repo/.t3/wt-1', worktreePath: '/repo/.t3/wt-1',
  status: 'running', pid: 1, exitCode: null, exitSignal: null, hasRunningSubprocess: false, label: 'zsh', updatedAt: '2026-10-06T00:00:00.000Z', ...extra });
const run = (client: T3Client, op: string, id = '', value = '') => terminalOps.call(client, `terminallocal:${op}`, id, value, 0, {} as Native, {} as Files, out());

describe('terminal drawer commands', () => {
  test('⌘J on a thread with no terminal opens term-1 in the worktree with the project script env', async () => {
    const { client, calls } = fixture();
    expect(terminalAvailable(client)).toBe(true);
    expect(terminalOpen(client)).toBe(false);
    await run(client, 'toggle');
    expect(calls).toEqual([{ method: 'terminal.open', payload: { threadId: 'thread-1', terminalId: 'term-1', cwd: '/repo/.t3/wt-1', worktreePath: '/repo/.t3/wt-1',
      env: { T3CODE_PROJECT_ROOT: '/repo', T3CODE_WORKTREE_PATH: '/repo/.t3/wt-1' } } }]);
    expect(terminalOpen(client)).toBe(true);
    const view = await terminalDrawerView(client, { available: true } as Native, 840, 0);
    expect(view).toMatchObject({ open: true, terminalId: 'term-1', label: 'Terminal 1', height: 280, cwd: '/repo/.t3/wt-1', threadKey: 'env-a:thread-1',
      closeTitle: 'Close terminal "Terminal 1"?', closeBody: 'This stops the running process and clears its history.', sessionKey: '["env-a","thread-1","term-1"]', focusRequest: 1 });
    // A second toggle only hides the drawer; the session stays on the server.
    await run(client, 'toggle');
    expect(calls).toHaveLength(1);
    expect(terminalOpen(client)).toBe(false);
    // The clipped native view stays mounted through closing, without requesting focus.
    const hidden = await terminalDrawerView(client, null, 840, 0);
    expect(hidden.panes).toMatchObject([{ terminalId: 'term-1', focusRequest: 0 }]);
    await run(client, 'toggle');
    const reopened = await terminalDrawerView(client, null, 840, 0);
    expect(reopened.panes).toMatchObject([{ terminalId: 'term-1', focusRequest: 2 }]);
  });

  test('⌘J dispatches the toggle where the drawer can open (keyboard-dispatch MAIN_ROWS)', () => {
    const { client, raw } = fixture();
    const added: string[][] = [];
    const add = (command: string, kind: string, target: string, label: string) => { added.push([command, kind, target, label]); };
    terminalRows(add, client);
    expect(added).toEqual([['terminal.toggle', 'command', 'terminallocal:toggle', 'Toggle Terminal']]);
    raw.threadId = '';
    terminalRows(add, client);
    expect(added).toHaveLength(2);
  });

  test('a thread without a worktree opens in the project root and sends no worktree path', async () => {
    const { client, calls, raw } = fixture();
    raw.threadId = 'thread-2';
    await run(client, 'toggle');
    expect(calls[0]).toEqual({ method: 'terminal.open', payload: { threadId: 'thread-2', terminalId: 'term-1', cwd: '/repo', env: { T3CODE_PROJECT_ROOT: '/repo' } } });
  });

  test('a thread without a project has no drawer', async () => {
    const { client, calls, raw } = fixture();
    raw.shell.projects = [];
    expect(terminalAvailable(client)).toBe(false);
    await run(client, 'toggle');
    expect(calls).toEqual([]);
    expect((await terminalDrawerView(client, null, 840, 0)).toggleLabel).toBe('Terminal drawer is unavailable');
  });

  test('Close deletes the session with its history and removes the tab at once', async () => {
    const { client, calls } = fixture();
    await run(client, 'toggle');
    await run(client, 'close', 'env-a:thread-1|term-1');
    expect(calls.slice(1)).toEqual([{ method: 'terminal.close', payload: { threadId: 'thread-1', terminalId: 'term-1', deleteHistory: true } }]);
    expect(terminalOpen(client)).toBe(false);
    expect((await terminalDrawerView(client, null, 840, 0)).terminalId).toBe('');
  });

  test('a failing close types exit into the shell instead', async () => {
    const { client, calls } = fixture({ fail: ['terminal.close'] });
    await run(client, 'toggle');
    await run(client, 'close', 'env-a:thread-1|term-1');
    expect(calls.slice(1).map(call => [call.method, call.payload.data ?? call.payload.deleteHistory])).toEqual([['terminal.close', true], ['terminal.write', 'exit\n']]);
    expect((await terminalDrawerView(client, null, 840, 0)).failure).toBe('');
  });

  test('an exit closes the tab without asking, through the same close', async () => {
    const { client, calls } = fixture();
    await run(client, 'toggle');
    await run(client, 'exited', 'env-a:thread-1', JSON.stringify({ type: 'exited', terminalId: 'term-1', thread: 'thread-1' }));
    expect(calls.map(call => call.method)).toEqual(['terminal.open', 'terminal.close']);
    expect(terminalOpen(client)).toBe(false);
  });

  test('the dragged height is kept per thread, clamped to the window', async () => {
    const { client } = fixture();
    await run(client, 'toggle');
    await terminalDrawerView(client, null, 840, 0);
    await run(client, 'height', 'env-a:thread-1', '400');
    expect((await terminalDrawerView(client, null, 840, 0)).height).toBe(400);
    await run(client, 'height', 'env-a:thread-1', '9000');
    expect((await terminalDrawerView(client, null, 840, 0)).height).toBe(630);
    // Saved in the preference record and read back after a relaunch.
    const saved = JSON.parse(JSON.stringify(client.local)) as Obj;
    const next = {};
    adoptTerminalPrefs(next, saved);
    const reopened = terminalUiStore({ local: next }).getState().terminalUiStateByThreadKey['env-a:thread-1'];
    expect(reopened).toMatchObject({ terminalOpen: true, terminalHeight: 630, terminalIds: ['term-1'], activeTerminalId: 'term-1' });
  });

  test('the drawer stops short of pushing the composer into the header (resolveTerminalDrawerMaxHeight)', async () => {
    const { client } = fixture();
    await run(client, 'toggle');
    await terminalDrawerView(client, null, 840, 0);
    await run(client, 'height', 'env-a:thread-1', '600');
    const drawer = contractComponent('terminal.contract', 'TerminalDrawer'), state = drawer.state();
    const view = await terminalDrawerView(client, null, 840, 0);
    const height = (viewportHeight: number, composerHeight: number) => drawer.derives(state, { view, viewportHeight, composerHeight, look: { panelMs: 0 },
      reduced: false, min: Math.min, max: Math.max, floor: Math.floor, command: () => {} }).height;
    // 75% of an 840pt window is 630; the 52pt header and a 170pt composer leave 618 of the column.
    expect(height(840, 0)).toBe(600);
    expect(height(840, 300)).toBe(488);
    // Below the 180pt minimum the column's room still wins.
    expect(height(400, 250)).toBe(98);
  });
  test('a smaller window and a drag returning to its start preserve the preferred height', async () => {
    const { client } = fixture();
    await run(client, 'toggle');
    await terminalDrawerView(client, null, 840, 0);
    await run(client, 'height', 'env-a:thread-1', '600');
    const drawer = contractComponent('terminal.contract', 'TerminalDrawer');
    const state = drawer.state(), sent: unknown[][] = [];
    const view = await terminalDrawerView(client, null, 620, 0);
    expect(view.height).toBe(465);
    const scope = { view, viewportHeight: 620, composerHeight: 0, look: { panelMs: 0 }, reduced: false,
      min: Math.min, max: Math.max, floor: Math.floor, command: (...args: unknown[]) => sent.push(args) };
    const call = (name: string, dy: number) => drawer.call(name, state, { ...scope, ...drawer.derives(state, scope) }, [0, dy]);
    call('resize', 20);
    expect(drawer.derives(state, scope).height).toBe(445);
    call('resize', -20);
    call('release', 0);
    expect(sent).toEqual([]);
    expect(state.dragHeight).toBe(0);
    expect((await terminalDrawerView(client, null, 840, 0)).height).toBe(600);

    call('resize', 20);
    call('release', 0);
    expect(sent).toEqual([['terminallocal:height', 'env-a:thread-1', '445', 0]]);
  });

  test('New in the empty state allocates the lowest free id the server and the drawer both leave', async () => {
    const { client, calls } = fixture();
    terminalMetadataEvent(client, { subscriptionId: '3-1', value: { type: 'snapshot', terminals: [{ threadId: 'thread-1', terminalId: 'term-1', cwd: '/repo', worktreePath: null,
      status: 'running', pid: 1, exitCode: null, exitSignal: null, hasRunningSubprocess: false, label: 'zsh', updatedAt: '2026-10-06T00:00:00.000Z' }] } });
    await run(client, 'new', 'env-a:thread-1');
    expect(calls[0]?.payload.terminalId).toBe('term-2');
  });

  test('thread delete closes every session of the thread with its history, then forgets its drawer', async () => {
    const { client, calls } = fixture();
    await run(client, 'toggle');
    await closeThreadTerminals(client, {} as Native, 'thread-1');
    expect(calls[1]).toEqual({ method: 'terminal.close', payload: { threadId: 'thread-1', deleteHistory: true } });
    expect(terminalUiStore(client).getState().terminalUiStateByThreadKey['env-a:thread-1']).toBeUndefined();
  });
});

test('the sidebar delete hook (worktree-cleanup) closes terminals through the drawer', () => {
  expect(cleanupCloseThreadTerminals).toBe(closeThreadTerminals);
});

describe('terminal drawer resource', () => {
  test('subscribes to the metadata stream once, labels the terminal from it and keeps closed ids hidden', async () => {
    const { client, native } = fixture();
    await run(client, 'toggle');
    await terminalDrawerView(client, { available: true } as Native, 840, 0);
    await terminalDrawerView(client, { available: true } as Native, 840, 10);
    expect(native.filter(request => request.op === 'subscribe')).toEqual([{ op: 'subscribe', key: TERMINAL_METADATA_KEY, method: 'subscribeTerminalMetadata', payload: {} }]);
    const summary = (terminalId: string, label: string) => ({ threadId: 'thread-1', terminalId, cwd: '/repo', worktreePath: null, status: 'running', pid: 1, exitCode: null,
      exitSignal: null, hasRunningSubprocess: true, label, updatedAt: '2026-10-06T00:00:00.000Z' });
    terminalMetadataEvent(client, { subscriptionId: '3-7', value: { type: 'snapshot', terminals: [summary('term-1', 'bun')] } });
    expect((await terminalDrawerView(client, null, 840, 20)).label).toBe('bun');
    // The server knows another terminal: the drawer follows (reconcileTerminalIds).
    terminalMetadataEvent(client, { subscriptionId: '3-7', value: { type: 'upsert', terminal: summary('term-3', 'Terminal 3') } });
    await terminalDrawerView(client, null, 840, 30);
    expect(terminalUiStore(client).getState().terminalUiStateByThreadKey['env-a:thread-1']?.terminalIds).toEqual(['term-1', 'term-3']);
    // A closed terminal stays hidden from stale metadata.
    await run(client, 'close', 'env-a:thread-1|term-3');
    await terminalDrawerView(client, null, 840, 40);
    expect(terminalUiStore(client).getState().terminalUiStateByThreadKey['env-a:thread-1']?.terminalIds).toEqual(['term-1']);
  });

  test('a session without terminal:operate gets the server message once and no retry loop', async () => {
    const { client, native } = fixture();
    await terminalDrawerView(client, { available: true } as Native, 840, 0);
    terminalMetadataEvent(client, { subscriptionId: '3-7', value: { _transportError: { kind: 'EnvironmentAuthorizationError', message: 'The authenticated token is missing required scope: terminal:operate.' } } });
    for (const now of [5000, 10_000, 60_000]) await terminalDrawerView(client, { available: true } as Native, 840, now);
    expect(native.filter(request => request.op === 'subscribe')).toHaveLength(1);
    expect((await terminalDrawerView(client, null, 840, 70_000)).metadata).toContain('terminal:operate');
  });

  test('a pane is its terminal page: the session it attaches to, a socket ticket for its environment, its posts and its menu', async () => {
    const { client, native } = fixture();
    await run(client, 'toggle');
    const frame = (await terminalDrawerView(client, null, 840, 0)).panes[0]!.frame;
    expect(frame.id).toBe(terminalFrameId('drawer', 'env-a', 'thread-1', 'term-1'));
    expect(frame.src).toBe('assets/terminal/index.html');
    expect(JSON.parse(frame.session)).toEqual({ mode: 'session', identity: { environmentId: 'env-a', threadId: 'thread-1', terminalId: 'term-1', surface: 'drawer' },
      readOnly: false, canAddToChat: true, attach: { threadId: 'thread-1', terminalId: 'term-1', cwd: '/repo/.t3/wt-1', worktreePath: '/repo/.t3/wt-1',
        env: { T3CODE_PROJECT_ROOT: '/repo', T3CODE_WORKTREE_PATH: '/repo/.t3/wt-1' } } });
    const identity = { environmentId: 'env-a', threadId: 'thread-1', terminalId: 'term-1', surface: 'drawer' };
    // A post waits for the page's boot; the boot drops what an earlier document was sent.
    await run(client, 'message', frame.id, JSON.stringify({ type: 'boot' }));
    await run(client, 'message', frame.id, JSON.stringify({ type: 'ticket', ...identity }));
    expect(native.filter(request => request.op === 'terminalSocket')).toEqual([{ op: 'terminalSocket', environmentId: 'env-a' }]);
    let posts = (await terminalDrawerView(client, null, 840, 1)).panes[0]!.frame.posts;
    expect(posts.map(post => JSON.parse(post.text))).toEqual([{ type: 'socket', url: 'ws://env-a/ws?wsTicket=t', serial: 0 }]);
    await run(client, 'posted', frame.id, posts[0]!.key);
    expect((await terminalDrawerView(client, null, 840, 2)).panes[0]!.frame.posts).toEqual([]);
    // The selection popup, then Copy: the raw selection goes to the clipboard and the terminal takes the focus back.
    const selection = { text: 'one\r\ntwo\n', position: { start: { x: 0, y: 2 }, end: { x: 3, y: 3 } } };
    await run(client, 'message', frame.id, JSON.stringify({ type: 'selection-ready', x: 40, y: 50, ...selection, ...identity }));
    const menu = (await terminalDrawerView(client, null, 840, 3)).panes[0]!.frame.menu;
    expect(menu).toEqual({ serial: 1, x: 40, y: 50, rows: [{ id: 'add-to-chat', label: 'Add to chat', disabled: false }, { id: 'copy', label: 'Copy', disabled: false }] });
    await run(client, 'menu', frame.id, `${menu.serial}|copy`);
    expect(native.filter(request => request.op === 'copyText')).toEqual([{ op: 'copyText', text: 'one\r\ntwo\n' }]);
    posts = (await terminalDrawerView(client, null, 840, 4)).panes[0]!.frame.posts;
    expect(posts.map(post => JSON.parse(post.text))).toEqual([{ type: 'focus' }]);
    // A right-click's menu: Paste, Select all and Jump to latest act on the page; a superseded menu's row does nothing.
    await run(client, 'message', frame.id, JSON.stringify({ type: 'contextmenu', x: 5, y: 6, text: '', position: null, ...identity }));
    expect((await terminalDrawerView(client, null, 840, 5)).panes[0]!.frame.menu.rows.map(row => [row.id, row.label, row.disabled])).toEqual([
      ['add-to-chat', 'Add to chat', true], ['copy', 'Copy', true], ['paste', 'Paste', false], ['select-all', 'Select all', false], ['scroll-to-bottom', 'Jump to latest', false]]);
    await run(client, 'menu', frame.id, '1|select-all');
    await run(client, 'menu', frame.id, '2|scroll-to-bottom');
    posts = (await terminalDrawerView(client, null, 840, 6)).panes[0]!.frame.posts;
    expect(posts).toHaveLength(1);
    await run(client, 'posted', frame.id, posts[0]!.key);
    const later = (await terminalDrawerView(client, null, 840, 7)).panes[0]!.frame.posts;
    expect([...posts, ...later].map(post => JSON.parse(post.text).type)).toEqual(['focus', 'scrollToBottom']);
  });
});

import { T3Client as Client } from '../../src/client/client';
import { launchThreadId, forgetDraftThreadId } from '../../src/composer/draft-thread-id';
import { activeRef } from '../../src/terminal/terminal-drawer-view';
test('draft terminal reserves its send identity, worktree and session across first send', async () => {
  const client = new Client();
  Object.assign(client, { scopes: [...AUTH_STANDARD_CLIENT_SCOPES], connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  client.environmentId = 'env'; client.projectId = 'project'; client.threadId = '';
  client.shell.projects = [{ id: 'project', workspaceRoot: '/repo' }];
  client.local.composerControls.contexts[client.draftKey] = { envMode: 'worktree', branch: 'feature', worktreePath: '/wt' };
  const calls: Call[] = [];
  client.restAccess = () => ({ ids: async () => ['reserved-thread'] }) as ReturnType<Client['restAccess']>;
  client.request = async (_native, method, payload) => { calls.push({ method, payload }); return {}; };
  const draftKey = client.draftKey;
  expect(terminalAvailable(client)).toBe(true);
  await run(client, 'toggle');
  expect(activeRef(client)).toEqual({ environmentId: 'env', threadId: 'reserved-thread' });
  expect(calls[0]).toMatchObject({ method: 'terminal.open', payload: { threadId: 'reserved-thread', cwd: '/wt', worktreePath: '/wt' } });
  expect(launchThreadId(client, draftKey, 'unused-fresh')).toBe('reserved-thread');
  client.threadId = 'reserved-thread'; client.shell.threads = [{ id: client.threadId, projectId: 'project', worktreePath: '/wt' }];
  forgetDraftThreadId(client, draftKey);
  expect(terminalOpen(client)).toBe(true);
  expect((await terminalDrawerView(client, null, 840, 0)).terminalId).toBe('term-1');
});
test('closing the last pane and hiding the drawer return composer focus; a remaining pane keeps focus', async () => {
  const { client } = fixture();
  await run(client, 'toggle'); await run(client, 'split', 'env-a:thread-1|term-1');
  let result = out();
  await terminalOps.call(client, 'terminallocal:close', 'env-a:thread-1|term-2', '', 0, {} as Native, {} as Files, result);
  expect(result.message).toBe('');
  result = out();
  await terminalOps.call(client, 'terminallocal:close', 'env-a:thread-1|term-1', '', 0, {} as Native, {} as Files, result);
  expect(result.message).toBe('terminal:focus-composer');
  await run(client, 'toggle'); result = out();
  await terminalOps.call(client, 'terminallocal:toggle', '', '', 0, {} as Native, {} as Files, result);
  expect(result.message).toBe('terminal:focus-composer');
});


test('terminal keys use actual running state for custom when clauses', () => {
  const { client } = fixture();
  client.config.keybindings = [{ command: 'thread.stop', shortcut: { key: 'x', modKey: true }, whenAst: { type: 'identifier', name: 'turnRunning' } }];
  client.projection = { runs: [{ status: 'running' }] };
  expect(JSON.parse(terminalKeybindings(client))).toEqual([{ chord: 'Meta+x', command: 'thread.stop' }]);
  client.projection = { runs: [{ status: 'completed' }] };
  expect(JSON.parse(terminalKeybindings(client))).toEqual([]);
});

test('a retained drawer session exiting never takes focus from the panel', async () => {
  for (const open of [true, false]) {
    const { client } = fixture();
    await run(client, 'toggle');
    if (!open) await run(client, 'toggle');
    const panel = { environmentId: 'env-a', threadId: 'thread-1', terminalId: 'term-2', surface: 'terminal:term-2' };
    recordTerminalFocus(client, { ...panel, focused: true });
    const result = out();
    await terminalOps.call(client, 'terminallocal:exited', 'env-a:thread-1|term-1', JSON.stringify({ type: 'exited', terminalId: 'term-1' }), 0, {} as Native, {} as Files, result);
    expect(result.message).toBe('');
    expect(focusedTerminal(client)).toEqual(panel);
    expect((await terminalDrawerView(client, null, 840, 0)).panes).toEqual([]);
  }
});

test('immediate native close targets the emitting split before focus state refresh', async () => {
  const { client, calls } = fixture(), ref = { environmentId: 'env-a', threadId: 'thread-1' };
  const store = terminalUiStore(client).getState();
  store.ensureTerminal(ref, 'term-1', { open: true }); store.splitTerminal(ref, 'term-2');
  store.setActiveTerminal(ref, 'term-1');
  await run(client, 'message', 'env-a:thread-1|term-2', JSON.stringify({ type: 'command', command: 'terminal.close', ...ref, terminalId: 'term-2', surface: 'drawer' }));
  expect(surfaceStore(client).terminalClose).toEqual({ serial: 1, title: 'Close terminal "Terminal 2"?', body: 'This stops the running process and clears its history.', target: 'env-a:thread-1|term-2', op: 'terminallocal:close' });
  expect(calls).toEqual([]);
});

test('native toggle preserves composer-focus result and rejects obsolete scope', async () => {
  const { client } = fixture(), ref = { environmentId: 'env-a', threadId: 'thread-1' }, result = out();
  terminalUiStore(client).getState().ensureTerminal(ref, 'term-1', { open: true });
  const message = { type: 'command', command: 'terminal.toggle', ...ref, terminalId: 'term-1', surface: 'drawer' };
  await terminalOps.call(client, 'terminallocal:message', 'env-a:thread-1|term-1', JSON.stringify(message), 0, {} as Native, {} as Files, result);
  expect(result.message).toBe('terminal:focus-composer'); expect(terminalOpen(client)).toBe(false);
  await run(client, 'message', '', JSON.stringify({ ...message, environmentId: 'old-env' }));
  expect(terminalOpen(client)).toBe(false);
});


test('the page\'s focus reports update pane context without dispatching a terminal command', async () => {
  const { client, calls } = fixture(), ref = { environmentId: 'env-a', threadId: 'thread-1' };
  const store = terminalUiStore(client).getState();
  store.ensureTerminal(ref, 'term-1', { open: true }); store.splitTerminal(ref, 'term-2');
  const focus = (terminalId: string, focused: boolean) => run(client, 'message', terminalFrameId('drawer', 'env-a', 'thread-1', terminalId),
    JSON.stringify({ type: 'focus', focused, ...ref, terminalId, surface: 'drawer' }));
  await focus('term-2', true);
  expect(focusedTerminal(client)).toEqual({ ...ref, terminalId: 'term-2', surface: 'drawer' });
  await focus('term-1', false);
  expect(focusedTerminal(client)?.terminalId).toBe('term-2');
  await focus('term-2', false);
  expect(focusedTerminal(client)).toBeNull(); expect(calls).toEqual([]);
});

describe('project actions in the terminal', () => {
  const ref = { environmentId: 'env-a', threadId: 'thread-1' };
  test('before the server lists sessions, an action starts a new terminal instead of guessing one is idle', async () => {
    const { client, calls } = fixture();
    terminalUiStore(client).getState().ensureTerminal(ref, 'term-1', { open: true });
    await run(client, 'run-command', '', 'bun dev');
    expect(calls[0]).toMatchObject({ method: 'terminal.open', payload: { terminalId: 'term-2', cols: 120, rows: 30 } });
  });

  test('reuses an idle drawer terminal, never a right-panel one, and types a shell command as is', async () => {
    const { client, calls } = fixture();
    terminalMetadataEvent(client, { subscriptionId: '3-1', value: { type: 'snapshot', terminals: [session('term-1'), session('term-2')] } });
    const store = terminalUiStore(client).getState();
    store.ensureTerminal(ref, 'term-1', { open: true });
    // term-1 is also a right-panel terminal; the server's term-2 is one this drawer has not opened yet.
    surfaceStore(client).panels.set('env-a:thread-1', { surfaces: [{ id: 'terminal:1', kind: 'terminal', path: '', line: 0, reveal: 0,
      terminal: { terminalIds: ['term-1'], activeTerminalId: 'term-1', splitDirection: 'horizontal' } }], active: 'terminal:1', visible: true, userRevision: 1 });
    await run(client, 'run-command', '', 'git status');
    expect(calls.map(call => [call.method, call.payload.terminalId, call.payload.cols, call.payload.data])).toEqual([
      ['terminal.open', 'term-2', undefined, undefined], ['terminal.write', 'term-2', undefined, 'git status\r']]);
  });

  test('a busy drawer terminal is skipped for the next idle one', async () => {
    const { client, calls } = fixture();
    terminalMetadataEvent(client, { subscriptionId: '3-1', value: { type: 'snapshot', terminals: [session('term-1', { hasRunningSubprocess: true }), session('term-2')] } });
    const store = terminalUiStore(client).getState();
    store.ensureTerminal(ref, 'term-1', { open: true }); store.newTerminal(ref, 'term-2'); store.setActiveTerminal(ref, 'term-1');
    await run(client, 'run-command', '', 'ls');
    expect(calls[0]?.payload.terminalId).toBe('term-2');
  });

  test('a checkout terminal keeps the project root while the thread works in a worktree', async () => {
    const { client } = fixture();
    terminalMetadataEvent(client, { subscriptionId: '3-1', value: { type: 'snapshot', terminals: [session('term-1', { cwd: '/repo', worktreePath: null })] } });
    const store = terminalUiStore(client).getState();
    store.ensureTerminal(ref, 'term-1', { open: true }); store.splitTerminal(ref, 'term-2');
    const view = await terminalDrawerView(client, null, 840, 0);
    expect(view.panes.map(pane => [pane.terminalId, pane.cwd, pane.worktree, pane.env])).toEqual([
      ['term-1', '/repo', '', JSON.stringify({ T3CODE_PROJECT_ROOT: '/repo' })],
      ['term-2', '/repo/.t3/wt-1', '/repo/.t3/wt-1', JSON.stringify({ T3CODE_PROJECT_ROOT: '/repo', T3CODE_WORKTREE_PATH: '/repo/.t3/wt-1' })]]);
  });

  test('a reconnect to the same environment keeps the known sessions until its first snapshot', async () => {
    const { client, raw } = fixture();
    await terminalDrawerView(client, { available: true } as Native, 840, 0);
    terminalMetadataEvent(client, { subscriptionId: '3-7', value: { type: 'snapshot', terminals: [session('term-1', { cwd: '/repo', worktreePath: null })] } });
    terminalUiStore(client).getState().ensureTerminal(ref, 'term-1', { open: true });
    raw.generation = 4;
    const reconnected = await terminalDrawerView(client, { available: true } as Native, 840, 0);
    expect(reconnected.panes[0]).toMatchObject({ cwd: '/repo', worktree: '' });
    // Another environment's sessions are not this one's.
    raw.generation = 5; raw.environmentId = 'env-b';
    expect((await terminalDrawerView(client, { available: true } as Native, 840, 0)).metadata).toBe('waiting');
  });
});
