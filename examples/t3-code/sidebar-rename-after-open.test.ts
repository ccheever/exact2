// A double-click on a sidebar row whose thread is not the open one renames it, as Sidebar.tsx's row does (reference
// 1e2ecbd975: onDoubleClick → startThreadRename, whatever row it is; its title input autoFocus + select, :1602-1608):
// the first click opens the thread, and the rename starts once that open has landed and keeps the focus. Opening a
// background environment's row moves the window to that environment, so the row's `fleet:<environment>:<id>` is its
// own id by the time the rename starts. Run with the Contract action semantics of contract-action-fixture.ts.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { initialShell, type Obj } from './domain';
import type { Native } from './protocol';
import { sidebarLocal } from './sidebar-commands';
import { sidebarSession, setRuntimeClock } from './sidebar-state';
import { fleet } from './settings-b-fleet';
import { contractComponent, type Env } from './contract-action-fixture';

beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => { setRuntimeClock(() => Number.NaN); fleet.entries.clear(); });

const row = (id: string): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null, createdAt: '2026-10-16T00:00:00.000Z',
  updatedAt: '2026-10-16T00:00:00.000Z', archivedAt: null, settledOverride: null, settledAt: null, modelSelection: { instanceId: 'codex', model: 'm' },
  lineage: { relationshipToParent: null } });
/** A client the open just moved to env-a (the row was `fleet:env-a:<id>` when it was double-clicked). */
function focusedOn(threads: Obj[]) {
  const client = {
    shell: { ...initialShell(), projects: [{ id: 'p1', title: 'shared-notes', workspaceRoot: '/a' }], threads },
    config: { environment: { capabilities: {} }, providers: [], keybindings: [] },
    environmentId: 'env-a', threadId: 'a', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, presentation: {},
    local: { drafts: {}, selections: {}, snapshotDrafts: {}, snapshotReleases: [], deviceSettings: {}, clientSettings: {} },
  } as unknown as T3Client;
  return { client, native: { available: true, watch() {}, later: async () => ({ ok: true, value: {} }) } as unknown as Native };
}

describe('the rename a double-click starts lands on the row the open left', () => {
  for (const op of ['rename-start', 'legacy-rename-start']) {
    test(`${op}: a fleet id naming the focused environment renames that environment's own row`, async () => {
      const { client, native } = focusedOn([row('a'), row('b')]);
      await sidebarLocal(client, native, op, 'fleet:env-a:a', '');
      expect(sidebarSession(client)).toMatchObject({ renameId: 'a', renameTitle: 'Thread a' });
    });
  }
});

describe('the window holds a double-click\'s rename until its first click\'s open lands', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const recorder = () => {
    const calls: string[] = [];
    const record = (name: string) => (...args: unknown[]) => { calls.push(`${name}:${args.join(',')}`); return name; };
    return { calls, scope: { command: record('command'), focus: record('focus'), openPage: record('openPage'), prSelect: record('prSelect'), relaunch: record('relaunch'),
      openSettings: record('openSettings'), routeKeyboardComplete: () => '', routeKeyboardOwned: false } as Env };
  };

  test('a rename asked while the open runs waits; one asked with nothing running starts at once', () => {
    const { calls, scope } = recorder();
    const state: Env = { renameAfterOp: '', renameAfterId: '' };
    root.call('sidebarLocal', state, { ...scope, commandPending: true }, ['rename-start', 'fleet:env-b:t1', '']);
    expect(state).toEqual({ renameAfterOp: 'rename-start', renameAfterId: 'fleet:env-b:t1' });
    expect(calls).toEqual([]);
    root.call('sidebarLocal', state, { ...scope, commandPending: false }, ['legacy-rename-start', 'fleet:env-b:t2', '']);
    expect(calls).toEqual(['command:sidebarlocal:legacy-rename-start,fleet:env-b:t2,,0']);
    // Other sidebar ops never wait.
    root.call('sidebarLocal', state, { ...scope, commandPending: true }, ['shelf', 'settled', '']);
    expect(calls.at(-1)).toBe('command:sidebarlocal:shelf,settled,,0');
  });

  test('the open\'s result starts the held rename and leaves the composer unfocused', () => {
    const { calls, scope } = recorder();
    const state: Env = { renameAfterOp: 'rename-start', renameAfterId: 'fleet:env-b:t1', noticeExit: 'x', routeKeyboard: '', modalError: '', pendingModal: '', commandPending: true,
      draftOwner: '', jumpRequest: 0, utilityPage: '' };
    const result = { message: 'focus:composer', relaunch: false, routeReorder: 0 };
    root.call('commandCompleted', state, { ...scope, changed: result, data: { sidebar: { renameId: '' } } }, []);
    expect(calls).toEqual(['command:sidebarlocal:rename-start,fleet:env-b:t1,,0']);
    expect(state).toMatchObject({ renameAfterOp: '', commandPending: false });
    // With no rename held, the open focuses the composer as ChatView does.
    const plain = recorder();
    root.call('commandCompleted', { ...state, renameAfterOp: '', renameAfterId: '' }, { ...plain.scope, changed: result, data: { sidebar: { renameId: '' } } }, []);
    expect(plain.calls).toEqual(['focus:composer']);
  });
});
