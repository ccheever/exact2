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
import { appPath } from '../support/app-files';
const root = () => contractComponent('app.contract', 'T3Code');

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
  test('Maximize leads the layout controls in either layout; the sheet, which covers the details card, then offers its toggle', () => {
    const header = contractComponent('panel.contract', 'PanelHeaderBar'), local = header.state();
    const scope = (sheet: boolean) => ({ sheet, min: Math.min, max: Math.max, panel: { tabStrip: { activeLeft: 0, activeRight: 0, view: 0, max: 0, serial: 0, overflow: false } } });
    expect([header.drawn('testId="maximize-right-panel"', local, scope(true)), header.drawn('testId="panel-toggle-details"', local, scope(true))]).toEqual([true, true]);
    expect([header.drawn('testId="maximize-right-panel"', local, scope(false)), header.drawn('testId="panel-toggle-details"', local, scope(false))]).toEqual([true, false]);
  });
  test('pointer movement crosses tab midpoints after the drag threshold', () => {
    const header = contractComponent('panel.contract', 'PanelHeaderBar'), local = header.state(), calls: unknown[][] = [];
    const scope = { max: Math.max, panel: { tabs: [{ id: 'a' }, { id: 'b' }, { id: 'c' }] }, frame: (id: string) => ({ x: id.endsWith('a') ? 0 : id.endsWith('b') ? 64 : 128, width: 60 }), local: (...args: unknown[]) => calls.push(args) };
    header.call('beginTabDrag', local, scope, ['a', 20]);
    header.call('moveTab', local, scope, [5, 0]); expect(calls).toEqual([]);
    header.call('moveTab', local, scope, [150, 0]); expect(calls).toEqual([['surface-move', 'a', '2']]);
    header.call('endTabDrag', local, scope, [150, 0]); expect(local.dragTab).toBe('');
  });
  test('smooth native pan increments cross the neighboring tab midpoint', () => {
    const header = contractComponent('panel.contract', 'PanelHeaderBar'), local = header.state(), calls: unknown[][] = [];
    const scope = { max: Math.max, panel: { tabs: [{ id: 'a' }, { id: 'b' }] },
      frame: (id: string) => ({ x: id.endsWith('a') ? 749 : 805.48, width: id.endsWith('a') ? 52.48 : 53.11 }),
      local: (...args: unknown[]) => calls.push(args) };
    header.call('beginTabDrag', local, scope, ['a', 783.24]);
    // The 80pt/400ms native contact emits 24 increments after its initial slop.
    for (const dx of [6.4, ...Array(23).fill(3.2)]) header.call('moveTab', local, scope, [dx, 0]);
    expect(local.dragDx).toBeCloseTo(80);
    expect(calls.at(-1)).toEqual(['surface-move', 'a', '1']);
    header.call('endTabDrag', local, scope, [0, 0]);
    expect([local.dragTab, local.dragDx, local.draggingTab]).toEqual(['', 0, false]);
  });
  test('small increments accumulate through the threshold and reverse across a midpoint', () => {
    const header = contractComponent('panel.contract', 'PanelHeaderBar'), local = header.state(), calls: unknown[][] = [];
    const scope = { max: Math.max, panel: { tabs: [{ id: 'a' }, { id: 'b' }] },
      frame: (id: string) => ({ x: id.endsWith('a') ? 0 : 64, width: 60 }), local: (...args: unknown[]) => calls.push(args) };
    header.call('beginTabDrag', local, scope, ['a', 20]);
    for (const dx of [2, 2]) header.call('moveTab', local, scope, [dx, 0]);
    expect(calls).toEqual([]);
    for (let step = 0; step < 38; step++) header.call('moveTab', local, scope, [2, 0]);
    expect(calls.at(-1)).toEqual(['surface-move', 'a', '1']);
    for (let step = 0; step < 40; step++) header.call('moveTab', local, scope, [-2, 0]);
    expect(local.dragDx).toBe(0);
    expect(calls.at(-1)).toEqual(['surface-move', 'a', '0']);
  });
  test('a second gesture resets displacement and its threshold for either direction', () => {
    const header = contractComponent('panel.contract', 'PanelHeaderBar'), local = header.state(), calls: unknown[][] = [];
    const scope = { max: Math.max, panel: { tabs: [{ id: 'a' }, { id: 'b' }] },
      frame: (id: string) => ({ x: id.endsWith('a') ? 0 : 64, width: 60 }), local: (...args: unknown[]) => calls.push(args) };
    header.call('beginTabDrag', local, scope, ['a', 20]);
    header.call('moveTab', local, scope, [100, 0]);
    header.call('endTabDrag', local, scope, [0, 0]);
    header.call('beginTabDrag', local, scope, ['b', 94]); calls.length = 0;
    header.call('moveTab', local, scope, [-4, 0]); expect(calls).toEqual([]);
    for (let step = 0; step < 38; step++) header.call('moveTab', local, scope, [-2, 0]);
    expect(local.dragDx).toBe(-80);
    expect(calls.at(-1)).toEqual(['surface-move', 'b', '0']);
    header.call('beginTabDrag', local, scope, ['a', 20]); calls.length = 0;
    header.call('moveTab', local, scope, [4, 0]);
    expect([local.dragDx, local.draggingTab, calls.length]).toEqual([4, false, 0]);
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

describe('window actions keep close-history effects separate from selection', () => {
  const root = contractComponent('app.contract', 'T3Code');
  test('selection and automatic clearing update only selection; explicit close records the original target once', () => {
    const slots = { prSelected: 'original', prPanelOpen: false, prTab: 'code' };
    root.call('prSelect', slots, {}, ['next']); expect(slots).toEqual({ prSelected: 'next', prPanelOpen: true, prTab: 'summary' });
    root.call('prSelect', slots, {}, ['']); expect(slots.prSelected).toBe('');
    const calls: unknown[][] = [], selects: unknown[] = [];
    slots.prSelected = 'closed-target';
    root.call('prClose', slots, { command: (...args: unknown[]) => calls.push(args), prSelect: (ref: unknown) => selects.push(ref) }, []);
    expect(calls).toEqual([['shelllocal:surface-remember-pr', 'closed-target', '', 0]]); expect(selects).toEqual(['']);
    slots.prSelected = ''; calls.length = 0;
    root.call('prClose', slots, { command: (...args: unknown[]) => calls.push(args), prSelect: () => {} }, []);
    expect(calls).toEqual([]);
  });
  test('reopen and surface choice dispatch one request each', () => {
    for (const [what, id, expected] of [['reopen', '', 'shell:surface-reopen'], ['choose', 'files', 'shelllocal:surface-open'], ['key', 'f', 'shelllocal:surface-open']]) {
      const calls: unknown[][] = [];
      const slots = { rightPanelAt: '', settingsOpen: false, utilityPage: '', draftOwner: '', jumpRequest: 0, commandPending: false };
      root.call('panelUi', slots, { data: { diffOpen: false, threadId: 't1', projectId: 'p1' }, rightPanel: false,
        shell: { panel: { tabs: [], key: 'env:t1', count: 0, open: false }, terminalAvailable: false }, command: (...args: unknown[]) => calls.push(args) }, [what, id]);
      expect(calls).toHaveLength(1); expect(calls[0]![0]).toBe(expected);
    }
  });
});

describe('reopen a file tab on a paired environment', () => {
  // A thread of a non-primary environment, its stream not yet live (`ready` false, as openThread leaves it until the
  // resume marker): the file tab still comes back, at the end and active, and its history entry is consumed.
  function lane(projects = [{ id: 'p1', workspaceRoot: '/repo' }]): T3Client {
    const requests: unknown[] = [];
    return { local: {}, environmentId: 'lane', threadId: 't1', projectId: 'p1', diffOpen: true, ready: false, scopes: ['filesystem:read'], requests,
      shell: { projects, threads: [{ id: 't1', projectId: 'p1' }] }, config: {}, presentation: {},
      get draftKey() { return `lane:${this.threadId}`; },
      restAccess: () => ({ request: async (method: string, payload: unknown) => { requests.push([method, payload]); return { entries: [], contents: '' }; }, call: async () => ({}) }) } as unknown as T3Client;
  }
  test('⌘⇧T reopens README.md at the end of the strip, active, and consumes its entry', async () => {
    const app = lane(), panel = panelState(app);
    Object.assign(panel, { surfaces: [{ id: 'diff', kind: 'diff', path: '', line: 0, reveal: 0 }, file('README.md')], active: 'file:README.md', visible: true });
    await surfaceLocal(app, native, 'close', 'file:README.md', '');
    expect(panel.surfaces.map(tab => tab.id)).toEqual(['diff']);
    expect(closedViews(app).map(view => view.key)).toEqual(['lane:t1']);
    expect(await surfaceCommand(app, native, {} as never, 'reopen', '', '')).toBe('');
    expect(panel.surfaces.map(tab => tab.id)).toEqual(['diff', 'file:README.md']);
    expect([panel.active, panel.visible, app.diffOpen]).toEqual(['file:README.md', true, false]);
    expect(closedViews(app)).toEqual([]);
  });
  test('with no project the entry is deferred, not reopened and not an error', async () => {
    const app = lane([]), panel = panelState(app);
    Object.assign(panel, { surfaces: [], active: '', visible: false });
    rememberClosedView(app, entry('old.ts', 'lane:t1')); rememberClosedView(app, entry('a.ts', 'lane:t1'));
    expect(await surfaceCommand(app, native, {} as never, 'reopen', '', '')).toBe('');
    expect(panel.surfaces).toEqual([]);
    expect(closedViews(app).map(view => view.id)).toEqual(['old.ts', 'a.ts']);
  });
});

describe('"Add panel surface" while the panel has no tab', () => {
  // RightPanelTabs draws the "+" whenever the panel is open (`props.open !== false`), so the launcher's bar has it too.
  test('the empty panel bar has panel-add-surface at its left, before its controls', async () => {
    const source = await Bun.file(appPath('shell-panels.contract')).text();
    const launcher = source.slice(source.indexOf('testId="panel-tabbar"'), source.indexOf('SurfaceLauncher(surfaces='));
    expect(launcher.indexOf('AddSurfaceMenu(')).toBeGreaterThan(0);
    expect(launcher.indexOf('AddSurfaceMenu(')).toBeLessThan(launcher.indexOf('testId="maximize-right-panel"'));
    const header = await Bun.file(appPath('panel.contract')).text();
    expect(header).not.toContain('if panel.count > 0');
  });
  test('⌘T with the launcher open and no tab opens add-surface-menu at its first available row', () => {
    const done: unknown[][] = [];
    const slots = { rightPanelAt: 'env:t1', settingsOpen: false, utilityPage: '', draftOwner: '', jumpRequest: 0, commandPending: false };
    root().call('panelUi', slots, { data: { diffOpen: false, threadId: 't1', projectId: 'p1' }, rightPanel: true,
      shell: { panel: { tabs: [], key: 'env:t1', count: 0, open: false }, terminalAvailable: true, surfaces: [{ id: 'browser', available: true }] },
      showPopover: (...args: unknown[]) => done.push(['showPopover', ...args]), focus: (...args: unknown[]) => done.push(['focus', ...args]),
      command: (...args: unknown[]) => done.push(['command', ...args]) }, ['new', '']);
    expect(done).toEqual([['showPopover', 'add-surface-menu', { source: 'panel-add-surface' }], ['focus', 'add-surface-browser']]);
  });
});
