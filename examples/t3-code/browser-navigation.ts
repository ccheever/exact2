// browser-surface part 2: the Browser tab's navigation aids (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// apps/web/src/components/preview/{PreviewView,PreviewEmptyState,PreviewRecentUrlCard,PreviewLocalServerCard,
// PreviewMoreMenu,ZoomIndicator}.tsx, openDiscoveredPort.ts; apps/web/src/browser/{HostedBrowserWebview,
// BrowserDeviceToolbar,useBrowserViewportResize}.tsx; apps/web/src/routes/_chat.tsx and ChatView.tsx
// `preview.toggle`/`togglePreviewPanel`; packages/shared/src/keybindings.ts `previewFocus`).
//
// The data module's half of part 2: the empty state's Recently used (at most 8, from browser-history.ts) and
// Local servers (browser-targets.ts), every visit recorded and titled; zoom and appearance (the module's
// `browserSet`: WebKit's `pageZoom` and the web view's `appearance`, which pages read as
// `prefers-color-scheme`); the viewport (`preview.resize`, serialized per tab by browser-viewport.ts) with the
// device toolbar's preset, size, aspect lock, rotate and close, and the rails' drags and arrow keys; the zoom
// indicator's trigger; the chords the module answers while the preview has the focus; and ⇧⌘J.
import type { T3Client } from './client';
import { arr, obj, str } from './domain';
import type { Native } from './protocol';
import { letGo } from './let-go';
import { pushToast } from './toast';
import { activeRef } from './terminal-drawer-view';
import { parseScopedThreadKey, scopedThreadKey, type ScopedThreadRef } from './terminal-ui-state';
import { chordWinners, type DispatchAdd } from './keyboard-dispatch';
import { panelState, type PanelState, type Surface } from './r4-surfaces-panel';
import { readSnapshot, type PreviewNavStatus, type PreviewSessionSnapshot, type PreviewViewportSetting } from './browser-state';
import { normalizePreviewUrl } from './browser-url';
import { browserHistory, formatRelativeTimeLabel, recentUrlLabel } from './browser-history';
import { discoveredServers, environmentHostname, getConfiguredPreviewUrls, preparedConnection, previewableServers, resolveDiscoveredServerUrl, watchDiscoveredServers } from './browser-targets';
import {
  FILL_PREVIEW_VIEWPORT, PREVIEW_VIEWPORT_PRESETS, browserResponsiveViewportForToggle, browserViewportSettingKey, commitViewportAndAspectRatio, isValidViewport,
  normalizeZoomFactor, resizeBrowserViewportFromRail, resizeFreeformViewport, resolveBrowserDeviceViewportArea, resolveBrowserDeviceViewportLayout, resolvePreviewViewport,
  runBrowserViewportMutation, zoomLabel, type BrowserViewportResizeDirection,
} from './browser-viewport';
import { addBrowserSurface, browserHost, browserTabFavicon, effectiveNav, nativeTabs, openBrowserIn, type NativeTab } from './browser-surface';
import { browserOpenDefaults } from './browser-defaults';

// ── Per-client state the reference keeps in components (HostedBrowserWebview, ZoomIndicator) ──────
/** `now`: the latest projection's wall time (a data source reads no clock), the time a visit an op makes is recorded at. */
type Nav = { zoomSeen: Map<string, number>; zoomSerial: Map<string, number>; viewportSerial: Map<string, number>; aspectLocked: Set<string>; now: number };
const navs = new WeakMap<T3Client, Nav>();
function nav(client: T3Client): Nav {
  let value = navs.get(client);
  if (!value) { value = { zoomSeen: new Map(), zoomSerial: new Map(), viewportSerial: new Map(), aspectLocked: new Set(), now: 0 }; navs.set(client, value); }
  return value;
}

/** The module's zoom and appearance for a tab (desktopOverlay.zoomFactor/colorScheme); 100% and System until it reports. */
export const tabZoom = (tab: NativeTab | null): number => normalizeZoomFactor(tab?.zoomFactor ?? 1);
export const tabColorScheme = (tab: NativeTab | null): 'system' | 'light' | 'dark' => tab?.colorScheme === 'light' || tab?.colorScheme === 'dark' ? tab.colorScheme : 'system';
const viewportOf = (snapshot: PreviewSessionSnapshot | null): PreviewViewportSetting => snapshot?.viewport && typeof snapshot.viewport === 'object' ? snapshot.viewport : FILL_PREVIEW_VIEWPORT;

// ── Effects before the projection (PreviewView's effects, PreviewEmptyState's query) ───────────
export async function navigationPrepare(client: T3Client, native: Native, state: PanelState): Promise<void> {
  const ref = activeRef(client), history = browserHistory(client.local);
  if (!ref) return;
  // No project registration here: at 1e2ecbd975 nothing calls registerThreadProject (de34391427 removed ChatView's
  // effect), so a thread's visits wait unregistered (user decision 2026-10-09: match the reference).
  // PreviewView: a settled page titles the visit it came from (agent-driven pages only enrich an existing one).
  const host = environmentHostname(client), sessions = browserHost(client).store.read(ref).sessions;
  if (history.threadRecentHistory(ref, 1).length) for (const tabId of Object.keys(sessions)) {
    const { nav: status } = effectiveNav(client, ref, tabId);
    if (status._tag === 'Success' && status.title) history.setTitleForThreadUrl(ref, status.url, status.title, host);
  }
  // The live server list lasts while a Browser tab of this thread shows its empty state.
  const active = state.visible ? state.surfaces.find(surface => surface.id === state.active) : undefined;
  const empty = active?.kind === 'browser' && !!active.browser && effectiveNav(client, ref, active.browser.tabId).nav._tag === 'Idle';
  if (client.connection === 'connected') await watchDiscoveredServers(client, native, empty, configuredUrls(client));
}
const configuredUrls = (client: T3Client): ReadonlyArray<string> =>
  getConfiguredPreviewUrls(arr(client.shell.projects.find(entry => entry.id === client.projectId)?.scripts));

/** URL.canParse, without the static (the app's JS runtime has `URL` but not every newer static). */
const canParse = (url: string): boolean => { try { new URL(url); return true; } catch { return false; } };

// ── The projection's part-2 fields ──────────────────────────────────────────────────────────────
export type BrowserRecent = { url: string; title: string; subtitle: string; favicon: string; fallback: string; removeLabel: string };
export type BrowserServer = { key: string; title: string; description: string; url: string; favicon: string; fallback: string };
export type BrowserPresetChoice = { id: string; label: string; detail: string };
export type BrowserNavigationView = {
  zoomFactor: number; zoomLabel: string; zoomSerial: number; colorScheme: string;
  viewportMode: string; viewportWidth: number; viewportHeight: number; viewportKey: string; viewportSerial: number; presetId: string; presetLabel: string; aspectLocked: boolean;
  presets: BrowserPresetChoice[]; keys: string; recents: BrowserRecent[]; servers: BrowserServer[];
};
const PRESET_CHOICES: BrowserPresetChoice[] = PREVIEW_VIEWPORT_PRESETS.map(preset => ({ id: preset.id, label: preset.label, detail: preset.detail }));
export const emptyNavigationView = (): BrowserNavigationView => ({
  zoomFactor: 1, zoomLabel: '100%', zoomSerial: 0, colorScheme: 'system', viewportMode: 'fill', viewportWidth: 0, viewportHeight: 0, viewportKey: 'fill', viewportSerial: 0,
  presetId: '', presetLabel: 'Responsive', aspectLocked: false, presets: PRESET_CHOICES.map(choice => ({ ...choice })), keys: '', recents: [], servers: [],
});

/** The chords the module answers while the preview has the focus (`when: previewFocus`), as `chord=command` words. */
const PREVIEW_COMMANDS: Record<string, string> = { 'preview.refresh': 'refresh', 'preview.focusUrl': 'focus-url', 'preview.zoomIn': 'zoom-in', 'preview.zoomOut': 'zoom-out', 'preview.resetZoom': 'reset-zoom' };
export function previewKeys(client: T3Client): string {
  const winners = chordWinners(arr(client.config.keybindings), { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false,
    settingsOpen: false, diffOpen: false, previewFocus: true });
  return [...winners].filter(([, command]) => PREVIEW_COMMANDS[command]).map(([chord, command]) => `${chord}=${PREVIEW_COMMANDS[command]}`).join(' ');
}

/** A recent site's or a local server's icon: a Browser tab's captured icon of that origin, the public service, the globe. */
function siteIcon(client: T3Client, url: string): { favicon: string; fallback: string } {
  const status: PreviewNavStatus = { _tag: 'Success', url, title: '' };
  let captured = '';
  for (const tab of Object.values(nativeTabs(client))) {
    const icon = browserTabFavicon(status, tab.favicon);
    if (icon.captured) { captured = icon.captured; break; }
  }
  return { favicon: captured, fallback: browserTabFavicon(status, null).fallback };
}

export function navigationView(client: T3Client, ref: ScopedThreadRef, runtimeId: string, tab: NativeTab | null, snapshot: PreviewSessionSnapshot | null, empty: boolean, at: number): BrowserNavigationView {
  const state = nav(client), zoom = tabZoom(tab);
  if (at > 0) state.now = Math.max(state.now, at);
  const now = state.now;
  // ZoomIndicator: a change of the tab's zoom shows the pill; the first value it sees does not.
  const seen = state.zoomSeen.get(runtimeId);
  if (tab && seen !== undefined && Math.abs(seen - zoom) > 0.001) state.zoomSerial.set(runtimeId, (state.zoomSerial.get(runtimeId) ?? 0) + 1);
  if (tab) state.zoomSeen.set(runtimeId, zoom);
  const viewport = viewportOf(snapshot);
  const preset = viewport._tag === 'preset' ? PREVIEW_VIEWPORT_PRESETS.find(candidate => candidate.id === viewport.presetId) : undefined;
  const view: BrowserNavigationView = {
    ...emptyNavigationView(), zoomFactor: zoom, zoomLabel: zoomLabel(zoom), zoomSerial: state.zoomSerial.get(runtimeId) ?? 0, colorScheme: tabColorScheme(tab),
    viewportMode: viewport._tag, viewportWidth: viewport._tag === 'fill' ? 0 : viewport.width, viewportHeight: viewport._tag === 'fill' ? 0 : viewport.height,
    viewportKey: browserViewportSettingKey(viewport), viewportSerial: state.viewportSerial.get(runtimeId) ?? 0, presetId: preset?.id ?? '', presetLabel: preset?.label ?? 'Responsive',
    aspectLocked: viewport._tag !== 'fill' && state.aspectLocked.has(runtimeId), keys: previewKeys(client),
  };
  if (!empty) return view;
  // PreviewEmptyState: Recently used (parseable, at most 8) and the live Local servers.
  view.recents = browserHistory(client.local).threadRecentHistory(ref, 50).filter(entry => canParse(entry.url)).slice(0, 8).map(entry => {
    const label = recentUrlLabel(entry.url), visitedAt = formatRelativeTimeLabel(entry.lastVisitedAt, now);
    return { url: entry.url, title: entry.title ?? label, subtitle: `${entry.title ? `${label} · ` : ''}${visitedAt}`, ...siteIcon(client, entry.url), removeLabel: `Remove ${label} from history` };
  });
  const discovery = discoveredServers(client);
  view.servers = previewableServers(ref.environmentId, discovery.servers, configuredUrls(client), discovery.configuredUrlProbing, preparedConnection(client)).map(server => {
    const icon = siteIcon(client, server.requestedUrl);
    return { key: `${server.host}:${server.port}`, title: server.processName ?? 'Listening', description: `${server.host}:${server.port}`, url: server.requestedUrl, favicon: icon.favicon, fallback: icon.fallback };
  });
  return view;
}

// ── Commands ────────────────────────────────────────────────────────────────────────────────────
/** The time an op's visit is recorded at: the latest projection's wall time. */
export const navigationNow = (client: T3Client): number => nav(client).now;
/** The module's `browserSet`: an absolute zoom, a ladder step taken from the page's own zoom (`zoomStep`), or an appearance. */
async function nativeSet(client: T3Client, native: Native, runtimeId: string, change: { zoom?: number; zoomStep?: 'in' | 'out'; colorScheme?: string }): Promise<void> {
  await client.raw(native, { op: 'browserSet', tab: runtimeId, ...change });
}

/** handleViewportChange: `preview.resize`, its answer applied; serialized per tab; a failure toasts. The serial lets the view drop a drag it held. */
async function commitViewport(client: T3Client, native: Native, ref: ScopedThreadRef, tabId: string, runtimeId: string, setting: PreviewViewportSetting, locked?: boolean): Promise<void> {
  const state = nav(client);
  try {
    if (!isValidViewport(setting)) throw new Error('The viewport must be 240 to 3,840 pixels a side and at most 3,840 × 2,160 in area.');
    await commitViewportAndAspectRatio(setting, locked === undefined ? null : locked ? 1 : null, next => runBrowserViewportMutation(runtimeId, async () => {
      const snapshot = readSnapshot(await client.rpc(native, 'preview.resize', { threadId: ref.threadId, tabId, viewport: next }, true));
      if (snapshot) browserHost(client).store.updateServerSnapshot(ref, snapshot);
    }), ratio => { if (locked !== undefined) { if (ratio === null) state.aspectLocked.delete(runtimeId); else state.aspectLocked.add(runtimeId); } });
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Unable to resize browser viewport', description: error instanceof Error ? error.message : 'An error occurred.' });
  } finally { state.viewportSerial.set(runtimeId, (state.viewportSerial.get(runtimeId) ?? 0) + 1); }
}

/** A drag or key that asks for no change still answers: the view drops a size it held for it (`viewportSerial`). */
const settleViewport = (client: T3Client, runtimeId: string) => { const state = nav(client); state.viewportSerial.set(runtimeId, (state.viewportSerial.get(runtimeId) ?? 0) + 1); };

const DIRECTIONS: ReadonlySet<string> = new Set(['north', 'northeast', 'east', 'southeast', 'south', 'southwest', 'west', 'northwest']);
const numbers = (value: string): number[] => value.split('|').map(Number);

/** ⇧⌘J (togglePreviewPanel): a Browser panel closes (the dispatch row's panel-close); otherwise the thread's active tab shows, or a new one opens. */
export async function showPreview(client: T3Client, native: Native, state: PanelState): Promise<string> {
  const ref = activeRef(client);
  const activeTab = ref ? browserHost(client).store.read(ref).activeTabId : null;
  // rightPanelStore.openBrowser: the thread's active preview session, its surface added when the panel has none.
  if (ref && activeTab) {
    client.diffOpen = false;
    openBrowserIn(state, activeTab, scopedThreadKey(ref));
    return '';
  }
  return addBrowserSurface(client, native, state);
}

/** The part-2 ops of one Browser tab (`surface-browser-<op>`); null for any other op. */
export async function navigationLocal(client: T3Client, native: Native, surface: Surface, op: string, value: string): Promise<string | null> {
  const ref = surface.browser ? parseScopedThreadKey(surface.browser.threadKey) : null;
  if (!ref || !surface.browser) return null;
  const tabId = surface.browser.tabId, { runtimeId, tab, snapshot } = effectiveNav(client, ref, tabId), history = browserHistory(client.local);
  const viewport = viewportOf(snapshot), zoom = tabZoom(tab), locked = nav(client).aspectLocked.has(runtimeId);
  const aspect = locked && viewport._tag !== 'fill' ? viewport.width / viewport.height : undefined;
  switch (op) {
    case 'open-url': {
      // handleOpenServerUrl (a Recently used row or a Local server): the server's address resolved for this environment; history keeps the requested one.
      const resolved = resolveDiscoveredServerUrl(ref.environmentId, value, preparedConnection(client));
      let url: string;
      try { url = normalizePreviewUrl(resolved); } catch { return ''; }
      await client.raw(native, { op: 'browserNavigate', tab: runtimeId, url, profile: snapshot?.profileId ?? 'default', environment: ref.environmentId });
      browserHost(client).store.rememberUrl(ref, url);
      history.recordVisitForThread(ref, value, navigationNow(client), environmentHostname(client));
      return '';
    }
    case 'remove-recent': history.removeUrlForThread(ref, value); return '';
    // Manager.ts applyZoom steps the tab's own zoom: the module steps the page's, so held ⌘= never steps from a stale report.
    case 'zoom-in': case 'zoom-out': await nativeSet(client, native, runtimeId, { zoomStep: op === 'zoom-in' ? 'in' : 'out' }); return '';
    case 'zoom-reset': await nativeSet(client, native, runtimeId, { zoom: 1 }); return '';
    case 'color-scheme': if (['system', 'light', 'dark'].includes(value)) await nativeSet(client, native, runtimeId, { colorScheme: value }); return '';
    case 'device-toolbar': {
      // handleToggleDeviceToolbar: a fixed viewport goes back to fill; fill opens at the configured default (part 4's
      // Settings › Integrations › Browser row), else the panel's framed area.
      if (viewport._tag !== 'fill') { await commitViewport(client, native, ref, tabId, runtimeId, FILL_PREVIEW_VIEWPORT); return ''; }
      const [width = 0, height = 0] = numbers(value);
      await commitViewport(client, native, ref, tabId, runtimeId, browserResponsiveViewportForToggle({ defaults: browserOpenDefaults(client), panelRect: width > 0 && height > 0 ? { width, height } : null, zoomFactor: zoom }));
      return '';
    }
  }
  // A viewport op that finds fill (the toolbar's view was older) still answers, so the view drops what it drew.
  if (viewport._tag === 'fill') { if (!op.startsWith('viewport-')) return null; settleViewport(client, runtimeId); return ''; }
  switch (op) {
    case 'viewport-preset': {
      // BrowserDeviceToolbar selectViewport: Responsive keeps the size as freeform; a preset opens portrait, the lock follows its ratio.
      if (value === 'responsive') { if (viewport._tag !== 'freeform') await commitViewport(client, native, ref, tabId, runtimeId, { _tag: 'freeform', width: viewport.width, height: viewport.height }); return ''; }
      if (!PREVIEW_VIEWPORT_PRESETS.some(candidate => candidate.id === value)) return '';
      await commitViewport(client, native, ref, tabId, runtimeId, resolvePreviewViewport({ mode: 'preset', preset: value }), locked);
      return '';
    }
    case 'viewport-size': {
      // applyCustomSize: whole numbers within the bounds, and only a change.
      const [width = Number.NaN, height = Number.NaN] = numbers(value);
      const next: PreviewViewportSetting = { _tag: 'freeform', width, height };
      if (!isValidViewport(next) || (width === viewport.width && height === viewport.height)) { settleViewport(client, runtimeId); return ''; }
      await commitViewport(client, native, ref, tabId, runtimeId, next);
      return '';
    }
    case 'viewport-rotate': {
      // rotate: a valid typed size that differs is rotated as freeform; otherwise the setting itself (a preset stays one).
      const [width = Number.NaN, height = Number.NaN] = numbers(value);
      const custom = isValidViewport({ _tag: 'freeform', width, height }) && (width !== viewport.width || height !== viewport.height);
      const source = custom ? { _tag: 'freeform' as const, width, height } : viewport;
      await commitViewport(client, native, ref, tabId, runtimeId, { ...source, width: source.height, height: source.width }, locked);
      return '';
    }
    case 'viewport-lock': { const state = nav(client); if (locked) state.aspectLocked.delete(runtimeId); else state.aspectLocked.add(runtimeId); return ''; }
    case 'viewport-close': await commitViewport(client, native, ref, tabId, runtimeId, FILL_PREVIEW_VIEWPORT, false); return '';
    case 'viewport-drag': {
      // A rail's drag (useBrowserViewportResize): its pointer total since the press, against the panel it was made in.
      const [direction = '', ...rest] = value.split('|');
      const [dx = 0, dy = 0, panelWidth = 0, panelHeight = 0] = rest.map(Number);
      if (!DIRECTIONS.has(direction) || !(panelWidth > 0 && panelHeight > 0)) return '';
      const container = { width: panelWidth, height: panelHeight }, layout = resolveBrowserDeviceViewportLayout(container, viewport, zoom);
      const next = resizeBrowserViewportFromRail(viewport, { x: dx, y: dy }, resolveBrowserDeviceViewportArea(container), zoom * layout.viewportScale, direction as BrowserViewportResizeDirection, aspect);
      if (next.width !== viewport.width || next.height !== viewport.height) await commitViewport(client, native, ref, tabId, runtimeId, { _tag: 'freeform', ...next });
      else settleViewport(client, runtimeId); // the view stops holding the size it drew
      return '';
    }
    case 'viewport-key': {
      // handleResizeKeyDown: 10 px (50 with Shift) at the page's zoom, along the axes the rail controls.
      const [direction = '', key = '', shift = ''] = value.split('|');
      if (!DIRECTIONS.has(direction)) return '';
      const controlsWidth = direction.includes('east') || direction.includes('west'), controlsHeight = direction.includes('north') || direction.includes('south');
      const step = (shift === 'true' ? 50 : 10) * zoom;
      const delta = key === 'ArrowLeft' && controlsWidth ? { x: -step, y: 0 } : key === 'ArrowRight' && controlsWidth ? { x: step, y: 0 }
        : key === 'ArrowUp' && controlsHeight ? { x: 0, y: -step } : key === 'ArrowDown' && controlsHeight ? { x: 0, y: step } : null;
      if (!delta) return '';
      const next = resizeFreeformViewport(viewport, delta, zoom, direction as BrowserViewportResizeDirection, aspect);
      if (next.width !== viewport.width || next.height !== viewport.height) await commitViewport(client, native, ref, tabId, runtimeId, { _tag: 'freeform', ...next });
      else settleViewport(client, runtimeId);
      return '';
    }
  }
  return null;
}

/** preview.toggle (⇧⌘J, global): closes a shown Browser panel, else shows the thread's Browser tab. */
export function previewToggleRow(add: DispatchAdd, client: T3Client): void {
  if (!client.environmentId) return;
  const state = panelState(client), active = state.surfaces.find(surface => surface.id === state.active);
  if (state.visible && active?.kind === 'browser') add('preview.toggle', 'panel-close', '', 'Toggle Browser');
  else add('preview.toggle', 'command', 'shelllocal:surface-browser-show', 'Toggle Browser');
}

/** The module's reported zoom and appearance, read with the rest of a tab (browser-surface.ts nativeTabs). */
export const readTabNavigation = (raw: unknown): { zoomFactor: number; colorScheme: string } => {
  const value = obj(raw);
  return { zoomFactor: Number(value.zoomFactor) || 1, colorScheme: str(value.colorScheme, 'system') };
};
