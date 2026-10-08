// The pull request panel's header actions and host stack (pr-header-actions-and-stacks). The first
// half ports T3 Code 1e2ecbd975's tests with their original names (pullRequestDetail.logic.test.ts,
// pullRequestStackSnapshot.test.ts, pullRequestList.logic.test.ts; "what to say when an action fails"
// is in pages-pr-logic.test.ts). Not ported: "hands back the held object for a row a refresh did not
// change" and "does not hand back the old order when only the order changed" (reusePullRequestEntries,
// a React memo aid: Exact diffs the answer itself) — n/a-ui. The second half drives the clone's panel
// (pages-pr-detail.ts → pages-pr-actions.ts / pages-pr-stack.ts) against injected host replies: the
// states, permissions and failures the real-GitHub lane cannot produce on request.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { toasts } from './toast';
import { prCommand, prUiLocal, pullRequestDetail } from './pages-pr-detail';
import { pullRequestsPage } from './pages-prs';
import {
  PULL_REQUEST_ACTIONS, allowsSinglePullRequestMerge, isStackedPullRequestBase, pullRequestActionMenuHasGroup, pullRequestActionNeedsHostRefresh,
  resolveBaseFreshness, resolvePullRequestMergeMethod, resolvePullRequestPrimaryControl,
} from './r6-pr-logic';
import { applyPullRequestOverrides, pullRequestOverrideAfterAction, settlePullRequestOverrides, type ListOverride } from './pages-pr-actions';
import { pullRequestStackView, savedPullRequestStack } from './pages-pr-stack';
import { noteNow } from './composer-controls';

// ── Ported (original names) ─────────────────────────────────────────────────

describe('pull request merge method', () => {
  test('uses the current choice, then the project default, then the last choice', () => {
    expect(resolvePullRequestMergeMethod(['merge', 'squash', 'rebase'], null, 'squash', 'rebase')).toBe('squash');
    expect(resolvePullRequestMergeMethod(['merge', 'squash', 'rebase'], 'rebase', 'squash', 'merge')).toBe('rebase');
    expect(resolvePullRequestMergeMethod(['merge', 'rebase'], null, 'squash', 'rebase')).toBe('rebase');
    expect(resolvePullRequestMergeMethod(['squash'], null, 'merge', 'rebase')).toBe('squash');
  });
});

describe('pull request action menu', () => {
  test('keeps the group divider when auto-merge is the only action', () => {
    expect(pullRequestActionMenuHasGroup(false, true, false)).toBe(true);
  });
});

describe('pull request primary control', () => {
  const open = { state: 'open', isDraft: false, mergeability: 'mergeable', checksState: 'passing', autoMergeEnabled: false as boolean | undefined, hasMergeMethod: true, canMerge: true, canMarkReady: true, canEnableAutoMerge: true };
  test('moves pending and failing checks to auto-merge', () => {
    expect(resolvePullRequestPrimaryControl({ ...open, checksState: 'pending' })).toBe('enable-auto-merge');
    expect(resolvePullRequestPrimaryControl({ ...open, checksState: 'failing' })).toBe('enable-auto-merge');
  });
  test('does not offer auto-merge while the host state is unknown', () => {
    expect(resolvePullRequestPrimaryControl({ ...open, checksState: 'pending', autoMergeEnabled: undefined })).toBe('merge');
  });
  test('keeps armed and terminal states in the merge button slot', () => {
    expect(resolvePullRequestPrimaryControl({ ...open, autoMergeEnabled: true })).toBe('auto-merge-armed');
    expect(resolvePullRequestPrimaryControl({ ...open, state: 'merged' })).toBe('merged');
    expect(resolvePullRequestPrimaryControl({ ...open, state: 'closed' })).toBe('closed');
  });
  test('keeps conflicts and drafts actionable before merge', () => {
    expect(resolvePullRequestPrimaryControl({ ...open, mergeability: 'conflicting' })).toBe('resolve');
    expect(resolvePullRequestPrimaryControl({ ...open, isDraft: true })).toBe('ready');
  });
});

describe('stacked pull request classification', () => {
  test('requires a known default branch', () => {
    expect(isStackedPullRequestBase('main', [{ name: 'main', isDefault: false }])).toBe(false);
  });
  test('recognizes local and remote forms of the default branch', () => {
    expect(isStackedPullRequestBase('main', [{ name: 'main', isDefault: true, isRemote: false }])).toBe(false);
    expect(isStackedPullRequestBase('main', [{ name: 'origin/main', isDefault: true, isRemote: true, remoteName: 'origin' }])).toBe(false);
  });
  test('classifies a non-default base as stacked once the default is known', () => {
    expect(isStackedPullRequestBase('feature-base', [{ name: 'origin/main', isDefault: true, isRemote: true, remoteName: 'origin' }])).toBe(true);
  });
  test('does not mistake a nested branch suffix for the default branch', () => {
    expect(isStackedPullRequestBase('main', [{ name: 'origin/feature/main', isDefault: true, isRemote: true, remoteName: 'origin' }])).toBe(true);
    expect(isStackedPullRequestBase('1.0', [{ name: 'release/1.0', isDefault: true, isRemote: false }])).toBe(true);
  });
});

describe('how the branch stands against its base', () => {
  const detail = (overrides: Obj = {}): Obj => ({ state: 'open', mergeability: 'mergeable', baseComparison: 'behind', behindBy: 12,
    capabilities: { updateMethods: ['merge', 'rebase'] }, viewerPermissions: { updateMethods: ['merge', 'rebase'] }, ...overrides });
  test('offers both ways where the host and the reader both allow them', () => {
    expect(resolveBaseFreshness(detail())).toEqual({ behindBy: 12, methods: ['merge', 'rebase'] });
  });
  test('says nothing about a branch that is already current', () => {
    expect(resolveBaseFreshness(detail({ baseComparison: 'up-to-date' }))).toBeNull();
  });
  test('says nothing where the host could not compare, rather than claiming it is current', () => {
    expect(resolveBaseFreshness(detail({ baseComparison: 'unknown' }))).toBeNull();
    expect(resolveBaseFreshness(detail({ baseComparison: undefined }))).toBeNull();
  });
  test('leaves a conflicting branch to the conflicts row', () => {
    expect(resolveBaseFreshness(detail({ mergeability: 'conflicting' }))).toBeNull();
  });
  test('says nothing where the host has no merge verdict yet', () => {
    expect(resolveBaseFreshness(detail({ mergeability: 'unknown' }))).toBeNull();
  });
  test('says nothing about a merged or closed pull request', () => {
    expect(resolveBaseFreshness(detail({ state: 'merged' }))).toBeNull();
    expect(resolveBaseFreshness(detail({ state: 'closed' }))).toBeNull();
  });
  test('narrows to what this reader may actually take', () => {
    expect(resolveBaseFreshness(detail({ viewerPermissions: { updateMethods: ['merge'] } }))?.methods).toEqual(['merge']);
  });
  test('still reports the news where the reader may take none of it', () => {
    expect(resolveBaseFreshness(detail({ viewerPermissions: {} }))).toEqual({ behindBy: 12, methods: [] });
  });
  test('reports a count only where the host counted', () => {
    expect(resolveBaseFreshness(detail({ behindBy: undefined }))?.behindBy).toBeNull();
  });
});

describe('which actions need the host read again after they run', () => {
  test('classifies every action the contract knows about', () => {
    expect(PULL_REQUEST_ACTIONS.map(pullRequestActionNeedsHostRefresh)).toEqual(PULL_REQUEST_ACTIONS.map(action => action === 'update-branch' || action === 'approve-workflows'));
  });
  test('sends update-branch back to the host, having moved the head commit', () => {
    expect(pullRequestActionNeedsHostRefresh('update-branch')).toBe(true);
  });
  test('leaves every action that only changes metadata to the cheaper detail refresh', () => {
    for (const action of ['ready', 'draft', 'close', 'reopen', 'enable-auto-merge', 'disable-auto-merge', 'merge', 'revert']) expect(pullRequestActionNeedsHostRefresh(action)).toBe(false);
  });
});

describe('single-PR merge compatibility during stack discovery', () => {
  test.each([
    [false, true, false, null, true],
    [false, false, true, null, true],
    [true, false, true, null, false],
    [true, false, false, 'Lookup failed', false],
    [true, true, false, null, false],
    [true, false, false, null, true],
  ] as const)('capability=%s stack=%s pending=%s error=%s permits=%s', (supportsStackActions, hasStack, stackPending, stackError, allowed) => {
    expect(allowsSinglePullRequestMerge({ supportsStackActions, hasStack, stackPending, stackError })).toBe(allowed);
  });
});

const savedReference = { host: 'github.com', repository: 'acme/web', number: 2 };
const link: Obj = {
  host: 'github.com', repository: 'acme/web', number: 2, url: 'https://github.com/acme/web/pull/2', source: 'manual', linkedAt: '2026-09-09T10:00:00Z',
  snapshot: { title: 'Top layer', headBranch: 'top', baseBranch: 'bottom', state: 'open', isDraft: false, updatedAt: null, syncedAt: '2026-09-09T10:00:00Z' },
  stack: { kind: 'native', id: 'stack', number: 3, url: 'https://github.com/acme/web/pull/3', base: 'main', layers: [{ number: 1, headBranch: 'bottom', state: 'open' }, { number: 2, headBranch: 'top', state: 'open' }] },
};
describe('saved stack navigation', () => {
  test('preserves all native layers even when only one has a linked snapshot, without action SHAs', () => {
    const saved = savedPullRequestStack([link], savedReference);
    expect(saved?.layers).toEqual([{ number: 1, headBranch: 'bottom', state: 'open' }, { number: 2, headBranch: 'top', state: 'open', title: 'Top layer', isDraft: false }]);
    expect(savedPullRequestStack([link], { ...savedReference, number: 1 })?.number).toBe(3);
  });
  test('does not borrow stacks across hosts or repositories', () => {
    expect(savedPullRequestStack([link], { ...savedReference, host: 'enterprise.example' })).toBeNull();
    expect(savedPullRequestStack([link], { ...savedReference, repository: 'other/web' })).toBeNull();
    expect(savedPullRequestStack([link], { ...savedReference, host: undefined })).toBeNull();
  });
  test('honors a newer saved removal across linked threads', () => {
    expect(savedPullRequestStack([link, { ...link, stack: null, snapshot: { ...(link.snapshot as Obj), syncedAt: '2026-09-09T11:00:00Z' } }], savedReference)).toBeNull();
  });
  test('shows saved data during loading and marks a failed refresh stale', () => {
    const saved = savedPullRequestStack([link], savedReference);
    const query = { data: null, isSuccess: false, isPending: true, error: null };
    expect(pullRequestStackView(query, saved)).toMatchObject({ data: saved, isFresh: false, notice: expect.stringContaining('Refreshing') });
    expect(pullRequestStackView({ ...query, isPending: false, error: 'Rate limited' }, saved)).toMatchObject({ data: saved, isFresh: false, notice: expect.stringContaining('stale') });
  });
  test('prefers refreshed data and honors a successful absence', () => {
    const saved = savedPullRequestStack([link], savedReference);
    const query = { data: saved, isSuccess: true, isPending: false, error: null };
    expect(pullRequestStackView(query, null)).toEqual({ data: saved, isFresh: true, notice: null });
    expect(pullRequestStackView({ ...query, data: null }, saved)).toEqual({ data: null, isFresh: true, notice: null });
    expect(pullRequestStackView({ ...query, isPending: true }, saved).isFresh).toBe(false);
  });
});

describe('pull request list overrides', () => {
  const entry = (number: number, state: string): Obj => ({ host: 'github.com', repository: 'pingdotgg/t3code', number, state, isDraft: false, updatedAt: '2026-07-01T00:00:00Z', labels: [] });
  const key = (row: Obj) => `#${row.number}`;
  test("maps the actions that change a row's state and nothing else", () => {
    const now = new Date('2026-07-02T00:00:00Z');
    expect(pullRequestOverrideAfterAction(entry(1, 'open'), 'close', now, 7)).toEqual({ state: 'closed', updatedAt: '2026-07-02T00:00:00.000Z', token: 7, at: now.getTime() });
    expect(pullRequestOverrideAfterAction(entry(1, 'closed'), 'reopen', now, 1)?.state).toBe('open');
    expect(pullRequestOverrideAfterAction(entry(1, 'open'), 'merge', now, 1)?.state).toBe('merged');
    expect(pullRequestOverrideAfterAction(entry(1, 'open'), 'draft', now, 1)?.isDraft).toBe(true);
    expect(pullRequestOverrideAfterAction(entry(1, 'open'), 'update-branch', now, 1)).toBeNull();
  });
  test('writes the override over the row and drops it from a list whose state it left', () => {
    const rows = [entry(1, 'open'), entry(2, 'open')];
    const overrides = new Map<string, ListOverride>([['#1', { state: 'closed', updatedAt: '2026-07-03T00:00:00Z', token: 1, at: 0 }]]);
    expect(applyPullRequestOverrides(rows, overrides, key, 'open').map(row => row.number)).toEqual([2]);
    expect(applyPullRequestOverrides(rows, overrides, key, 'all').map(row => [row.number, row.state])).toEqual([[1, 'closed'], [2, 'open']]);
    expect(applyPullRequestOverrides(rows, new Map(), key, 'open')).toBe(rows);
  });
});

describe('pull request list override settlement', () => {
  const entry = (number: number, state: string): Obj => ({ number, state, isDraft: false, labels: [] });
  const key = (row: Obj) => `#${row.number}`;
  test('keeps an override until an answer agrees with it', () => {
    const at = 1_000_000;
    const overrides = new Map<string, ListOverride>([['#1', { state: 'closed', updatedAt: '2026-07-03T00:00:00Z', token: 1, at }]]);
    expect(settlePullRequestOverrides(overrides, [entry(1, 'open')], key, at + 5_000)).toBe(overrides);
    expect(settlePullRequestOverrides(overrides, [entry(2, 'open')], key, at + 5_000).size).toBe(1);
    expect(settlePullRequestOverrides(overrides, [entry(1, 'closed')], key, at + 5_000).size).toBe(0);
    expect(settlePullRequestOverrides(overrides, [entry(1, 'open')], key, at + 90_000).size).toBe(0);
  });
});

// ── The clone's panel against injected host replies ─────────────────────────

const VERDICTS = ['comment', 'approve', 'request-changes'];
const CAPABILITIES = { diff: true, comment: true, actions: [...PULL_REQUEST_ACTIONS], mergeMethods: ['merge', 'squash', 'rebase'], updateMethods: ['merge', 'rebase'], stacks: true, stackActions: true,
  reviewers: { request: true, listCandidates: true }, edit: { changeRequest: true, comment: true }, labels: true, review: { inlineComment: true, reply: true, resolve: true, verdicts: VERDICTS } };
const WRITE = { stackRebase: true, actions: [...PULL_REQUEST_ACTIONS], comment: true, resolve: true, verdicts: VERDICTS, requestReviewers: true, updateMethods: ['merge', 'rebase'], labels: true };
const base = (over: Obj = {}): Obj => ({
  provider: 'github', projectId: 'p1', repository: 'lane/sandbox', number: 7, title: 'Add a changelog', body: '', url: 'https://github.com/lane/sandbox/pull/7', workspaceRoot: '/repos/sandbox',
  state: 'open', isDraft: false, mergeability: 'mergeable', baseComparison: 'up-to-date', changedFiles: 1, additions: 3, deletions: 0, headBranch: 'feature/changelog', baseBranch: 'main',
  author: { login: 'lane-primary' }, checks: [{ name: 'ci/build', status: 'success' }], labels: [], mergeCapabilities: { merge: true, squash: true, rebase: true },
  autoMergeEnabled: false, capabilities: CAPABILITIES, viewerPermissions: WRITE, updatedAt: '2026-10-08T10:00:00Z', ...over,
});
type Reply = (payload: Obj) => unknown;
function fakeClient(replies: Record<string, Reply>, settings: Obj = {}) {
  const calls: { method: string; payload: Obj }[] = [];
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, revision: 0, generation: 1, local: {},
    config: { environment: { capabilities: { pullRequests: true, threadPullRequests: true, pullRequestStackActions: true } }, settings },
    shell: { projects: [{ id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox', locator: { remoteUrl: 'https://github.com/lane/sandbox.git' } } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj) => {
      calls.push({ method, payload });
      const reply = replies[method];
      if (!reply) throw new Error(`no reply for ${method}`);
      return reply(payload);
    },
  } as unknown as T3Client;
  return { client, calls, writes: () => calls.filter(call => call.method === 'pullRequests.runAction').map(call => call.payload) };
}
const defaults = (detail: Obj, extra: Record<string, Reply> = {}): Record<string, Reply> => ({
  'pullRequests.detail': () => detail, 'pullRequests.activity': () => ({ comments: [], commits: [], reviewers: [], commentCount: 0 }),
  'pullRequests.stack': () => null, 'vcs.listRefs': () => ({ refs: [{ name: 'origin/main', isDefault: true, isRemote: true, remoteName: 'origin' }] }),
  'pullRequests.invalidate': () => ({}), 'pullRequests.runAction': () => ({}), ...extra,
});
let wakes = 0;
const native = { available: true, watch: () => {}, later: async () => { wakes++; return { ok: true }; } } as unknown as Native;
const selected = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 });
let refresh = 100;
/** The panel's resource as the runner asks it: again while an answer woke it. */
async function panel(client: T3Client, input: { selected?: string; refresh?: number } = {}) {
  const ask = { selected: input.selected ?? selected, refresh: input.refresh ?? refresh, now: 0 };
  let seen = wakes, view = await pullRequestDetail(client, native, ask);
  for (let asked = 0; wakes > seen && asked < 8; asked++) { seen = wakes; view = await pullRequestDetail(client, native, ask); }
  return view;
}
const header = async (detail: Obj) => { const { client } = fakeClient(defaults(detail)); return (await panel(client)).actions; };

describe('the header per state (resolvePullRequestPrimaryControl and the menu matrix)', () => {
  test('open and clean: Merge with the method, Enable auto-merge, three method radios, Close', async () => {
    const actions = await header(base());
    expect([actions.primary, actions.primaryLabel, actions.armedLabel, actions.draftToggle, actions.mergeNow, actions.autoMerge]).toEqual(['merge', 'Merge', '', 'draft', false, 'enable']);
    expect(actions.methods.map(item => [item.label, item.selected])).toEqual([['Merge', true], ['Squash and merge', false], ['Rebase and merge', false]]);
    expect([actions.methodsSeparator, actions.groupSeparator, actions.closeItem, actions.reopenItem, actions.revertItem]).toEqual([true, true, true, false, false]);
  });
  test('failing checks: "Auto-merge (merge)" in the slot, Merge now in the menu', async () => {
    const actions = await header(base({ checks: [{ name: 'ci/build', status: 'success' }, { name: 'ci/test', status: 'failure' }] }));
    expect([actions.primary, actions.primaryLabel, actions.mergeNow, actions.autoMerge]).toEqual(['enable-auto-merge', 'Auto-merge (merge)', true, '']);
  });
  test('a conflict: Resolve conflicts (destructive), no methods and no auto-merge; the armed badge stays beside it', async () => {
    const actions = await header(base({ mergeability: 'conflicting' }));
    expect([actions.primary, actions.primaryLabel, actions.primaryDisabled, actions.methods.length, actions.autoMerge, actions.draftToggle]).toEqual(['resolve', 'Resolve conflicts', false, 0, '', 'draft']);
    const armed = await header(base({ mergeability: 'conflicting', autoMergeEnabled: true, autoMergeMethod: 'squash' }));
    expect([armed.primary, armed.armedLabel, armed.autoMerge]).toEqual(['resolve', 'Auto-merge (squash and merge)', 'disable']);
  });
  test('a draft: Ready for review in the slot, and not again in the menu', async () => {
    const actions = await header(base({ isDraft: true }));
    expect([actions.primary, actions.primaryLabel, actions.draftToggle, actions.methods.length, actions.mergeNow, actions.autoMerge]).toEqual(['ready', 'Ready for review', '', 0, false, '']);
  });
  test('armed: the info badge in the slot, Disable auto-merge and Merge now in the menu, the armed method selected', async () => {
    const actions = await header(base({ autoMergeEnabled: true, autoMergeMethod: 'squash' }));
    expect([actions.primary, actions.primaryLabel, actions.armedLabel, actions.autoMerge, actions.mergeNow]).toEqual(['auto-merge-armed', 'Auto-merge (squash and merge)', '', 'disable', true]);
    expect(actions.methods.find(item => item.selected)?.method).toBe('squash');
    expect(actions.armedTip).toBe('Auto-merge (squash and merge): the host will merge this on its own once its requirements are met');
  });
  test('merged and closed: the state badge, with Revert changes or Reopen', async () => {
    const merged = await header(base({ state: 'merged' })), closed = await header(base({ state: 'closed' }));
    expect([merged.primary, merged.stateLabel, merged.revertItem, merged.closeItem, merged.open]).toEqual(['merged', 'Merged', true, false, false]);
    expect([closed.primary, closed.stateLabel, closed.reopenItem, closed.revertItem]).toEqual(['closed', 'Closed', true, false]);
  });
  test('the out-of-date base: the sentence and both ways to update, or the news alone', async () => {
    const actions = await header(base({ baseComparison: 'behind', behindBy: 3 }));
    expect([actions.freshness, actions.freshnessSummary, actions.freshnessMethods.map(item => [item.label, item.value])]).toEqual([true, 'This branch is out-of-date with main by 3 commits.',
      [['Update branch', 'update-branch:merge'], ['Update with rebase', 'update-branch:rebase']]]);
    const one = await header(base({ baseComparison: 'behind', behindBy: 1, viewerPermissions: { ...WRITE, updateMethods: [] } }));
    expect([one.freshnessSummary, one.freshnessMethods.length]).toEqual(['This branch is out-of-date with main by 1 commit.', 0]);
  });
  test('workflows awaiting approval: the warning button in the tab bar, unless the checks there are stale', async () => {
    const actions = await header(base({ workflowApprovalsRequired: 2 }));
    expect([actions.approveWorkflows, actions.approveLabel]).toEqual([true, 'Approve workflows to run']);
  });
});

/** gitHubViewerPermissions for the profiles the lane's two accounts cannot hold (pr-profiles-injection.test.ts). */
const PROFILE = {
  reader: { actions: [], comment: true, resolve: false, verdicts: VERDICTS, requestReviewers: false, labels: false },
  'contributor-author': { actions: ['ready', 'draft', 'close', 'reopen'], comment: true, resolve: true, verdicts: ['comment'], requestReviewers: false, labels: false },
};
describe('permissions (the host and the viewer both have to say yes)', () => {
  test('a reader sees the pull request and no action at all', async () => {
    const actions = await header(base({ viewerPermissions: PROFILE.reader, baseComparison: 'behind', behindBy: 2 }));
    expect([actions.primary, actions.draftToggle, actions.mergeNow, actions.autoMerge, actions.methods.length, actions.closeItem, actions.freshnessMethods.length, actions.stack.canRebase]).toEqual(['', '', false, '', 0, false, 0, false]);
    const closed = await header(base({ viewerPermissions: PROFILE.reader, state: 'closed' }));
    expect([closed.primary, closed.reopenItem]).toEqual(['closed', false]);
  });
  test('a read-only author closes, reopens, readies and drafts, and is offered no merge', async () => {
    const actions = await header(base({ viewerPermissions: PROFILE['contributor-author'] }));
    expect([actions.primary, actions.draftToggle, actions.closeItem, actions.methods.length, actions.autoMerge, actions.mergeNow]).toEqual(['', 'draft', true, 0, '', false]);
    expect((await header(base({ viewerPermissions: PROFILE['contributor-author'], isDraft: true }))).primary).toBe('ready');
    expect((await header(base({ viewerPermissions: PROFILE['contributor-author'], state: 'closed' }))).reopenItem).toBe(true);
  });
  test('update-branch only where GitHub says this viewer may update the branch', async () => {
    const withUpdate = await header(base({ baseComparison: 'behind', behindBy: 2 }));
    const without = await header(base({ baseComparison: 'behind', behindBy: 2, viewerPermissions: { ...WRITE, actions: WRITE.actions.filter(action => action !== 'update-branch'), updateMethods: undefined } }));
    expect([withUpdate.freshnessMethods.length, without.freshness, without.freshnessMethods.length]).toEqual([2, true, 0]);
  });
});

describe('the actions, their confirmations and their toasts', () => {
  test('Merge asks first, names the method, sends it, toasts and reads the host again; Cancel sends nothing', async () => {
    const { client, writes, calls } = fakeClient(defaults(base()));
    expect((await panel(client)).actions.primary).toBe('merge');
    prUiLocal(client, 'method', selected, 'squash');
    prUiLocal(client, 'ask', selected, 'merge');
    let actions = (await panel(client)).actions;
    expect([actions.primaryLabel, actions.dialogOpen, actions.dialogTitle, actions.dialogDescription, actions.dialogConfirm, actions.dialogValue])
      .toEqual(['Squash and merge', true, 'Merge pull request?', 'This merges #7 using squash.', 'Squash and merge', 'merge:squash']);
    prUiLocal(client, 'cancel', selected, '');
    expect((await panel(client)).actions.dialogOpen).toBe(false);
    expect(writes()).toEqual([]);
    prUiLocal(client, 'ask', selected, 'merge');
    const detailReads = calls.filter(call => call.method === 'pullRequests.detail').length;
    expect(await prCommand(client, native, 'action', selected, 'merge:squash')).toBe('');
    expect(writes()).toEqual([{ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7, action: 'merge', mergeMethod: 'squash' }]);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Pull request merged' });
    actions = (await panel(client)).actions;
    expect(actions.dialogOpen).toBe(false);
    expect(calls.filter(call => call.method === 'pullRequests.detail').length).toBe(detailReads + 1);
    // The method chosen is this device's last selection from now on (useUiStateStore).
    expect((client.local as { pages?: { mergeMethod?: string } }).pages?.mergeMethod).toBe('squash');
  });
  test('"Merging..." while the host works, every control held; then the control comes back', async () => {
    let release: () => void = () => {};
    const { client } = fakeClient(defaults(base(), { 'pullRequests.runAction': () => new Promise(resolve => { release = () => resolve({}); }) }));
    await panel(client);
    const running = prCommand(client, native, 'action', selected, 'merge:merge');
    await Bun.sleep(5);
    const during = (await panel(client)).actions;
    expect([during.primaryLabel, during.primaryDisabled, during.pending]).toEqual(['Merging...', true, true]);
    release();
    await running;
    expect((await panel(client, { refresh: ++refresh })).actions.pending).toBe(false);
  });
  test('each confirmation says the reference\'s words', async () => {
    const say = async (detail: Obj, action: string) => {
      const { client } = fakeClient(defaults(detail));
      await panel(client);
      prUiLocal(client, 'ask', selected, action);
      const actions = (await panel(client)).actions;
      return [actions.dialogTitle, actions.dialogDescription, actions.dialogConfirm, actions.dialogDestructive];
    };
    expect(await say(base(), 'close')).toEqual(['Close pull request?', 'This closes #7 without merging it.', 'Close', true]);
    expect(await say(base({ state: 'merged' }), 'revert')).toEqual(['Revert these changes?', 'This opens a new pull request that reverses the changes merged by #7.', 'Create revert PR', false]);
    expect(await say(base({ checks: [{ name: 'ci/build', status: 'pending' }] }), 'enable-auto-merge'))
      .toEqual(['Enable auto-merge?', 'This merges #7 using merge as soon as the host considers it ready, which may be immediately.', 'Enable auto-merge', false]);
    expect(await say(base({ workflowApprovalsRequired: 1 }), 'approve-workflows'))
      .toEqual(['Approve workflows to run?', 'This allows 1 workflow from #7 to run. Review the code and workflow changes first.', 'Approve and run', false]);
  });
  test('every action sends the verb table\'s payload and toasts its own success', async () => {
    const cases: [string, Obj, string][] = [
      ['ready', { action: 'ready' }, 'Marked ready for review'], ['draft', { action: 'draft' }, 'Converted to draft'], ['close', { action: 'close' }, 'Pull request closed'],
      ['reopen', { action: 'reopen' }, 'Pull request reopened'], ['update-branch:merge', { action: 'update-branch', updateMethod: 'merge' }, 'Branch updated with the base branch'],
      ['update-branch:rebase', { action: 'update-branch', updateMethod: 'rebase' }, 'Branch updated with the base branch'],
      ['enable-auto-merge:squash', { action: 'enable-auto-merge', mergeMethod: 'squash' }, 'Auto-merge turned on — merges as soon as this is ready, sooner if it already is'],
      ['disable-auto-merge', { action: 'disable-auto-merge' }, 'Auto-merge turned off'], ['revert', { action: 'revert' }, 'Revert pull request opened'],
      ['approve-workflows', { action: 'approve-workflows' }, 'Workflows approved'], ['merge:rebase', { action: 'merge', mergeMethod: 'rebase' }, 'Pull request merged'],
    ];
    for (const [value, payload, title] of cases) {
      const { client, writes, calls } = fakeClient(defaults(base()));
      await panel(client);
      await prCommand(client, native, 'action', selected, value);
      expect(writes()).toEqual([{ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7, ...payload }]);
      expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title });
      // pullRequestActionNeedsHostRefresh: the update and the approval go around the server's cache.
      await panel(client);
      expect(calls.some(call => call.method === 'pullRequests.invalidate')).toBe(value.startsWith('update-branch') || value === 'approve-workflows');
    }
  });
  test('a refusal: "Could not …", the host\'s sentence else the hint (the rebase one for a rebase), the controls back', async () => {
    const refuse = (message: string) => () => { throw new ClientError(`Pull request operation runAction failed: ${message}`, 'PullRequestOperationError'); };
    const conflicting = fakeClient(defaults(base(), { 'pullRequests.runAction': refuse('Pull Request is not mergeable') }));
    await panel(conflicting.client);
    expect(await prCommand(conflicting.client, native, 'action', selected, 'merge:merge')).toBe('Pull request operation runAction failed: Pull Request is not mergeable');
    expect(toasts(conflicting.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not merge this pull request', description: 'Pull Request is not mergeable' });
    expect((await panel(conflicting.client)).actions).toMatchObject({ pending: false, primaryDisabled: false, primaryLabel: 'Merge' });
    const silent = fakeClient(defaults(base(), { 'pullRequests.runAction': refuse('GitHub CLI command failed.') }));
    await panel(silent.client);
    await prCommand(silent.client, native, 'action', selected, 'update-branch:rebase');
    expect(toasts(silent.client).at(-1)).toMatchObject({ title: 'Could not update this branch', description: 'The host refused it. A rebase stops at the first commit that does not apply cleanly; updating with a merge commit may still work.' });
    await prCommand(silent.client, native, 'action', selected, 'update-branch:merge');
    expect(toasts(silent.client).at(-1)?.description).toBe('The host refused it. Check that you have write access to the branch — one from a fork also needs its author to allow edits from maintainers — and that it does not conflict with the base.');
    await prCommand(silent.client, native, 'action', selected, 'enable-auto-merge:merge');
    expect(toasts(silent.client).at(-1)).toMatchObject({ title: 'Could not turn on auto-merge', description: 'The host refused it. Check that this repository allows auto-merge, that you have write access, and that there is something left for it to wait on.' });
  });
  // fix-misc-batch (#298 bug 15): refreshFromHost invalidates once. Each invalidate announces a change, the
  // announcement asks the panel again, and Exact lets the answer that sent it go (its call may already have been
  // sent). Before, the run that replaced it found the host refresh still owed and sent it again, which announced
  // again: 237 invalidates and detail reads in 40 s after one Update with rebase, the toast and the up-to-date
  // header held back until GitHub's reads caught up.
  test('Update with rebase: one invalidate however often its announcement lets the read go, then the fresh header', async () => {
    let behind = true, cut = 0;
    const { client, calls } = fakeClient(defaults(base(), {
      // The invalidate lands; the announcement it made asks the resource again while GitHub is still read, so
      // the detail read after it is let go (the live trace: invalidate 3 ms, then a detail read of ~1 s).
      'pullRequests.detail': () => { if (cut-- > 0) throw new ClientError('The answer was let go before this reply.', 'superseded'); return base(behind ? { baseComparison: 'behind', behindBy: 8 } : {}); },
    }));
    expect((await panel(client)).actions.freshnessSummary).toBe('This branch is out-of-date with main by 8 commits.');
    behind = false; cut = 6;
    await prCommand(client, native, 'action', selected, 'update-branch:rebase');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Branch updated with the base branch' });
    const sent = calls.length;
    let view = null;
    for (let ask = 0; ask < 8 && !view; ask++) view = await panel(client).catch(() => null);
    const after = calls.slice(sent).map(call => call.method);
    expect(after.filter(method => method === 'pullRequests.invalidate')).toHaveLength(1);
    expect(after.filter(method => method === 'pullRequests.detail')).toHaveLength(7);
    expect(view?.actions).toMatchObject({ freshness: false, freshnessSummary: '', pending: false });
  });
});

describe('the list overrides the panel reports ("sent", "done", "failed")', () => {
  // The reader's clock is the newest the snapshot saw (noteNow); the list's answers are read at it.
  const LIST_NOW = Date.parse('2026-10-08T12:00:00Z');
  const entries = [{ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7, title: 'Add a changelog', state: 'open', isDraft: false, author: { login: 'lane-primary' }, updatedAt: '2026-10-08T10:00:00Z', additions: 3, deletions: 0 }];
  const rows = (page: Awaited<ReturnType<typeof pullRequestsPage>>) => page.groups.flatMap(group => group.rows.map(row => `${row.number}:${row.state}`));
  test('a close leaves the open list as it is sent, and comes back when the host refuses', async () => {
    let refuse = true;
    const { client } = fakeClient(defaults(base(), { 'pullRequests.list': () => ({ entries, viewers: {} }), 'pullRequests.listStats': () => ({ stats: [] }),
      'pullRequests.runAction': () => { if (refuse) throw new Error('Resource not accessible by integration'); return {}; } }));
    noteNow(client, LIST_NOW);
    const page = () => pullRequestsPage(client, native, { open: true, refresh: 0, now: LIST_NOW, selected, query: '', typed: false });
    expect(rows(await page())).toEqual(['7:open']);
    await panel(client);
    let seen: string[] = [];
    const running = prCommand(client, native, 'action', selected, 'close');
    seen = rows(await page());
    await running;
    expect(seen).toEqual([]); // "sent": the row left the open list on the press
    expect(rows(await page())).toEqual(['7:open']); // "failed": its note is taken back
    refuse = false;
    await prCommand(client, native, 'action', selected, 'close');
    expect(rows(await page())).toEqual([]); // "done": the note stands until a list answer agrees
  });
  test('a reopen finds the row a close took off the open list, after the list has read it gone (heldPullRequestsBySurface)', async () => {
    let listed = entries;
    const { client } = fakeClient(defaults(base(), { 'pullRequests.list': () => ({ entries: listed, viewers: {} }), 'pullRequests.listStats': () => ({ stats: [] }), 'pullRequests.runAction': () => ({}) }));
    noteNow(client, LIST_NOW);
    const page = (pageRefresh: number) => pullRequestsPage(client, native, { open: true, refresh: pageRefresh, now: LIST_NOW, selected, query: '', typed: false });
    expect(rows(await page(0))).toEqual(['7:open']);
    await panel(client);
    await prCommand(client, native, 'action', selected, 'close');
    listed = [];
    expect(rows(await page(1))).toEqual([]);
    await prCommand(client, native, 'action', selected, 'reopen');
    listed = entries;
    expect(rows(await page(2))).toEqual(['7:open']); // the reopen's note replaced the close's, and the answer agrees
  });
  test('a merge is written on only once the host has done it', async () => {
    let release: () => void = () => {};
    const { client } = fakeClient(defaults(base(), { 'pullRequests.list': () => ({ entries, viewers: {} }), 'pullRequests.listStats': () => ({ stats: [] }),
      'pullRequests.runAction': () => new Promise(resolve => { release = () => resolve({}); }) }));
    noteNow(client, LIST_NOW);
    const page = () => pullRequestsPage(client, native, { open: true, refresh: 0, now: LIST_NOW, selected, query: '', typed: false });
    await page(); await panel(client);
    const running = prCommand(client, native, 'action', selected, 'merge:merge');
    await Bun.sleep(5);
    expect(rows(await page())).toEqual(['7:open']);
    release(); await running;
    expect(rows(await page())).toEqual([]);
  });
});

describe('the host stack (PullRequestStackMenu)', () => {
  const layers = [
    { number: 6, title: 'Let greet take options', headBranch: 'feature/a', headSha: 'aaa111', state: 'open', isDraft: false },
    { number: 7, title: 'Add a changelog', headBranch: 'feature/changelog', headSha: 'bbb222', state: 'open', isDraft: false },
    { number: 8, title: 'Document the options', headBranch: 'docs/b', headSha: 'ccc333', state: 'open', isDraft: false },
  ];
  const stack = { id: 'S_1', number: 120, url: 'https://github.com/lane/sandbox/stacks/120', base: 'main', layers };
  test('layer 2 of 3: the trigger, the layers top-down with this one checked, "↳ main", Merge stack (2), Rebase stack; no single merge', async () => {
    const { client } = fakeClient(defaults(base({ baseBranch: 'feature/a' }), { 'pullRequests.stack': () => stack }));
    const actions = (await panel(client)).actions;
    expect([actions.stack.shown, actions.stack.label, actions.stack.ariaLabel, actions.stack.tooltip]).toEqual([true, '2/3', 'Stack 120, layer 2 of 3', 'View stack #120, layer 2 of 3']);
    expect(actions.stack.layers.map(layer => [layer.number, layer.title, layer.detail, layer.current])).toEqual([
      [8, 'Document the options', '#8 · docs/b · Open', false], [7, 'Add a changelog', '#7 · feature/changelog · Open', true], [6, 'Let greet take options', '#6 · feature/a · Open', false]]);
    expect([actions.stack.base, actions.stack.mergeItemLabel, actions.stack.canMerge, actions.stack.mergeDisabled, actions.stack.canRebase, actions.stack.rebaseDisabled, actions.stack.mergeButton])
      .toEqual(['↳ main', 'Merge stack (2)', true, false, true, false, true]);
    expect(actions.stack.mergeTooltip).toBe('Merge stack through #7 into main (2 pull requests)');
    // allowsSinglePullRequestMerge: a stack exists, so the single Merge is not the primary control.
    expect([actions.primary, actions.autoMerge, actions.stacked, actions.baseTip]).toEqual(['', '', true, 'Stacked on feature/a']);
  });
  test('Merge stack asks, sends the stack number and the heads it saw, and says what GitHub did', async () => {
    const { client, writes } = fakeClient(defaults(base(), { 'pullRequests.stack': () => stack }));
    await panel(client);
    prUiLocal(client, 'stack-ask', selected, 'merge');
    const asked = (await panel(client)).actions.stack;
    expect([asked.dialogOpen, asked.dialogTitle, asked.dialogConfirm, asked.dialogLayers.map(layer => layer.compact)]).toEqual([true, 'Merge 2 pull requests?', 'Merge stack', ['#6 · Open', '#7 · Open']]);
    expect(asked.dialogDescription).toBe('Merge #7 and its unmerged layers below into main using merge. GitHub checks their rules before merging or queueing them and rebases the remaining stack after merging.');
    await prCommand(client, native, 'stack-run', selected, '');
    expect(writes()).toEqual([{ stackNumber: 120, expectedStackHeads: [{ number: 6, headSha: 'aaa111' }, { number: 7, headSha: 'bbb222' }], projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7, action: 'merge', mergeMethod: 'merge' }]);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Stack merge request completed', description: 'GitHub merged the stack or added it to its merge queue.' });
    expect((await panel(client)).actions.stack.dialogOpen).toBe(false);
  });
  test('Rebase stack targets the top with every unmerged head; a stale head fails with the host\'s words', async () => {
    const { client, writes } = fakeClient(defaults(base(), { 'pullRequests.stack': () => stack,
      'pullRequests.runAction': () => { throw new ClientError('The stack changed. Refresh it before trying again.', 'PullRequestOperationError'); } }));
    await panel(client);
    prUiLocal(client, 'stack-ask', selected, 'update-branch');
    expect((await panel(client)).actions.stack.dialogTitle).toBe('Rebase 3 pull requests?');
    expect(await prCommand(client, native, 'stack-run', selected, '')).toBe('The stack changed. Refresh it before trying again.');
    expect(writes()[0]).toMatchObject({ number: 8, action: 'update-branch', updateMethod: 'rebase', stackNumber: 120, expectedStackHeads: [{ number: 6, headSha: 'aaa111' }, { number: 7, headSha: 'bbb222' }, { number: 8, headSha: 'ccc333' }] });
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Stack operation did not complete', description: 'PullRequestOperationError: The stack changed. Refresh it before trying again.' });
  });
  test('a draft layer below holds the merge and says why; a failed lookup offers Retry stack lookup', async () => {
    const { client } = fakeClient(defaults(base(), { 'pullRequests.stack': () => ({ ...stack, layers: [{ ...layers[0]!, isDraft: true }, layers[1]!, layers[2]!] }) }));
    const held = (await panel(client)).actions.stack;
    expect([held.mergeDisabled, held.note]).toEqual([true, 'Every layer being merged must be open and ready for review.']);
    let fail = true;
    const lookup = fakeClient(defaults(base(), { 'pullRequests.stack': () => { if (fail) throw new Error('GitHub API rate limit exceeded'); return stack; } }));
    const failed = (await panel(lookup.client)).actions;
    expect([failed.stack.retryLookup, failed.stack.shown, failed.primary]).toEqual([true, false, '']);
    fail = false;
    prUiLocal(lookup.client, 'stack-retry', selected, '');
    expect((await panel(lookup.client)).actions.stack).toMatchObject({ retryLookup: false, shown: true, label: '2/3' });
  });
});
