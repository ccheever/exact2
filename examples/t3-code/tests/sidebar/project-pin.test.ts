// Pin favorite projects from the sidebar's project filter (T3 Code 2359643cff, MIT, see LICENSE-T3:
// hooks/useProjectGroupUpdate.ts, hooks/useOrderWhileOpen.ts and its test, ProjectPinButton.tsx, Sidebar.tsx,
// Sidebar.logic.ts shouldCreateNewThreadInCurrentProject).
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { canPinProjectGroup, keepOrder, orderWhileOpen, setProjectGroupPinned, withArrivedKeys } from '../../src/sidebar/project-pin';
import { scopeHighlight, shouldCreateNewThreadInCurrentProject, sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import { sidebarCommand } from '../../src/sidebar/sidebar-commands';
import { sidebarSession } from '../../src/sidebar/sidebar-state';
import { toasts } from '../../src/shell/toast';
import { contractComponent } from '../support/contract-action-fixture';
import { keyboardDispatch } from '../../src/shell/keyboard-dispatch';

afterEach(() => fleet.entries.clear());
const at = '2026-11-01T00:00:00.000Z';
const ORGANIZE = { projectOrganization: true };
const KEY = 'https://second.example\nenv-b';

function rig(caps: Obj = ORGANIZE, scopes = ['orchestration:read', 'orchestration:operate']) {
  const writes: Obj[] = [], there: Obj[] = [];
  const client = new T3Client();
  Object.assign(client, { connection: 'connected', environmentId: 'env-a', loaded: true, configLive: true, shellLive: true, presentation: {} });
  client.config = { environment: { label: 'This Mac', capabilities: caps }, settings: {}, providers: [], keybindings: [{ command: 'thread.pin', shortcut: { key: 'p', modKey: true, shiftKey: true } }] };
  client.scopes = scopes;
  client.shell = { projects: [{ id: 'alpha', title: 'alpha', workspaceRoot: '/alpha', createdAt: at, updatedAt: at }, { id: 'beta', title: 'beta', workspaceRoot: '/beta', createdAt: at, updatedAt: at, pinnedAt: at }], threads: [], sequence: 1 };
  Object.assign(client, { restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
    request: async (method: string, payload: Obj) => { writes.push({ method, ...payload }); return {}; } }) });
  const native = { available: true, watch() {}, later: async (request: unknown) => {
    const sent = obj(request), payload = obj(sent.payload);
    if (sent.fleet === KEY && sent.method === 'projects.mutate') there.push(payload);
    return { ok: true, generation: Number(sent.generation) || 2, value: {} };
  } } as unknown as Native;
  return { client, native, writes, there };
}
const background = (caps: Obj = ORGANIZE, phase = 'connected'): FleetEntry => ({ key: KEY, origin: 'https://second.example', environmentId: 'env-b', phase, message: '', traceId: '',
  generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {}, config: { environment: { label: 'Lane Second', capabilities: caps } },
  shell: { projects: [{ id: 'gamma', title: 'gamma', workspaceRoot: '/gamma', createdAt: at, updatedAt: at }], threads: [] }, scopes: ['orchestration:operate'], error: '', requested: true } as unknown as FleetEntry);
const identity = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2), projectInk: 'ink', projectSurface: 'surface' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
const scopes = (client: T3Client) => sidebarSnapshot(client, Date.parse(at), identity).sidebar.scopes;

describe('the project filter offers a pin', () => {
  test('pinned projects come first and show their pin; the title names the shortcut', () => {
    const { client } = rig();
    expect(scopes(client).map(scope => [scope.label, scope.canPin, scope.pinned, scope.pinTitle])).toEqual([
      ['All projects', false, false, ''], ['beta', true, true, 'Unpin beta (⇧⌘P)'], ['alpha', true, false, 'Pin alpha (⇧⌘P)']]);
    expect(sidebarSnapshot(client, Date.parse(at), identity).sidebar.scopePinChord).toBe('Meta+Shift+p');
  });
  test('not where a server drops pins or this session may not operate', () => {
    expect(scopes(rig({}).client).some(scope => scope.canPin)).toBe(false);
    expect(scopes(rig(ORGANIZE, ['orchestration:read']).client).some(scope => scope.canPin)).toBe(false);
    const { client } = rig();
    fleet.entries.set(KEY, background({}));
    expect(canPinProjectGroup(client, [{ id: 'fleet:env-b:gamma', environmentId: 'env-b' }])).toBe(false);
    fleet.entries.set(KEY, background());
    expect(canPinProjectGroup(client, [{ id: 'fleet:env-b:gamma', environmentId: 'env-b' }])).toBe(true);
  });
});

describe('pinning writes project.update on each member\'s environment', () => {
  test('a focused project, from the filter\'s row', async () => {
    const { client, native, writes } = rig();
    sidebarSession(client).scopeOpen = true;
    await sidebarCommand(client, native, {} as Files, 'project-pin', '', scopes(client).find(scope => scope.label === 'alpha')!.key);
    expect(writes).toEqual([{ method: 'projects.mutate', type: 'project.update', commandId: 'c0', projectId: 'alpha', pinned: true }]);
    expect(sidebarSession(client).scopeOpen).toBe(true);
  });
  test('a background member through its own environment; a disconnected one is refused with the reference\'s sentence', async () => {
    const { client, native, writes, there } = rig();
    fleet.entries.set(KEY, background());
    const members = [{ id: 'alpha', environmentId: 'env-a' }, { id: 'fleet:env-b:gamma', environmentId: 'env-b' }];
    expect(await setProjectGroupPinned(client, native, members, false)).toBe(true);
    expect(writes.map(write => [write.projectId, write.pinned])).toEqual([['alpha', false]]);
    expect(there.map(payload => [payload.type, payload.projectId, payload.pinned])).toEqual([['project.update', 'gamma', false]]);
    fleet.entries.set(KEY, background(ORGANIZE, 'reconnecting'));
    expect(await setProjectGroupPinned(client, native, members, true)).toBe(false);
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Failed to pin project', description: 'Connect Lane Second and try again.' });
  });
  test('a session that cannot operate is refused before any write', async () => {
    const { client, native, writes } = rig(ORGANIZE, ['orchestration:read']);
    expect(await setProjectGroupPinned(client, native, [{ id: 'alpha', environmentId: 'env-a' }], true)).toBe(false);
    expect(writes).toEqual([]);
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Failed to pin project', description: 'This connection cannot change projects in This Mac.' });
  });
});

describe('useOrderWhileOpen', () => {
  // useOrderWhileOpen.test.ts
  test('keepOrder sorts into the order and keeps the rest after, in their own order', () => {
    expect(keepOrder(['c', 'a', 'b', 'd'], ['a', 'b', 'c'], key => key)).toEqual(['a', 'b', 'c', 'd']);
    expect(keepOrder(['c', 'a'], null, key => key)).toEqual(['c', 'a']);
  });
  test('withArrivedKeys adds unseen keys and returns the same order when nothing is new', () => {
    const order = ['a', 'b'];
    expect(withArrivedKeys(order, ['b', 'a'])).toBe(order);
    expect(withArrivedKeys(order, ['a', 'c'])).toEqual(['a', 'b', 'c']);
    expect(withArrivedKeys(null, ['a'])).toEqual(['a']);
  });
  test('a pin keeps the filter\'s rows in place until it closes', () => {
    const { client } = rig();
    sidebarSession(client).scopeOpen = true;
    expect(scopes(client).map(scope => scope.label)).toEqual(['All projects', 'beta', 'alpha']);
    client.shell.projects[0]!.pinnedAt = at; client.shell.projects[1]!.pinnedAt = null;
    expect(scopes(client).map(scope => [scope.label, scope.pinned])).toEqual([['All projects', false], ['beta', false], ['alpha', true]]);
    sidebarSession(client).scopeOpen = false; scopes(client);
    sidebarSession(client).scopeOpen = true;
    expect(scopes(client).map(scope => scope.label)).toEqual(['All projects', 'alpha', 'beta']);
    const holder = { order: null as readonly string[] | null };
    expect(orderWhileOpen(holder, ['x'], key => key, false)).toEqual(['x']);
    expect(holder.order).toBeNull();
  });
});

describe('the filter\'s keys', () => {
  const menu = contractComponent('sidebar-overlays.contract', 'SidebarScopeMenu');
  const press = (highlighted: string, key: string, e: Obj) => {
    const calls: unknown[][] = [];
    const data = { sidebar: { scopePinChord: 'Meta+Shift+p', scopes: [{ key: '', canPin: false }, { key: 'g1', canPin: true }, { key: 'g2', canPin: false }], scopeHighlight: '' } };
    menu.call('pinKey', { hovered: highlighted, pinOver: '' }, { data, run: (...args: unknown[]) => calls.push(['run', ...args]), preventDefault: () => calls.push(['preventDefault']), stopPropagation: () => calls.push(['stopPropagation']) },
      [key, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...e }]);
    return calls;
  };
  test('the thread pin shortcut pins the highlighted project', () => {
    expect(press('g1', 'P', { metaKey: true, shiftKey: true })).toEqual([['preventDefault'], ['stopPropagation'], ['run', 'project-pin', '', 'g1']]);
  });
  test('and nothing else: another chord, the first row (All projects) highlighted, or one that cannot be pinned', () => {
    expect(press('g1', 'p', { metaKey: true })).toEqual([]);
    expect(press('', 'P', { metaKey: true, shiftKey: true })).toEqual([]);
    expect(press('g2', 'P', { metaKey: true, shiftKey: true })).toEqual([]);
  });
  // Base UI Combobox's list navigation (loopFocus): the arrows move the highlight from the row Base UI highlighted or the
  // pointer rests on, and wrap at the ends; the shortcut then pins the row they reached.
  test('the arrows move the highlight, from no row to the first or the last, and wrap', () => {
    const data = { sidebar: { scopePinChord: 'Meta+Shift+p', scopes: [{ key: '', canPin: false }, { key: 'g1', canPin: true }, { key: 'g2', canPin: true }], scopeHighlight: '' } };
    const step = (state: Obj, key: string) => {
      menu.call('searchKey', state, { data, local() {}, preventDefault() {} }, [key, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false }]);
      return menu.derives(state, { data }).highlighted;
    };
    const state: Obj = { hovered: '\n', keyed: '\n', pinOver: '' };
    expect([step(state, 'ArrowDown'), step(state, 'ArrowDown'), step(state, 'ArrowDown'), step(state, 'ArrowUp')]).toEqual(['g1', 'g2', '', 'g2']);
    expect(step({ hovered: '\n', keyed: '\n', pinOver: '' }, 'ArrowUp')).toBe('g2');
    expect(step({ hovered: 'g1', keyed: 'g1', pinOver: '' }, 'ArrowDown')).toBe('g2');
    const none = { sidebar: { ...data.sidebar, scopeHighlight: '\n' } }, fresh: Obj = { hovered: '\n', keyed: '\n', pinOver: '' };
    menu.call('searchKey', fresh, { data: none, local() {}, preventDefault() {} }, ['ArrowDown', { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false }]);
    expect(menu.derives(fresh, { data: none }).highlighted).toBe('');
    const calls: unknown[][] = [];
    menu.call('pinKey', state, { data, run: (...args: unknown[]) => calls.push(args), preventDefault() {}, stopPropagation() {} }, ['P', { metaKey: true, ctrlKey: false, altKey: false, shiftKey: true }]);
    expect(calls).toEqual([['project-pin', '', 'g2']]);
  });
  // Base UI Combobox: before the pointer moves, the selected row is highlighted (focusItemOnOpen over selectedIndex).
  test('before the pointer moves, the shortcut pins the chosen project, which Base UI highlights on open', () => {
    const calls: unknown[][] = [];
    const data = { sidebar: { scopePinChord: 'Meta+Shift+p', scopes: [{ key: '', canPin: false }, { key: 'g1', canPin: true }], scopeHighlight: 'g1' } };
    menu.call('pinKey', { hovered: '\n', pinOver: '' }, { data, run: (...args: unknown[]) => calls.push(['run', ...args]), preventDefault() {}, stopPropagation() {} },
      ['P', { metaKey: true, ctrlKey: false, altKey: false, shiftKey: true }]);
    expect(calls).toEqual([['run', 'project-pin', '', 'g1']]);
  });
  test('a new query drops the row the pointer rested on, so Return takes the first match', () => {
    const state = { hovered: 'beta', pinOver: '' }, calls: unknown[][] = [];
    menu.call('queryInput', state, { data: { sidebar: { scopes: [], scopeHighlight: '' } }, local: (...args: unknown[]) => calls.push(args) }, ['al']);
    expect([state.hovered, calls]).toEqual(['\n', [['scope-query', '', 'al']]]);
  });
  test('Return picks the highlighted row, and nothing when no row is highlighted', () => {
    const picks = (hovered: string, scopeHighlight: string) => {
      const calls: unknown[][] = [];
      menu.call('submitScope', { hovered, pinOver: '' }, { data: { sidebar: { scopes: [], scopeHighlight } }, local: (...args: unknown[]) => calls.push(args) }, []);
      return calls;
    };
    expect(picks('\n', 'g1')).toEqual([['scope', '', 'g1']]);
    expect(picks('\n', '')).toEqual([['scope', '', '']]);
    expect(picks('\n', '\n')).toEqual([]);
    expect(picks('g2', '\n')).toEqual([['scope', '', 'g2']]);
  });
});

test('scopeHighlight: the first match with a query; else the selected row, none with several chosen', () => {
  const row = (key: string, selected = false, all = false) => ({ key, selected, all });
  expect(scopeHighlight([row('', true, true), row('g1'), row('g2')], '')).toBe('');
  expect(scopeHighlight([row('', false, true), row('g1'), row('g2', true)], '')).toBe('g2');
  expect(scopeHighlight([row('', false, true), row('g1', true), row('g2', true)], '')).toBe('\n');
  expect(scopeHighlight([row('g2', true), row('g1')], 'g')).toBe('g2');
  expect(scopeHighlight([], 'zzz')).toBe('\n');
});

test('shouldCreateNewThreadInCurrentProject: the picker opens unless there is one open project or none', () => {
  expect(shouldCreateNewThreadInCurrentProject(false, 1, 3)).toBe(true);
  expect(shouldCreateNewThreadInCurrentProject(false, 0, 0)).toBe(true);
  expect(shouldCreateNewThreadInCurrentProject(false, 0, 2)).toBe(false);
  expect(shouldCreateNewThreadInCurrentProject(false, 2, 2)).toBe(false);
  expect(shouldCreateNewThreadInCurrentProject(true, 2, 2)).toBe(true);
  // The New thread tooltip's second line follows it: two projects, one archived, starts in the open one.
  const { client } = rig();
  expect(sidebarSnapshot(client, Date.parse(at), identity).sidebar.newThreadPicks).toBe(true);
  client.shell.projects[0]!.archivedAt = at;
  expect(sidebarSnapshot(client, Date.parse(at), identity).sidebar.newThreadPicks).toBe(false);
});

test('while the project filter is open the thread pin shortcut is its, not the open thread\'s', () => {
  const { client } = rig({ ...ORGANIZE, threadPinning: true });
  client.shell.threads = [{ id: 't1', projectId: 'alpha', title: 'T', status: 'idle', createdAt: at, updatedAt: at, archivedAt: null }];
  client.threadId = 't1';
  const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
  const pins = () => keyboardDispatch(client, [], '', '', context).filter(item => item.command === 'thread.pin').map(item => item.chord);
  expect(pins()).toEqual(['Meta+Shift+p']);
  sidebarSession(client).scopeOpen = true;
  expect(pins()).toEqual([]);
});
