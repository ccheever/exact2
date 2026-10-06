// Lane r12-sidebar: the row-action sweep's Escape, the rows' keyboard context
// menus, the Environments list without a switched-off primary, and the sidebar's
// minimum width at the Interface font size (f870c419fc).
import { describe, expect, test } from 'bun:test';
import './client';
import type { T3Client } from './client';
import { initialShell, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { sidebarCommand } from './sidebar-commands';
import { adoptCommandTime, sidebarSession } from './sidebar-state';
import { parseSweep } from './r11-upstream-sweep';
import { isMenuKey, menuAnchor, rowKeyMenu, withMenuAnchor } from './r12-sidebar-keys';
import { environmentRows } from './r12-sidebar-connections';
import { connectionsProjection, type ConnectionHost } from './connections';
import { environmentKey, type FleetEntry } from './settings-b-fleet';
import { brandProbeWidth, clampSidebarWidth, sidebarMinimumWidth } from './r12-sidebar-width';
import { sidebarLaunchWidth } from './r4-polish-sidebar-width';

const NOW = Date.parse('2026-10-05T12:00:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true };
const shell = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null,
  activeProviderThreadId: null, pendingRuntimeRequest: null, createdAt: iso(-3_600_000), updatedAt: iso(-3_600_000), archivedAt: null,
  settledOverride: null, settledAt: null, modelSelection: { instanceId: 'codex', model: 'm' }, lineage: { relationshipToParent: null }, ...extra });

function fake(threads: Obj[], picks: string[] = []) {
  const dispatched: Obj[] = [], calls: Obj[] = [];
  let ids = 0;
  const client = {
    shell: { projects: [{ id: 'p1', title: 'Parity fixture', workspaceRoot: '/fixture' }], threads, sequence: 1 },
    config: { environment: { capabilities: CAPS }, providers: [], keybindings: [] },
    environmentId: 'env', threadId: '', projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true,
    presentation: {}, local: { drafts: { 'env:new:p1': 'Second sketch' }, snapshotDrafts: {}, snapshotReleases: [], composerControls: { contexts: {} }, deviceSettings: { timestampFormat: '24-hour' },
      clientSettings: { confirmThreadArchive: false, confirmThreadDelete: true, confirmThreadUnpin: false }, sidebarWidth: 256 },
    projection: { runs: [], turnItems: [] },
    projectGroups() { return [{ key: 'g1', name: 'Parity fixture', members: [{ id: 'p1' }] }]; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, () => `c${ids++}`),
      request: async (method: string, payload: Obj) => { if (method === 'orchestration.dispatchCommand') dispatched.push(payload); return {}; },
      dispatch: async (_storage: Files, payload: Obj) => { dispatched.push(payload); return {}; },
      call: async (request: Obj) => { calls.push(request); return request.op === 'sidebarMenu' ? { id: picks.shift() ?? null } : {}; },
    }),
  } as unknown as T3Client;
  adoptCommandTime(client, NOW);
  return { client, dispatched, calls };
}
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const files = {} as Files;

describe('a row-action sweep cancelled with Escape (SidebarPointerSensor keydown)', () => {
  test('the cancelled release applies nothing, not even the pressed button, and ends the sweep', async () => {
    const { client, dispatched } = fake([shell('a'), shell('b'), shell('s1', { settledOverride: 'settled', settledAt: iso(-60_000) })]);
    const epoch = sidebarSession(client).sweepEpoch;
    expect(parseSweep('sweep|settle|active|a||cancel|')?.keys).toEqual(['cancel']);
    await sidebarCommand(client, native, files, 'drop', 'sweep|settle|active|a||cancel|', '|0|0|0|false', NOW);
    await sidebarCommand(client, native, files, 'drop', 'sweep|unsettle|settled|s1||cancel|', '', NOW);
    expect(dispatched).toEqual([]);
    expect(sidebarSession(client).sweepEpoch).toBe(epoch + 2);
    // An uncancelled sweep still applies.
    await sidebarCommand(client, native, files, 'drop', 'sweep|settle|active|a||a|b|', '', NOW);
    expect(dispatched.map(entry => [entry.type, entry.threadId])).toEqual([['thread.settle', 'a'], ['thread.settle', 'b']]);
  });
});

describe('keyboard context menus on sidebar rows (refkbd.mjs on the f870c41 reference)', () => {
  test('ContextMenu opens a thread row menu at its centre and a draft row menu at its bottom left; other keys do nothing', () => {
    expect(rowKeyMenu('t1', 'ContextMenu')).toEqual({ op: 'menu', id: 't1', value: 'row', anchor: 'center' });
    expect(rowKeyMenu('draft:p1', 'ContextMenu')).toEqual({ op: 'draft-menu', id: 'p1', value: 'key', anchor: 'bottom-left' });
    expect(rowKeyMenu('t1', '')?.anchor).toBe('center');
    // Shift+F10 is the draft row's window shortcut; F10 at the row key handler (no modifiers) is not a menu key.
    for (const key of ['F10', '', 'Enter', ' ', 'a', 'Escape']) expect(rowKeyMenu('t1', key)).toBeNull();
    expect(rowKeyMenu('draft:', 'ContextMenu')).toBeNull();
    expect(isMenuKey('ContextMenu')).toBe(true);
  });

  test('a keyed thread menu asks the native menu for the focused row anchor; a right click does not', async () => {
    const { client, calls, dispatched } = fake([shell('t1')], ['settle']);
    await sidebarCommand(client, native, files, 'row-key', 't1', 'ContextMenu', NOW);
    expect(calls.filter(call => call.op === 'sidebarMenu').map(call => call.anchor)).toEqual(['center']);
    expect(dispatched.map(entry => entry.type)).toEqual(['thread.settle']);
    await sidebarCommand(client, native, files, 'menu', 't1', 'row', NOW);
    expect(calls.filter(call => call.op === 'sidebarMenu').map(call => call.anchor)).toEqual(['center', undefined]);
    expect(menuAnchor(client)).toEqual({});
  });

  test('a draft row: ContextMenu and the Shift+F10 shortcut anchor at the row; the right click at the pointer', async () => {
    const { client, calls } = fake([shell('t1')]);
    await sidebarCommand(client, native, files, 'row-key', 'draft:p1', 'ContextMenu', NOW);
    await sidebarCommand(client, native, files, 'draft-menu', 'p1', 'key', NOW);
    await sidebarCommand(client, native, files, 'draft-menu', 'p1', '', NOW);
    const menus = calls.filter(call => call.op === 'sidebarMenu');
    expect(menus.map(call => call.anchor)).toEqual(['bottom-left', 'bottom-left', undefined]);
    expect((menus[0]!.items as Obj[]).map(item => item.label)).toEqual(['Copy', 'Project settings', undefined, 'Discard draft']);
  });

  test('the anchor never outlives its menu, even when the op throws', async () => {
    const { client } = fake([]);
    await expect(withMenuAnchor(client, 'center', async () => { expect(menuAnchor(client)).toEqual({ anchor: 'center' }); throw new Error('x'); })).rejects.toThrow('x');
    expect(menuAnchor(client)).toEqual({});
  });
});

describe('Environments without a switched-off primary (ConnectionsSettings savedEnvironments)', () => {
  const A = 'http://127.0.0.1:15083', B = 'http://127.0.0.1:15084';
  const focusB = (): ConnectionHost => ({ connection: 'connected', origin: B, environmentId: 'env-b', statusMessage: 'Connected.', scopes: ['orchestration:read'],
    config: { environment: { label: 'Mac B', platform: { machine: 'laptop' } } } });
  const live = (origin: string, environmentId: string): FleetEntry => ({ key: environmentKey(origin, environmentId), origin, environmentId, phase: 'connected', message: '',
    traceId: '', generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(), scopes: [], error: '', requested: true });

  test('A switched off with B on: only B is listed, while Load balancing still counts A as This machine', () => {
    const saved = [{ origin: A, environmentId: 'env-a', label: 'Mac A', enabled: false }, { origin: B, environmentId: 'env-b', label: 'Mac B', enabled: true }];
    const page = connectionsProjection(focusB(), saved, new Map([[environmentKey(B, 'env-b'), live(B, 'env-b')]]));
    expect(page.environments.map(row => [row.label, row.first, row.enabled])).toEqual([['Mac B', true, true]]);
    expect(page.machines.map(machine => [machine.label, machine.subtitle])).toEqual([['Mac A', 'This machine'], ['Mac B', 'http://127.0.0.1:15084/']]);
  });

  test('the primary stays listed while it is on, or while nothing else is on (so it can be switched back on)', () => {
    const rows = (a: boolean, b: boolean) => environmentRows([{ key: 'a', origin: A, enabled: a }, { key: 'b', origin: B, enabled: b }]).map(row => row.key);
    expect(rows(true, true)).toEqual(['a', 'b']);
    expect(rows(false, true)).toEqual(['b']);
    expect(rows(false, false)).toEqual(['a', 'b']);
    // Remote machines only: no primary, every row stays.
    expect(environmentRows([{ key: 'r', origin: 'https://one.example.com', enabled: false }, { key: 's', origin: 'https://two.example.com', enabled: true }]).map(row => row.key)).toEqual(['r', 's']);
  });
});

describe('the sidebar minimum width follows the brand at the Interface font size', () => {
  test('max(13rem, ceil(brand probe)): the macOS probe is 90 + 2.5rem + mark + 0.75rem + 1', () => {
    expect(brandProbeWidth(16)).toBeCloseTo(196.78125, 5);
    expect([12, 16, 17, 18, 19, 20].map(sidebarMinimumWidth)).toEqual([208, 208, 208, 210, 216, 223]);
    expect([undefined, 'x', 30, 4].map(sidebarMinimumWidth)).toEqual([208, 208, 223, 208]);
  });

  test('a width clamps to [minimum, max(minimum, viewport - 40rem)]', () => {
    expect(clampSidebarWidth(256, 1280, 223)).toBe(256);
    expect(clampSidebarWidth(208, 840, 223)).toBe(223);
    expect(clampSidebarWidth(400, 900, 210)).toBe(260);
  });

  test('the launch width (no stored width) is clamped to the live minimum', () => {
    const owner = { local: { clientSettings: { fontSizeInterface: 20 } }, preferencesLoaded: true };
    expect(sidebarLaunchWidth(owner, 840, true)).toBe(223);
    // Sized once at load (208 at 840), then only clamped: a wider window keeps it.
    expect(sidebarLaunchWidth(owner, 1280, true)).toBe(223);
    expect(sidebarLaunchWidth({ local: { clientSettings: { fontSizeInterface: 20 } }, preferencesLoaded: true }, 1280, true)).toBe(256);
    owner.local.clientSettings.fontSizeInterface = 16;
    expect(sidebarLaunchWidth(owner, 840, true)).toBe(208);
  });
});
