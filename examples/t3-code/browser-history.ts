// browser-surface part 2: the Browser tab's history (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// apps/web/src/browserHistoryStore.ts; apps/web/src/timestampFormat.ts `formatRelativeTimeLabel`).
//
// Visits are kept per project (the sidebar's logical project key, so history follows the project rows the
// person sees): at most 50 entries per project and 20 projects, a URL of at most 2,048 characters (without its
// credentials) and a title of at most 512. A visit made before the thread's project is known waits, at most 10
// per thread and 20 threads, and is slotted in by its time once the thread registers its project. Loopback
// aliases (localhost, 127.0.0.1, 0.0.0.0) and the environment's own host are one site.
//
// The reference persists `byProjectKey` and `projectKeyByThreadKey` in localStorage
// (`t3code:browser-history:v1`); here they are the client's saved preferences (`browserHistory`, written with
// the rest of client.local), read back through the same sanitizer (`mergeBrowserHistoryState`). The pending
// queues stay in memory, as the reference's `partialize` leaves them out.
//
// Registration: at 1e2ecbd975 nothing calls `registerThreadProject` (#2829, de34391427, dropped ChatView's
// effect), so a thread's visits wait in the pending queue and Recently used lists only the history of a thread whose
// project mapping was saved before that change. The clone does the same (user decision 2026-10-09: match the
// reference, de34391427): the store keeps the reference's API, and nothing registers a thread's project.
import { normalizePreviewUrl } from './browser-url';
import { isLocalLoopbackHost, normalizeHostname } from './host-classification';
import { scopedThreadKey, type ScopedThreadRef } from './terminal-ui-state';

export type BrowserHistoryEntry = { url: string; lastVisitedAt: number; title?: string };

export const BROWSER_HISTORY_MAX_ENTRIES_PER_PROJECT = 50;
export const BROWSER_HISTORY_MAX_PROJECTS = 20;
export const BROWSER_HISTORY_MAX_URL_LENGTH = 2048;
export const BROWSER_HISTORY_MAX_TITLE_LENGTH = 512;
const MAX_VALID_DATE_MS = 8_640_000_000_000_000;

export function isValidHistoryTimestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= MAX_VALID_DATE_MS;
}

export function normalizeHistoryUrl(raw: string): string | null {
  let parsed: URL;
  try { parsed = new URL(normalizePreviewUrl(raw)); } catch { return null; }
  parsed.username = parsed.password = '';
  return parsed.href.length > BROWSER_HISTORY_MAX_URL_LENGTH ? null : parsed.href;
}

function visitLookupKey(normalized: string, environmentHostname?: string | null): string {
  const parsed = new URL(normalized);
  const host = normalizeHostname(parsed.hostname);
  const environmentHost = environmentHostname && normalizeHostname(environmentHostname);
  if (isLocalLoopbackHost(host) || host === '0.0.0.0' || host === environmentHost) parsed.hostname = 'local';
  return parsed.href;
}

function titleLookupKey(normalized: string, environmentHostname?: string | null): string {
  const parsed = new URL(visitLookupKey(normalized, environmentHostname));
  if (parsed.pathname !== '/' && parsed.pathname.endsWith('/')) parsed.pathname = parsed.pathname.slice(0, -1);
  return parsed.href;
}

function isStableLocalUrl(normalized: string): boolean {
  const host = normalizeHostname(new URL(normalized).hostname);
  return isLocalLoopbackHost(host) || host === '0.0.0.0';
}

export function upsertHistoryEntry(entries: ReadonlyArray<BrowserHistoryEntry>, url: string, at: number,
  options?: { insertOrdered?: boolean; environmentHostname?: string | null }): BrowserHistoryEntry[] {
  const key = visitLookupKey(url, options?.environmentHostname);
  const existing = entries.find(candidate => visitLookupKey(candidate.url, options?.environmentHostname) === key);
  const rest = entries.filter(candidate => visitLookupKey(candidate.url, options?.environmentHostname) !== key);
  const visitedAt = options?.insertOrdered && existing && existing.lastVisitedAt > at ? existing.lastVisitedAt : at;
  const storedUrl = existing && (isStableLocalUrl(existing.url) || !isStableLocalUrl(url)) ? existing.url : url;
  const entry: BrowserHistoryEntry = existing ? { ...existing, url: storedUrl, lastVisitedAt: visitedAt } : { url, lastVisitedAt: visitedAt };
  if (!options?.insertOrdered) return [entry, ...rest].slice(0, BROWSER_HISTORY_MAX_ENTRIES_PER_PROJECT);
  const index = rest.findIndex(candidate => candidate.lastVisitedAt < entry.lastVisitedAt);
  const next = index === -1 ? [...rest, entry] : [...rest.slice(0, index), entry, ...rest.slice(index)];
  return next.slice(0, BROWSER_HISTORY_MAX_ENTRIES_PER_PROJECT);
}

export function evictExcessProjects(byProjectKey: Record<string, BrowserHistoryEntry[]>): Record<string, BrowserHistoryEntry[]> {
  const keys = Object.keys(byProjectKey);
  if (keys.length <= BROWSER_HISTORY_MAX_PROJECTS) return byProjectKey;
  const kept = [...keys].sort((a, b) => (byProjectKey[b]?.[0]?.lastVisitedAt ?? 0) - (byProjectKey[a]?.[0]?.lastVisitedAt ?? 0)).slice(0, BROWSER_HISTORY_MAX_PROJECTS);
  return Object.fromEntries(kept.map(key => [key, byProjectKey[key] ?? []]));
}

export function migratePersistedBrowserHistoryState(persistedState: unknown): { byProjectKey: Record<string, BrowserHistoryEntry[]> } {
  if (!persistedState || typeof persistedState !== 'object') return { byProjectKey: {} };
  const raw = (persistedState as { byProjectKey?: unknown }).byProjectKey;
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return { byProjectKey: {} };
  const byProjectKey: Record<string, BrowserHistoryEntry[]> = {};
  for (const [projectKey, value] of Object.entries(raw as Record<string, unknown>)) {
    if (!Array.isArray(value)) continue;
    const seenUrls = new Set<string>();
    const entries = value
      .flatMap<BrowserHistoryEntry>(candidate => {
        if (!candidate || typeof candidate !== 'object') return [];
        const { url, lastVisitedAt, title } = candidate as Record<string, unknown>;
        if (typeof url !== 'string') return [];
        const normalizedUrl = normalizeHistoryUrl(url);
        if (!normalizedUrl || !isValidHistoryTimestamp(lastVisitedAt)) return [];
        return [{ url: normalizedUrl, lastVisitedAt, ...(typeof title === 'string' && title.length > 0 ? { title: title.slice(0, BROWSER_HISTORY_MAX_TITLE_LENGTH) } : {}) }];
      })
      .sort((a, b) => b.lastVisitedAt - a.lastVisitedAt)
      .filter(entry => {
        const key = visitLookupKey(entry.url);
        if (seenUrls.has(key)) return false;
        seenUrls.add(key);
        return true;
      })
      .slice(0, BROWSER_HISTORY_MAX_ENTRIES_PER_PROJECT);
    if (entries.length > 0) byProjectKey[projectKey] = entries;
  }
  return { byProjectKey: evictExcessProjects(byProjectKey) };
}

function migratePersistedThreadProjectKeys(persistedState: unknown, byProjectKey: Record<string, BrowserHistoryEntry[]>): Record<string, string> {
  if (!persistedState || typeof persistedState !== 'object') return {};
  const raw = (persistedState as { projectKeyByThreadKey?: unknown }).projectKeyByThreadKey;
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {};
  return Object.fromEntries(Object.entries(raw as Record<string, unknown>)
    .filter((entry): entry is [string, string] => typeof entry[1] === 'string' && entry[1] in byProjectKey).slice(-100));
}

/** The saved half (the reference's `partialize`). */
export type PersistedBrowserHistory = { byProjectKey: Record<string, BrowserHistoryEntry[]>; projectKeyByThreadKey: Record<string, string> };
type PendingVisit = { url: string; at: number; environmentHostname: string | null };
type PendingTitle = { url: string; title: string; environmentHostname: string | null | undefined };
export type BrowserHistoryState = PersistedBrowserHistory & { pendingVisitsByThreadKey: Record<string, PendingVisit[]>; pendingTitlesByThreadKey: Record<string, PendingTitle[]> };

/** mergeBrowserHistoryState: saved state sanitized on every read, the pending queues empty. */
export function mergeBrowserHistoryState(persistedState: unknown): BrowserHistoryState {
  const migrated = migratePersistedBrowserHistoryState(persistedState);
  return { ...migrated, projectKeyByThreadKey: migratePersistedThreadProjectKeys(persistedState, migrated.byProjectKey), pendingVisitsByThreadKey: {}, pendingTitlesByThreadKey: {} };
}

const PENDING_MAX_PER_THREAD = 10;
const PENDING_MAX_THREADS = 20;
function addPendingByThread<T>(pendingByThreadKey: Record<string, T[]>, threadKey: string, item: T): Record<string, T[]> {
  const existing = pendingByThreadKey[threadKey] ?? [];
  const next = { ...pendingByThreadKey };
  next[threadKey] = [...existing, item].slice(-PENDING_MAX_PER_THREAD);
  const keys = Object.keys(next);
  if (keys.length > PENDING_MAX_THREADS) {
    const oldestKey = keys[0];
    if (oldestKey !== undefined && oldestKey !== threadKey) delete next[oldestKey];
  }
  return next;
}

/** useBrowserHistoryStore as one object per client; `writes` counts saved-state changes (the persist middleware's writes). */
export class BrowserHistoryStore {
  state: BrowserHistoryState;
  writes = 0;
  constructor(saved?: unknown) { this.state = mergeBrowserHistoryState(saved ?? {}); }
  /** What the client saves. */
  get persisted(): PersistedBrowserHistory { return { byProjectKey: this.state.byProjectKey, projectKeyByThreadKey: this.state.projectKeyByThreadKey }; }
  private set(patch: Partial<BrowserHistoryState>, persisted = true): void {
    this.state = { ...this.state, ...patch };
    if (persisted) this.writes++;
  }

  recordVisit(projectKey: string, url: string, at: number, options?: { insertOrdered?: boolean; environmentHostname?: string | null }): void {
    const normalized = normalizeHistoryUrl(url);
    if (!normalized) return;
    this.set({ byProjectKey: evictExcessProjects({ ...this.state.byProjectKey, [projectKey]: upsertHistoryEntry(this.state.byProjectKey[projectKey] ?? [], normalized, at, options) }) });
  }

  setTitleForUrl(projectKey: string, url: string, title: string, environmentHostname?: string | null): void {
    const normalized = normalizeHistoryUrl(url), entries = this.state.byProjectKey[projectKey];
    const trimmed = title.trim().slice(0, BROWSER_HISTORY_MAX_TITLE_LENGTH);
    if (!normalized || !entries || trimmed.length === 0) return;
    const key = titleLookupKey(normalized, environmentHostname);
    const index = entries.findIndex(candidate => titleLookupKey(candidate.url, environmentHostname) === key);
    if (index === -1 || entries[index]?.title === trimmed) return;
    this.set({ byProjectKey: { ...this.state.byProjectKey, [projectKey]: entries.map((candidate, candidateIndex) => candidateIndex === index ? { ...candidate, title: trimmed } : candidate) } });
  }

  removeUrl(projectKey: string, url: string): void {
    const normalized = normalizeHistoryUrl(url), entries = this.state.byProjectKey[projectKey];
    if (!normalized || !entries) return;
    const next = entries.filter(candidate => candidate.url !== normalized);
    if (next.length === entries.length) return;
    if (next.length === 0) {
      const { [projectKey]: _removed, ...rest } = this.state.byProjectKey;
      this.set({ byProjectKey: rest });
      return;
    }
    this.set({ byProjectKey: { ...this.state.byProjectKey, [projectKey]: next } });
  }

  registerThreadProject(ref: ScopedThreadRef, projectKey: string): void {
    const threadKey = scopedThreadKey(ref), state = this.state;
    const pendingVisits = state.pendingVisitsByThreadKey[threadKey], pendingTitles = state.pendingTitlesByThreadKey[threadKey];
    if (state.projectKeyByThreadKey[threadKey] === projectKey && !pendingVisits && !pendingTitles) return;
    const nextPendingVisits = { ...state.pendingVisitsByThreadKey }, nextPendingTitles = { ...state.pendingTitlesByThreadKey };
    delete nextPendingVisits[threadKey];
    delete nextPendingTitles[threadKey];
    this.set({ projectKeyByThreadKey: { ...state.projectKeyByThreadKey, [threadKey]: projectKey }, pendingVisitsByThreadKey: nextPendingVisits, pendingTitlesByThreadKey: nextPendingTitles },
      state.projectKeyByThreadKey[threadKey] !== projectKey);
    for (const visit of pendingVisits ?? []) this.recordVisit(projectKey, visit.url, visit.at, { insertOrdered: true, environmentHostname: visit.environmentHostname });
    for (const pendingTitle of pendingTitles ?? []) this.setTitleForUrl(projectKey, pendingTitle.url, pendingTitle.title, pendingTitle.environmentHostname);
  }

  /** recordVisitForThread: the environment's host (its prepared connection) makes it one site with loopback. The time is the
   *  caller's (a data source reads no clock: the projection's wall time). */
  recordVisitForThread(ref: ScopedThreadRef, url: string, at: number, environmentHostname: string | null = null): void {
    const threadKey = scopedThreadKey(ref), projectKey = this.state.projectKeyByThreadKey[threadKey];
    if (!projectKey) {
      this.set({ pendingVisitsByThreadKey: addPendingByThread(this.state.pendingVisitsByThreadKey, threadKey, { url, at, environmentHostname }) }, false);
      return;
    }
    this.recordVisit(projectKey, url, at, { environmentHostname });
  }

  setTitleForThreadUrl(ref: ScopedThreadRef, url: string, title: string, environmentHostname?: string | null): void {
    const threadKey = scopedThreadKey(ref), projectKey = this.state.projectKeyByThreadKey[threadKey];
    if (!projectKey) {
      this.set({ pendingTitlesByThreadKey: addPendingByThread(this.state.pendingTitlesByThreadKey, threadKey, { url, title, environmentHostname }) }, false);
      return;
    }
    this.setTitleForUrl(projectKey, url, title, environmentHostname);
  }

  removeUrlForThread(ref: ScopedThreadRef, url: string): void {
    const projectKey = this.state.projectKeyByThreadKey[scopedThreadKey(ref)];
    if (projectKey) this.removeUrl(projectKey, url);
  }

  /** useThreadRecentHistory. */
  threadRecentHistory(ref: ScopedThreadRef, limit: number): ReadonlyArray<BrowserHistoryEntry> {
    const projectKey = this.state.projectKeyByThreadKey[scopedThreadKey(ref)];
    const entries = projectKey ? this.state.byProjectKey[projectKey] : undefined;
    return entries && entries.length > 0 ? entries.slice(0, limit) : [];
  }
}

/** formatRelativeTimeLabel: "just now", "5m ago", "3h ago", "2d ago". */
export function formatRelativeTimeLabel(at: number, now: number): string {
  if (!isValidHistoryTimestamp(at)) return '';
  const diffMs = now - at;
  if (diffMs < 0) return 'just now';
  const seconds = Math.floor(diffMs / 1000);
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

/** PreviewRecentUrlCard's label: host, path (none for "/"), search and hash. */
export function recentUrlLabel(url: string): string {
  const parsed = new URL(url), path = parsed.pathname === '/' ? '' : parsed.pathname;
  return `${parsed.host}${path}${parsed.search}${parsed.hash}`;
}

/** The client's history, kept in its saved preferences (client.ts adopts it with the rest). */
const stores = new WeakMap<object, BrowserHistoryStore>();
export function browserHistory(local: object): BrowserHistoryStore {
  let store = stores.get(local);
  if (!store) {
    const made = new BrowserHistoryStore((local as { browserHistory?: unknown }).browserHistory);
    // Saved with client.local (JSON.stringify reads the getter): always the store's current state.
    Object.defineProperty(local, 'browserHistory', { get: () => made.persisted, enumerable: true, configurable: true });
    stores.set(local, store = made);
  }
  return store;
}
/** client.ts preferences adoption: the saved history, sanitized. */
export function adoptBrowserHistory(next: object, saved: Record<string, unknown>): void {
  (next as { browserHistory?: PersistedBrowserHistory }).browserHistory = new BrowserHistoryStore(saved.browserHistory).persisted;
}
