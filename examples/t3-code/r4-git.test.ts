import { afterEach, describe, expect, test } from 'bun:test';
import { primaryAt, resetPrimary } from './local-primary-fixture';
afterEach(resetPrimary);
import { defaultBranchCopy, formatElapsed, menuItems, menuNotes, menuReason, progressPresentation, publishReadiness, quickAction, quickActionIcon,
  requiresDefaultBranchConfirmation, sortedPublishProviders, terminology } from './r4-git-logic';
import { GIT_ACTION_KEY, gitActionEvent, gitCardView, gitCommand, gitLocal, gitState, transportActionId } from './r4-git-actions';
import { branchLocal, branchState, includeRef, originLabel, startFromOrigin } from './r4-git-branch';
import { toasts } from './toast';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';

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
    expect(terminology(gitlab)).toEqual({ shortLabel: 'MR', singular: 'merge request' });
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
    expect(menuReason(items[2]!, dirty, false, true)).toBe('Commit local changes before creating a pull request.');
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
    expect(publishReadiness('github', discovered)).toMatchObject({ ready: false, hint: 'GitHub is not authenticated. Open Settings -> Source Control for setup guidance.' });
    expect(publishReadiness('bitbucket', discovered).hint).toBe('Provider status unavailable. Open Settings -> Source Control and rescan.');
    expect(sortedPublishProviders(discovered).map(option => option.value)).toEqual(['gitlab', 'azure-devops', 'bitbucket', 'forgejo', 'github']);
  });
});

describe('the action stream (vcsAction.ts)', () => {
  const make = () => {
    const calls: Obj[] = [];
    const client = { environmentId: 'env', generation: 3, threadId: 't1', projectId: 'p1', draftKey: 'env:t1', writable: true, local: { composerControls: {} },
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
  test('pull reports inline', async () => {
    const { client } = make();
    gitCardView(client, status({ behindCount: 1 }), '', '/repo', 100);
    await gitCommand(client, native, 'quick', '/repo', '');
    expect(gitCardView(client, gitState(client).status, '', '/repo', 200)).toMatchObject({ success: true, successTitle: 'Pulled', successDescription: 'Updated main from origin/main' });
  });
});

describe('branch picker (BranchToolbarBranchSelector)', () => {
  test('filtering matches sanitized names; opening starts a fresh search', () => {
    expect(includeRef('feature/new-branch', 'new branch')).toBe(true);
    expect(includeRef('main', 'MA')).toBe(true);
    expect(includeRef('main', 'dev')).toBe(false);
    const client = { local: {}, threadId: '', draftKey: 'env:new:p1', projectId: 'p1', config: { settings: {} } } as unknown as T3Client;
    branchLocal(client, 'query', '', 'feat');
    branchState(client).picked = true;
    branchLocal(client, 'open', 'branch', '');
    expect(branchState(client)).toMatchObject({ open: 'branch', query: '', picked: false });
  });
  test('a new worktree starts from origin by default, and the label says so', () => {
    const client = { local: { composerControls: { contexts: { 'env:new:p1': { envMode: 'worktree', branch: '', worktreePath: '' } } } }, threadId: '', draftKey: 'env:new:p1', projectId: 'p1',
      config: { settings: {} } } as unknown as T3Client;
    expect(startFromOrigin(client)).toBe(true);
    branchState(client).refs = { cwd: '/repo', query: '', refs: [{ name: 'main', isDefault: true, current: true }], total: 1, nextCursor: null, loaded: true, generation: 0, stale: false };
    expect(originLabel(client, 'main')).toBe('origin/main');
    branchLocal(client, 'origin', '', '');
    expect(startFromOrigin(client)).toBe(false);
    expect(originLabel(client, 'main')).toBe('main');
    const off = { ...client, config: { settings: { newWorktreesStartFromOrigin: false } } } as unknown as T3Client;
    expect(startFromOrigin(off)).toBe(false);
  });
});

describe('Run on (BranchToolbarEnvironmentSelector, logicalProjectEnvironments)', () => {
  const { logicalProjectKey, environmentOptions, runOnEnvironment, stripRunOn } = require('./r4-git-env') as typeof import('./r4-git-env');
  const { EnvironmentFleet } = require('./settings-b-fleet') as typeof import('./settings-b-fleet');
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
