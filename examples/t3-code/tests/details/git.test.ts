import { afterEach, describe, expect, test } from 'bun:test';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';
import { appPath } from '../support/app-files';
afterEach(resetPrimary);
import { defaultBranchCopy, featureBranchUnavailableReason, formatElapsed, menuItems, menuNotes, menuReason, progressPresentation, publishReadiness, quickAction, quickActionIcon,
  requiresDefaultBranchConfirmation, sortedPublishProviders, terminology } from '../../src/details/git-logic';
import { GIT_ACTION_KEY, gitActionEvent, gitCardView, gitCommand, gitLocal, gitState, transportActionId } from '../../src/details/git-actions';
import { branchLocal, branchState, cardBranchView, includeRef, originLabel, startFromOrigin } from '../../src/details/git-branch';
import { gitChatLocal } from '../../src/details/git-route';
import { toasts } from '../../src/shell/toast';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { contractComponent } from '../support/contract-action-fixture';
import { adoptHostLocale } from '../../src/shared/timestamp-format';

const status = (extra: Obj = {}): Obj => ({ isRepo: true, hasPrimaryRemote: true, isDefaultRef: false, refName: 'feature', hasWorkingTreeChanges: false,
  workingTree: { files: [], insertions: 0, deletions: 0 }, hasUpstream: true, aheadCount: 0, behindCount: 0, pr: null, ...extra });

describe('quick action (resolveQuickAction)', () => {
  test('changes commit, push and open a PR unless on the default ref or without a remote', () => {
    expect(quickAction(status({ hasWorkingTreeChanges: true }), false, false, true)).toMatchObject({ label: 'Commit, push & PR', action: 'commit_push_pr' });
    expect(quickAction(status({ hasWorkingTreeChanges: true, isDefaultRef: true }), false, true, true)).toMatchObject({ label: 'Commit & push', action: 'commit_push' });
    expect(quickAction(status({ hasWorkingTreeChanges: true, hasUpstream: false }), false, false, false)).toMatchObject({ label: 'Commit', action: 'commit' });
    expect(quickAction(status({ hasUpstream: false }), false, false, false)).toMatchObject({ label: 'Publish repository', kind: 'open_publish' });
    expect(quickAction(status({ behindCount: 2 }), false)).toMatchObject({ label: 'Pull', kind: 'run_pull' });
    expect(quickAction(status({ aheadCount: 1, behindCount: 1 }), false)).toMatchObject({ label: 'Sync ref', disabled: true, hint: 'Branch has diverged from upstream. Rebase/merge first.' });
    expect(quickAction(status(), false)).toMatchObject({ label: 'Commit', disabled: true, hint: 'Branch is up to date. No action needed.' });
    expect(quickAction(status(), true)).toMatchObject({ hint: 'Git action in progress.' });
    expect(quickAction(null, false)).toMatchObject({ hint: 'Git status is unavailable.' });
    expect(quickAction(status({ refName: null }), false).hint).toBe('Create and checkout a ref before pushing or opening a pull request.');
  });
  test('GitLab says MR; the icon follows the action', () => {
    const gitlab = status({ hasWorkingTreeChanges: true, sourceControlProvider: { kind: 'gitlab', name: 'GitLab', baseUrl: '' } });
    expect(terminology(gitlab)).toEqual({ shortLabel: 'MR', shortLabelWithArticle: 'an MR', singular: 'merge request', plural: 'merge requests' });
    // A status with no host reads as GitHub's; a kind no host claims is a "change request".
    expect(terminology(status())).toMatchObject({ shortLabel: 'PR', shortLabelWithArticle: 'a PR' });
    expect(terminology(status({ sourceControlProvider: { kind: 'unknown', name: 'git.example', baseUrl: '' } }))).toEqual({ shortLabel: 'change request', shortLabelWithArticle: 'a change request', singular: 'change request', plural: 'change requests' });
    expect(quickAction(gitlab, false).label).toBe('Commit, push & MR');
    expect(quickActionIcon(quickAction(gitlab, false), gitlab)).toBe('gitlab');
    expect(quickActionIcon(quickAction(status({ hasWorkingTreeChanges: true, isDefaultRef: true }), false, true), null)).toBe('cloud-upload');
    expect(quickActionIcon(quickAction(status(), false), null)).toBe('git-commit');
  });
});

describe('options menu (buildMenuItems, getMenuActionDisabledReason)', () => {
  test('a dirty default ref: Commit enabled, Push and Create PR explain themselves', () => {
    const dirty = status({ hasWorkingTreeChanges: true, isDefaultRef: true, refName: 'main' });
    const items = menuItems(dirty, false, true);
    expect(items.map(item => [item.label, item.disabled])).toEqual([['Commit', false], ['Push', true], ['Create PR', true]]);
    expect(menuReason(items[1]!, dirty, false, true)).toBe('Commit or stash local changes before pushing.');
    // getMenuActionDisabledReason: uncommitted work does not hold Create PR back; nothing ahead of the default branch does.
    expect(menuReason(items[2]!, dirty, false, true)).toBe('No local commits to include in a pull request.');
    const ahead = status({ hasWorkingTreeChanges: true, aheadCount: 0, aheadOfDefaultCount: 2 });
    expect(menuItems(ahead, false, true).map(item => [item.label, item.disabled])).toEqual([['Commit', false], ['Push', true], ['Create PR', false]]);
    expect(menuItems(dirty, false, false).map(item => item.id)).toEqual(['commit']);
    expect(menuItems(status({ pr: { state: 'open' } }), false, true).map(item => item.id)).toEqual(['commit', 'push']);
    expect(menuReason(menuItems(status(), false, true)[0]!, status(), false, true)).toBe('Worktree is clean. Make changes before committing.');
  });
  test('notes: detached HEAD and behind upstream', () => {
    expect(menuNotes(status({ refName: null }), '').map(note => note.id)).toEqual(['detached']);
    expect(menuNotes(status({ behindCount: 3 }), 'boom')).toEqual([{ id: 'behind', text: 'Behind upstream. Pull/rebase first.', tone: 'warning' }, { id: 'error', text: 'boom', tone: 'error' }]);
  });
  test('default-branch confirmation copy', () => {
    expect(requiresDefaultBranchConfirmation('commit', true)).toBe(false);
    expect(requiresDefaultBranchConfirmation('commit_push', true)).toBe(true);
    expect(defaultBranchCopy('commit_push', 'main', true, terminology(null))).toEqual({ title: 'Commit & push to default ref?',
      description: 'This action will commit and push changes on "main". You can continue on this ref or create a feature ref and run the same action there.', continueLabel: 'Commit & push to main' });
    expect(defaultBranchCopy('create_pr', 'main', false, terminology(null)).title).toBe('Push & create PR from default ref?');
  });
  test('progress and elapsed', () => {
    expect(progressPresentation({ running: true, operation: 'run_change_request', label: 'Running source control action', output: '', phaseAt: 1000, hookAt: 0 }))
      .toEqual({ status: 'Starting source control action...', output: '', startedAt: 1000 });
    expect(progressPresentation({ running: true, operation: 'pull', label: '', output: 'x', phaseAt: 5, hookAt: 0 })).toEqual({ status: 'Pulling latest changes...', output: '', startedAt: 5 });
    expect([formatElapsed(1000, 13_500), formatElapsed(0, 5), formatElapsed(0 + 1, 65_001)]).toEqual(['12s', '', '1m 5s']);
  });
  test('publish readiness', () => {
    const discovered = [{ kind: 'github', label: 'GitHub', status: 'available', auth: { status: 'unauthenticated', account: { _tag: 'None' }, detail: { _tag: 'None' } } },
      { kind: 'gitlab', label: 'GitLab', status: 'available', auth: { status: 'authenticated', account: { _tag: 'Some', value: 'me' } } }];
    expect(publishReadiness('github', discovered)).toMatchObject({ ready: false, hint: 'GitHub is not authenticated. Open Settings -> Source control for setup guidance.' });
    expect(publishReadiness('bitbucket', discovered).hint).toBe('Provider status unavailable. Open Settings -> Source control and rescan.');
    expect(sortedPublishProviders(discovered).map(option => option.value)).toEqual(['gitlab', 'azure-devops', 'bitbucket', 'forgejo', 'gitcafe', 'github']);
  });
});

describe('the action stream (vcsAction.ts)', () => {
  const make = () => {
    const calls: Obj[] = [];
    const client = { scopes: ['source-control:write'], ready: true, environmentId: 'env', generation: 3, threadId: 't1', projectId: 'p1', draftKey: 'env:t1', writable: true, local: { composerControls: {} },
      shell: { threads: [], projects: [] }, config: {}, projection: {},
      restAccess: () => ({ ids: async () => ['a1'], call: async (request: Obj) => { calls.push(request); return { id: '3-7' }; },
        request: async (method: string, payload: Obj) => { calls.push({ method, payload }); return method === 'vcs.pull' ? { status: 'pulled', refName: 'main', upstreamRef: 'origin/main' } : {}; } }) } as unknown as T3Client;
    return { client, calls };
  };
  const native = { available: true, watch() {}, later: async () => ({}) } as Native;
  test('a commit streams its phases, then the inline success', async () => {
    const { client, calls } = make();
    gitCardView(client, status({ hasWorkingTreeChanges: true, workingTree: { files: [{ path: 'a.ts', insertions: 2, deletions: 1 }, { path: 'b.ts', insertions: 0, deletions: 0 }] } }), '', '/repo', 100);
    await gitCommand(client, native, 'menu', '/repo', 'commit');
    expect(gitState(client).dialog).toBe('commit');
    await gitLocal(client, native, 'files-edit', '', '');
    await gitLocal(client, native, 'file', 'b.ts', '');
    expect(gitCardView(client, gitState(client).status, '', '/repo', 100)).toMatchObject({ dialog: 'commit', selectedCount: 1, fileCount: 2, someSelected: true, allSelected: false });
    await gitCommand(client, native, 'commit', '/repo', '  Fix it  ');
    const id = transportActionId('env', '/repo', 'a1');
    expect(calls.at(-1)).toEqual({ op: 'subscribe', key: GIT_ACTION_KEY, method: 'git.runStackedAction',
      payload: { actionId: id, cwd: '/repo', action: 'commit', commitMessage: 'Fix it', filePaths: ['a.ts'], threadId: 't1' } });
    gitActionEvent(client, { subscriptionId: '3-7', value: { actionId: id, cwd: '/repo', action: 'commit', kind: 'phase_started', phase: 'commit', label: 'Committing...' } });
    expect(gitCardView(client, gitState(client).status, '', '/repo', 2000)).toMatchObject({ progress: true, progressStatus: 'Committing...', progressElapsed: '0s', menuDisabled: true, dialog: '' });
    expect(gitCardView(client, gitState(client).status, '', '/repo', 5200).progressElapsed).toBe('3s');
    gitActionEvent(client, { subscriptionId: '3-7', value: { actionId: id, cwd: '/repo', action: 'commit', kind: 'action_finished', result: {
      branch: { status: 'skipped_not_requested' }, toast: { title: 'Committed abc1234', description: 'Fix it', cta: { kind: 'none' } } } } });
    gitActionEvent(client, { subscriptionId: '3-7', value: { _streamEnded: true } });
    expect(gitCardView(client, gitState(client).status, '', '/repo', 6000)).toMatchObject({ progress: false, success: true, successTitle: 'Committed abc1234', successDescription: 'Fix it' });
    expect(gitCardView(client, gitState(client).status, '', '/repo', 16_100).success).toBe(false);
  });
  test('a result seen before the card clock moves still shows its success', async () => {
    const { client } = make();
    const wall = 1_791_000_000_000;
    gitCardView(client, status({ hasWorkingTreeChanges: true, workingTree: { files: [{ path: 'a.ts', insertions: 1, deletions: 0 }] } }), '', '/repo', wall);
    await gitCommand(client, native, 'menu', '/repo', 'commit');
    await gitCommand(client, native, 'commit', '/repo', 'Fix it');
    const id = transportActionId('env', '/repo', 'a1');
    gitActionEvent(client, { subscriptionId: '3-7', value: { actionId: id, cwd: '/repo', action: 'commit', kind: 'action_finished', result: {
      branch: { status: 'skipped_not_requested' }, toast: { title: 'Committed abc1234', description: 'Fix it', cta: { kind: 'none' } } } } });
    expect(gitCardView(client, gitState(client).status, '', '/repo', wall).success).toBe(true);
    expect(gitCardView(client, gitState(client).status, '', '/repo', wall + 9_000).success).toBe(true);
    expect(gitCardView(client, gitState(client).status, '', '/repo', wall + 19_100).success).toBe(false);
  });
  test('a default-ref push asks first; a failure toasts "Action failed"', async () => {
    const { client, calls } = make();
    gitCardView(client, status({ hasWorkingTreeChanges: true, isDefaultRef: true, refName: 'main' }), '', '/repo', 100);
    await gitCommand(client, native, 'quick', '/repo', '');
    expect(gitCardView(client, gitState(client).status, '', '/repo', 100)).toMatchObject({ dialog: 'confirm', confirmContinue: 'Commit & push to main' });
    await gitCommand(client, native, 'confirm', '/repo', 'feature');
    const subscribe = calls.at(-1)!;
    expect(subscribe.payload).toMatchObject({ action: 'commit_push', featureBranch: true });
    gitActionEvent(client, { subscriptionId: '3-7', value: { actionId: (subscribe.payload as Obj).actionId as string, cwd: '/repo', action: 'commit_push', kind: 'action_failed', phase: 'push', message: 'rejected' } });
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Action failed', description: "Source control action 'commit_push' failed during push." });
    expect(gitState(client).run).toBeNull();
  });
  test('a clean default ref offers no feature branch it cannot name (GitActionsControl.logic resolveFeatureBranchUnavailableReason)', async () => {
    const { client, calls } = make();
    expect(featureBranchUnavailableReason('main', true)).toBe('');
    gitCardView(client, status({ hasWorkingTreeChanges: false, isDefaultRef: true, refName: 'main', aheadCount: 1 }), '', '/repo', 100);
    await gitCommand(client, native, 'menu', '/repo', 'push');
    const reason = "There are no changes to commit, so a feature branch can't be named. Create a branch yourself or continue on main.";
    expect(gitCardView(client, gitState(client).status, '', '/repo', 100)).toMatchObject({ dialog: 'confirm', confirmContinue: 'Push to main', confirmFeatureReason: reason });
    const sent = calls.length;
    await gitCommand(client, native, 'confirm', '/repo', 'feature');
    expect(calls.length).toBe(sent);
    expect(gitState(client).dialog).toBe('confirm');
    // The option follows the live status while the dialog is open.
    expect(gitCardView(client, status({ hasWorkingTreeChanges: true, isDefaultRef: true, refName: 'main', aheadCount: 1 }), '', '/repo', 100).confirmFeatureReason).toBe('');
    await gitCommand(client, native, 'confirm', '/repo', 'continue');
    expect(calls.at(-1)!.payload).toMatchObject({ action: 'push' });
    expect(calls.at(-1)!.payload).not.toHaveProperty('featureBranch');
  });
  test('a server thread changes its ref only with orchestration:operate (GitActionsControl canChangeThreadBranch)', async () => {
    const { client, calls } = make();
    (client.shell.threads as Obj[]).push({ id: 't1' });
    const changes = status({ hasWorkingTreeChanges: true, isDefaultRef: true, refName: 'main', workingTree: { files: [{ path: 'a.ts', insertions: 1, deletions: 0 }] } });
    expect(gitCardView(client, changes, '', '/repo', 100).canChangeBranch).toBe(false);
    await gitCommand(client, native, 'quick', '/repo', '');
    const sent = calls.length;
    await gitCommand(client, native, 'confirm', '/repo', 'feature');
    expect(calls.length).toBe(sent);
    expect(gitState(client).dialog).toBe('confirm');
    await gitLocal(client, native, 'dialog-close', '', '');
    await gitCommand(client, native, 'menu', '/repo', 'commit');
    await gitCommand(client, native, 'commit-branch', '/repo', 'Fix it');
    expect(calls.length).toBe(sent);
    (client as unknown as { scopes: string[] }).scopes = ['source-control:write', 'orchestration:operate'];
    expect(gitCardView(client, changes, '', '/repo', 100).canChangeBranch).toBe(true);
    await gitCommand(client, native, 'commit-branch', '/repo', 'Fix it');
    expect(calls.at(-1)!.payload).toMatchObject({ action: 'commit', featureBranch: true });
  });
  test('a closed confirmation keeps its copy while it fades out (ui/dialog.tsx RetainedWhileClosed)', async () => {
    const { client } = make();
    const ahead = status({ hasWorkingTreeChanges: false, isDefaultRef: true, refName: 'main', aheadCount: 1 });
    gitCardView(client, ahead, '', '/repo', 100);
    await gitCommand(client, native, 'menu', '/repo', 'push');
    await gitLocal(client, native, 'dialog-close', '', '');
    expect(gitCardView(client, ahead, '', '/repo', 100)).toMatchObject({ dialog: '', confirmTitle: 'Push to default ref?', confirmContinue: 'Push to main' });
  });
  test('pull reports inline', async () => {
    const { client } = make();
    gitCardView(client, status({ behindCount: 1 }), '', '/repo', 100);
    await gitCommand(client, native, 'quick', '/repo', '');
    expect(gitCardView(client, gitState(client).status, '', '/repo', 200)).toMatchObject({ success: true, successTitle: 'Pulled', successDescription: 'Updated main from origin/main' });
  });
  test('a change request the action created leaves "Created PR" with no second line; on GitLab "Created MR"', async () => {
    for (const [extra, title] of [[{}, 'Created PR'], [{ sourceControlProvider: { kind: 'gitlab', name: 'GitLab', baseUrl: 'https://gitlab.com' } }, 'Created MR']] as const) {
      const { client, calls } = make();
      gitCardView(client, status({ aheadOfDefaultCount: 1, ...extra }), '', '/repo', 100);
      await gitCommand(client, native, 'quick', '/repo', '');
      const subscribe = calls.at(-1)!;
      expect(subscribe.payload).toMatchObject({ action: 'create_pr' });
      gitActionEvent(client, { subscriptionId: '3-7', value: { actionId: (subscribe.payload as Obj).actionId as string, cwd: '/repo', action: 'create_pr', kind: 'action_finished', result: {
        pr: { status: 'created', number: 5, url: 'https://github.com/acme/app/pull/5' }, toast: { title: 'Created PR #5', description: 'Add the thing', cta: { kind: 'none' } } } } });
      expect(gitCardView(client, gitState(client).status, '', '/repo', 200)).toMatchObject({ success: true, successTitle: title, successDescription: '' });
    }
  });
});

describe('the card\'s Git rows (GitActionsControl panel)', () => {
  const { client } = (() => ({ client: { scopes: [], ready: true, environmentId: 'env', generation: 1, threadId: '', projectId: 'p1', draftKey: 'env:new:p1', local: { composerControls: {} },
    shell: { threads: [], projects: [] }, config: {}, projection: {}, restAccess: () => ({}) } as unknown as T3Client }))();
  test('a draft or a run-less thread with nothing to commit, push or diff shows neither the Git action nor Changes (quietEmptyDraft)', () => {
    expect(gitCardView(client, status(), '', '/repo', 100, '', true).quiet).toBe(true);
    expect(gitCardView(client, status(), '', '/repo', 100, '', false).quiet).toBe(false);
    expect(gitCardView(client, status({ aheadCount: 1 }), '', '/repo', 100, '', true).quiet).toBe(false);
    expect(gitCardView(client, status({ branchChanges: { insertions: 0, deletions: 2 } }), '', '/repo', 100, '', true).quiet).toBe(false);
    expect(gitCardView(client, status(), 'fatal: bad object', '/repo', 100, '', true).quiet).toBe(false);
    const rows = contractComponent('git.contract', 'GitRows');
    const scope = (quiet: boolean) => ({ compact: false, KmItem: (item: Obj) => item, details: { items: { branch: true, gitActions: true, changes: true }, git: { repo: true, quiet, progress: false, success: false, menu: [], branch: { show: false } } } });
    expect([rows.drawn('testId="details-git-group"', rows.state(), scope(true)), rows.drawn('GitChanges(details=details', rows.state(), scope(true))]).toEqual([false, false]);
    expect([rows.drawn('testId="details-git-group"', rows.state(), scope(false)), rows.drawn('GitChanges(details=details', rows.state(), scope(false))]).toEqual([true, true]);
  });
  test('Changes counts the branch\'s lines where the server reports them, grouped as the reference\'s runtime locale writes them (formatDiffCount)', () => {
    // The packaged reference ships only en-US (scripts/build-desktop-artifact.ts DESKTOP_ELECTRON_LANGUAGES), so its default
    // locale is en-US on every Mac: a German region (the host's format locale) still reads "1,234".
    adoptHostLocale('de-DE');
    try {
      expect(gitCardView(client, status({ workingTree: { files: [], insertions: 1234, deletions: 0 } }), '', '/repo', 100)).toMatchObject({ changesAdd: '+1,234', changesDel: '-0' });
      expect(gitCardView(client, status({ workingTree: { files: [], insertions: 9, deletions: 9 }, branchChanges: { insertions: 12_500, deletions: 3 } }), '', '/repo', 100))
        .toMatchObject({ changesAdd: '+12,500', changesDel: '-3' });
    } finally { adoptHostLocale(null); }
  });
  test('a split pair keeps its fill while its menu is open (data-popup-open): the branch picker, the options and Run on', () => {
    const rows = contractComponent('git.contract', 'GitRows'), state = rows.state(), fill = 'light-dark(#0000000e, #ffffff13)', clear = '#00000000';
    const scope = { compact: false, inline: true, rem: 16, KmItem: (item: Obj) => item, details: { cwd: '/repo', git: { repo: true, quiet: false, progress: false, success: false, menu: [], branch: { show: true, label: 'main', labelHead: '', labelTail: '' } } } };
    const branchFill = () => rows.attribute('testId="details-branch" ', 'background-color', state, scope);
    const groupFill = () => rows.attribute('testId="details-git-group"', 'background-color', state, scope);
    expect([branchFill(), groupFill()]).toEqual([clear, clear]);
    rows.call('branchToggled', state, { local: () => undefined }, [{ newState: 'open' }]);
    rows.call('gitMenuToggled', state, {}, [{ newState: 'open' }]);
    expect([state.branchOpen, state.gitMenuOpen, branchFill(), groupFill()]).toEqual([true, true, fill, fill]);
    rows.call('branchToggled', state, {}, [{ newState: 'closed' }]);
    expect([branchFill(), groupFill()]).toEqual([clear, fill]);
    const runOn = contractComponent('git.contract', 'RunOnRow'), on = runOn.state();
    const runOnFill = () => runOn.attribute('testId="details-environment"', 'background-color', on, { inline: true, details: { git: { envPick: true } } });
    expect(runOnFill()).toBe(clear);
    runOn.call('toggled', on, {}, [{ newState: 'open' }]);
    expect([on.open, runOnFill()]).toEqual([true, fill]);
  });
  test('the branch label\'s title is its whole name, cut or not (MiddleTruncate showTitle)', () => {
    const rows = contractComponent('git.contract', 'GitRows'), state = rows.state();
    const scope = (branch: Obj) => ({ compact: false, inline: true, rem: 16, KmItem: (item: Obj) => item, details: { cwd: '/repo', git: { repo: true, quiet: false, menu: [], branch: { show: true, ...branch } } } });
    const long = { label: 't3/clone-repo-to-other-machine', labelHead: 't3/clone-repo-to-other-', labelTail: 'machine' };
    expect(rows.attribute('row flex=1 min-width=0 align-items="center" title=', 'title', state, scope(long))).toBe('t3/clone-repo-to-other-machine');
    expect(rows.attribute('text details.git.branch.label title=', 'title', state, scope({ label: 'main', labelHead: '', labelTail: '' }))).toBe('main');
  });
});

describe('branch picker (BranchToolbarBranchSelector)', () => {
  test('filtering matches sanitized names; opening starts a fresh search', () => {
    expect(includeRef('feature/new-branch', 'new branch')).toBe(true);
    expect(includeRef('main', 'MA')).toBe(true);
    expect(includeRef('main', 'dev')).toBe(false);
    const client = { local: {}, threadId: '', draftKey: 'env:new:p1', projectId: 'p1', config: { settings: {} } } as unknown as T3Client;
    branchLocal(client, 'query', '', 'feat');
    branchLocal(client, 'open', 'branch', '');
    expect(branchState(client)).toMatchObject({ open: 'branch', query: '' });
  });
  test('the branch popover showing refreshes the workspace\'s local status, unawaited (handleOpenChange)', async () => {
    const asked: [string, Obj][] = [];
    const client = { local: { composerControls: {} }, threadId: 't', draftKey: 'env:new:p1', ready: true, generation: 1, config: { settings: {} }, projection: { thread: {} }, shell: { projects: [], threads: [] },
      restAccess: () => ({ read: async () => ({ refs: [{ name: 'main', isDefault: true, current: true }], totalCount: 1, nextCursor: null }),
        request: async (method: string, input: Obj) => { asked.push([method, input]); return new Promise(() => {}); } }) } as unknown as T3Client;
    const native = { available: true } as unknown as Native;
    await gitChatLocal(client, native, 'opened', '', '');
    expect(asked).toEqual([]); // no row drawn yet: no workspace to refresh
    // The card draws the branch row for its workspace (BranchToolbarBranchSelector's branchCwd), then the popover shows.
    await cardBranchView(client, native, '/repo/worktree', '/repo', true);
    expect(await gitChatLocal(client, native, 'opened', '', '')).toBe('');
    expect(asked).toEqual([['vcs.refreshStatus', { cwd: '/repo/worktree', localOnly: true }]]);
    // A workspace that is no repository draws no row: nothing to refresh.
    await cardBranchView(client, native, '/elsewhere', '/elsewhere', false);
    await gitChatLocal(client, native, 'opened', '', '');
    expect(asked).toHaveLength(1);
    // GitBranchPicker's popover reports its showing; closing asks nothing.
    const rows = contractComponent('git.contract', 'GitRows'), state = rows.state(), sent: unknown[][] = [];
    const scope = { local: (...args: unknown[]) => sent.push(args) };
    rows.call('branchToggled', state, scope, [{ newState: 'open' }]);
    rows.call('branchToggled', state, scope, [{ newState: 'closed' }]);
    expect(sent).toEqual([['git-opened', '', '']]);
  });
  test('a new worktree starts from remote by default, and the label says so', () => {
    const client = { local: { composerControls: { contexts: { 'env:new:p1': { envMode: 'worktree', branch: '', worktreePath: '' } } } }, threadId: '', draftKey: 'env:new:p1', projectId: 'p1',
      config: { settings: {} }, shell: { projects: [], threads: [] } } as unknown as T3Client;
    expect(startFromOrigin(client)).toBe(true);
    branchState(client).refs = { cwd: '/repo', query: '', refs: [{ name: 'main', isDefault: true, current: true }], total: 1, nextCursor: null, loaded: true, generation: 0, stale: false };
    expect(originLabel(client, 'main')).toBe('main (remote)');
    branchLocal(client, 'origin', '', '');
    expect(startFromOrigin(client)).toBe(false);
    expect(originLabel(client, 'main')).toBe('main');
    const off = { ...client, config: { settings: { newWorktreesStartFromOrigin: false } } } as unknown as T3Client;
    expect(startFromOrigin(off)).toBe(false);
  });
});

describe('Run on (BranchToolbarEnvironmentSelector, logicalProjectEnvironments)', () => {
  const { logicalProjectKey, environmentOptions, runOnEnvironment, stripRunOn } = require('../../src/details/git-env') as typeof import('../../src/details/git-env');
  const { EnvironmentFleet } = require('../../src/connections/settings-b-fleet') as typeof import('../../src/connections/settings-b-fleet');
  const identity = { canonicalKey: 'git.example.invalid/acme/shared', rootPath: '/repos/shared' };
  test('projects group by repository identity; a path without one stays on its machine', () => {
    expect(logicalProjectKey({ workspaceRoot: '/repos/shared/', repositoryIdentity: identity }, 'a', 'repository')).toBe('git.example.invalid/acme/shared');
    expect(logicalProjectKey({ workspaceRoot: '/repos/shared/app', repositoryIdentity: identity }, 'a', 'repository_path')).toBe('git.example.invalid/acme/shared::app');
    expect(logicalProjectKey({ workspaceRoot: '/repos/shared', repositoryIdentity: identity }, 'a', 'separate')).toBe('a:/repos/shared');
    expect(logicalProjectKey({ workspaceRoot: '/repos/solo' }, 'a', 'repository')).toBe('a:/repos/solo');
  });
  const make = () => {
    primaryAt('http://127.0.0.1:1', 'a'); // this Mac's embedded server is environment a
    const source = new EnvironmentFleet();
    source.entries.set('https://box.example.invalid\nb', { key: 'https://box.example.invalid\nb', origin: 'https://box.example.invalid', environmentId: 'b', phase: 'connected', message: '', traceId: '',
      generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: { environment: { label: 'Build box', platform: { machine: 'cloud' } } },
      shell: { projects: [{ id: 'pb', workspaceRoot: '/repos/shared', repositoryIdentity: identity }], threads: [], sequence: 0 }, scopes: [], error: '', requested: true });
    const calls: Obj[] = [];
    const client = { environmentId: 'a', origin: 'http://127.0.0.1:1', projectId: 'pa', threadId: '', draftKey: 'a:new:pa', config: { environment: { label: 'Local' } },
      shell: { projects: [{ id: 'pa', workspaceRoot: '/repos/shared', repositoryIdentity: identity }], threads: [] },
      local: { groupingMode: 'repository', groupingOverrides: {}, drafts: { 'a:new:pa': 'carry me' }, selections: {}, composerControls: { contexts: { 'a:new:pa': { envMode: 'worktree', branch: 'main', worktreePath: '' } } } } } as unknown as T3Client;
    const native = { available: true, watch() {}, later: async (request: unknown) => { calls.push(request as Obj); return { ok: true, value: { state: 'connecting', origin: 'http://127.0.0.1:2', environmentId: 'b', message: '' }, generation: 4 }; } } as Native;
    return { source, client, native, calls };
  };
  test('a draft lists both machines, this one selected, and moves with its text and workspace choice', async () => {
    const { source, client, native, calls } = make();
    expect(environmentOptions(client, source).map(option => [option.id, option.label, option.machine, option.selected])).toEqual([['a', 'This device', 'server', true], ['b', 'Build box', 'cloud', false]]);
    const moved = await runOnEnvironment(client, native, 'b', source);
    expect(moved.generation).toBe(4);
    expect(client.local.drafts).toEqual({ 'b:new:pb': 'carry me' });
    expect((client.local.composerControls as { contexts: Record<string, Obj> }).contexts['b:new:pb']).toMatchObject({ envMode: 'worktree', branch: 'main' });
    expect(client.local.selections.b).toEqual({ projectId: 'pb', threadId: '' });
    expect(calls.map(call => call.op)).toEqual(['fleetStop', 'connect']);
    expect(source.entries.size).toBe(0);
  });
  test('a started thread keeps its machine; the strip shows the machine only when there is a choice', async () => {
    const { source, client, native } = make();
    expect(stripRunOn(client).envShow).toBe(false); // the module fleet is empty here: nothing else to run on
    const thread = { ...client, threadId: 't1' } as unknown as T3Client;
    await expect(runOnEnvironment(thread, native, 'b', source)).rejects.toThrow('A started thread keeps its environment.');
  });
});

describe('Git branch Return dismissal', () => {
  test('a disabled Git menu row still shows its reason without requesting focus', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const row = contractComponent('git.contract', 'GitMenuRow'), state = row.state(), focused: string[] = [];
    const scope = { item: { id: 'push', disabled: true }, keysId: 'details-git-options-keys', focus: (id: string) => focused.push(id) };
    row.call('hover', state, scope, [true]);
    expect(state.over).toBe(true);
    expect(focused).toEqual([]);
    row.call('hover', state, { ...scope, item: { id: 'push', disabled: false } }, [true]);
    expect(focused).toEqual(['details-git-menu-push']);
  });

  test('accepted Return hides the actual popup, returns focus and dispatches once; an empty command does nothing', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const picker = contractComponent('git.contract', 'GitBranchPicker');
    for (const enterOp of ['shell:git-branch', 'shell:git-branch-create', 'chatlocal:git-pr-open', '']) {
      const calls: unknown[][] = [], state = picker.state();
      const scope = { git: { branch: { enterOp, enterValue: 'feature/audit' } },
        hidePopover: (id: string) => calls.push(['hide', id]), focus: (id: string) => calls.push(['focus', id]),
        act: (...args: unknown[]) => calls.push(['act', ...args]) };
      picker.call('acceptedReturn', state, scope, []);
      expect(calls).toEqual(enterOp ? [['hide', 'details-branch-menu'], ['focus', 'details-branch'], ['act', enterOp, 'enter', 'feature/audit']] : []);
      expect(picker.drawn('testId="details-branch-picker"', state, scope)).toBe(true);
    }
    const source = await Bun.file(appPath('git.contract')).text();
    expect(source).toContain('input id="details-branch-search" value=git.branch.query input=local("git-query", "") submit=acceptedReturn');
    expect(source).toContain('data-dialog-return="menu" focusin=dialogMenuEntered("details-branch-menu", "details-branch")');
    expect(source).toContain('data-dialog-return="menu" focusin=dialogMenuEntered("details-git-options", "details-git-menu")');
    expect(source).not.toContain('if not git.branch.picked');
  });
});
