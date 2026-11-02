// The flat sidebar's rows of a background environment (`fleet:<environment>:<id>`) have the reference's context menu and
// take drags: Sidebar.tsx handleThreadContextMenu builds a row's items from its own environment's capabilities and
// acts on `scopeThreadRef(thread.environmentId, thread.id)`; handleThreadDragEnd plans over every environment's rows
// and writes each order key on that thread's environment.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { obj, str, initialShell, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { sidebarCommand, sidebarLocal, sidebarSelecting } from '../../src/sidebar/sidebar-commands';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import { sidebarSession, setRuntimeClock } from '../../src/sidebar/sidebar-state';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { planDrop, parseDrop } from '../../src/sidebar/sidebar-drop';
import { toasts } from '../../src/shell/toast';

beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => { setRuntimeClock(() => Number.NaN); fleet.entries.clear(); });

const NOW = Date.now();
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const ALL = { threadSettlement: true, threadSnooze: true, threadPinning: true, threadPinReorder: true, threadActiveReorder: true,
  threadAutoSettleOptOut: true, threadTitleRegeneration: true, threadVisitedTracking: true };
const row = (id: string, projectId: string, extra: Obj = {}): Obj => ({ id, projectId, title: `Thread ${id}`, status: 'idle', latestRunId: null,
  createdAt: iso(-60_000), updatedAt: iso(-60_000), archivedAt: null, settledOverride: null, settledAt: null,
  modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });
const KEY = 'https://second.example\nenv-b';

function background(threads: Obj[], caps: Obj = ALL): FleetEntry {
  return { key: KEY, origin: 'https://second.example', environmentId: 'env-b', phase: 'connected', message: '', traceId: '',
    generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {}, config: { environment: { environmentId: 'env-b', label: 'Lane Second', capabilities: caps } },
    shell: { ...initialShell(), projects: [{ id: 'pb', title: 'background-only', workspaceRoot: '/b' }], threads }, scopes: [], error: '', requested: true };
}

/** A client focused on env-a; `here` holds the focused environment's dispatches, `there` env-b's (through its fleet transport). */
function focused(threads: Obj[], caps: Obj = ALL) {
  const here: Obj[] = [], there: Obj[] = [];
  const client = {
    shell: { projects: [{ id: 'p1', title: 'shared-notes', workspaceRoot: '/a' }], threads, sequence: 1 },
    config: { environment: { capabilities: caps }, providers: [], keybindings: [] },
    environmentId: 'env-a', threadId: '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, presentation: {},
    local: { drafts: {}, selections: {}, snapshotDrafts: {}, snapshotReleases: [], composerControls: { contexts: {} }, deviceSettings: { timestampFormat: '24-hour' },
      clientSettings: { confirmThreadArchive: false, confirmThreadDelete: true, confirmThreadUnpin: false, sidebarWorkingShelfEnabled: false }, sidebarWidth: 256 },
    projectGroups() { return [{ key: 'g1', name: 'shared-notes', members: [{ id: 'p1' }] }, { key: 'g2', name: 'background-only', members: [{ id: 'fleet:env-b:pb' }] }]; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
      request: async (method: string, payload: Obj) => { if (method === 'orchestration.dispatchCommand') here.push(payload); return {}; },
      call: async () => ({}),
    }),
  } as unknown as T3Client;
  const native = { available: true, watch() {}, later: async (request: unknown) => {
    const sent = obj(request), payload = obj(sent.payload);
    if (sent.fleet === KEY && sent.method === 'orchestration.dispatchCommand') there.push(payload);
    return { ok: true, generation: Number(sent.generation) || 2, value: {} };
  } } as unknown as Native;
  return { client, native, here, there };
}
const helpers = { projectIdentity: (name: string) => ({ projectMark: name.slice(0, 2).toUpperCase(), projectInk: 'ink', projectSurface: 'surface' }),
  providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
const strip = (payloads: Obj[]) => payloads.map(({ commandId: _id, ...rest }) => rest);
const ids = (menu: { id: string; parent: string }[]) => menu.filter(entry => entry.parent === '').map(entry => entry.id);

describe('a background environment\'s row menu', () => {
  test('is the row\'s own, from its environment\'s capabilities', () => {
    fleet.entries.set(KEY, background([row('tb', 'pb')], { threadSettlement: true, threadPinning: true }));
    const { client } = focused([row('a', 'p1')]);
    const threads = sidebarSnapshot(client, NOW, helpers).threads;
    const away = threads.find(thread => thread.id === 'fleet:env-b:tb')!, home = threads.find(thread => thread.id === 'a')!;
    expect(ids(away.menu)).toEqual(['pin', 'settle', 'rename', 'mark-unread', 'filter-by-project', 'copy', 'project-settings', 'archive', 'delete']);
    expect(ids(home.menu)).toContain('snooze');
    expect(ids(home.menu)).toContain('regenerate-title');
  });

  test('its snooze popover lists its own environment\'s options, which its pick resolves against', async () => {
    const entry = background([row('tb', 'pb')]);
    (entry.config as Obj).settings = { snoozePresets: [{ kind: 'after', minutes: 45 }] };
    fleet.entries.set(KEY, entry);
    const { client, native, there } = focused([row('a', 'p1')]);
    const threads = sidebarSnapshot(client, NOW, helpers).threads;
    expect(threads.find(thread => thread.id === 'fleet:env-b:tb')!.snoozePresets.map(preset => preset.label)).toEqual(['In 45 minutes']);
    expect(threads.find(thread => thread.id === 'a')!.snoozePresets.map(preset => preset.id)).toContain('after:60');
    await sidebarCommand(client, native, {} as Files, 'snooze:after:45', 'fleet:env-b:tb', '');
    expect(strip(there).map(payload => [payload.type, payload.threadId])).toEqual([['thread.snooze', 'tb']]);
  });

  test('its choices act on that environment with its own thread id', async () => {
    fleet.entries.set(KEY, background([row('tb', 'pb')]));
    const { client, native, here, there } = focused([row('a', 'p1')]);
    await sidebarCommand(client, native, {} as Files, 'menu-choice', 'fleet:env-b:tb', 'pin');
    await sidebarCommand(client, native, {} as Files, 'menu-choice', 'fleet:env-b:tb', 'auto-settle:disabled');
    await sidebarCommand(client, native, {} as Files, 'menu-choice', 'fleet:env-b:tb', 'regenerate-title');
    expect(here).toEqual([]);
    expect(strip(there).map(payload => [payload.type, payload.threadId])).toEqual([['thread.pin', 'tb'], ['thread.auto-settle.set', 'tb'], ['thread.metadata.update', 'tb']]);
    expect(toasts(client)).toEqual([]);
  });

  test('Project settings opens the background member; Rename edits the row and renames it there', async () => {
    fleet.entries.set(KEY, background([row('tb', 'pb')]));
    const { client, native, there } = focused([row('a', 'p1')]);
    expect(await sidebarCommand(client, native, {} as Files, 'menu-choice', 'fleet:env-b:tb', 'project-settings')).toBe('sidebar:project-settings:fleet:env-b:pb');
    await sidebarCommand(client, native, {} as Files, 'menu-choice', 'fleet:env-b:tb', 'rename');
    expect(sidebarSession(client).renameId).toBe('fleet:env-b:tb');
    await sidebarCommand(client, native, {} as Files, 'rename', 'fleet:env-b:tb', 'Renamed');
    expect(strip(there)).toEqual([{ type: 'thread.metadata.update', title: 'Renamed', threadId: 'tb' }]);
  });

  test('a double click starts the rename on a background row too', async () => {
    fleet.entries.set(KEY, background([row('tb', 'pb')]));
    const { client, native } = focused([row('a', 'p1')]);
    await sidebarLocal(client, native, 'rename-start', 'fleet:env-b:tb', '');
    expect(sidebarSession(client)).toMatchObject({ renameId: 'fleet:env-b:tb', renameTitle: 'Thread tb' });
  });

  test('a selection across environments gets the bulk menu, counted by each row\'s capabilities, and settles each on its own', async () => {
    fleet.entries.set(KEY, background([row('tb', 'pb', { pinnedAt: iso(-1_000), pinOrderKey: 'a0' })], { threadSettlement: true }));
    const { client, native, here, there } = focused([row('a', 'p1', { pinnedAt: iso(-1_000), pinOrderKey: 'a1' })]);
    await sidebarSelecting(client, native, 'a', 'click:meta');
    await sidebarSelecting(client, native, 'fleet:env-b:tb', 'click:meta');
    const menu = sidebarSnapshot(client, NOW, helpers).threads.find(thread => thread.id === 'fleet:env-b:tb')!.menu;
    // env-b does not pin: Unpin counts the focused row alone; Snooze needs every row's environment to snooze.
    expect(menu.filter(entry => entry.parent === '').map(entry => entry.label)).toEqual(['Unpin (1)', 'Settle (2)', 'Regenerate titles (1)', 'Mark unread (2)', 'Delete (2)']);
    await sidebarCommand(client, native, {} as Files, 'menu-choice', 'fleet:env-b:tb', 'settle');
    expect(strip(here).map(payload => [payload.type, payload.threadId])).toEqual([['thread.settle', 'a']]);
    expect(strip(there).map(payload => [payload.type, payload.threadId])).toEqual([['thread.settle', 'tb']]);
  });
});

describe('dragging a background environment\'s row', () => {
  const drop = (target: string, centerY: number) => `${target}|${centerY}|0|0|false`;

  test('reorders Active with each key written on its own environment', async () => {
    fleet.entries.set(KEY, background([row('tb', 'pb', { activeOrderKey: 'u', updatedAt: iso(-30_000) })]));
    const { client, native, here, there } = focused([row('a', 'p1', { activeOrderKey: 'n', updatedAt: iso(-10_000) }), row('b', 'p1', { activeOrderKey: 'x', updatedAt: iso(-20_000) })]);
    const before = sidebarSnapshot(client, NOW, helpers).threads.map(thread => thread.id);
    expect(before).toEqual(['a', 'fleet:env-b:tb', 'b']);
    // Lifted to the top of Active: its centre above the first row's middle (2 labels + half a card under the divider at 0).
    await sidebarCommand(client, native, {} as Files, 'drop', 'fleet:env-b:tb', drop('active', 10));
    expect(here).toEqual([]);
    expect(strip(there)).toEqual([{ type: 'thread.active.reorder', orderKey: expect.any(String), threadId: 'tb' }]);
    expect(str(strip(there)[0]!.orderKey) < 'n').toBe(true);
    expect(toasts(client)).toEqual([]);
  });

  test('pins it on its environment', async () => {
    fleet.entries.set(KEY, background([row('tb', 'pb')]));
    const { client, native, here, there } = focused([row('a', 'p1', { pinnedAt: iso(-1_000), pinOrderKey: 'n' })]);
    await sidebarCommand(client, native, {} as Files, 'drop', 'fleet:env-b:tb', drop('pinned', 0));
    expect(here).toEqual([]);
    expect(strip(there)).toEqual([{ type: 'thread.pin', orderKey: 'h', threadId: 'tb' }]);
  });

  test('a key rewrite that reaches the focused environment\'s rows writes theirs here', async () => {
    fleet.entries.set(KEY, background([row('tb', 'pb')]));
    // Keys that leave no room before 'a' make the drop give every pinned row a fresh key.
    const { client, native, here, there } = focused([row('a', 'p1', { pinnedAt: iso(-1_000), pinOrderKey: '0' })]);
    await sidebarCommand(client, native, {} as Files, 'drop', 'fleet:env-b:tb', drop('pinned', 0));
    expect(strip(there).map(payload => [payload.type, payload.threadId])).toEqual([['thread.pin', 'tb']]);
    expect(strip(here).map(payload => [payload.type, payload.threadId])).toEqual([['thread.pin.reorder', 'a']]);
  });

  test('writes nothing when a key would land on a row whose environment cannot take it (reorderableKeys)', () => {
    const geometry = parseDrop('pinned|0|0|0|false');
    const base = { id: 'x', from: 'active', target: 'pinned', pinnedIds: ['p', 'q'], activeIds: ['x'], activeKeys: new Map(), geometry,
      supportsSettlement: true, pinReorder: true, activeReorder: true };
    // No keys yet: the drop rewrites every pinned row's key.
    const pinnedKeys = new Map<string, string | null>([['p', null], ['q', null]]);
    expect(planDrop({ ...base, pinnedKeys }).steps.length).toBeGreaterThan(1);
    const allowed = planDrop({ ...base, pinnedKeys, pinReorderable: new Set(['x', 'p']) });
    expect(allowed.verb).toBe('pin');
    expect(allowed.steps.map(step => step.threadId)).toEqual(['x', 'p']);
    expect(planDrop({ ...base, pinnedKeys, pinReorderable: new Set(['x', 'p', 'q']) }).verb).toBe('pin');
  });

  // Sidebar.logic.test.ts at 2359643cff, "reordering with cached offline configuration".
  test('a disconnected environment keeps its cached config but its rows are not reorderable', async () => {
    // A background environment's rows leave the list while it is away; the focused one's stay, from its cached shell.
    fleet.entries.set(KEY, background([row('tb', 'pb', { activeOrderKey: 'u', updatedAt: iso(-30_000) })]));
    const { client, native, here, there } = focused([row('a', 'p1', { activeOrderKey: 'n', updatedAt: iso(-10_000) })]);
    Object.assign(client, { connection: 'reconnecting' });
    expect(sidebarSnapshot(client, NOW, helpers).threads.map(thread => thread.id)).toEqual(['a', 'fleet:env-b:tb']);
    await sidebarCommand(client, native, {} as Files, 'drop', 'a', drop('active', 1000));
    expect([here, there]).toEqual([[], []]);
    // The connected row still moves above the offline row, whose key stays a bound.
    await sidebarCommand(client, native, {} as Files, 'drop', 'fleet:env-b:tb', drop('active', 10));
    expect(strip(there)).toEqual([{ type: 'thread.active.reorder', orderKey: expect.any(String), threadId: 'tb' }]);
    expect(str(strip(there)[0]!.orderKey) < 'n').toBe(true);
    expect(here).toEqual([]);
  });

  const frozen = (target: 'active' | 'pinned', ids: string[], centerY: number) => planDrop({ id: 'online', from: target, target, geometry: parseDrop(`${target}|${centerY}|0|0|false`),
    pinnedIds: target === 'pinned' ? ids : [], activeIds: target === 'active' ? ids : [], pinnedKeys: new Map(ids.map(id => [id, null])), activeKeys: new Map(ids.map(id => [id, null])),
    supportsSettlement: true, pinReorder: true, activeReorder: true, pinReorderable: new Set(ids.filter(id => id !== 'offline')), activeReorderable: new Set(ids.filter(id => id !== 'offline')) });
  test('rejects crossing the frozen keyless boundary', () => {
    // Keyless Active rows sort before keyed ones: a key for the row lifted above the offline keyless row lands below it.
    expect(frozen('active', ['offline', 'online'], 0).steps).toEqual([]);
    // Keyless Pinned rows sort after keyed ones: a key for the row dropped below the offline keyless row lands above it.
    expect(frozen('pinned', ['online', 'offline'], 1000).steps).toEqual([]);
  });
  test('allows realizable moves beside a frozen keyless row', () => {
    expect(frozen('active', ['online', 'offline', 'neighbor'], 1000).steps.map(step => step.threadId)).toEqual(['neighbor', 'online']);
    expect(frozen('pinned', ['neighbor', 'online', 'offline'], 0).steps.map(step => step.threadId)).toEqual(['online', 'neighbor']);
  });
});
