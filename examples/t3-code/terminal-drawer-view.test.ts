// The drawer's client side (terminal-drawer-view.ts) against a fake client and native bridge: what
// toggle, New, Close, the exit report and thread delete send to the server (T3 Code 1e2ecbd975
// ChatView.tsx toggleTerminalVisibility / closeTerminal, useThreadActions.ts), the metadata stream's
// reconcile and the mounted threads the native side keeps attached.
import { describe, expect, test } from 'bun:test';

import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Files, Native } from './protocol';
import { closeThreadTerminals as cleanupCloseThreadTerminals } from './worktree-cleanup';
import { closeThreadTerminals, terminalRows, terminalKeybindings, terminalDrawerView, terminalMetadataEvent, terminalOps, terminalAvailable, terminalOpen, TERMINAL_METADATA_KEY } from './terminal-drawer-view';
import { surfaceStore } from './r4-surfaces-panel';
import { focusedTerminal, recordTerminalFocus } from './terminal-focus';
import { adoptTerminalPrefs, terminalUiStore } from './terminal-ui-state';

type Call = { method: string; payload: Obj };
function fixture(options: { fail?: string[]; connected?: boolean } = {}) {
  const calls: Call[] = [], native: Obj[] = [];
  const client = {
    environmentId: 'env-a', threadId: 'thread-1', projectId: 'project-1', generation: 3, connection: options.connected === false ? 'disconnected' : 'connected',
    ready: options.connected !== false, config: { keybindings: [] }, local: {} as object,
    shell: { projects: [{ id: 'project-1', workspaceRoot: '/repo' }], threads: [{ id: 'thread-1', projectId: 'project-1', worktreePath: '/repo/.t3/wt-1' }, { id: 'thread-2', projectId: 'project-1' }] },
    async request(_native: Native, method: string, payload: Obj) {
      calls.push({ method, payload });
      if (options.fail?.includes(method)) throw new Error(`${method} refused`);
      return {};
    },
    async call(_native: Native, request: Obj) { native.push(request); return request.op === 'subscribe' ? { id: '3-7' } : {}; },
  };
  return { client: client as unknown as T3Client, raw: client, calls, native };
}
const out = () => ({ message: '', id: '', value: '' });
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

  test('keeps the active and the ten most recent hidden open threads attached natively', async () => {
    const { client, native, raw } = fixture();
    raw.shell.threads = Array.from({ length: 12 }, (_, index) => ({ id: `thread-${index}`, projectId: 'project-1' }));
    for (let index = 0; index < 12; index += 1) {
      raw.threadId = `thread-${index}`;
      await run(client, 'toggle');
      await terminalDrawerView(client, { available: true } as Native, 840, index);
    }
    const last = native.filter(request => request.op === 'terminalRetain').at(-1)!;
    expect(last.sessions).toHaveLength(11);
    expect(last.sessions).not.toContain('["env-a","thread-0","term-1"]');
    expect(last.sessions).toContain('["env-a","thread-11","term-1"]');
    expect((await terminalDrawerView(client, null, 840, 99)).mounted).toBe(11);
  });
});

import { T3Client as Client } from './client';
import { launchThreadId, forgetDraftThreadId } from './r7-handoff-thread';
import { activeRef } from './terminal-drawer-view';
test('draft terminal reserves its send identity, worktree and session across first send', async () => {
  const client = new Client();
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


test('dedicated focus operations update pane context without dispatching a terminal command', async () => {
  const { client, calls } = fixture(), ref = { environmentId: 'env-a', threadId: 'thread-1' };
  const store = terminalUiStore(client).getState();
  store.ensureTerminal(ref, 'term-1', { open: true }); store.splitTerminal(ref, 'term-2');
  await run(client, 'focus-in', 'env-a:thread-1|term-2', 'drawer');
  expect(focusedTerminal(client)).toEqual({ ...ref, terminalId: 'term-2', surface: 'drawer' });
  await run(client, 'focus-out', 'env-a:thread-1|term-1', 'drawer');
  expect(focusedTerminal(client)?.terminalId).toBe('term-2');
  await run(client, 'focus-out', 'env-a:thread-1|term-2', 'drawer');
  expect(focusedTerminal(client)).toBeNull(); expect(calls).toEqual([]);
});
