// T3 Code 9c161462c9: rightPanelStore, closedViewStore and reopenClosedView rules over the port.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { moveSurface } from '../../src/panel/right-panel-tabs';
import { panelState, surfaceLocal, surfaceCommand, type PanelState, type Surface } from '../../src/panel/panel-surfaces';
import { deviceStateEvent, deviceStateOf } from '../../src/device/device-surface';
import { savedPanel, adoptRightPanels, restoreRightPanel } from '../../src/panel/saved-panel';
import { adoptClosedViews, captureClosedView, closedViews, rememberClosedView, planNextReopen, type ClosedView, type ReopenOwner } from '../../src/panel/closed-views';
import { contractComponent } from '../support/contract-action-fixture';

const file = (name: string): Surface => ({ id: `file:${name}`, kind: 'file', path: name, line: 3, reveal: 0 });
const state = (): PanelState => ({ surfaces: [file('a.ts'), file('b.ts'), file('c.ts')], active: 'file:b.ts', visible: true, userRevision: 4 });
const native: Native = { available: true, watch() {}, async later() { return {}; } };
function client(): T3Client {
  return { local: {}, environmentId: 'env', threadId: 't1', projectId: 'p1', diffOpen: false, ready: true,
    get draftKey() { return `env:${this.threadId || 'new:p1'}`; }, restAccess: () => ({ call: async () => ({}) }) } as unknown as T3Client;
}
const entry = (id: string, key = 'env:t1'): ClosedView => ({ id, key, surface: file(id) });
const owner = (change: Partial<ReopenOwner> = {}): ReopenOwner => ({ environmentKnown: true, catalogReady: true, ownerExists: true, shellLive: true, panel: { visible: false, surfaces: [] }, ...change });

describe('panel layout belongs to the owning thread', () => {
  test('a drag changes order without changing the selection or proactive-open revision, and persists', () => {
    const panel = state();
    moveSurface(panel, 'file:a.ts', 2);
    expect(panel.surfaces.map(tab => tab.path)).toEqual(['b.ts', 'c.ts', 'a.ts']);
    expect([panel.active, panel.userRevision, panel.visible]).toEqual(['file:b.ts', 4, true]);
    expect(savedPanel(panel)?.surfaces.map(tab => tab.path)).toEqual(['b.ts', 'c.ts', 'a.ts']);
    moveSurface(panel, 'absent', 0); moveSurface(panel, 'file:b.ts', NaN);
    expect(panel.surfaces.map(tab => tab.path)).toEqual(['b.ts', 'c.ts', 'a.ts']);
  });
  test('maximize survives switching threads and a reload, without changing the user revision', async () => {
    const app = client(), a = panelState(app); a.surfaces = [file('a.ts')]; a.active = a.surfaces[0]!.id; a.visible = true;
    await surfaceLocal(app, native, 'maximize', '', '');
    expect(a.maximized).toBe(true); expect(a.userRevision).toBe(0);
    app.threadId = 't2'; expect(panelState(app).maximized).not.toBe(true);
    app.threadId = 't1'; expect(panelState(app).maximized).toBe(true);
    const local = {}; adoptRightPanels(local, { rightPanels: { 'env:t1': savedPanel(a) } });
    const restored = state(); restored.surfaces = []; restored.userRevision = 0;
    expect(restoreRightPanel({ local, preferencesLoaded: true, ready: true }, 'env:t1', restored)).toBe(true);
    expect(restored.maximized).toBe(true);
    await surfaceLocal(app, native, 'hide', '', ''); expect(a.maximized).toBe(false);
  });
  test('pointer movement crosses tab midpoints after the drag threshold', () => {
    const header = contractComponent('panel.contract', 'PanelHeaderBar'), local = header.state(), calls: unknown[][] = [];
    const scope = { max: Math.max, panel: { tabs: [{ id: 'a' }, { id: 'b' }, { id: 'c' }] }, frame: (id: string) => ({ x: id.endsWith('a') ? 0 : id.endsWith('b') ? 64 : 128, width: 60 }), local: (...args: unknown[]) => calls.push(args) };
    header.call('beginTabDrag', local, scope, ['a', 20]);
    header.call('moveTab', local, scope, [5, 0]); expect(calls).toEqual([]);
    header.call('moveTab', local, scope, [150, 0]); expect(calls).toEqual([['surface-move', 'a', '2']]);
    header.call('endTabDrag', local, scope, [150, 0]); expect(local.dragTab).toBe('');
  });
});

describe('reopen the most recently closed view', () => {
  test('history deduplicates targets, retains twenty, and excludes private Browser sessions from persistence', () => {
    const app = client();
    for (let id = 0; id < 24; id++) rememberClosedView(app, entry(String(id)));
    rememberClosedView(app, entry('newest', 'env:t2'));
    expect(closedViews(app)).toHaveLength(20);
    rememberClosedView(app, { ...entry('duplicate', 'env:t2'), surface: file('newest') });
    expect(closedViews(app).filter(view => view.key === 'env:t2')).toHaveLength(1);
    rememberClosedView(app, { ...entry('private'), surface: { id: 'browser:p', kind: 'browser', path: '', line: 0, reveal: 0 }, snapshot: { threadId: 't1', tabId: 'p', navStatus: { _tag: 'Idle' }, canGoBack: false, canGoForward: false, profileId: 'incognito', updatedAt: '' } });
    expect(closedViews(app)[0]!.id).toBe('private');
    expect(JSON.stringify(app.local)).not.toContain('incognito');
  });
  test('unknown ownership waits, removed environments drop, and a deleted thread is skipped but retained', () => {
    const views = [entry('unknown'), entry('older')];
    expect(planNextReopen(views, view => view.id === 'unknown' ? owner({ environmentKnown: false, catalogReady: false }) : owner())).toEqual({ drop: [], restore: null });
    expect(planNextReopen(views, view => view.id === 'unknown' ? owner({ environmentKnown: false }) : owner())).toEqual({ drop: [views[0]], restore: views[1] });
    expect(planNextReopen(views, view => view.id === 'unknown' ? owner({ ownerExists: false }) : owner())).toEqual({ drop: [], restore: views[1] });
    expect(planNextReopen(views, view => view.id === 'unknown' ? owner({ ownerExists: false, shellLive: false }) : owner())).toEqual({ drop: [], restore: null });
  });
  test('an already open tab drops, while a hidden panel can reopen the same tab', () => {
    const view = entry('a.ts');
    expect(planNextReopen([view], () => owner({ panel: { visible: true, surfaces: [view.surface] } }))).toEqual({ drop: [view], restore: null });
    expect(planNextReopen([view], () => owner({ panel: { visible: false, surfaces: [view.surface] } }))).toEqual({ drop: [], restore: view });
  });
  test('closing remembers each removed tab and a draft owns its allocated thread id by project', async () => {
    const app = client(), panel = panelState(app); Object.assign(panel, state());
    await surfaceLocal(app, native, 'close-to-right', 'file:a.ts', '');
    expect(closedViews(app).map(view => view.surface.path)).toEqual(['b.ts', 'c.ts']);
    app.threadId = '';
    expect(captureClosedView(app, 'env:allocated-draft-id', file('draft.ts'))).toMatchObject({ draft: true, projectId: 'p1', key: 'env:allocated-draft-id' });
  });
  test('a successful device power-off remembers its tab, while a rejected power-off keeps it open', async () => {
    const app = client(), panel = panelState(app), target = { hostId: 'local', deviceId: 'sim', platform: 'ios' as const, name: 'Fixture' };
    const surface: Surface = { id: 'device:local:sim', kind: 'device', path: '', line: 0, reveal: 0, device: target };
    Object.assign(panel, { surfaces: [surface], active: surface.id, visible: true });
    await surfaceLocal(app, native, 'activate', surface.id, '');
    deviceStateOf(app); deviceStateEvent(app, { subscriptionId: 'device-1', value: { hosts: [], devices: [{ id: 'sim', hostId: 'local', platform: 'ios' }], sessions: [{ threadId: 't1', deviceId: 'sim', hostId: 'local' }] } });
    let fail = true;
    app.restAccess = (() => ({ request: async () => { if (fail) throw new Error('Fixture refusal'); return {}; } })) as T3Client['restAccess'];
    await surfaceCommand(app, native, {} as never, 'session-poweroff', '', '');
    expect(panel.surfaces).toHaveLength(1); expect(closedViews(app)).toHaveLength(0);
    fail = false; await surfaceCommand(app, native, {} as never, 'session-poweroff', '', '');
    expect(panel.surfaces).toHaveLength(0); expect(closedViews(app)[0]!.surface.device).toEqual(target);
  });
  test('persisted history is validated and retains each surface payload', () => {
    const local = {}; adoptClosedViews(local, { closedViews: [entry('a.ts'), { id: 'bad', key: 'bad', surface: file('b.ts') }, { ...entry('terminal'), surface: { id: 'terminal:one', kind: 'terminal', terminal: { terminalIds: ['one'] } } }] });
    expect(closedViews({ local })).toEqual([entry('a.ts')]);
  });
});
