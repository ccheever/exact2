// A pull request's checks as every surface shows them, the list's answers across a relaunch, and how long an armed
// confirmation lives (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3). The first half ports the original tests with their
// names: packages/client-runtime pullRequestChecks.test.ts and apps/web state/pullRequests.test.ts. Not ported: "falls
// back to a fresh read when the browser blocks storage" and "removes the composite snapshots it replaced" (the answers
// live in the preference record, not in the browser's storage) — n/a-ui. The second half drives the clone's PR page
// and list (pages-pr-detail.ts, pages-prs.ts) against injected host replies, and the controls' actions as Contract
// runs them.
import { describe, expect, test } from 'bun:test';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';
import { initialShell, type Obj } from '../../src/client/domain';
import { ClientError, PREFERENCE_PATH, type Files, type Native } from '../../src/client/protocol';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { T3Client } from '../../src/client/client';
import { storage } from '../support/composer-controls-fixture';
import { checksRefreshIntervalMs, resolvePullRequestDetailChecks, PULL_REQUEST_ACTIONS } from '../../src/details/pr-row-logic';
import { readDetail, forgetDetails, rowChecksPoll } from '../../src/details/pr-row-actions';
import { observePullRequestChecks, observePullRequestSummary, sharedPullRequestSummary } from '../../src/pull-requests/pages-pr-shared';
import { noteInteraction, PR_REFRESH_KEY, prRefreshEvent, rememberListAnswer } from '../../src/pull-requests/pages-pr-refresh';
import { pagesSource } from '../../src/pages/pages-sources';
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

// The prDetail resource as pull-requests-members.contract declares it: its arguments, evaluated over the root's state,
// reach the page through pages-sources.ts, and the checks timer (prChecksTick under prChecksPending or prChecksSettled)
// moves the clock it reads.
const root = contractComponent('app.contract', 'T3Code');
const rootLines = appText('app.contract').split('\n');
const declared = (prefix: string) => rootLines.map(line => line.trim()).find(line => line.startsWith(prefix))!;
const detailCall = (() => { const line = declared('resource prDetail = '); return line.slice('resource prDetail = '.length, line.indexOf(' as shape PrDetailView')); })();
/** A gated task's gate, its key and the interval its body arms. */
const timer = (name: string, tick = 'prChecksTick') => {
  const at = rootLines.findIndex(line => line.trim().startsWith(`task ${name} when `));
  const [gate, key] = rootLines[at]!.trim().slice(`task ${name} when `.length).split(' key=');
  return { gate: gate!, key: key!, every: Number(new RegExp(`every\\((\\d+), ${tick}\\)`).exec(rootLines[at + 1]!)![1]) };
};
/** A members derive's expression. */
const derived = (name: string) => declared(`derive ${name} = `).slice(`derive ${name} = `.length);
const clock = { elapsed: 0, prChecksNow: 0 };
/** The root's state the prDetail resource and the checks timer read: the Pull requests page with its panel open. */
const pageScope = { utilityPage: 'pull-requests', prSelected: selected, shell: { panel: { pr: { selected: '' }, open: false, kind: '' } }, prRefresh: 1, wallTime: { epochAtZero: 0 },
  data: { revision: 0 }, page: { visibilityState: 'visible' }, windowReturns: 0, prTab: 'summary', liveNow: 0, settingsOpen: false, prPanelOpen: true, max: Math.max, floor: Math.floor };
/** Whether the timer runs: its gate over the root's state, with prChecksShown as the members derive it. */
const runs = (name: string, view: unknown, scope: Obj = {}) => {
  const env = { ...pageScope, ...scope, prDetail: view };
  return value(timer(name).gate, { ...env, prChecksShown: value(derived('prChecksShown'), env) });
};
/** The runner asking prDetail on the root's clock (`elapsed`, the minute tick; `prChecksNow`, the checks timer). */
async function askOnClock(client: T3Client, scope: Obj = {}) {
  const env = { ...pageScope, ...clock, ...scope };
  const args = value(detailCall, { ...env, prChecksShown: value(derived('prChecksShown'), env), prDetail: (...values: unknown[]) => values }) as unknown[];
  let seen = wakes, view = await pagesSource(client, native, 'prDetail', args) as Awaited<ReturnType<typeof pullRequestDetail>>;
  for (let asked = 0; wakes > seen && asked < 8; asked++) { seen = wakes; view = await pagesSource(client, native, 'prDetail', args) as typeof view; }
  return view;
}
/** The timer's tick at `at`, as the runner runs prChecksTick. */
const tick = (at: number) => { const state = { prChecksNow: clock.prChecksNow }; root.call('prChecksTick', state, { performanceNow: () => at }, []); clock.prChecksNow = state.prChecksNow as number; };
const announce = (client: T3Client, value: number) => prRefreshEvent(client, { key: PR_REFRESH_KEY, subscriptionId: '1-1', value });

describe('the PR page reads its checks as the details row does (usePullRequestDetail, usePullRequestChecksRefresh)', () => {
  test('the detail around the server\'s cache, then the checks alone: every 45 s while runs are pending, 60 s once they settle', async () => {
    let checks = [{ name: 'ci/build', status: 'pending' }];
    const { client, calls, count } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ checks }), 'pullRequests.checks': () => ({ state: 'open', checks }) }));
    Object.assign(clock, { elapsed: 1_000, prChecksNow: 0 });
    let view = await askOnClock(client);
    expect(calls.find(call => call.method === 'pullRequests.detail')?.payload).toMatchObject({ number: 7, allowStale: false });
    expect(count('pullRequests.checks')).toBe(1);
    // Pending runs arm the 45 s timer, not the settled one.
    const pending = timer('prChecksPending'), settled = timer('prChecksSettled');
    expect([pending.every, settled.every]).toEqual([45_000, 60_000]);
    expect([runs('prChecksPending', view), runs('prChecksSettled', view)]).toEqual([true, false]);
    // Another ask before the timer comes round (a data revision on the same clock) reads nothing.
    await askOnClock(client, { data: { revision: 1 } });
    expect(count('pullRequests.checks')).toBe(1);
    checks = [{ name: 'ci/build', status: 'success' }];
    tick(1_000 + pending.every);
    view = await askOnClock(client);
    expect(count('pullRequests.checks')).toBe(2);
    expect([view.checksSummary, view.checksTone]).toEqual(['All checks passed', 'passing']);
    expect([runs('prChecksPending', view), runs('prChecksSettled', view)]).toEqual([false, true]);
    // The minute clock coming round 15 s later reads nothing; the settled timer's tick does.
    clock.elapsed = 61_000;
    await askOnClock(client);
    expect(count('pullRequests.checks')).toBe(2);
    tick(1_000 + pending.every + settled.every);
    await askOnClock(client);
    expect(count('pullRequests.checks')).toBe(3);
    // Refresh reads the detail and its activity again, not the checks (refreshDetail); an announcement reads all three.
    await askOnClock(client, { prRefresh: 2 });
    expect([count('pullRequests.detail'), count('pullRequests.checks')]).toEqual([2, 3]);
    announce(client, 1);
    await askOnClock(client, { prRefresh: 2 });
    expect([count('pullRequests.detail'), count('pullRequests.checks')]).toEqual([3, 4]);
    expect([checksRefreshIntervalMs([]), checksRefreshIntervalMs([{ status: 'action-required' }]), checksRefreshIntervalMs([{ status: 'success' }])]).toEqual([45_000, 45_000, 60_000]);
  });
  test('the timer runs only while the detail is drawn: not with the page\'s panel closed or Settings over the page; a thread\'s surface keeps it under Settings', async () => {
    const { client, count } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ checks: [{ name: 'ci/build', status: 'pending' }] }) }));
    Object.assign(clock, { elapsed: 1_000, prChecksNow: 0 });
    const shown = await askOnClock(client);
    expect([shown.checksPoll, runs('prChecksPending', shown), count('pullRequests.checks')]).toEqual(['pending', true, 1]);
    // The panel closed (prTogglePanel keeps the selection): no timer, and the clock coming round reads nothing.
    const closed = { prPanelOpen: false };
    tick(1_000 + 45_000);
    const hidden = await askOnClock(client, closed);
    expect([hidden.checksPoll, runs('prChecksPending', hidden, closed), count('pullRequests.checks')]).toEqual(['', false, 1]);
    // Settings over the page leaves utilityPage on it.
    const settings = { settingsOpen: true };
    tick(1_000 + 90_000);
    expect([(await askOnClock(client, settings)).checksPoll, count('pullRequests.checks')]).toEqual(['', 1]);
    // A thread's pull request surface stays mounted under Settings, as the thread does.
    const surface = { utilityPage: '', settingsOpen: true, shell: { panel: { pr: { selected }, open: true, kind: 'pull-request' } } };
    tick(1_000 + 135_000);
    const thread = await askOnClock(client, surface);
    expect([thread.checksPoll, runs('prChecksPending', thread, surface), count('pullRequests.checks')]).toEqual(['pending', true, 2]);
  });
  test('the timer starts again for another pull request (useLiveRefresh\'s key)', async () => {
    const { client } = fakeClient(defaults({ 'pullRequests.detail': payload => detail({ number: payload.number, url: `https://github.com/lane/sandbox/pull/${String(payload.number)}`, checks: [{ name: 'ci/build', status: 'pending' }] }) }));
    Object.assign(clock, { elapsed: 1_000, prChecksNow: 0 });
    const seven = await askOnClock(client);
    const eight = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 8 });
    const other = await askOnClock(client, { prSelected: eight });
    for (const name of ['prChecksPending', 'prChecksSettled']) {
      const { key } = timer(name);
      expect(key).toBe('prDetail.ref');
      expect([value(key, { prDetail: seven }), value(key, { prDetail: other })]).toEqual([seven.ref, other.ref]);
    }
    expect(seven.ref).not.toBe(other.ref);
  });
  test('the timer stops once the reader has been idle for six minutes and comes back with them (useLiveRefresh)', async () => {
    const { client, count } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ checks: [{ name: 'ci/build', status: 'pending' }] }) }));
    Object.assign(clock, { elapsed: 1_000, prChecksNow: 0 });
    noteInteraction(client, 1_000, false);
    await askOnClock(client);
    clock.elapsed = 1_000 + 6 * 60_000;
    tick(1_000 + 6 * 60_000);
    const idle = await askOnClock(client);
    expect([idle.checksPoll, count('pullRequests.checks')]).toEqual(['', 1]);
    noteInteraction(client, 1_000 + 6 * 60_000 + 5_000, true);
    tick(1_000 + 6 * 60_000 + 5_000);
    const back = await askOnClock(client);
    expect([back.checksPoll, count('pullRequests.checks')]).toEqual(['pending', 2]);
  });
  test('a checks answer read before the detail leaves it be, a later one takes only the state and the checks, and merged stays merged', async () => {
    let checksReply: Obj = { state: 'open', observedAt: 50, checks: [{ name: 'ci/build', status: 'failure' }] };
    let detailReply = detail({ observedAt: 100 });
    const { client } = fakeClient(defaults({ 'pullRequests.detail': () => detailReply, 'pullRequests.checks': () => checksReply }));
    expect((await panel(client, { now: 1_000 })).checksTone).toBe('passing');
    checksReply = { state: 'closed', observedAt: 200, checks: [{ name: 'ci/build', status: 'failure' }] };
    announce(client, 1);
    const later = await panel(client, { now: 1_000 });
    expect([later.checksTone, later.title, later.state]).toEqual(['failing', 'Add a changelog', 'closed']);
    detailReply = detail({ observedAt: 300, state: 'merged' });
    checksReply = { state: 'open', observedAt: 400, checks: [{ name: 'ci/build', status: 'success' }] };
    announce(client, 2);
    const merged = await panel(client, { now: 1_000 });
    expect([merged.state, merged.checksTone]).toEqual(['merged', 'passing']);
  });
  test('a server without pullRequestChecks is not asked; a closed pull request and a host that reports no checks are asked once', async () => {
    const old = fakeClient(defaults(), { pullRequests: true, threadPullRequests: true });
    await panel(old.client, { now: 1_000 });
    await panel(old.client, { now: 200_000 });
    expect(old.count('pullRequests.checks')).toBe(0);
    const closed = fakeClient(defaults({ 'pullRequests.detail': () => detail({ state: 'closed' }) }));
    await panel(closed.client, { now: 1_000 });
    expect((await panel(closed.client, { now: 200_000 })).checksPoll).toBe('');
    expect(closed.count('pullRequests.checks')).toBe(1);
    const none = fakeClient(defaults({ 'pullRequests.checks': () => null }));
    await panel(none.client, { now: 1_000 });
    announce(none.client, 1);
    expect((await panel(none.client, { now: 200_000 })).checksPoll).toBe('');
    expect(none.count('pullRequests.checks')).toBe(1);
  });
});

/** The card's row in a visible window (prRowsView's `poll`). */
const visibleRow = { visible: true };
describe('the details row merges a checks answer as the PR page does', () => {
  test('only the state and the checks, never undoing merged, and not when the detail is newer', async () => {
    let checksReply: Obj = { state: 'open', observedAt: 50, checks: [{ name: 'test', status: 'failure' }] };
    const { client } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ number: 104, observedAt: 100, checks: [{ name: 'test', status: 'pending' }] }), 'pullRequests.checks': () => checksReply }));
    const ref = { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 104 };
    expect((await readDetail(client, native, ref, 1_000, visibleRow))?.checks).toEqual([{ name: 'test', status: 'pending' }]);
    checksReply = { state: 'merged', observedAt: 150, checks: [{ name: 'test', status: 'success' }] };
    const polled = await readDetail(client, native, ref, 1_000 + 45_000, visibleRow);
    expect([polled?.state, polled?.title, polled?.checks]).toEqual(['merged', 'Add a changelog', [{ name: 'test', status: 'success' }]]);
    checksReply = { state: 'open', observedAt: 200, checks: [{ name: 'test', status: 'success' }] };
    expect((await readDetail(client, native, ref, 1_000 + 45_000 + 60_000, visibleRow))?.state).toBe('merged');
    forgetDetails(client);
  });
  test('a host whose checks answer is null is not polled again; a failed read is (ThreadDetailsPrRow isSettledWithoutChecks)', async () => {
    let fail = true;
    const failing = fakeClient(defaults({ 'pullRequests.detail': () => detail({ number: 105, checks: [] }), 'pullRequests.checks': () => { if (fail) throw new Error('GitHub API rate limit exceeded'); return { state: 'open', checks: [] }; } }));
    const ref = { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 105 };
    await readDetail(failing.client, native, ref, 1_000, visibleRow);
    fail = false;
    await readDetail(failing.client, native, ref, 1_000 + 45_000, visibleRow);
    expect(failing.count('pullRequests.checks')).toBe(2);
    forgetDetails(failing.client);
    const none = fakeClient(defaults({ 'pullRequests.detail': () => detail({ number: 105, checks: [] }), 'pullRequests.checks': () => null }));
    await readDetail(none.client, native, ref, 1_000, visibleRow);
    await readDetail(none.client, native, ref, 1_000 + 45_000, visibleRow);
    await readDetail(none.client, native, ref, 1_000 + 105_000, visibleRow);
    expect(none.count('pullRequests.checks')).toBe(1);
    forgetDetails(none.client);
  });
  test('the row polls on its own timer as the page does: 45 s pending, 60 s settled, not in a hidden window or for an idle reader', async () => {
    const { client, count } = fakeClient(defaults({ 'pullRequests.detail': () => detail({ number: 106, checks: [{ name: 'test', status: 'pending' }] }) }));
    const ref = { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 106 };
    noteInteraction(client, 1_000, false);
    await readDetail(client, native, ref, 1_000, visibleRow);
    expect([rowChecksPoll(client, ref, 1_000, true), rowChecksPoll(client, ref, 1_000, false)]).toEqual(['pending', '']);
    // A hidden window reads nothing; a visible one reads at the tick.
    await readDetail(client, native, ref, 1_000 + 45_000, { visible: false });
    expect(count('pullRequests.checks')).toBe(1);
    await readDetail(client, native, ref, 1_000 + 45_000, visibleRow);
    expect(count('pullRequests.checks')).toBe(2);
    // Six idle minutes stop it.
    await readDetail(client, native, ref, 1_000 + 6 * 60_000, visibleRow);
    expect([count('pullRequests.checks'), rowChecksPoll(client, ref, 1_000 + 6 * 60_000, true)]).toEqual([2, '']);
    forgetDetails(client);
    // shell-members.contract's row timer: its gates and key over the card's rows, its tick into the clock the card reads.
    const rows = (checksPoll: string) => ({ prRows: { checksPoll, checksKey: 'k' } });
    const pending = timer('prRowChecksPending', 'prRowChecksTick'), settled = timer('prRowChecksSettled', 'prRowChecksTick');
    expect([pending.every, settled.every, pending.key, settled.key]).toEqual([45_000, 60_000, 'details.prRows.checksKey', 'details.prRows.checksKey']);
    expect([value(pending.gate, { details: rows('pending') }), value(settled.gate, { details: rows('pending') }), value(settled.gate, { details: rows('settled') }), value(pending.gate, { details: rows('') })])
      .toEqual([true, false, true, false]);
    const state = { prRowChecksNow: 0 };
    root.call('prRowChecksTick', state, { performanceNow: () => 46_000 }, []);
    const detailsCall = (() => { const line = declared('resource details = '); return line.slice('resource details = '.length, line.indexOf(' as shape ShellDetails')); })();
    const args = value(detailsCall, { shellDetails: (...values: unknown[]) => values, detailsShown: true, data: { threadId: 't1', revision: 0, sidebarOpen: false }, conversationWidth: 800, wallTime: { epochAtZero: 0 },
      shellClock: 30_000, prRowChecksNow: state.prRowChecksNow, viewport: { width: 1280 }, sidebarWidth: 0, page: { hasFocus: false, visibilityState: 'visible' }, lineageWorkflows: [], lineagePhasesOpen: [],
      lineagePhasesClosed: [], max: Math.max, floor: Math.floor }) as unknown[];
    // The card's clock is the later of the shell's and the row timer's; the window counts as visible without the focus.
    expect([args[4], args[6], args[10]]).toEqual([46_000, false, true]);
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
  // The preference file as T3Client writes it (persist: the whole record, `version: 1`).
  Object.assign(client.local, { selections: {} });
  client.savePreferences = (files: Files) => (T3Client.prototype as unknown as { persist(this: T3Client, files: Files): Promise<void> }).persist.call(client, files);
  const page = (input: Partial<PrInput> = {}, files?: Files) =>
    pullRequestsPage(client, listNative, { open: true, refresh: 0, now: Date.parse('2026-10-08T12:00:00Z'), selected: '', query: '', typed: false, ...input }, files);
  return { client, calls, count, page };
}
const row = (number: number, extra: Obj = {}): Obj => ({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number, title: `Change ${number}`, state: 'open', isDraft: false,
  author: { login: 'lane' }, url: `https://github.com/lane/sandbox/pull/${number}`, updatedAt: '2026-10-08T11:00:00Z', additions: 2, deletions: 1, ...extra });

describe('the Pull Requests list across a relaunch and beside its panel (PullRequestsPage)', () => {
  test('a first-page answer is written to the preference file, and a cold start shows it before the host answers again', async () => {
    const disk = storage(), first = listFixture([row(7), row(8)]);
    await first.page({}, disk);
    // Read back as a launch reads its preferences (T3Client.load, adoptPrSnapshots).
    const relaunch = new T3Client();
    await (relaunch as unknown as { load(files: Files): Promise<void> }).load(disk);
    const relaunched = listFixture([row(9)]);
    relaunched.client.local = { ...relaunch.local, selections: {} } as T3Client['local'];
    const restored = await relaunched.page({}, disk);
    expect(relaunched.count('pullRequests.list')).toBe(0);
    expect(restored.groups.flatMap(group => group.rows.map(entry => entry.number))).toEqual([7, 8]);
    const read = await relaunched.page({}, disk);
    expect(relaunched.count('pullRequests.list')).toBe(1);
    expect(read.groups.flatMap(group => group.rows.map(entry => entry.number))).toEqual([9]);
    // The new answer replaced the kept one on disk; a search is never kept.
    await relaunched.page({ query: 'fix' }, disk);
    const file = JSON.parse(new TextDecoder().decode(await disk.fs.readFile(PREFERENCE_PATH))) as { prListAnswers: { scopes: Record<string, Record<string, { entries: Obj[] }>> } };
    const scopes = Object.values(file.prListAnswers.scopes);
    expect([scopes.length, scopes[0]![relaunched.client.environmentId]!.entries.map(entry => entry.number)]).toEqual([1, [9]]);
  });
  test('each server\'s answer is written as it lands: a read let go at the next server keeps the first on disk', async () => {
    const disk = storage();
    const entry = { key: 'http://127.0.0.1:16551\nenv-b', origin: 'http://127.0.0.1:16551', environmentId: 'env-b', phase: 'connected', message: '', traceId: '', generation: 4, synchronized: 4,
      lastEvent: 0, subscriptions: {}, config: { environment: { label: 'Lane B', capabilities: { pullRequests: true } } }, scopes: ['orchestration:read'], error: '', requested: true,
      shell: { ...initialShell(), projects: [{ id: 'b1', title: 'other', workspaceRoot: '/b', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/other' } }] } } as unknown as FleetEntry;
    fleet.entries.set(entry.key, entry);
    try {
      const { client, page } = listFixture([row(7)], { 'pullRequests.list': payload => {
        if (payload.environmentId === 'env-b') throw new ClientError('The answer was let go.', 'superseded');
        return { entries: [row(7)], viewers: {} };
      } });
      await expect(page({}, disk)).rejects.toThrow('let go');
      const file = JSON.parse(new TextDecoder().decode(await disk.fs.readFile(PREFERENCE_PATH))) as { prListAnswers: { scopes: Record<string, Record<string, { entries: Obj[] }>> } };
      expect(Object.values(file.prListAnswers.scopes).map(scope => Object.keys(scope))).toEqual([[client.environmentId]]);
    } finally { fleet.entries.delete(entry.key); }
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
