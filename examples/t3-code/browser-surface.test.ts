// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names:
// apps/web/src/components/preview/openPreviewSession.test.ts (3 of 4: the unread-settings retry waits for
// part 2's browser defaults), addBrowserSurface.test.ts (2) and closePreviewSession.test.ts (2). The
// reference's atom commands are the clone's `client.rpc`; its rightPanelStore is the panel state
// (r4-surfaces-panel.ts). Then the clone's own rows for part 1, each named after the reference code it
// follows: rightPanelStore.reconcileBrowserSurfaces, RightPanelTabs surfaceTitle / PreviewFavicon,
// usePreviewBridge projectDesktopState / buildReportInput, ElectronBrowserHost's live set, PreviewView's
// chrome state and handleSubmitUrl, the profile rules of RightPanelTabs and PreviewView, and the panel
// projection through `panelView` (desktopTabLifetime's "a tab outlives a hidden panel and a thread switch":
// the web view stays while the session does, and only a close drops it).
import { describe, expect, it } from 'bun:test';
import {
  BROWSER_PROFILES, DEFAULT_BROWSER_PROFILE_ID, addBrowserSurface, browserHost, browserLocal, browserProfileName, browserTabFavicon, browserTabTitle, browserView,
  buildReportInput, closePreviewSession, launcherOffersProfiles, liveSessions, openBrowserIn, openPreviewSession, projectDesktopState, reconcileBrowserSurfaces, type NativeTab, type Rpc,
} from './browser-surface';
import { PreviewStateStore, previewRuntimeTabId, type PreviewSessionSnapshot } from './browser-state';
import { panelState, panelView, surfaceLocal, type PanelState } from './r4-surfaces-panel';
import type { T3Client } from './client';
import type { Native } from './protocol';
import type { Obj } from './domain';

const threadRef = { environmentId: 'local', threadId: 'thread-1' };
const FILL = { _tag: 'fill' };
const snapshot: PreviewSessionSnapshot = {
  threadId: 'thread-1', tabId: 'tab-1', navStatus: { _tag: 'Loading', url: 'https://t3.chat/', title: '' },
  canGoBack: false, canGoForward: false, updatedAt: '2026-06-11T23:00:00.000Z',
};
const idle = (tabId: string): PreviewSessionSnapshot => ({ threadId: 'thread-1', tabId, navStatus: { _tag: 'Idle' }, canGoBack: false, canGoForward: false, updatedAt: `2026-06-18T19:00:0${tabId.at(-1) ?? '0'}.000Z` });
const emptyPanel = (): PanelState => ({ surfaces: [], active: '', visible: false, userRevision: 0 });

type Call = { method: string; payload: Obj };
/** A client with the pieces the browser surface reads: its thread, `rpc` to a fake server and `raw` to a fake module. */
function fakeClient(answer: (method: string, payload: Obj) => Obj | Promise<Obj>, extra: Obj = {}) {
  const calls: Call[] = [], native: Call[] = [];
  const client = {
    environmentId: 'local', threadId: 'thread-1', projectId: 'p1', generation: 1, connection: 'connected', ready: true, available: true, diffOpen: false,
    draftKey: 'local:thread-1', presentation: {} as Obj, config: { environment: { capabilities: {} } }, shell: { threads: [{ id: 'thread-1', projectId: 'p1' }], projects: [{ id: 'p1', workspaceRoot: '/p' }] },
    local: { clientSettings: {}, deviceSettings: {}, composerControls: false },
    async rpc(_native: Native, method: string, payload: Obj) { calls.push({ method, payload }); return answer(method, payload); },
    async raw(_native: Native, request: Obj) { native.push({ method: String(request.op), payload: request }); return { ok: true, generation: 0, value: {} }; },
    ...extra,
  } as unknown as T3Client;
  return { client, calls, native };
}
const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;
const nativeTab = (overrides: Partial<NativeTab> = {}): NativeTab => ({ kind: 'Success', url: 'https://example.com/', title: 'Example Domain', code: 0, description: '', canGoBack: false, canGoForward: false, favicon: null, ...overrides });

describe('openPreviewSession', () => {
  it('creates an idle tab without recording a recently visited URL', async () => {
    const store = new PreviewStateStore(), idleSnapshot = { ...snapshot, tabId: 'tab-blank', navStatus: { _tag: 'Idle' as const } };
    const opened: Obj[] = [];
    const rpc: Rpc = async (_method, payload) => { opened.push(payload); return idleSnapshot; };
    await openPreviewSession(rpc, store, threadRef);
    expect(opened).toEqual([{ threadId: 'thread-1', viewport: FILL, profileId: DEFAULT_BROWSER_PROFILE_ID }]);
    expect(store.read(threadRef).snapshot).toEqual(idleSnapshot);
    expect(store.read(threadRef).recentlySeenUrls).toEqual([]);
  });

  it('applies the RPC response without waiting for a preview event', async () => {
    const store = new PreviewStateStore(), opened: Obj[] = [];
    await openPreviewSession(async (_method, payload) => { opened.push(payload); return snapshot; }, store, threadRef, { url: 't3.chat' });
    expect(opened).toEqual([{ threadId: 'thread-1', url: 't3.chat', viewport: FILL, profileId: DEFAULT_BROWSER_PROFILE_ID }]);
    expect(store.read(threadRef).snapshot).toEqual(snapshot);
    expect(store.read(threadRef).recentlySeenUrls).toEqual(['https://t3.chat/']);
  });

  it('returns failures without mutating preview state', async () => {
    const store = new PreviewStateStore();
    await expect(openPreviewSession(async () => { throw new Error('preview unavailable'); }, store, threadRef, { url: 't3.chat' })).rejects.toThrow('preview unavailable');
    expect(store.read(threadRef).snapshot).toBeNull();
    expect(store.read(threadRef).recentlySeenUrls).toEqual([]);
  });
});

describe('addBrowserSurface', () => {
  // The clone lists the thread's sessions before it opens one (the tab's native identity names the server's epoch).
  const server = (next: () => PreviewSessionSnapshot, existing: PreviewSessionSnapshot[] = []) => (method: string): Obj =>
    method === 'preview.list' ? { sessions: existing, serverEpoch: 'epoch-1', revision: 0 } : method === 'preview.open' ? next() : {};

  it('opens under the requested profile', async () => {
    const { client, calls } = fakeClient(server(() => idle('tab-1')));
    await addBrowserSurface(client, module, emptyPanel(), 'profile-work');
    expect(calls.filter(call => call.method === 'preview.open').map(call => call.payload)).toEqual([{ threadId: 'thread-1', viewport: FILL, profileId: 'profile-work' }]);
  });

  it('creates another preview session when a browser tab is already active', async () => {
    const first = idle('tab-1'), second = idle('tab-2');
    const { client, calls } = fakeClient(server(() => second, [first]));
    browserHost(client).store.applyServerSnapshot(threadRef, first);
    const state = emptyPanel();
    openBrowserIn(state, first.tabId, 'local:thread-1');
    await addBrowserSurface(client, module, state);
    expect(calls.filter(call => call.method === 'preview.open').map(call => call.payload)).toEqual([{ threadId: 'thread-1', viewport: FILL, profileId: DEFAULT_BROWSER_PROFILE_ID }]);
    expect(Object.keys(browserHost(client).store.read(threadRef).sessions)).toEqual(['tab-1', 'tab-2']);
    expect(state.surfaces.map(surface => surface.id)).toEqual(['browser:tab-1', 'browser:tab-2']);
  });
});

describe('closePreviewSession', () => {
  const loaded: PreviewSessionSnapshot = { ...snapshot, navStatus: { _tag: 'Success', url: 'http://localhost:3000/', title: 'Local app' }, updatedAt: '2026-06-18T19:00:00.000Z' };

  it('suppresses stale server snapshots while the close is in flight', async () => {
    const store = new PreviewStateStore(), closed: Obj[] = [];
    store.applyServerSnapshot(threadRef, loaded);
    let finishClose: (() => void) | undefined;
    const closing = closePreviewSession((_method, payload) => new Promise<Obj>(resolve => { closed.push(payload); finishClose = () => resolve({}); }), store, threadRef, loaded.tabId, loaded);
    expect(store.read(threadRef).sessions).toEqual({});
    store.applyServerSnapshot(threadRef, loaded);
    expect(store.read(threadRef).sessions).toEqual({});
    finishClose?.();
    await closing;
    expect(closed).toEqual([{ threadId: 'thread-1', tabId: 'tab-1' }]);
  });

  it('restores the last snapshot when the server close fails', async () => {
    const store = new PreviewStateStore();
    store.applyServerSnapshot(threadRef, loaded);
    const result = await closePreviewSession(async () => { throw new Error('close failed'); }, store, threadRef, loaded.tabId, loaded);
    expect(result).toBe(false);
    expect(store.read(threadRef).snapshot).toEqual(loaded);
    expect(store.read(threadRef).sessions).toEqual({ [loaded.tabId]: loaded });
  });
});

describe('rightPanelStore browser surfaces', () => {
  it('reconcileBrowserSurfaces: tabs follow the sessions, keeping the others and the active one', () => {
    const state = emptyPanel();
    state.surfaces.push({ id: 'files', kind: 'files', path: '', line: 0, reveal: 0 });
    openBrowserIn(state, 'tab-1', 'local:thread-1');
    openBrowserIn(state, 'tab-2', 'local:thread-1');
    expect(reconcileBrowserSurfaces(state, ['tab-2', 'tab-3'], 'local:thread-1')).toBe(true);
    expect(state.surfaces.map(surface => surface.id)).toEqual(['files', 'browser:tab-2', 'browser:tab-3']);
    expect(state.active).toBe('browser:tab-2');
    expect(reconcileBrowserSurfaces(state, ['tab-2', 'tab-3'], 'local:thread-1')).toBe(false);
    reconcileBrowserSurfaces(state, [], 'local:thread-1');
    expect(state.surfaces.map(surface => surface.id)).toEqual(['files']);
    expect(state.active).toBe('files');
  });
});

describe('RightPanelTabs: a browser tab', () => {
  it('surfaceTitle: the page title, else its host, else "Browser"', () => {
    expect(browserTabTitle({ _tag: 'Idle' })).toBe('Browser');
    expect(browserTabTitle({ _tag: 'Success', url: 'https://example.com/a', title: 'Example Domain' })).toBe('Example Domain');
    expect(browserTabTitle({ _tag: 'Loading', url: 'http://localhost:16651/slow', title: '  ' })).toBe('localhost:16651');
    expect(browserTabTitle({ _tag: 'LoadFailed', url: 'not a url', title: '', code: -1, description: '' })).toBe('Browser');
  });

  it('PreviewFavicon: the captured icon of the same origin, then the public service, then the glyph', () => {
    const favicon = { dataUrl: 'data:image/png;base64,AA==', pageUrl: 'https://example.com', capturedAt: 1 };
    expect(browserTabFavicon({ _tag: 'Success', url: 'https://example.com/a', title: '' }, favicon)).toEqual({ captured: favicon.dataUrl, fallback: 'https://www.google.com/s2/favicons?domain=example.com&sz=32' });
    expect(browserTabFavicon({ _tag: 'Success', url: 'https://other.example.org/', title: '' }, favicon).captured).toBe('');
    // A loopback or private host never goes to the public service (faviconUrlForOrigin's isPublicFaviconHost).
    expect(browserTabFavicon({ _tag: 'Success', url: 'http://localhost:16651/', title: '' }, null)).toEqual({ captured: '', fallback: '' });
    expect(browserTabFavicon({ _tag: 'Idle' }, favicon)).toEqual({ captured: '', fallback: '' });
  });
});

describe('usePreviewBridge', () => {
  it('projectDesktopState keeps a captured favicon only for the page origin', () => {
    const favicon = { dataUrl: 'data:image/png;base64,AA==', pageUrl: 'https://example.com', capturedAt: 1 };
    expect(projectDesktopState(nativeTab({ favicon, canGoBack: true })).favicon).toEqual(favicon);
    expect(projectDesktopState(nativeTab({ favicon, url: 'https://elsewhere.test/' })).favicon).toBeNull();
    expect(projectDesktopState(nativeTab({ kind: 'Loading' }))).toMatchObject({ loading: true, hasWebContents: true, canGoBack: false });
  });

  it('buildReportInput: Idle never reports, (kind, url) repeats collapse, LoadFailed always reports', () => {
    expect(buildReportInput('thread-1', 'tab-1', nativeTab({ kind: 'Idle' }), null)).toBeNull();
    const first = buildReportInput('thread-1', 'tab-1', nativeTab({ kind: 'Loading', title: '' }), null)!;
    expect(first.input).toEqual({ threadId: 'thread-1', tabId: 'tab-1', canGoBack: false, canGoForward: false, navStatus: { _tag: 'Loading', url: 'https://example.com/', title: '' } });
    expect(buildReportInput('thread-1', 'tab-1', nativeTab({ kind: 'Loading' }), first.report)).toBeNull();
    expect(buildReportInput('thread-1', 'tab-1', nativeTab(), first.report)?.input.navStatus).toEqual({ _tag: 'Success', url: 'https://example.com/', title: 'Example Domain' });
    const failed = nativeTab({ kind: 'LoadFailed', url: 'http://localhost:16699/', title: '', code: -1004, description: 'ERR_CONNECTION_REFUSED' });
    const report = buildReportInput('thread-1', 'tab-1', failed, null)!;
    expect(report.input.navStatus).toEqual({ _tag: 'LoadFailed', url: 'http://localhost:16699/', title: '', code: -1004, description: 'ERR_CONNECTION_REFUSED' });
    expect(buildReportInput('thread-1', 'tab-1', failed, report.report)).not.toBeNull();
  });
});

describe('profiles (part 1: the built-in Default)', () => {
  it('names a tab profile, says when it is gone, and offers the launcher chevron only with a choice', () => {
    expect(browserProfileName(BROWSER_PROFILES, 'default')).toBe('Default');
    expect(browserProfileName(BROWSER_PROFILES, 'work')).toBe('Removed profile');
    expect(launcherOffersProfiles(BROWSER_PROFILES)).toBe(false);
    expect(launcherOffersProfiles([...BROWSER_PROFILES, { id: 'incognito', name: 'Incognito' }])).toBe(true);
  });
});

describe('the chrome row and the native host', () => {
  const opened = (client: T3Client, state: PanelState, tab: PreviewSessionSnapshot) => {
    browserHost(client).store.reconcileServerSessions(threadRef, { sessions: [tab], serverEpoch: 'epoch-1', revision: 1 });
    return openBrowserIn(state, tab.tabId, 'local:thread-1');
  };

  it('an idle tab shows the empty state; the module page state drives the chrome', () => {
    const { client } = fakeClient(() => ({}));
    const state = emptyPanel(), surface = opened(client, state, idle('tab-1'));
    expect(browserView(client, surface)).toMatchObject({ empty: true, live: false, url: '', refreshDisabled: true, hasWebContents: false, profileName: 'Default', showProfile: false });
    const runtimeId = previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1');
    (client as unknown as { presentation: Obj }).presentation = { browserTabs: { [runtimeId]: { kind: 'Loading', url: 'https://example.com/', title: '', canGoBack: true } } };
    expect(browserView(client, surface)).toMatchObject({ empty: false, live: true, loading: true, url: 'https://example.com/', canGoBack: true, refreshDisabled: false, hasWebContents: true });
    (client as unknown as { presentation: Obj }).presentation = { browserTabs: { [runtimeId]: { kind: 'LoadFailed', url: 'http://localhost:16699/', title: '', code: -1004, description: 'ERR_CONNECTION_REFUSED' } } };
    expect(browserView(client, surface)).toMatchObject({ failed: true, live: false, failHost: 'localhost:16699', failMessage: 'Connection refused', failLabel: 'ERR_CONNECTION_REFUSED' });
  });

  it('handleSubmitUrl: a good address loads in the tab web view; one the rules refuse does nothing', async () => {
    const { client, native } = fakeClient(() => ({}));
    const state = emptyPanel();
    opened(client, state, idle('tab-1'));
    await browserLocal(client, module, state, 'navigate', 'tab-1', 'localhost:16651');
    await browserLocal(client, module, state, 'navigate', 'tab-1', 'ftp://example.com');
    await browserLocal(client, module, state, 'navigate', 'tab-1', '   ');
    expect(native.map(call => call.payload)).toEqual([{ op: 'browserNavigate', tab: previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1'), url: 'http://localhost:16651/', profile: 'default', environment: 'local' }]);
    expect(browserHost(client).store.read(threadRef).recentlySeenUrls).toEqual(['http://localhost:16651/']);
  });

  it('Back, Forward, Refresh and Hard reload go to the tab web view; Open in system browser opens its URL', async () => {
    const { client, native } = fakeClient(() => ({}));
    const state = emptyPanel();
    opened(client, state, { ...idle('tab-1'), navStatus: { _tag: 'Success', url: 'https://example.com/', title: 'Example Domain' } });
    for (const op of ['back', 'forward', 'refresh', 'hard-reload', 'external']) await browserLocal(client, module, state, op, 'tab-1', '');
    const runtimeId = previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1');
    expect(native.map(call => call.payload)).toEqual([
      { op: 'browserCommand', tab: runtimeId, command: 'back' }, { op: 'browserCommand', tab: runtimeId, command: 'forward' },
      { op: 'browserCommand', tab: runtimeId, command: 'refresh' }, { op: 'browserCommand', tab: runtimeId, command: 'hard-reload' },
      { op: 'remoteEditorsOpen', url: 'https://example.com/' },
    ]);
  });

  it('ElectronBrowserHost: one web view per live session of every thread, at its last URL', () => {
    const { client } = fakeClient(() => ({}));
    const store = browserHost(client).store;
    store.reconcileServerSessions(threadRef, { sessions: [idle('tab-1'), { ...idle('tab-2'), navStatus: { _tag: 'Success', url: 'https://example.com/', title: '' }, profileId: 'default' }], serverEpoch: 'epoch-1', revision: 1 });
    store.reconcileServerSessions({ environmentId: 'local', threadId: 'thread-2' }, { sessions: [{ ...idle('tab-1'), threadId: 'thread-2' }], serverEpoch: 'epoch-1', revision: 1 });
    expect(liveSessions(client)).toEqual([
      { id: previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1'), url: '', profile: 'default', environment: 'local' },
      { id: previewRuntimeTabId(threadRef, 'epoch-1', 'tab-2'), url: 'https://example.com/', profile: 'default', environment: 'local' },
      { id: previewRuntimeTabId({ environmentId: 'local', threadId: 'thread-2' }, 'epoch-1', 'tab-1'), url: '', profile: 'default', environment: 'local' },
    ]);
  });
});

describe('desktopTabLifetime (through the panel)', () => {
  const list = { sessions: [] as PreviewSessionSnapshot[], serverEpoch: 'epoch-1', revision: 0 };
  const server = (method: string, payload: Obj): Obj => {
    if (method === 'preview.list') return { ...list, sessions: list.sessions.filter(session => session.threadId === payload.threadId) };
    if (method === 'preview.open') { const opened = { ...idle(`tab-${list.sessions.length + 1}`), threadId: String(payload.threadId) }; list.sessions.push(opened); list.revision++; return opened; }
    if (method === 'preview.close') { list.sessions = list.sessions.filter(session => session.tabId !== payload.tabId || session.threadId !== payload.threadId); list.revision++; }
    return {};
  };

  it('a tab outlives a hidden panel and a thread switch; closing it ends its session and its web view', async () => {
    const { client, calls, native } = fakeClient(server);
    await surfaceLocal(client, module, 'open', '', 'browser');
    const synced = () => native.filter(call => call.method === 'browserSync').map(call => (call.payload.tabs as Obj[]).map(tab => tab.id));
    const tab1 = previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1');
    expect(synced().at(-1)).toEqual([tab1]);
    expect(panelState(client).surfaces.map(surface => surface.id)).toEqual(['browser:tab-1']);
    // The panel hides and shows: nothing closes.
    await surfaceLocal(client, module, 'hide', '', '');
    await panelView(client, module, 0);
    await surfaceLocal(client, module, 'show', '', '');
    expect((await panelView(client, module, 0)).tabs.map(tab => tab.id)).toEqual(['browser:tab-1']);
    // Another thread: its own panel; the first thread's web view stays.
    (client as unknown as { threadId: string }).threadId = 'thread-2';
    (client as unknown as { draftKey: string }).draftKey = 'local:thread-2';
    expect((await panelView(client, module, 0)).tabs).toEqual([]);
    expect(native.some(call => call.method === 'browserSync' && (call.payload.tabs as Obj[]).length === 0)).toBe(false);
    (client as unknown as { threadId: string }).threadId = 'thread-1';
    (client as unknown as { draftKey: string }).draftKey = 'local:thread-1';
    const back = await panelView(client, module, 0);
    expect(back.tabs.map(tab => `${tab.id} ${tab.title} ${tab.icon}`)).toEqual(['browser:tab-1 Browser earth']);
    expect(back.browser).toMatchObject({ tabId: 'tab-1', runtimeId: tab1, empty: true });
    // Closing the tab closes its session and drops its web view.
    await surfaceLocal(client, module, 'close', 'browser:tab-1', '');
    expect(calls.filter(call => call.method === 'preview.close').map(call => call.payload)).toEqual([{ threadId: 'thread-1', tabId: 'tab-1' }]);
    expect(synced().at(-1)).toEqual([]);
    expect(panelState(client).surfaces).toEqual([]);
  });
});
