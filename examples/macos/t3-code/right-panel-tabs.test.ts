// Ported from T3 Code 1e2ecbd975 rightPanelStore.test.ts (MIT, LICENSE-T3).
// closeSurface maps to closeSurfaceIn; files/diff/devices replace Browser/terminal.
// Host/device test port is complete; Zustand/refA plumbing becomes PanelState.
import { describe, expect, test } from 'bun:test';
import { closeSurface, closeOtherSurfaces, closeSurfacesToRight, closeAllSurfaces, openDeviceSurface, renameDevice, tabContextMenuItems, registerSurfaceClose, closePanelSurfaces, editTabName, tabRename, copyTabPath, showTabMenu } from './right-panel-tabs';
import { panelState, surfaceLocal, type PanelState, type Surface } from './r4-surfaces-panel';
import { savedPanel, adoptRightPanels, restoreRightPanel } from './r10-device-panels';
import { deviceTargetOf } from './r6-media-device';
import { toasts } from './toast';
import type { T3Client } from './client';
import type { Native } from './protocol';
const surface = (id: string, kind: Surface['kind'] = 'file'): Surface => ({ id, kind, path: id, line: 0, reveal: 0 });
const panel = (): PanelState => ({ surfaces: [surface('a'), surface('b'), surface('c')], active: 'b', visible: true, userRevision: 0 });
const android = { hostId: 'nucbox', deviceId: 'emulator-5580', name: 'Pixel', platform: 'android' };
const ios = { hostId: 'macmini', deviceId: 'ios-1', name: 'iPhone', platform: 'ios' };
const native: Native = { available: true, watch() {}, later: async () => ({}) };
function client(call: (request: Record<string, unknown>) => Promise<unknown> = async () => ({})): T3Client {
  return { draftKey: 'env:t1', diffOpen: false, restAccess: () => ({ call }) } as unknown as T3Client;
}
describe('rightPanelStore', () => {
  test('closing the active surface activates a neighboring surface', () => { const state = panel(); closeSurface(state, 'b'); expect(state.active).toBe('c'); closeSurface(state, 'c'); expect(state.active).toBe('a'); });
  test('closing the final surface closes the panel', () => { const state = panel(); for (const id of ['a', 'b', 'c']) closeSurface(state, id); expect(state).toMatchObject({ active: '', visible: false, surfaces: [] }); });
  test('closing other surfaces keeps the selected surface active', () => { const state = panel(); state.visible = false; closeOtherSurfaces(state, 'a'); expect(state).toMatchObject({ active: 'a', visible: true }); expect(state.surfaces.map(entry => entry.id)).toEqual(['a']); });
  test('closing surfaces to the right activates the selected surface when active was removed', () => { const state = panel(); closeSurfacesToRight(state, 'a'); expect(state.active).toBe('a'); expect(state.surfaces.map(entry => entry.id)).toEqual(['a']); });
  test('closing all surfaces closes the panel', () => { const state = panel(); closeAllSurfaces(state); expect(state).toMatchObject({ surfaces: [], active: '', visible: false }); });
  test('inactive close preserves selection; close-right preserves a surviving active tab and visibility', () => { const state = panel(); closeSurface(state, 'a'); expect(state.active).toBe('b'); state.visible = false; closeSurfacesToRight(state, 'b'); expect(state).toMatchObject({ active: 'b', visible: false }); });
  test('disabled and missing-target closes do nothing', () => { const state = panel(), before = structuredClone(state); closeSurface(state, 'missing'); closeOtherSurfaces(state, 'missing'); closeSurfacesToRight(state, 'c'); expect(state).toEqual(before); closeOtherSurfaces(state, 'b'); state.visible = false; closeOtherSurfaces(state, 'b'); expect(state.visible).toBe(false); });
  test('gives each host/device its own tab and preserves renamed tabs', () => {
    const state = panel(); state.surfaces = [surface('device', 'device')];
    openDeviceSurface(state, android); state.surfaces.push(surface('device', 'device'));
    expect(state.surfaces).toHaveLength(2); openDeviceSurface(state, ios);
    expect(state.surfaces.map(entry => entry.id)).toEqual(['device:nucbox:emulator-5580', 'device:macmini:ios-1']);
    renameDevice(state, 'device:nucbox:emulator-5580', 'Android test'); openDeviceSurface(state, android);
    expect(state.surfaces).toHaveLength(2); expect(state.surfaces[0]).toMatchObject({ title: 'Android test', device: android });
    expect(state.active).toBe('device:nucbox:emulator-5580'); closeSurface(state, state.active);
    expect(state.surfaces).toEqual([expect.objectContaining({ device: ios })]);
  });
  test('does not collide when two hosts expose the same device id', () => { const state = panel(); state.surfaces = []; openDeviceSurface(state, { ...android, hostId: 'a:b' }); openDeviceSurface(state, { ...android, hostId: 'a' }); expect(new Set(state.surfaces.map(entry => entry.id)).size).toBe(2); });
  test('renames trim, fall back to device name or Device, and leave other kinds alone', () => { const state = panel(), opened = openDeviceSurface(state, android); renameDevice(state, opened.id, ' Name '); expect(opened.title).toBe('Name'); renameDevice(state, opened.id, ' '); expect(opened.title).toBe('Pixel'); const placeholder = surface('device', 'device'); state.surfaces.push(placeholder); renameDevice(state, 'device', ''); expect(placeholder.title).toBe('Device'); renameDevice(state, 'a', 'No'); expect(state.surfaces[0]?.title).toBeUndefined(); });
  test('restores separate device targets and titles after relaunch', () => { const state = panel(); state.surfaces = []; openDeviceSurface(state, android); const second = openDeviceSurface(state, ios); renameDevice(state, second.id, 'Test phone'); const saved = savedPanel(state); const local = {}; adoptRightPanels(local, { rightPanels: { thread: JSON.parse(JSON.stringify(saved)) } }); const fresh = { ...panel(), surfaces: [] }; expect(restoreRightPanel({ local, ready: true, preferencesLoaded: true }, 'thread', fresh)).toBe(true); expect(fresh.surfaces).toEqual(state.surfaces); expect(fresh.active).toBe(second.id); });
});
describe('tab menu and rename editor', () => {
  test('menu order, kinds and disabled flags', () => { const state = panel(); expect(tabContextMenuItems(state.surfaces[0]!, state.surfaces).map(item => item.id)).toEqual(['copy-path', 'close', 'close-others', 'close-to-right', 'close-all']); const device = openDeviceSurface(state, android); expect(tabContextMenuItems(device, state.surfaces).map(item => item.id)).toEqual(['rename', 'close', 'close-others', 'close-to-right', 'close-all']); expect(tabContextMenuItems(device, state.surfaces).find(item => item.id === 'close-to-right')?.disabled).toBe(true); const diff = surface('diff', 'diff'); expect(tabContextMenuItems(diff, [diff])).toEqual([{ id: 'close', label: 'Close' }, { id: 'close-others', label: 'Close others', disabled: true }, { id: 'close-to-right', label: 'Close to the right', disabled: true }, { id: 'close-all', label: 'Close all', disabled: false }]); expect(tabContextMenuItems(diff, [])).toEqual([]); const attachment = { ...surface('attached'), attachment: { id: 'a', name: 'a', mimeType: '', sizeBytes: 0 } }; expect(tabContextMenuItems(attachment, [attachment]).some(item => item.id === 'copy-path')).toBe(false); });
  test('Enter/blur commit; Escape cancels even if blur follows', () => { const state = panel(), device = openDeviceSurface(state, android); editTabName(state, 'rename', device.id, ''); expect(tabRename(state)).toEqual({ id: device.id, value: 'Pixel' }); editTabName(state, 'rename-edit', device.id, 'Changed'); editTabName(state, 'rename-cancel', device.id, ''); editTabName(state, 'rename-commit', device.id, ''); expect(device.title).toBeUndefined(); editTabName(state, 'rename', device.id, ''); editTabName(state, 'rename-edit', device.id, ' New '); editTabName(state, 'rename-commit', device.id, ''); expect(device.title).toBe('New'); expect(tabRename(state).id).toBe(''); });
  test('menu ignores disabled, unknown and dismissed responses', async () => { const state = panel(); for (const clicked of ['close-to-right', 'toggle-mute', '', null]) expect(await showTabMenu(client(async () => ({ clicked })), native, state, 'c')).toBe(''); });
});
describe('close hooks and integration', () => {
  test('single close can be cancelled, all bulk actions bypass guard but run cleanup', async () => {
    const owner = {}, calls: string[] = [];
    const unregister = registerSurfaceClose(owner, 'file', { guard: entry => { calls.push(`guard:${entry.id}`); return false; }, cleanup: entry => { calls.push(`cleanup:${entry.id}`); } });
    const state = panel(); await closePanelSurfaces(owner, state, 'close', 'a'); expect(calls).toEqual(['guard:a']); expect(state.surfaces).toHaveLength(3);
    for (const action of ['close-others', 'close-to-right', 'close-all']) { calls.length = 0; await closePanelSurfaces(owner, panel(), action, 'a'); expect(calls).toEqual(action === 'close-all' ? ['cleanup:a', 'cleanup:b', 'cleanup:c'] : ['cleanup:b', 'cleanup:c']); }
    unregister(); calls.length = 0; registerSurfaceClose(owner, 'file', { guard: () => true, cleanup: entry => { calls.push(entry.id); } }); await closePanelSurfaces(owner, state, 'close', 'a'); expect(calls).toEqual(['a']);
  });
  test('an async cleanup preserves newly opened tabs and a newer selection', async () => {
    const owner = {}, state = panel();
    let done: (() => void) | undefined;
    registerSurfaceClose(owner, 'file', { cleanup: () => new Promise<void>(resolve => { done = resolve; }) });
    const closing = closePanelSurfaces(owner, state, 'close', 'b');
    await Promise.resolve();
    state.surfaces.push(surface('new')); state.active = 'new';
    done?.(); await closing;
    expect(state.surfaces.map(entry => entry.id)).toEqual(['a', 'c', 'new']);
    expect(state.active).toBe('new');
  });
  test('activating device tabs selects each retained device target', async () => {
    const c = client(), state = panelState(c), first = openDeviceSurface(state, android), second = openDeviceSurface(state, ios);
    await surfaceLocal(c, native, 'activate', first.id, '');
    expect(deviceTargetOf(c, c.draftKey)).toEqual(android);
    await surfaceLocal(c, native, 'activate', second.id, '');
    expect(deviceTargetOf(c, c.draftKey)).toEqual(ios);
    await surfaceLocal(c, native, 'close', second.id, '');
    expect(deviceTargetOf(c, c.draftKey)).toEqual(android);
  });
  test('all close paths keep diffOpen synchronized', async () => { for (const action of ['close', 'close-others', 'close-to-right', 'close-all']) { const c = client(), state = panelState(c); state.surfaces = [surface('file:a'), surface('diff', 'diff')]; state.active = 'diff'; state.visible = true; c.diffOpen = true; await surfaceLocal(c, native, action, action === 'close' ? 'diff' : 'file:a', ''); expect(c.diffOpen).toBe(false); expect(state.surfaces.some(entry => entry.kind === 'diff')).toBe(false); } });
});
describe('Copy path', () => {
  test('copies relative path and reports success', async () => { const requests: unknown[] = [], c = client(async request => { requests.push(request); return { copied: true }; }); await copyTabPath(c, native, surface('src/app.ts')); expect(requests).toEqual([{ op: 'copyText', text: 'src/app.ts' }]); expect(toasts(c)[0]).toMatchObject({ title: 'Path copied', description: 'src/app.ts' }); });
  test('reports clipboard failures and unavailable API', async () => { for (const failure of [new Error('Denied'), null]) { const c = client(async () => { throw failure; }); await copyTabPath(c, native, surface('a')); expect(toasts(c)[0]).toMatchObject({ title: 'Failed to copy path', description: failure?.message ?? 'Clipboard API unavailable.' }); } });
});
