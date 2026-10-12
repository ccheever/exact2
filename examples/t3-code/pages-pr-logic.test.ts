// Ported tests (pr-conversation-and-refresh): the names and cases of T3 Code 1e2ecbd975
// (MIT, see LICENSE-T3) apps/web/src/components/pullRequest/pullRequestDetail.logic.test.ts
// ("pull request activity refresh", "review thread comment pages", "ordering comments", "review
// verdicts", "pull request timeline", "cached pull request detail", "what to say when an action
// fails") and apps/web/src/hooks/useLiveRefresh.test.ts (shouldLiveRefresh, shouldRefreshOnArrival,
// shouldRefreshOnInterval and the interval constant). Not ported, as React-hook-only (n/a-ui):
// useLiveRefresh.test.ts "live refresh cadence" (the hook's timers and listeners over a fake
// document; here the panel's arguments are the window's facts and clock, pages-pr-conversation.test.ts
// covers them) and "keeps an idle view paused after focus/visibilitychange until input".
import { describe, expect, it } from 'bun:test';
import type { Obj } from './domain';
import {
  LIVE_REFRESH_IDLE_AFTER_MS, LIVE_REFRESH_INTERVAL_MS, LIVE_REFRESH_MIN_INTERVAL_MS, buildPullRequestTimeline, editPullRequestThreadComment, groupPullRequestTimelineConversations,
  isPullRequestVerdictStale, latestPullRequestReviewOutcomes, mergePullRequestThreadComments, newestPullRequestCommitAt, orderPullRequestComments, pullRequestReviewOutcome,
  pullRequestReviewOutcomeLabel, pullRequestReviewOutcomeStaleLabel, readPullRequestDetailSnapshot, resolveDisplayedPullRequestDetail, resolvePullRequestReferenceHost,
  shouldLiveRefresh, shouldRefreshOnArrival, shouldRefreshOnInterval, shouldRefreshPullRequestActivity, writePullRequestDetailSnapshot,
} from './pages-pr-logic';
import { readableFailure } from './r6-pr-logic';

const TIMELINE_SOURCE = {
  createdAt: '2026-07-01T00:00:00Z',
  author: { login: 'octocat', name: null, avatarUrl: null },
  commits: [{ oid: '1baf7bdcafe', messageHeadline: 'add the page', committedDate: '2026-07-02T00:00:00Z' }] as Obj[],
  comments: [{ id: 'c1', kind: 'issue-comment', author: { login: 'bilal', name: null, avatarUrl: null }, body: 'looks good', createdAt: '2026-07-03T00:00:00Z', url: null, path: null, reviewState: null }] as Obj[],
  mergedAt: null as string | null,
  closedAt: null as string | null,
};
const first = TIMELINE_SOURCE.comments[0]!;

describe('pull request activity refresh', () => {
  const head = { key: 'project:acme/web#7', updatedAt: '2026-08-13T13:00:00Z' };
  it('refreshes activity only after the same pull request changes', () => {
    expect(shouldRefreshPullRequestActivity(head, { ...head, updatedAt: '2026-08-13T13:01:00Z' })).toBe(true);
  });
  it('does not duplicate the first activity read or carry a revision across pull requests', () => {
    expect(shouldRefreshPullRequestActivity(null, head)).toBe(false);
    expect(shouldRefreshPullRequestActivity(head, head)).toBe(false);
    expect(shouldRefreshPullRequestActivity(head, { key: 'project:acme/web#8', updatedAt: '2026-08-13T13:01:00Z' })).toBe(false);
  });
});

describe('review thread comment pages', () => {
  it('appends new comments once and keeps refreshed base comments', () => {
    expect(mergePullRequestThreadComments([{ id: 'c1', body: 'refreshed' }, { id: 'c2', body: 'already in base' }], [{ id: 'c2', body: 'stale page copy' }, { id: 'c3', body: 'next page' }]))
      .toEqual([{ id: 'c1', body: 'refreshed' }, { id: 'c2', body: 'already in base' }, { id: 'c3', body: 'next page' }]);
  });
  it('keeps a loaded comment after its body is edited', () => {
    expect(editPullRequestThreadComment([{ id: 'c2', body: 'old body' }, { id: 'c3', body: 'another loaded comment' }], 'c2', 'saved body'))
      .toEqual([{ id: 'c2', body: 'saved body' }, { id: 'c3', body: 'another loaded comment' }]);
  });
});

describe('ordering comments', () => {
  it('reverses the chronological list for newest first, and leaves oldest first alone', () => {
    const comments = [{ createdAt: 'a' }, { createdAt: 'b' }, { createdAt: 'c' }];
    expect(orderPullRequestComments(comments, 'newest')).toEqual([{ createdAt: 'c' }, { createdAt: 'b' }, { createdAt: 'a' }]);
    expect(orderPullRequestComments(comments, 'oldest')).toEqual(comments);
    expect(comments).toEqual([{ createdAt: 'a' }, { createdAt: 'b' }, { createdAt: 'c' }]);
  });
});

describe('review verdicts', () => {
  it('reads the same three verdicts however a host spells them', () => {
    expect(pullRequestReviewOutcome('APPROVED')).toBe('approved');
    expect(pullRequestReviewOutcome('approved')).toBe('approved');
    expect(pullRequestReviewOutcome('CHANGES_REQUESTED')).toBe('changes-requested');
    expect(pullRequestReviewOutcome('changes_requested')).toBe('changes-requested');
    expect(pullRequestReviewOutcome('DISMISSED')).toBe('dismissed');
  });
  it('is not a verdict where the review only carried remarks', () => {
    expect(pullRequestReviewOutcome('COMMENTED')).toBeNull();
    expect(pullRequestReviewOutcome('PENDING')).toBeNull();
    expect(pullRequestReviewOutcome(null)).toBeNull();
  });
  it("keeps each reviewer's last word, whatever order the host returned them in", () => {
    const review = (id: string, login: string, reviewState: string, createdAt: string): Obj => ({ id, kind: 'review', author: { login, name: null, avatarUrl: null }, body: '', createdAt, url: null, path: null, reviewState });
    expect(latestPullRequestReviewOutcomes([
      review('r3', 'bilal', 'APPROVED', '2026-07-03T00:00:00Z'), review('r1', 'bilal', 'CHANGES_REQUESTED', '2026-07-01T00:00:00Z'),
      review('r2', 'octocat', 'CHANGES_REQUESTED', '2026-07-02T00:00:00Z'), review('r4', 'octocat', 'COMMENTED', '2026-07-04T00:00:00Z'),
    ]).map(entry => [entry.actor?.login, entry.outcome])).toEqual([['bilal', 'approved'], ['octocat', 'changes-requested']]);
  });
  it('keeps two deleted accounts apart rather than counting them as one reviewer', () => {
    expect(latestPullRequestReviewOutcomes([
      { ...first, id: 'r1', kind: 'review', author: null, reviewState: 'APPROVED', createdAt: '2026-07-01T00:00:00Z' },
      { ...first, id: 'r2', kind: 'review', author: null, reviewState: 'APPROVED', createdAt: '2026-07-02T00:00:00Z' },
    ])).toHaveLength(2);
  });
  it('gives every entry a key that separates the reviewers it kept apart', () => {
    const entries = latestPullRequestReviewOutcomes([
      { ...first, id: 'r1', kind: 'review', author: null, reviewState: 'APPROVED', createdAt: '2026-07-01T00:00:00Z' },
      { ...first, id: 'r2', kind: 'review', author: null, reviewState: 'APPROVED', createdAt: '2026-07-01T00:00:00Z' },
    ]);
    expect(new Set(entries.map(entry => entry.key)).size).toBe(2);
  });
  it('calls a verdict stale once commits land after it, and current before that', () => {
    const commits = [{ oid: 'c0ffee', messageHeadline: 'later work', committedDate: '2026-07-05T00:00:00Z' }];
    const review = (createdAt: string): Obj => ({ ...first, kind: 'review', reviewState: 'APPROVED', createdAt });
    expect(latestPullRequestReviewOutcomes([review('2026-07-01T00:00:00Z')], commits)[0]?.stale).toBe(true);
    expect(latestPullRequestReviewOutcomes([review('2026-07-06T00:00:00Z')], commits)[0]?.stale).toBe(false);
    expect(latestPullRequestReviewOutcomes([review('2026-07-01T00:00:00Z')], [])[0]?.stale).toBe(false);
  });
  it('measures staleness against the newest commit, not the last one listed', () => {
    expect(newestPullRequestCommitAt([{ oid: 'a', messageHeadline: '', committedDate: '2026-07-09T00:00:00Z' }, { oid: 'b', messageHeadline: '', committedDate: '2026-07-02T00:00:00Z' }])).toBe('2026-07-09T00:00:00Z');
    expect(newestPullRequestCommitAt([])).toBeNull();
  });
  it('orders instants rather than their text, so a UTC offset cannot invert them', () => {
    expect(newestPullRequestCommitAt([{ oid: 'a', messageHeadline: '', committedDate: '2026-07-05T00:30:00Z' }, { oid: 'b', messageHeadline: '', committedDate: '2026-07-05T01:00:00+02:00' }])).toBe('2026-07-05T00:30:00Z');
    expect(isPullRequestVerdictStale('2026-07-05T00:30:00Z', '2026-07-05T01:00:00+02:00')).toBe(false);
    expect(isPullRequestVerdictStale('2026-07-01T00:00:00Z', 'not a date')).toBe(false);
    expect(newestPullRequestCommitAt([{ oid: 'a', messageHeadline: '', committedDate: 'not a date' }])).toBeNull();
  });
  it('shows nothing for a reviewer whose verdict was dismissed', () => {
    expect(latestPullRequestReviewOutcomes([
      { ...first, kind: 'review', reviewState: 'APPROVED', createdAt: '2026-07-01T00:00:00Z' },
      { ...first, id: 'c2', kind: 'review', reviewState: 'DISMISSED', createdAt: '2026-07-02T00:00:00Z' },
    ])).toEqual([]);
  });
  // pullRequestPresentation.tsx: the words a verdict and a superseded verdict read as.
  it('labels each verdict, and a stale one with when it applied', () => {
    expect(['approved', 'changes-requested', 'dismissed'].map(outcome => pullRequestReviewOutcomeLabel(outcome as 'approved'))).toEqual(['Approved', 'Changes requested', 'Review dismissed']);
    expect(pullRequestReviewOutcomeStaleLabel('approved')).toBe('Approved earlier changes');
  });
});

describe('pull request timeline', () => {
  it('orders creation, commits and comments newest first', () => {
    expect(buildPullRequestTimeline(TIMELINE_SOURCE).map(event => event.id)).toEqual(['c1', '1baf7bdcafe', 'created']);
  });
  it('carries the comment url, and leaves the events the host cannot address without one', () => {
    expect(buildPullRequestTimeline({ ...TIMELINE_SOURCE, comments: [{ ...first, url: 'https://example.test/pull/1#c1' }] }).map(event => event.url)).toEqual(['https://example.test/pull/1#c1', null, null]);
  });
  it('carries actors and review context into the presentation model', () => {
    const commitAuthors = [{ login: 'octocat', name: null, avatarUrl: 'https://example.test/octocat.png' }, { login: 'pair', name: 'Pair Author', avatarUrl: null }];
    const events = buildPullRequestTimeline({ ...TIMELINE_SOURCE, commits: [{ ...TIMELINE_SOURCE.commits[0]!, authors: commitAuthors }], comments: [{ ...first, kind: 'review', path: 'src/app.ts', reviewState: 'APPROVED' }] });
    expect(events.find(event => event.kind === 'commit')?.commitAuthors).toEqual(commitAuthors);
    expect(events.find(event => event.kind === 'review')).toMatchObject({ actor: first.author, path: 'src/app.ts', reviewState: 'APPROVED' });
  });
  it("carries each commit's line counts into its timeline event", () => {
    expect(buildPullRequestTimeline({ ...TIMELINE_SOURCE, commits: [{ ...TIMELINE_SOURCE.commits[0]!, additions: 12, deletions: 4 }] }).find(event => event.kind === 'commit')).toMatchObject({ additions: 12, deletions: 4 });
  });
  it("drops a body that is nothing but a bot's HTML comment, and keeps one that says more", () => {
    const events = buildPullRequestTimeline({ ...TIMELINE_SOURCE, comments: [{ ...first, body: '<!-- MURMUR_IGNORE -->' }, { ...first, id: 'c2', body: '<!-- summarize by coderabbit.ai -->\nNeeds a test.', createdAt: '2026-07-04T00:00:00Z' }] });
    expect(events.find(event => event.id === 'c1')?.body).toBeNull();
    expect(events.find(event => event.id === 'c2')?.body).toBe('<!-- summarize by coderabbit.ai -->\nNeeds a test.');
  });
  it('calls a comment markdown and a commit headline plain text', () => {
    const events = buildPullRequestTimeline(TIMELINE_SOURCE);
    expect(events.map(event => [event.title.startsWith('Commit'), event.markdown])).toEqual(expect.arrayContaining([[true, false]]));
    expect(events.find(event => event.id === 'c1')?.markdown).toBe(true);
  });
  it('reports a merge rather than the close GitHub records alongside it', () => {
    const events = buildPullRequestTimeline({ ...TIMELINE_SOURCE, mergedAt: '2026-07-04T00:00:00Z', closedAt: '2026-07-04T00:00:00Z' });
    expect(events[0]?.id).toBe('merged');
    expect(events.some(event => event.id === 'closed')).toBe(false);
  });
  it('groups conversation sections without crossing commits or PR updates', () => {
    const events = buildPullRequestTimeline({ ...TIMELINE_SOURCE, comments: [
      { ...first, id: 'new-comment-1', createdAt: '2026-07-04T00:00:00Z' }, { ...first, id: 'new-comment-2', createdAt: '2026-07-03T00:00:00Z' },
      { ...first, id: 'old-comment-1', createdAt: '2026-07-01T12:00:00Z' }, { ...first, id: 'old-comment-2', createdAt: '2026-07-01T06:00:00Z' },
    ], mergedAt: '2026-07-05T00:00:00Z' });
    expect(groupPullRequestTimelineConversations(events).map(row => (row.kind === 'comments' ? [row.kind, ...row.events.map(event => event.id)] : [row.kind, row.event.id]))).toEqual([
      ['event', 'merged'], ['comments', 'new-comment-1', 'new-comment-2'], ['event', '1baf7bdcafe'], ['comments', 'old-comment-1', 'old-comment-2'], ['event', 'created'],
    ]);
  });
  it('keeps a verdict out of the collapsed conversation it was submitted in', () => {
    const events = buildPullRequestTimeline({ ...TIMELINE_SOURCE, comments: [
      { ...first, id: 'chatter-1', createdAt: '2026-07-05T00:00:00Z' }, { ...first, id: 'approval', kind: 'review', body: '', reviewState: 'APPROVED', createdAt: '2026-07-04T00:00:00Z' },
      { ...first, id: 'chatter-2', createdAt: '2026-07-03T00:00:00Z' }, { ...first, id: 'remark', kind: 'review', reviewState: 'COMMENTED', createdAt: '2026-07-02T12:00:00Z' },
    ] });
    expect(groupPullRequestTimelineConversations(events).map(row => (row.kind === 'comments' ? [row.kind, ...row.events.map(event => event.id)] : [row.kind, row.event.id]))).toEqual([
      ['comments', 'chatter-1'], ['event', 'approval'], ['comments', 'chatter-2', 'remark'], ['event', '1baf7bdcafe'], ['event', 'created'],
    ]);
  });
});

describe('what to say when an action fails', () => {
  const hint = 'The host refused the merge. Check that you have write access.';
  it("says the host's own reason, without the operation it arrived wrapped in", () => {
    expect(readableFailure(new Error('Pull request operation runAction failed: At least 1 approving review is required.'), hint)).toBe('At least 1 approving review is required.');
  });
  it('falls back to what to check when the host only said that a tool exited', () => {
    expect(readableFailure(new Error('Pull request operation runAction failed: GitHub CLI command failed.'), hint)).toBe(hint);
    expect(readableFailure(new Error('exited with code 1'), hint)).toBe(hint);
    expect(readableFailure(undefined, hint)).toBe(hint);
  });
  it('bounds a host that answers with a page of output', () => {
    const long = readableFailure(new Error('x'.repeat(900)), hint);
    expect(long.length).toBeLessThanOrEqual(320);
    expect(long.endsWith('…')).toBe(true);
  });
});

describe('cached pull request detail', () => {
  const reference = { projectId: 'project-1', repository: 'acme/web', number: 7 };
  const detail = (overrides: Obj = {}): Obj => ({
    provider: 'github', projectId: 'project-1', projectTitle: 'web', workspaceRoot: '/repo', repository: 'acme/web', number: 7, title: 'Cache the title', body: 'who made it',
    url: 'https://github.com/acme/web/pull/7', author: { login: 'octocat', name: null, avatarUrl: 'https://avatars.example/octocat' }, state: 'open', isDraft: false, mergeability: 'mergeable',
    additions: 12, deletions: 3, changedFiles: 2, headBranch: 'feat/cache', baseBranch: 'main', createdAt: '2026-07-01T00:00:00.000Z', updatedAt: '2026-07-02T00:00:00.000Z',
    mergedAt: null, closedAt: null, reviewers: [], labels: [], checks: [], ...overrides,
  });
  const makeStorage = () => { const held = new Map<string, string>(); return { getItem: (key: string) => held.get(key) ?? null, setItem: (key: string, value: string) => void held.set(key, value) }; };
  it('hydrates the last title, author, and counts so a reopen does not ghost the tab', () => {
    const storage = makeStorage();
    writePullRequestDetailSnapshot(storage, 'env-1', reference, detail());
    const snapshot = readPullRequestDetailSnapshot(storage, 'env-1', reference);
    expect([snapshot?.title, (snapshot?.author as Obj | undefined)?.login, snapshot?.additions, snapshot?.deletions]).toEqual(['Cache the title', 'octocat', 12, 3]);
  });
  it('reuses a host-qualified snapshot when reopening a thread link without a host', () => {
    const storage = makeStorage();
    writePullRequestDetailSnapshot(storage, 'env-1', { ...reference, host: 'github.com' }, detail());
    const resolved = resolvePullRequestReferenceHost(reference, { canonicalKey: 'github.com/acme/web', locator: { source: 'git-remote', remoteName: 'origin', remoteUrl: 'https://github.com/acme/web.git' }, provider: 'github' });
    expect(readPullRequestDetailSnapshot(storage, 'env-1', resolved)?.title).toBe('Cache the title');
    const explicit = { ...reference, host: 'github.example.com' };
    expect(resolvePullRequestReferenceHost(explicit, { canonicalKey: 'github.com/acme/web', locator: { source: 'git-remote', remoteName: 'origin', remoteUrl: 'https://github.com/acme/web.git' } })).toBe(explicit);
  });
  it('leaves server-resolved Azure SSH references unchanged', () => {
    expect(resolvePullRequestReferenceHost(reference, { canonicalKey: 'ssh.dev.azure.com/v3/org/project/web', locator: { source: 'git-remote', remoteName: 'origin', remoteUrl: 'git@ssh.dev.azure.com:v3/org/project/web' }, provider: 'azure-devops' })).toBe(reference);
    expect(resolvePullRequestReferenceHost(reference, undefined)).toBe(reference);
  });
  it('hydrates legacy hostless snapshots only for the matching host', () => {
    const storage = makeStorage();
    writePullRequestDetailSnapshot(storage, 'env-1', reference, detail());
    expect(readPullRequestDetailSnapshot(storage, 'env-1', { ...reference, host: 'github.com' })?.title).toBe('Cache the title');
    expect(readPullRequestDetailSnapshot(storage, 'env-1', { ...reference, host: 'github.example.com' })).toBeNull();
  });
  it('keeps Forgejo ports isolated when recovering legacy snapshots', () => {
    const storage = makeStorage();
    const cached = detail({ provider: 'forgejo', url: 'https://forge.example:8443/acme/web/pulls/7' });
    writePullRequestDetailSnapshot(storage, 'env-1', reference, cached);
    expect(readPullRequestDetailSnapshot(storage, 'env-1', { ...reference, host: 'forge.example:8443' })?.title).toBe(cached.title);
    expect(readPullRequestDetailSnapshot(storage, 'env-1', { ...reference, host: 'forge.example:9443' })).toBeNull();
    expect(readPullRequestDetailSnapshot(storage, 'env-1', { ...reference, host: 'forge.example' })).toBeNull();
  });
  for (const provider of ['github', 'gitlab']) {
    it(`retains portless ${provider} snapshot identities for custom web ports`, () => {
      const storage = makeStorage(), host = `${provider}.example.com`, hosted = { ...reference, host };
      const cached = detail({ provider, url: `https://${host}:8443/acme/web/${provider === 'github' ? 'pull' : '-/merge_requests'}/7` });
      writePullRequestDetailSnapshot(storage, 'env-1', hosted, cached);
      expect(readPullRequestDetailSnapshot(storage, 'env-1', hosted)?.title).toBe(cached.title);
    });
  }
  it('keeps a cached tab painted while the live read replaces the counts', () => {
    const cached = detail(), live = detail({ additions: 40, deletions: 9 });
    expect(resolveDisplayedPullRequestDetail({ live, cached, reference })?.additions).toBe(40);
    expect(resolveDisplayedPullRequestDetail({ live: null, cached, reference })?.additions).toBe(12);
  });
  it("does not paint another change request's snapshot", () => {
    expect(resolveDisplayedPullRequestDetail({ live: null, cached: detail({ number: 8 }), reference })).toBeNull();
    expect(readPullRequestDetailSnapshot(makeStorage(), 'env-2', reference)).toBeNull();
  });
  it('isolates stored and displayed details between hosts with the same repository and number', () => {
    const storage = makeStorage(), publicRef = { ...reference, host: 'github.com' }, enterpriseRef = { ...reference, host: 'ghe.example.com' };
    const publicDetail = detail(), enterpriseDetail = detail({ title: 'Enterprise change', url: 'https://ghe.example.com/acme/web/pull/7' });
    writePullRequestDetailSnapshot(storage, 'env-1', publicRef, publicDetail);
    expect(readPullRequestDetailSnapshot(storage, 'env-1', enterpriseRef)).toBeNull();
    writePullRequestDetailSnapshot(storage, 'env-1', enterpriseRef, enterpriseDetail);
    expect(readPullRequestDetailSnapshot(storage, 'env-1', publicRef)?.title).toBe(publicDetail.title);
    expect(readPullRequestDetailSnapshot(storage, 'env-1', enterpriseRef)?.title).toBe(enterpriseDetail.title);
    expect(resolveDisplayedPullRequestDetail({ live: null, cached: publicDetail, reference: enterpriseRef })).toBeNull();
    expect(resolveDisplayedPullRequestDetail({ live: null, cached: enterpriseDetail, reference: enterpriseRef })).toBe(enterpriseDetail);
    writePullRequestDetailSnapshot(storage, 'env-1', enterpriseRef, publicDetail);
    expect(readPullRequestDetailSnapshot(storage, 'env-1', enterpriseRef)).toBeNull();
  });
  it('shrugs off corrupt storage and no storage at all', () => {
    const storage = makeStorage();
    storage.setItem('t3.pullRequests.detail:env-1:project-1:acme/web#7', '{not json');
    expect(readPullRequestDetailSnapshot(storage, 'env-1', reference)).toBeNull();
    expect(readPullRequestDetailSnapshot(undefined, 'env-1', reference)).toBeNull();
    const hosted = { ...reference, host: 'github.com' };
    writePullRequestDetailSnapshot(storage, 'env-1', hosted, detail({ url: 'invalid url' }));
    expect(readPullRequestDetailSnapshot(storage, 'env-1', hosted)).toBeNull();
  });
});

describe('live refresh cadence', () => {
  it('waits five minutes between automatic host reads', () => {
    expect(LIVE_REFRESH_INTERVAL_MS).toBe(5 * 60_000);
  });
});

describe('shouldLiveRefresh', () => {
  const at = (now: number, lastRefreshedAt: number, visible = true) => shouldLiveRefresh({ visible, now, lastRefreshedAt });
  it('reads a view again when it is navigated to', () => { expect(at(LIVE_REFRESH_MIN_INTERVAL_MS, 0)).toBe(true); });
  it('does not read a view again that was left and returned to seconds later', () => { expect(at(3_000, 0)).toBe(false); });
  it('reads again when the interval comes round on a view left open', () => { expect(at(LIVE_REFRESH_INTERVAL_MS, 0)).toBe(true); });
  it('does not read again for every window tabbed through', () => { expect(at(1_000, 0)).toBe(false); });
  it('stays quiet while the window is not showing', () => { expect(at(LIVE_REFRESH_MIN_INTERVAL_MS * 5, 0, false)).toBe(false); });
  it('reads once for a window hidden an hour, not once per interval it missed', () => {
    const hour = 60 * 60_000;
    let lastRefreshedAt = 0, reads = 0;
    const tick = (now: number, visible: boolean) => { if (!at(now, lastRefreshedAt, visible)) return; lastRefreshedAt = now; reads += 1; };
    for (let now = LIVE_REFRESH_INTERVAL_MS; now < hour; now += LIVE_REFRESH_INTERVAL_MS) tick(now, false);
    tick(hour, true); tick(hour, true);
    expect(reads).toBe(1);
  });
});

describe('shouldRefreshOnArrival', () => {
  it('leaves a view alone the first time it is opened, because it is already reading', () => {
    expect(shouldRefreshOnArrival({ visible: true, now: 5_000, lastRefreshedAt: undefined })).toBe(false);
  });
  it('reads a view that was read earlier in the session and returned to', () => {
    expect(shouldRefreshOnArrival({ visible: true, now: 90_000, lastRefreshedAt: 0 })).toBe(true);
  });
  it('keeps the minimum interval on a view returned to straight away', () => {
    expect(shouldRefreshOnArrival({ visible: true, now: 2_000, lastRefreshedAt: 0 })).toBe(false);
  });
});

describe('shouldRefreshOnInterval', () => {
  const tick = (now: number, lastInteractedAt: number) => shouldRefreshOnInterval({ visible: true, now, lastRefreshedAt: 0, lastInteractedAt });
  it('reads for a reader who is here', () => { expect(tick(LIVE_REFRESH_INTERVAL_MS, LIVE_REFRESH_INTERVAL_MS - 1_000)).toBe(true); });
  it('reads on the first interval after an untouched mount', () => { expect(tick(LIVE_REFRESH_INTERVAL_MS + 1_000, 0)).toBe(true); });
  it('stops reading for a window left showing on a desk nobody is at', () => { expect(tick(LIVE_REFRESH_IDLE_AFTER_MS + 60_000, 0)).toBe(false); });
  it('starts reading again once the reader touches the window', () => {
    const away = LIVE_REFRESH_IDLE_AFTER_MS + 60_000;
    expect(tick(away + LIVE_REFRESH_MIN_INTERVAL_MS, away)).toBe(true);
  });
});
