// Every Browser tab's page as a partitioned `iframe` (LLP 1020.002; MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/web/src/browser/ElectronBrowserHost.tsx, BrowserSurfaceSlot.tsx; apps/desktop/src/preview/Manager.ts
// `computeNavStatus`, `failed`, `refresh`, `hardReload`, `openDevTools`; BrowserSession.ts's partitions).
//
// The reference mounts a `<webview>` per live preview session at the app root and positions the one a slot shows.
// Here the window's frame layer (browser-frames.contract) mounts an `iframe` per live session with the session's
// partition, and places the page in the box the stage measures or at the floating player's frame. This file is the data
// module's half: each tab's frame (its id, partition, mount URL, zoom and appearance), the page state its records
// give (the reference's nav status), the commands the frames take (a queue the root's `frameCommand` task runs one at
// a time), and clearing a profile's partitions.
//
// Partitions: each environment's profile is one `persist:` partition (the reference's `persist:t3code-preview-<scope>`),
// named by a hash of the environment and the profile, since a partition name is at most 64 of [A-Za-z0-9._-];
// Incognito is an in-memory partition per environment. The framework derives the stores from the app and the name,
// so they are not the stores the module kept before: a person signs in again once.
import type { T3Client } from '../client/client';
import { letGo } from '../client/let-go';
// The bake binds the package from its exact.json (`__exact_modules`); a bare tsc and Bun read the exact.json itself, so
// the call is typed here and is undefined under Bun's tests.
import * as browserPackage from '@exact/browser';

export type BrowserCalls = { clearSiteData(options: { partition: string; types: string[] }): Promise<{ partition: string; types: string[] }> };
let browserCalls = browserPackage as unknown as BrowserCalls;
/** A test's own `@exact/browser` (null puts the package back). */
export function useBrowserCalls(fake: BrowserCalls | null): void { browserCalls = fake ?? browserPackage as unknown as BrowserCalls; }
import { INCOGNITO_BROWSER_PROFILE_ID, type BrowserProfileDataBridge } from './browser-profiles';

/** `FrameNavigation` (LLP 1020.002 D2), as the frame layer forwards it. */
export type FrameRecord = {
  url: string; destination: string; navigationId: number; navigationType: string; title: string; icons: string[];
  canGoBack: boolean; canGoForward: boolean; loading: boolean; controller: string; error: string; message: string; recovering: boolean;
};
export type FrameFailure = { url: string; code: number; description: string };
/** One tab's frame and what its records said. */
export type FrameEntry = {
  frameId: string; runtimeId: string; partition: string; environment: string; profile: string;
  /** The URL the frame mounted at; it never changes while mounted (a command moves the page). */
  src: string;
  record: FrameRecord | null;
  /** The destination of the navigation in flight, and its id. */
  pending: string; pendingId: number;
  failure: FrameFailure | null; failures: number;
  zoom: number; colorScheme: 'system' | 'light' | 'dark';
  /** The page's icon (its first candidate) and the page it came from. */
  favicon: { dataUrl: string; pageUrl: string; capturedAt: number } | null;
  crashed: boolean;
  /** The frame's `controller` ("person" or "agent", D9). */
  controller: string;
  /** Records the frame sent, counted: a test or a waiter reads it. */
  events: number;
  /** The last record's event and the navigation that committed a document last (its `currententrychange`). */
  event: string; committedId: number;
  /** A dialog the frame holds while it is the agent's and no agent run uses it, waiting on the person (D9 `userpromptopened`). */
  prompt: FramePrompt | null;
};
export type FramePrompt = { type: string; title: string; message: string; defaultValue: string };
/** `window`: the window's own View command a preview chord fell through to, `reload()` (`flag`) or the zoom step `delta`. */
export type FrameCommand = { serial: number; kind: 'navigate' | 'traverse' | 'reload' | 'stop' | 'devtools' | 'window' | ''; id: string; url: string; delta: number; flag: boolean };
const NO_COMMAND: FrameCommand = { serial: 0, kind: '', id: '', url: '', delta: 0, flag: false };

type Waiter = { frameId: string; test: (entry: FrameEntry, event: string) => boolean; done: (ok: boolean) => void };
/** A wait of `ms` on the app's clock (command-waits.ts `sleep`). */
export type Sleep = (ms: number) => Promise<void>;
type Frames = {
  entries: Map<string, FrameEntry>; byFrame: Map<string, string>;
  queue: FrameCommand[]; serial: number;
  /** Partitions to name with a blank frame before they are cleared (no frame named them this session), and the types to clear. */
  namers: Map<string, Set<string>>;
  waiters: Set<Waiter>;
};
const all = new WeakMap<object, Frames>();
export function frames(owner: object): Frames {
  let value = all.get(owner);
  if (!value) { value = { entries: new Map(), byFrame: new Map(), queue: [], serial: 0, namers: new Map(), waiters: new Set() }; all.set(owner, value); }
  return value;
}

// ── Names ──────────────────────────────────────────────────────────────────────────────────────
/** FNV-1a over UTF-16 units, twice with two offsets: 16 hex digits, enough to keep names apart. */
export function shortHash(text: string): string {
  let a = 0x811c9dc5, b = 0x01000193 ^ 0x5bd1e995;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    a = Math.imul(a ^ c, 0x01000193) >>> 0;
    b = Math.imul(b ^ c, 0x01000193) >>> 0;
  }
  return a.toString(16).padStart(8, '0') + b.toString(16).padStart(8, '0');
}
/** The frame's element id: the runtime id is a JSON string, so the id is its hash. */
export const frameIdOf = (runtimeId: string): string => `browser-frame-${shortHash(runtimeId)}`;
/** A profile's partition in an environment (D1's names: `persist:` and 1 to 64 of [A-Za-z0-9._-]). */
export function partitionOf(environment: string, profile: string): string {
  return profile === INCOGNITO_BROWSER_PROFILE_ID ? `t3-incognito-${shortHash(environment)}` : `persist:t3-preview-${shortHash(`${environment}\u0000${profile}`)}`;
}

// ── The live set ───────────────────────────────────────────────────────────────────────────────
export type LiveFrame = { runtimeId: string; url: string; environment: string; profile: string; zoom: number; colorScheme: 'system' | 'light' | 'dark' };
/** ElectronBrowserHost: a frame for every live session, at its last URL when it mounts; the rest unmount. A new frame
 *  starts at the given zoom and appearance (the configured defaults, desktopTabLifetime's `browserDefaultTabState`). */
export function syncFrames(owner: object, live: readonly LiveFrame[]): void {
  const store = frames(owner), keep = new Set<string>();
  for (const tab of live) {
    keep.add(tab.runtimeId);
    if (store.entries.has(tab.runtimeId)) continue;
    const frameId = frameIdOf(tab.runtimeId), src = /^https?:\/\//i.test(tab.url) ? tab.url : 'about:blank';
    store.entries.set(tab.runtimeId, {
      frameId, runtimeId: tab.runtimeId, partition: partitionOf(tab.environment, tab.profile), environment: tab.environment, profile: tab.profile, src,
      record: null, pending: src === 'about:blank' ? '' : src, pendingId: 0, failure: null, failures: 0, zoom: tab.zoom, colorScheme: tab.colorScheme,
      favicon: null, crashed: false, controller: 'person', events: 0, event: '', committedId: 0, prompt: null,
    });
    store.byFrame.set(frameId, tab.runtimeId);
  }
  for (const [runtimeId, entry] of store.entries) {
    if (keep.has(runtimeId)) continue;
    store.entries.delete(runtimeId); store.byFrame.delete(entry.frameId);
    store.queue = store.queue.filter(command => command.id !== entry.frameId);
  }
}
export const frameEntry = (owner: object, runtimeId: string): FrameEntry | null => frames(owner).entries.get(runtimeId) ?? null;
export const frameByElement = (owner: object, frameId: string): FrameEntry | null => {
  const store = frames(owner), runtimeId = store.byFrame.get(frameId);
  return runtimeId ? store.entries.get(runtimeId) ?? null : null;
};

// ── Records (computeNavStatus, failed) ─────────────────────────────────────────────────────────
/** The record the frame layer sends: a query string, its strings URI-encoded (browser-frames.contract `packed`). */
export function readRecord(packed: string): { event: string; record: FrameRecord; extra: URLSearchParams } {
  const q = new URLSearchParams(packed), flag = (key: string) => q.get(key) === 'true';
  return {
    event: q.get('event') ?? '', extra: q,
    record: {
      url: q.get('url') ?? '', destination: q.get('destination') ?? '', navigationId: Number(q.get('id')) || 0, navigationType: q.get('type') ?? '',
      title: q.get('title') ?? '', icons: (q.get('icons') ?? '').split('\n').filter(Boolean), canGoBack: flag('back'), canGoForward: flag('forward'),
      loading: flag('loading'), controller: q.get('controller') || 'person', error: q.get('error') ?? '', message: q.get('message') ?? '', recovering: flag('recovering'),
    },
  };
}

/** The Chromium net error a WebKit error stands for (net_error_list.h), so the load-failed page words it as the reference does. */
const CHROMIUM: Record<number, string> = {
  [-1003]: 'ERR_NAME_NOT_RESOLVED', [-1006]: 'ERR_NAME_RESOLUTION_FAILED', [-1004]: 'ERR_CONNECTION_REFUSED', [-1005]: 'ERR_CONNECTION_CLOSED',
  [-1001]: 'ERR_TIMED_OUT', [-1009]: 'ERR_INTERNET_DISCONNECTED', [-1202]: 'ERR_CERT_AUTHORITY_INVALID', [-1203]: 'ERR_CERT_AUTHORITY_INVALID',
  [-1201]: 'ERR_CERT_DATE_INVALID', [-1204]: 'ERR_CERT_DATE_INVALID', [-1200]: 'ERR_SSL_PROTOCOL_ERROR', [-1206]: 'ERR_SSL_CLIENT_AUTH_CERT_NEEDED',
  [-1007]: 'ERR_TOO_MANY_REDIRECTS', [-1002]: 'ERR_UNKNOWN_URL_SCHEME', [-1000]: 'ERR_INVALID_URL', [-1011]: 'ERR_INVALID_RESPONSE', [-1014]: 'ERR_EMPTY_RESPONSE',
};
/** A `navigateerror` message, "<domain> <code>: <text>" (the arm's), as the failure the load-failed page shows; null when it
 *  is no failure (Manager.ts `failed`: WebKit's "plug-in handled load", 204, ends a load without one). */
export function failureOf(url: string, message: string): FrameFailure | null {
  const match = /^(\S+) (-?\d+): ?(.*)$/s.exec(message);
  if (!match) return { url, code: 0, description: message || 'The page could not be loaded.' };
  const [, domain, raw, text] = match, code = Number(raw);
  if ((domain === 'WebKitErrorDomain' || domain === 'WKErrorDomain') && (code === 102 || code === 204)) return null;
  return { url, code, description: (domain === 'NSURLErrorDomain' ? CHROMIUM[code] : undefined) ?? text };
}

const originOf = (url: string): string | null => { try { const parsed = new URL(url); return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.origin : null; } catch { return null; } };

/** One record from a frame: the tab's page state moves as the module's did (a cancelled or superseded load is no failure;
 *  a failure keeps its URL until the next navigation starts). Returns whether a tab took it. */
export function adoptFrameEvent(owner: object, frameId: string, packed: string, now: number): boolean {
  const entry = frameByElement(owner, frameId);
  if (!entry) return false;
  const { event, record } = readRecord(packed);
  if (!['navigate', 'currententrychange', 'navigatesuccess', 'navigateerror'].includes(event)) return false;
  entry.events++;
  entry.record = record;
  entry.event = event;
  if (event === 'currententrychange' && record.navigationId) entry.committedId = record.navigationId;
  entry.controller = record.controller;
  switch (event) {
    case 'navigate':
      entry.pending = record.destination; entry.pendingId = record.navigationId; entry.failure = null; entry.crashed = false;
      break;
    case 'currententrychange':
      if (!record.loading) entry.pending = '';
      break;
    case 'navigatesuccess':
      entry.pending = '';
      break;
    case 'navigateerror':
      if (record.error === 'crashed') { entry.crashed = !record.recovering; break; }
      if (record.error === 'NetworkError') {
        const failure = failureOf(entry.pending || record.url, record.message);
        if (failure) { entry.failure = failure; entry.failures++; }
      }
      if (record.error !== 'InvalidStateError' && (record.navigationId === entry.pendingId || !record.loading)) entry.pending = '';
      break;
  }
  // A committed document of another origin drops the old icon (Manager.ts clearFavicon); its first candidate is the tab's.
  const icon = record.icons[0] ?? '';
  if (icon && originOf(record.url)) entry.favicon = { dataUrl: icon, pageUrl: record.url, capturedAt: now };
  else if (entry.favicon && originOf(entry.favicon.pageUrl) !== originOf(record.url)) entry.favicon = null;
  for (const waiter of [...frames(owner).waiters]) if (waiter.frameId === frameId && waiter.test(entry, event)) settle(owner, waiter, true);
  return true;
}

/** The tab's page state (computeNavStatus): Idle until a page loads, Loading while one does, Success after, LoadFailed. */
export function frameNav(entry: FrameEntry): { kind: 'Idle' | 'Loading' | 'Success' | 'LoadFailed'; url: string; title: string } {
  const title = entry.record?.title ?? '';
  if (entry.failure) return { kind: 'LoadFailed', url: entry.failure.url, title };
  const current = entry.pending || entry.record?.url || '';
  if (!current || current === 'about:blank') return { kind: 'Idle', url: '', title: '' };
  return { kind: entry.pending || entry.record?.loading ? 'Loading' : 'Success', url: current, title };
}

/** Waits for a frame's record that passes `test`, at most `ms`; false when it never came. */
export function waitForFrame(owner: object, frameId: string, test: (entry: FrameEntry, event: string) => boolean, sleep: Sleep, ms: number): Promise<boolean> {
  return new Promise(done => {
    const waiter: Waiter = { frameId, test, done };
    frames(owner).waiters.add(waiter);
    void sleep(ms).then(() => settle(owner, waiter, false));
  });
}
function settle(owner: object, waiter: Waiter, ok: boolean): void {
  if (frames(owner).waiters.delete(waiter)) waiter.done(ok);
}

// ── Commands (Manager.ts navigate / goBack / goForward / refresh / hardReload / openDevTools) ────
export function enqueue(owner: object, command: Omit<FrameCommand, 'serial'>): void {
  const store = frames(owner);
  store.queue.push({ ...command, serial: ++store.serial });
}
/** The command the root's `frameCommand` task runs next (one at a time: it acknowledges each). */
export const frameCommandView = (owner: object): FrameCommand => frames(owner).queue[0] ?? NO_COMMAND;
/** The root ran the command with this serial. */
export function frameCommandDone(owner: object, serial: number): void {
  const store = frames(owner);
  store.queue = store.queue.filter(command => command.serial > serial);
}
export function navigateFrame(owner: object, runtimeId: string, url: string): boolean {
  const entry = frameEntry(owner, runtimeId);
  if (!entry || !/^https?:\/\//i.test(url)) return false;
  entry.failure = null; entry.crashed = false;
  enqueue(owner, { kind: 'navigate', id: entry.frameId, url, delta: 0, flag: false });
  return true;
}
/** back, forward, refresh, hard-reload, stop and devtools (the platform's inspector for the page, opened or brought
 *  forward). Refresh restarts a load in flight and retries a failed one at its own URL
 *  (Electron's reload restarts the pending entry; Chromium's error page sits at the failed URL). */
export function commandFrame(owner: object, runtimeId: string, name: string): boolean {
  const entry = frameEntry(owner, runtimeId);
  if (!entry) return false;
  const again = entry.failure?.url || entry.pending;
  switch (name) {
    case 'back': case 'forward':
      if (!(name === 'back' ? entry.record?.canGoBack : entry.record?.canGoForward)) return true;
      enqueue(owner, { kind: 'traverse', id: entry.frameId, url: '', delta: name === 'back' ? -1 : 1, flag: false });
      return true;
    case 'refresh': case 'hard-reload':
      entry.crashed = false;
      if (again) return navigateFrame(owner, runtimeId, again);
      enqueue(owner, { kind: 'reload', id: entry.frameId, url: '', delta: 0, flag: name === 'hard-reload' });
      return true;
    case 'stop':
      enqueue(owner, { kind: 'stop', id: entry.frameId, url: '', delta: 0, flag: false });
      return true;
    case 'devtools':
      enqueue(owner, { kind: 'devtools', id: entry.frameId, url: '', delta: 0, flag: false });
      return true;
  }
  return false;
}

// ── Zoom and appearance (Manager.ts applyZoom, setColorScheme) ─────────────────────────────────
export const ZOOM_LEVELS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 5];
export const normalizedZoom = (value: number): number => Number.isFinite(value) ? ZOOM_LEVELS.reduce((best, step) => Math.abs(step - value) < Math.abs(best - value) ? step : best, 1) : 1;
/** Manager.ts nextZoomLevel, from the tab's own zoom. */
export function steppedZoom(current: number, up: boolean): number {
  const at = ZOOM_LEVELS.findIndex(step => Math.abs(step - current) < 0.001 || step > current);
  const step = at < 0 ? ZOOM_LEVELS.length - 1 : Math.abs(ZOOM_LEVELS[at] - current) < 0.001 ? at : at - 1;
  return up ? ZOOM_LEVELS[Math.min(step + 1, ZOOM_LEVELS.length - 1)] : ZOOM_LEVELS[Math.max(step - 1, 0)];
}
export function setFrame(owner: object, runtimeId: string, change: { zoom?: number; zoomStep?: 'in' | 'out'; colorScheme?: string }): boolean {
  const entry = frameEntry(owner, runtimeId);
  if (!entry) return false;
  if (change.zoom !== undefined) entry.zoom = normalizedZoom(change.zoom);
  if (change.zoomStep) entry.zoom = steppedZoom(entry.zoom, change.zoomStep === 'in');
  if (change.colorScheme === 'system' || change.colorScheme === 'light' || change.colorScheme === 'dark') entry.colorScheme = change.colorScheme;
  return true;
}

// ── Clearing (BrowserSession.ts clearCookies / clearCache) ─────────────────────────────────────
/** `clearSiteData` (D4) for a profile's partition in an environment. The framework clears only a partition a frame named
 *  this session: a partition no tab has used yet is named by a blank frame the layer mounts (`namers`), and cleared once
 *  that frame has loaded (`namerLoaded`). */
export async function clearFramePartition(owner: object, environment: string, profile: string, what: 'cookies' | 'cache'): Promise<void> {
  const partition = partitionOf(environment, profile), types = what === 'cookies' ? ['cookies', 'storage'] : ['cache'];
  try { await browserCalls.clearSiteData({ partition, types }); return; }
  catch (error) { if (letGo(error) || !String((error as { code?: string })?.code ?? (error as Error)?.message ?? '').includes('NotFound')) throw error; }
  const pending = frames(owner).namers.get(partition) ?? new Set<string>();
  for (const type of types) pending.add(type);
  frames(owner).namers.set(partition, pending);
}
/** The profile clearing Settings and the More menu use: cookies are cookies and storage (the reference's
 *  `clearStorageData` storages), the cache is the cache; only that environment's partition for that profile. */
export const frameProfileBridge = (owner: object): BrowserProfileDataBridge => ({
  clearCookies: (environmentId, profileId) => clearFramePartition(owner, environmentId, profileId, 'cookies'),
  clearCache: (environmentId, profileId) => clearFramePartition(owner, environmentId, profileId, 'cache'),
});
export const namerIdOf = (partition: string): string => `browser-namer-${shortHash(partition)}`;
/** A namer frame loaded: its partition is named, so the clears waiting on it run, and it unmounts. */
export async function namerLoaded(owner: object, frameId: string): Promise<void> {
  const store = frames(owner);
  for (const [partition, types] of store.namers) {
    if (namerIdOf(partition) !== frameId) continue;
    store.namers.delete(partition);
    await browserCalls.clearSiteData({ partition, types: [...types] }).catch((error: unknown) => { if (letGo(error)) throw error; });
  }
}

// ── The layer's projection (browser-frames.contract BrowserFrameView) ───────────────────────────
export type BrowserFrameView = {
  frameId: string; src: string; partition: string; zoom: number; colorScheme: string;
  /** The tab's environment and profile, which the hatch reads to move a profile's old cookies in (T3BrowserPage.swift). */
  environment: string; profile: string;
  /** Where it shows: "panel" (the stage's slot), "mini" (the floating player's frame), or "" (mounted, hidden). */
  place: string;
  /** A fixed viewport's CSS size (0 in fill): the frame lays out at it times its zoom and is scaled into the slot. */
  pageWidth: number; pageHeight: number;
  /** Hidden but recorded or shown in its separate window: it stays laid out at its last size, transparent. */
  held: boolean; heldWidth: number; heldHeight: number;
  /** The dialog it holds for the person (`FramePrompt`), "" when none. */
  promptType: string; promptTitle: string; promptMessage: string; promptDefault: string;
};
export type BrowserFramesView = { frames: BrowserFrameView[]; namers: BrowserFrameView[]; command: FrameCommand };
export const EMPTY_FRAME: BrowserFrameView = { frameId: '', src: 'about:blank', partition: '', zoom: 1, colorScheme: 'system', environment: '', profile: '', place: '', pageWidth: 0, pageHeight: 0,
  held: false, heldWidth: 0, heldHeight: 0, promptType: '', promptTitle: '', promptMessage: '', promptDefault: '' };
export const emptyFramesView = (): BrowserFramesView => ({ frames: [], namers: [], command: NO_COMMAND });
/** A hidden page that must keep drawing, and the size it last laid out at. */
export type FrameHold = (runtimeId: string) => { width: number; height: number } | null;
/** The layer's frames: the panel's tab placed in its slot (`panel`: its runtime id and fixed viewport, if it shows one), the
 *  floating player's at the player's frame, and the rest hidden, those `held` says keep drawing, transparent. */
export function frameLayerView(owner: object, panel: { runtimeId: string; pageWidth: number; pageHeight: number } | null, miniRuntimeId: string | null, held: FrameHold): BrowserFramesView {
  const store = frames(owner), layer: BrowserFrameView[] = [];
  for (const entry of store.entries.values()) {
    const place = panel?.runtimeId === entry.runtimeId ? 'panel' : entry.runtimeId === miniRuntimeId ? 'mini' : '';
    const hold = place ? null : held(entry.runtimeId), prompt = entry.prompt;
    layer.push({
      frameId: entry.frameId, src: entry.src, partition: entry.partition, zoom: entry.zoom, colorScheme: entry.colorScheme, environment: entry.environment, profile: entry.profile, place,
      pageWidth: place === 'panel' ? panel!.pageWidth : 0, pageHeight: place === 'panel' ? panel!.pageHeight : 0,
      held: hold !== null, heldWidth: hold?.width || 1280, heldHeight: hold?.height || 800,
      promptType: prompt?.type ?? '', promptTitle: prompt?.title ?? '', promptMessage: prompt?.message ?? '', promptDefault: prompt?.defaultValue ?? '',
    });
  }
  const namers = [...store.namers.keys()].map(partition => ({ ...EMPTY_FRAME, frameId: namerIdOf(partition), partition }));
  return { frames: layer, namers, command: frameCommandView(owner) };
}
