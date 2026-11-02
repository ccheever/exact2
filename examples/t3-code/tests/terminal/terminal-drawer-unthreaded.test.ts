import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// The drawer of a draft and of a background environment's thread.
// T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3) ChatView.tsx: a draft is the active thread under its own
// `draftThread.threadId`, `toggleTerminalVisibility` opens its `term-1` in the project root, and the mounted drawers
// keep their state per thread. The clone opens a background environment's thread by moving the focus there
// (settings-b-fleet.ts); each pane's terminal page attaches to its own environment's server (terminal-frames.ts), so a
// drawer shown again names the same session and gets its history back from the attach snapshot.
import { afterEach, expect, test } from 'bun:test';

import { T3Client as Client } from '../../src/client/client';
import { initialShell, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { fleet } from '../../src/connections/settings-b-fleet';
import { terminalDrawerView, terminalOps } from '../../src/terminal/terminal-drawer-view';

type Call = { method: string; payload: Obj };
const native = { available: true } as Native;

function focusedClient() {
  const client = new Client();
  Object.assign(client, { scopes: [...AUTH_STANDARD_CLIENT_SCOPES], connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  const calls: Call[] = [];
  client.environmentId = 'env-a'; client.projectId = 'project-a'; client.threadId = '';
  client.shell.projects = [{ id: 'project-a', workspaceRoot: '/a' }];
  client.shell.threads = [{ id: 'a-1', projectId: 'project-a' }];
  client.shellLoaded = true;
  let ids = 0;
  client.restAccess = () => ({ ids: async () => [`draft-${++ids}`] }) as ReturnType<Client['restAccess']>;
  client.request = async (_native, method, payload) => { calls.push({ method, payload }); return {}; };
  client.call = async () => ({});
  return { client, calls };
}
const toggle = (client: Client) => terminalOps.call(client, 'terminallocal:toggle', '', '', 0, native, {} as Files, { message: '', id: '', value: '' });
const view = (client: Client, now = 0) => terminalDrawerView(client, native, 840, now);
const key = (environment: string, thread: string) => JSON.stringify([environment, thread, 'term-1']);

/** adoptStatus moving the focus (client.ts): the new environment's selection, its shell not loaded yet. */
function focus(client: Client, environmentId: string, projectId: string, threadId: string) {
  client.environmentId = environmentId; client.projectId = projectId; client.threadId = threadId;
  client.shell = initialShell(); client.shellLoaded = false;
}
function loadShell(client: Client, projects: Obj[], threads: Obj[]) {
  client.shell = { ...initialShell(), projects, threads }; client.shellLoaded = true;
}

afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

/** The pane's page config: the session it attaches to. */
const attached = async (client: Client, now: number) => JSON.parse((await view(client, now)).panes[0]!.frame.session);

test('⌘J in a draft opens term-1 in the project root under the draft id, and the draft drawer comes back behind a server thread', async () => {
  const { client, calls } = focusedClient();
  await toggle(client);
  expect(calls).toEqual([{ method: 'terminal.open', payload: { threadId: 'draft-1', terminalId: 'term-1', cwd: '/a', env: { T3CODE_PROJECT_ROOT: '/a' } } }]);
  expect(await view(client)).toMatchObject({ open: true, threadId: 'draft-1', environmentId: 'env-a', cwd: '/a', worktree: '', sessionKey: key('env-a', 'draft-1') });
  expect(await attached(client, 1)).toMatchObject({ mode: 'session', identity: { environmentId: 'env-a', threadId: 'draft-1', terminalId: 'term-1', surface: 'drawer' },
    attach: { threadId: 'draft-1', terminalId: 'term-1', cwd: '/a', worktreePath: null, env: { T3CODE_PROJECT_ROOT: '/a' } } });
  // The user opens a server thread, then the draft again: its drawer and session are the same.
  client.threadId = 'a-1';
  expect(await view(client, 2)).toMatchObject({ open: false, threadId: 'a-1', panes: [] });
  client.threadId = '';
  expect(await view(client, 3)).toMatchObject({ open: true, threadId: 'draft-1', sessionKey: key('env-a', 'draft-1') });
  expect(calls).toHaveLength(1);
});

test('A background environment\'s thread keeps its drawer across a focus change, each page attached to its own environment', async () => {
  const { client, calls } = focusedClient();
  client.threadId = 'a-1';
  await toggle(client);
  expect(calls.at(-1)).toMatchObject({ method: 'terminal.open', payload: { threadId: 'a-1', cwd: '/a' } });
  const first = (await view(client)).panes[0]!.frame.id;
  // The user opens B's thread: the focus moves to B.
  fleet.saved = [{ origin: 'http://env-a', environmentId: 'env-a', enabled: true }];
  focus(client, 'env-b', 'project-b', 'b-1');
  loadShell(client, [{ id: 'project-b', workspaceRoot: '/b' }], [{ id: 'b-1', projectId: 'project-b' }]);
  await toggle(client);
  expect(calls.at(-1)).toEqual({ method: 'terminal.open', payload: { threadId: 'b-1', terminalId: 'term-1', cwd: '/b', env: { T3CODE_PROJECT_ROOT: '/b' } } });
  expect(await view(client, 2)).toMatchObject({ open: true, environmentId: 'env-b', threadId: 'b-1' });
  expect((await attached(client, 3)).identity).toMatchObject({ environmentId: 'env-b', threadId: 'b-1' });
  // The focus goes back to A: A's drawer shows its own session, in a frame of its own.
  focus(client, 'env-a', 'project-a', 'a-1');
  loadShell(client, [{ id: 'project-a', workspaceRoot: '/a' }], [{ id: 'a-1', projectId: 'project-a' }]);
  const back = await view(client, 4);
  expect(back).toMatchObject({ open: true, environmentId: 'env-a', threadId: 'a-1', sessionKey: key('env-a', 'a-1') });
  expect(back.panes[0]!.frame.id).toBe(first);
  expect(calls.filter(call => call.method === 'terminal.open')).toHaveLength(2);
});
