// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/previewStateStore.test.ts (26
// tests), apps/web/src/browser/previewRuntimeTabId.test.ts (3) and
// apps/web/src/components/preview/previewEmptyStateLogic.test.ts (`shouldShowPreviewEmptyState`, 2),
// under their own names. Substitutions: the per-thread Effect atoms are one `PreviewStateStore` per
// client (`read` for `readThreadPreviewState`, `apply…`/`reconcile…`/`…Close` for the store's exported
// functions); "keeps independent state atoms for each thread" checks independent per-thread state, and
// "does not publish duplicate desktop browser state" counts the store's change results instead of an
// atom subscription. `getConfiguredPreviewUrls` belongs to part 2 (configured local servers).
import { beforeEach, describe, expect, it } from 'bun:test';
import {
  EMPTY_THREAD_PREVIEW_STATE, PREVIEW_RECENT_URL_LIMIT, PreviewStateStore, isCurrentPreviewRuntimeTab, previewRuntimeTabId, readSnapshot,
  shouldShowPreviewEmptyState, type DesktopPreviewOverlay, type PreviewEvent, type PreviewSessionSnapshot,
} from './browser-state';
import { scopeThreadRef } from './terminal-ui-state';

const environmentId = 'env-1';
const ref = scopeThreadRef(environmentId, 'thread-1');
const otherRef = scopeThreadRef(environmentId, 'thread-2');

const makeSnapshot = (overrides: Partial<PreviewSessionSnapshot> = {}): PreviewSessionSnapshot => ({
  threadId: 'thread-1', tabId: 'tab_a', navStatus: { _tag: 'Loading', url: 'http://localhost:5173/', title: '' },
  canGoBack: false, canGoForward: false, updatedAt: '2026-01-01T00:00:00.000Z', ...overrides,
});
const overlay = (overrides: Partial<DesktopPreviewOverlay> = {}): DesktopPreviewOverlay => ({
  hasWebContents: true, canGoBack: true, canGoForward: false, loading: false, zoomFactor: 1, pictureInPicture: false,
  colorScheme: 'system', audioMuted: false, audible: false, controller: 'none', favicon: null, ...overrides,
});

type EventDraft = PreviewEvent extends infer Event ? Event extends { readonly revision: number } ? Omit<Event, 'revision' | 'serverEpoch'> : never : never;
const serverEpoch = 'server-a';
let store = new PreviewStateStore();
let nextServerRevision = 0;
const applyEvent = (eventRef: typeof ref, event: EventDraft): void => {
  nextServerRevision += 1;
  store.applyServerEvent(eventRef, { ...event, serverEpoch, revision: nextServerRevision } as PreviewEvent);
};
const opened = (snapshot: PreviewSessionSnapshot): EventDraft => ({ type: 'opened', threadId: 'thread-1', tabId: snapshot.tabId, createdAt: snapshot.updatedAt, snapshot });

beforeEach(() => { nextServerRevision = 0; store = new PreviewStateStore(); });

describe('previewStateStore (single-tab)', () => {
  it('keeps independent state atoms for each thread', () => {
    store.applyServerSnapshot(ref, makeSnapshot());
    expect(store.read(ref).snapshot?.tabId).toBe('tab_a');
    expect(store.read(otherRef)).toEqual(EMPTY_THREAD_PREVIEW_STATE);
  });

  it('opened event seeds the snapshot and remembers the URL', () => {
    const snapshot = makeSnapshot();
    applyEvent(ref, opened(snapshot));
    const state = store.read(ref);
    expect(state.snapshot?.tabId).toBe(snapshot.tabId);
    expect(state.recentlySeenUrls).toContain('http://localhost:5173/');
  });

  it('a second `opened` for a different tab replaces the rendered snapshot', () => {
    applyEvent(ref, opened(makeSnapshot({ tabId: 'tab_a' })));
    applyEvent(ref, opened(makeSnapshot({ tabId: 'tab_b' })));
    expect(store.read(ref).snapshot?.tabId).toBe('tab_b');
  });

  it('navigated event updates the snapshot URL', () => {
    const snapshot = makeSnapshot();
    applyEvent(ref, opened(snapshot));
    applyEvent(ref, { type: 'navigated', threadId: 'thread-1', tabId: snapshot.tabId, createdAt: '2026-01-01T00:00:01.000Z',
      snapshot: { ...snapshot, navStatus: { _tag: 'Success', url: 'http://localhost:5173/about', title: 'About' } } });
    const state = store.read(ref);
    expect(state.snapshot?.navStatus._tag).toBe('Success');
    if (state.snapshot?.navStatus._tag === 'Success') expect(state.snapshot.navStatus.url).toBe('http://localhost:5173/about');
  });

  it('resized event updates tab viewport without changing the active tab', () => {
    const active = makeSnapshot({ tabId: 'tab_a' }), background = makeSnapshot({ tabId: 'tab_b' });
    store.applyServerSnapshot(ref, background);
    store.applyServerSnapshot(ref, active);
    applyEvent(ref, { type: 'resized', threadId: 'thread-1', tabId: background.tabId, createdAt: '2026-01-01T00:00:01.000Z',
      snapshot: { ...background, viewport: { _tag: 'preset', presetId: 'pixel-8', width: 412, height: 915 }, updatedAt: '2026-01-01T00:00:01.000Z' } });
    const state = store.read(ref);
    expect(state.activeTabId).toBe(active.tabId);
    expect(state.sessions[background.tabId]?.viewport).toEqual({ _tag: 'preset', presetId: 'pixel-8', width: 412, height: 915 });
  });

  it('failed event flips the snapshot to LoadFailed when tabId matches', () => {
    const snapshot = makeSnapshot();
    applyEvent(ref, opened(snapshot));
    applyEvent(ref, { type: 'failed', threadId: 'thread-1', tabId: snapshot.tabId, createdAt: '2026-01-01T00:00:01.000Z', url: 'http://localhost:5173/', title: '', code: -105, description: 'ERR_NAME_NOT_RESOLVED' });
    expect(store.read(ref).snapshot?.navStatus._tag).toBe('LoadFailed');
  });

  it('failed event for a non-active tab is ignored', () => {
    applyEvent(ref, opened(makeSnapshot({ tabId: 'tab_a' })));
    applyEvent(ref, { type: 'failed', threadId: 'thread-1', tabId: 'tab_b', createdAt: '2026-01-01T00:00:01.000Z', url: 'http://localhost:9999/', title: '', code: -105, description: 'ERR_NAME_NOT_RESOLVED' });
    expect(store.read(ref).snapshot?.navStatus._tag).toBe('Loading');
  });

  it('closed event clears snapshot but retains recently-seen URLs', () => {
    const snapshot = makeSnapshot();
    applyEvent(ref, opened(snapshot));
    applyEvent(ref, { type: 'closed', threadId: 'thread-1', tabId: snapshot.tabId, createdAt: '2026-01-01T00:00:01.000Z' });
    const state = store.read(ref);
    expect(state.snapshot).toBeNull();
    expect(state.recentlySeenUrls).toContain('http://localhost:5173/');
  });

  it('optimistically removes a session before the server close event arrives', () => {
    const first = makeSnapshot({ tabId: 'tab_a' }), second = makeSnapshot({ tabId: 'tab_b', updatedAt: '2026-01-01T00:00:01.000Z' });
    store.applyServerSnapshot(ref, first);
    store.applyServerSnapshot(ref, second);
    store.beginSessionClose(ref, second.tabId);
    const state = store.read(ref);
    expect(Object.keys(state.sessions)).toEqual([first.tabId]);
    expect(state.activeTabId).toBe(first.tabId);
    expect(state.snapshot?.tabId).toBe(first.tabId);
  });

  it('treats a late server close event after optimistic removal as a no-op', () => {
    const snapshot = makeSnapshot();
    store.applyServerSnapshot(ref, snapshot);
    store.beginSessionClose(ref, snapshot.tabId);
    applyEvent(ref, { type: 'closed', threadId: 'thread-1', tabId: snapshot.tabId, createdAt: '2026-01-01T00:00:01.000Z' });
    const state = store.read(ref);
    expect(state.sessions).toEqual({});
    expect(state.snapshot).toBeNull();
  });

  it('does not resurrect an intentionally closed tab from a stale list snapshot', () => {
    const snapshot = makeSnapshot();
    store.applyServerSnapshot(ref, snapshot);
    store.beginSessionClose(ref, snapshot.tabId);
    store.applyServerSnapshot(ref, snapshot);
    const state = store.read(ref);
    expect(state.sessions).toEqual({});
    expect(state.snapshot).toBeNull();
  });

  it('can restore a suppressed tab after a failed close', () => {
    const snapshot = makeSnapshot();
    store.applyServerSnapshot(ref, snapshot);
    store.beginSessionClose(ref, snapshot.tabId);
    store.cancelSessionClose(ref, snapshot, snapshot.tabId);
    const state = store.read(ref);
    expect(state.sessions).toEqual({ [snapshot.tabId]: snapshot });
    expect(state.snapshot).toEqual(snapshot);
  });

  it('closed event for a different tab is a no-op', () => {
    const snapshot = makeSnapshot({ tabId: 'tab_a' });
    applyEvent(ref, opened(snapshot));
    applyEvent(ref, { type: 'closed', threadId: 'thread-1', tabId: 'tab_b', createdAt: '2026-01-01T00:00:01.000Z' });
    expect(store.read(ref).snapshot?.tabId).toBe(snapshot.tabId);
  });

  it('desktopOverlay updates independently of snapshot', () => {
    const snapshot = makeSnapshot();
    applyEvent(ref, opened(snapshot));
    store.applyDesktopState(ref, snapshot.tabId, overlay());
    const state = store.read(ref);
    expect(state.desktopOverlay?.canGoBack).toBe(true);
    expect(state.snapshot?.canGoBack).toBe(false);
  });

  it('does not publish duplicate desktop browser state', () => {
    const snapshot = makeSnapshot();
    store.applyServerSnapshot(ref, snapshot);
    const value = overlay({ favicon: { dataUrl: 'data:image/png;base64,AA==', pageUrl: 'https://example.com', capturedAt: 1 } });
    let updateCount = 0;
    if (store.applyDesktopState(ref, snapshot.tabId, value)) updateCount += 1;
    if (store.applyDesktopState(ref, snapshot.tabId, { ...value, favicon: { ...value.favicon! } })) updateCount += 1;
    expect(updateCount).toBe(1);
  });

  it('retains multiple tabs and switches active desktop state', () => {
    const first = makeSnapshot(), second = { ...makeSnapshot(), tabId: 'tab_2', updatedAt: '2026-01-02T00:00:00.000Z' };
    store.applyServerSnapshot(ref, first);
    store.applyServerSnapshot(ref, second);
    store.applyDesktopState(ref, first.tabId, overlay());
    store.setActiveTab(ref, first.tabId);
    const state = store.read(ref);
    expect(Object.keys(state.sessions)).toEqual([first.tabId, second.tabId]);
    expect(state.snapshot?.tabId).toBe(first.tabId);
    expect(state.desktopOverlay?.canGoBack).toBe(true);
  });

  it('updates a background snapshot without changing the active tab', () => {
    const background = makeSnapshot({ tabId: 'tab_a' }), active = makeSnapshot({ tabId: 'tab_b', updatedAt: '2026-01-01T00:00:01.000Z' });
    store.applyServerSnapshot(ref, background);
    store.applyServerSnapshot(ref, active);
    const resized = { ...background, viewport: { _tag: 'freeform' as const, width: 900, height: 700 }, updatedAt: '2026-01-01T00:00:02.000Z' };
    store.updateServerSnapshot(ref, resized);
    const state = store.read(ref);
    expect(state.activeTabId).toBe(active.tabId);
    expect(state.snapshot?.tabId).toBe(active.tabId);
    expect(state.sessions[background.tabId]).toEqual(resized);
  });

  it('reconciles an authoritative session list without focusing a background tab', () => {
    const active = makeSnapshot({ tabId: 'tab_a' }), stale = makeSnapshot({ tabId: 'tab_stale', updatedAt: '2026-01-01T00:00:01.000Z' });
    store.applyServerSnapshot(ref, stale);
    store.applyServerSnapshot(ref, active);
    store.applyDesktopState(ref, stale.tabId, overlay({ canGoBack: false }));
    store.reconcileServerSessions(ref, { sessions: [active], serverEpoch, revision: 1 });
    const state = store.read(ref);
    expect(Object.keys(state.sessions)).toEqual([active.tabId]);
    expect(state.activeTabId).toBe(active.tabId);
    expect(state.snapshot).toEqual(active);
    expect(state.desktopByTabId[stale.tabId]).toBeUndefined();
  });

  it('clears stale sessions when an authoritative list is empty', () => {
    store.applyServerSnapshot(ref, makeSnapshot());
    store.reconcileServerSessions(ref, { sessions: [], serverEpoch, revision: 1 });
    const state = store.read(ref);
    expect(state.sessions).toEqual({});
    expect(state.activeTabId).toBeNull();
    expect(state.snapshot).toBeNull();
  });

  it('ignores a list response older than the latest server event', () => {
    const snapshot = makeSnapshot();
    applyEvent(ref, opened(snapshot));
    store.reconcileServerSessions(ref, { sessions: [], serverEpoch, revision: 0 });
    expect(store.read(ref).sessions).toEqual({ [snapshot.tabId]: snapshot });
  });

  it('does not resurrect a tab from an event older than its close', () => {
    const snapshot = makeSnapshot();
    applyEvent(ref, opened(snapshot));
    applyEvent(ref, { type: 'closed', threadId: 'thread-1', tabId: snapshot.tabId, createdAt: '2026-01-01T00:00:01.000Z' });
    store.applyServerEvent(ref, { ...opened(snapshot), serverEpoch, revision: 1 } as PreviewEvent);
    expect(store.read(ref).sessions).toEqual({});
  });

  it('accepts a lower revision from a newly restarted server', () => {
    const snapshot = makeSnapshot();
    store.applyServerEvent(ref, { ...opened(snapshot), serverEpoch, revision: 12 } as PreviewEvent);
    store.reconcileServerSessions(ref, { sessions: [], serverEpoch: 'server-b', revision: 0 });
    const state = store.read(ref);
    expect(state.sessions).toEqual({});
    expect(state.serverEpoch).toBe('server-b');
    expect(state.serverRevision).toBe(0);
  });

  it('does not carry raw-tab state across a server restart', () => {
    const previous = makeSnapshot({ navStatus: { _tag: 'Success', url: 'https://old.example', title: 'Old' }, updatedAt: '2026-01-01T00:00:02.000Z' });
    store.applyServerEvent(ref, { ...opened(previous), serverEpoch, revision: 12 } as PreviewEvent);
    store.beginSessionClose(ref, previous.tabId);
    store.applyDesktopState(ref, previous.tabId, overlay({ canGoBack: false }));
    const restarted = makeSnapshot({ navStatus: { _tag: 'Success', url: 'https://new.example', title: 'New' }, updatedAt: '2026-01-01T00:00:01.000Z' });
    store.reconcileServerSessions(ref, { sessions: [restarted], serverEpoch: 'server-b', revision: 0 });
    const state = store.read(ref);
    expect(state.sessions[restarted.tabId]).toEqual(restarted);
    expect(state.suppressedTabIds).toEqual(new Set());
    expect(state.desktopByTabId).toEqual({});
    expect(state.desktopOverlay).toBeNull();
  });

  it('applyServerSnapshot null clears snapshot for a thread that had one', () => {
    store.applyServerSnapshot(ref, makeSnapshot());
    store.applyServerSnapshot(ref, null);
    expect(store.read(ref).snapshot).toBeNull();
  });

  it('does not replace a streamed snapshot with older SWR data', () => {
    store.applyServerSnapshot(ref, makeSnapshot({ navStatus: { _tag: 'Success', url: 'http://localhost:5173/new', title: 'New' }, updatedAt: '2026-01-01T00:00:02.000Z' }));
    store.applyServerSnapshot(ref, makeSnapshot({ navStatus: { _tag: 'Success', url: 'http://localhost:5173/old', title: 'Old' }, updatedAt: '2026-01-01T00:00:01.000Z' }));
    expect(store.read(ref).snapshot?.navStatus).toEqual({ _tag: 'Success', url: 'http://localhost:5173/new', title: 'New' });
  });

  it('rememberUrl dedupes and caps at limit', () => {
    for (let i = 0; i < PREVIEW_RECENT_URL_LIMIT + 5; i += 1) store.rememberUrl(ref, `http://localhost:${5000 + i}/`);
    const state = store.read(ref);
    expect(state.recentlySeenUrls.length).toBeLessThanOrEqual(PREVIEW_RECENT_URL_LIMIT);
    expect(state.recentlySeenUrls[0]).toBe(`http://localhost:${5000 + PREVIEW_RECENT_URL_LIMIT + 4}/`);
  });
});

describe('previewRuntimeTabId', () => {
  it('scopes process-local tab ids to their environment, thread, and server epoch', () => {
    const tabId = 'tab_1';
    const left = previewRuntimeTabId(scopeThreadRef('environment-a', 'thread-1'), 'server-a', tabId);
    const otherEnvironment = previewRuntimeTabId(scopeThreadRef('environment-b', 'thread-1'), 'server-a', tabId);
    const otherThread = previewRuntimeTabId(scopeThreadRef('environment-a', 'thread-2'), 'server-a', tabId);
    const otherServer = previewRuntimeTabId(scopeThreadRef('environment-a', 'thread-1'), 'server-b', tabId);
    expect(new Set([left, otherEnvironment, otherThread, otherServer]).size).toBe(4);
  });

  it('is stable for the same runtime tab', () => {
    const tabRef = scopeThreadRef('environment-a', 'thread-1');
    expect(previewRuntimeTabId(tabRef, 'server-a', 'tab_1')).toBe(previewRuntimeTabId(tabRef, 'server-a', 'tab_1'));
  });

  it('rejects a pinned operation target after the server epoch changes', () => {
    const tabRef = scopeThreadRef('environment-a', 'thread-1');
    const pinned = previewRuntimeTabId(tabRef, 'server-a', 'tab_1');
    expect(isCurrentPreviewRuntimeTab(tabRef, 'server-a', 'tab_1', pinned)).toBe(true);
    expect(isCurrentPreviewRuntimeTab(tabRef, 'server-b', 'tab_1', pinned)).toBe(false);
  });
});

describe('shouldShowPreviewEmptyState', () => {
  it('shows quick-open options for a new idle browser tab', () => {
    expect(shouldShowPreviewEmptyState(makeSnapshot({ navStatus: { _tag: 'Idle' } }))).toBe(true);
  });

  it('shows browser content once navigation starts', () => {
    expect(shouldShowPreviewEmptyState(makeSnapshot({ navStatus: { _tag: 'Loading', url: 'http://localhost:3000/', title: '' } }))).toBe(false);
  });
});

describe('readSnapshot (the server answer, read defensively)', () => {
  it('keeps a well-formed snapshot and refuses malformed ones', () => {
    const snapshot = makeSnapshot({ profileId: 'default', viewport: { _tag: 'fill' } });
    expect(readSnapshot(JSON.parse(JSON.stringify(snapshot)))).toEqual(snapshot);
    expect(readSnapshot({ ...snapshot, tabId: '' })).toBeNull();
    expect(readSnapshot({ ...snapshot, navStatus: { _tag: 'Teleported' } })).toBeNull();
    expect(readSnapshot(null)).toBeNull();
  });
});
