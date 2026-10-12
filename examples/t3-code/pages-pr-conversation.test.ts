// The pull request panel's conversation, loading states and live refresh (pr-conversation-and-
// refresh): the Summary and Timeline models (pages-pr-summary.ts, pages-pr-timeline.ts) and the
// panel's reads (pages-pr-detail.ts, pages-pr-refresh.ts) against injected host replies — the
// failures, delays and announcements real GitHub cannot be made to produce on request. The live
// rows ran against the real-GitHub lane (the task record).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { prCommand, pullRequestDetail, type DetailInput } from './pages-pr-detail';
import { presentSummary } from './pages-pr-summary';
import { presentTimeline } from './pages-pr-timeline';
import { PR_REFRESH_KEY, prRefreshEvent, pullRequestRefreshStats } from './pages-pr-refresh';

const NOW = Date.parse('2026-10-08T12:00:00Z');
const at = (minutesAgo: number) => new Date(NOW - minutesAgo * 60_000).toISOString();
const person = (login: string, extra: Obj = {}) => ({ login, name: null, avatarUrl: null, ...extra });
const detail = (over: Obj = {}): Obj => ({
  provider: 'github', projectId: 'p1', repository: 'acme/playground', number: 115, title: 'Add input validation', body: 'Adds a small validator.', url: 'https://github.com/acme/playground/pull/115',
  author: person('second'), state: 'open', isDraft: false, mergeability: 'mergeable', changedFiles: 1, additions: 11, deletions: 0, headBranch: 'feature/input-validation', baseBranch: 'main',
  createdAt: at(600), updatedAt: at(60), mergedAt: null, closedAt: null, reviewers: [], labels: [{ name: 'needs-review', color: 'fbca04' }], checks: [{ name: 'ci/build', status: 'success', url: 'https://ci.example/1' }],
  capabilities: { labels: true }, viewerPermissions: {}, ...over,
});
const remark = (id: string, minutesAgo: number, over: Obj = {}): Obj => ({ id, kind: 'issue-comment', author: person('primary'), body: `Remark ${id}`, createdAt: at(minutesAgo), url: `https://github.com/acme/playground/pull/115#${id}`, path: null, reviewState: null, ...over });
/** The seeded second-review conversation's shape: plain remarks, a dismissed and a standing review, line threads (one resolved, one outdated), bots. */
function conversation(): Obj {
  const comments: Obj[] = [];
  for (let index = 0; index < 14; index++) comments.push(remark(`c${index}`, 300 - index * 10));
  comments.push(remark('dismissed', 290, { kind: 'review', reviewState: 'DISMISSED', body: 'A few things before this lands.' }));
  comments.push(remark('resolved-line', 280, { kind: 'review-comment', path: 'src/validate.js', body: 'This pattern also matches tabs. Is that intended?' }));
  comments.push(remark('outdated-line', 275, { kind: 'review-comment', path: 'src/validate.js', body: 'Return a sorted list so the output is stable.' }));
  comments.push(remark('changes', 100, { kind: 'review', reviewState: 'CHANGES_REQUESTED', body: 'Still needs the empty-input case.' }));
  for (let index = 0; index < 4; index++) comments.push(remark(`bot${index}`, 200 - index, { author: person('ci-helper[bot]', { isBot: true }), body: `<!-- bot -->\nReport ${index}` }));
  comments.sort((a, b) => String(a.createdAt).localeCompare(String(b.createdAt)));
  return {
    author: person('second'), reviewers: [person('primary')], commentCount: comments.length, commentsTruncated: false, comments,
    reviewThreads: [
      { id: 't1', path: 'src/validate.js', line: 6, side: 'right', isResolved: true, isOutdated: false, comments: [{ id: 'resolved-line' }] },
      { id: 't2', path: 'src/validate.js', line: 8, side: 'right', isResolved: false, isOutdated: true, comments: [{ id: 'outdated-line' }] },
    ],
    commits: [{ oid: 'aaaaaaa1', messageHeadline: 'Add the validator', committedDate: at(500) }, { oid: 'bbbbbbb2', messageHeadline: 'Sort the reported problems', committedDate: at(150) }],
  };
}

describe('the Summary tab (PullRequestSummaryTab)', () => {
  const view = presentSummary({ detail: detail(), activity: conversation(), activityPending: false, activityError: '', now: NOW, listEntry: null });
  test('splits the conversation: finished work, bots, and the rest windowed from the newest', () => {
    expect([view.activeCount, view.botCount, view.finishedCount]).toEqual([16, 4, 2]);
    expect(view.newest.filter(card => card.fromEnd < 10).map(card => card.id)).toEqual(['changes', 'c13', 'c12', 'c11', 'c10', 'c9', 'c8', 'c7', 'c6', 'c5']);
    // "Show 6 older comments (6 hidden)" is the window's (pages-pr-summary.contract PrdShowOlder over activeCount - shown).
    expect(view.oldest[0]?.fromEnd).toBe(15);
    expect([view.bots.label, view.finished.label]).toEqual(['4 bot comments', '2 resolved or dismissed comments']);
    expect(view.finishedNewest.map(card => [card.id, card.finishedLabel])).toEqual([['resolved-line', 'Resolved'], ['dismissed', 'Review dismissed']]);
  });
  test('a line remark names its place, and says when the line has moved on', () => {
    const outdated = view.newest.find(card => card.id === 'outdated-line')!;
    expect([outdated.location, outdated.outdated]).toEqual(['src/validate.js:8', true]);
    expect(view.finishedNewest.find(card => card.id === 'resolved-line')?.location).toBe('src/validate.js:6');
  });
  test('a bot group says how many wrote, across how many files, and when last', () => {
    expect([view.bots.authorsText, view.bots.filesText, view.bots.latest, view.bots.faces.map(face => face.login)]).toEqual(['1 author', '', '3h ago', ['ci-helper[bot]']]);
    // A marker-only bot body still renders what follows the marker; a review's verdict reads as its badge.
    expect(view.newest.find(card => card.id === 'changes')).toMatchObject({ outcome: 'changes-requested', outcomeLabel: 'Changes requested', bodyId: 'pr-comment:changes' });
  });
  test('reviewers wear their last verdict; a verdict older than the newest commit is stale', () => {
    const fresh = presentSummary({ detail: detail(), activity: conversation(), activityPending: false, activityError: '', now: NOW, listEntry: null });
    expect(fresh.reviewers.map(entry => [entry.login, entry.outcome, entry.stale, entry.tooltip])).toEqual([['primary', 'changes-requested', false, 'primary — Changes requested']]);
    const later = conversation();
    (later.commits as Obj[]).push({ oid: 'ccccccc3', messageHeadline: 'Handle empty input', committedDate: at(30) });
    const stale = presentSummary({ detail: detail(), activity: later, activityPending: false, activityError: '', now: NOW, listEntry: null });
    expect(stale.reviewers[0]).toMatchObject({ outcome: 'changes-requested', stale: true, outcomeText: 'Changes requested earlier changes' });
  });
  test('a truncated conversation says so with the reference words and the count read', () => {
    const cut = presentSummary({ detail: detail(), activity: { ...conversation(), commentsTruncated: true, reviewThreadsTruncated: true }, activityPending: false, activityError: '', now: NOW, listEntry: null });
    expect([cut.truncated, cut.truncatedText]).toEqual([true, 'This conversation is longer than this page reads in one go. The most recent 22 are here; open it on the host to read the rest.']);
  });
  test('checks a newer list rollup contradicts are out of date', () => {
    const listEntry = { updatedAt: at(1), checksState: 'failing' };
    const stale = presentSummary({ detail: detail(), activity: conversation(), activityPending: false, activityError: '', now: NOW, listEntry });
    expect([stale.checksStale, stale.checksState]).toEqual([true, 'failing']);
    const older = presentSummary({ detail: detail(), activity: conversation(), activityPending: false, activityError: '', now: NOW, listEntry: { updatedAt: at(600), checksState: 'failing' } });
    expect(older.checksStale).toBe(false);
  });
});

describe('the Timeline tab (PullRequestTimelineTab)', () => {
  test('folds consecutive remarks, keeps verdicts and commits as rows, and counts in the tab bar', () => {
    const view = presentTimeline({ detail: detail(), activity: conversation(), activityPending: false, activityError: '', now: NOW });
    expect(view.newest.map(row => row.kind)).toEqual(['verdict', 'commit', 'comments', 'verdict', 'comments', 'commit', 'opened']);
    expect(view.newest[0]).toMatchObject({ outcome: 'changes-requested', stale: false, outcomeLabel: 'Changes requested' });
    expect(view.newest[3]).toMatchObject({ kind: 'verdict', outcome: 'dismissed', stale: true, staleLabel: 'Review dismissed earlier changes' });
    expect(view.oldest.map(row => row.kind)).toEqual(['opened', 'commit', 'comments', 'verdict', 'comments', 'commit', 'verdict']);
    expect([view.comments, view.commentsAria, view.commits, view.commitsAria, view.approvals]).toEqual(['22', '22 comments', '2', '2 commits', 0]);
  });
  test('says "…" while the activity reads and "—" when it failed', () => {
    expect(presentTimeline({ detail: detail(), activity: null, activityPending: true, activityError: '', now: NOW }).comments).toBe('…');
    const failed = presentTimeline({ detail: detail(), activity: null, activityPending: false, activityError: 'GitHub rate limit', now: NOW });
    expect([failed.comments, failed.commentsAria, failed.commits, failed.commitsAria]).toEqual(['—', 'Comments unavailable', '—', 'Commits unavailable']);
  });
  test('counts approvals that stand, and none from a conversation read only in part', () => {
    const approved = { ...conversation(), comments: [remark('ok', 10, { kind: 'review', reviewState: 'APPROVED', author: person('second') })] };
    expect(presentTimeline({ detail: detail(), activity: approved, activityPending: false, activityError: '', now: NOW }).approvals).toBe(1);
    expect(presentTimeline({ detail: detail(), activity: { ...approved, commentsTruncated: true }, activityPending: false, activityError: '', now: NOW }).approvals).toBe(0);
  });
});

// ── The panel's reads ───────────────────────────────────────────────────────

type Reply = (payload: Obj) => unknown;
function fixture(replies: Record<string, Reply>) {
  const calls: string[] = [], native: string[] = [];
  const client = {
    environmentId: 'env', ready: true, generation: 1, revision: 0, local: {} as Obj,
    config: { environment: { capabilities: { pullRequests: true } } }, shell: { projects: [{ id: 'p1', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/playground' } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj) => { calls.push(method); const reply = replies[method]; if (!reply) throw new Error(`no reply for ${method}`); return reply(payload); },
    restAccess: () => ({ call: async (request: Obj) => { native.push(`${request.op}:${String(request.key)}`); return { id: `1-${native.length}` }; } }),
    savePreferences: async () => { native.push('save'); },
  } as unknown as T3Client;
  let wakes = 0;
  const waking = { available: true, watch: () => {}, later: async (request: Obj) => { if (request.op === 'r10Wake') wakes++; return { ok: true, generation: 1, value: { at: NOW } }; } } as unknown as Native;
  const ask = (input: Partial<DetailInput> = {}) => pullRequestDetail(client, waking, { selected, refresh: 0, now: NOW, ...input });
  /** Ask until the answer stops waking the resource, as the runner does (LLP 1016.002 D4). */
  const settle = async (input: Partial<DetailInput> = {}) => { let before = wakes, view = await ask(input); for (let i = 0; wakes > before && i < 8; i++) { before = wakes; view = await ask(input); } return view; };
  return { client, calls, native, ask, settle, wakes: () => wakes };
}
const selected = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'acme/playground', number: 115 });
const deferred = () => { let release: (value: unknown) => void = () => {}; const promise = new Promise(resolve => { release = resolve; }); return { promise, release }; };

describe('the panel reads (PullRequestDetailPanel queries)', () => {
  test('loading: the detail ghost, then the conversation and timeline ghosts, then the content, without a second layout', async () => {
    const slowActivity = deferred();
    const { calls, ask, wakes } = fixture({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => slowActivity.promise });
    const ghost = await ask();
    expect([ghost.phase, ghost.loading, calls.length, wakes()]).toEqual(['ghost', true, 0, 1]);
    expect(ghost.checkoutCommand).toBe('gh pr checkout 115'); // loadingPullRequestCheckoutCommand
    const withDetail = await ask();
    expect([withDetail.phase, withDetail.title, withDetail.activityPending, withDetail.summary.activityPending, withDetail.timeline.comments, wakes()]).toEqual(['content', 'Add input validation', true, true, '…', 2]);
    expect(calls).toEqual(['pullRequests.detail']);
    // The activity answers 2 s later (the injected delay); the conversation ghost stays meanwhile.
    const answer = ask();
    await Bun.sleep(20);
    expect(calls).toEqual(['pullRequests.detail', 'pullRequests.activity']);
    setTimeout(() => slowActivity.release(conversation()), 2000);
    const loaded = await answer;
    expect([loaded.phase, loaded.activityPending, loaded.summary.activeCount, loaded.timeline.comments, wakes()]).toEqual(['content', false, 16, '22', 2]);
  }, 10_000);

  test('a failed activity read says "Could not load pull request activity" with Retry, and Retry reads it once more', async () => {
    let fail = true;
    const { client, calls, settle, ask, wakes } = fixture({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => { if (fail) throw new ClientError('GitHub CLI command failed: HTTP 502', 'PullRequestOperationError'); return conversation(); } });
    const failed = await settle();
    expect([failed.activityError, failed.summary.activityError, failed.summary.empty, failed.timeline.comments]).toEqual(['GitHub CLI command failed: HTTP 502', 'GitHub CLI command failed: HTTP 502', true, '—']);
    const reads = calls.filter(method => method === 'pullRequests.activity').length;
    fail = false;
    expect(await prCommand(client, { available: true } as Native, 'activity-retry', selected, 'timeline')).toBe('');
    const before = wakes();
    const retrying = await ask();
    expect([retrying.activityError, retrying.activityPending, wakes()]).toEqual(['', true, before + 1]); // the ghost shows while it reads
    const recovered = await ask();
    expect([recovered.activityError, recovered.summary.activeCount]).toEqual(['', 16]);
    expect(calls.filter(method => method === 'pullRequests.activity').length).toBe(reads + 1);
    expect(calls.filter(method => method === 'pullRequests.detail').length).toBe(1); // the Timeline's Retry reads the activity alone
  });

  test('the server announces a change: the detail and the activity read again once, the last detail shown meanwhile', async () => {
    let title = 'Add input validation';
    const { client, calls, native, ask, settle } = fixture({ 'pullRequests.detail': () => detail({ title, updatedAt: at(1) }), 'pullRequests.activity': () => conversation() });
    await ask(); // the ghost; the stream is held from here
    expect(native.filter(op => op.startsWith('subscribe:'))).toEqual([`subscribe:${PR_REFRESH_KEY}`]);
    const subscription = '1-1';
    // The server's current epoch arrives while the first read is out: that read answers it.
    prRefreshEvent(client, { key: PR_REFRESH_KEY, subscriptionId: subscription, value: 5 });
    await settle();
    expect(calls).toEqual(['pullRequests.detail', 'pullRequests.activity']);
    title = 'Add input validation (renamed)';
    prRefreshEvent(client, { key: PR_REFRESH_KEY, subscriptionId: subscription, value: 5 }); // the same value again is no change
    await settle();
    expect(calls).toHaveLength(2);
    prRefreshEvent(client, { key: PR_REFRESH_KEY, subscriptionId: subscription, value: 6 }); // another client's pullRequests.invalidate
    const refreshed = await settle();
    expect(calls.slice(2)).toEqual(['pullRequests.detail', 'pullRequests.activity']);
    expect(refreshed.title).toBe('Add input validation (renamed)');
    expect(pullRequestRefreshStats(client)).toMatchObject({ subscribed: true, subscribes: 1, epoch: 2 });
    // Closing the panel lets the stream go.
    await pullRequestDetail(client, { available: true, watch: () => {}, later: async () => ({}) } as unknown as Native, { selected: '', refresh: 0, now: NOW });
    expect(native.filter(op => op.startsWith('unsubscribe:'))).toEqual([`unsubscribe:${PR_REFRESH_KEY}`]);
    expect(pullRequestRefreshStats(client)).toMatchObject({ subscribed: false, unsubscribes: 1 });
  });

  test('the detail reads again on the 5-minute interval and when the window comes back, not while the reader is away', async () => {
    const { calls, settle } = fixture({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => conversation() });
    await settle();
    const details = () => calls.filter(method => method === 'pullRequests.detail').length;
    await settle({ now: NOW + 60_000 });
    expect(details()).toBe(1);
    await settle({ now: NOW + 5 * 60_000 }); // the reporter's last interaction is NOW, within six minutes
    expect(details()).toBe(2);
    await settle({ now: NOW + 5 * 60_000 + 30_000, returns: 1 }); // focus back after 30 s: an arrival
    expect(details()).toBe(3);
    await settle({ now: NOW + 20 * 60_000 }); // nobody touched the window for 20 minutes
    expect(details()).toBe(3);
  });

  test('a focus read counts from the focus, not from the last minute tick (app.contract windowReturned)', async () => {
    // pr-list-live-refresh: the panel took `page.hasFocus` with the clock of the last minute tick, so a window
    // focused again within that minute saw no time pass and read nothing. The window's return now reads the clock.
    const { calls, settle } = fixture({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => conversation() });
    await settle();
    const details = () => calls.filter(method => method === 'pullRequests.detail').length;
    await settle({ now: NOW + 30_000, returns: 1 }); // focused again 30 s after the first read, the clock read then
    expect(details()).toBe(2);
    await settle({ now: NOW + 35_000, returns: 2 }); // and again 5 s later: not a read per tab stop
    expect(details()).toBe(2);
  });
  test('a relaunch shows the kept detail first (t3-code.json), then the live read replaces it', async () => {
    const first = fixture({ 'pullRequests.detail': () => detail({ title: 'Kept title' }), 'pullRequests.activity': () => conversation() });
    await first.settle();
    expect(Object.keys(first.client.local.prDetailSnapshots as Obj)).toHaveLength(1);
    const slowDetail = deferred();
    const relaunch = fixture({ 'pullRequests.detail': () => slowDetail.promise, 'pullRequests.activity': () => conversation() });
    (relaunch.client as unknown as { local: Obj }).local = first.client.local;
    const cached = await relaunch.ask();
    expect([cached.phase, cached.title, cached.activityPending, relaunch.calls.length]).toEqual(['content', 'Kept title', true, 0]);
    const answer = relaunch.ask();
    await Bun.sleep(10);
    slowDetail.release(detail({ title: 'Live title' }));
    expect((await answer).title).toBe('Live title');
  });
});
