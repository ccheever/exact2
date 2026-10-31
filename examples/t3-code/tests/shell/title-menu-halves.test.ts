// The chat header's thread menu as useThreadActionMenu builds it at T3 Code 6d450a4284 (MIT, see LICENSE-T3): the
// environment's snooze options (Sidebar.snooze.ts resolveSnoozePresets, picked by pickSnoozePreset), Turn into project
// and Copy › Thread, which run the sidebar's own actions; and Custom snooze's "Edit snooze options", which lands on
// Settings › Threads › Snooze options (/settings/threads#snooze-options).
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { titleMenu } from '../../src/shell/shell';
import { chatCommand } from '../../src/sidebar/chat-commands';
import { sidebarCommand } from '../../src/sidebar/sidebar-commands';
import { setRuntimeClock } from '../../src/sidebar/sidebar-state';
import { toasts } from '../../src/shell/toast';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const NOW = new Date(2026, 9, 7, 17, 30).getTime(); // a Wednesday evening
const CAPS = { threadSnooze: true, threadProjectFromScratch: true };
function fake(threads: Obj[], settings: Obj = {}) {
  const dispatched: Obj[] = [], copied: string[] = [], created: string[] = [];
  const client = {
    shell: { projects: [{ id: 'p1', title: 'P', workspaceRoot: '/p' }, { id: 'scratch', title: 'Scratch', workspaceRoot: '/scratch' }], threads, sequence: 1 },
    config: { scratchWorkspaceRoot: '/scratch', environment: { capabilities: CAPS }, settings, providers: [], keybindings: [] },
    environmentId: 'env', threadId: 't1', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, presentation: {},
    local: { drafts: {}, snapshotDrafts: {}, deviceSettings: { timestampFormat: '24-hour' }, clientSettings: {} },
    projectGroups: () => [],
    restAccess: () => ({ ids: async () => ['c1'], dispatch: async (_s: Files, payload: Obj) => { dispatched.push(payload); return {}; },
      call: async (request: Obj) => { if (request.op === 'copyText') copied.push(String(request.text)); return {}; } }),
    createProjectFromThread: async (_native: Native, threadId: string) => { created.push(threadId); return 'p9'; },
  } as unknown as T3Client;
  return { client, dispatched, copied, created };
}
const thread = (extra: Obj = {}): Obj => ({ id: 't1', projectId: 'p1', title: 'Fixture', status: 'idle', snoozedUntil: null, ...extra });
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
afterEach(() => setRuntimeClock(() => Number.NaN));

describe('the title menu\'s snooze options are its environment\'s', () => {
  test('soonest first, from `snoozePresets`; with none, Custom… alone and not set apart', () => {
    const { client } = fake([thread()], { snoozePresets: [{ kind: 'at', day: 'tomorrow', time: '08:00' }, { kind: 'after', minutes: 90 }] });
    const rows = titleMenu(client, NOW).filter(item => item.submenu === 'snooze');
    expect(rows.map(item => item.label)).toEqual(['In 90 minutes (19:00)', 'Tomorrow (08:00)', 'Custom…']);
    expect(rows[0]).toMatchObject({ op: 'chat:snooze', target: 't1', value: `after:90|${new Date(NOW + 90 * 60_000).toISOString()}` });
    expect(rows.at(-1)).toMatchObject({ separated: true });
    const none = fake([thread()], { snoozePresets: [] }).client;
    expect(titleMenu(none, NOW).filter(item => item.submenu === 'snooze').map(item => [item.label, item.separated])).toEqual([['Custom…', false]]);
  });

  test('a choice is picked as the sidebar\'s: a duration from the pick, a set time as shown, a passed one warns', async () => {
    const { client, dispatched } = fake([thread()], { snoozePresets: [{ kind: 'after', minutes: 90 }, { kind: 'at', day: 'today', time: '18:00' }] });
    const value = (id: string) => titleMenu(client, NOW).find(item => item.id === `snooze:${id}`)!.value;
    const duration = value('after:90'), evening = value('at:today:18:00');
    setRuntimeClock(() => NOW + 60_000); // picked a minute after the menu was drawn
    await chatCommand(client, native, {} as Files, 'snooze', 't1', duration);
    await chatCommand(client, native, {} as Files, 'snooze', 't1', evening);
    expect(dispatched.map(payload => payload.snoozedUntil)).toEqual([new Date(NOW + 91 * 60_000).toISOString(), new Date(2026, 9, 7, 18, 0).toISOString()]);
    setRuntimeClock(() => new Date(2026, 9, 7, 18, 1).getTime());
    await chatCommand(client, native, {} as Files, 'snooze', 't1', evening);
    expect(dispatched).toHaveLength(2);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'warning', title: 'Could not snooze thread' });
  });
});

describe('Turn into project and Copy › Thread', () => {
  test('Turn into project shows for a No project thread with its own folder, disabled while it runs, and runs the sidebar\'s', async () => {
    const { client, created } = fake([thread({ projectId: 'scratch', worktreePath: '/scratch/t1' })]);
    const row = titleMenu(client, NOW).find(item => item.id === 'turn-into-project')!;
    expect(row).toMatchObject({ label: 'Turn into project', icon: 'folder-plus', op: 'sidebar:turn-into-project', target: 't1', disabled: false });
    const top = titleMenu(client, NOW).filter(item => item.submenu === '').map(item => item.id);
    expect(top.slice(top.indexOf('mark-unread'), top.indexOf('mark-unread') + 2)).toEqual(['mark-unread', 'turn-into-project']);
    await sidebarCommand(client, native, {} as Files, 'turn-into-project', 't1', '');
    expect(created).toEqual(['t1']);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Project created', description: 'Fixture' });
    const running = fake([thread({ projectId: 'scratch', worktreePath: '/scratch/t1', status: 'running' })]).client;
    expect(titleMenu(running, NOW).find(item => item.id === 'turn-into-project')!.disabled).toBe(true);
    expect(titleMenu(fake([thread({ worktreePath: '/p/t1' })]).client, NOW).some(item => item.id === 'turn-into-project')).toBe(false);
  });

  test('Copy › Thread copies the thread\'s chip, between Path and Thread ID', async () => {
    const { client, copied } = fake([thread()]);
    const copy = titleMenu(client, NOW).filter(item => item.submenu === 'copy');
    expect(copy.map(item => [item.id, item.label, item.icon])).toEqual([['copy-path', 'Path', 'folder'], ['copy-thread', 'Thread', 'message-square'], ['copy-thread-id', 'Thread ID', 'hash']]);
    expect(copy[1]).toMatchObject({ op: 'sidebar:copy-thread', target: 't1' });
    await sidebarCommand(client, native, {} as Files, 'copy-thread', 't1', '');
    expect(copied).toEqual(['[Fixture](t3-context://v1/thread/thread_t1)']);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Thread copied', description: 'Fixture' });
  });
});

test('Edit snooze options opens Settings › Threads and lands on Snooze options', () => {
  const root = contractComponent('app.contract', 'T3Code'), opened: string[] = [];
  const fresh = (): Env => ({ settingsRoute: '', settingsFocusTarget: '', settingsJumpOpened: 'x', renameAfterOp: '', renameAfterId: '', noticeExit: '', routeKeyboard: '',
    modalError: '', pendingModal: '', commandPending: true, draftOwner: '', jumpRequest: 0, utilityPage: '', lineageStopping: '' });
  const state = fresh();
  const scope = { openSettings: () => opened.push('settings'), focus: () => '', command: () => '', routeKeyboardComplete: () => '', routeKeyboardOwned: false,
    changed: { message: 'sidebar:settings:threads:snooze-options', relaunch: false, routeReorder: 0 }, data: { sidebar: { renameId: '' } } } as Env;
  root.call('commandCompleted', state, scope, []);
  expect(opened).toEqual(['settings']);
  expect(state).toMatchObject({ settingsRoute: 'threads', settingsFocusTarget: 'snooze-options', settingsJumpOpened: '' });
  // A route alone lands on no row.
  const plain = fresh();
  root.call('commandCompleted', plain, { ...scope, changed: { ...scope.changed as Obj, message: 'sidebar:settings:about' } }, []);
  expect(plain).toMatchObject({ settingsRoute: 'about', settingsFocusTarget: '' });
});
