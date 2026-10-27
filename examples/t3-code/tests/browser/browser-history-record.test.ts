// browserHistoryStore.ts at 7c3b3b3605ef (MIT, see LICENSE-T3), as threads use it: every thread's visits and titles
// reach its project's Recently used (#25491), and a thread whose tab settles again on the same page and title leaves the
// newer title another thread gave it (#25521). Two threads of one project, each with a Browser tab.
import { describe, expect, it } from 'bun:test';
import { recentHistory, recordPreviewTitles, recordVisit, threadHistoryProjectKey } from '../../src/browser/browser-history-record';
import { browserHistory } from '../../src/browser/browser-history';
import { browserHost } from '../../src/browser/browser-surface';
import type { PreviewSessionSnapshot } from '../../src/browser/browser-state';
import { EnvironmentFleet } from '../../src/connections/settings-b-fleet';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';

const threadA = { environmentId: 'local', threadId: 'thread-a' }, threadB = { environmentId: 'local', threadId: 'thread-b' };
let clock = 0;
const session = (threadId: string, url: string, title: string): PreviewSessionSnapshot => ({
  threadId, tabId: 'tab-1', navStatus: { _tag: 'Success', url, title }, canGoBack: false, canGoForward: false, updatedAt: new Date(Date.UTC(2026, 9, 27) + ++clock * 1000).toISOString(), runtime: 'server',
} as PreviewSessionSnapshot);

function setup() {
  const client = {
    environmentId: 'local', threadId: 'thread-a', projectId: 'p1', preferencesLoaded: true, origin: 'http://127.0.0.1:3773',
    shell: { threads: [{ id: 'thread-a', projectId: 'p1' }, { id: 'thread-b', projectId: 'p1' }], projects: [{ id: 'p1', workspaceRoot: '/repo', repositoryIdentity: { canonicalKey: 'github.com/acme/app' } }] },
    local: { clientSettings: {}, composerControls: { draftThreads: {} }, groupingMode: 'repository' } as Obj,
  } as unknown as T3Client;
  const show = (ref: typeof threadA, url: string, title: string) => browserHost(client).store.applyServerSnapshot(ref, session(ref.threadId, url, title));
  return { client, show, fleet: new EnvironmentFleet() };
}

describe('Browser history follows the project, not the thread', () => {
  it('lists thread A\'s visits, titled, in thread B of the same project', () => {
    const { client, show, fleet } = setup();
    show(threadA, 'http://localhost:5173/', 'Home');
    recordPreviewTitles(client);
    recordVisit(client, threadA, 'http://localhost:5173/', 1, fleet);
    show(threadA, 'http://localhost:5173/docs', 'Docs');
    recordVisit(client, threadA, 'http://localhost:5173/docs', 2, fleet);
    recordPreviewTitles(client);
    expect(threadHistoryProjectKey(client, threadB, fleet)).toBe('github.com/acme/app');
    expect(recentHistory(client, threadB, 8, fleet).map(entry => [entry.url, entry.title])).toEqual([['http://localhost:5173/docs', 'Docs'], ['http://localhost:5173/', 'Home']]);
  });
  it('keeps the newer title when thread A\'s tab settles again on its older one', () => {
    const { client, show, fleet } = setup();
    show(threadA, 'http://localhost:5173/', 'Old');
    recordPreviewTitles(client);
    recordVisit(client, threadA, 'http://localhost:5173/', 1, fleet);
    // Thread B opens the same page, which now loads with a newer title.
    recordVisit(client, threadB, 'http://localhost:5173/', 2, fleet);
    show(threadB, 'http://localhost:5173/', 'New');
    recordPreviewTitles(client);
    expect(recentHistory(client, threadA, 8, fleet)[0]?.title).toBe('New');
    // Back in thread A its tab is replaced by an equal snapshot (a list reconcile): no older title comes back.
    show(threadA, 'http://localhost:5173/', 'Old');
    recordPreviewTitles(client);
    expect(recentHistory(client, threadA, 8, fleet)[0]?.title).toBe('New');
    // A page that really retitles does write.
    show(threadA, 'http://localhost:5173/', 'Retitled');
    recordPreviewTitles(client);
    expect(recentHistory(client, threadA, 8, fleet)[0]?.title).toBe('Retitled');
  });
  it('reads a draft\'s project from its draft key', () => {
    const { client, fleet } = setup();
    (client.local.composerControls as Obj).draftThreads = { 'local:new:p1': 'draft-1' };
    expect(threadHistoryProjectKey(client, { environmentId: 'local', threadId: 'draft-1' }, fleet)).toBe('github.com/acme/app');
    expect(browserHistory(client.local).registeredProject({ environmentId: 'local', threadId: 'draft-1' })).toBeUndefined();
  });
});
