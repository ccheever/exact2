// The pull request states the real-GitHub lane cannot produce (20261007-real-github-lane): the
// viewer profiles its two accounts cannot hold on a personal repository (read, triage, a
// read-only author), and the host's failures and delays, which real GitHub does not inject on
// request. Each profile's permissions are what the pinned server sends: a transcription of T3
// Code 1e2ecbd975 `gitHubViewerPermissions` (apps/server/src/pullRequest/GitHubPullRequestProvider.ts:71-102)
// over `toCanWrite`/`toCanTriage` (gitHubPullRequestJson.ts:2418-2432) and GitHub's
// `viewerCanUpdate`/`viewerDidAuthor`. The assertions are on the clone's own derivations.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import { toasts } from './toast';
import { emptyDetail, presentDetail, prCandidates, prCommand, pullRequestDetail } from './pages-pr-detail';
import { rowAction } from './r6-pr-logic';
import { readDetail, forgetDetails } from './r6-pr-actions';

const VERDICTS = ['comment', 'approve', 'request-changes'];
/** GitHubPullRequestProvider.ts CAPABILITIES (the host side, the same for every viewer). */
const CAPABILITIES = { diff: true, comment: true, actions: ['merge', 'ready', 'draft', 'close', 'reopen', 'update-branch', 'enable-auto-merge', 'disable-auto-merge', 'revert', 'approve-workflows'],
  mergeMethods: ['merge', 'squash', 'rebase'], updateMethods: ['merge', 'rebase'], reviewers: { request: true, listCandidates: true }, edit: { changeRequest: true, comment: true }, labels: true,
  review: { inlineComment: true, reply: true, resolve: true, verdicts: VERDICTS } };
type Access = { permission: 'ADMIN' | 'MAINTAIN' | 'WRITE' | 'TRIAGE' | 'READ'; canUpdate: boolean; didAuthor: boolean; canUpdateBranch?: boolean };
/** gitHubViewerPermissions over the viewer query's answer. */
function serverPermissions({ permission, canUpdate, didAuthor, canUpdateBranch = false }: Access) {
  const canWrite = ['ADMIN', 'MAINTAIN', 'WRITE'].includes(permission), canTriage = permission === 'TRIAGE' || canWrite;
  return {
    ...(canWrite ? { stackRebase: true } : {}),
    actions: [...(canWrite ? ['merge', 'enable-auto-merge', 'disable-auto-merge', 'revert', 'approve-workflows'] : []), ...(canUpdate ? ['ready', 'draft', 'close', 'reopen'] : []), ...(canUpdateBranch ? ['update-branch'] : [])],
    comment: true, resolve: canWrite || didAuthor, verdicts: didAuthor ? ['comment'] : VERDICTS, requestReviewers: canWrite,
    ...(canUpdateBranch ? { updateMethods: ['merge', 'rebase'] } : {}), labels: canTriage,
  };
}
/** The profiles the lane's two accounts cannot hold (personal repositories have no read or triage collaborators). */
const PROFILES: Record<string, Access> = {
  reader: { permission: 'READ', canUpdate: false, didAuthor: false },
  triage: { permission: 'TRIAGE', canUpdate: false, didAuthor: false },
  'contributor-author': { permission: 'READ', canUpdate: true, didAuthor: true },
  maintainer: { permission: 'MAINTAIN', canUpdate: true, didAuthor: false, canUpdateBranch: true },
};
const detail = (access: Access, over: Record<string, unknown> = {}) => ({
  provider: 'github', projectId: 'p1', repository: 'lane/sandbox', number: 7, title: 'Add input validation', body: '', url: 'https://github.com/lane/sandbox/pull/7',
  state: 'open', isDraft: false, mergeability: 'mergeable', baseComparison: 'behind', behindBy: 2, changedFiles: 1, additions: 11, deletions: 0, headBranch: 'seed/second-review', baseBranch: 'main',
  author: { login: 'lane-second' }, checks: [{ name: 'lane/ci', status: 'success' }], labels: [], mergeCapabilities: { merge: true, squash: true, rebase: true },
  capabilities: CAPABILITIES, viewerPermissions: serverPermissions(access), ...over,
});
const controls = (view: ReturnType<typeof emptyDetail>) => ({ close: view.canClose, reopen: view.canReopen, draft: view.canDraft, ready: view.canReady, merge: view.canMerge, updateBranch: view.canUpdateBranch, reviewers: view.canReview, labels: view.canLabel });
const present = (access: Access, over: Record<string, unknown> = {}) => controls(presentDetail(emptyDetail(), detail(access, over), null, 0));

describe('viewer profiles the sandbox cannot hold (pull request detail and row)', () => {
  test('a reader may comment and review, and is offered no action, label or reviewer request', () => {
    expect(serverPermissions(PROFILES.reader!)).toMatchObject({ actions: [], verdicts: VERDICTS, resolve: false, labels: false, requestReviewers: false });
    expect(present(PROFILES.reader!)).toEqual({ close: false, reopen: false, draft: false, ready: false, merge: false, updateBranch: false, reviewers: false, labels: false });
    expect(present(PROFILES.reader!, { state: 'closed' }).reopen).toBe(false);
    expect(rowAction(detail(PROFILES.reader!, { isDraft: true }))).toBe('');
    expect(rowAction(detail(PROFILES.reader!))).toBe('');
  });
  test('triage labels and nothing else', () => {
    expect(present(PROFILES.triage!)).toEqual({ close: false, reopen: false, draft: false, ready: false, merge: false, updateBranch: false, reviewers: false, labels: true });
    expect(rowAction(detail(PROFILES.triage!))).toBe('');
  });
  test('a read-only author (a fork contributor) closes, reopens, drafts and readies their own pull request, and only comments as a reviewer', () => {
    expect(serverPermissions(PROFILES['contributor-author']!)).toMatchObject({ actions: ['ready', 'draft', 'close', 'reopen'], verdicts: ['comment'], resolve: true, labels: false });
    expect(present(PROFILES['contributor-author']!)).toEqual({ close: true, reopen: false, draft: true, ready: false, merge: false, updateBranch: false, reviewers: false, labels: false });
    expect(present(PROFILES['contributor-author']!, { isDraft: true })).toMatchObject({ ready: true, draft: false, merge: false });
    expect(present(PROFILES['contributor-author']!, { state: 'closed' }).reopen).toBe(true);
    expect(rowAction(detail(PROFILES['contributor-author']!, { isDraft: true }))).toBe('ready');
    // Clean and green, but this viewer may not merge: the row offers nothing.
    expect(rowAction(detail(PROFILES['contributor-author']!))).toBe('');
  });
  test('a maintainer gets every action, the branch update included when GitHub says it may', () => {
    expect(present(PROFILES.maintainer!)).toEqual({ close: true, reopen: false, draft: true, ready: false, merge: true, updateBranch: true, reviewers: true, labels: true });
    expect(rowAction(detail(PROFILES.maintainer!))).toBe('merge');
    expect(present(PROFILES.maintainer!, { mergeability: 'conflicting' }).merge).toBe(false);
  });
});

type Reply = (payload: Record<string, unknown>) => unknown;
function fakeClient(replies: Record<string, Reply>) {
  const calls: string[] = [];
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, revision: 0, local: {},
    config: { environment: { capabilities: { pullRequests: true, pullRequestChecks: true } } },
    shell: { projects: [{ id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox', locator: { remoteUrl: 'https://github.com/lane/sandbox.git' } } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Record<string, unknown>) => { calls.push(method); const reply = replies[method]; if (!reply) throw new Error(`no reply for ${method}`); return reply(payload); },
  } as unknown as T3Client;
  return { client, calls };
}
const native = { available: true } as unknown as Native;
const selected = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 });
const operationError = (reason: string, message: string) => new ClientError(message, 'PullRequestOperationError', false, { reason });

describe('host failures and delays (injected; real GitHub does not fail or stall on request)', () => {
  test('a failed detail read shows the reference\'s unavailable state with the host\'s words, and a GitHub link', async () => {
    const { client } = fakeClient({ 'pullRequests.detail': () => { throw operationError('failed', 'GitHub CLI command failed: HTTP 502 Bad Gateway'); } });
    const view = await pullRequestDetail(client, native, { selected, refresh: 0, now: 0 });
    expect(view).toMatchObject({ open: true, number: 7, errorTitle: 'Could not load pull requests', error: 'GitHub CLI command failed: HTTP 502 Bad Gateway', githubUrl: 'https://github.com/lane/sandbox/pull/7' });
  });
  test('a pull request GitHub does not find reads as not found, not as a failure', async () => {
    const { client } = fakeClient({ 'pullRequests.detail': () => { throw operationError('not-found', 'Pull request lane/sandbox#7 was not found.'); } });
    const view = await pullRequestDetail(client, native, { selected, refresh: 1, now: 0 });
    expect([view.errorTitle, view.error]).toEqual(['Pull request #7 not found', "It may be an issue rather than a pull request, or this account can't see it."]);
  });
  test('a failed activity read leaves the detail standing (the conversation is optional)', async () => {
    const { client } = fakeClient({ 'pullRequests.detail': () => detail(PROFILES.maintainer!), 'pullRequests.activity': () => { throw operationError('rate-limited', 'GitHub rate limit'); } });
    const view = await pullRequestDetail(client, native, { selected, refresh: 2, now: 0 });
    expect([view.error, view.number, view.commentsLabel]).toEqual(['', 7, 'Comments (0)']);
  });
  test('a refused write toasts the reference\'s title with GitHub\'s reason and returns it to the form', async () => {
    const { client } = fakeClient({ 'pullRequests.runAction': () => { throw new Error('Pull request is not mergeable: the base branch policy prohibits the merge'); } });
    const message = await prCommand(client, native, 'action', selected, 'merge');
    expect(message).toBe('Pull request is not mergeable: the base branch policy prohibits the merge');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not merge this pull request' });
    const failedComment = fakeClient({ 'pullRequests.comment': () => { throw new Error('HTTP 403: Resource not accessible by integration'); } });
    expect(await prCommand(failedComment.client, native, 'comment', selected, 'hello')).toBe('HTTP 403: Resource not accessible by integration');
    expect(toasts(failedComment.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not update this pull request' });
  });
  test('a failed candidate read says so in the menu instead of an empty list', async () => {
    const { client } = fakeClient({ 'pullRequests.reviewerCandidates': () => { throw new Error('GitHub CLI is not signed in'); } });
    expect(await prCandidates(client, native, selected, 'reviewers')).toMatchObject({ reviewers: [], error: 'GitHub CLI is not signed in' });
  });
  test('a slow host: the panel shows its loading state until the answer lands, then the pull request', async () => {
    let release: () => void = () => {};
    const { client, calls } = fakeClient({ 'pullRequests.detail': () => new Promise((resolve) => { release = () => resolve(detail(PROFILES.maintainer!)); }), 'pullRequests.activity': () => null });
    const answer = pullRequestDetail(client, native, { selected, refresh: 3, now: 0 });
    // Until the first answer arrives the resource holds its shape's default, which the panel draws as loading
    // (pages-pr-detail.contract: `when detail.number == 0 and detail.error == ""` → pull-request-loading).
    const before = emptyDetail();
    expect([before.number, before.error]).toEqual([0, '']);
    await Bun.sleep(20);
    expect(calls).toEqual(['pullRequests.detail']);
    release();
    const view = await answer;
    expect([view.number, view.error, view.title, view.canMerge]).toEqual([7, '', 'Add input validation', true]);
  });
  test('the row: a failed read is retried after 30 s, not on every frame; a slow read lands when the host answers', async () => {
    const ref = { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 };
    let fail = true, pending: ((value: unknown) => void) | null = null;
    const { client, calls } = fakeClient({
      'pullRequests.detail': () => (fail ? Promise.reject(new Error('timeout')) : new Promise((resolve) => { pending = resolve; })),
      'pullRequests.checks': () => ({ checks: [] }),
    });
    expect(await readDetail(client, native, ref, 1_000)).toBe(null);
    expect(await readDetail(client, native, ref, 20_000)).toBe(null);
    expect(calls.filter((c) => c === 'pullRequests.detail').length).toBe(1);
    fail = false;
    const slow = readDetail(client, native, ref, 32_000);
    await Bun.sleep(10);
    expect(calls.filter((c) => c === 'pullRequests.detail').length).toBe(2);
    pending!(detail(PROFILES.maintainer!));
    expect((await slow)?.number).toBe(7);
    forgetDetails(client);
  });
});
