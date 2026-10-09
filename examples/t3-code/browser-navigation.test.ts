// browser-surface part 2, the clone's rows for the data module's half (browser-navigation.ts), each named after the
// reference code it follows (T3 Code 1e2ecbd975, MIT, see LICENSE-T3): PreviewEmptyState / PreviewRecentUrlCard /
// PreviewLocalServerCard, PreviewView handleSubmitUrl / handleOpenServerUrl / the title effect, PreviewMoreMenu's zoom
// and appearance, handleToggleDeviceToolbar, BrowserDeviceToolbar, useBrowserViewportResize, ZoomIndicator, the
// `previewFocus` keybindings and ChatView `togglePreviewPanel` (⇧⌘J). The module and the server are fakes. Ported:
// PreviewEmptyState.test.tsx (4, under their own names), over the projection the Contract view draws (a group shows
// when its list is not empty; both empty draw "No preview yet").
import { describe, expect, it } from 'bun:test';
import { browserHost, browserLocal, browserView } from './browser-surface';
import { navigationPrepare, previewKeys, previewToggleRow, showPreview } from './browser-navigation';
import { browserHistory } from './browser-history';
import { discoveredServersEvent } from './browser-targets';
import { previewRuntimeTabId, type PreviewSessionSnapshot, type PreviewViewportSetting } from './browser-state';
import { openBrowserIn } from './browser-surface';
import type { PanelState } from './r4-surfaces-panel';
import type { T3Client } from './client';
import type { Native } from './protocol';
import type { Obj } from './domain';
import { toasts } from './toast';

const ref = { environmentId: 'local', threadId: 'thread-1' };
const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;
let clock = 0; // the server's answers are newer each time (updatedAt orders them)
const snap = (viewport?: PreviewViewportSetting, tabId = 'tab-1', nav: PreviewSessionSnapshot['navStatus'] = { _tag: 'Success', url: 'http://localhost:5173/', title: 'Vite' }): PreviewSessionSnapshot => ({
  threadId: 'thread-1', tabId, navStatus: nav, canGoBack: false, canGoForward: false, updatedAt: new Date(Date.UTC(2026, 9, 9) + ++clock * 1000).toISOString(), ...(viewport ? { viewport } : {}),
});
type Call = { method: string; payload: Obj };

/** A client on thread-1 of project p1 with one Browser tab, a fake server (`rpc`) and a fake module (`raw`). */
function setup(options: { viewport?: PreviewViewportSetting; zoom?: number; resize?: (payload: Obj) => Obj; nav?: PreviewSessionSnapshot['navStatus']; origin?: string } = {}) {
  const calls: Call[] = [], native: Obj[] = [], requests: Obj[] = [];
  let serial = 0;
  const client = {
    environmentId: 'local', threadId: 'thread-1', projectId: 'p1', generation: 1, connection: 'connected', ready: true, diffOpen: false, origin: options.origin ?? 'http://127.0.0.1:3773',
    draftKey: 'local:thread-1', presentation: {} as Obj, config: { environment: { capabilities: {} }, keybindings: [] as Obj[] },
    shell: { threads: [{ id: 'thread-1', projectId: 'p1' }], projects: [{ id: 'p1', workspaceRoot: '/p', scripts: [{ previewUrl: 'http://localhost:8080/docs' }] }] },
    local: { clientSettings: {}, deviceSettings: {}, composerControls: false, groupingMode: 'separate' } as Obj,
    async rpc(_native: Native, method: string, payload: Obj) {
      calls.push({ method, payload });
      if (method === 'preview.resize') return options.resize ? options.resize(payload) : snap(payload.viewport as PreviewViewportSetting);
      return {};
    },
    async raw(_native: Native, request: Obj) { native.push(request); return { ok: true, generation: 0, value: {} }; },
    restAccess: () => ({ call: async (request: Obj) => { requests.push(request); return request.op === 'subscribe' ? { id: `sub-${++serial}` } : {}; } }),
  } as unknown as T3Client;
  const host = browserHost(client);
  host.store.reconcileServerSessions(ref, { sessions: [snap(options.viewport, 'tab-1', options.nav)], serverEpoch: 'epoch-1', revision: 1 });
  const runtimeId = previewRuntimeTabId(ref, 'epoch-1', 'tab-1');
  (client.presentation as Obj).browserTabs = { [runtimeId]: { kind: options.nav?._tag === 'Idle' ? 'Idle' : 'Success', url: 'http://localhost:5173/', title: 'Vite', zoomFactor: options.zoom ?? 1, colorScheme: 'system' } };
  const state: PanelState = { surfaces: [], active: '', visible: false, userRevision: 0 };
  const surface = openBrowserIn(state, 'tab-1', 'local:thread-1');
  return { client, calls, native, requests, state, surface, runtimeId };
}
const local = (s: ReturnType<typeof setup>, op: string, value = '') => browserLocal(s.client, module, s.state, op, 'tab-1', value);
const sets = (s: ReturnType<typeof setup>) => s.native.filter(request => request.op === 'browserSet').map(({ zoom, zoomStep, colorScheme }) =>
  ({ ...(zoom === undefined ? {} : { zoom }), ...(zoomStep === undefined ? {} : { zoomStep }), ...(colorScheme === undefined ? {} : { colorScheme }) }));
const resizes = (s: ReturnType<typeof setup>) => s.calls.filter(call => call.method === 'preview.resize').map(call => call.payload.viewport);

describe('PreviewMoreMenu zoom and appearance', () => {
  it('steps the zoom ladder in the module (from the page\'s own zoom) and resets it', async () => {
    const s = setup({ zoom: 1.25 });
    await local(s, 'zoom-in');
    await local(s, 'zoom-out');
    await local(s, 'zoom-reset');
    expect(sets(s)).toEqual([{ zoomStep: 'in' }, { zoomStep: 'out' }, { zoom: 1 }]);
    expect(browserView(s.client, s.surface, Date.now())).toMatchObject({ zoomFactor: 1.25, zoomLabel: '125%' });
  });
  it('sends System, Light or Dark and refuses anything else', async () => {
    const s = setup();
    await local(s, 'color-scheme', 'dark');
    await local(s, 'color-scheme', 'sepia');
    expect(sets(s)).toEqual([{ colorScheme: 'dark' }]);
  });
});

describe('ZoomIndicator', () => {
  it('shows on a change of the zoom, not for the first value', () => {
    const s = setup({ zoom: 1 });
    expect(browserView(s.client, s.surface, Date.now()).zoomSerial).toBe(0);
    expect(browserView(s.client, s.surface, Date.now()).zoomSerial).toBe(0);
    ((s.client.presentation as Obj).browserTabs as Obj)[s.runtimeId] = { kind: 'Success', url: 'http://localhost:5173/', zoomFactor: 1.5 };
    expect(browserView(s.client, s.surface, Date.now())).toMatchObject({ zoomSerial: 1, zoomLabel: '150%' });
  });
});

describe('handleToggleDeviceToolbar and BrowserDeviceToolbar', () => {
  it('opens at the panel\'s framed area and closes back to fill', async () => {
    const s = setup();
    await local(s, 'device-toolbar', '600|700');
    expect(resizes(s)).toEqual([{ _tag: 'freeform', width: 580, height: 658 }]);
    expect(browserView(s.client, s.surface, Date.now())).toMatchObject({ viewportMode: 'freeform', viewportWidth: 580, viewportHeight: 658, presetLabel: 'Responsive', viewportSerial: 1 });
    await local(s, 'device-toolbar', '600|700');
    expect(resizes(s).at(-1)).toEqual({ _tag: 'fill' });
  });
  it('chooses a preset, types a size, rotates, locks the ratio and closes', async () => {
    const s = setup({ viewport: { _tag: 'freeform', width: 800, height: 600 } });
    await local(s, 'viewport-preset', 'iphone-12-pro');
    expect(browserView(s.client, s.surface, Date.now())).toMatchObject({ viewportMode: 'preset', presetId: 'iphone-12-pro', presetLabel: 'iPhone 12 Pro', viewportWidth: 390, viewportHeight: 844 });
    await local(s, 'viewport-rotate');
    expect(resizes(s).at(-1)).toEqual({ _tag: 'preset', presetId: 'iphone-12-pro', width: 844, height: 390 });
    await local(s, 'viewport-preset', 'responsive');
    expect(resizes(s).at(-1)).toEqual({ _tag: 'freeform', width: 844, height: 390 });
    const before = resizes(s).length;
    await local(s, 'viewport-size', '239|600');
    await local(s, 'viewport-size', '3840|2161');
    await local(s, 'viewport-size', '844|390');
    expect(resizes(s)).toHaveLength(before);
    await local(s, 'viewport-size', '1024|768');
    expect(resizes(s).at(-1)).toEqual({ _tag: 'freeform', width: 1024, height: 768 });
    await local(s, 'viewport-rotate', '700|500');
    expect(resizes(s).at(-1)).toEqual({ _tag: 'freeform', width: 500, height: 700 });
    await local(s, 'viewport-lock');
    expect(browserView(s.client, s.surface, Date.now()).aspectLocked).toBe(true);
    await local(s, 'viewport-preset', 'ipad-mini');
    expect(browserView(s.client, s.surface, Date.now()).aspectLocked).toBe(true);
    await local(s, 'viewport-close');
    expect(resizes(s).at(-1)).toEqual({ _tag: 'fill' });
    expect(browserView(s.client, s.surface, Date.now()).aspectLocked).toBe(false);
  });
  it('a failed resize toasts and lets the view drop the size it held', async () => {
    const s = setup({ viewport: { _tag: 'freeform', width: 800, height: 600 }, resize: () => { throw new Error('resize refused'); } });
    await local(s, 'viewport-size', '900|700');
    const view = browserView(s.client, s.surface, Date.now());
    expect(view).toMatchObject({ viewportWidth: 800, viewportSerial: 1 });
    expect(toasts(s.client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Unable to resize browser viewport', 'resize refused']]);
  });
  it('a drag or key that changes nothing still lets the view drop the size it drew', async () => {
    const s = setup({ viewport: { _tag: 'freeform', width: 240, height: 600 } });
    await local(s, 'viewport-drag', 'west|200|0|1220|842'); // already at the 240 floor
    await local(s, 'viewport-key', 'east|ArrowLeft|false');
    await local(s, 'viewport-size', '240|600'); // the size it already has
    await local(s, 'viewport-size', '100|600'); // out of bounds
    expect(resizes(s)).toEqual([]);
    expect(browserView(s.client, s.surface, Date.now()).viewportSerial).toBe(4);
    const fill = setup();
    await local(fill, 'viewport-size', '800|600'); // the toolbar's view was older than a fill answer
    expect(resizes(fill)).toEqual([]);
    expect(browserView(fill.client, fill.surface, Date.now()).viewportSerial).toBe(1);
  });
});

describe('useBrowserViewportResize', () => {
  it('commits a rail drag from where the press began, and an arrow key by 10 (50 with Shift)', async () => {
    const s = setup({ viewport: { _tag: 'freeform', width: 800, height: 600 } });
    await local(s, 'viewport-drag', 'east|300|0|1220|842');
    expect(resizes(s).at(-1)).toEqual({ _tag: 'freeform', width: 1300, height: 600 });
    const t = setup({ viewport: { _tag: 'freeform', width: 800, height: 600 } });
    await local(t, 'viewport-key', 'east|ArrowRight|true');
    await local(t, 'viewport-key', 'south|ArrowLeft|false'); // the bottom rail controls only the height
    expect(resizes(t)).toEqual([{ _tag: 'freeform', width: 850, height: 600 }]);
  });
  it('keeps the locked ratio on a drag', async () => {
    const s = setup({ viewport: { _tag: 'freeform', width: 800, height: 600 } });
    await local(s, 'viewport-lock');
    await local(s, 'viewport-drag', 'east|200|0|5000|5000'); // centred: the far rail moves too, so 200 pt grow it by 400
    expect(resizes(s).at(-1)).toEqual({ _tag: 'freeform', width: 1200, height: 900 });
  });
});

describe('PreviewEmptyState', () => {
  it('lists at most eight recent visits and the live local servers, configured ones first', async () => {
    const s = setup({ nav: { _tag: 'Idle' } });
    const history = browserHistory(s.client.local);
    await navigationPrepare(s.client, module, { ...s.state, visible: true });
    for (let i = 9; i >= 0; i--) history.recordVisitForThread(ref, `http://localhost:${3000 + i}/`, Date.now() - i * 120_000);
    history.setTitleForThreadUrl(ref, 'http://localhost:3000/', 'Home');
    discoveredServersEvent(s.client, { subscriptionId: 'sub-1', value: { servers: [
      { host: 'localhost', port: 9000, url: 'http://localhost:9000/', processName: null, pid: null, terminal: null },
      { host: 'localhost', port: 8080, url: 'http://localhost:8080/', processName: 'vite', pid: 7, terminal: null },
    ] } });
    const view = browserView(s.client, s.surface, Date.now());
    expect(view.empty).toBe(true);
    expect(view.recents).toHaveLength(8);
    expect(view.recents[0]).toMatchObject({ url: 'http://localhost:3000/', title: 'Home', subtitle: 'localhost:3000 · just now', removeLabel: 'Remove localhost:3000 from history' });
    expect(view.recents[1]).toMatchObject({ title: 'localhost:3001', subtitle: '2m ago' });
    expect(view.servers.map(server => [server.title, server.description])).toEqual([['vite', 'localhost:8080'], ['Listening', 'localhost:9000']]);
    expect(s.requests[0]).toMatchObject({ op: 'subscribe', method: 'subscribeDiscoveredLocalServers', payload: { configuredUrls: ['http://localhost:8080/docs'] } });
  });
  it('lets the stream go once the tab leaves its empty state', async () => {
    const s = setup({ nav: { _tag: 'Idle' } });
    await navigationPrepare(s.client, module, { ...s.state, visible: true });
    ((s.client.presentation as Obj).browserTabs as Obj)[s.runtimeId] = { kind: 'Success', url: 'http://localhost:5173/', title: 'Vite' };
    await navigationPrepare(s.client, module, { ...s.state, visible: true });
    expect(s.requests.map(request => request.op)).toEqual(['subscribe', 'unsubscribe']);
  });
});

describe('PreviewView handleSubmitUrl and handleOpenServerUrl', () => {
  it('records a typed address and titles it once it settles', async () => {
    const s = setup();
    await navigationPrepare(s.client, module, s.state); // registers the thread's project
    await local(s, 'navigate', 'localhost:5173');
    await navigationPrepare(s.client, module, s.state);
    expect(browserHistory(s.client.local).threadRecentHistory(ref, 8)).toEqual([expect.objectContaining({ url: 'http://localhost:5173/', title: 'Vite' })]);
  });
  it('opens a local server at the environment\'s host and keeps the address it was found at', async () => {
    const s = setup({ origin: 'http://192.168.1.25:3773' });
    await navigationPrepare(s.client, module, s.state);
    await local(s, 'open-url', 'http://localhost:5173/app');
    expect(s.native.find(request => request.op === 'browserNavigate')).toMatchObject({ url: 'http://192.168.1.25:5173/app' });
    expect(browserHistory(s.client.local).threadRecentHistory(ref, 8).map(entry => entry.url)).toEqual(['http://localhost:5173/app']);
    await local(s, 'remove-recent', 'http://localhost:5173/app');
    expect(browserHistory(s.client.local).threadRecentHistory(ref, 8)).toEqual([]);
  });
});

describe('the preview keys and ⇧⌘J', () => {
  it('answers the previewFocus chords of the resolved keybindings', () => {
    const s = setup();
    const focus = { type: 'identifier', name: 'previewFocus' };
    (s.client.config as Obj).keybindings = [
      { command: 'preview.refresh', shortcut: { key: 'r', modKey: true }, whenAst: focus },
      { command: 'preview.focusUrl', shortcut: { key: 'l', modKey: true }, whenAst: focus },
      { command: 'preview.zoomIn', shortcut: { key: '=', modKey: true }, whenAst: focus },
      { command: 'preview.zoomIn', shortcut: { key: '+', modKey: true }, whenAst: focus },
      { command: 'preview.zoomOut', shortcut: { key: '-', modKey: true }, whenAst: focus },
      { command: 'preview.resetZoom', shortcut: { key: '0', modKey: true }, whenAst: focus },
      { command: 'preview.toggle', shortcut: { key: 'j', modKey: true, shiftKey: true } },
    ];
    expect(previewKeys(s.client).split(' ').sort()).toEqual(['Meta+-=zoom-out', 'Meta+0=reset-zoom', 'Meta+==zoom-in', 'Meta+Plus=zoom-in', 'Meta+l=focus-url', 'Meta+r=refresh']);
    expect(browserView(s.client, s.surface, Date.now()).keys).toBe(previewKeys(s.client));
  });
  it('closes a shown Browser panel, else shows the thread\'s tab without opening another', async () => {
    const s = setup();
    const rows: string[][] = [];
    const add = (command: string, kind: string, target: string) => rows.push([command, kind, target]);
    const { panelState } = await import('./r4-surfaces-panel');
    const shown = panelState(s.client);
    shown.surfaces = s.state.surfaces; shown.active = s.state.active; shown.visible = true;
    previewToggleRow(add, s.client);
    shown.visible = false;
    previewToggleRow(add, s.client);
    expect(rows).toEqual([['preview.toggle', 'panel-close', ''], ['preview.toggle', 'command', 'shelllocal:surface-browser-show']]);
    await showPreview(s.client, module, shown);
    expect(shown).toMatchObject({ visible: true, active: 'browser:tab-1' });
    // openBrowser: the thread's active session shows even when the panel has no surface for it.
    const bare: PanelState = { surfaces: [], active: '', visible: false, userRevision: 0 };
    await showPreview(s.client, module, bare);
    expect(bare).toMatchObject({ visible: true, active: 'browser:tab-1' });
    expect(s.calls.filter(call => call.method === 'preview.open')).toEqual([]);
  });
});

describe('PreviewEmptyState', () => {
  /** The thread's recent entries as given, and the live servers; the projection of an idle tab. */
  function render(recentEntries: Array<{ url: string; lastVisitedAt: number; title?: string }>, ports: number[]) {
    const s = setup({ nav: { _tag: 'Idle' } });
    const history = browserHistory(s.client.local);
    history.registerThreadProject(ref, 'proj');
    history.state = { ...history.state, byProjectKey: { proj: recentEntries } };
    discoveredServersEvent(s.client, { subscriptionId: 'sub-1', value: { servers: ports.map(port => ({ host: 'localhost', port, url: `http://localhost:${port}`, processName: 'node', pid: 1, terminal: null })) } });
    return browserView(s.client, s.surface, Date.now());
  }
  const text = (view: ReturnType<typeof render>) => JSON.stringify([view.recents, view.servers]);
  it('renders a history entry in both groups when its host:port matches a live server', () => {
    const view = render([{ url: 'https://myapp.test/admin#users', lastVisitedAt: Date.now(), title: 'Admin' }, { url: 'http://localhost:5173/', lastVisitedAt: Date.now(), title: 'Recent Local' }], [5173]);
    expect(view.recents.length).toBeGreaterThan(0);
    expect(view.servers.length).toBeGreaterThan(0);
    expect(text(view)).toContain('myapp.test/admin#users');
    expect(text(view)).toContain('Admin');
    expect(text(view)).toContain('Recent Local');
    expect(text(view)).toContain('node');
  });
  it('renders only the recents group when no servers are found', () => {
    const view = render([{ url: 'https://myapp.test/', lastVisitedAt: 0 }], []);
    expect(view.recents).toHaveLength(1);
    expect(view.servers).toEqual([]);
  });
  it('keeps the original empty state when both groups are empty', () => {
    const view = render([], []);
    expect(view).toMatchObject({ empty: true, recents: [], servers: [] });
  });
  it('renders an out-of-range lastVisitedAt entry without throwing', () => {
    let view: ReturnType<typeof render> | null = null;
    expect(() => { view = render([{ url: 'https://myapp.test/', lastVisitedAt: 1e20 }], []); }).not.toThrow();
    expect(text(view!)).toContain('myapp.test');
    expect(text(view!)).toContain('Remove');
  });
});
