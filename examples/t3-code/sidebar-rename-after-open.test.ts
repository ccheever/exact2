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
import { dropHeldRename, renameHeld, startHeldRename, startRename } from './sidebar-rename-wait';
import { scopedRowKey } from './sidebar-view';

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

// The open's slow order: the window's status names the row's environment before that environment's shell is in (the
// first refresh after the move resets the shell; synchronize loads it on a later one), and a draft to restore makes
// that later. A rename asked in between waits for the row, starts on the refresh that brings it, and until then the
// open's composer focus stays off the prompt.
describe('a double-click\'s rename waits for its row\'s environment to come in', () => {
  /** The window just moved to env-b: its status names env-b, its shell is not in yet. */
  function moving(environmentId = 'env-b') {
    const made = focusedOn([]);
    Object.assign(made.client, { environmentId, threadId: 't1', shellLoaded: false });
    return made;
  }
  const arrive = (client: T3Client, threads: Obj[]) => Object.assign(client, { shell: { ...client.shell, threads }, shellLoaded: true });

  for (const op of ['rename-start', 'legacy-rename-start']) {
    test(`${op}: held while the shell is out, started by the refresh that brings the row`, async () => {
      const { client, native } = moving();
      await sidebarLocal(client, native, op, 'fleet:env-b:t1', '');
      expect(sidebarSession(client).renameId).toBe('');
      expect(renameHeld(client)).toBe(true);
      startHeldRename(client); // a refresh still without the shell (the draft restore not done): it keeps waiting
      expect(renameHeld(client)).toBe(true);
      arrive(client, [row('t0'), row('t1')]);
      startHeldRename(client);
      expect(sidebarSession(client)).toMatchObject({ renameId: 't1', renameTitle: 'Thread t1' });
      expect(renameHeld(client)).toBe(false);
    });
  }

  test('the status has not named the new environment yet: the fleet has let go of it, so the rename waits for it', async () => {
    const { client, native } = moving('env-a');
    Object.assign(client, { shellLoaded: true });
    await sidebarLocal(client, native, 'rename-start', 'fleet:env-b:t1', '');
    expect(renameHeld(client)).toBe(true);
    Object.assign(client, { environmentId: 'env-b', shellLoaded: false, shell: initialShell() });
    startHeldRename(client);
    expect(renameHeld(client)).toBe(true);
    arrive(client, [row('t1')]);
    startHeldRename(client);
    expect(sidebarSession(client).renameId).toBe('t1');
  });

  test('a row the environment does not hold, or a background environment the window stays off, holds nothing', async () => {
    const { client, native } = moving();
    await sidebarLocal(client, native, 'rename-start', 'fleet:env-b:gone', '');
    arrive(client, [row('t1')]);
    startHeldRename(client);
    expect(sidebarSession(client).renameId).toBe('');
    expect(renameHeld(client)).toBe(false);
    // env-c is connected in the background and its shell lacks the row: nothing to wait for.
    fleet.entries.set('c', { environmentId: 'env-c', phase: 'connected', shell: { projects: [], threads: [] }, config: {} } as never);
    await sidebarLocal(client, native, 'rename-start', 'fleet:env-c:t9', '');
    expect(renameHeld(client)).toBe(false);
  });

  test('the window going back to the row\'s environment as a background one drops the wait', async () => {
    const { client, native } = moving('env-a');
    await sidebarLocal(client, native, 'rename-start', 'fleet:env-b:t1', '');
    expect(renameHeld(client)).toBe(true);
    fleet.entries.set('b', { environmentId: 'env-b', phase: 'connected', shell: { projects: [], threads: [row('t1')] }, config: {} } as never);
    startHeldRename(client);
    expect(renameHeld(client)).toBe(false);
    expect(sidebarSession(client).renameId).toBe('');
  });

  test('a press on another row drops the wait; the double-click\'s own press keeps it', () => {
    const { client } = moving();
    startRename(client, 'fleet:env-b:t1');
    dropHeldRename(client, 'fleet:env-b:t1');
    dropHeldRename(client, 't1');
    expect(renameHeld(client)).toBe(true);
    dropHeldRename(client, 't2');
    expect(renameHeld(client)).toBe(false);
  });
});

describe('no composer focus the open causes lands while its double-click\'s rename is coming', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const scopeOf = (calls: string[]) => {
    const record = (name: string) => (...args: unknown[]) => { calls.push(`${name}:${args.join(',')}`); return name; };
    return { command: record('command'), focus: record('focus'), openPage: record('openPage'), prSelect: record('prSelect'), relaunch: record('relaunch'),
      openSettings: record('openSettings'), routeKeyboardComplete: () => '', routeKeyboardOwned: false } as Env;
  };
  const fresh = (): Env => ({ renameAfterOp: '', renameAfterId: '', noticeExit: '', routeKeyboard: '', modalError: '', pendingModal: '', commandPending: true,
    draftOwner: '', jumpRequest: 0, utilityPage: '' });
  const focusResult = { message: 'focus:composer', relaunch: false, routeReorder: 0 };

  test('the slow order: the open lands, the rename waits for the shell, a later open result asks for the composer', () => {
    const calls: string[] = [], scope = scopeOf(calls), state = fresh();
    // The double-click's rename comes while the first click's open runs, and is held.
    root.call('sidebarLocal', state, { ...scope, commandPending: true }, ['legacy-rename-start', 'fleet:env-b:t1', '']);
    // The open lands: its result asks for the composer; the held rename goes out instead.
    root.call('commandCompleted', state, { ...scope, changed: focusResult, data: { sidebar: { renameId: '', renameHeld: false } } }, []);
    expect(calls).toEqual(['command:sidebarlocal:legacy-rename-start,fleet:env-b:t1,,0']);
    // The environment's shell is not in yet (sidebar-rename-wait.ts holds the rename); another open result lands.
    root.call('commandCompleted', { ...state, commandPending: true }, { ...scope, changed: focusResult, data: { sidebar: { renameId: '', renameHeld: true } } }, []);
    // The rename has started: its field holds the focus.
    root.call('commandCompleted', { ...state, commandPending: true }, { ...scope, changed: focusResult, data: { sidebar: { renameId: 't1', renameHeld: false } } }, []);
    expect(calls.filter(call => call.startsWith('focus:'))).toEqual([]);
  });

  test('with no rename coming, the open focuses the composer', () => {
    const calls: string[] = [];
    root.call('commandCompleted', fresh(), { ...scopeOf(calls), changed: focusResult, data: { sidebar: { renameId: '', renameHeld: false } } }, []);
    expect(calls).toEqual(['focus:composer']);
  });
});

// The second click of a double-click on a background row comes after the first click's open has moved the window to
// that row's environment, where the row's id loses its `fleet:` prefix. Sidebar.tsx keys its rows by the scoped thread
// key, so the element under the pointer stays; a list keyed by the bare id replaced it between the clicks, and the second
// click's press and double-click went nowhere.
describe('a row keeps its element while an open moves the window to its environment', () => {
  test('the scoped key of a background row is the key the same row has once its environment is focused', () => {
    const before = focusedOn([]).client, after = focusedOn([row('t1')]).client;
    Object.assign(before, { environmentId: 'env-z' });
    expect(scopedRowKey(before, 'fleet:env-a:t1')).toBe(scopedRowKey(after, 't1'));
    expect(scopedRowKey(after, 't1')).not.toBe(scopedRowKey(after, 't2'));
  });
});
