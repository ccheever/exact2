import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// The drawer of a draft and of a background environment's thread.
// T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3) ChatView.tsx: a draft is the active thread under its own
// `draftThread.threadId`, `toggleTerminalVisibility` opens its `term-1` in the project root, and the mounted drawers
// are the open ones among `existingThreadKeys` = every environment's `serverThreadKeys` plus `draftThreadKeys`.
// The clone opens a background environment's thread by moving the focus there (settings-b-fleet.ts), so the drawers
// the focus leaves behind must stay mounted: their sessions stay retained natively (`terminalRetain`) and ride on
// their own environment's link (T3TerminalSessions.swift over T3Link.swift; the AppKit test in macos/tests/terminal). These tests
// use only the drawer's resource and ops, so they run against the base as well (the before record).
import { afterEach, expect, test } from 'bun:test';

import { T3Client as Client } from '../../src/client/client';
import { initialShell, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { terminalDrawerView, terminalOps } from '../../src/terminal/terminal-drawer-view';

type Call = { method: string; payload: Obj };
const native = { available: true } as Native;

function focusedClient() {
  const client = new Client();
  Object.assign(client, { scopes: [...AUTH_STANDARD_CLIENT_SCOPES], connection: 'connected', configLive: true, shellLive: true, threadLive: true });
  const calls: Call[] = [], retains: string[][] = [];
  client.environmentId = 'env-a'; client.projectId = 'project-a'; client.threadId = '';
  client.shell.projects = [{ id: 'project-a', workspaceRoot: '/a' }];
  client.shell.threads = [{ id: 'a-1', projectId: 'project-a' }];
  client.shellLoaded = true;
  let ids = 0;
  client.restAccess = () => ({ ids: async () => [`draft-${++ids}`] }) as ReturnType<Client['restAccess']>;
  client.request = async (_native, method, payload) => { calls.push({ method, payload }); return {}; };
  client.call = async (_native, request) => {
    if (request.op === 'terminalRetain') retains.push([...(request.sessions as string[])].sort());
    return {};
  };
  return { client, calls, retains };
}
const toggle = (client: Client) => terminalOps.call(client, 'terminallocal:toggle', '', '', 0, native, {} as Files, { message: '', id: '', value: '' });
const view = (client: Client, now = 0) => terminalDrawerView(client, native, 840, now);
const key = (environment: string, thread: string) => JSON.stringify([environment, thread, 'term-1']);

function entry(environmentId: string, threads: Obj[], synchronized = true): FleetEntry {
  return { key: `http://${environmentId}\n${environmentId}`, origin: `http://${environmentId}`, environmentId, phase: synchronized ? 'connected' : 'connecting',
    message: '', traceId: '', generation: 1, synchronized: synchronized ? 1 : -1, lastEvent: 0, subscriptions: {}, config: {},
    shell: { ...initialShell(), projects: [{ id: `project-${environmentId}`, workspaceRoot: `/${environmentId}` }], threads }, scopes: ['terminal:operate'], error: '', requested: true };
}
/** adoptStatus moving the focus (client.ts): the new environment's selection, its shell not loaded yet. */
function focus(client: Client, environmentId: string, projectId: string, threadId: string) {
  client.environmentId = environmentId; client.projectId = projectId; client.threadId = threadId;
  client.shell = initialShell(); client.shellLoaded = false;
}
function loadShell(client: Client, projects: Obj[], threads: Obj[]) {
  client.shell = { ...initialShell(), projects, threads }; client.shellLoaded = true;
}

afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

test('⌘J in a draft opens term-1 in the project root under the draft id, and the draft drawer stays mounted behind a server thread', async () => {
  const { client, calls, retains } = focusedClient();
  await toggle(client);
  expect(calls).toEqual([{ method: 'terminal.open', payload: { threadId: 'draft-1', terminalId: 'term-1', cwd: '/a', env: { T3CODE_PROJECT_ROOT: '/a' } } }]);
  expect(await view(client)).toMatchObject({ open: true, threadId: 'draft-1', environmentId: 'env-a', cwd: '/a', worktree: '', sessionKey: key('env-a', 'draft-1'), mounted: 1 });
  expect(retains.at(-1)).toEqual([key('env-a', 'draft-1')]);
  // The user opens a server thread: the reference keeps the draft's open drawer mounted (draftThreadKeys).
  client.threadId = 'a-1';
  expect(await view(client, 1)).toMatchObject({ open: false, threadId: 'a-1', mounted: 1 });
  expect(retains.at(-1)).toEqual([key('env-a', 'draft-1')]);
  // Back in the draft, its drawer and session are the same.
  client.threadId = '';
  expect(await view(client, 2)).toMatchObject({ open: true, threadId: 'draft-1', sessionKey: key('env-a', 'draft-1'), mounted: 1 });
  expect(calls).toHaveLength(1);
});

test('A background environment\'s thread keeps its drawer mounted across a focus change, each session under its own environment', async () => {
  const { client, calls, retains } = focusedClient();
  client.threadId = 'a-1';
  await toggle(client);
  expect(calls.at(-1)).toMatchObject({ method: 'terminal.open', payload: { threadId: 'a-1', cwd: '/a' } });
  await view(client);
  expect(retains.at(-1)).toEqual([key('env-a', 'a-1')]);
  // The user opens B's thread: the focus moves to B; A is switched on (saved), not yet back as a background environment.
  fleet.saved = [{ origin: 'http://env-a', environmentId: 'env-a', enabled: true }];
  focus(client, 'env-b', 'project-b', 'b-1');
  await view(client, 1);
  expect(retains.at(-1)).toEqual([key('env-a', 'a-1')]);
  loadShell(client, [{ id: 'project-b', workspaceRoot: '/b' }], [{ id: 'b-1', projectId: 'project-b' }]);
  await toggle(client);
  expect(calls.at(-1)).toEqual({ method: 'terminal.open', payload: { threadId: 'b-1', terminalId: 'term-1', cwd: '/b', env: { T3CODE_PROJECT_ROOT: '/b' } } });
  expect(await view(client, 2)).toMatchObject({ open: true, environmentId: 'env-b', threadId: 'b-1', mounted: 2 });
  expect(retains.at(-1)).toEqual([key('env-a', 'a-1'), key('env-b', 'b-1')]);
  // A connects again as a background environment and lists the thread: still mounted.
  fleet.entries.set('a', entry('env-a', [{ id: 'a-1', projectId: 'project-env-a' }]));
  expect((await view(client, 3)).mounted).toBe(2);
  // The focus goes back to A (B now a switched-on background environment still connecting): both stay, A's drawer shows.
  fleet.entries.clear();
  fleet.saved = [{ origin: 'http://env-b', environmentId: 'env-b', enabled: true }];
  fleet.entries.set('b', entry('env-b', [], false));
  focus(client, 'env-a', 'project-a', 'a-1');
  loadShell(client, [{ id: 'project-a', workspaceRoot: '/a' }], [{ id: 'a-1', projectId: 'project-a' }]);
  expect(await view(client, 4)).toMatchObject({ open: true, environmentId: 'env-a', threadId: 'a-1', sessionKey: key('env-a', 'a-1'), mounted: 2 });
  expect(retains.at(-1)).toEqual([key('env-a', 'a-1'), key('env-b', 'b-1')]);
  expect(calls.filter(call => call.method === 'terminal.open')).toHaveLength(2);
});

test('A background environment\'s drawer goes when its thread is gone or the environment is switched off', async () => {
  const { client, retains } = focusedClient();
  client.threadId = 'a-1';
  await toggle(client);
  await view(client);
  fleet.saved = [{ origin: 'http://env-a', environmentId: 'env-a', enabled: true }];
  focus(client, 'env-b', 'project-b', 'b-1');
  loadShell(client, [{ id: 'project-b', workspaceRoot: '/b' }], [{ id: 'b-1', projectId: 'project-b' }]);
  fleet.entries.set('a', entry('env-a', [{ id: 'a-1', projectId: 'project-env-a' }, { id: 'a-2', projectId: 'project-env-a' }]));
  expect((await view(client, 1)).mounted).toBe(1);
  // A's server deleted the thread (its synchronized shell no longer lists it).
  fleet.entries.set('a', entry('env-a', [{ id: 'a-2', projectId: 'project-env-a' }]));
  expect((await view(client, 2)).mounted).toBe(0);
  expect(retains.at(-1)).toEqual([]);

  const second = focusedClient();
  second.client.threadId = 'a-1';
  await toggle(second.client);
  await view(second.client);
  fleet.entries.clear();
  fleet.saved = [{ origin: 'http://env-a', environmentId: 'env-a', enabled: false }];
  focus(second.client, 'env-b', 'project-b', 'b-1');
  expect((await view(second.client, 1)).mounted).toBe(0);
  expect(second.retains.at(-1)).toEqual([]);
});
