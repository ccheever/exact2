// browser-surface part 1: the right panel's Browser tabs over a native WKWebView (X1 path B, user
// decision 2026-10-08; MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/web/src/components/preview/{addBrowserSurface,openPreviewSession,closePreviewSession,usePreviewBridge,
// PreviewView}.ts(x), apps/web/src/rightPanelStore.ts `openBrowser`/`reconcileBrowserSurfaces`,
// apps/web/src/components/RightPanelTabs.tsx `surfaceTitle`/`SurfaceIcon`, apps/web/src/browser/ElectronBrowserHost.tsx).
//
// A tab is a server preview session (`preview.open`, `.close`, `.list`, `.reportStatus`): the server
// issues its id and keeps it across a hidden panel and a thread switch. The page itself is the clone
// module's `t3-browser` view (T3BrowserView.swift, T3BrowserSessions.swift): one WKWebView per live
// session, kept by the module while the session exists, whether or not the panel shows it (the
// reference's ElectronBrowserHost keeps a <webview> per session at the app root). This file is the
// data module's half: the per-thread session store (browser-state.ts), opening and closing tabs, the
// chrome row's commands, the native state mirrored back to the server, and the panel's projection.
import type { T3Client } from './client';
import type { Native } from './protocol';
import { obj, str, type Obj } from './domain';
import { activeRef } from './terminal-drawer-view';
import { parseScopedThreadKey, scopedThreadKey, type ScopedThreadRef } from './terminal-ui-state';
import { isPublicFaviconHost } from './host-classification';
import { registerSurfaceClose } from './right-panel-tabs';
import type { PanelState, Surface } from './r4-surfaces-panel';
import { letGo } from './let-go';
import { ensureDraftThreadId } from './r7-handoff-thread';
import {
  PreviewStateStore, previewRuntimeTabId, readSnapshot, shouldShowPreviewEmptyState,
  type DesktopPreviewOverlay, type PreviewNavStatus, type PreviewSessionSnapshot, type PreviewViewportSetting,
} from './browser-state';
import { describePreviewError, normalizePreviewUrl, previewErrorLabel, previewHost } from './browser-url';
// Part 3 (browser-surface-capture): Annotate, Capture, Float preview, the separate window and the floating player.
import { adoptCaptureNative, applyCaptureResults, artifactLocal, browserCaptureView, browserMiniLocal, captureLocal, emptyCaptureView, type BrowserCaptureView } from './browser-capture';
// browser-surface part 2: history, discovery, zoom, appearance, the viewport and the preview keys (browser-navigation.ts).
import { emptyNavigationView, navigationLocal, navigationNow, navigationPrepare, navigationView, readTabNavigation, showPreview, type BrowserNavigationView } from './browser-navigation';
import { browserHistory } from './browser-history';
import { environmentHostname } from './browser-targets';
import { adoptAutomationTabs, automationOverlay, automationPrepare } from './browser-automation';

// ── Profiles (browserProfile.ts) ───────────────────────────────────────────────────────────────
export const DEFAULT_BROWSER_PROFILE_ID = 'default';
export type BrowserProfileChoice = { id: string; name: string };
/** The profiles a new tab can open under. Part 1 has the built-in Default only; Incognito and the 24
 *  named profiles arrive with part 4 (browser-surface-profiles), which fills this list. */
export const BROWSER_PROFILES: readonly BrowserProfileChoice[] = [{ id: DEFAULT_BROWSER_PROFILE_ID, name: 'Default' }];
/** PreviewView previewProfileName: a tab's profile by name, "Removed profile" once it is gone. */
export const browserProfileName = (profiles: readonly BrowserProfileChoice[], profileId: string): string =>
  profiles.find(profile => profile.id === profileId)?.name ?? 'Removed profile';
/** RightPanelEmptyState: the launcher's Browser row shows its profile chevron only with a choice to make. */
export const launcherOffersProfiles = (profiles: readonly BrowserProfileChoice[]): boolean => profiles.length > 1;
/** browserDefaultOpenViewport: fill, the reference's default (the Settings row that changes it moved to browser-surface-profiles, part 4). */
export const DEFAULT_OPEN_VIEWPORT: PreviewViewportSetting = { _tag: 'fill' };

// ── The data module's browser host, one per client ─────────────────────────────────────────────
export type Rpc = (method: string, payload: Obj) => Promise<Obj>;
/** The last report per tab: its kind and URL, and for a failure the module's failure count (one report per failure). */
type Report = { kind: string; url: string; failures: number };
type Host = {
  store: PreviewStateStore;
  /** Threads whose sessions were listed on this connection (thread key → client generation). */
  listed: Map<string, number>;
  listing: Map<string, Promise<void>>;
  /** usePreviewBridge's dedupe: the last (kind, url) reported per runtime tab. */
  reported: Map<string, Report>;
  /** The live set last sent to the module (`browserSync`). */
  synced: string;
};
const hosts = new WeakMap<T3Client, Host>();
export function browserHost(client: T3Client): Host {
  let host = hosts.get(client);
  if (!host) { host = { store: new PreviewStateStore(), listed: new Map(), listing: new Map(), reported: new Map(), synced: '' }; hosts.set(client, host); }
  return host;
}
const rpcOf = (client: T3Client, native: Native): Rpc => (method, payload) => client.rpc(native, method, payload, method !== 'preview.list');

// ── Sessions: open, close, list (openPreviewSession, addBrowserSurface, closePreviewSession, usePreviewSession) ──
export type OpenInput = { url?: string; profileId?: string; viewport?: PreviewViewportSetting };
/** openPreviewSession: `preview.open` with the configured defaults; the answer is applied at once (no event needed). */
export async function openPreviewSession(rpc: Rpc, store: PreviewStateStore, ref: ScopedThreadRef, input: OpenInput = {}): Promise<PreviewSessionSnapshot> {
  const answer = await rpc('preview.open', {
    threadId: ref.threadId, ...(input.url === undefined ? {} : { url: input.url }),
    viewport: input.viewport ?? DEFAULT_OPEN_VIEWPORT, profileId: input.profileId ?? DEFAULT_BROWSER_PROFILE_ID,
  });
  const snapshot = readSnapshot(answer);
  if (!snapshot) throw new Error('The server answered preview.open with no session.');
  store.applyServerSnapshot(ref, snapshot);
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
  // A new thread's draft has its id from the start in the reference (activeThreadRef); here it is allocated now.
  if (!client.threadId && !activeRef(client) && client.environmentId) await ensureDraftThreadId(client, native);
  const ref = activeRef(client);
  if (!ref) return '';
  installBrowserCleanup(client, native);
  const host = browserHost(client);
  await listPreviewSessions(client, native, ref); // the server's epoch first: the tab's native identity names it
  const snapshot = await openPreviewSession(rpcOf(client, native), host.store, ref, profileId === undefined ? {} : { profileId });
  client.diffOpen = false;
  openBrowserIn(state, snapshot.tabId, scopedThreadKey(ref));
  await syncNativeSessions(client, native);
  return '';
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
    await syncNativeSessions(client, native);
  } });
}

// ── The native web views (T3BrowserSessions.swift) ─────────────────────────────────────────────
/** One tab as the module reports it (`presentation.browserTabs[runtimeId]`). */
export type NativeTab = {
  kind: 'Idle' | 'Loading' | 'Success' | 'LoadFailed'; url: string; title: string; code: number; description: string;
  canGoBack: boolean; canGoForward: boolean; favicon: { dataUrl: string; pageUrl: string; capturedAt: number } | null;
  /** How many loads of this page have failed: each failure is reported once. */
  failures: number;
  /** Part 2: the page's zoom (`pageZoom`) and the appearance it is told to prefer. */
  zoomFactor: number; colorScheme: string;
};
export function nativeTabs(client: T3Client): Record<string, NativeTab> {
  const tabs: Record<string, NativeTab> = {};
  for (const [id, raw] of Object.entries(obj(client.presentation.browserTabs))) {
    const value = obj(raw), favicon = obj(value.favicon), kind = str(value.kind);
    tabs[id] = {
      kind: kind === 'Loading' || kind === 'Success' || kind === 'LoadFailed' ? kind : 'Idle', url: str(value.url), title: str(value.title),
      code: Number(value.code) || 0, description: str(value.description), canGoBack: value.canGoBack === true, canGoForward: value.canGoForward === true,
      favicon: str(favicon.dataUrl) && str(favicon.pageUrl) ? { dataUrl: str(favicon.dataUrl), pageUrl: str(favicon.pageUrl), capturedAt: Number(favicon.capturedAt) || 0 } : null,
      failures: Number(value.failures) || 0, ...readTabNavigation(value),
    };
  }
  return tabs;
}
const originOf = (url: string): string | null => { try { const parsed = new URL(url); return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.origin : null; } catch { return null; } };
export const navOf = (tab: NativeTab): PreviewNavStatus =>
  tab.kind === 'Idle' ? { _tag: 'Idle' } : tab.kind === 'LoadFailed' ? { _tag: 'LoadFailed', url: tab.url, title: tab.title, code: tab.code, description: tab.description } : { _tag: tab.kind, url: tab.url, title: tab.title };
/** usePreviewBridge projectDesktopState: a captured favicon counts only for the page's own origin. */
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
 *  module's failure count and reported once. */
export function buildReportInput(threadId: string, tabId: string, tab: NativeTab, last: Report | null): { input: Obj; report: Report } | null {
  if (tab.kind === 'Idle') return null;
  if (last && tab.kind === last.kind && tab.url === last.url && (tab.kind !== 'LoadFailed' || tab.failures === last.failures)) return null;
  const base = { threadId, tabId, canGoBack: tab.canGoBack, canGoForward: tab.canGoForward };
  const navStatus = tab.kind === 'LoadFailed' ? { _tag: 'LoadFailed', url: tab.url, title: tab.title, code: tab.code, description: tab.description } : { _tag: tab.kind, url: tab.url, title: tab.title };
  return { input: { ...base, navStatus }, report: { kind: tab.kind, url: tab.url, failures: tab.failures } };
}

type Live = { id: string; url: string; profile: string; environment: string };
/** ElectronBrowserHost: a web view for every live session of every thread, at its last URL; the rest close. */
export function liveSessions(client: T3Client): Live[] {
  const host = browserHost(client), live: Live[] = [];
  for (const [key, state] of host.store.active()) {
    const ref = parseScopedThreadKey(key);
    if (!ref) continue;
    for (const snapshot of Object.values(state.sessions)) {
      live.push({ id: previewRuntimeTabId(ref, state.serverEpoch, snapshot.tabId), url: snapshot.navStatus._tag === 'Idle' ? '' : snapshot.navStatus.url,
        profile: snapshot.profileId ?? DEFAULT_BROWSER_PROFILE_ID, environment: ref.environmentId });
    }
  }
  return live;
}
export async function syncNativeSessions(client: T3Client, native: Native): Promise<void> {
  const host = browserHost(client), live = liveSessions(client);
  const signature = JSON.stringify(live.map(entry => entry.id).sort());
  if (signature === host.synced) return;
  const reply = await client.raw(native, { op: 'browserSync', tabs: live });
  if (reply.ok) host.synced = signature;
}
async function nativeOp(client: T3Client, native: Native, request: Obj): Promise<Obj> {
  const reply = await client.raw(native, request);
  return reply.ok ? obj(reply.value) : {};
}

/** Mirrors the module's tab state into the store and reports navigations to the server (usePreviewBridge). */
async function mirrorNativeState(client: T3Client, native: Native): Promise<void> {
  const host = browserHost(client), tabs = nativeTabs(client);
  for (const [key, state] of host.store.active()) {
    const ref = parseScopedThreadKey(key);
    if (!ref) continue;
    for (const snapshot of Object.values(state.sessions)) {
      const runtimeId = previewRuntimeTabId(ref, state.serverEpoch, snapshot.tabId), tab = tabs[runtimeId];
      host.store.applyDesktopState(ref, snapshot.tabId, tab ? { ...projectDesktopState(tab), ...automationOverlay(client, runtimeId) } : null); // part 5: audio, appearance, controller
      if (!tab || ref.environmentId !== client.environmentId || client.connection !== 'connected') continue;
      const report = buildReportInput(ref.threadId, snapshot.tabId, tab, host.reported.get(runtimeId) ?? null);
      if (!report) continue;
      host.reported.set(runtimeId, report.report);
      try { await client.rpc(native, 'preview.reportStatus', report.input, true); }
      catch (error) { if (letGo(error)) throw error; host.reported.delete(runtimeId); }
    }
  }
}

// ── The panel's projection ─────────────────────────────────────────────────────────────────────
/** What a tab shows: the module's state while it has a web view, else the server's snapshot. */
export function effectiveNav(client: T3Client, ref: ScopedThreadRef, tabId: string): { nav: PreviewNavStatus; tab: NativeTab | null; snapshot: PreviewSessionSnapshot | null; runtimeId: string } {
  const state = browserHost(client).store.read(ref), snapshot = state.sessions[tabId] ?? null;
  const runtimeId = previewRuntimeTabId(ref, state.serverEpoch, tabId), tab = nativeTabs(client)[runtimeId] ?? null;
  const nav = tab && tab.kind !== 'Idle' ? navOf(tab) : (snapshot?.navStatus ?? { _tag: 'Idle' });
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
  tabId: string; runtimeId: string; environment: string; profileId: string; profileName: string; showProfile: boolean;
  url: string; loading: boolean; canGoBack: boolean; canGoForward: boolean; refreshDisabled: boolean; hasWebContents: boolean;
  empty: boolean; failed: boolean; failHost: string; failMessage: string; failLabel: string; live: boolean;
  /** The "+" menu's profile submenu (RightPanelTabs MenuSubPopup). */
  profiles: BrowserProfileChoice[];
  /** Part 3: Annotate, Capture, Float preview and the separate window (browser-capture.ts). */
  capture: BrowserCaptureView;
} & BrowserNavigationView;
export const emptyBrowserView = (): BrowserView => ({
  tabId: '', runtimeId: '', environment: '', profileId: DEFAULT_BROWSER_PROFILE_ID, profileName: 'Default', showProfile: false, url: '', loading: false, canGoBack: false, canGoForward: false,
  refreshDisabled: true, hasWebContents: false, empty: true, failed: false, failHost: '', failMessage: '', failLabel: '', live: false,
  profiles: BROWSER_PROFILES.map(profile => ({ ...profile })), capture: emptyCaptureView(), ...emptyNavigationView(),
});

/** PreviewView's chrome and body for the active Browser tab. */
export function browserView(client: T3Client, surface: Surface | null, now = 0): BrowserView {
  const ref = surface?.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (!ref || !surface?.browser) return emptyBrowserView();
  const host = browserHost(client), { nav, tab, snapshot, runtimeId } = effectiveNav(client, ref, surface.browser.tabId);
  const profileId = snapshot?.profileId ?? DEFAULT_BROWSER_PROFILE_ID, empty = shouldShowPreviewEmptyState(snapshot ? { navStatus: nav } : null), failed = nav._tag === 'LoadFailed';
  return {
    tabId: surface.browser.tabId, runtimeId, environment: ref.environmentId, profileId, profileName: browserProfileName(BROWSER_PROFILES, profileId), showProfile: profileId !== DEFAULT_BROWSER_PROFILE_ID,
    url: nav._tag === 'Idle' ? '' : nav.url, loading: nav._tag === 'Loading', canGoBack: tab?.canGoBack ?? snapshot?.canGoBack ?? false,
    canGoForward: tab?.canGoForward ?? snapshot?.canGoForward ?? false, refreshDisabled: nav._tag === 'Idle', hasWebContents: !!tab,
    empty, failed, failHost: failed ? previewHost(nav.url) : '', failMessage: failed ? describePreviewError(nav.description).replace(/\.+$/, '') : '',
    failLabel: failed ? previewErrorLabel(nav.code, nav.description) : '', live: !!snapshot && !empty && !failed,
    profiles: BROWSER_PROFILES.map(profile => ({ ...profile })), ...navigationView(client, ref, runtimeId, tab, snapshot, empty, now),
    capture: browserCaptureView(client, ref, surface.browser.tabId, runtimeId, !!tab, failed),
  };
}

/** The panel's effects before it projects (usePreviewSession, the reconcile effect, usePreviewBridge, ElectronBrowserHost). */
export async function browserPrepare(client: T3Client, native: Native | null | undefined, state: PanelState): Promise<void> {
  if (!native?.available) return;
  const ref = activeRef(client), host = browserHost(client);
  installBrowserCleanup(client, native);
  adoptAutomationTabs(client); // part 5: tabs the previewAutomation host opened (browser-automation.ts)
  try {
    if (ref && client.connection === 'connected' && ref.environmentId === client.environmentId) {
      await listPreviewSessions(client, native, ref);
      // The reference reconciles on every session change; here once the thread was listed (a tab the panel shows is
      // not dropped before the server answers) and the client is ready (the saved panel is restored first:
      // restoreRightPanel takes only a panel with no surfaces).
      if (client.ready && host.listed.get(scopedThreadKey(ref)) === client.generation) reconcileBrowserSurfaces(state, Object.keys(host.store.read(ref).sessions), scopedThreadKey(ref));
    }
    await mirrorNativeState(client, native);
    await syncNativeSessions(client, native);
    // Part 3: Annotate's settled picks reach the composer once each (browser-capture.ts).
    adoptCaptureNative(client, native);
    await applyCaptureResults(client, native, liveTabKeys(client));
    await navigationPrepare(client, native, state);
    await automationPrepare(client, native); // part 5: the previewAutomation host (browser-automation.ts)
  } catch (error) { if (letGo(error)) throw error; }
}

// ── The chrome row's commands (`surface-browser-<op>`, PreviewView's handlers) ───────────────────
export async function browserLocal(client: T3Client, native: Native, state: PanelState, op: string, id: string, value: string): Promise<string> {
  const host = browserHost(client);
  if (op === 'open') return addBrowserSurface(client, native, state, value || undefined);
  // Part 3: a toast's artifact buttons (id = the file) and the floating player's pill (id = its runtime tab).
  if (op.startsWith('artifact-')) return artifactLocal(client, native, op.slice(9), id, value);
  if (op.startsWith('mini-')) return browserMiniLocal(client, native, op.slice(5), id, tabId => reopenBrowserTab(client, state, tabId));
  if (op === 'show') return showPreview(client, native, state); // ⇧⌘J (browser-navigation.ts)
  const surface = state.surfaces.find(entry => entry.id === browserSurfaceId(id) && entry.kind === 'browser');
  const ref = surface?.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (!ref || !surface?.browser) return '';
  const { nav, runtimeId, snapshot } = effectiveNav(client, ref, surface.browser.tabId);
  const profile = snapshot?.profileId ?? DEFAULT_BROWSER_PROFILE_ID;
  if (await captureLocal(client, native, state, op, { ref, tabId: surface.browser.tabId, runtimeId }, value)) return ''; // part 3
  switch (op) {
    case 'navigate': {
      // handleSubmitUrl: an address the rules refuse does nothing (the server's failed event, not a toast, is the
      // reference's only error path); a good one loads in this tab's web view and is remembered.
      let url: string;
      try { url = normalizePreviewUrl(value); } catch { return ''; }
      await nativeOp(client, native, { op: 'browserNavigate', tab: runtimeId, url, profile, environment: ref.environmentId });
      host.store.rememberUrl(ref, url);
      browserHistory(client.local).recordVisitForThread(ref, url, navigationNow(client), environmentHostname(client)); // part 2: recordVisitForThread
      return '';
    }
    case 'toggle-mute': { // part 5: the tab menu's Mute / Unmute and the tab's audio button (previewBridge.setAudioMuted)
      const overlay = snapshot ? host.store.read(ref).desktopByTabId[snapshot.tabId] : undefined;
      if (overlay) await nativeOp(client, native, { op: 'browserMute', tab: runtimeId, muted: !overlay.audioMuted });
      return '';
    }
    case 'back': case 'forward': case 'refresh': case 'hard-reload':
      await nativeOp(client, native, { op: 'browserCommand', tab: runtimeId, command: op });
      return '';
    case 'external': {
      // PreviewView handleOpenInBrowser: localApi.shell.openExternal(url); an agent run records it (T3RemoteEditors).
      const url = nav._tag === 'Idle' ? '' : nav.url;
      if (url) await nativeOp(client, native, { op: 'remoteEditorsOpen', url });
      return '';
    }
  }
  return (await navigationLocal(client, native, surface, op, value)) ?? ''; // part 2: zoom, appearance, the viewport, the empty state's rows
}

// ── Part 3: the tabs the module has, by thread, and the floating player's Open in right panel ─────────
/** Every live tab with its thread (Annotate's results go to that thread's composer). */
function liveTabKeys(client: T3Client): Array<{ runtimeId: string; threadKey: string }> {
  const host = browserHost(client), out: Array<{ runtimeId: string; threadKey: string }> = [];
  for (const [key, state] of host.store.active()) {
    const ref = parseScopedThreadKey(key);
    if (ref) for (const snapshot of Object.values(state.sessions)) out.push({ runtimeId: previewRuntimeTabId(ref, state.serverEpoch, snapshot.tabId), threadKey: key });
  }
  return out;
}
/** ThreadPreviewMiniPlayer openInPanel: the floating tab back in the right panel (rightPanelStore.openBrowser). */
function reopenBrowserTab(client: T3Client, state: PanelState, tabId: string): void {
  const ref = activeRef(client);
  if (!ref) return;
  client.diffOpen = false;
  openBrowserIn(state, tabId, scopedThreadKey(ref));
}
/** The thread's sessions as the floating player reads them (browser-capture.ts browserMiniView). */
export function browserMiniSessions(client: T3Client, ref: ScopedThreadRef): { serverEpoch: string | null; tabs: Record<string, { url: string; profileId: string }> } {
  const state = browserHost(client).store.read(ref), tabs: Record<string, { url: string; profileId: string }> = {};
  for (const snapshot of Object.values(state.sessions)) tabs[snapshot.tabId] = { url: snapshot.navStatus._tag === 'Idle' ? '' : snapshot.navStatus.url, profileId: snapshot.profileId ?? DEFAULT_BROWSER_PROFILE_ID };
  return { serverEpoch: state.serverEpoch, tabs };
}
