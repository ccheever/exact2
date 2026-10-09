// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/browserHistoryStore.test.ts (33 tests),
// under their own names. Substitutions: the zustand store is one `BrowserHistoryStore`; `readPreparedConnection`'s
// environment host is `recordVisitForThread`'s last argument; "persist writes" are the store's `writes` count (the
// saved half, as `partialize` keeps it); `mergeBrowserHistoryState` takes only the saved value (the store's actions
// are methods, so "preserves actions" checks the class's). Clone rows (marked) cover the saved-preferences holder
// and the relative time label.
import { beforeEach, describe, expect, it } from 'bun:test';
import {
  BROWSER_HISTORY_MAX_ENTRIES_PER_PROJECT, BROWSER_HISTORY_MAX_PROJECTS, BROWSER_HISTORY_MAX_TITLE_LENGTH, BrowserHistoryStore, adoptBrowserHistory,
  browserHistory, evictExcessProjects, formatRelativeTimeLabel, mergeBrowserHistoryState, migratePersistedBrowserHistoryState, normalizeHistoryUrl,
  recentUrlLabel, upsertHistoryEntry, type BrowserHistoryEntry,
} from './browser-history';
import { scopeThreadRef } from './terminal-ui-state';

function entry(overrides: Partial<BrowserHistoryEntry> = {}): BrowserHistoryEntry {
  return { url: 'http://localhost:3000/', lastVisitedAt: 1000, ...overrides };
}

describe('normalizeHistoryUrl', () => {
  it('normalizes bare loopback hosts to http and keeps path/query', () => {
    expect(normalizeHistoryUrl('localhost:3000/admin?tab=1')).toBe('http://localhost:3000/admin?tab=1');
  });
  it('normalizes bare public hosts to https', () => {
    expect(normalizeHistoryUrl('myapp.test')).toBe('https://myapp.test/');
  });
  it('preserves hash routes and strips credentials', () => {
    expect(normalizeHistoryUrl('http://localhost:3000/app#/route')).toBe('http://localhost:3000/app#/route');
    expect(normalizeHistoryUrl('https://user:secret@example.com/')).toBe('https://example.com/');
  });
  it('rejects non-http(s), unparseable, and oversized urls', () => {
    expect(normalizeHistoryUrl('ftp://example.com')).toBeNull();
    expect(normalizeHistoryUrl('')).toBeNull();
    expect(normalizeHistoryUrl(`http://localhost/${'a'.repeat(2048)}`)).toBeNull();
  });
});

describe('upsertHistoryEntry', () => {
  it('prepends new urls', () => {
    const next = upsertHistoryEntry([entry()], 'http://localhost:5173/', 2000);
    expect(next.map(e => e.url)).toEqual(['http://localhost:5173/', 'http://localhost:3000/']);
    expect(next[0]).toEqual({ url: 'http://localhost:5173/', lastVisitedAt: 2000 });
  });
  it('moves revisits to front, updates the timestamp, and keeps the title', () => {
    const existing = [entry({ url: 'http://a.test/', lastVisitedAt: 500, title: 'A' }), entry({ url: 'http://b.test/', lastVisitedAt: 400 })];
    const next = upsertHistoryEntry(existing, 'http://b.test/', 3000);
    expect(next.map(e => e.url)).toEqual(['http://b.test/', 'http://a.test/']);
    expect(next[0]?.lastVisitedAt).toBe(3000);
    expect(next[1]?.title).toBe('A');
  });
  it('caps the list at the per-project limit', () => {
    const full = Array.from({ length: BROWSER_HISTORY_MAX_ENTRIES_PER_PROJECT }, (_, i) => entry({ url: `http://localhost:${3000 + i}/`, lastVisitedAt: i }));
    const next = upsertHistoryEntry(full, 'http://new.test/', 9999);
    expect(next).toHaveLength(BROWSER_HISTORY_MAX_ENTRIES_PER_PROJECT);
    expect(next[0]?.url).toBe('http://new.test/');
    const lastPort = 3000 + BROWSER_HISTORY_MAX_ENTRIES_PER_PROJECT - 1;
    expect(next.some(e => e.url === `http://localhost:${lastPort}/`)).toBe(false);
    expect(next.some(e => e.url === 'http://localhost:3000/')).toBe(true);
  });
  it('with insertOrdered, slots an older entry below a newer one instead of prepending', () => {
    const next = upsertHistoryEntry([entry({ url: 'http://newer.test/', lastVisitedAt: 2000 })], 'http://older.test/', 1000, { insertOrdered: true });
    expect(next.map(e => e.url)).toEqual(['http://newer.test/', 'http://older.test/']);
  });
  it('with insertOrdered, replaying an older visit for an existing entry keeps its newer timestamp', () => {
    const next = upsertHistoryEntry([entry({ url: 'http://a.test/', lastVisitedAt: 2000 })], 'http://a.test/', 1000, { insertOrdered: true });
    expect(next).toEqual([{ url: 'http://a.test/', lastVisitedAt: 2000 }]);
  });
});

describe('evictExcessProjects', () => {
  it('keeps the most recently visited projects when over the cap', () => {
    const byProjectKey = Object.fromEntries(Array.from({ length: BROWSER_HISTORY_MAX_PROJECTS + 2 }, (_, i) => [`project-${i}`, [entry({ lastVisitedAt: i })]]));
    const next = evictExcessProjects(byProjectKey);
    expect(Object.keys(next)).toHaveLength(BROWSER_HISTORY_MAX_PROJECTS);
    expect(next['project-0']).toBeUndefined();
    expect(next['project-1']).toBeUndefined();
    expect(next[`project-${BROWSER_HISTORY_MAX_PROJECTS + 1}`]).toBeDefined();
  });
});

describe('migratePersistedBrowserHistoryState', () => {
  it('drops malformed state and invalid entries', () => {
    expect(migratePersistedBrowserHistoryState(null)).toEqual({ byProjectKey: {} });
    expect(migratePersistedBrowserHistoryState({ byProjectKey: 42 })).toEqual({ byProjectKey: {} });
    const migrated = migratePersistedBrowserHistoryState({ byProjectKey: {
      good: [{ url: 'http://a.test/', lastVisitedAt: 100, title: 'A' }, { url: '', lastVisitedAt: 100 }, { url: 'ftp://ghost.test/', lastVisitedAt: 100 }, { url: 'http://b.test/', lastVisitedAt: Number.NaN }, 'junk'],
      bad: 'junk',
    } });
    expect(migrated.byProjectKey['good']).toEqual([{ url: 'http://a.test/', lastVisitedAt: 100, title: 'A' }]);
    expect(migrated.byProjectKey['bad']).toBeUndefined();
  });
  it('normalizes persisted urls with the same rules as live writes', () => {
    const migrated = migratePersistedBrowserHistoryState({ byProjectKey: { good: [{ url: 'a.test/path#section', lastVisitedAt: 100 }] } });
    expect(migrated.byProjectKey['good']).toEqual([{ url: 'https://a.test/path#section', lastVisitedAt: 100 }]);
  });
  it('restores MRU ordering, deduplicates normalized urls, and enforces project bounds', () => {
    const byProjectKey: Record<string, unknown> = Object.fromEntries(Array.from({ length: BROWSER_HISTORY_MAX_PROJECTS + 1 }, (_, index) => [`project-${index}`, [{ url: `http://project-${index}.test/`, lastVisitedAt: index }]]));
    byProjectKey['project-1'] = [{ url: 'a.test/', lastVisitedAt: 1 }, { url: 'http://newer.test/', lastVisitedAt: 3 }, { url: 'https://a.test/', lastVisitedAt: 2 }];
    const migrated = migratePersistedBrowserHistoryState({ byProjectKey });
    expect(Object.keys(migrated.byProjectKey)).toHaveLength(BROWSER_HISTORY_MAX_PROJECTS);
    expect(migrated.byProjectKey['project-0']).toBeUndefined();
    expect(migrated.byProjectKey['project-1']).toEqual([{ url: 'http://newer.test/', lastVisitedAt: 3 }, { url: 'https://a.test/', lastVisitedAt: 2 }]);
  });
  it("rejects a lastVisitedAt outside Date's valid range", () => {
    const migrated = migratePersistedBrowserHistoryState({ byProjectKey: { good: [{ url: 'http://a.test/', lastVisitedAt: 100 }, { url: 'http://b.test/', lastVisitedAt: 1e20 }] } });
    expect(migrated.byProjectKey['good']).toEqual([{ url: 'http://a.test/', lastVisitedAt: 100 }]);
  });
  it('truncates oversized persisted titles to the contract bound', () => {
    const oversized = 'x'.repeat(BROWSER_HISTORY_MAX_TITLE_LENGTH + 100);
    const migrated = migratePersistedBrowserHistoryState({ byProjectKey: { good: [{ url: 'http://a.test/', lastVisitedAt: 100, title: oversized }] } });
    expect(migrated.byProjectKey['good']?.[0]?.title).toHaveLength(BROWSER_HISTORY_MAX_TITLE_LENGTH);
    expect(migrated.byProjectKey['good']?.[0]?.title).toBe(oversized.slice(0, BROWSER_HISTORY_MAX_TITLE_LENGTH));
  });
});

const threadRef = scopeThreadRef('env-1', 'thread-1');
let store: BrowserHistoryStore;

describe('useBrowserHistoryStore', () => {
  beforeEach(() => { store = new BrowserHistoryStore(); });

  it('records visits for registered threads under the project key', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'myapp.test/admin#section', 1234);
    expect(store.state.byProjectKey['proj-a']).toEqual([{ url: 'https://myapp.test/admin#section', lastVisitedAt: 1234 }]);
  });
  it('does not persist when a thread is already registered to the same project', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    const writes = store.writes;
    store.registerThreadProject(threadRef, 'proj-a');
    expect(store.writes).toBe(writes);
  });
  it('ignores invalid urls whether queued pending or recorded post-registration', () => {
    store.recordVisitForThread(threadRef, 'ftp://a.test/', 1);
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'ftp://a.test/', 2);
    expect(store.state.byProjectKey).toEqual({});
  });
  it('sets titles update-only via the thread helper', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.setTitleForThreadUrl(threadRef, 'http://a.test/', 'Should not create');
    expect(store.state.byProjectKey).toEqual({});
    store.recordVisitForThread(threadRef, 'http://a.test/#/settings', 1);
    store.setTitleForThreadUrl(threadRef, 'http://a.test/#/settings', 'My App');
    expect(store.state.byProjectKey['proj-a']?.[0]?.title).toBe('My App');
  });
  it('does not persist when the title is already set', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'http://a.test/', 1);
    store.setTitleForThreadUrl(threadRef, 'http://a.test/', 'My App');
    const writes = store.writes, byProjectKey = store.state.byProjectKey;
    store.setTitleForThreadUrl(threadRef, 'http://a.test/', 'My App');
    expect(store.state.byProjectKey).toBe(byProjectKey);
    expect(store.writes).toBe(writes);
  });
  it('sets a title against a settled url that differs from the stored one only by a trailing slash', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'http://a.test/community', 1);
    store.setTitleForThreadUrl(threadRef, 'http://a.test/community/', 'Community');
    expect(store.state.byProjectKey['proj-a']?.[0]).toMatchObject({ url: 'http://a.test/community', title: 'Community' });
    store.state = { ...store.state, byProjectKey: {} };
    store.recordVisitForThread(threadRef, 'http://a.test/community/', 1);
    store.setTitleForThreadUrl(threadRef, 'http://a.test/community', 'Community');
    expect(store.state.byProjectKey['proj-a']?.[0]).toMatchObject({ url: 'http://a.test/community/', title: 'Community' });
  });
  it('matches a requested localhost URL to the resolved environment host', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'http://localhost:5173/app', 1);
    store.setTitleForThreadUrl(threadRef, 'http://192.168.64.2:5173/app', 'Local App', '192.168.64.2');
    expect(store.state.byProjectKey['proj-a']?.[0]?.title).toBe('Local App');
  });
  it('deduplicates loopback aliases and the resolved environment host', () => {
    const host = '192.168.64.2';
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'http://localhost:5173/app', 1, host);
    store.recordVisitForThread(threadRef, 'http://127.0.0.1:5173/app', 2, host);
    store.recordVisitForThread(threadRef, 'http://192.168.64.2:5173/app', 3, host);
    expect(store.state.byProjectKey['proj-a']).toEqual([{ url: 'http://localhost:5173/app', lastVisitedAt: 3 }]);
    store.state = { ...store.state, byProjectKey: {} };
    store.recordVisitForThread(threadRef, 'http://192.168.64.2:5173/app', 4, host);
    store.recordVisitForThread(threadRef, 'http://localhost:5173/app', 5, host);
    expect(store.state.byProjectKey['proj-a']).toEqual([{ url: 'http://localhost:5173/app', lastVisitedAt: 5 }]);
  });
  it('does not match a genuinely different path via the trailing-slash comparison', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'http://a.test/community', 1);
    store.setTitleForThreadUrl(threadRef, 'http://a.test/community/foo', 'Foo');
    expect(store.state.byProjectKey['proj-a']?.[0]?.title).toBeUndefined();
  });
  it('updates only the most recent entry when several share a trailing-slash comparison key', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'http://a.test/community/', 1);
    store.recordVisitForThread(threadRef, 'http://a.test/community', 2);
    store.setTitleForThreadUrl(threadRef, 'http://a.test/community/', 'Community');
    const entries = store.state.byProjectKey['proj-a'];
    expect(entries?.[0]).toMatchObject({ url: 'http://a.test/community', title: 'Community' });
    expect(entries?.[1]).toMatchObject({ url: 'http://a.test/community/' });
    expect(entries?.[1]?.title).toBeUndefined();
  });
  it('truncates oversized titles to the contract bound', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'http://a.test/', 1);
    const oversized = 'y'.repeat(BROWSER_HISTORY_MAX_TITLE_LENGTH + 50);
    store.setTitleForThreadUrl(threadRef, 'http://a.test/', oversized);
    const title = store.state.byProjectKey['proj-a']?.[0]?.title;
    expect(title).toHaveLength(BROWSER_HISTORY_MAX_TITLE_LENGTH);
    expect(title).toBe(oversized.slice(0, BROWSER_HISTORY_MAX_TITLE_LENGTH));
  });
  it('removes entries', () => {
    store.registerThreadProject(threadRef, 'proj-a');
    store.recordVisitForThread(threadRef, 'http://a.test/', 1);
    store.recordVisitForThread(threadRef, 'http://b.test/', 2);
    store.removeUrlForThread(threadRef, 'http://a.test/');
    expect(store.state.byProjectKey['proj-a']?.map(e => e.url)).toEqual(['http://b.test/']);
  });
});

describe('pendingVisitsByThreadKey', () => {
  beforeEach(() => { store = new BrowserHistoryStore(); });

  it('queues a visit recorded before registration and drains it in order on registration', () => {
    store.recordVisitForThread(threadRef, 'http://a.test/', 1);
    store.recordVisitForThread(threadRef, 'http://b.test/', 2);
    expect(store.state.byProjectKey).toEqual({});
    store.registerThreadProject(threadRef, 'proj-a');
    expect(store.state.byProjectKey['proj-a']?.map(e => e.url)).toEqual(['http://b.test/', 'http://a.test/']);
    expect(store.state.byProjectKey['proj-a']?.[0]?.lastVisitedAt).toBe(2);
    expect(store.state.byProjectKey['proj-a']?.[1]?.lastVisitedAt).toBe(1);
    expect(store.state.pendingVisitsByThreadKey).toEqual({});
  });
  it('caps the per-thread pending list at 10, dropping the oldest', () => {
    for (let i = 0; i < 12; i++) store.recordVisitForThread(threadRef, `http://a.test/${i}`, i);
    store.registerThreadProject(threadRef, 'proj-a');
    const urls = store.state.byProjectKey['proj-a']?.map(e => e.url);
    expect(urls).toHaveLength(10);
    expect(urls).not.toContain('http://a.test/0');
    expect(urls).not.toContain('http://a.test/1');
    expect(urls?.[0]).toBe('http://a.test/11');
  });
  it('slots a replayed visit by timestamp instead of hoisting it above a newer live visit', () => {
    const otherThreadRef = scopeThreadRef('env-1', 'thread-2');
    store.registerThreadProject(otherThreadRef, 'proj-a');
    store.recordVisitForThread(otherThreadRef, 'http://newer.test/', 2000);
    store.recordVisitForThread(threadRef, 'http://older.test/', 1000);
    store.registerThreadProject(threadRef, 'proj-a');
    const entries = store.state.byProjectKey['proj-a'];
    expect(entries?.map(e => e.url)).toEqual(['http://newer.test/', 'http://older.test/']);
    expect(entries?.[0]?.lastVisitedAt).toBe(2000);
  });
});

describe('pendingTitlesByThreadKey', () => {
  beforeEach(() => { store = new BrowserHistoryStore(); });

  it('buffers a title set before registration and applies it once the matching visit drains', () => {
    store.recordVisitForThread(threadRef, 'http://a.test/', 1);
    store.setTitleForThreadUrl(threadRef, 'http://a.test/', 'My App');
    expect(store.state.byProjectKey).toEqual({});
    store.registerThreadProject(threadRef, 'proj-a');
    expect(store.state.byProjectKey['proj-a']?.[0]).toMatchObject({ url: 'http://a.test/', title: 'My App' });
    expect(store.state.pendingTitlesByThreadKey).toEqual({});
  });
  it('preserves environment host matching while a title is pending', () => {
    store.recordVisitForThread(threadRef, 'http://localhost:5173/app', 1);
    store.setTitleForThreadUrl(threadRef, 'http://192.168.64.2:5173/app', 'Local App', '192.168.64.2');
    store.registerThreadProject(threadRef, 'proj-a');
    expect(store.state.byProjectKey['proj-a']?.[0]?.title).toBe('Local App');
  });
});

describe('mergeBrowserHistoryState', () => {
  it('sanitizes same-version corrupt persisted data and preserves actions', () => {
    const merged = mergeBrowserHistoryState({
      byProjectKey: { a: [{ url: 'ftp://bad.test/', lastVisitedAt: 1 }], b: [{ url: 'http://ok.test/', lastVisitedAt: 5 }] },
      projectKeyByThreadKey: { good: 'b', stale: 'a', malformed: 42 },
    });
    expect(merged.byProjectKey).toEqual({ b: [{ url: 'http://ok.test/', lastVisitedAt: 5 }] });
    expect(typeof new BrowserHistoryStore(merged).recordVisit).toBe('function');
    expect(merged.projectKeyByThreadKey).toEqual({ good: 'b' });
    expect(merged.pendingVisitsByThreadKey).toEqual({});
    expect(merged.pendingTitlesByThreadKey).toEqual({});
  });
});

describe('the saved preferences (clone)', () => {
  it('saves the current history with client.local and reads it back sanitized', () => {
    const local: Record<string, unknown> = {};
    const history = browserHistory(local);
    history.registerThreadProject(threadRef, 'proj-a');
    history.recordVisitForThread(threadRef, 'localhost:5173', 7);
    const saved = JSON.parse(JSON.stringify(local)) as Record<string, unknown>;
    expect(saved.browserHistory).toEqual({ byProjectKey: { 'proj-a': [{ url: 'http://localhost:5173/', lastVisitedAt: 7 }] }, projectKeyByThreadKey: { [`env-1:thread-1`]: 'proj-a' } });
    const next: Record<string, unknown> = {};
    adoptBrowserHistory(next, { browserHistory: { ...(saved.browserHistory as object), byProjectKey: { ...(saved.browserHistory as { byProjectKey: object }).byProjectKey, junk: 'x' } } });
    expect(browserHistory(next).threadRecentHistory(threadRef, 8)).toEqual([{ url: 'http://localhost:5173/', lastVisitedAt: 7 }]);
  });
  it('labels a recent visit as the reference does', () => {
    expect(formatRelativeTimeLabel(1_000, 30_000)).toBe('just now');
    expect(formatRelativeTimeLabel(0, 5 * 60_000)).toBe('5m ago');
    expect(formatRelativeTimeLabel(0, 3 * 3_600_000)).toBe('3h ago');
    expect(formatRelativeTimeLabel(0, 2 * 86_400_000)).toBe('2d ago');
    expect(recentUrlLabel('http://localhost:5173/')).toBe('localhost:5173');
    expect(recentUrlLabel('https://a.test/docs?x=1#top')).toBe('a.test/docs?x=1#top');
  });
});
