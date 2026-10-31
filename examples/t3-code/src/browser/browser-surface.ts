import { can, requirePermission } from '../connections/command-permissions';
// The right panel's Browser tabs over a native WKWebView (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/web/src/components/preview/{addBrowserSurface,openPreviewSession,closePreviewSession,usePreviewBridge,
// PreviewView}.ts(x), apps/web/src/rightPanelStore.ts `openBrowser`/`reconcileBrowserSurfaces`,
// apps/web/src/components/RightPanelTabs.tsx `surfaceTitle`/`SurfaceIcon`, apps/web/src/browser/ElectronBrowserHost.tsx;
// at 9c161462c9, apps/web/src/browser/previewRuntime.ts `alternatePreviewRuntime` and PreviewView `handleMoveTab`).
//
// A tab is a server preview session (`preview.open`, `.close`, `.list`, `.reportStatus`): the server
// issues its id and keeps it across a hidden panel and a thread switch. The page itself is a partitioned
// `iframe` the window's frame layer mounts for every live session, whether or not the panel shows it
// (browser-frames.ts, browser-frames.contract; the reference's ElectronBrowserHost keeps a <webview> per
// session at the app root). This file is the rest of the data module's half: the per-thread session store
// (browser-state.ts), opening and closing tabs, the chrome row's commands, the page state mirrored back to the
// server, and the panel's projection.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { obj, str, type Obj } from '../client/domain';
import { activeRef } from '../terminal/terminal-drawer-view';
import { parseScopedThreadKey, scopedThreadKey, type ScopedThreadRef } from '../terminal/terminal-ui-state';
import { isPublicFaviconHost } from '../connections/host-classification';
import { registerSurfaceClose } from '../panel/right-panel-tabs';
import type { PanelState, Surface } from '../panel/panel-surfaces';
import { letGo } from '../client/let-go';
import { ensureDraftThreadId } from '../composer/draft-thread-id';
import {
  PreviewStateStore, previewRuntimeTabId, readSnapshot, shouldShowPreviewEmptyState,
  type DesktopPreviewOverlay, type PreviewNavStatus, type PreviewSessionSnapshot, type PreviewViewportSetting,
} from './browser-state';
import { describePreviewError, previewErrorLabel, previewHost, previewPdfAssetUrl, resolveAddressBarInput } from './browser-url';
import { emptyViewerFrame, type ViewerFrameView } from '../panel/viewer-frames';
// Annotate, Capture, Float preview, the separate window and the floating player.
import { adoptAgentDownload, adoptCaptureNative, browserAutoShowFloatingPreview, floatingTabOf, toggleFloat, artifactLocal, browserCaptureView, browserMiniLocal, cancelHiddenPicks, captureLocal, emptyCaptureView, type BrowserCaptureView } from './browser-capture';
import { annotationAttachedToDraft } from './browser-annotation';
import { DEFAULT_BROWSER_PROFILE_ID, browserProfileChoices, tabProfile, type BrowserDefaults } from './browser-profiles';
import { addressBarSearchTemplate, resolveBrowserOpenDefaults } from './browser-defaults';
// History, discovery, zoom, appearance, the viewport and the preview keys (browser-navigation.ts).
import { browserKey, focusUrlTaken, emptyNavigationView, navigationLocal, navigationNow, navigationPrepare, navigationView, showPreview, type BrowserNavigationView } from './browser-navigation';
import { recordVisit } from './browser-history-record';
import { preparedConnection, resolveDiscoveredServerUrl } from './browser-targets';
import { fleet } from '../connections/settings-b-fleet';
import { pushToast } from '../shell/toast';
import { miniStoreOf, deviceThreadId } from '../device/device-session';
import { browserMiniPlayerSource } from '../device/previewMiniPlayerStore';
import { primaryEnvironmentId } from '../connections/local-primary';
import { closeServerPictureInPicture, serverPipOf, settleServerPictureInPicture } from './server-browser-pip';
import { drainServerNotices, prepareServerStream, retainServerStreams, serverStreamUrl, serverStreamFrame, sendServerCommand } from './server-browser-runtime';
import { adoptAutomationTabs, automationOverlay, automationPrepare, toggleFrameMute } from './browser-automation';
import { openExternal } from '../client/external-open';
import { adoptFrameEvent, clearFramePartition, commandFrame, frameCommandDone, frameNav, frames, namerLoaded, navigateFrame, syncFrames, type LiveFrame } from './browser-frames';
import { openFrameWindow } from './browser-links';
import { adoptFramePrompt, answerFramePrompt } from './browser-automation';

// ── Profiles (browser-profiles.ts) ─────────────────────────────────────────────────────────
export { DEFAULT_BROWSER_PROFILE_ID };
export type BrowserProfileChoice = { id: string; name: string };
/** browserDefaultOpenViewport without settings (a caller that names no defaults): fill. */
export const DEFAULT_OPEN_VIEWPORT: PreviewViewportSetting = { _tag: 'fill' };

// ── The data module's browser host, one per client ─────────────────────────────────────────────
export type Rpc = (method: string, payload: Obj) => Promise<Obj>;
/** The last report per tab: its kind and URL, and for a failure the module's failure count (one report per failure). */
type Report = { kind: string; url: string; failures: number };
type Host = {
  store: PreviewStateStore;
  serverSeen: Map<string, Map<string, string | undefined>>;
  /** Threads whose sessions were listed on this connection (thread key → client generation). */
  listed: Map<string, number>;
  listing: Map<string, Promise<void>>;
  /** usePreviewBridge's dedupe: the last (kind, url) reported per runtime tab. */
  reported: Map<string, Report>;
  /** Frames reloaded once for the cookies moved into their partition. */
  migrationReloads: Set<string>;
};
const hosts = new WeakMap<T3Client, Host>();
export function browserHost(client: T3Client): Host {
  let host = hosts.get(client);
  if (!host) { host = { serverSeen: new Map(), store: new PreviewStateStore(), listed: new Map(), listing: new Map(), reported: new Map(), migrationReloads: new Set() }; hosts.set(client, host); }
  return host;
}
const rpcOf = (client: T3Client, native: Native): Rpc => (method, payload) => client.rpc(native, method, payload);

// ── Sessions: open, close, list (openPreviewSession, addBrowserSurface, closePreviewSession, usePreviewSession) ──
/** `background`: the answer joins the thread's sessions without becoming its active tab (openUrlInPreview's
 *  `updatePreviewServerSnapshot`). */
export type OpenInput = { url?: string; profileId?: string; viewport?: PreviewViewportSetting; background?: boolean; runtime?: 'desktop' | 'server' };
/** The configured defaults a new tab opens with (browserDefaults.ts: its profile and viewport). */
export type OpenDefaults = () => Pick<BrowserDefaults, 'profileId'> & { viewport?: PreviewViewportSetting; runtime?: 'desktop' | 'server' };
const builtInDefaults: OpenDefaults = () => ({ profileId: DEFAULT_BROWSER_PROFILE_ID });
/** openPreviewSession: `preview.open` with the configured defaults, resolved once and never from unread settings (the
 *  resolver throws BrowserSettingsReadError and nothing opens); the answer is applied at once (no event needed). */
export async function openPreviewSession(rpc: Rpc, store: PreviewStateStore, ref: ScopedThreadRef, input: OpenInput = {}, defaults: OpenDefaults = builtInDefaults): Promise<PreviewSessionSnapshot> {
  const resolved = defaults();
  const answer = await rpc('preview.open', {
    threadId: ref.threadId, ...(input.runtime ? { runtime: input.runtime } : {}), ...(input.url === undefined ? {} : { url: input.url }),
    viewport: input.viewport ?? resolved.viewport ?? DEFAULT_OPEN_VIEWPORT, profileId: input.profileId ?? resolved.profileId,
  });
  const snapshot = readSnapshot(answer);
  if (!snapshot) throw new Error('The server answered preview.open with no session.');
  if (input.background) store.updateServerSnapshot(ref, snapshot);
  else store.applyServerSnapshot(ref, snapshot);
  if (input.url !== undefined) store.rememberUrl(ref, snapshot.navStatus._tag === 'Idle' ? input.url : snapshot.navStatus.url);
  return snapshot;
}

/** closePreviewSession: optimistic, so a stale list cannot bring the tab back; a failed close restores it. */
export async function closePreviewSession(rpc: Rpc, store: PreviewStateStore, ref: ScopedThreadRef, tabId: string, snapshot: PreviewSessionSnapshot | null): Promise<boolean> {
  store.beginSessionClose(ref, tabId);
  try { await rpc('preview.close', { threadId: ref.threadId, tabId }); return true; }
  catch (error) { if (letGo(error)) throw error; store.cancelSessionClose(ref, snapshot, tabId); return false; }
}

/** usePreviewSession: the thread's sessions from the server, once per connection (an older server without the RPC keeps what it has). */
export async function listPreviewSessions(client: T3Client, native: Native, ref: ScopedThreadRef): Promise<void> {
  const host = browserHost(client), key = scopedThreadKey(ref);
  if (host.listed.get(key) === client.generation) return;
  const pending = host.listing.get(key);
  if (pending) {
    // Another answer's listing runs through that answer's native: if it was let go, this one lists for itself.
    try { await pending; } catch (error) { if (!letGo(error)) throw error; }
    if (host.listed.get(key) === client.generation) return;
  }
  const generation = client.generation;
  const listing = (async () => {
    try {
      const result = await client.rpc(native, 'preview.list', { threadId: ref.threadId });
      const sessions = (Array.isArray(result.sessions) ? result.sessions : []).map(readSnapshot).filter((entry): entry is PreviewSessionSnapshot => !!entry);
      if (str(result.serverEpoch)) host.store.reconcileServerSessions(ref, { sessions, serverEpoch: str(result.serverEpoch), revision: Number(result.revision) || 0 });
      host.listed.set(key, generation);
    } catch (error) { if (letGo(error)) throw error; host.listed.set(key, generation); }
    finally { host.listing.delete(key); }
  })();
  host.listing.set(key, listing);
  return listing;
}

// ── The panel's browser surfaces (rightPanelStore) ───────────────────────────────────────────────
export const browserSurfaceId = (tabId: string) => `browser:${tabId}`;
const browserSurface = (tabId: string, threadKey: string): Surface => ({ id: browserSurfaceId(tabId), kind: 'browser', path: '', line: 0, reveal: 0, browser: { tabId, threadKey } });
/** rightPanelStore.openBrowser: the tab's surface, active, the panel shown. */
export function openBrowserIn(state: PanelState, tabId: string, threadKey: string): Surface {
  const surface = state.surfaces.find(entry => entry.id === browserSurfaceId(tabId)) ?? browserSurface(tabId, threadKey);
  if (!state.surfaces.includes(surface)) state.surfaces.push(surface);
  state.active = surface.id; state.visible = true;
  return surface;
}
/** ChatView activateRightPanelSurface / syncActivePreviewSurface (setActivePreviewTab): a Browser tab the panel selects
 *  becomes its thread's active session, which ⇧⌘J shows again after hiding the panel. */
export function selectBrowserTab(client: T3Client, surface: Surface | undefined): void {
  const ref = surface?.kind === 'browser' && surface.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (ref && surface?.browser) browserHost(client).store.setActiveTab(ref, surface.browser.tabId);
}
/** rightPanelStore.reconcileBrowserSurfaces: browser tabs follow the thread's sessions (new ones join at the end). */
export function reconcileBrowserSurfaces(state: PanelState, tabIds: readonly string[], threadKey: string): boolean {
  const valid = new Set(tabIds.map(browserSurfaceId));
  const existing = state.surfaces.filter(surface => surface.kind === 'browser' && valid.has(surface.id));
  const known = new Set(existing.map(surface => surface.id));
  const added = tabIds.filter(tabId => !known.has(browserSurfaceId(tabId))).map(tabId => browserSurface(tabId, threadKey));
  const surfaces = [...state.surfaces.filter(surface => surface.kind !== 'browser' || valid.has(surface.id)), ...added];
  if (surfaces.length === state.surfaces.length && surfaces.every((surface, index) => surface === state.surfaces[index])) return false;
  state.surfaces = surfaces;
  if (!surfaces.some(surface => surface.id === state.active)) state.active = surfaces.find(surface => surface.kind === 'browser')?.id ?? surfaces[0]?.id ?? '';
  if (!surfaces.length) state.visible = false;
  return true;
}

/** addBrowserSurface (the "+" menu's Browser row, B): a new session under the profile, then its tab. */
export async function addBrowserSurface(client: T3Client, native: Native, state: PanelState, profileId?: string): Promise<string> {
  requirePermission(client.scopes, 'preview:operate');
  // A new thread's draft has its id from the start in the reference (activeThreadRef); here it is allocated now.
  if (!client.threadId && !activeRef(client) && client.environmentId) await ensureDraftThreadId(client, native);
  const ref = activeRef(client);
  if (!ref) return '';
  installBrowserCleanup(client, native);
  const host = browserHost(client);
  await listPreviewSessions(client, native, ref); // the server's epoch first: the tab's native identity names it
  const defaults = resolveBrowserOpenDefaults(client); // read once: never a tab born from unread settings
  const snapshot = await openPreviewSession(rpcOf(client, native), host.store, ref, { ...(profileId === undefined ? {} : { profileId }), ...(obj(obj(client.config.environment).capabilities).serverBrowser === true ? { runtime: ref.environmentId === primaryEnvironmentId() ? 'server' as const : 'desktop' as const } : {}) }, () => defaults);
  if (snapshot.runtime === 'server' && (defaults.zoomFactor !== 1 || defaults.appearance !== 'system')) {
    const updated = readSnapshot(await client.rpc(native, 'preview.adjust', { threadId: ref.threadId, tabId: snapshot.tabId, zoomFactor: defaults.zoomFactor, colorScheme: defaults.appearance }));
    if (updated) host.store.applyServerSnapshot(ref, updated);
  }
  client.diffOpen = false;
  openBrowserIn(state, snapshot.tabId, scopedThreadKey(ref));
  syncNativeSessions(client); // the new page starts at the default zoom and appearance there
  return '';
}

/** The environment's own config: the focused client's, else its background fleet entry's. */
function environmentConfig(client: T3Client, environmentId: string): Obj {
  if (environmentId === client.environmentId) return client.config;
  return [...fleet.entries.values()].find(entry => entry.environmentId === environmentId)?.config ?? {};
}
/** alternatePreviewRuntime: a tab of an environment other than this computer's, which runs a browser of its own,
 *  moves between this computer and that environment. This app always has its own web views. */
export function alternatePreviewRuntime(environmentId: string, primaryId: string | null, serverBrowser: boolean, snapshot: Pick<PreviewSessionSnapshot, 'runtime'> | null): 'desktop' | 'server' | null {
  if (!snapshot || !serverBrowser || environmentId === primaryId) return null;
  return snapshot.runtime === 'server' ? 'desktop' : 'server';
}
/** PreviewView's move row: its target and label ("" when the tab cannot move). */
function previewMove(client: T3Client, ref: ScopedThreadRef, snapshot: PreviewSessionSnapshot | null): { moveTarget: '' | 'desktop' | 'server'; moveLabel: string } {
  const environment = obj(environmentConfig(client, ref.environmentId).environment);
  const target = client.available === true ? alternatePreviewRuntime(ref.environmentId, primaryEnvironmentId(), obj(environment.capabilities).serverBrowser === true, snapshot) : null;
  if (!target) return { moveTarget: '', moveLabel: '' };
  return { moveTarget: target, moveLabel: target === 'server' ? `Open in ${str(environment.label) || 'the environment'}'s browser` : 'Open on this computer' };
}
/** handleMoveTab: the tab's page opens in the other runtime at the same URL, viewport and profile, and becomes the
 *  active tab. True when it opened; the caller then closes the old tab as the tab menu's Close does. */
export async function moveBrowserTab(client: T3Client, native: Native, state: PanelState, tabId: string): Promise<boolean> {
  requirePermission(client.scopes, 'preview:operate');
  const surface = state.surfaces.find(entry => entry.id === browserSurfaceId(tabId) && entry.kind === 'browser');
  const ref = surface?.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (!ref) return false;
  const { nav, snapshot } = effectiveNav(client, ref, tabId), { moveTarget } = previewMove(client, ref, snapshot);
  if (!moveTarget || !snapshot) return false;
  const url = nav._tag === 'Idle' ? '' : nav.url;
  let opened: PreviewSessionSnapshot;
  try {
    opened = await openPreviewSession(rpcOf(client, native), browserHost(client).store, ref, {
      ...(url ? { url: moveTarget === 'desktop' ? resolveDiscoveredServerUrl(ref.environmentId, url, preparedConnection(client)) : url } : {}),
      viewport: snapshot.viewport ?? DEFAULT_OPEN_VIEWPORT, ...(snapshot.profileId === undefined ? {} : { profileId: snapshot.profileId }),
      ...(moveTarget === 'server' ? { runtime: 'server' as const } : {}),
    }, () => resolveBrowserOpenDefaults(client));
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Unable to move the browser tab', description: error instanceof Error && error.message ? error.message : 'An error occurred.' });
    return false;
  }
  client.diffOpen = false;
  openBrowserIn(state, opened.tabId, scopedThreadKey(ref));
  return true;
}

/** Closing a tab closes its session (closePreviewSession) and drops its web view. Registered again by every answer
 *  that can close a tab, as the terminal's hook is: the hook calls through that answer's native, never an older one's. */
export function installBrowserCleanup(client: T3Client, native: Native): void {
  const host = browserHost(client);
  registerSurfaceClose(client, 'browser', { cleanup: async surface => {
    const ref = surface.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
    if (!ref || !surface.browser) return;
    const snapshot = host.store.read(ref).sessions[surface.browser.tabId] ?? null;
    if (ref.environmentId === client.environmentId) await closePreviewSession(rpcOf(client, native), host.store, ref, surface.browser.tabId, snapshot);
    else host.store.beginSessionClose(ref, surface.browser.tabId);
    syncNativeSessions(client);
  } });
}

// ── The tabs' frames (browser-frames.ts) ───────────────────────────────────────────────────────
/** One tab as its frame's records give it (browser-frames.ts `frameNav`). */
export type NativeTab = {
  kind: 'Idle' | 'Loading' | 'Success' | 'LoadFailed'; url: string; title: string; code: number; description: string;
  canGoBack: boolean; canGoForward: boolean; favicon: { dataUrl: string; pageUrl: string; capturedAt: number } | null;
  /** How many loads of this page have failed: each failure is reported once. */
  failures: number;
  /** Part 2: the frame's zoom and the appearance it is told to prefer. */
  zoomFactor: number; colorScheme: string;
};
export function nativeTabs(client: T3Client): Record<string, NativeTab> {
  const tabs: Record<string, NativeTab> = {};
  for (const entry of frames(client).entries.values()) {
    const nav = frameNav(entry);
    tabs[entry.runtimeId] = {
      kind: nav.kind, url: nav.url, title: nav.title, code: entry.failure?.code ?? 0, description: entry.failure?.description ?? '',
      canGoBack: entry.record?.canGoBack === true, canGoForward: entry.record?.canGoForward === true, favicon: entry.favicon,
      failures: entry.failures, zoomFactor: entry.zoom, colorScheme: entry.colorScheme,
    };
  }
  return tabs;
}
const originOf = (url: string): string | null => { try { const parsed = new URL(url); return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.origin : null; } catch { return null; } };
export const navOf = (tab: NativeTab): PreviewNavStatus =>
  tab.kind === 'Idle' ? { _tag: 'Idle' } : tab.kind === 'LoadFailed' ? { _tag: 'LoadFailed', url: tab.url, title: tab.title, code: tab.code, description: tab.description } : { _tag: tab.kind, url: tab.url, title: tab.title };
/** usePreviewBridge projectDesktopState: a page's icon counts only for the page's own origin. */
export function projectDesktopState(tab: NativeTab): DesktopPreviewOverlay {
  const navOrigin = tab.kind === 'Idle' ? null : originOf(tab.url);
  return {
    hasWebContents: true, canGoBack: tab.canGoBack, canGoForward: tab.canGoForward, loading: tab.kind === 'Loading', zoomFactor: tab.zoomFactor, pictureInPicture: false,
    colorScheme: tab.colorScheme === 'light' || tab.colorScheme === 'dark' ? tab.colorScheme : 'system', audioMuted: false, audible: false, controller: 'none',
    favicon: tab.favicon && originOf(tab.favicon.pageUrl) === navOrigin ? tab.favicon : null,
  };
}
/** usePreviewBridge buildReportInput: Idle never reports; (kind, url) repeats collapse; every failure reports. The reference
 *  runs it once per desktop state change; here it runs on every projection, so a failure is told apart by the
 *  frame's failure count and reported once. */
export function buildReportInput(threadId: string, tabId: string, tab: NativeTab, last: Report | null): { input: Obj; report: Report } | null {
  if (tab.kind === 'Idle') return null;
  if (last && tab.kind === last.kind && tab.url === last.url && (tab.kind !== 'LoadFailed' || tab.failures === last.failures)) return null;
  const base = { threadId, tabId, canGoBack: tab.canGoBack, canGoForward: tab.canGoForward };
  const navStatus = tab.kind === 'LoadFailed' ? { _tag: 'LoadFailed', url: tab.url, title: tab.title, code: tab.code, description: tab.description } : { _tag: tab.kind, url: tab.url, title: tab.title };
  return { input: { ...base, navStatus }, report: { kind: tab.kind, url: tab.url, failures: tab.failures } };
}

/** ElectronBrowserHost: a frame for every live session of every thread, at its last URL; the rest unmount. A frame that
 *  is new starts at the default zoom and appearance (desktopTabLifetime's `browserDefaultTabState`), unread settings
 *  leaving it at 100% and System. */
export function liveSessions(client: T3Client): LiveFrame[] {
  const host = browserHost(client), live: LiveFrame[] = [];
  let zoom = 1, colorScheme: LiveFrame['colorScheme'] = 'system';
  try { const defaults = resolveBrowserOpenDefaults(client); zoom = defaults.zoomFactor; colorScheme = defaults.appearance; } catch { /* unread settings */ }
  for (const [key, state] of host.store.active()) {
    const ref = parseScopedThreadKey(key);
    if (!ref) continue;
    for (const snapshot of Object.values(state.sessions)) {
      if (snapshot.runtime === 'server') continue;
      live.push({ runtimeId: previewRuntimeTabId(ref, state.serverEpoch, snapshot.tabId), url: snapshot.navStatus._tag === 'Idle' ? '' : snapshot.navStatus.url,
        profile: snapshot.profileId ?? DEFAULT_BROWSER_PROFILE_ID, environment: ref.environmentId, zoom, colorScheme });
    }
  }
  return live;
}
/** The live tabs' frames; a page whose profile's old cookies the hatch just moved in (T3BrowserPage.swift `migrateCookies`)
 *  reloads once, so a site the person was signed in to sees them. */
export function syncNativeSessions(client: T3Client): void {
  syncFrames(client, liveSessions(client));
  const pages = obj(client.presentation.browserPages), reloaded = browserHost(client).migrationReloads;
  for (const entry of frames(client).entries.values()) {
    if (!(Number(obj(pages[entry.frameId]).migrated) > 0) || reloaded.has(entry.frameId)) continue;
    reloaded.add(entry.frameId);
    commandFrame(client, entry.runtimeId, 'refresh');
  }
}
function frameOp(client: T3Client): void { requirePermission(client.scopes, 'preview:operate'); }

/** Mirrors the module's tab state into the store and reports navigations to the server (usePreviewBridge). */
async function mirrorNativeState(client: T3Client, native: Native): Promise<void> {
  const host = browserHost(client), tabs = nativeTabs(client);
  for (const [key, state] of host.store.active()) {
    const ref = parseScopedThreadKey(key);
    if (!ref) continue;
    for (const snapshot of Object.values(state.sessions)) {
      if (snapshot.runtime === 'server') continue;
      const runtimeId = previewRuntimeTabId(ref, state.serverEpoch, snapshot.tabId), tab = tabs[runtimeId];
      host.store.applyDesktopState(ref, snapshot.tabId, tab ? { ...projectDesktopState(tab), ...automationOverlay(client, runtimeId) } : null); // audio, appearance, controller
      if (!tab || ref.environmentId !== client.environmentId || client.connection !== 'connected') continue;
      const report = buildReportInput(ref.threadId, snapshot.tabId, tab, host.reported.get(runtimeId) ?? null);
      if (!report) continue;
      host.reported.set(runtimeId, report.report);
      try { await client.rpc(native, 'preview.reportStatus', report.input); }
      catch (error) { if (letGo(error)) throw error; host.reported.delete(runtimeId); }
    }
  }
}

// ── The panel's projection ─────────────────────────────────────────────────────────────────────
/** What a tab shows: the module's state while it has a web view, else the server's snapshot. */
export function effectiveNav(client: T3Client, ref: ScopedThreadRef, tabId: string): { nav: PreviewNavStatus; tab: NativeTab | null; snapshot: PreviewSessionSnapshot | null; runtimeId: string } {
  const state = browserHost(client).store.read(ref), snapshot = state.sessions[tabId] ?? null;
  const runtimeId = previewRuntimeTabId(ref, state.serverEpoch, tabId), tab = snapshot?.runtime === 'server' ? null : nativeTabs(client)[runtimeId] ?? null;
  let nav = tab && tab.kind !== 'Idle' ? navOf(tab) : (snapshot?.navStatus ?? { _tag: 'Idle' });
  // A frame that has not reported its page's title yet (a hidden tab mounted at launch) keeps the server's for that URL.
  const known = snapshot?.navStatus;
  if (tab && nav._tag !== 'Idle' && !nav.title && known && known._tag !== 'Idle' && known.url === nav.url && known.title) nav = { ...nav, title: known.title };
  return { nav, tab, snapshot, runtimeId };
}

/** RightPanelTabs surfaceTitle: the page title, else its host, else "Browser". */
export function browserTabTitle(nav: PreviewNavStatus): string {
  if (nav._tag === 'Idle') return 'Browser';
  if (nav.title.trim().length > 0) return nav.title;
  try { return new URL(nav.url).host || 'Browser'; } catch { return 'Browser'; }
}
/** RightPanelTabs PreviewFavicon: the captured icon of this page's origin, then the public favicon service for
 *  public hosts (`faviconUrlForOrigin`, size 32), then the globe glyph (Contract's fallback). */
export function browserTabFavicon(nav: PreviewNavStatus, favicon: DesktopPreviewOverlay['favicon']): { captured: string; fallback: string } {
  const url = nav._tag === 'Idle' ? null : nav.url;
  const captured = favicon && url && originOf(favicon.pageUrl) !== null && originOf(favicon.pageUrl) === originOf(url) ? favicon.dataUrl : '';
  let fallback = '';
  if (url) try {
    const parsed = new URL(url);
    if (parsed.host && (parsed.protocol === 'http:' || parsed.protocol === 'https:') && isPublicFaviconHost(parsed.hostname)) fallback = `https://www.google.com/s2/favicons?domain=${encodeURIComponent(parsed.host)}&sz=32`;
  } catch { /* not a URL: the glyph */ }
  return { captured, fallback };
}

export type BrowserTabView = { title: string; favicon: string; faviconFallback: string };
export function browserTab(client: T3Client, surface: Surface): BrowserTabView {
  const ref = surface.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (!ref || !surface.browser) return { title: 'Browser', favicon: '', faviconFallback: '' };
  const { nav, snapshot } = effectiveNav(client, ref, surface.browser.tabId);
  const overlay = snapshot ? browserHost(client).store.read(ref).desktopByTabId[snapshot.tabId] ?? null : null;
  const icon = browserTabFavicon(nav, overlay?.favicon ?? null);
  return { title: browserTabTitle(nav), favicon: icon.captured, faviconFallback: icon.fallback };
}

export type BrowserView = {
  tabId: string; runtimeId: string; server: boolean; streamUrl: string; environment: string; profileId: string; profileName: string; showProfile: boolean;
  /** A server tab on one of the environment's PDF assets: the PDF this client shows over the stream (browser-url.ts previewPdfAssetUrl). */
  pdfUrl: string;
  /** The viewer page that shows it (server-pdf-viewer.ts; panel-surfaces.ts sets it). */
  pdfViewer: ViewerFrameView;
  url: string; loading: boolean; canGoBack: boolean; canGoForward: boolean; refreshDisabled: boolean; hasWebContents: boolean;
  empty: boolean; failed: boolean; failHost: string; failMessage: string; failLabel: string; live: boolean;
  /** The "+" menu's profile submenu (RightPanelTabs MenuSubPopup). */
  profiles: BrowserProfileChoice[];
  /** Part 3: Annotate, Capture, Float preview and the separate window (browser-capture.ts). */
  capture: BrowserCaptureView;
  /** The More menu's move row and the load-failed page's move button: the other runtime ('' = none) and its label. */
  moveTarget: '' | 'desktop' | 'server'; moveLabel: string;
} & BrowserNavigationView;
/** Part 4: the profile lists are the client's (Default, Incognito and the named ones); without a client, the built-ins. */
export const emptyBrowserView = (client?: T3Client): BrowserView => ({
  tabId: '', runtimeId: '', server: false, streamUrl: '', pdfUrl: '', pdfViewer: emptyViewerFrame(), environment: '', profileId: DEFAULT_BROWSER_PROFILE_ID, profileName: 'Default', showProfile: false, url: '', loading: false, canGoBack: false, canGoForward: false,
  refreshDisabled: true, hasWebContents: false, empty: true, failed: false, failHost: '', failMessage: '', failLabel: '', live: false,
  profiles: browserProfileChoices(client), capture: emptyCaptureView(), moveTarget: '', moveLabel: '', ...emptyNavigationView(),
});

/** PreviewView's chrome and body for the active Browser tab. */
export function browserView(client: T3Client, surface: Surface | null, now = 0): BrowserView {
  const ref = surface?.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (!ref || !surface?.browser) return emptyBrowserView(client);
  const host = browserHost(client), { nav, tab, snapshot, runtimeId } = effectiveNav(client, ref, surface.browser.tabId);
  const empty = shouldShowPreviewEmptyState(snapshot ? { navStatus: nav } : null), failed = nav._tag === 'LoadFailed';
  return {
    tabId: surface.browser.tabId, runtimeId, server: snapshot?.runtime === 'server', streamUrl: serverStreamUrl(client, runtimeId), environment: ref.environmentId,
    pdfUrl: snapshot?.runtime === 'server' && nav._tag === 'Success' ? previewPdfAssetUrl(nav.url, ref.environmentId === client.environmentId ? client.origin : null) ?? '' : '', pdfViewer: emptyViewerFrame(), ...tabProfile(client, snapshot?.profileId), // the badge shows a profile other than the configured default
    url: nav._tag === 'Idle' ? '' : nav.url, loading: nav._tag === 'Loading', canGoBack: snapshot?.runtime === 'server' && !serverStreamFrame(client, runtimeId).controlled ? false : tab?.canGoBack ?? snapshot?.canGoBack ?? false,
    canGoForward: snapshot?.runtime === 'server' && !serverStreamFrame(client, runtimeId).controlled ? false : tab?.canGoForward ?? snapshot?.canGoForward ?? false, refreshDisabled: nav._tag === 'Idle' || (snapshot?.runtime === 'server' && !serverStreamFrame(client, runtimeId).controlled), hasWebContents: !!tab || snapshot?.runtime === 'server',
    empty, failed, failHost: failed ? previewHost(nav.url) : '', failMessage: failed ? describePreviewError(nav.description).replace(/\.+$/, '') : '',
    failLabel: failed ? previewErrorLabel(nav.code, nav.description) : '', live: !!snapshot && !empty && !failed,
    profiles: browserProfileChoices(client), ...previewMove(client, ref, snapshot), ...navigationView(client, ref, runtimeId, tab, snapshot, empty, now),
    capture: snapshot?.runtime === 'server' ? { ...browserCaptureView(client, ref, surface.browser.tabId, runtimeId, true, failed), pickDisabled: true, captureDisabled: true } : browserCaptureView(client, ref, surface.browser.tabId, runtimeId, !!tab, failed),
  };
}

/** useAgentOpenedPreviews: persisted sessions form a baseline, and only a
 * new reveal request may reopen a dismissed floating preview. */
export function revealServerSessions(client: T3Client, state: PanelState, ref: ScopedThreadRef): void {
  const host = browserHost(client), key = scopedThreadKey(ref), sessions = Object.values(host.store.read(ref).sessions).filter(session => session.runtime === 'server');
  const previous = host.serverSeen.get(key);
  host.serverSeen.set(key, new Map(sessions.map(session => [session.tabId, session.revealRequest?.id])));
  if (!previous) return;
  for (const session of sessions) {
    const requested = session.revealRequest, fresh = requested ? previous.get(session.tabId) !== requested.id : !previous.has(session.tabId);
    if (!fresh || session.reveal !== true || (!browserAutoShowFloatingPreview(client) && requested?.force !== true)) continue;
    const existing = state.surfaces.some(surface => surface.browser?.tabId === session.tabId);
    if (existing && requested?.force === true) openBrowserIn(state, session.tabId, key);
    else if (!existing) miniStoreOf(client).open(deviceThreadId(client), browserMiniPlayerSource(session.tabId));
  }
}

/** The panel's effects before it projects (usePreviewSession, the reconcile effect, usePreviewBridge, ElectronBrowserHost). */
export async function browserPrepare(client: T3Client, native: Native | null | undefined, state: PanelState): Promise<void> {
  if (!native?.available) return;
  const operate = can(client, 'preview:operate'), read = can(client, 'orchestration:read');
  if (!operate && !read) { await retainServerStreams(client, []); return; }
  const ref = activeRef(client), host = browserHost(client);
  installBrowserCleanup(client, native);
  if (operate) adoptAutomationTabs(client); // tabs the previewAutomation host opened (browser-automation.ts)
  try {
    if (read && ref && client.connection === 'connected' && ref.environmentId === client.environmentId) {
      await listPreviewSessions(client, native, ref);
      revealServerSessions(client, state, ref);
      // The reference reconciles on every session change; here once the thread was listed (a tab the panel shows is
      // not dropped before the server answers) and the client is ready (the saved panel is restored first:
      // restoreRightPanel takes only a panel with no surfaces).
      if (client.ready && host.listed.get(scopedThreadKey(ref)) === client.generation) reconcileBrowserSurfaces(state, Object.keys(host.store.read(ref).sessions), scopedThreadKey(ref));
    }
    if (operate) {
      await mirrorNativeState(client, native);
      syncNativeSessions(client);
      adoptCaptureNative(client, native);
      await cancelHiddenPicks(client, shownRuntimeId(client, state)); // PreviewView's unmount: a hidden tab's pick ends
      await navigationPrepare(client, native, state);
      await automationPrepare(client, native); // the previewAutomation host (browser-automation.ts)
    }
    const active = state.visible ? state.surfaces.find(surface => surface.id === state.active) : undefined;
    const shown = active?.browser && ref ? effectiveNav(client, ref, active.browser.tabId) : null;
    const floatingId = floatingTabOf(client), floating = ref && floatingId && floatingId !== shown?.snapshot?.tabId ? effectiveNav(client, ref, floatingId) : null;
    const slots = (read ? [shown, floating] : []).filter((slot): slot is NonNullable<typeof shown> => !!slot && slot.snapshot?.runtime === 'server');
    // The picture-in-picture window's stream lives while it floats, whichever thread shows (server-browser-pip.ts).
    const pipTab = serverPipOf(client), pipNav = pipTab && ref?.threadId === pipTab.threadId ? effectiveNav(client, ref, pipTab.tabId).nav : null;
    settleServerPictureInPicture(client, pipTab && { threadId: pipTab.threadId, tabId: pipTab.tabId, pdf: pipNav?._tag === 'Success' && previewPdfAssetUrl(pipNav.url, pipNav.url) !== null });
    const pip = read ? serverPipOf(client) : null;
    await retainServerStreams(client, [...slots.map(slot => slot.runtimeId), ...(pip ? [pip.runtimeId] : [])]);
    if (pip) await prepareServerStream(client, native, pip.runtimeId, pip.threadId, pip.tabId);
    for (const slot of slots) {
      await drainServerNotices(client, native, slot.runtimeId, tabId => {
        if (!ref) return; host.listed.delete(scopedThreadKey(ref));
        if (slot === floating) miniStoreOf(client).open(deviceThreadId(client), browserMiniPlayerSource(tabId));
        else openBrowserIn(state, tabId, scopedThreadKey(ref));
      });
      await prepareServerStream(client, native, slot.runtimeId, ref?.threadId ?? '', slot.snapshot?.tabId ?? '');
    }
  } catch (error) { if (letGo(error)) throw error; }
}

// ── The frames' records and requests (browser-frames.contract) ──────────────────────────────────
/** `surface-browser-frame*`: a frame's navigation record, its window request, download and agent-held dialog, a namer
 *  frame's load and the root's acknowledgement of a frame command, which are the pages' own; and the person's answer to a
 *  held dialog and a View chord over a shown page. None is a use of the panel, so they run before its state. */
export async function frameLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  switch (op) {
    case 'frame': adoptFrameEvent(client, id, value, navigationNow(client)); break;
    case 'frame-done': frameCommandDone(client, Number(value) || 0); break;
    case 'frame-named': await namerLoaded(client, id); break;
    case 'frame-window': { const q = new URLSearchParams(value); await openFrameWindow(client, native, id, q.get('disposition') ?? '', q.get('url') ?? ''); break; }
    case 'frame-prompt': adoptFramePrompt(client, id, value); break;
    case 'frame-answer': await answerFramePrompt(client, id, value); break;
    case 'frame-key': await browserKey(client, id, value); break;
    case 'frame-url-focused': focusUrlTaken(client, id, Number(value) || 0); break;
    // A person's download asks where with the platform's save sheet; an agent's goes to setDownloadBehavior's folder and
    // then into the artifact directory. The app shows nothing about either, as the reference's handler shows nothing.
    case 'frame-download': await adoptAgentDownload(client, native, value); break;
  }
  return '';
}

// ── The chrome row's commands (`surface-browser-<op>`, PreviewView's handlers) ───────────────────
export async function browserLocal(client: T3Client, native: Native, state: PanelState, op: string, id: string, value: string): Promise<string> {
  if (!op.startsWith('artifact-') && op !== 'annotation-held') requirePermission(client.scopes, 'preview:operate');
  const host = browserHost(client);
  if (op === 'open') return addBrowserSurface(client, native, state, value || undefined);
  // Part 3: a toast's artifact buttons (id = the file) and the floating player's pill (id = its runtime tab).
  if (op.startsWith('artifact-')) return artifactLocal(client, native, op.slice(9), id, value);
  if (op.startsWith('mini-')) return browserMiniLocal(client, native, op.slice(5), id, value, tabId => reopenBrowserTab(client, state, tabId));
  if (op === 'pip-left') { closeServerPictureInPicture(client); return ''; } // the picture-in-picture window's close
  if (op === 'annotation-held') return annotationAttachedToDraft(client); // an annotation's ⌘Return the window could not send
  if (op === 'show') return showPreview(client, native, state); // ⇧⌘J (browser-navigation.ts)
  const surface = state.surfaces.find(entry => entry.id === browserSurfaceId(id) && entry.kind === 'browser');
  const ref = surface?.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (!ref || !surface?.browser) return '';
  const { nav, runtimeId, snapshot } = effectiveNav(client, ref, surface.browser.tabId);
  const profile = snapshot?.profileId ?? DEFAULT_BROWSER_PROFILE_ID;
  if (snapshot?.runtime === 'server') {
    if (op === 'float') { toggleFloat(client, state, { ref, tabId: snapshot.tabId, runtimeId }); return ''; }
    if (op === 'navigate' || op === 'open-url') {
      let url: string; try { url = resolveAddressBarInput(value, addressBarSearchTemplate(client)); } catch { return ''; }
      if (sendServerCommand(client, runtimeId, { type: 'navigate', url })) {
        host.store.rememberUrl(ref, url); recordVisit(client, ref, url, navigationNow(client));
      }
      return '';
    }
    if (op === 'back' || op === 'forward') { sendServerCommand(client, runtimeId, { type: 'history', delta: op === 'back' ? -1 : 1 }); return ''; }
    if (op === 'refresh') { sendServerCommand(client, runtimeId, { type: 'reload' }); return ''; }
    if (op === 'hard-reload' || op === 'clear-cookies' || op === 'clear-cache') {
      const answer = await client.rpc(native, 'preview.adjust', { threadId: ref.threadId, tabId: snapshot.tabId, ...(op === 'hard-reload' ? { hardReload: true } : { clear: op === 'clear-cookies' ? 'cookies' : 'cache' }) });
      const updated = readSnapshot(answer); if (updated) host.store.applyServerSnapshot(ref, updated); return '';
    }
  } else if (await captureLocal(client, native, state, op, { ref, tabId: surface.browser.tabId, runtimeId }, value)) return '';
  switch (op) {
    case 'navigate': {
      // handleSubmitUrl: text that is not an address searches the chosen engine for it; only empty input or an unsupported
      // scheme does nothing (the field keeps the text). The URL loads in this tab's web view and is remembered.
      let url: string;
      try { url = resolveAddressBarInput(value, addressBarSearchTemplate(client)); } catch { return ''; }
      frameOp(client); navigateFrame(client, runtimeId, url);
      host.store.rememberUrl(ref, url);
      recordVisit(client, ref, url, navigationNow(client));
      return '';
    }
    case 'toggle-mute': { // the tab menu's Mute / Unmute and the tab's audio button (previewBridge.setAudioMuted)
      const overlay = snapshot ? host.store.read(ref).desktopByTabId[snapshot.tabId] : undefined;
      if (overlay) await toggleFrameMute(client, native, runtimeId, !overlay.audioMuted);
      return '';
    }
    case 'back': case 'forward': case 'refresh': case 'hard-reload':
      frameOp(client); commandFrame(client, runtimeId, op);
      return '';
    case 'external': {
      // PreviewView handleOpenInBrowser: localApi.shell.openExternal(url) (external-open.ts).
      const url = nav._tag === 'Idle' ? '' : nav.url;
      if (url) openExternal(url);
      return '';
    }
    case 'clear-cookies': case 'clear-cache': {
      // Part 4, PreviewMoreMenu's Profile group: this tab's environment and profile only; a failure is ignored, as there.
      await clearFramePartition(client, ref.environmentId, profile, op === 'clear-cookies' ? 'cookies' : 'cache').catch((error: unknown) => { if (letGo(error)) throw error; });
      return '';
    }
  }
  return (await navigationLocal(client, native, surface, op, value)) ?? ''; // zoom, appearance, the viewport, the empty state's rows
}

// ── Part 3: the tabs the module has, by thread, and the floating player's Open in right panel ─────────
/** Every live tab with its thread (Annotate's results go to that thread's composer). */
/** The runtime id of the Browser tab the panel shows, if it shows one. */
function shownRuntimeId(client: T3Client, state: PanelState): string | null {
  const active = state.visible ? state.surfaces.find(entry => entry.id === state.active && entry.kind === 'browser') : undefined;
  const ref = active?.browser ? parseScopedThreadKey(active.browser.threadKey) : null;
  return ref && active?.browser ? effectiveNav(client, ref, active.browser.tabId).runtimeId : null;
}
/** ThreadPreviewMiniPlayer openInPanel: the floating tab back in the right panel (rightPanelStore.openBrowser). */
function reopenBrowserTab(client: T3Client, state: PanelState, tabId: string): void {
  const ref = activeRef(client);
  if (!ref) return;
  client.diffOpen = false;
  openBrowserIn(state, tabId, scopedThreadKey(ref));
}
/** The thread's sessions as the floating player reads them (browser-capture.ts browserMiniView). */
export function browserMiniSessions(client: T3Client, ref: ScopedThreadRef): { serverEpoch: string | null; tabs: Record<string, { url: string; profileId: string; server?: boolean }> } {
  const state = browserHost(client).store.read(ref), tabs: Record<string, { url: string; profileId: string; server?: boolean }> = {};
  for (const snapshot of Object.values(state.sessions)) tabs[snapshot.tabId] = { url: snapshot.navStatus._tag === 'Idle' ? '' : snapshot.navStatus.url, profileId: snapshot.profileId ?? DEFAULT_BROWSER_PROFILE_ID, ...(snapshot.runtime === 'server' ? { server: true } : {}) };
  return { serverEpoch: state.serverEpoch, tabs };
}
