// A pull request's checks as every surface shows them, the list's answers across a relaunch, and how long an armed
// confirmation lives (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3). The first half ports the original tests with their
// names: packages/client-runtime pullRequestChecks.test.ts and apps/web state/pullRequests.test.ts. Not ported: "falls
// back to a fresh read when the browser blocks storage" and "removes the composite snapshots it replaced" (the answers
// live in the preference record, not in the browser's storage) — n/a-ui. The second half drives the clone's PR page
// and list (pages-pr-detail.ts, pages-prs.ts) against injected host replies, and the controls' actions as Contract
// runs them.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { checksRefreshIntervalMs, resolvePullRequestDetailChecks, PULL_REQUEST_ACTIONS } from '../../src/details/pr-row-logic';
import { readDetail, forgetDetails } from '../../src/details/pr-row-actions';
import { observePullRequestChecks, observePullRequestSummary, sharedPullRequestSummary } from '../../src/pull-requests/pages-pr-shared';
import { adoptPrSnapshots, rememberListAnswer } from '../../src/pull-requests/pages-pr-refresh';
import { restoredPullRequestList } from '../../src/pull-requests/pages-pr-paging';
import { prUiLocal, pullRequestDetail } from '../../src/pull-requests/pages-pr-detail';
import { pullRequestsPage, rowRef, type PrInput } from '../../src/pull-requests/pages-prs';
import { actionState } from '../../src/pull-requests/pages-pr-actions';
import { filterPullRequestConversationItems, pullRequestConversationItems } from '../../src/pull-requests/pages-pr-logic';

// ── Ported (original names) ─────────────────────────────────────────────────

describe('resolvePullRequestDetailChecks', () => {
  const detail = (over: Obj = {}): Obj => ({ title: 'Cache the title', state: 'open', checks: [], ...over });
  const pending = (name: string): Obj => ({ name, status: 'pending', url: null, description: null });
  test('uses fresh checks for detail counts without replacing other PR fields', () => {
    const original = detail({ checks: [pending('build'), pending('test')] });
    const checks = [pending('build'), { ...pending('test'), status: 'success' }];
    expect(resolvePullRequestDetailChecks(original, { state: 'closed', checks }, true)).toEqual({ ...original, state: 'closed', checks });
    expect(resolvePullRequestDetailChecks(original, { state: 'closed', checks }, false)).toBe(original);
    expect(resolvePullRequestDetailChecks(original, null, true)).toBe(original);
    expect(resolvePullRequestDetailChecks(null, { state: 'closed', checks }, true)).toBeNull();
  });
  test('keeps newer checks when an older cached detail arrives later', () => {
    const cached = detail({ observedAt: 1_000, checks: [pending('build')] });
    const checks = { state: 'open', checks: [{ ...pending('build'), status: 'success' }], observedAt: 2_000 };
    const resolved = resolvePullRequestDetailChecks(cached, checks, false);
    expect(resolved?.checks).toEqual(checks.checks);
    expect(resolved?.observedAt).toBe(1_000);
    expect(resolvePullRequestDetailChecks(cached, { state: checks.state, checks: checks.checks }, true)?.checks).toEqual(checks.checks);
    const refreshed = detail({ observedAt: 3_000, checks: [pending('build')] });
    expect(resolvePullRequestDetailChecks(refreshed, checks, true)).toBe(refreshed);
  });
  test('keeps merged details settled while applying newer checks', () => {
    const merged = detail({ state: 'merged', observedAt: 1_000 });
    const checks = [{ name: 'build', status: 'success', url: null, description: null }];
    expect(resolvePullRequestDetailChecks(merged, { state: 'open', observedAt: 2_000, checks }, true)).toEqual({ ...merged, checks });
  });
});

const summary = (over: Obj = {}): Obj => ({
  provider: 'github', projectId: 'pull-request-cache-test', repository: 'acme/widget', number: 7, title: 'Improve widget', url: 'https://github.com/acme/widget/pull/7',
  state: 'open', isDraft: false, headBranch: 'improve-widget', baseBranch: 'main', updatedAt: '2026-09-10T00:00:00Z', author: { login: 'oliver', name: null, avatarUrl: null },
  mergeability: 'mergeable', checksState: 'passing', ...over,
});
/** A `pullRequests.checks` answer whose checks roll up to `state`. */
const checksAnswer = (state: 'passing' | 'failing' | 'pending', observedAt?: number): Obj =>
  ({ state: 'open', checks: [{ name: 'build', status: state === 'passing' ? 'success' : state === 'failing' ? 'failure' : 'pending' }], ...(observedAt === undefined ? {} : { observedAt }) });

describe('pull request observations', () => {
  const reference = { projectId: 'pull-request-cache-test', host: 'github.com', repository: 'acme/widget', number: 7 };
  const owner = () => ({ environmentId: 'env', shell: { projects: [], threads: [] } }) as unknown as T3Client;
  const shown = (client: T3Client) => sharedPullRequestSummary(client, reference, null);
  test('keeps a newer list\'s mergeability when a checks read lands after it', () => {
    const client = owner();
    observePullRequestSummary(client, 'env', reference, summary({ mergeability: 'conflicting', checksState: 'pending', observedAt: 100 }));
    observePullRequestSummary(client, 'env', reference, summary({ mergeability: 'mergeable', checksState: 'pending', observedAt: 200 }));
    observePullRequestChecks(client, 'env', reference, checksAnswer('passing', 300));
    expect(shown(client)).toMatchObject({ mergeability: 'mergeable', checksState: 'passing' });
  });
  test('keeps a newer checks read when an older list read lands after it', () => {
    const client = owner();
    observePullRequestSummary(client, 'env', reference, summary({ mergeability: 'conflicting', checksState: 'pending', observedAt: 100 }));
    observePullRequestChecks(client, 'env', reference, checksAnswer('passing', 300));
    observePullRequestSummary(client, 'env', reference, summary({ mergeability: 'mergeable', checksState: 'pending', observedAt: 250 }));
    expect(shown(client)).toMatchObject({ mergeability: 'mergeable', checksState: 'passing' });
  });
  test('lets a newer read\'s checks replace an older checks read', () => {
    const client = owner();
    observePullRequestSummary(client, 'env', reference, summary({ checksState: 'pending', observedAt: 100 }));
    observePullRequestChecks(client, 'env', reference, checksAnswer('passing', 300));
    observePullRequestSummary(client, 'env', reference, summary({ checksState: 'failing', observedAt: 400 }));
    expect(shown(client)?.checksState).toBe('failing');
  });
  // The clone stamps arrival as the order answers land, so the later of two unstamped reads is the newer.
  test('orders checks by arrival when the server reports no read time', () => {
    const client = owner();
    observePullRequestSummary(client, 'env', reference, summary({ checksState: 'pending' }));
    observePullRequestChecks(client, 'env', reference, checksAnswer('passing'));
    expect(shown(client)?.checksState).toBe('passing');
    observePullRequestChecks(client, 'env', reference, checksAnswer('failing'));
    expect(shown(client)?.checksState).toBe('failing');
    // A stamped read and an unstamped one have no common clock: arrival orders them too.
    observePullRequestSummary(client, 'env', reference, summary({ checksState: 'pending', observedAt: 5 }));
    expect(shown(client)?.checksState).toBe('pending');
  });
  test('shows nothing until some read has described the pull request', () => {
    const client = owner();
    observePullRequestChecks(client, 'env', reference, checksAnswer('failing', 1));
    expect(shown(client)).toBeNull();
  });
});

describe('pull request list answers across a reload', () => {
  const listEntry = (number: number, updatedAt = '2026-07-02T00:00:00Z'): Obj => ({
    provider: 'github', host: 'github.com', projectId: 'project-1', projectTitle: 't3code', repository: 'pingdotgg/t3code', number, title: `Pull request ${number}`,
    url: `https://github.com/pingdotgg/t3code/pull/${number}`, author: { login: 'octocat', name: null, avatarUrl: null }, headBranch: `feat/branch-${number}`, baseBranch: 'main',
    state: 'open', isDraft: false, mergeability: 'mergeable', additions: 1, deletions: 0, createdAt: '2026-07-01T00:00:00Z', updatedAt, viewerReviewRequested: false, labels: [],
  });
  const answer = (entries: Obj[], host = 'github.com'): Obj => ({
    entries, viewers: { [host]: 'Bilal' }, providers: [{ host, kind: 'github', searchesOnHost: true, projectCount: 1, configured: true, detail: null }],
    errors: [{ projectId: 'project-1', projectTitle: 't3code', message: 'boom' }], truncated: true, nextCursors: { 'pingdotgg/t3code': 'cursor-1' },
  });
  const local = 'env-local', remote = 'env-remote';
  const input: Obj = { state: 'open', involvement: 'all', limit: 99 };
  /** The preference record's owner, focused on the local server (a background server's rows carry its id). */
  const store = () => ({ environmentId: local, local: {} }) as unknown as T3Client;
  const both = (scope: Obj = input) => [{ environmentId: local, input: scope }, { environmentId: remote, input: scope }];
  test('restores every environment\'s first page as one merged list without stale failures or cursors', () => {
    const owner = store();
    rememberListAnswer(owner, { environmentId: local, input }, answer([listEntry(1, '2026-07-01T00:00:00Z')]));
    rememberListAnswer(owner, { environmentId: remote, input }, answer([listEntry(2, '2026-07-03T00:00:00Z')], 'gitlab.com'));
    const restored = restoredPullRequestList(owner, both());
    expect(restored?.entries.map(row => [row.environmentId ?? local, row.number])).toEqual([[remote, 2], [local, 1]]);
    expect(restored?.viewers).toMatchObject({ 'env-local github.com': 'Bilal', 'env-remote gitlab.com': 'Bilal' });
    expect(restored?.providers.map(provider => provider.host)).toEqual(['github.com', 'gitlab.com']);
    expect(restored?.errors).toEqual([]);
    expect(restored?.nextCursors).toEqual({});
  });
  test('keeps a page of rows and only first-page reads', () => {
    const owner = store();
    const rows = Array.from({ length: 120 }, (_, index) => listEntry(index + 1));
    rememberListAnswer(owner, { environmentId: local, input }, answer(rows));
    const searched = { ...input, query: 'fix' };
    rememberListAnswer(owner, { environmentId: local, input: searched }, answer(rows));
    const continued = { ...input, cursors: { 'pingdotgg/t3code': 'cursor-1' } };
    rememberListAnswer(owner, { environmentId: local, input: continued }, answer(rows));
    expect(restoredPullRequestList(owner, [{ environmentId: local, input }])?.entries).toHaveLength(99);
    expect(restoredPullRequestList(owner, [{ environmentId: local, input: searched }])).toBeNull();
    expect(restoredPullRequestList(owner, [{ environmentId: local, input: continued }])).toBeNull();
  });
  test('restores answers kept under a project assignment for a cold start that has none yet', () => {
    const owner = store();
    rememberListAnswer(owner, { environmentId: local, input: { ...input, projectIds: ['project-1'] } }, answer([listEntry(1)]));
    rememberListAnswer(owner, { environmentId: remote, input: { ...input, projectIds: ['project-2'] } }, answer([listEntry(2, '2026-07-03T00:00:00Z')]));
    expect(restoredPullRequestList(owner, both())?.entries.map(row => [row.environmentId ?? local, row.number])).toEqual([[remote, 2], [local, 1]]);
  });
  test('shows a pull request once when answers from before a reassignment overlap', () => {
    const owner = store();
    rememberListAnswer(owner, { environmentId: local, input }, answer([listEntry(1, '2026-07-01T00:00:00Z')]));
    rememberListAnswer(owner, { environmentId: remote, input }, answer([listEntry(1, '2026-07-03T00:00:00Z')]));
    expect(restoredPullRequestList(owner, both())?.entries.map(row => [row.environmentId ?? local, row.updatedAt])).toEqual([[remote, '2026-07-03T00:00:00Z']]);
  });
  test('keeps a shared repository\'s rows when it moves to an environment that has not answered yet', () => {
    const owner = store();
    const own = 'project-own', shared = { ...listEntry(2), projectId: 'project-shared' };
    rememberListAnswer(owner, { environmentId: local, input }, answer([{ ...listEntry(1), projectId: own }, shared]));
    rememberListAnswer(owner, { environmentId: remote, input }, answer([listEntry(3, '2026-07-03T00:00:00Z')]));
    // The shared repository now belongs to the remote environment, which has not answered yet.
    rememberListAnswer(owner, { environmentId: local, input: { ...input, projectIds: [own] } }, answer([{ ...listEntry(1), projectId: own }]));
    expect(restoredPullRequestList(owner, both())?.entries.map(row => row.number).sort()).toEqual([1, 2, 3]);
  });
  test('keeps one page per environment when retained rows join a partial answer', () => {
    const owner = store();
    const rows = (projectId: string, first: number) => Array.from({ length: 99 }, (_, index) => ({ ...listEntry(first + index), projectId }));
    rememberListAnswer(owner, { environmentId: local, input }, answer(rows('project-shared', 1)));
    rememberListAnswer(owner, { environmentId: local, input: { ...input, projectIds: ['project-own'] } }, answer(rows('project-own', 101)));
    const restored = restoredPullRequestList(owner, [{ environmentId: local, input }]);
    expect(restored?.entries).toHaveLength(99);
    expect(restored?.entries.every(row => row.projectId === 'project-own')).toBe(true);
  });
  test('does not keep grown pages', () => {
    const owner = store(), grown = { ...input, limit: 198 };
    rememberListAnswer(owner, { environmentId: local, input: grown }, answer([]));
    expect(restoredPullRequestList(owner, [{ environmentId: local, input: grown }])).toBeNull();
  });
  test('forgets whole scopes, never one environment of a scope that is kept', () => {
    const owner = store();
    const scoped = (projectIndex: number) => ({ ...input, projectId: `p-${projectIndex}` });
    for (let projectIndex = 0; projectIndex < 9; projectIndex++) {
      for (const [offset, environmentId] of [local, remote].entries()) {
        rememberListAnswer(owner, { environmentId, input: scoped(projectIndex) }, answer([listEntry(projectIndex * 2 + offset + 1, `2026-07-0${2 - offset}T00:00:00Z`)]));
      }
    }
    expect(restoredPullRequestList(owner, both(scoped(0)))).toBeNull();
    for (let projectIndex = 1; projectIndex < 9; projectIndex++) {
      expect(restoredPullRequestList(owner, both(scoped(projectIndex)))?.entries.map(row => row.environmentId ?? local)).toEqual([local, remote]);
    }
  });
});

// ── The clone's PR page, list and controls against injected host replies ──

const CAPABILITIES = { diff: true, comment: true, actions: [...PULL_REQUEST_ACTIONS], mergeMethods: ['merge', 'squash'], updateMethods: ['merge'], labels: true,
  review: { inlineComment: true, reply: true, resolve: true, verdicts: ['comment', 'approve'] }, edit: { changeRequest: true, comment: true }, reviewers: { request: false } };
const detail = (over: Obj = {}): Obj => ({
  provider: 'github', projectId: 'p1', repository: 'lane/sandbox', number: 7, title: 'Add a changelog', body: '', url: 'https://github.com/lane/sandbox/pull/7', workspaceRoot: '/repos/sandbox',
  state: 'open', isDraft: false, mergeability: 'mergeable', baseComparison: 'up-to-date', changedFiles: 1, additions: 3, deletions: 0, headBranch: 'feature/changelog', baseBranch: 'main',
  author: { login: 'lane' }, checks: [{ name: 'ci/build', status: 'success' }], labels: [], mergeCapabilities: { merge: true, squash: true }, autoMergeEnabled: false, capabilities: CAPABILITIES,
  viewerPermissions: { actions: [...PULL_REQUEST_ACTIONS], comment: true, verdicts: ['comment', 'approve'], updateMethods: ['merge'], labels: true },
  createdAt: '2026-10-01T00:00:00Z', updatedAt: '2026-10-08T10:00:00Z', ...over,
});
type Reply = (payload: Obj) => unknown;
function fakeClient(replies: Record<string, Reply>, capabilities: Obj = { pullRequests: true, threadPullRequests: true, pullRequestChecks: true }) {
  const calls: { method: string; payload: Obj }[] = [];
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, revision: 0, generation: 1, local: {},
    config: { environment: { capabilities }, settings: {} },
    shell: { projects: [{ id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox' } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj) => {
      calls.push({ method, payload });
      const reply = replies[method];
      if (!reply) throw new Error(`no reply for ${method}`);
      return reply(payload);
    },
  } as unknown as T3Client;
  client.projectGroups = () => client.shell.projects.map(project => ({ key: String(project.id), name: String(project.title ?? ''), members: [project] }));
  return { client, calls, count: (method: string) => calls.filter(call => call.method === method).length };
}
/** The host's replies; its checks, unless a test says otherwise, are the detail's own as it stands now. */
function defaults(over: Record<string, Reply> = {}): Record<string, Reply> {
  const replies: Record<string, Reply> = {
    'pullRequests.detail': () => detail(), 'pullRequests.activity': () => ({ comments: [], commits: [], reviewers: [], commentCount: 0 }), 'pullRequests.stack': () => null,
    'vcs.listRefs': () => ({ refs: [{ name: 'origin/main', isDefault: true, isRemote: true, remoteName: 'origin' }] }), ...over,
  };
  replies['pullRequests.checks'] ??= payload => { const current = replies['pullRequests.detail']!(payload) as Obj; return { state: current.state, checks: current.checks }; };
  return replies;
}
let wakes = 0;
const native = { available: true, watch: () => {}, later: async () => { wakes++; return { ok: true }; } } as unknown as Native;
const selected = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 });
/** The panel's resource as the runner asks it: again while an answer woke it. */
async function panel(client: T3Client, input: { selected?: string; refresh?: number; now?: number; tab?: string } = {}) {
  const ask = { selected: input.selected ?? selected, refresh: input.refresh ?? 1, now: input.now ?? 0, ...(input.tab ? { tab: input.tab } : {}) };
  let seen = wakes, view = await pullRequestDetail(client, native, ask);
  for (let asked = 0; wakes > seen && asked < 8; asked++) { seen = wakes; view = await pullRequestDetail(client, native, ask); }
  return view;
}

describe('the PR page reads its checks as the details row does (usePullRequestDetail, usePullRequestChecksRefresh)', () => {
  test('the detail around the server\'s cache, then the checks alone: every 45 s while runs are pending, 60 s once they settle', async () => {
    let checks = [{ name: 'ci/build', status: 'pending' }];
    const { client, calls, count } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ checks }), 'pullRequests.checks': () => ({ state: 'open', checks }) }));
    await panel(client, { now: 1_000 });
    expect(calls.find(call => call.method === 'pullRequests.detail')?.payload).toMatchObject({ number: 7, allowStale: false });
    expect(count('pullRequests.checks')).toBe(1);
    await panel(client, { now: 40_000 });
    expect(count('pullRequests.checks')).toBe(1);
    checks = [{ name: 'ci/build', status: 'success' }];
    const settled = await panel(client, { now: 46_000 });
    expect(count('pullRequests.checks')).toBe(2);
    expect([settled.checksSummary, settled.checksTone]).toEqual(['All checks passed', 'passing']);
    await panel(client, { now: 46_000 + 59_000 });
    expect(count('pullRequests.checks')).toBe(2);
    await panel(client, { now: 46_000 + 60_000 });
    expect(count('pullRequests.checks')).toBe(3);
    // Refresh asks the checks again with the detail.
    await panel(client, { now: 46_000 + 61_000, refresh: 2 });
    expect([count('pullRequests.detail'), count('pullRequests.checks')]).toEqual([2, 4]);
    expect([checksRefreshIntervalMs([]), checksRefreshIntervalMs([{ status: 'action-required' }]), checksRefreshIntervalMs([{ status: 'success' }])]).toEqual([45_000, 45_000, 60_000]);
  });
  test('a checks answer read before the detail leaves it be, a later one takes only the state and the checks, and merged stays merged', async () => {
    let checksReply: Obj = { state: 'open', observedAt: 50, checks: [{ name: 'ci/build', status: 'failure' }] };
    let detailReply = detail({ observedAt: 100 });
    const { client } = fakeClient(defaults({ 'pullRequests.detail': () => detailReply, 'pullRequests.checks': () => checksReply }));
    expect((await panel(client, { now: 1_000 })).checksTone).toBe('passing');
    checksReply = { state: 'closed', observedAt: 200, checks: [{ name: 'ci/build', status: 'failure' }] };
    const later = await panel(client, { now: 1_000, refresh: 2 });
    expect([later.checksTone, later.title, later.state]).toEqual(['failing', 'Add a changelog', 'closed']);
    detailReply = detail({ observedAt: 300, state: 'merged' });
    checksReply = { state: 'open', observedAt: 400, checks: [{ name: 'ci/build', status: 'success' }] };
    const merged = await panel(client, { now: 1_000, refresh: 3 });
    expect([merged.state, merged.checksTone]).toEqual(['merged', 'passing']);
  });
  test('a server without pullRequestChecks is not asked; a closed pull request and a host that reports no checks are asked once', async () => {
    const old = fakeClient(defaults(), { pullRequests: true, threadPullRequests: true });
    await panel(old.client, { now: 1_000 });
    await panel(old.client, { now: 200_000 });
    expect(old.count('pullRequests.checks')).toBe(0);
    const closed = fakeClient(defaults({ 'pullRequests.detail': () => detail({ state: 'closed' }) }));
    await panel(closed.client, { now: 1_000 });
    await panel(closed.client, { now: 200_000 });
    expect(closed.count('pullRequests.checks')).toBe(1);
    const none = fakeClient(defaults({ 'pullRequests.checks': () => ({ state: 'open' }) }));
    await panel(none.client, { now: 1_000 });
    await panel(none.client, { now: 200_000, refresh: 2 });
    expect(none.count('pullRequests.checks')).toBe(1);
  });
});

describe('the details row merges a checks answer as the PR page does', () => {
  test('only the state and the checks, never undoing merged, and not when the detail is newer', async () => {
    let checksReply: Obj = { state: 'open', observedAt: 50, checks: [{ name: 'test', status: 'failure' }] };
    const { client } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ number: 104, observedAt: 100, checks: [{ name: 'test', status: 'pending' }] }), 'pullRequests.checks': () => checksReply }));
    const ref = { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 104 };
    expect((await readDetail(client, native, ref, 1_000))?.checks).toEqual([{ name: 'test', status: 'pending' }]);
    checksReply = { state: 'merged', observedAt: 150, checks: [{ name: 'test', status: 'success' }] };
    const polled = await readDetail(client, native, ref, 1_000 + 45_000);
    expect([polled?.state, polled?.title, polled?.checks]).toEqual(['merged', 'Add a changelog', [{ name: 'test', status: 'success' }]]);
    checksReply = { state: 'open', observedAt: 200, checks: [{ name: 'test', status: 'success' }] };
    expect((await readDetail(client, native, ref, 1_000 + 45_000 + 60_000))?.state).toBe('merged');
    forgetDetails(client);
  });
});

describe('an armed confirmation ends with its control (useInlineActionConfirm)', () => {
  test('another pull request on screen, or a primary action that changed, lets it go', async () => {
    let over: Obj = {};
    const { client } = fakeClient(defaults({ 'pullRequests.detail': payload => detail({ number: payload.number, url: `https://github.com/lane/sandbox/pull/${String(payload.number)}`, ...over }) }));
    await panel(client);
    prUiLocal(client, 'arm', selected, 'primary|merge');
    expect((await panel(client)).actions.armedAction).toBe('merge');
    const eight = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 8 });
    await panel(client, { selected: eight });
    expect(actionState(client).confirm).toBeNull();
    expect((await panel(client)).actions.armedAction).toBe('');
    prUiLocal(client, 'arm', selected, 'primary|merge');
    over = { checks: [{ name: 'ci/build', status: 'pending' }] };
    const pending = (await panel(client, { refresh: 2 })).actions;
    expect([pending.primary, pending.armedAction, pending.primaryLabel]).toEqual(['enable-auto-merge', '', 'Auto-merge (merge)']);
  });
  test('a More row that left the menu lets go of its arming; one still in the menu keeps it', async () => {
    let state = 'open';
    const { client } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ state }) }));
    await panel(client);
    prUiLocal(client, 'arm', selected, 'menu|close');
    expect((await panel(client)).actions.closeLabel).toBe('Confirm close?');
    state = 'closed';
    const closed = (await panel(client, { refresh: 2 })).actions;
    expect([closed.closeItem, closed.armedAction]).toEqual([false, '']);
    state = 'open';
    expect((await panel(client, { refresh: 3 })).actions.closeLabel).toBe('Close pull request');
  });
  test('the approval\'s arming goes when its bar leaves the screen: another tab', async () => {
    const { client } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ workflowApprovalsRequired: 2 }) }));
    expect((await panel(client)).actions.approveWorkflows).toBe(true);
    prUiLocal(client, 'arm', selected, 'approve|approve-workflows');
    expect((await panel(client, { tab: 'summary' })).actions.approveLabel).toBe('Confirm approve?');
    await panel(client, { tab: 'timeline' });
    expect(actionState(client).confirm).toBeNull();
    expect((await panel(client, { tab: 'summary' })).actions.approveLabel).toBe('Approve workflows to run');
  });
  test('the More menu and the comment composer let go of their armings as their popovers close', () => {
    const local: unknown[][] = [];
    const menu = contractComponent('pages-pr-actions.contract', 'PrdActionsMenu');
    const scope = (armedControl: string) => ({ ref: 'pr7', actions: { armedControl, armedAction: 'close' }, local: (...args: unknown[]) => local.push(args) });
    menu.call('menuToggled', menu.state(), scope('menu'), [{ newState: 'open' }]);
    menu.call('menuToggled', menu.state(), scope('primary'), [{ newState: 'closed' }]);
    expect(local).toEqual([]);
    menu.call('menuToggled', menu.state(), scope('menu'), [{ newState: 'closed' }]);
    expect(local).toEqual([['pr-ui-disarm', 'pr7', 'menu|close']]);
    const composer = contractComponent('pages-pr-compose.contract', 'PrdComposer');
    const composerScope = (armedControl: string) => ({ selected: 'pr7', actions: { armedControl, armedAction: 'close' }, local: (...args: unknown[]) => local.push(args) });
    composer.call('composerToggled', {}, composerScope('menu'), [{ newState: 'closed' }]);
    composer.call('composerToggled', {}, composerScope('comment'), [{ newState: 'closed' }]);
    expect(local.at(-1)).toEqual(['pr-ui-disarm', 'pr7', 'comment|close']);
    expect(local).toHaveLength(2);
  });
});

describe('the conversation under Hide bots (PullRequestTimelineTab)', () => {
  const comment = (id: string, author: Obj, extra: Obj = {}): Obj => ({ id, kind: 'comment', body: `Comment ${id}`, author, createdAt: '2026-10-01T00:00:00Z', ...extra });
  test('a bot is what the host says is one; its approval or change request is a verdict and stays', () => {
    const bot = { login: 'reviewer-app', isBot: true }, named = { login: 'renovate[bot]' };
    const items = pullRequestConversationItems([
      comment('chatter', bot), comment('approved', bot, { kind: 'review', reviewState: 'APPROVED' }), comment('changes', bot, { kind: 'review', reviewState: 'CHANGES_REQUESTED' }),
      comment('remark', bot, { kind: 'review', reviewState: 'COMMENTED' }), comment('suffix', named),
    ], []);
    expect(filterPullRequestConversationItems(items, { bots: false, resolved: true }).map(item => item.id)).toEqual(['approved', 'changes', 'suffix']);
    expect(filterPullRequestConversationItems(items, { bots: true, resolved: true })).toHaveLength(5);
  });
});

/** The Pull Requests list with the panel beside it, as the page asks them. */
function listFixture(entries: Obj[], extra: Record<string, Reply> = {}) {
  const { client, calls, count } = fakeClient(defaults({ 'pullRequests.list': () => ({ entries, viewers: {} }), 'pullRequests.listStats': () => ({ stats: [] }),
    'pullRequests.changeBreakdown': () => null, ...extra }));
  (client as unknown as { restAccess: () => unknown }).restAccess = () => ({ call: async () => ({ id: '1-1' }) });
  const listNative = { available: true, watch: () => {}, later: async () => ({ ok: true, generation: 1, value: {} }) } as unknown as Native;
  const page = (input: Partial<PrInput> = {}) => pullRequestsPage(client, listNative, { open: true, refresh: 0, now: Date.parse('2026-10-08T12:00:00Z'), selected: '', query: '', typed: false, ...input });
  return { client, calls, count, page };
}
const row = (number: number, extra: Obj = {}): Obj => ({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number, title: `Change ${number}`, state: 'open', isDraft: false,
  author: { login: 'lane' }, url: `https://github.com/lane/sandbox/pull/${number}`, updatedAt: '2026-10-08T11:00:00Z', additions: 2, deletions: 1, ...extra });

describe('the Pull Requests list across a relaunch and beside its panel (PullRequestsPage)', () => {
  test('a first-page answer is kept in the preference record, and a cold start shows it before the host answers again', async () => {
    const first = listFixture([row(7), row(8)]);
    await first.page();
    const saved = JSON.parse(JSON.stringify(first.client.local)) as Obj;
    const next: Obj = {};
    adoptPrSnapshots(next, saved);
    const relaunched = listFixture([row(9)]);
    Object.assign(relaunched.client.local, next);
    const restored = await relaunched.page();
    expect(relaunched.count('pullRequests.list')).toBe(0);
    expect(restored.groups.flatMap(group => group.rows.map(entry => entry.number))).toEqual([7, 8]);
    const read = await relaunched.page();
    expect(relaunched.count('pullRequests.list')).toBe(1);
    expect(read.groups.flatMap(group => group.rows.map(entry => entry.number))).toEqual([9]);
    // A search is never kept.
    await relaunched.page({ query: 'fix' });
    expect(Object.keys((relaunched.client.local as { prListAnswers: { scopes: Obj } }).prListAnswers.scopes)).toHaveLength(1);
  });
  test('a row the list could not measure takes the +/- of the pull request open beside it', async () => {
    const unmeasured = row(7, { additions: 0, deletions: 0 });
    const { client, page } = listFixture([unmeasured], { 'pullRequests.detail': () => detail({ additions: 12, deletions: 3 }) });
    const ref = rowRef(unmeasured);
    expect((await page({ selected: ref })).groups[0]!.rows[0]).toMatchObject({ additions: '', deletions: '' });
    await panel(client, { selected: ref });
    expect((await page({ selected: ref })).groups[0]!.rows[0]).toMatchObject({ additions: '+12', deletions: '-3' });
  });
  test('a row keeps the checks its list read reported; the newer checks read is the panel\'s and the shared summary\'s', async () => {
    const listed = row(7, { checksState: 'passing', updatedAt: '2026-10-08T10:00:00Z' });
    const { client, page } = listFixture([listed], { 'pullRequests.detail': () => detail({ checks: [{ name: 'ci/build', status: 'pending' }] }) });
    const ref = rowRef(listed);
    await page({ selected: ref });
    expect((await panel(client, { selected: ref })).checksTone).toBe('pending');
    // What the routed reads record (pages-pr-environments.ts): the list's row, then the panel's checks read.
    observePullRequestSummary(client, client.environmentId, listed, { ...listed, observedAt: 1 });
    observePullRequestChecks(client, client.environmentId, listed, { checks: [{ name: 'ci/build', status: 'pending' }], observedAt: 2 });
    expect(sharedPullRequestSummary(client, listed, null)?.checksState).toBe('pending');
    expect((await page({ selected: ref })).groups[0]!.rows[0]!.checks).toBe('passing');
  });
  test('Enter and a paste search at once; typing waits for the pause (usePullRequestQueryFilters)', () => {
    const list = contractComponent('pages-prs.contract', 'PrList'), sent: unknown[][] = [];
    const scope = { query: 'fix crash', search: (text: string) => sent.push(['search', text]), control: (op: string, value: string) => sent.push(['control', op, value]) };
    list.call('typed', list.state(), scope, ['fix', { inputType: 'insertText' }]);
    expect(sent).toEqual([['search', 'fix']]);
    list.call('typed', list.state(), scope, ['label:bug', { inputType: 'insertFromPaste' }]);
    list.call('submitted', list.state(), scope, []);
    expect(sent.slice(1)).toEqual([['search', 'label:bug'], ['control', 'query', 'label:bug'], ['control', 'query', 'fix crash']]);
  });
});

describe('reviewer faces carry their verdict (PullRequestReviewOutcomeAvatarBadge)', () => {
  test('the glyph rides a face with a verdict and none without one', () => {
    const face = contractComponent('pages-pr-summary.contract', 'PrdReviewerFace'), badge = 'svg position="absolute" right="-0.25rem" bottom="-0.25rem"';
    expect(face.drawn(badge, {}, { reviewer: { outcome: 'approved', stale: false } })).toBe(true);
    expect(face.drawn(badge, {}, { reviewer: { outcome: '', stale: false } })).toBe(false);
  });
});
