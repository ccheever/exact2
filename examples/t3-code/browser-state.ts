// browser-surface: per-thread preview state (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// apps/web/src/previewStateStore.ts; apps/web/src/browser/previewRuntimeTabId.ts;
// packages/contracts/src/preview.ts `PreviewNavStatus`, `PreviewSessionSnapshot`). A Browser tab is a
// server preview session (`preview.open`, keyed by thread and tab); this store keeps each thread's
// sessions, the tabs this client is closing (so a stale list or event cannot bring them back), the
// server's epoch and revision, and the native web view's state per tab ("desktop overlay"). The
// reference keeps one Effect atom per thread; here one Map per client holds the same reducers, each
// returning whether it changed anything (the atom registry's change notification).
import { scopedThreadKey, type ScopedThreadRef } from './terminal-ui-state';

export type PreviewNavStatus =
  | { _tag: 'Idle' }
  | { _tag: 'Loading'; url: string; title: string }
  | { _tag: 'Success'; url: string; title: string }
  | { _tag: 'LoadFailed'; url: string; title: string; code: number; description: string };

export type PreviewViewportSetting = { _tag: 'fill' } | { _tag: 'freeform'; width: number; height: number } | { _tag: 'preset'; presetId: string; width: number; height: number };

export type PreviewSessionSnapshot = {
  threadId: string;
  tabId: string;
  navStatus: PreviewNavStatus;
  canGoBack: boolean;
  canGoForward: boolean;
  viewport?: PreviewViewportSetting;
  profileId?: string;
  updatedAt: string;
};

export type PreviewListResult = { sessions: readonly PreviewSessionSnapshot[]; serverEpoch: string; revision: number };

type EventBase = { threadId: string; tabId: string; createdAt: string; serverEpoch: string; revision: number };
export type PreviewEvent =
  | (EventBase & { type: 'opened' | 'navigated' | 'resized'; snapshot: PreviewSessionSnapshot })
  | (EventBase & { type: 'failed'; url: string; title: string; code: number; description: string })
  | (EventBase & { type: 'closed' });

export type DesktopPreviewFavicon = { dataUrl: string; pageUrl: string; capturedAt: number };
/** The native web view's state for a tab (desktopByTabId). Zoom, appearance, picture in picture, audio and
 *  the agent controller arrive with parts 2, 3 and 5; their reference defaults stand until then. */
export type DesktopPreviewOverlay = {
  hasWebContents: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  loading: boolean;
  zoomFactor: number;
  pictureInPicture: boolean;
  colorScheme: 'system' | 'light' | 'dark';
  audioMuted: boolean;
  audible: boolean;
  controller: 'human' | 'agent' | 'none';
  favicon: DesktopPreviewFavicon | null;
};

export type ThreadPreviewState = {
  snapshot: PreviewSessionSnapshot | null;
  sessions: Record<string, PreviewSessionSnapshot>;
  /** Tabs intentionally closed by this client. Stale list snapshots must not resurrect them. */
  suppressedTabIds: ReadonlySet<string>;
  activeTabId: string | null;
  desktopOverlay: DesktopPreviewOverlay | null;
  desktopByTabId: Record<string, DesktopPreviewOverlay>;
  recentlySeenUrls: string[];
  /** Server process currently authoritative for revision ordering. */
  serverEpoch: string | null;
  /** Latest ordered server revision applied from a list response or event. */
  serverRevision: number;
};

/** previewConstants.ts PREVIEW_RECENT_URL_LIMIT. */
export const PREVIEW_RECENT_URL_LIMIT = 10;

export const EMPTY_THREAD_PREVIEW_STATE: ThreadPreviewState = Object.freeze({
  snapshot: null, sessions: {}, suppressedTabIds: new Set<string>(), activeTabId: null, desktopOverlay: null,
  desktopByTabId: {}, recentlySeenUrls: [] as string[], serverEpoch: null, serverRevision: 0,
});

const dedupeRecentUrls = (existing: string[], url: string): string[] => [url, ...existing.filter(entry => entry !== url)].slice(0, PREVIEW_RECENT_URL_LIMIT);
const rememberSnapshotUrl = (recentlySeenUrls: string[], snapshot: PreviewSessionSnapshot): string[] =>
  snapshot.navStatus._tag === 'Idle' ? recentlySeenUrls : dedupeRecentUrls(recentlySeenUrls, snapshot.navStatus.url);
const latestSnapshot = (sessions: Record<string, PreviewSessionSnapshot>): PreviewSessionSnapshot | null =>
  Object.values(sessions).sort((a, b) => a.updatedAt.localeCompare(b.updatedAt)).at(-1) ?? null;

function removeSession(current: ThreadPreviewState, tabId: string): ThreadPreviewState {
  if (!current.sessions[tabId]) return current;
  const { [tabId]: _closed, ...sessions } = current.sessions;
  const { [tabId]: _desktop, ...desktopByTabId } = current.desktopByTabId;
  const nextSnapshot = latestSnapshot(sessions);
  const activeTabId = current.activeTabId === tabId ? (nextSnapshot?.tabId ?? null) : current.activeTabId;
  const snapshot = activeTabId ? (sessions[activeTabId] ?? nextSnapshot) : nextSnapshot;
  return { ...current, sessions, desktopByTabId, activeTabId: snapshot?.tabId ?? null, snapshot, desktopOverlay: snapshot ? (desktopByTabId[snapshot.tabId] ?? null) : null };
}

function sameOverlay(previous: DesktopPreviewOverlay | null, next: DesktopPreviewOverlay | null): boolean {
  return previous === next || (previous !== null && next !== null &&
    previous.hasWebContents === next.hasWebContents && previous.canGoBack === next.canGoBack && previous.canGoForward === next.canGoForward &&
    previous.loading === next.loading && previous.zoomFactor === next.zoomFactor && previous.pictureInPicture === next.pictureInPicture &&
    previous.colorScheme === next.colorScheme && previous.audioMuted === next.audioMuted && previous.audible === next.audible &&
    previous.controller === next.controller && previous.favicon?.dataUrl === next.favicon?.dataUrl &&
    previous.favicon?.pageUrl === next.favicon?.pageUrl && previous.favicon?.capturedAt === next.favicon?.capturedAt);
}

/** previewStateStore: one state per scoped thread key; every write returns whether it changed the state. */
export class PreviewStateStore {
  private readonly byThreadKey = new Map<string, ThreadPreviewState>();

  read(ref: ScopedThreadRef): ThreadPreviewState { return this.byThreadKey.get(scopedThreadKey(ref)) ?? EMPTY_THREAD_PREVIEW_STATE; }
  /** useActivePreviewSessions: every thread with a live session (the native host keeps a web view for each). */
  active(): Array<[string, ThreadPreviewState]> { return [...this.byThreadKey].filter(([, state]) => Object.keys(state.sessions).length > 0); }

  private update(ref: ScopedThreadRef, updater: (current: ThreadPreviewState) => ThreadPreviewState): boolean {
    const key = scopedThreadKey(ref), current = this.read(ref), next = updater(current);
    if (next === current) return false;
    this.byThreadKey.set(key, next);
    return true;
  }

  applyServerEvent(ref: ScopedThreadRef, event: PreviewEvent): boolean {
    return this.update(ref, current => {
      if (current.serverEpoch !== null && event.serverEpoch !== current.serverEpoch) return current;
      if (event.revision < current.serverRevision) return current;
      const next = ((): ThreadPreviewState => {
        switch (event.type) {
          case 'opened': case 'navigated': case 'resized': {
            const snapshot = event.snapshot;
            if (current.suppressedTabIds.has(snapshot.tabId)) return current;
            const recentlySeenUrls = snapshot.navStatus._tag === 'Idle' ? current.recentlySeenUrls : dedupeRecentUrls(current.recentlySeenUrls, snapshot.navStatus.url);
            const sessions = { ...current.sessions, [snapshot.tabId]: snapshot };
            const activeTabId = event.type === 'opened' ? snapshot.tabId : current.activeTabId;
            const activeSnapshot = sessions[activeTabId ?? snapshot.tabId] ?? snapshot;
            return { ...current, sessions, activeTabId: activeTabId ?? snapshot.tabId, snapshot: activeSnapshot, desktopOverlay: current.desktopByTabId[activeSnapshot.tabId] ?? null, recentlySeenUrls };
          }
          case 'failed': {
            const existing = current.sessions[event.tabId];
            if (!existing) return current;
            const failedSnapshot: PreviewSessionSnapshot = { ...existing, navStatus: { _tag: 'LoadFailed', url: event.url, title: event.title, code: event.code, description: event.description }, updatedAt: event.createdAt };
            return { ...current, sessions: { ...current.sessions, [event.tabId]: failedSnapshot }, snapshot: current.activeTabId === event.tabId ? failedSnapshot : current.snapshot };
          }
          case 'closed': {
            const closed = removeSession(current, event.tabId);
            if (!closed.suppressedTabIds.has(event.tabId)) return closed;
            const suppressedTabIds = new Set(closed.suppressedTabIds);
            suppressedTabIds.delete(event.tabId);
            return { ...closed, suppressedTabIds };
          }
        }
      })();
      return next.serverRevision === event.revision && next.serverEpoch === event.serverEpoch ? next : { ...next, serverEpoch: event.serverEpoch, serverRevision: event.revision };
    });
  }

  applyServerSnapshot(ref: ScopedThreadRef, snapshot: PreviewSessionSnapshot | null): boolean {
    return this.update(ref, current => {
      if (!snapshot && current.snapshot === null) return current;
      if (!snapshot) return { ...current, snapshot: null, sessions: {}, activeTabId: null, desktopOverlay: null, desktopByTabId: {} };
      if (current.suppressedTabIds.has(snapshot.tabId)) return current;
      const existing = current.sessions[snapshot.tabId];
      if (existing && existing.updatedAt > snapshot.updatedAt) return current;
      return { ...current, snapshot, sessions: { ...current.sessions, [snapshot.tabId]: snapshot }, activeTabId: snapshot.tabId,
        desktopOverlay: current.desktopByTabId[snapshot.tabId] ?? null, recentlySeenUrls: rememberSnapshotUrl(current.recentlySeenUrls, snapshot) };
    });
  }

  /** A server mutation's answer for a tab, without changing which tab the person is viewing. */
  updateServerSnapshot(ref: ScopedThreadRef, snapshot: PreviewSessionSnapshot): boolean {
    return this.update(ref, current => {
      if (current.suppressedTabIds.has(snapshot.tabId)) return current;
      const existing = current.sessions[snapshot.tabId];
      if (existing && existing.updatedAt > snapshot.updatedAt) return current;
      const sessions = { ...current.sessions, [snapshot.tabId]: snapshot };
      const activeTabId = current.activeTabId && sessions[current.activeTabId] ? current.activeTabId : snapshot.tabId;
      const activeSnapshot = sessions[activeTabId] ?? snapshot;
      return { ...current, sessions, activeTabId, snapshot: activeSnapshot, desktopOverlay: current.desktopByTabId[activeTabId] ?? null, recentlySeenUrls: rememberSnapshotUrl(current.recentlySeenUrls, snapshot) };
    });
  }

  /** An authoritative `preview.list`: missing tabs go, the active tab stays while it still exists. */
  reconcileServerSessions(ref: ScopedThreadRef, result: PreviewListResult): boolean {
    return this.update(ref, current => {
      const sameServer = current.serverEpoch === result.serverEpoch;
      if (sameServer && result.revision < current.serverRevision) return current;
      const sessions: Record<string, PreviewSessionSnapshot> = {};
      const currentSuppressed = sameServer ? current.suppressedTabIds : new Set<string>();
      let recentlySeenUrls = current.recentlySeenUrls;
      for (const snapshot of result.sessions) {
        if (currentSuppressed.has(snapshot.tabId)) continue;
        const existing = sameServer ? current.sessions[snapshot.tabId] : undefined;
        const next = existing && existing.updatedAt > snapshot.updatedAt ? existing : snapshot;
        sessions[next.tabId] = next;
        recentlySeenUrls = rememberSnapshotUrl(recentlySeenUrls, next);
      }
      const fallback = latestSnapshot(sessions);
      const activeTabId = current.activeTabId && sessions[current.activeTabId] ? current.activeTabId : (fallback?.tabId ?? null);
      const desktopByTabId = sameServer ? Object.fromEntries(Object.entries(current.desktopByTabId).filter(([tabId]) => sessions[tabId] !== undefined)) : {};
      const suppressedTabIds = new Set([...currentSuppressed].filter(tabId => result.sessions.some(snapshot => snapshot.tabId === tabId)));
      return { ...current, sessions, suppressedTabIds, activeTabId, snapshot: activeTabId ? (sessions[activeTabId] ?? null) : null, desktopByTabId,
        desktopOverlay: activeTabId ? (desktopByTabId[activeTabId] ?? null) : null, recentlySeenUrls, serverEpoch: result.serverEpoch, serverRevision: result.revision };
    });
  }

  applyDesktopState(ref: ScopedThreadRef, tabId: string, overlay: DesktopPreviewOverlay | null): boolean {
    return this.update(ref, current => {
      if (sameOverlay(current.desktopByTabId[tabId] ?? null, overlay)) return current;
      const desktopByTabId = { ...current.desktopByTabId };
      if (overlay) desktopByTabId[tabId] = overlay; else delete desktopByTabId[tabId];
      return { ...current, desktopByTabId, desktopOverlay: current.activeTabId === tabId ? overlay : current.desktopOverlay };
    });
  }

  beginSessionClose(ref: ScopedThreadRef, tabId: string): boolean {
    return this.update(ref, current => {
      const suppressedTabIds = new Set(current.suppressedTabIds);
      suppressedTabIds.add(tabId);
      return { ...removeSession(current, tabId), suppressedTabIds };
    });
  }

  cancelSessionClose(ref: ScopedThreadRef, snapshot: PreviewSessionSnapshot | null, tabId: string): boolean {
    return this.update(ref, current => {
      if (!current.suppressedTabIds.has(tabId)) return current;
      const suppressedTabIds = new Set(current.suppressedTabIds);
      suppressedTabIds.delete(tabId);
      if (!snapshot) return { ...current, suppressedTabIds };
      const recentlySeenUrls = snapshot.navStatus._tag !== 'Idle' ? dedupeRecentUrls(current.recentlySeenUrls, snapshot.navStatus.url) : current.recentlySeenUrls;
      return { ...current, snapshot, sessions: { ...current.sessions, [snapshot.tabId]: snapshot }, suppressedTabIds, activeTabId: snapshot.tabId,
        desktopOverlay: current.desktopByTabId[snapshot.tabId] ?? null, recentlySeenUrls };
    });
  }

  setActiveTab(ref: ScopedThreadRef, tabId: string): boolean {
    return this.update(ref, current => {
      const snapshot = current.sessions[tabId];
      if (!snapshot || current.activeTabId === tabId) return current;
      return { ...current, activeTabId: tabId, snapshot, desktopOverlay: current.desktopByTabId[tabId] ?? null };
    });
  }

  rememberUrl(ref: ScopedThreadRef, url: string): boolean {
    if (url.trim().length === 0) return false;
    return this.update(ref, current => ({ ...current, recentlySeenUrls: dedupeRecentUrls(current.recentlySeenUrls, url) }));
  }
}

/**
 * previewRuntimeTabId: the server only guarantees preview tab ids are unique within one process, and the
 * native web views live across every connected environment, so a tab's native identity also names its
 * environment, thread and server process.
 */
export const previewRuntimeTabId = (ref: ScopedThreadRef, serverEpoch: string | null, tabId: string): string =>
  JSON.stringify([ref.environmentId, ref.threadId, serverEpoch, tabId]);

export const isCurrentPreviewRuntimeTab = (ref: ScopedThreadRef, serverEpoch: string | null, tabId: string, runtimeTabId: string): boolean =>
  previewRuntimeTabId(ref, serverEpoch, tabId) === runtimeTabId;

/** previewEmptyStateLogic: an idle (or missing) tab shows the empty state; navigation shows the page. */
export const shouldShowPreviewEmptyState = (snapshot: { navStatus: PreviewNavStatus } | null): boolean => snapshot === null || snapshot.navStatus._tag === 'Idle';

/** A server answer (`preview.open`, `preview.list`) read defensively into a snapshot; anything malformed is null. */
export function readSnapshot(value: unknown): PreviewSessionSnapshot | null {
  if (!value || typeof value !== 'object') return null;
  const raw = value as Record<string, unknown>, nav = raw.navStatus as Record<string, unknown> | undefined;
  if (typeof raw.threadId !== 'string' || typeof raw.tabId !== 'string' || !raw.tabId || !nav || typeof nav._tag !== 'string') return null;
  const text = (field: unknown) => (typeof field === 'string' ? field : '');
  const navStatus: PreviewNavStatus | null = nav._tag === 'Idle' ? { _tag: 'Idle' }
    : nav._tag === 'Loading' || nav._tag === 'Success' ? { _tag: nav._tag, url: text(nav.url), title: text(nav.title) }
      : nav._tag === 'LoadFailed' ? { _tag: 'LoadFailed', url: text(nav.url), title: text(nav.title), code: Number(nav.code) || 0, description: text(nav.description) } : null;
  if (!navStatus) return null;
  return {
    threadId: raw.threadId, tabId: raw.tabId, navStatus, canGoBack: raw.canGoBack === true, canGoForward: raw.canGoForward === true,
    ...(raw.viewport && typeof raw.viewport === 'object' ? { viewport: raw.viewport as PreviewViewportSetting } : {}),
    ...(typeof raw.profileId === 'string' ? { profileId: raw.profileId } : {}), updatedAt: text(raw.updatedAt),
  };
}
