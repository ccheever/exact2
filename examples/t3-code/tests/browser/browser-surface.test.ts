import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names:
// apps/web/src/components/preview/openPreviewSession.test.ts (4 of 4, with the unread-settings retry's
// "session" row; its "link" row is browser-links.ts's openUrlInPreview), addBrowserSurface.test.ts (2) and closePreviewSession.test.ts (2). The
// reference's atom commands are the clone's `client.rpc`; its rightPanelStore is the panel state
// (panel-surfaces.ts). Then the clone's own rows, each named after the reference code it
// follows: rightPanelStore.reconcileBrowserSurfaces, RightPanelTabs surfaceTitle / PreviewFavicon,
// usePreviewBridge projectDesktopState / buildReportInput, ElectronBrowserHost's live set, PreviewView's
// chrome state and handleSubmitUrl, the profile rules of RightPanelTabs and PreviewView, and the panel
// projection through `panelView` (desktopTabLifetime's "a tab outlives a hidden panel and a thread switch":
// the web view stays while the session does, and only a close drops it). At 9c161462c9: previewRuntime.test.ts
// `alternatePreviewRuntime` (1 of 2; its "no move outside the desktop app" row has no counterpart in this desktop app)
// and PreviewView handleMoveTab through the More menu's move row.
import { describe, expect, it } from 'bun:test';
import {
  DEFAULT_BROWSER_PROFILE_ID, addBrowserSurface, browserHost, browserLocal, browserTab, browserTabFavicon, browserTabTitle, browserView,
  revealServerSessions, effectiveNav, buildReportInput, closePreviewSession, installBrowserCleanup, listPreviewSessions, liveSessions, openBrowserIn, openPreviewSession, projectDesktopState, browserPrepare,
  reconcileBrowserSurfaces, syncNativeSessions, alternatePreviewRuntime, type NativeTab, type Rpc,
} from '../../src/browser/browser-surface';
import { toasts } from '../../src/shell/toast';
import { adoptFrameEvent, frameCommandDone, frameCommandView, frameIdOf, frames, partitionOf } from '../../src/browser/browser-frames';
import { PreviewStateStore, previewRuntimeTabId, type PreviewSessionSnapshot } from '../../src/browser/browser-state';
import { BrowserSettingsReadError, resolveBrowserDefaults } from '../../src/browser/browser-profiles';
import { panelState, panelView, surfaceLocal, surfaceStore, panelKey, type PanelState } from '../../src/panel/panel-surfaces';
import type { T3Client } from '../../src/client/client';
import { floatingTabOf } from '../../src/browser/browser-capture';
import { miniStoreOf, deviceThreadId } from '../../src/device/device-session';
import { ClientError, type Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { serverStreamSource, serverStreamInput, serverStreamFrame } from '../../src/browser/server-browser-runtime';
import type { SocketEvent, SocketHandle } from '../../src/transport/transport-session';
import { adoptAppFiles } from '../../src/media/save-file';
import { externalOpensForTests, resetExternalOpensForTests } from '../../src/client/external-open';

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
    scopes: AUTH_STANDARD_CLIENT_SCOPES,
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
/** `surface-<op>` on a given panel state (the client's own panel for its thread). */
async function surfaceLocalIn(client: T3Client, state: PanelState, op: string, id: string) {
  surfaceStore(client).panels.set(panelKey(client), state);
  return surfaceLocal(client, module, op, id, '');
}
const nativeTab = (overrides: Partial<NativeTab> = {}): NativeTab => ({ kind: 'Success', url: 'https://example.com/', title: 'Example Domain', code: 0, description: '', canGoBack: false, canGoForward: false, favicon: null, failures: 0, zoomFactor: 1, colorScheme: 'system', ...overrides });

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

  // The reference's it.each over "session" and "link"; the clone's link entry point is browser-links.ts's.
  it('does not open a session with unread settings and uses the saved profile on retry', async () => {
    const store = new PreviewStateStore(), opened: Obj[] = [];
    const settings = { local: { browserProfiles: [{ id: 'work', name: 'Work', kind: 'persistent' }], browserDefaultProfileId: 'work' }, preferencesLoaded: false };
    const rpc: Rpc = async (_method, payload) => { opened.push(payload); return snapshot; };
    const input = { url: 'https://t3.chat/' };
    await expect(openPreviewSession(rpc, store, threadRef, input, () => resolveBrowserDefaults(settings))).rejects.toBeInstanceOf(BrowserSettingsReadError);
    expect(opened).toEqual([]);
    expect(store.read(threadRef).snapshot).toBeNull();
    expect(store.read(threadRef).recentlySeenUrls).toEqual([]);
    settings.preferencesLoaded = true;
    await openPreviewSession(rpc, store, threadRef, input, () => resolveBrowserDefaults(settings));
    expect(opened).toEqual([{ threadId: 'thread-1', url: input.url, viewport: FILL, profileId: 'work' }]);
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
  // d0b81ceb6e rightPanelStore.test.ts: "hides the panel when browser reconciliation removes its last session tab".
  it('hides the panel when reconciliation removes its last tab, and keeps it open on another surface', () => {
    const state = emptyPanel();
    openBrowserIn(state, 'tab-a', 'local:thread-1');
    reconcileBrowserSurfaces(state, [], 'local:thread-1');
    expect([state.visible, state.active, state.surfaces]).toEqual([false, '', []]);
    openBrowserIn(state, 'tab-b', 'local:thread-1');
    state.surfaces.push({ id: 'files', kind: 'files', path: '', line: 0, reveal: 0 });
    state.active = 'files';
    reconcileBrowserSurfaces(state, [], 'local:thread-1');
    expect([state.visible, state.active]).toEqual([true, 'files']);
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

  it('buildReportInput: Idle never reports, (kind, url) repeats collapse, each failure reports once', () => {
    expect(buildReportInput('thread-1', 'tab-1', nativeTab({ kind: 'Idle' }), null)).toBeNull();
    const first = buildReportInput('thread-1', 'tab-1', nativeTab({ kind: 'Loading', title: '' }), null)!;
    expect(first.input).toEqual({ threadId: 'thread-1', tabId: 'tab-1', canGoBack: false, canGoForward: false, navStatus: { _tag: 'Loading', url: 'https://example.com/', title: '' } });
    expect(buildReportInput('thread-1', 'tab-1', nativeTab({ kind: 'Loading' }), first.report)).toBeNull();
    expect(buildReportInput('thread-1', 'tab-1', nativeTab(), first.report)?.input.navStatus).toEqual({ _tag: 'Success', url: 'https://example.com/', title: 'Example Domain' });
    const failed = nativeTab({ kind: 'LoadFailed', url: 'http://localhost:16699/', title: '', code: -1004, description: 'ERR_CONNECTION_REFUSED', failures: 1 });
    const report = buildReportInput('thread-1', 'tab-1', failed, null)!;
    expect(report.input.navStatus).toEqual({ _tag: 'LoadFailed', url: 'http://localhost:16699/', title: '', code: -1004, description: 'ERR_CONNECTION_REFUSED' });
    // The projection runs on every answer: the same failure is not sent again; the next failure of the same URL is.
    expect(buildReportInput('thread-1', 'tab-1', failed, report.report)).toBeNull();
    expect(buildReportInput('thread-1', 'tab-1', { ...failed, failures: 2 }, report.report)).not.toBeNull();
  });
});

describe('the chrome row and the native host', () => {
  const opened = (client: T3Client, state: PanelState, tab: PreviewSessionSnapshot) => {
    browserHost(client).store.reconcileServerSessions(threadRef, { sessions: [tab], serverEpoch: 'epoch-1', revision: 1 });
    return openBrowserIn(state, tab.tabId, 'local:thread-1');
  };

  /** One record the tab's frame sends (browser-frames.contract `record`). */
  const record = (client: T3Client, tabId: string, query: string) => adoptFrameEvent(client, frameIdOf(previewRuntimeTabId(threadRef, 'epoch-1', tabId)), query, 0);
  const queued = (client: T3Client) => frames(client).queue.map(({ kind, id, url, delta, flag }) => ({ kind, id, url, delta, flag }));

  it('an idle tab shows the empty state; its frame\'s records drive the chrome', () => {
    const { client } = fakeClient(() => ({}));
    const state = emptyPanel(), surface = opened(client, state, idle('tab-1'));
    expect(browserView(client, surface)).toMatchObject({ empty: true, live: false, url: '', refreshDisabled: true, hasWebContents: false, profileName: 'Default', showProfile: false });
    syncNativeSessions(client);
    record(client, 'tab-1', 'event=navigate&id=1&destination=https%3A%2F%2Fexample.com%2F&loading=true&back=true');
    expect(browserView(client, surface)).toMatchObject({ empty: false, live: true, loading: true, url: 'https://example.com/', canGoBack: true, refreshDisabled: false, hasWebContents: true });
    record(client, 'tab-1', 'event=navigatesuccess&id=1&url=https%3A%2F%2Fexample.com%2F&title=Example&back=true');
    expect(browserView(client, surface)).toMatchObject({ live: true, loading: false, url: 'https://example.com/' });
    record(client, 'tab-1', 'event=navigate&id=2&destination=http%3A%2F%2Flocalhost%3A16699%2F&loading=true');
    record(client, 'tab-1', 'event=navigateerror&id=2&url=https%3A%2F%2Fexample.com%2F&error=NetworkError&message=' + encodeURIComponent('NSURLErrorDomain -1004: Could not connect to the server.'));
    expect(browserView(client, surface)).toMatchObject({ failed: true, live: false, failHost: 'localhost:16699', failMessage: 'Connection refused', failLabel: 'ERR_CONNECTION_REFUSED' });
    // A cancelled load is no failure (Manager.ts `failed`): the next navigation clears the failure, its abort leaves none.
    record(client, 'tab-1', 'event=navigate&id=3&destination=https%3A%2F%2Fexample.com%2Fb&loading=true');
    record(client, 'tab-1', 'event=navigateerror&id=3&url=https%3A%2F%2Fexample.com%2F&error=AbortError&message=stopLoading');
    expect(browserView(client, surface)).toMatchObject({ failed: false, live: true, loading: false, url: 'https://example.com/' });
  });

  it('a frame that has not reported its title keeps the server\'s title for the same URL', () => {
    const { client } = fakeClient(() => ({}));
    const state = emptyPanel(), surface = opened(client, state, { ...idle('tab-1'), navStatus: { _tag: 'Success', url: 'https://example.com/', title: 'Example Domain' } });
    syncNativeSessions(client);
    record(client, 'tab-1', 'event=navigatesuccess&id=1&url=https%3A%2F%2Fexample.com%2F');
    expect(browserTab(client, surface!)).toMatchObject({ title: 'Example Domain' });
    record(client, 'tab-1', 'event=currententrychange&id=2&url=https%3A%2F%2Fexample.com%2Fother');
    expect(browserTab(client, surface!).title).toBe('example.com');
  });

  it('handleSubmitUrl: a good address loads in the tab\'s frame, other text searches for it; one the rules refuse says so', async () => {
    const { client } = fakeClient(() => ({}));
    const state = emptyPanel();
    opened(client, state, idle('tab-1'));
    syncNativeSessions(client);
    await browserLocal(client, module, state, 'navigate', 'tab-1', 'localhost:16651');
    await browserLocal(client, module, state, 'navigate', 'tab-1', 'ftp://example.com');
    await browserLocal(client, module, state, 'navigate', 'tab-1', '   ');
    await browserLocal(client, module, state, 'navigate', 'tab-1', 'how to center a div');
    const id = frameIdOf(previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1')), search = 'https://duckduckgo.com/?q=how%20to%20center%20a%20div';
    expect(queued(client)).toEqual([
      { kind: 'navigate', id, url: 'http://localhost:16651/', delta: 0, flag: false },
      { kind: 'navigate', id, url: search, delta: 0, flag: false },
    ]);
    expect(browserHost(client).store.read(threadRef).recentlySeenUrls).toEqual([search, 'http://localhost:16651/']);
    // The root runs one command at a time and acknowledges it (app.contract frameCommand).
    expect(frameCommandView(client).url).toBe('http://localhost:16651/');
    frameCommandDone(client, frameCommandView(client).serial);
    expect(frameCommandView(client).url).toBe(search);
    // PreviewView's catch: the unsupported scheme toasts; empty input never submitted.
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Could not open this address', 'Enter an http or https URL.']]);
  });

  it('Back, Forward, Refresh, Hard reload and Open DevTools go to the tab\'s frame; Open in system browser opens its URL', async () => {
    const { client } = fakeClient(() => ({}));
    const state = emptyPanel();
    resetExternalOpensForTests();
    opened(client, state, { ...idle('tab-1'), navStatus: { _tag: 'Success', url: 'https://example.com/', title: 'Example Domain' } });
    syncNativeSessions(client);
    record(client, 'tab-1', 'event=navigatesuccess&id=1&url=https%3A%2F%2Fexample.com%2F&back=true&forward=true');
    for (const op of ['back', 'forward', 'refresh', 'hard-reload', 'devtools', 'external']) await browserLocal(client, module, state, op, 'tab-1', '');
    const id = frameIdOf(previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1'));
    expect(queued(client)).toEqual([
      { kind: 'traverse', id, url: '', delta: -1, flag: false }, { kind: 'traverse', id, url: '', delta: 1, flag: false },
      { kind: 'reload', id, url: '', delta: 0, flag: false }, { kind: 'reload', id, url: '', delta: 0, flag: true },
      { kind: 'devtools', id, url: '', delta: 0, flag: false },
    ]);
    expect(externalOpensForTests()).toEqual(['https://example.com/']);
  });

  it('ElectronBrowserHost: one frame per live session of every thread, at its last URL, in its profile\'s partition', () => {
    const { client } = fakeClient(() => ({}));
    const store = browserHost(client).store;
    store.reconcileServerSessions(threadRef, { sessions: [idle('tab-1'), { ...idle('tab-2'), navStatus: { _tag: 'Success', url: 'https://example.com/', title: '' }, profileId: 'incognito' }], serverEpoch: 'epoch-1', revision: 1 });
    store.reconcileServerSessions({ environmentId: 'local', threadId: 'thread-2' }, { sessions: [{ ...idle('tab-1'), threadId: 'thread-2' }], serverEpoch: 'epoch-1', revision: 1 });
    expect(liveSessions(client).map(({ runtimeId, url, profile }) => ({ runtimeId, url, profile }))).toEqual([
      { runtimeId: previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1'), url: '', profile: 'default' },
      { runtimeId: previewRuntimeTabId(threadRef, 'epoch-1', 'tab-2'), url: 'https://example.com/', profile: 'incognito' },
      { runtimeId: previewRuntimeTabId({ environmentId: 'local', threadId: 'thread-2' }, 'epoch-1', 'tab-1'), url: '', profile: 'default' },
    ]);
    syncNativeSessions(client);
    const entries = [...frames(client).entries.values()];
    expect(entries.map(entry => [entry.src, entry.partition])).toEqual([
      ['about:blank', partitionOf('local', 'default')], ['https://example.com/', partitionOf('local', 'incognito')], ['about:blank', partitionOf('local', 'default')],
    ]);
    expect(partitionOf('local', 'incognito')).toMatch(/^t3-incognito-[0-9a-f]{16}$/);
    // A session that ends unmounts its frame.
    store.reconcileServerSessions(threadRef, { sessions: [idle('tab-1')], serverEpoch: 'epoch-1', revision: 2 });
    syncNativeSessions(client);
    expect(frames(client).entries.size).toBe(2);
  });
});

describe('a new frame at the default zoom and appearance (desktopTabLifetime browserDefaultTabState)', () => {
  it('a frame starts at the configured defaults and keeps its own zoom after; unread settings start it at 100% and System', () => {
    const iphone = { _tag: 'preset', presetId: 'iphone-12-pro', width: 390, height: 844 } as const;
    const { client } = fakeClient(() => ({}), { local: { clientSettings: {}, deviceSettings: {}, composerControls: false, browserDefaultViewport: iphone, browserDefaultZoomFactor: 1.25, browserDefaultAppearance: 'dark' }, preferencesLoaded: true });
    const store = browserHost(client).store;
    store.reconcileServerSessions(threadRef, { sessions: [{ ...idle('tab-1'), viewport: iphone }], serverEpoch: 'epoch-1', revision: 1 });
    syncNativeSessions(client);
    const tab1 = previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1');
    expect(frames(client).entries.get(tab1)).toMatchObject({ zoom: 1.25, colorScheme: 'dark' });
    frames(client).entries.get(tab1)!.zoom = 1.5; // a person's ⌘+ in the tab
    syncNativeSessions(client);
    expect(frames(client).entries.get(tab1)!.zoom).toBe(1.5);
    (client as unknown as { preferencesLoaded: boolean }).preferencesLoaded = false;
    store.reconcileServerSessions(threadRef, { sessions: [{ ...idle('tab-1'), viewport: iphone }, idle('tab-2')], serverEpoch: 'epoch-1', revision: 2 });
    syncNativeSessions(client);
    expect(frames(client).entries.get(previewRuntimeTabId(threadRef, 'epoch-1', 'tab-2'))).toMatchObject({ zoom: 1, colorScheme: 'system' });
  });
});

describe('answers that are let go', () => {
  const superseded = () => new ClientError('This operation was superseded.', 'superseded');

  it('closing a tab calls through the closing answer, not the first answer that installed the hook', async () => {
    const { client, calls } = fakeClient(() => ({}));
    const state = emptyPanel();
    browserHost(client).store.reconcileServerSessions(threadRef, { sessions: [idle('tab-1')], serverEpoch: 'epoch-1', revision: 1 });
    openBrowserIn(state, 'tab-1', 'local:thread-1');
    // An early answer installs the hook, then is let go: its native rejects every call.
    const gone = { available: true, watch() {}, later: async () => { throw superseded(); } } as Native;
    const rpc = (client as unknown as { rpc: (n: Native, m: string, p: Obj) => Promise<Obj> }).rpc;
    (client as unknown as { rpc: typeof rpc }).rpc = async (native, method, payload) => { if (native === gone) throw superseded(); return rpc.call(client, native, method, payload); };
    installBrowserCleanup(client, gone);
    // The close comes from a live answer, which installs the hook again before it closes.
    await surfaceLocalIn(client, state, 'close', 'browser:tab-1');
    expect(calls.filter(call => call.method === 'preview.close').map(call => call.payload)).toEqual([{ threadId: 'thread-1', tabId: 'tab-1' }]);
  });

  it('a listing another answer started and lost is done again by the answer that waits for it', async () => {
    let first = true;
    const { client, calls } = fakeClient(method => {
      if (method !== 'preview.list') return idle('tab-1');
      if (first) { first = false; return new Promise<Obj>((_resolve, reject) => setTimeout(() => reject(superseded()), 5)); }
      return { sessions: [], serverEpoch: 'epoch-1', revision: 0 };
    });
    const lost = listPreviewSessions(client, module, threadRef).catch(error => error);
    await addBrowserSurface(client, module, emptyPanel());
    expect((await lost).kind).toBe('superseded');
    expect(calls.map(call => call.method)).toEqual(['preview.list', 'preview.list', 'preview.open']);
    expect(Object.keys(browserHost(client).store.read(threadRef).sessions)).toEqual(['tab-1']);
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

  it('a tab outlives a hidden panel and a thread switch; closing it ends its session and its frame', async () => {
    const { client, calls } = fakeClient(server);
    await surfaceLocal(client, module, 'open', '', 'browser');
    const synced = () => [...frames(client).entries.keys()];
    const tab1 = previewRuntimeTabId(threadRef, 'epoch-1', 'tab-1');
    expect(synced()).toEqual([tab1]);
    expect(panelState(client).surfaces.map(surface => surface.id)).toEqual(['browser:tab-1']);
    // The panel hides and shows: nothing closes.
    await surfaceLocal(client, module, 'hide', '', '');
    await panelView(client, module, 0);
    await surfaceLocal(client, module, 'show', '', '');
    expect((await panelView(client, module, 0)).tabs.map(tab => tab.id)).toEqual(['browser:tab-1']);
    // Another thread: its own panel; the first thread's frame stays.
    (client as unknown as { threadId: string }).threadId = 'thread-2';
    (client as unknown as { draftKey: string }).draftKey = 'local:thread-2';
    expect((await panelView(client, module, 0)).tabs).toEqual([]);
    expect(synced()).toEqual([tab1]);
    (client as unknown as { threadId: string }).threadId = 'thread-1';
    (client as unknown as { draftKey: string }).draftKey = 'local:thread-1';
    const back = await panelView(client, module, 0);
    expect(back.tabs.map(tab => `${tab.id} ${tab.title} ${tab.icon}`)).toEqual(['browser:tab-1 Browser earth']);
    expect(back.browser).toMatchObject({ tabId: 'tab-1', runtimeId: tab1, empty: true });
    // Closing the tab closes its session and unmounts its frame.
    await surfaceLocal(client, module, 'close', 'browser:tab-1', '');
    expect(calls.filter(call => call.method === 'preview.close').map(call => call.payload)).toEqual([{ threadId: 'thread-1', tabId: 'tab-1' }]);
    expect(synced()).toEqual([]);
    expect(panelState(client).surfaces).toEqual([]);
  });
});

it('a server runtime keeps server navigation and never mounts or reports a local frame', () => {
  const { client } = fakeClient(() => ({}));
  const server = { ...snapshot, runtime: 'server' as const };
  browserHost(client).store.applyServerSnapshot(threadRef, server);
  expect(liveSessions(client)).toEqual([]);
  syncNativeSessions(client);
  expect(frames(client).entries.size).toBe(0);
  expect(effectiveNav(client, threadRef, server.tabId)).toMatchObject({ tab: null, nav: server.navStatus });
});

it('a read-only server viewer lists and prepares the stream without desktop effects or input grants', async () => {
  const tab = { ...snapshot, runtime: 'server' as const }, transport: Obj[] = [], removed: string[] = [];
  adoptAppFiles({ fs: { readFile: async () => new ArrayBuffer(0), writeFile: async () => {}, mkdir: async () => {}, rm: async (path: string) => { removed.push(path); } } });
  const { client, calls, native } = fakeClient(() => ({ sessions: [tab], serverEpoch: 'epoch-1', revision: 1 }), {
    scopes: ['orchestration:read'], ids: async () => ['reader-frame-store'],
    call: async (_native: Native, request: Obj) => { transport.push(request); return { wsBase: 'ws://fixture/api/preview-stream', ticket: 'reader-ticket' }; },
  });
  const state = emptyPanel(), surface = openBrowserIn(state, tab.tabId, 'local:thread-1');
  await browserPrepare(client, module, state);
  expect(calls.map(call => call.method)).toEqual(['preview.list']);
  expect(transport).toEqual([{ op: 'previewStreamAccess' }]); expect(native).toEqual([]);
  const view = browserView(client, surface), sent: string[] = [];
  expect(new URL(view.streamUrl).searchParams.get('wsTicket')).toBe('reader-ticket');
  const socket: SocketHandle = { readyState: 1, bufferedAmount: 0, send: value => sent.push(value), close() {} };
  const fetchBefore = globalThis.fetch;
  try {
    globalThis.fetch = ((_url: unknown, options: unknown) => {
      const event = (options as { exactStream: (event: SocketEvent, socket: SocketHandle) => unknown }).exactStream;
      event({ type: 'message', data: JSON.stringify({ type: 'control', canOperate: false, controller: 'you', generation: 1, dialog: null }) }, socket);
      event({ type: 'message', data: new Uint8Array([255, 216, 255, 217]).buffer }, socket);
      return Promise.resolve(new Response());
    }) as typeof fetch;
    serverStreamSource(client, view.runtimeId, view.streamUrl);
  } finally { globalThis.fetch = fetchBefore; }
  expect(serverStreamFrame(client, view.runtimeId)).toMatchObject({ canOperate: false, controlled: false, label: 'Read-only', decoding: true });
  for (const kind of ['takeControl', 'mouse', 'wheel', 'key', 'text']) serverStreamInput(client, view.runtimeId, kind, { key: 'x', text: 'x', buttons: 1, phase: 'down', x: 10, y: 10, width: 100, height: 100 });
  expect(sent).toEqual(['{"type":"ack"}']);
  client.scopes = []; await browserPrepare(client, module, state);
  expect(browserView(client, surface).streamUrl).toBe('');
  expect(removed).toEqual(['app:/tmp/browser-frames/reader-frame-store']);
});

it('server reveals baseline persisted sessions and only reopen a preview for a fresh reveal request', () => {
  const { client } = fakeClient(() => ({})), state = emptyPanel(), store = browserHost(client).store;
  store.applyServerSnapshot(threadRef, { ...snapshot, tabId: 'persisted', runtime: 'server', reveal: true });
  revealServerSessions(client, state, threadRef); expect(floatingTabOf(client)).toBeNull();
  store.applyServerSnapshot(threadRef, { ...snapshot, tabId: 'hidden-agent', runtime: 'server', reveal: false });
  revealServerSessions(client, state, threadRef); expect(floatingTabOf(client)).toBeNull();
  store.applyServerSnapshot(threadRef, { ...snapshot, tabId: 'revealed', runtime: 'server', reveal: true });
  revealServerSessions(client, state, threadRef); expect(floatingTabOf(client)).toBe('revealed');
  miniStoreOf(client).close(deviceThreadId(client)); revealServerSessions(client, state, threadRef); expect(floatingTabOf(client)).toBeNull();
  openBrowserIn(state, 'revealed', 'local:thread-1'); state.visible = false;
  store.applyServerSnapshot(threadRef, { ...snapshot, tabId: 'revealed', runtime: 'server', reveal: true, revealRequest: { id: 'again', force: true } });
  revealServerSessions(client, state, threadRef); expect(state.visible).toBe(true); expect(state.active).toBe('browser:revealed');
});

describe('alternatePreviewRuntime', () => {
  it("moves a remote environment's tab between this computer and the environment", () => {
    expect(alternatePreviewRuntime('remote', 'local', true, {})).toBe('server');
    expect(alternatePreviewRuntime('remote', 'local', true, { runtime: 'server' })).toBe('desktop');
    expect(alternatePreviewRuntime('local', 'local', true, {})).toBeNull();
    expect(alternatePreviewRuntime('remote', 'local', false, {})).toBeNull();
    expect(alternatePreviewRuntime('remote', 'local', true, null)).toBeNull();
  });
});

describe('PreviewView handleMoveTab (the More menu\'s move row)', () => {
  const lane = { config: { environment: { label: 'Lane Second', capabilities: { serverBrowser: true } } } };
  const page: PreviewSessionSnapshot = { ...idle('tab-1'), navStatus: { _tag: 'Success', url: 'http://127.0.0.1:16687/', title: 'Lane Page' }, viewport: { _tag: 'freeform', width: 390, height: 844 }, profileId: 'work' };
  const listed = (client: T3Client, tab: PreviewSessionSnapshot) => {
    browserHost(client).store.reconcileServerSessions(threadRef, { sessions: [tab], serverEpoch: 'epoch-1', revision: 1 });
    const state = emptyPanel(); openBrowserIn(state, tab.tabId, 'local:thread-1'); return state;
  };

  it("offers the environment's browser on a desktop tab and this computer on a server tab, only where the environment runs one", () => {
    const desktop = fakeClient(() => ({}), lane).client, desktopState = listed(desktop, page);
    expect(browserView(desktop, desktopState.surfaces[0]!)).toMatchObject({ moveTarget: 'server', moveLabel: "Open in Lane Second's browser" });
    const server = fakeClient(() => ({}), lane).client, serverState = listed(server, { ...page, runtime: 'server' });
    expect(browserView(server, serverState.surfaces[0]!)).toMatchObject({ moveTarget: 'desktop', moveLabel: 'Open on this computer' });
    const unnamed = fakeClient(() => ({}), { config: { environment: { capabilities: { serverBrowser: true } } } }).client, unnamedState = listed(unnamed, page);
    expect(browserView(unnamed, unnamedState.surfaces[0]!).moveLabel).toBe("Open in the environment's browser");
    const plain = fakeClient(() => ({})).client, plainState = listed(plain, page);
    expect(browserView(plain, plainState.surfaces[0]!)).toMatchObject({ moveTarget: '', moveLabel: '' });
  });

  it('opens the other runtime at the same URL, viewport and profile, shows it, then closes the old tab', async () => {
    const moved: PreviewSessionSnapshot = { ...page, tabId: 'tab-2', runtime: 'server' };
    const { client, calls } = fakeClient(method => method === 'preview.open' ? moved : {}, lane);
    const state = listed(client, page);
    await surfaceLocalIn(client, state, 'browser-move', 'tab-1');
    expect(calls.map(call => [call.method, call.payload])).toEqual([
      ['preview.open', { threadId: 'thread-1', runtime: 'server', url: 'http://127.0.0.1:16687/', viewport: page.viewport, profileId: 'work' }],
      ['preview.close', { threadId: 'thread-1', tabId: 'tab-1' }],
    ]);
    expect(state.surfaces.map(surface => surface.id)).toEqual(['browser:tab-2']);
    expect(state.active).toBe('browser:tab-2');
    expect(Object.keys(browserHost(client).store.read(threadRef).sessions)).toEqual(['tab-2']);
  });

  it('a server tab moves to this computer with no runtime named', async () => {
    const { client, calls } = fakeClient(method => method === 'preview.open' ? { ...page, tabId: 'tab-2' } : {}, lane);
    await surfaceLocalIn(client, listed(client, { ...page, runtime: 'server' }), 'browser-move', 'tab-1');
    expect(calls[0]).toEqual({ method: 'preview.open', payload: { threadId: 'thread-1', url: 'http://127.0.0.1:16687/', viewport: page.viewport, profileId: 'work' } });
  });

  it('a failed open keeps the tab and says why', async () => {
    const { client, calls } = fakeClient(method => { if (method === 'preview.open') throw new ClientError('Browser unavailable.'); return {}; }, lane);
    const state = listed(client, page);
    await surfaceLocalIn(client, state, 'browser-move', 'tab-1');
    expect(calls.map(call => call.method)).toEqual(['preview.open']);
    expect(state.surfaces.map(surface => surface.id)).toEqual(['browser:tab-1']);
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Unable to move the browser tab', 'Browser unavailable.']]);
  });
});

// PreviewMoreMenu: Open DevTools under Hard reload, enabled with the tab's web contents. PreviewView offers it through
// the desktop bridge only, and on a server tab only one Electron renders natively; here a server tab is the server's
// headless page, so the row is a desktop tab's in the macOS app (`devTools`). The root runs the frame command as
// `openDevTools`, the platform's inspector for that frame (LLP 1020.002 D11).
describe('Open DevTools', () => {
  it('is the More menu\'s second row where the view offers it, and opens the frame\'s inspector', async () => {
    const { readFileSync } = await import('node:fs');
    const { appPath } = await import('../support/app-files');
    const menu = readFileSync(appPath('browser-surface.contract'), 'utf8'), root = readFileSync(appPath('shell-members.contract'), 'utf8');
    const rows = [...menu.matchAll(/BsMenuItem\(keysId="browser-more-keys", id="([\w-]+)"/g)].map(match => match[1]);
    expect(rows.slice(0, 3)).toEqual(['browser-more-hard-reload', 'browser-more-devtools', 'browser-more-window']);
    expect(menu).toContain('id="browser-more-devtools", label="Open DevTools", note="", chevron=false, disabled=(not view.hasWebContents), press=local("surface-browser-devtools", view.tabId, ""))');
    expect(menu).toMatch(/if view\.devTools\n\s+BsMenuItem\(keysId="browser-more-keys", id="browser-more-devtools"/);
    expect(menu).toContain('(view.devTools or item.id != "browser-more-devtools")');
    expect(root).toContain('else if c.kind == "devtools"\n      openDevTools(c.id)');
  });

  it('is offered on a desktop tab in the macOS app, not on a server tab or the web', () => {
    const view = (available: boolean, runtime: 'desktop' | 'server') => {
      const { client } = fakeClient(() => ({}));
      client.available = available;
      const state = emptyPanel();
      browserHost(client).store.reconcileServerSessions(threadRef, { sessions: [{ ...idle('tab-1'), runtime }], serverEpoch: 'epoch-1', revision: 1 });
      return browserView(client, openBrowserIn(state, 'tab-1', 'local:thread-1')).devTools;
    };
    expect([view(true, 'desktop'), view(true, 'server'), view(false, 'desktop')]).toEqual([true, false, false]);
  });
});

// The app's runtime freezes the intrinsics, so Error.prototype.name is read-only there and assigning a subclass's own
// `name` throws; the Browser's errors define it instead (an invalid address's toast says "Enter an http or https URL.").
describe('Browser errors under frozen intrinsics', () => {
  it('construct with their names', async () => {
    const { PreviewUrlNormalizationError } = await import('../../src/browser/browser-url');
    const { PreviewCaptureFailure, BrowserRecordingError } = await import('../../src/browser/browser-recording');
    const { TerminalLinkPreviewOpenError } = await import('../../src/browser/browser-links');
    const original = Object.getOwnPropertyDescriptor(Error.prototype, 'name')!;
    Object.defineProperty(Error.prototype, 'name', { ...original, writable: false });
    try {
      expect(new PreviewUrlNormalizationError({ inputLength: 3, reason: 'unsupported-protocol', protocol: 'ftp:' }).name).toBe('PreviewUrlNormalizationError');
      expect(new PreviewCaptureFailure('unavailable' as never, 'capture' as never).name).toBe('PreviewCaptureFailure');
      expect(new BrowserRecordingError('RecordingFailed', 'x').name).toBe('RecordingFailed');
      expect(new TerminalLinkPreviewOpenError(threadRef, 'http://localhost:1', null).name).toBe('TerminalLinkPreviewOpenError');
    } finally { Object.defineProperty(Error.prototype, 'name', original); }
  });
});
