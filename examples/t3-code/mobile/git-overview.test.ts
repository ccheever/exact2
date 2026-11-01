import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { gitActionEvent, gitState } from './shared/r4-git-actions';
import { vcsStatusEvent } from './shared/shell-vcs';
import { mobileGitAction, mobileGitRead, mobileGitSnapshot, mobileGitEvents } from './git-overview';
import { mobileGitBranchAction, mobileGitBranchesRead, mobileGitBranchesSnapshot } from './git-branches';
import { mobileFeatureBranch, mobileAutoBranch } from './git-overview-model';
import { mobileGitFeedbackSnapshot, mobileGitFeedbackMetadata, mobileGitFeedbackAction, mobileGitFeedbackError } from './git-feedback';
function fixture() {
  const client = new T3Client(); Object.assign(client, { origin: 'https://example.test', environmentId: 'e', projectId: 'p', threadId: 't',
    connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 9 });
  client.shell.projects = [{ id: 'p', title: 'Repo', workspaceRoot: '/repo' }];
  client.shell.threads = [{ id: 't', projectId: 'p', branch: 'main', worktreePath: null }];
  client.config = { environment: { capabilities: { threadPullRequests: true } } };
  const status: Obj = { isRepo: true, refName: 'main', isDefaultRef: true, hasWorkingTreeChanges: false,
    hasUpstream: true, hasPrimaryRemote: true, aheadCount: 2, behindCount: 0, pr: null, workingTree: { files: [] } };
  const refs: Obj[] = [{ name: 'main', isRemote: false, isDefault: true, current: true, worktreePath: null },
    { name: 'other', isRemote: false, worktreePath: '/occupied' }, { name: 'origin/remote', isRemote: true, worktreePath: null }];
  const calls: Obj[] = []; let grants = ['orchestration:read', 'source-control:write', 'orchestration:operate'], count = 0;
  let hook: ((request: Obj) => unknown) | undefined;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const custom = hook?.(request); if (custom !== undefined) return await custom;
    const value = request.path === '/api/auth/session' ? { authenticated: true, permissions: grants }
      : request.method === 'vcs.refreshStatus' ? status : request.method === 'vcs.listRefs' ? { refs }
      : request.method === 'vcs.createRef' ? { refName: obj(request.payload).refName }
      : request.method === 'vcs.createWorktree' ? { worktree: { refName: obj(request.payload).newRefName, path: '/worktrees/new' } }
      : request.method === 'vcs.switchRef' ? { refName: obj(request.payload).refName }
      : request.op === 'subscribe' ? { id: `subscription-${++count}` }
      : request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `id-${++count}`)
      : request.op === 'mobileOpenURL' ? { opened: true } : {};
    return { ok: true, generation: 9, value };
  } };
  const owner = () => mobileGitSnapshot(10, client).owner;
  return { client, status, refs, calls, native, owner, grant(value: string[]) { grants = value; }, hook(value: typeof hook) { hook = value; } };
}
const read = (f: ReturnType<typeof fixture>) => mobileGitRead(f.owner(), 10, f.native, f.client);
const action = (f: ReturnType<typeof fixture>, op: string, id = '', value = '') => mobileGitAction(f.owner(), op, id, value, 10, f.native, f.client);

async function feedbackRun() {
  const f = fixture(); f.status.isDefaultRef = false; await read(f); await action(f, 'select', 'push');
  const run = gitState(f.client).run!;
  const event = (value: Obj, seq = 1) => ({ key: 'r4-git-action', generation: 9, subscriptionId: run.subscriptionId,
    seq, value: { actionId: run.transportId, cwd: '/repo', ...value } });
  return { ...f, run, event };
}
test('mobile Git feedback reads actual shared phase/output and never completes from CTA metadata alone', async () => {
  const f = await feedbackRun();
  const phase = f.event({ kind: 'phase_started', label: 'Pushing branch' }); gitActionEvent(f.client, phase);
  mobileGitSnapshot(1000, f.client);
  expect(mobileGitFeedbackSnapshot(1000, f.client)).toMatchObject({ phase: 'running', label: 'Pushing branch', description: '', visible: true });
  expect(mobileGitFeedbackSnapshot(3000, f.client).description).toBe('Running for 2s');
  gitActionEvent(f.client, f.event({ kind: 'hook_output', text: 'older line\nlatest line\n' }, 2));
  expect(mobileGitFeedbackSnapshot(3100, f.client).description).toBe('latest line');
  const terminal = f.event({ kind: 'action_finished', result: { toast: { title: 'Pushed', description: 'main', cta: { kind: 'open_pr', url: 'https://github.test/a/b/pull/8' } } } }, 3);
  mobileGitFeedbackMetadata([terminal], 2, f.client);
  expect(mobileGitFeedbackSnapshot(3200, f.client)).toMatchObject({ phase: 'running', prUrl: '' });
  gitActionEvent(f.client, terminal);
  expect(mobileGitFeedbackSnapshot(10000, f.client)).toMatchObject({ phase: 'success', label: 'Pushed', description: 'main', prUrl: 'https://github.test/a/b/pull/8', deadline: 15000 });
});
test('Git results last five fresh-clock seconds, fade for 150ms and cannot be revived by retained shared success', async () => {
  const f = await feedbackRun(); gitActionEvent(f.client, f.event({ kind: 'action_finished', result: { toast: { title: 'Pushed' } } }));
  const shown = mobileGitFeedbackSnapshot(20000, f.client);
  expect(shown.deadline).toBe(25000); expect(mobileGitFeedbackSnapshot(24999, f.client).visible).toBe(true);
  expect(mobileGitFeedbackSnapshot(25000, f.client)).toMatchObject({ visible: false, deadline: 25150 });
  const before = f.calls.length; await mobileGitFeedbackAction(shown.owner, shown.id, 'press', 25001, f.native, f.client);
  expect(f.calls).toHaveLength(before);
  expect(mobileGitFeedbackSnapshot(25150, f.client)).toMatchObject({ phase: 'idle', id: '' });
  expect(gitState(f.client).success?.title).toBe('Pushed'); expect(mobileGitFeedbackSnapshot(26000, f.client).phase).toBe('idle');
});
test('Git feedback presses dismiss results, ignore running and stale identities, and revalidate the completed PR URL', async () => {
  const f = await feedbackRun(); const running = mobileGitFeedbackSnapshot(11, f.client), before = f.calls.length;
  await mobileGitFeedbackAction(running.owner, running.id, 'press', 12, f.native, f.client);
  expect(mobileGitFeedbackSnapshot(12, f.client).phase).toBe('running'); expect(f.calls).toHaveLength(before);
  const terminal = f.event({ kind: 'action_finished', result: { toast: { title: 'Created PR #8', cta: { kind: 'open_pr', url: 'https://github.test/a/b/pull/8' } } } });
  mobileGitFeedbackMetadata([terminal], 0, f.client); gitActionEvent(f.client, terminal);
  const success = mobileGitFeedbackSnapshot(20, f.client);
  expect(await mobileGitFeedbackAction(success.owner, success.id, 'press', 21, f.native, f.client)).toEqual({ opened: true });
  expect(f.calls.at(-1)).toEqual({ op: 'mobileOpenURL', url: 'https://github.test/a/b/pull/8' });
  expect(mobileGitFeedbackSnapshot(22, f.client).visible).toBe(true);
  await mobileGitFeedbackAction(success.owner, success.id, 'dismiss', 23, f.native, f.client);
  expect(mobileGitFeedbackSnapshot(23, f.client).visible).toBe(false);
  mobileGitFeedbackMetadata([terminal], 0, f.client); gitActionEvent(f.client, terminal);
  expect(mobileGitFeedbackSnapshot(200, f.client).phase).toBe('idle');
  mobileGitFeedbackError(f.client, 200, 'second result'); const replacement = mobileGitFeedbackSnapshot(200, f.client);
  await mobileGitFeedbackAction(success.owner, success.id, 'dismiss', 201, f.native, f.client);
  expect(mobileGitFeedbackSnapshot(201, f.client).id).toBe(replacement.id);
  f.client.generation++; const calls = f.calls.length;
  await mobileGitFeedbackAction(replacement.owner, replacement.id, 'press', 202, f.native, f.client);
  expect(mobileGitFeedbackSnapshot(202, f.client).phase).toBe('idle'); expect(f.calls).toHaveLength(calls);
});
test('Git CTA metadata rejects stale batches, wrong owners, mismatched folded results and credential-bearing URLs', async () => {
  for (const bad of ['old', 'generation', 'workspace', 'action', 'subscription', 'title', 'url']) {
    const f = await feedbackRun();
    const terminal = f.event({ kind: 'action_finished', result: { toast: { title: 'Created PR', cta: { kind: 'open_pr', url: bad === 'url' ? 'https://user:secret@github.test/a/b/pull/8' : 'https://github.test/a/b/pull/8' } } } });
    const metadata = { ...terminal, value: { ...terminal.value } };
    if (bad === 'generation') metadata.generation = 8;
    if (bad === 'workspace') metadata.value.cwd = '/other';
    if (bad === 'action') metadata.value.actionId = 'other';
    if (bad === 'subscription') metadata.subscriptionId = 'other';
    if (bad === 'title') metadata.value.result = { toast: { title: 'Forged result', cta: { kind: 'open_pr', url: 'https://github.test/a/b/pull/8' } } };
    mobileGitFeedbackMetadata([metadata], bad === 'old' ? 1 : 0, f.client); gitActionEvent(f.client, terminal);
    const result = mobileGitFeedbackSnapshot(20, f.client); expect(result.phase).toBe('success'); expect(result.prUrl).toBe('');
    const before = f.calls.length; await mobileGitFeedbackAction(result.owner, result.id, 'press', 21, f.native, f.client);
    expect(f.calls).toHaveLength(before); expect(mobileGitFeedbackSnapshot(21, f.client).visible).toBe(false);
  }
});
test('shared asynchronous failures become one mobile error; new thread selection preserves the global connection result', async () => {
  const f = await feedbackRun(); gitActionEvent(f.client, f.event({ kind: 'action_failed', action: 'push', phase: 'push' }));
  const error = mobileGitFeedbackSnapshot(20, f.client);
  expect(error).toMatchObject({ phase: 'error', label: 'Git action failed', description: "Source control action 'push' failed during push.", deadline: 5020 });
  f.client.threadId = 'other'; f.client.threadEpoch++;
  expect(mobileGitFeedbackSnapshot(21, f.client).id).toBe(error.id);
  expect(mobileGitFeedbackSnapshot(22, f.client, false).id).toBe('');
  expect(mobileGitFeedbackSnapshot(23, f.client).id).toBe(error.id);
  f.client.environmentId = 'other'; expect(mobileGitFeedbackSnapshot(24, f.client).phase).toBe('idle');
});
test('a real mobile pull presents progress while awaiting RPC and uses the shared success; request refusal shows an error', async () => {
  const f = fixture(); f.status.behindCount = 1; await read(f);
  let release!: (value: unknown) => void, started!: () => void;
  const gate = new Promise(resolve => release = resolve), begun = new Promise<void>(resolve => started = resolve);
  f.hook(request => { if (request.method === 'vcs.pull') { started(); return gate; } });
  const pending = action(f, 'select', 'pull'); await begun;
  expect(mobileGitFeedbackSnapshot(20, f.client)).toMatchObject({ phase: 'running', label: 'Pulling latest changes' });
  release({ ok: true, generation: 9, value: { status: 'pulled', refName: 'main' } }); await pending;
  expect(mobileGitFeedbackSnapshot(30, f.client)).toMatchObject({ phase: 'success', label: 'Pulled latest on main', deadline: 5030 });
  f.hook(request => request.method === 'vcs.pull' ? { ok: false, generation: 9, error: { kind: 'server', message: 'pull refused' } } : undefined);
  expect((await action(f, 'select', 'pull')).message).toBe('pull refused');
  expect(mobileGitFeedbackSnapshot(40, f.client)).toMatchObject({ phase: 'error', label: 'Git action failed', description: 'pull refused' });
});

test('mobile status menu keeps source View PR and stricter dirty Push/Create PR rules', async () => {
  const f = fixture(); let data = await read(f);
  expect(data.branchLabel).toBe('main'); expect(data.statusSummary).toBe('Clean · 2 ahead');
  expect(data.rows.map(row => row.label)).toEqual(['Commit', 'Push', 'Create PR', 'Review changes', 'Branches & worktrees']);
  f.status.hasWorkingTreeChanges = true; f.status.workingTree = { files: [{ path: 'a', insertions: 3, deletions: 2 }] };
  data = mobileGitSnapshot(10, f.client); expect(data.rows.filter(row => ['push', 'pr'].includes(row.id)).every(row => row.disabled)).toBe(true);
  expect(data.statusSummary).toBe('1 file changed · 2 ahead');
  f.status.pr = { number: 7, state: 'open', url: 'https://github.test/a/b/pull/7' };
  data = mobileGitSnapshot(10, f.client); expect(data.rows.find(row => row.id === 'pr')).toMatchObject({ label: 'View PR', disabled: false, subtitle: 'PR #7 open' });
  expect(f.calls.filter(call => call.method === 'subscribeVcsStatus')).toHaveLength(1);
});
test('default branch confirmation never sends before choice and rejects changed branch', async () => {
  const f = fixture(); await read(f); const selected = await action(f, 'select', 'push');
  expect(selected.destination).toBe('confirm'); expect(selected.data.confirmTitle).toBe('Push to default branch?');
  expect(f.calls.some(call => call.method === 'git.runStackedAction')).toBe(false);
  f.status.refName = 'other'; expect((await action(f, 'confirm', 'continue')).message).toContain('branch changed');
  expect(f.calls.some(call => call.method === 'git.runStackedAction')).toBe(false);
});
test('accepted action uses shared owned stream and completion reducer', async () => {
  const f = fixture(); await read(f); await action(f, 'select', 'push'); const result = await action(f, 'confirm', 'continue');
  expect(result.message).toBe(''); expect(result.dismiss).toBe(true);
  const call = f.calls.find(call => call.method === 'git.runStackedAction')!;
  expect(call.payload).toMatchObject({ cwd: '/repo', action: 'push', threadId: 't' }); expect(obj(call.payload).featureBranch).toBeUndefined();
  gitActionEvent(f.client, { subscriptionId: 'action-1', value: { kind: 'action_finished', actionId: obj(call.payload).actionId, cwd: '/repo', result: { toast: { title: 'Pushed' }, branch: { status: 'unchanged' } } } });
  expect(mobileGitSnapshot(11, f.client).success).toBe('Pushed'); expect(mobileGitSnapshot(11, f.client).busy).toBe(false);
});
test('commit respects shared exclusions, trimmed message and branch-operation scope', async () => {
  const f = fixture(); f.status.hasWorkingTreeChanges = true; f.status.workingTree = { files: [{ path: 'a', insertions: 3, deletions: 2 }, { path: 'b', insertions: 1, deletions: 0 }] };
  await read(f); await action(f, 'select', 'commit'); await action(f, 'file', 'b');
  expect(mobileGitSnapshot(10, f.client)).toMatchObject({ selectedCount: 1, selectedInsertions: 3, selectedDeletions: 2 });
  f.grant(['orchestration:read', 'source-control:write']); expect((await action(f, 'commit', 'feature', ' no ')).message).toContain("thread's branch");
  expect(f.calls.some(call => call.method === 'git.runStackedAction')).toBe(false);
  expect((await action(f, 'commit', 'continue', '  actual message  ')).message).toBe('');
  expect(obj(f.calls.find(call => call.method === 'git.runStackedAction')!.payload)).toMatchObject({ action: 'commit', commitMessage: 'actual message', filePaths: ['a'] });
});
test('fresh write denial prevents an enabled stale action and cached host status is hidden on read denial', async () => {
  const f = fixture(); await read(f); f.grant(['orchestration:read']);
  expect((await action(f, 'select', 'push')).message).toContain('cannot change');
  expect(f.calls.some(call => call.method === 'git.runStackedAction')).toBe(false);
  f.grant([]); const denied = await read(f); expect(denied.ready).toBe(false); expect(denied.files).toEqual([]); expect(denied.groups).toEqual([]);
  expect(denied.rows.every(row => row.disabled)).toBe(true);
});
test('late authorization rejects owner change before any Git request', async () => {
  const f = fixture(); let release!: (value: unknown) => void, started!: () => void;
  const gate = new Promise(resolve => release = resolve), begun = new Promise<void>(resolve => started = resolve);
  f.hook(request => { if (request.path === '/api/auth/session') { started(); return gate; } });
  const pending = read(f); await begun; f.client.threadId = 'other'; f.client.threadEpoch++;
  release({ ok: true, generation: 9, value: { authenticated: true, permissions: ['orchestration:read', 'source-control:write'] } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' }); expect(f.calls.some(call => call.method)).toBe(false);
});
test('native action rejection is surfaced even though shared stream helper records a toast', async () => {
  const f = fixture(); f.status.isDefaultRef = false; await read(f);
  f.hook(request => request.method === 'git.runStackedAction' ? { ok: false, generation: 9, error: { message: 'server refused push', kind: 'server' } } : undefined);
  const result = await action(f, 'select', 'push'); expect(result.message).toBe('server refused push'); expect(result.dismiss).toBe(false);
});
test('linked PR chains show real source fields and external opening revalidates linked identity', async () => {
  const f = fixture(); f.client.shell.threads[0]!.pullRequests = [{ host: 'github.test', repository: 'a/b', number: 1, url: 'https://github.test/a/b/pull/1', source: 'manual', snapshot: null }];
  const data = await read(f); const row = data.groups[0]!.rows[0]!;
  expect(row).toMatchObject({ title: '#1 Pull request', subtitle: 'a/b · Status pending' });
  expect((await action(f, 'link', row.id)).message).toBe(''); expect(f.calls.some(call => call.op === 'mobileOpenURL')).toBe(true);
  f.client.shell.threads[0]!.pullRequests = []; expect((await action(f, 'link', row.id)).message).toContain('no longer available');
});
test('shared live status supersedes quiet refresh; reads never flush metadata writes', async () => {
  const f = fixture(); await read(f); gitState(f.client).branchSync.push({ threadId: 't', draftKey: f.client.draftKey, branch: 'feature/a' });
  vcsStatusEvent(f.client, { subscriptionId: 'subscription-99', value: { _tag: 'snapshot', local: { ...f.status, refName: 'stream' }, remote: { aheadCount: 3, behindCount: 0 } } });
  await read(f); expect(mobileGitSnapshot(10, f.client).branchLabel).toBe('stream'); expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false);
  expect(mobileGitSnapshot(10, f.client).needsReconcile).toBe(false); // unowned queue cannot cross the mobile seam
});
test('branch list disables other worktrees and only shows real local refs', async () => {
  const f = fixture(); await read(f); const data = await mobileGitBranchesRead(f.owner(), 10, f.native, f.client);
  expect(data.rows.map(row => row.name)).toEqual(['main', 'other']); expect(data.rows[1]).toMatchObject({ disabled: true, subtitle: 'Checked out in another worktree' });
  const result = await mobileGitBranchAction(f.owner(), 'checkout', 'other', '', 10, f.native, f.client);
  expect(result.changed).toBe(false); expect(f.calls.some(call => call.method === 'vcs.switchRef')).toBe(false);
});
test('new worktree uses project root, real result and shared durable thread metadata dispatch', async () => {
  const f = fixture(); f.client.shell.threads[0]!.worktreePath = '/old'; await read(f);
  const result = await mobileGitBranchAction(f.owner(), 'worktree', 'Mobile Polish', ' main ', 10, f.native, f.client);
  expect(result.message).toBe(''); expect(result.changed).toBe(true);
  expect(f.calls.find(call => call.method === 'vcs.createWorktree')?.payload).toEqual({ cwd: '/repo', refName: 'main', newRefName: 'feature/mobile-polish', path: null });
  expect(f.calls.find(call => call.method === 'orchestration.dispatchCommand')?.payload).toMatchObject({ type: 'thread.metadata.update', threadId: 't', branch: 'feature/mobile-polish', worktreePath: '/worktrees/new' });
  expect(f.calls.some(call => call.op === 'writePreferences')).toBe(true);
});
test('default feature choice creates branch first, then starts exactly the requested push', async () => {
  const f = fixture(); f.refs.push({ name: 'feature/update', isRemote: false }); await read(f); await action(f, 'select', 'push');
  expect((await action(f, 'confirm', 'feature')).message).toBe('');
  const create = f.calls.findIndex(call => call.method === 'vcs.createRef'), dispatch = f.calls.findIndex(call => call.method === 'orchestration.dispatchCommand'), push = f.calls.findIndex(call => call.method === 'git.runStackedAction');
  expect(create).toBeGreaterThan(-1); expect(dispatch).toBeGreaterThan(create); expect(push).toBeGreaterThan(dispatch);
  expect(f.calls[create]!.payload).toMatchObject({ refName: 'feature/update-2', switchRef: true });
  expect(obj(f.calls[push]!.payload).featureBranch).toBeUndefined();
  expect(mobileFeatureBranch(' AbC/Hello! ')).toBe('feature/abc/hello'); expect(mobileAutoBranch(['FEATURE/UPDATE', 'feature/update-2'])).toBe('feature/update-3');
});

test('metadata reconcile accepts only actual admitted stream completion in its original environment generation', async () => {
  const f = fixture(); f.status.isDefaultRef = false; await read(f); await action(f, 'select', 'push');
  const call = f.calls.find(call => call.method === 'git.runStackedAction')!;
  gitActionEvent(f.client, { subscriptionId: 'action-1', value: { kind: 'action_finished', actionId: obj(call.payload).actionId, cwd: '/repo', result: { toast: { title: 'Pushed' }, branch: { status: 'created', name: 'feature/new' } } } });
  expect(mobileGitSnapshot(11, f.client).needsReconcile).toBe(true);
  f.client.environmentId = 'other'; f.client.generation++;
  expect(mobileGitSnapshot(11, f.client).needsReconcile).toBe(false);
  expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false);
});

test('admitted metadata effect runs once in explicit action with persistence and current permission', async () => {
  const f = fixture(); f.status.isDefaultRef = false; await read(f); await action(f, 'select', 'push');
  const call = f.calls.find(call => call.method === 'git.runStackedAction')!;
  gitActionEvent(f.client, { subscriptionId: 'action-1', value: { kind: 'action_finished', actionId: obj(call.payload).actionId, cwd: '/repo', result: { toast: { title: 'Pushed' }, branch: { status: 'created', name: 'feature/new' } } } });
  expect((await action(f, 'reconcile')).message).toBe('');
  expect(f.calls.filter(call => call.method === 'orchestration.dispatchCommand')).toHaveLength(1);
  expect(f.calls.find(call => call.method === 'orchestration.dispatchCommand')?.payload).toMatchObject({ type: 'thread.metadata.update', threadId: 't', branch: 'feature/new' });
  expect(mobileGitSnapshot(10, f.client).needsReconcile).toBe(false);
});

test('retry markers resubscribe the shared status key, and a failed attempt consumes only its invalidation', async () => {
  const f = fixture(); await read(f);
  mobileGitEvents([{ key: 'shell-vcs-status', generation: 9, subscriptionId: 'subscription-1', seq: 10, value: { _retryDue: true } }], f.client);
  expect(mobileGitSnapshot(10, f.client).needsRefresh).toBe(true);
  await read(f); expect(f.calls.filter(call => call.method === 'subscribeVcsStatus')).toHaveLength(2);
  expect(mobileGitSnapshot(10, f.client).needsRefresh).toBe(false);
  mobileGitEvents([{ key: 'shell-vcs-status', generation: 9, subscriptionId: 'subscription-2', seq: 11, value: { _retryDue: true } }], f.client);
  f.hook(request => request.op === 'subscribe' ? { ok: false, generation: 9, error: { message: 'offline', kind: 'server' } } : undefined);
  await read(f); expect(mobileGitSnapshot(10, f.client).needsRefresh).toBe(false);
});

test('same-cwd thread selection cannot inherit another thread confirmation or excluded files', async () => {
  const f = fixture(); await read(f); await action(f, 'select', 'push'); gitState(f.client).excluded.add('a');
  f.client.shell.threads.push({ id: 'other', projectId: 'p', branch: 'main' }); f.client.threadId = 'other'; f.client.threadEpoch++;
  const data = mobileGitSnapshot(10, f.client); expect(data.dialog).toBe(''); expect(data.confirmDisabled).toBe(true); expect(gitState(f.client).excluded.size).toBe(0);
  await read(f); expect((await action(f, 'confirm', 'continue')).message).toContain('branch changed');
  expect(f.calls.some(call => call.method === 'git.runStackedAction')).toBe(false);
});
test('cross-environment cache cannot appear when new quiet refresh fails', async () => {
  const f = fixture(); await read(f);
  vcsStatusEvent(f.client, { subscriptionId: 'subscription-99', value: { _tag: 'snapshot', local: { ...f.status, refName: 'private-old-environment' } } });
  expect(mobileGitSnapshot(10, f.client).branchLabel).toBe('private-old-environment');
  f.client.environmentId = 'other'; f.client.generation++;
  f.hook(request => request.method === 'vcs.refreshStatus' ? { ok: false, generation: 10, error: { message: 'offline', kind: 'server' } } : undefined);
  // New transport generation is fixture-owned; native wrapper otherwise returns9.
  const native: Native = { ...f.native, async later(input) { const reply = obj(await f.native.later(input)); return { ...reply, generation: 10 }; } };
  const data = await mobileGitRead(f.owner(), 10, native, f.client);
  expect(data.branchLabel).toBe('main'); expect(data.statusSummary).toBe('Loading branch status…'); expect(data.error).toBe('offline');
});

test('Git action buttons use source secondary surface/border and adaptive diff colors in every palette', async () => {
  const { mobileGitColors } = await import('./git-colors');
  for (const palette of ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris']) for (const scheme of ['light', 'dark']) {
    const colors = mobileGitColors(scheme, palette);
    expect(Object.values(colors).every(color => color.length > 0)).toBe(true);
    const name = `${palette === 't3-code' ? '' : `${palette}-`}${scheme}`;
    const tokens = (await import(`./themes/${name}.json`)).default;
    expect(colors.secondaryBackground).toBe(tokens['--color-secondary']); expect(colors.secondaryBorder).toBe(tokens['--color-secondary-border']);
    expect(colors.addition).toBe(tokens['--color-adaptive-emerald-700-300']); expect(colors.deletion).toBe(tokens['--color-adaptive-rose-700-300']);
  }
});

test('overview and branch preparations may authorize concurrently without starving either read', async () => {
  const f = fixture(); let release!: (value: unknown) => void, started!: () => void; let auth = 0;
  const gate = new Promise(resolve => release = resolve), begun = new Promise<void>(resolve => started = resolve);
  f.hook(request => { if (request.path === '/api/auth/session' && ++auth === 1) { started(); return gate; } });
  const pending = read(f); await begun;
  await mobileGitBranchesRead(f.owner(), 10, f.native, f.client);
  release({ ok: true, generation: 9, value: { authenticated: true, permissions: ['orchestration:read', 'source-control:write', 'orchestration:operate'] } });
  const data = await pending; expect(data.branchLabel).toBe('main'); expect(data.loading).toBe(false);
  expect(mobileGitBranchesSnapshot(10, f.client).rows).toHaveLength(2);
});

test('landed branch creation refreshes after metadata IDs fail without pushing or reporting success', async () => {
  for (const kind of ['create', 'confirm']) {
    const f = fixture(); await read(f);
    if (kind === 'confirm') await action(f, 'select', 'push');
    f.calls.length = 0;
    f.hook(request => request.op === 'ids' ? { ok: false, generation: 9, error: { message: 'IDs unavailable', kind: 'server' } } : undefined);
    const result = kind === 'create'
      ? await mobileGitBranchAction(f.owner(), 'create', 'new', '', 10, f.native, f.client)
      : await action(f, 'confirm', 'feature');
    expect(result.message).not.toBe('');
    expect(f.calls.filter(call => call.method === 'vcs.createRef')).toHaveLength(1);
    expect(f.calls.some(call => call.method === 'vcs.refreshStatus')).toBe(true);
    expect(f.calls.some(call => call.method === 'git.runStackedAction')).toBe(false);
    if ('changed' in result) expect(result.changed).toBe(false); else expect(result.dismiss).toBe(false);
  }
});

test('partial branch mutation publishes an explicit refs invalidation and settled error is not pending', async () => {
  const f = fixture(); await read(f); await mobileGitBranchesRead(f.owner(), 10, f.native, f.client);
  const before = mobileGitBranchesSnapshot(10, f.client).readRevision;
  f.hook(request => request.op === 'ids' ? { ok: false, generation: 9, error: { message: 'IDs unavailable', kind: 'server' } } : undefined);
  await mobileGitBranchAction(f.owner(), 'create', 'new', '', 10, f.native, f.client);
  const data = mobileGitBranchesSnapshot(10, f.client); expect(data.readRevision).toBe(before + 1); expect(data.loading).toBe(false);
  f.hook(undefined); f.refs.push({ name: 'feature/new', isRemote: false, worktreePath: null });
  const refreshed = await mobileGitBranchesRead(f.owner(), 10, f.native, f.client);
  expect(refreshed.rows.some(row => row.name === 'feature/new')).toBe(true); expect(refreshed.loading).toBe(false);
});
