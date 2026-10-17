import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import { mobileThreadHeaderSnapshot, mobileThreadHeaderPrepare, mobileThreadHeaderAction, mobileThreadHeaderEvents, type ThreadHeaderOptions } from './thread-header';
import { mobileThreadHeaderLaunch } from './thread-header-terminal';
import { terminalMetadataEvent } from './shared/terminal-drawer-view';
import { resolveQuickAction, threadNextTerminalId, threadTerminalLabel } from './thread-header-model';
import { mobileReviewRead } from './review-data';
function fixture() {
  const client = new T3Client(), calls: Obj[] = [];
  Object.assign(client, { origin: 'https://example.test', environmentId: 'e', projectId: 'p', threadId: 't', generation: 9,
    connection: 'connected', configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { label: 'Moonbase', capabilities: { serverResolvedCommandContext: true } } };
  client.shell.projects = [{ id: 'p', title: 'Actual project', workspaceRoot: '/repo', scripts: [{ id: 'test', name: 'Run tests', command: 'bun test', icon: 'test', runOnWorktreeCreate: true }] }];
  client.shell.threads = [{ id: 't', title: 'Actual thread', projectId: 'p', branch: 'main', worktreePath: '/shell-tree' }];
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1, projection: {
    thread: { id: 't', projectId: 'p', worktreePath: '/detail-tree', lineage: { relationshipToParent: 'fork', parentThreadId: 'parent' } },
    runs: [{ id: 'r1', ordinal: 1, status: 'completed' }], messages: [] } };
  const status: Obj = { isRepo: true, refName: 'main', hasWorkingTreeChanges: true, hasUpstream: true, hasPrimaryRemote: true,
    isDefaultRef: true, aheadCount: 0, behindCount: 0, workingTree: { files: [{ path: 'a' }] }, pr: null };
  let permissions = ['orchestration:operate', 'orchestration:read', 'source-control:write', 'terminal:read', 'terminal:operate'];
  let counter = 0, hook: ((request: Obj) => unknown) | undefined;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const custom = hook?.(request); if (custom !== undefined) return await custom;
    const value = request.path === '/api/auth/session' ? { authenticated: true, permissions }
      : request.method === 'vcs.refreshStatus' ? status : request.op === 'subscribe' ? { id: `subscription-${++counter}` }
      : request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => '12345678-1234-1234-1234-123456789abc') : {};
    return { ok: true, generation: 9, value };
  } };
  const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('none'); }, async atomicWriteFile() {} } };
  const options: ThreadHeaderOptions = { routeKey: '2', routeIdentity: '/threads/e/t:default', focused: true, split: false, sidebarVisible: false, canGoBack: true, returnToChat: false, foreground: '#111111' };
  const snapshot = () => mobileThreadHeaderSnapshot(options, client);
  const prepare = () => mobileThreadHeaderPrepare(snapshot().owner, native, 10, client);
  const event = (id: string) => { const view = snapshot(); return JSON.stringify({ owner: view.owner, routeKey: view.routeKey, version: view.version, itemID: id }); };
  const action = (id: string, raw = event(id)) => mobileThreadHeaderAction(raw, options, 10, native, files, client);
  const metadata = (terminals: Obj[] = []) => { const entry = { key: 'terminal-metadata', generation: 9, subscriptionId: 'subscription-1', seq: 1, value: { type: 'snapshot', terminals } };
    terminalMetadataEvent(client, entry); mobileThreadHeaderEvents([entry], client); };
  return { client, status, options, native, files, calls, snapshot, prepare, event, action, metadata,
    grant(value: string[]) { permissions = value; }, hook(value: typeof hook) { hook = value; } };
}
const configuration = (f: ReturnType<typeof fixture>) => obj(JSON.parse(f.snapshot().configuration));

async function reviewFixture() {
  const f = fixture(); await f.prepare();
  f.options.review = true; f.options.split = true; f.options.inspectorAvailable = true; f.options.inspectorVisible = true;
  f.client.projection.checkpoints = [{ runId: 'r3', appRunOrdinal: 3, status: 'ready', files: [] },
    { runId: 'r2', appRunOrdinal: 2, status: 'missing', files: [] }, { runId: 'r1', appRunOrdinal: 1, status: 'ready', files: [] }];
  const diff = 'diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n';
  f.hook(request => request.path === '/api/auth/session' ? { ok: true, generation: 9, value: { authenticated: true, permissions: ['filesystem:read'] } }
    : request.method === 'review.getDiffPreview' ? { ok: true, generation: 9, value: { cwd: '/shell-tree', sources: [
      { kind: 'working-tree', title: 'Uncommitted', diff }, { kind: 'branch-range', title: 'Changes', diff }] } }
    : request.method === 'orchestration.getTurnDiff' ? { ok: true, generation: 9, value: { diff } } : undefined);
  await mobileReviewRead(f.native, '', false, false, f.client);
  f.snapshot(); return f;
}
test('Review projects source primary choices, latest ready turn, title/counts and inspector eligibility', async () => {
  const f = await reviewFixture(), config = configuration(f), review = obj(config.review);
  expect(config).toMatchObject({ title: 'Changes', subtitle: '+1 · -1', canOpenFiles: false, canOpenTerminal: false, terminalItems: [] });
  expect(review.primary).toMatchObject([{ id: 'review:section:git:branch-range', title: 'Changes', selected: true, disabled: false },
    { id: 'review:section:git:working-tree', title: 'Uncommitted', selected: false, disabled: false },
    { id: 'review:section:turn:3', title: 'Latest turn', selected: false, disabled: false }]);
  expect(review.turns).toMatchObject([{ id: 'review:section:turn:3', title: 'Turn 3' }, { id: 'review:section:turn:1', title: 'Turn 1' }]);
  expect(review).toMatchObject({ showSections: true, inspectorAvailable: true, inspectorVisible: true });
  const count = f.calls.length;
  expect(await f.action('review:section:git:working-tree')).toMatchObject({ navigation: 'review-section', key: 'git:working-tree' });
  expect(await f.action('review:back')).toMatchObject({ navigation: 'return-chat', location: '/threads/e/t' });
  expect((await f.action('review:sidebar')).navigation).toBe('sidebar');
  expect((await f.action('review:inspector')).navigation).toBe('review-inspector');
  for (const id of ['files', 'sidebar', 'new-task', 'terminal:new', 'review:section:turn:2', 'git:review']) expect((await f.action(id)).navigation).toBe('');
  expect(f.calls).toHaveLength(count);
});
test('Review rejects an old section menu after selection, route or workspace changes', async () => {
  const f = await reviewFixture(), old = f.event('review:section:turn:1');
  await mobileReviewRead(f.native, 'git:working-tree', false, false, f.client);
  expect(configuration(f)).toMatchObject({ title: 'Uncommitted', subtitle: '+1 · -1' });
  const count = f.calls.length;
  expect((await f.action('', old)).navigation).toBe('');
  const current = f.event('review:section:turn:1'); f.options.focused = false; f.snapshot();
  expect((await f.action('', current)).navigation).toBe('');
  f.options.focused = true; f.options.routeKey = 'next'; f.snapshot();
  expect((await f.action('', current)).navigation).toBe('');
  f.client.shell.threads[0]!.worktreePath = '/different'; f.snapshot();
  expect((await f.action('', current)).navigation).toBe('');
  expect(f.calls).toHaveLength(count);
});
test('Review hides empty section menus and unavailable inspectors; missing source choices stay disabled', async () => {
  const f = fixture(); f.options.review = true; f.options.inspectorAvailable = true; f.options.split = false;
  expect(obj(configuration(f).review)).toMatchObject({ showSections: false, inspectorAvailable: false, turns: [],
    primary: [{ title: 'Changes', disabled: true }, { title: 'Uncommitted', disabled: true }, { title: 'Latest turn', disabled: true }] });
  for (const id of ['review:section:', 'review:inspector', 'review:sidebar']) expect((await f.action(id)).navigation).toBe('');
  const loaded = await reviewFixture(); loaded.options.inspectorAvailable = false;
  expect((await loaded.action('review:inspector')).navigation).toBe('');
});

test('header uses actual title/scripts and stable menu version, without opening a terminal during preparation', async () => {
  const f = fixture(); await f.prepare(); const before = f.snapshot(); f.client.revision += 10;
  expect(f.snapshot().version).toBe(before.version);
  const config = configuration(f); expect(config.title).toBe('Actual thread'); expect(String(config.subtitle)).toContain('Actual project');
  expect(config.gitItems).toContainEqual({ id: 'git:review', title: 'Review changes', subtitle: 'Turn diffs and worktree changes', symbol: 'text.bubble', disabled: false });
  expect(config.terminalItems).toContainEqual({ id: 'terminal:script:test', title: 'Run tests (setup)', subtitle: 'bun test', symbol: 'flask', disabled: false });
  expect(f.calls.some(call => call.method === 'terminal.open')).toBe(false);
  expect(f.calls.filter(call => call.method === 'subscribeTerminalMetadata')).toHaveLength(1);
});
test('forged, stale recipe, hidden-route and layout-ineligible callbacks have no side effect', async () => {
  const f = fixture(); await f.prepare(); const raw = f.event('terminal:script:test'), count = f.calls.length;
  obj((f.client.shell.projects[0]!.scripts as Obj[])[0]).command = 'different';
  expect((await f.action('', raw)).navigation).toBe('');
  expect((await f.action('sidebar')).navigation).toBe(''); expect((await f.action('home')).navigation).toBe('');
  f.options.focused = false; f.snapshot(); expect((await f.action('', raw)).navigation).toBe('');
  expect(f.calls).toHaveLength(count);
});
test('local Files/Review/More route projection needs no extra native request', async () => {
  const f = fixture(); await f.prepare(); const count = f.calls.length;
  expect(await f.action('files')).toMatchObject({ navigation: 'threadFiles', location: '/threads/e/t/files' });
  expect(await f.action('git:review')).toMatchObject({ navigation: 'threadReview', location: '/threads/e/t/review' });
  expect(await f.action('git:more')).toMatchObject({ navigation: 'gitOverview', location: '/threads/e/t/git' });
  f.options.split = true; f.options.returnToChat = true;
  expect(await f.action('return-chat')).toMatchObject({ navigation: 'return-chat', location: '/threads/e/t' });
  expect(f.calls).toHaveLength(count);
});
test('mobile quick action selects stacked commit/push and real default-branch confirmation', async () => {
  const f = fixture(); await f.prepare();
  const reply = await f.action('git:quick'); expect(reply).toMatchObject({ navigation: 'gitConfirm', location: '/threads/e/t/git-confirm' });
  expect(f.calls.some(call => call.method === 'git.runStackedAction')).toBe(false);
  expect(resolveQuickAction({ isRepo: true, refName: 'topic', hasWorkingTreeChanges: true, hasUpstream: true, aheadCount: 0, behindCount: 0, pr: null }, false).action).toBe('commit_push_pr');
});
test('fresh denied terminal grant refuses a previously enabled menu', async () => {
  const f = fixture(); await f.prepare(); const raw = f.event('terminal:new'); f.grant([]);
  expect((await f.action('', raw)).message).toContain('cannot operate');
  expect(f.calls.some(call => call.op === 'ids' || call.method === 'terminal.open')).toBe(false);
  expect(configuration(f).canOpenTerminal).toBe(false);
});
test('route departure while permission read is pending prevents script staging and allocation', async () => {
  const f = fixture(); await f.prepare(); let release!: () => void;
  f.hook(request => request.path === '/api/auth/session' ? new Promise(resolve => { release = () => resolve({ ok: true, generation: 9, value: { authenticated: true, permissions: ['terminal:operate'] } }); }) : undefined);
  const action = f.action('terminal:script:test'); await Promise.resolve();
  f.options.routeKey = '3'; f.options.focused = false; f.snapshot(); release();
  await expect(action).rejects.toThrow('screen changed'); expect(f.snapshot().pendingLaunch).toBe('');
  expect(f.calls.some(call => call.op === 'ids')).toBe(false);
});
test('script captures detail-preferred cwd/env, stays pending offscreen, and writes once only', async () => {
  const f = fixture(); await f.prepare(); f.metadata();
  const result = await f.action('terminal:script:test'); expect(result.key).toBe('default'); expect(result.pendingLaunch).not.toBe('');
  f.options.focused = false; f.options.routeKey = 'terminal'; expect(f.snapshot().pendingLaunch).toBe(result.pendingLaunch);
  const launch = await mobileThreadHeaderLaunch(result.key, result.pendingLaunch, f.native, f.client); expect(launch.message).toBe('');
  expect(f.calls.find(call => call.method === 'terminal.open')?.payload).toMatchObject({ cwd: '/detail-tree', worktreePath: '/detail-tree', env: { T3CODE_PROJECT_ROOT: '/repo', T3CODE_WORKTREE_PATH: '/detail-tree' } });
  expect(f.calls.find(call => call.method === 'terminal.write')?.payload).toMatchObject({ data: 'bun test\r' });
  await mobileThreadHeaderLaunch(result.key, result.pendingLaunch, f.native, f.client);
  expect(f.calls.filter(call => call.method === 'terminal.write')).toHaveLength(1); expect(f.snapshot().pendingLaunch).toBe('');
});
test('unknown metadata allocates UUID suffix; uncertain script write is consumed and never retried', async () => {
  const f = fixture(); await f.prepare(); const result = await f.action('terminal:script:test');
  expect(result.key).toBe('term-1-12345678-1234-1234-1234-123456789abc');
  f.hook(request => request.method === 'terminal.write' ? { ok: false, generation: 9, error: { kind: 'transport', message: 'No acknowledgment', uncertain: true } } : undefined);
  expect((await mobileThreadHeaderLaunch(result.key, result.pendingLaunch, f.native, f.client)).message).toContain('No acknowledgment');
  await mobileThreadHeaderLaunch(result.key, result.pendingLaunch, f.native, f.client);
  expect(f.calls.filter(call => call.method === 'terminal.write')).toHaveLength(1);
  expect(threadNextTerminalId(['default', 'term-1-12345678-1234-1234-1234-123456789abc'])).toBe('term-2');
  expect(threadTerminalLabel(result.key, null)).toBe('Terminal 1');
});
test('merge uses real shared durable dispatch, preserves ordinary draft and returns actual target route', async () => {
  const f = fixture(); await f.prepare(); f.client.local.drafts['e:t'] = 'keep';
  const reply = await f.action('git:merge'); expect(reply.message).toBe(''); expect(reply).toMatchObject({ threadId: 'parent', navigation: 'thread', location: '/threads/e/parent' });
  expect(f.calls.find(call => call.method === 'orchestration.dispatchCommand')?.payload).toMatchObject({ type: 'thread.merge_back', creationSource: 'mobile', sourceThreadId: 't', targetThreadId: 'parent', sourcePoint: { type: 'run', runId: 'r1' } });
  expect(f.client.local.drafts['e:t']).toBe('keep'); expect(f.client.threadId).toBe('t');
});
test('competing pending operation during merge IDs survives with no dispatch', async () => {
  const f = fixture(); await f.prepare(); const other = { method: 'orchestration.dispatchCommand', payload: { commandId: 'other' }, description: 'Other', threadId: 't', text: '', uncertain: false };
  f.hook(request => { if (request.op === 'ids') f.client.local.pending.e = other; });
  await expect(f.action('git:merge')).rejects.toThrow('Another operation');
  expect(f.client.pending).toBe(other); expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false);
});
test('route changes during merge persistence clean known-unsent pending without network write', async () => {
  const f = fixture(); await f.prepare(); let saves = 0;
  f.files.fs.atomicWriteFile = async () => { if (++saves === 1) { f.options.routeKey = 'other'; f.snapshot(); } };
  expect((await f.action('git:merge')).message).toContain('screen changed');
  expect(f.client.pending).toBeUndefined(); expect(f.calls.some(call => call.method === 'orchestration.dispatchCommand')).toBe(false); expect(saves).toBe(2);
});
test('metadata loss returns terminal allocation to UUID mode until an authoritative reply', async () => {
  const f = fixture(); await f.prepare(); f.metadata();
  mobileThreadHeaderEvents([{ key: 'terminal-metadata', generation: 9, subscriptionId: 'subscription-1', seq: 2, value: { _streamEnded: true } }], f.client);
  expect((await f.action('terminal:new')).key).toBe('term-1-12345678-1234-1234-1234-123456789abc');
});
test('parallel launch clicks claim once before auth and cannot erase the first command', async () => {
  const f = fixture(); await f.prepare(); f.metadata(); const launch = await f.action('terminal:script:test');
  let release!: () => void;
  f.hook(request => request.path === '/api/auth/session' ? new Promise(resolve => { release = () => resolve({ ok: true, generation: 9, value: { authenticated: true, permissions: ['terminal:operate'] } }); }) : undefined);
  const first = mobileThreadHeaderLaunch(launch.key, launch.pendingLaunch, f.native, f.client); await Promise.resolve();
  expect((await mobileThreadHeaderLaunch(launch.key, launch.pendingLaunch, f.native, f.client)).message).toBe('');
  release(); await first;
  expect(f.calls.filter(call => call.method === 'terminal.write')).toHaveLength(1);
  expect(f.calls.find(call => call.method === 'terminal.write')?.payload).toMatchObject({ data: 'bun test\r' });
});
test('late terminal open after route departure cannot inject a script into an offscreen session', async () => {
  const f = fixture(); await f.prepare(); f.metadata(); const launch = await f.action('terminal:script:test');
  f.options.routeKey = 'terminal'; f.options.focused = false; f.snapshot();
  let release!: () => void;
  f.hook(request => request.method === 'terminal.open' ? new Promise(resolve => { release = () => resolve({ ok: true, generation: 9, value: {} }); }) : undefined);
  const pending = mobileThreadHeaderLaunch(launch.key, launch.pendingLaunch, f.native, f.client);
  for (let i = 0; i < 20 && !release; i++) await Promise.resolve();
  f.options.routeKey = 'home'; f.snapshot(); release();
  await expect(pending).rejects.toThrow('terminal screen changed');
  expect(f.calls.some(call => call.method === 'terminal.write')).toBe(false); expect(f.snapshot().pendingLaunch).toBe('');
});
test('issued merge acknowledgment clears captured pending after route departure without stale navigation', async () => {
  const f = fixture(); await f.prepare(); let release!: () => void;
  f.hook(request => request.method === 'orchestration.dispatchCommand' ? new Promise(resolve => { release = () => resolve({ ok: true, generation: 9, value: {} }); }) : undefined);
  const pending = f.action('git:merge');
  for (let i = 0; i < 60 && !release; i++) await Promise.resolve();
  f.options.routeKey = 'settings'; f.options.focused = false; f.snapshot(); release();
  expect((await pending).navigation).toBe(''); expect(f.client.pending).toBeUndefined();
});
test('metadata error hides old sessions and a preparation cannot reenable stale allocation', async () => {
  const f = fixture(); await f.prepare(); f.metadata([{ threadId: 't', terminalId: 'term-1', cwd: '/repo', worktreePath: null, status: 'running', pid: 1,
    exitCode: null, exitSignal: null, hasRunningSubprocess: false, label: '', updatedAt: '' }]);
  expect((configuration(f).terminalItems as Obj[]).some(item => item.id === 'terminal:session:term-1')).toBe(true);
  mobileThreadHeaderEvents([{ key: 'terminal-metadata', generation: 9, subscriptionId: 'subscription-1', seq: 2, value: { _transportError: { message: 'Offline' } } }], f.client);
  await f.prepare();
  expect((configuration(f).terminalItems as Obj[]).some(item => String(item.id).startsWith('terminal:session:'))).toBe(false);
  expect((await f.action('terminal:new')).key).toBe('term-2-12345678-1234-1234-1234-123456789abc');
});
test('same-ID replace and away-back route ABA during terminal open never inject captured script', async () => {
  for (const returnToTerminal of [false, true]) {
    const f = fixture(); await f.prepare(); f.metadata(); const launch = await f.action('terminal:script:test');
    f.options.routeKey = 'terminal'; f.options.routeIdentity = '/threads/e/t/terminal:default'; f.options.focused = false; f.snapshot();
    let release!: () => void;
    f.hook(request => request.method === 'terminal.open' ? new Promise(resolve => { release = () => resolve({ ok: true, generation: 9, value: {} }); }) : undefined);
    const pending = mobileThreadHeaderLaunch(launch.key, launch.pendingLaunch, f.native, f.client);
    for (let i = 0; i < 20 && !release; i++) await Promise.resolve();
    f.options.routeIdentity = '/settings:default'; f.snapshot();
    if (returnToTerminal) { f.options.routeIdentity = '/threads/e/t/terminal:default'; f.snapshot(); }
    release(); await expect(pending).rejects.toThrow('terminal screen changed');
    expect(f.calls.some(call => call.method === 'terminal.write')).toBe(false); expect(f.snapshot().pendingLaunch).toBe('');
  }
});
