import { mobileLayoutFacts } from './root-presentation';
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { draftContext } from './shared/composer-controls-branch';
import { mobileComposerTarget } from './composer-target';
import { mobileComposerSettingsAction } from './composer-settings';
import { mobileNewTaskFlowView as view, mobileNewTaskFlowAction as act, mobileNewTaskFlowOwns as owns, mobileNewTaskRoute } from './new-task-flow';

async function fixture() {
  const client = new T3Client(), fleet = new EnvironmentFleet(), calls: Obj[] = [];
  const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('not saved'); }, async atomicWriteFile() {} } };
  await client.command('dismiss-error', '', '', 0, { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } }, files);
  Object.assign(client, { environmentId: 'env', origin: 'https://example.test', projectId: 'a', connection: 'connected',
    configLive: true, shellLive: true, shellLoaded: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [] };
  client.shell.projects = [{ id: 'a', title: 'A', workspaceRoot: '/a' }, { id: 'b', title: 'B', workspaceRoot: '/b' }];
  client.local.drafts['env:new:a'] = 'A original'; client.local.drafts['env:new:b'] = 'B original';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const value = request.op === 'http' ? { authenticated: true, permissions: ['source-control:write'] }
      : request.method === 'vcs.switchRef' ? { refName: 'normalized-branch' } : {};
    return { ok: true, generation: client.generation, value };
  } };
  const snapshot = (location: string, visit = 'visit', session = 'flow', ready = true) => view(session, visit, location, true, ready, client, fleet);
  const action = (owner: string, kind: string, id = '', value = '', visit = 'visit') => act(owner, visit, kind, id, value, native, files, client, fleet);
  return { client, fleet, files, native, calls, snapshot, action };
}

test('Choose project never adopts the prior thread or even a sole project automatically', async () => {
  const f = await fixture(); f.client.threadId = 'old-thread'; f.client.shell.projects = f.client.shell.projects.slice(0, 1);
  const chooser = f.snapshot('/new');
  expect(chooser).toMatchObject({ chooser: true, title: 'Choose project', ready: false, needsPrepare: false });
  expect(f.client.threadId).toBe('old-thread'); expect(f.client.local.drafts['env:new:a']).toBe('A original');
  expect(f.snapshot('/new/draft', 'draft')).toMatchObject({ status: 'pick', nextLocation: '/new', ready: false });
  expect(f.calls).toHaveLength(0);
});

test('explicit project selection keeps existing shared drafts and enables only that flow', async () => {
  const f = await fixture(), chooser = f.snapshot('/new');
  expect(await f.action(chooser.owner, 'project', '["env","b"]')).toMatchObject({ message: '', nextLocation: '/new/draft', submitted: false });
  const draft = f.snapshot('/new/draft', 'draft');
  expect(draft.ready).toBe(true); expect(owns(draft.owner, 'draft', f.client)).toBe(true);
  expect(f.client.projectId).toBe('b'); expect(f.client.draft).toBe('B original'); expect(f.client.local.drafts['env:new:a']).toBe('A original');
  expect(f.snapshot('/new/draft', 'fresh', 'other-flow')).toMatchObject({ ready: false, nextLocation: '/new' });
});

test('settings preparation retains presentation owner without admitting stale or unready draft actions', async () => {
  const f = await fixture(), chooser = f.snapshot('/new');
  expect(chooser.draftOwner).toBe('');
  await f.action(chooser.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft', 'draft'), contentOwner = mobileComposerTarget(f.client).owner;
  expect(draft).toMatchObject({ ready: true, draftOwner: contentOwner });
  expect(contentOwner).not.toBe('');
  let writes = 0; f.files.fs.atomicWriteFile = async () => { writes++; };
  for (const visit of ['model-1', 'model-2']) {
    const opening = f.snapshot('/new/draft/settings', visit);
    expect(opening).toMatchObject({ ready: false, needsPrepare: true, draftOwner: contentOwner });
    expect(owns(opening.owner, 'draft', f.client)).toBe(false);
    expect(owns(opening.owner, visit, f.client)).toBe(false);
    expect((await f.action(opening.owner, 'draft', '', 'stale', 'draft')).message).toContain('route changed');
    expect((await f.action(opening.owner, 'draft', '', 'unready', visit)).message).toContain('ready');
    expect(writes).toBe(0);
    expect(await f.action(opening.owner, 'prepare', '', '', visit)).toMatchObject({ message: '' });
    expect(f.snapshot('/new/draft/settings', visit)).toMatchObject({ ready: true, draftOwner: contentOwner });
    expect(owns(opening.owner, visit, f.client)).toBe(true);
    const nested = f.snapshot('/new/draft/settings/runtime', `${visit}-runtime`);
    expect(nested).toMatchObject({ ready: true, draftOwner: contentOwner });
    expect(owns(nested.owner, visit, f.client)).toBe(false);
    expect(f.snapshot('/new/draft/settings', visit)).toMatchObject({ ready: true, draftOwner: contentOwner });
    await mobileComposerSettingsAction('cancel', '', '', f.native, f.files, f.client);
    expect(f.snapshot('/new/draft', 'draft')).toMatchObject({ ready: true, draftOwner: contentOwner });
    expect(owns(draft.owner, 'draft', f.client)).toBe(true);
  }
  expect(f.client.local.drafts['env:new:a']).toBe('A original');
  expect(f.client.local.drafts['env:new:b']).toBe('B original'); expect(writes).toBe(0);
});

test('presentation owner disappears when selected draft proof is lost', async () => {
  const changes: Array<(client: T3Client) => void> = [
    client => { client.projectId = 'b'; }, client => { client.environmentId = 'elsewhere'; },
    client => { client.generation++; }, client => { client.threadEpoch++; },
    client => { client.threadId = 'thread'; }, client => { client.shell.projects = []; },
    client => { client.shell.projects[0].archivedAt = '2026-10-07T00:00:00Z'; },
  ];
  for (const change of changes) {
    const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
    const draft = f.snapshot('/new/draft', 'draft'); expect(draft.draftOwner).not.toBe('');
    change(f.client);
    expect(f.snapshot('/new/draft/settings', 'model')).toMatchObject({ draftOwner: '', ready: false });
    expect(owns(draft.owner, 'draft', f.client)).toBe(false);
    expect((await f.action(draft.owner, 'draft', '', 'blocked', 'model')).message).toContain('ready');
    expect(f.client.local.drafts['env:new:a']).toBe('A original');
  }
});

test('inactive, replacement and unselected flows never inherit a presentation owner', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft', 'draft'); expect(draft.draftOwner).not.toBe('');
  expect(view('flow', 'gone', '/', false, true, f.client, f.fleet)).toMatchObject({ draftOwner: '', ready: false });
  expect(owns(draft.owner, 'draft', f.client)).toBe(false);
  expect(f.snapshot('/new/draft', 'returned')).toMatchObject({ draftOwner: '', ready: false });
  const replacement = f.snapshot('/new/draft/settings', 'model', 'replacement');
  expect(replacement).toMatchObject({ draftOwner: '', ready: false });
  expect(owns(replacement.owner, 'model', f.client)).toBe(false);
  const unloaded = new T3Client();
  expect(view('unloaded', 'draft', '/new/draft', true, true, unloaded, f.fleet)).toMatchObject({ status: 'loading', draftOwner: '', ready: false });
});

test('direct context query initializes the actual project once and does not undo later picks', async () => {
  const f = await fixture(), url = '/new/draft/environment?environmentId=env&projectId=b';
  const first = f.snapshot(url); expect(first.needsPrepare).toBe(true);
  expect(await f.action(first.owner, 'prepare')).toMatchObject({ message: '', nextLocation: '' });
  expect(f.snapshot(url)).toMatchObject({ ready: true, title: 'Environment', needsPrepare: false });
  expect(await f.action(first.owner, 'project', '["env","a"]')).toMatchObject({ message: '' });
  expect(f.snapshot(url).ready).toBe(true); expect(f.client.projectId).toBe('a');
  f.snapshot('/new/draft/branch', 'child');
  expect(f.snapshot(url).needsPrepare).toBe(false); expect(f.client.projectId).toBe('a');
  expect(f.snapshot('/new/draft/environment?environmentId=env&projectId=a').needsPrepare).toBe(true);
});

test('unknown direct identity waits for catalog, then returns chooser without selecting a fallback', async () => {
  const f = await fixture(), url = '/new/draft?environmentId=env&projectId=deleted';
  expect(f.snapshot(url, 'visit', 'flow', false).status).toBe('loading');
  expect(f.snapshot(url)).toMatchObject({ status: 'pick', ready: false, nextLocation: '/new' });
  expect(f.client.projectId).toBe('a'); expect(f.calls).toHaveLength(0);
  expect(f.snapshot('/new/draft/branch?projectId=b')).toMatchObject({ status: 'pick', nextLocation: '/new' });
});

test('unimplemented draft/outbox/share identity never falls through to an existing project', async () => {
  const f = await fixture();
  for (const key of ['draftId', 'pendingTaskId', 'incomingShareId', 'cloning']) {
    expect(f.snapshot(`/new/draft?environmentId=env&projectId=b&${key}=real-id`)).toMatchObject({ status: 'pick', ready: false, needsPrepare: false, nextLocation: '/new' });
  }
  expect(f.client.projectId).toBe('a'); expect(mobileNewTaskRoute('/new/draft?environmentId=one&environmentId=two&projectId=a').environmentId).toBe('one');
});

test('branch links dispatch real checkout before accepting normalized branch context', async () => {
  const f = await fixture(), flow = f.snapshot('/new/draft?environmentId=env&projectId=b&branch=feature');
  expect(await f.action(flow.owner, 'prepare')).toMatchObject({ message: '' });
  expect(f.calls.find(call => call.method === 'vcs.switchRef')?.payload).toEqual({ cwd: '/b', refName: 'feature' });
  expect(draftContext(f.client)).toEqual({ envMode: 'local', branch: 'normalized-branch', worktreePath: '' });
  expect(f.client.draft).toBe('B original'); expect(f.snapshot('/new/draft?environmentId=env&projectId=b&branch=feature').ready).toBe(true);
});

test('checkout permission loss refuses the draft and preserves its previous context', async () => {
  const f = await fixture(), original = f.native.later;
  f.native.later = async input => obj(input).op === 'http' ? { ok: true, generation: f.client.generation, value: { authenticated: true, permissions: [] } } : original(input);
  const url = '/new/draft?environmentId=env&projectId=b&branch=feature', flow = f.snapshot(url);
  expect((await f.action(flow.owner, 'prepare')).message).toContain('cannot check out');
  expect(f.calls.some(call => call.method === 'vcs.switchRef')).toBe(false);
  expect(draftContext(f.client).branch).toBe(''); expect(f.snapshot(url).ready).toBe(false);
});

test('leaving during selection prevents late flow adoption without erasing any draft', async () => {
  const f = await fixture(); let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  const original = f.native.later;
  f.native.later = async input => { if (obj(input).op === 'unsubscribe') { entered(); await gate; } return original(input); };
  const first = f.snapshot('/new'), pending = f.action(first.owner, 'project', '["env","b"]');
  await started; view('flow', 'gone', '/', false, true, f.client, f.fleet); release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot('/new/draft', 'returned').ready).toBe(false);
  expect(f.client.local.drafts['env:new:a']).toBe('A original'); expect(f.client.local.drafts['env:new:b']).toBe('B original');
});

test('a dispatched checkout blocks another flow until it settles and cannot patch a replacement draft', async () => {
  const f = await fixture(); let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  const original = f.native.later;
  f.native.later = async input => { if (obj(input).method === 'vcs.switchRef') { entered(); await gate; } return original(input); };
  const first = f.snapshot('/new/draft?environmentId=env&projectId=b&branch=feature');
  const pending = f.action(first.owner, 'prepare'); await started;
  const replacement = f.snapshot('/new/draft?environmentId=env&projectId=a', 'new', 'other-flow');
  expect(replacement.busy).toBe(true); expect(replacement.needsPrepare).toBe(false);
  expect((await act(replacement.owner, 'new', 'prepare', '', '', f.native, f.files, f.client, f.fleet)).message).toContain('Wait');
  release(); await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(draftContext(f.client).branch).toBe('');
  expect(f.snapshot('/new/draft?environmentId=env&projectId=a', 'new', 'other-flow').needsPrepare).toBe(true);
});

test('draft edits retain their captured project while persistence overlaps and ownership changes', async () => {
  const f = await fixture(), first = f.snapshot('/new'); await f.action(first.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft'); let release!: () => void, entered!: () => void, writes = 0;
  const started = new Promise<void>(resolve => { entered = resolve; }), gate = new Promise<void>(resolve => { release = resolve; });
  f.files.fs.atomicWriteFile = async () => { if (++writes === 1) { entered(); await gate; } };
  const edit = f.action(draft.owner, 'draft', '', 'A changed'); await started;
  const latest = f.action(draft.owner, 'draft', '', 'A latest'); await latest;
  f.client.projectId = 'b'; release(); await edit;
  expect(f.client.local.drafts['env:new:a']).toBe('A latest'); expect(f.client.draft).toBe('B original');
  expect(owns(draft.owner, 'visit', f.client)).toBe(false);
});

test('a same-route external selection while unsubscribe is pending cannot become the clicked project flow', async () => {
  const f = await fixture(); let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  const original = f.native.later;
  f.native.later = async input => { if (obj(input).op === 'unsubscribe') { entered(); await gate; } return original(input); };
  const first = f.snapshot('/new'), pending = f.action(first.owner, 'project', '["env","b"]'); await started;
  f.client.projectId = 'a'; f.client.threadEpoch++; release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot('/new/draft').ready).toBe(false); expect(f.client.draft).toBe('A original');
});

test('attachment and settings child routes preserve the chosen draft owner', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  for (const path of ['/new/draft/attachments/file-id', '/new/draft/files/src%2Fa.ts']) {
    const child = f.snapshot(path, path); expect(child.ready).toBe(true); expect(owns(child.owner, path, f.client)).toBe(true);
  }
});

import { mobileNewTaskChooser } from './new-task';
test('Choose project reuses repository grouping, searches member paths and picks a real preferred-environment member', async () => {
  const f = await fixture();
  const repositoryIdentity = { canonicalKey: 'github.com/acme/repo', name: 'repo', displayName: 'acme/repo', rootPath: '/a' };
  f.client.shell.projects = [{ id: 'a', title: 'repo', workspaceRoot: '/a', repositoryIdentity },
    { id: 'b', title: 'repo', workspaceRoot: '/checkout/work', repositoryIdentity }];
  const grouped = mobileNewTaskChooser('checkout', 'repository', f.client, f.fleet);
  expect(grouped.projects).toHaveLength(1);
  expect(grouped.projects[0]).toMatchObject({ id: '["env","a"]', title: 'acme/repo', subtitle: '2 workspaces' });
  expect(mobileNewTaskChooser('', 'separate', f.client, f.fleet).projects).toHaveLength(2);
  expect(mobileNewTaskChooser('absent', 'repository', f.client, f.fleet)).toMatchObject({ projects: [], emptyTitle: 'No matching projects' });
});


test('unprepared explicit identity and direct settings children cannot operate the previous draft', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const draft = f.snapshot('/new/draft'); expect(owns(draft.owner, 'visit', f.client)).toBe(true);
  const changed = f.snapshot('/new/draft?environmentId=env&projectId=b');
  expect(changed.needsPrepare).toBe(true); expect(changed.draftOwner).toBe(draft.draftOwner); expect(owns(changed.owner, 'visit', f.client)).toBe(false);
  expect((await f.action(changed.owner, 'draft', '', 'wrong owner')).message).toContain('ready');
  expect(f.client.local.drafts['env:new:a']).toBe('A original');
  const settings = f.snapshot('/new/draft/settings/runtime', 'runtime');
  expect(settings.needsPrepare).toBe(true); expect(settings.draftOwner).toBe(draft.draftOwner); expect(owns(settings.owner, 'runtime', f.client)).toBe(false);
  await f.action(settings.owner, 'prepare', '', '', 'runtime');
  expect(f.snapshot('/new/draft/settings/runtime', 'runtime').ready).toBe(true);
});

// No project uses the real shared ensureScratch + shell decoder, never a synthetic chooser row.
import { mobileNewTask, mobileNewTaskChooser, mobileNewTaskPrepare } from './new-task';
import { mobileScratchTarget, mobileOpenScratch, mobileMoveScratch } from './new-task-scratch';
import { patchDraftContext } from './shared/composer-controls-branch';
function scratchNative(f: Awaited<ReturnType<typeof fixture>>, options: { grant?: boolean; missing?: boolean } = {}) {
  f.client.config.scratchWorkspaceRoot = '/scratch';
  const original = f.native.later;
  f.native.later = async input => {
    const request = obj(input);
    if (request.path === '/api/auth/session') { f.calls.push(request); return { ok: true, generation: f.client.generation,
      value: { authenticated: true, permissions: options.grant === false ? [] : ['orchestration:operate'] } }; }
    if (request.method === 'projects.ensureScratch') { f.calls.push(request); return { ok: true, generation: f.client.generation, value: { projectId: 'scratch' } }; }
    if (request.path === '/api/orchestration/shell') { f.calls.push(request); return { ok: true, generation: f.client.generation,
      value: { snapshotSequence: 1, projects: [...f.client.shell.projects, ...(options.missing ? [] : [{ id: 'scratch', title: 'Scratch', workspaceRoot: '/scratch' }])], threads: [] } }; }
    return original(input);
  };
}

test('No project uses actual shared creation and shell admission; previous and scratch drafts survive', async () => {
  const f = await fixture(); scratchNative(f);
  f.client.local.drafts['env:new:scratch'] = 'Existing scratch';
  patchDraftContext(f.client, { envMode: 'worktree', branch: 'stale', worktreePath: '/old' }, 'env:new:scratch');
  const chooser = f.snapshot('/new'), data = mobileNewTaskChooser('', 'repository', f.client, f.fleet);
  expect(data.canStartScratch).toBe(true); expect(data.projects.map(row => row.projectId)).toEqual(['a', 'b']);
  expect(await f.action(chooser.owner, 'scratch', data.scratchTarget)).toMatchObject({ nextLocation: '/new/draft', message: '', projectId: 'scratch' });
  expect(f.snapshot('/new/draft', 'draft').ready).toBe(true); expect(f.client.draft).toBe('Existing scratch');
  expect(f.client.local.drafts['env:new:a']).toBe('A original');
  expect(draftContext(f.client)).toEqual({ envMode: 'local', branch: '', worktreePath: '' });
  expect(f.calls.filter(call => call.method === 'projects.ensureScratch')).toHaveLength(1);
  expect(f.calls.filter(call => call.op === 'timelineSleep')).toHaveLength(1);
  expect(f.calls.some(call => call.path === '/api/orchestration/shell')).toBe(true);
  expect(mobileNewTaskChooser('', 'repository', f.client, f.fleet).projects.some(row => row.projectId === 'scratch')).toBe(false);
  const before = f.calls.length; await mobileNewTaskPrepare('', f.native, f.client); expect(f.calls.length).toBe(before);
  for (const kind of ['workspace', 'origin', 'branch']) expect((await f.action(chooser.owner, kind, '', 'worktree', 'draft')).message).toContain('without a project');
});

test('scratch eligibility excludes reconnecting cached shells and permission loss writes nothing', async () => {
  const f = await fixture(); scratchNative(f, { grant: false });
  const chooser = f.snapshot('/new'), key = mobileScratchTarget(f.client, f.fleet);
  f.client.connection = 'reconnecting'; expect(mobileScratchTarget(f.client, f.fleet)).toBe('');
  expect((await f.action(chooser.owner, 'scratch', key)).message).toContain('no longer available');
  f.client.connection = 'connected';
  expect((await f.action(chooser.owner, 'scratch', key)).message).toContain('cannot create');
  expect(f.calls.some(call => call.method === 'projects.ensureScratch')).toBe(false); expect(f.client.projectId).toBe('a');
});

test('missing shell project is bounded, does not select a guessed project, and retry is explicit', async () => {
  const f = await fixture(); scratchNative(f, { missing: true });
  const chooser = f.snapshot('/new');
  expect((await f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet))).message).toContain('has not reached');
  expect(f.calls.filter(call => call.op === 'timelineSleep')).toHaveLength(20);
  expect(f.calls.filter(call => call.method === 'projects.ensureScratch')).toHaveLength(1); expect(f.client.projectId).toBe('a');
});

test('leaving during ensureScratch never selects late project or starts another request', async () => {
  const f = await fixture(); scratchNative(f); const original = f.native.later;
  let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  f.native.later = async input => { if (obj(input).method === 'projects.ensureScratch') { entered(); await gate; } return original(input); };
  const chooser = f.snapshot('/new'), pending = f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet));
  await started;
  expect((await f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet))).message).toContain('Wait');
  view('flow', 'gone', '/', false, true, f.client, f.fleet); release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.projectId).toBe('a'); expect(f.calls.some(call => call.path === '/api/orchestration/shell')).toBe(false);
});

test('an aborted scratch polling answer stays superseded despite shared optional-wait catches', async () => {
  const f = await fixture(); scratchNative(f); const original = f.native.later;
  f.native.later = async input => { if (obj(input).op === 'timelineSleep') throw { name: 'FetchError', kind: 'Aborted' }; return original(input); };
  const chooser = f.snapshot('/new');
  await expect(f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet))).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.projectId).toBe('a'); expect(f.calls.some(call => call.path === '/api/orchestration/shell')).toBe(false);
});

test('scratch explicit links ignore obsolete worktree settings and preserve the saved draft', async () => {
  const f = await fixture(); scratchNative(f);
  f.client.shell.projects.push({ id: 'scratch', title: 'Scratch', workspaceRoot: '/scratch' });
  f.client.local.drafts['env:new:scratch'] = 'Keep me';
  const url = '/new/draft?environmentId=env&projectId=scratch&branch=old&worktreePath=%2Fold', flow = f.snapshot(url);
  expect((await f.action(flow.owner, 'prepare')).message).toBe('');
  expect(f.snapshot(url).ready).toBe(true); expect(f.client.draft).toBe('Keep me');
  expect(draftContext(f.client)).toEqual({ envMode: 'local', branch: '', worktreePath: '' });
  expect(f.calls.some(call => call.method === 'vcs.switchRef')).toBe(false);
});

test('remote scratch addresses actual fleet generation and adopts only that shell', async () => {
  const f = await fixture();
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const,
    message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {},
    config: { scratchWorkspaceRoot: '/remote-scratch' }, shell: { ...f.client.shell, projects: [] }, scopes: [], error: '', requested: true };
  f.fleet.entries.set(entry.key, entry);
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); f.calls.push(request);
    const value = request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate'] }
      : request.method === 'projects.ensureScratch' ? { projectId: 'remote-scratch' }
        : request.path === '/api/orchestration/shell' ? { snapshotSequence: 1, projects: [{ id: 'remote-scratch', workspaceRoot: '/remote-scratch' }], threads: [] } : {};
    return { ok: true, generation: request.generation ?? 0, value };
  } };
  const target = mobileScratchTarget(f.client, f.fleet);
  expect(await mobileOpenScratch(target, native, f.client, f.fleet, () => true)).toEqual({ environmentId: 'remote', projectId: 'remote-scratch' });
  for (const call of f.calls.filter(call => call.op === 'http' || call.op === 'request')) expect(call).toMatchObject({ fleet: 'remote-key', generation: 7 });
  expect(entry.shell.projects[0]?.id).toBe('remote-scratch'); expect(f.client.projectId).toBe('a');
  expect(f.client.shell.projects.some(project => project.id === 'remote-scratch')).toBe(false);
});

test('changing selected project while scratch authorization waits prevents the create write', async () => {
  const f = await fixture(); scratchNative(f); const original = f.native.later;
  let release!: () => void, entered!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; }), started = new Promise<void>(resolve => { entered = resolve; });
  f.native.later = async input => { if (obj(input).path === '/api/auth/session') { entered(); await gate; } return original(input); };
  const chooser = f.snapshot('/new'), pending = f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet));
  await started; f.client.projectId = 'b'; release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.some(call => call.method === 'projects.ensureScratch')).toBe(false);
  expect(f.client.projectId).toBe('b'); expect(f.client.draft).toBe('B original');
});

test('scratch branch routes redirect and scratch environment changes cannot strand attachment owners', async () => {
  const f = await fixture(); scratchNative(f); const chooser = f.snapshot('/new');
  await f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet));
  expect(f.snapshot('/new/draft/branch', 'branch')).toMatchObject({ ready: false, nextLocation: '/new/draft' });
  f.snapshot('/new/draft/environment', 'environment');
  f.client.local.snapshotDrafts[f.client.draftKey] = [{ id: 'owned-file', name: 'keep.txt', type: 'file' }];
  const before = f.calls.length;
  expect((await f.action(chooser.owner, 'environment', 'other', '', 'environment')).message).toContain('attachments');
  expect(f.calls).toHaveLength(before); expect(f.client.snapshotDrafts[0]?.id).toBe('owned-file');
});

test('scratch Run on refuses an occupied destination after real remote ensure without overwriting either draft', async () => {
  const f = await fixture(); scratchNative(f); const chooser = f.snapshot('/new');
  await f.action(chooser.owner, 'scratch', mobileScratchTarget(f.client, f.fleet));
  f.client.local.drafts[f.client.draftKey] = 'Source draft';
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const,
    message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {}, config: { scratchWorkspaceRoot: '/remote-scratch' },
    shell: { ...f.client.shell, projects: [{ id: 'remote-scratch', workspaceRoot: '/remote-scratch' }] }, scopes: [], error: '', requested: true };
  f.fleet.entries.set(entry.key, entry); f.client.local.drafts['remote:new:remote-scratch'] = 'Destination draft';
  const original = f.native.later;
  f.native.later = async input => {
    const request = obj(input);
    if (request.fleet === 'remote-key') { f.calls.push(request); return { ok: true, generation: 7,
      value: request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate'] } : { projectId: 'remote-scratch' } }; }
    return original(input);
  };
  f.snapshot('/new/draft/environment', 'environment');
  expect((await f.action(chooser.owner, 'environment', 'remote', '', 'environment')).message).toContain('already has a saved draft');
  expect(f.calls.some(call => call.method === 'projects.ensureScratch' && call.fleet === 'remote-key')).toBe(true);
  expect(f.calls.some(call => call.op === 'connect' || call.op === 'fleetStop')).toBe(false);
  expect(f.client.draft).toBe('Source draft'); expect(f.client.local.drafts['remote:new:remote-scratch']).toBe('Destination draft');
  expect(f.client.environmentId).toBe('env');
});

test('scratch shared move reduces before any command preamble and restores on route loss during fleet stop', async () => {
  const f = await fixture(); scratchNative(f);
  f.client.projectId = 'scratch'; f.client.shell.projects.push({ id: 'scratch', workspaceRoot: '/scratch' });
  f.client.local.drafts[f.client.draftKey] = 'Keep source';
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const,
    message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {}, config: { scratchWorkspaceRoot: '/remote-scratch' },
    shell: { ...f.client.shell, projects: [{ id: 'remote-scratch', workspaceRoot: '/remote-scratch' }] }, scopes: [], error: '', requested: true };
  f.fleet.entries.set(entry.key, entry);
  let active = true, release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); f.calls.push(request); if (request.op === 'fleetStop') await gate;
    return { ok: true, generation: f.client.generation, value: {} };
  } };
  const pending = mobileMoveScratch('remote', native, f.client, f.fleet, () => active);
  expect(f.client.local.drafts['remote:new:remote-scratch']).toBe('Keep source');
  expect(f.client.local.drafts['env:new:scratch']).toBeUndefined();
  expect(f.calls.map(call => call.op)).toEqual(['fleetStop']);
  active = false; release();
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.some(call => call.op === 'connect')).toBe(false);
  expect(f.client.local.drafts['env:new:scratch']).toBe('Keep source');
  expect(f.client.local.drafts['remote:new:remote-scratch']).toBeUndefined();
});

async function scratchRollbackFixture(connectFailure = false) {
  const f = await fixture(); scratchNative(f);
  f.client.projectId = 'scratch'; f.client.shell.projects.push({ id: 'scratch', workspaceRoot: '/scratch' });
  const from = 'env:new:scratch', to = 'remote:new:remote-scratch';
  f.client.local.drafts[from] = 'Original source';
  const context = { envMode: 'local', branch: '', worktreePath: '' };
  f.client.local.composerControls.contexts[from] = context;
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const,
    message: '', traceId: '', generation: 7, synchronized: 7, lastEvent: 0, subscriptions: {}, config: { scratchWorkspaceRoot: '/remote-scratch' },
    shell: { ...f.client.shell, projects: [{ id: 'remote-scratch', workspaceRoot: '/remote-scratch' }] }, scopes: [], error: '', requested: true };
  f.fleet.entries.set(entry.key, entry);
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); f.calls.push(request); if (request.op === 'fleetStop') await gate;
    if (request.op === 'connect' && connectFailure) return { ok: false, generation: f.client.generation, error: { kind: 'denied', message: 'Connect refused' } };
    return { ok: true, generation: f.client.generation, value: {} };
  } };
  const pending = mobileMoveScratch('remote', native, f.client, f.fleet, () => true);
  return { ...f, from, to, context, pending, release };
}

test('scratch rollback restores captured slots when another project becomes selected', async () => {
  const f = await scratchRollbackFixture();
  f.client.projectId = 'b'; f.release();
  await expect(f.pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.local.drafts[f.from]).toBe('Original source'); expect(f.client.local.drafts[f.to]).toBeUndefined();
  expect(f.client.local.composerControls.contexts[f.from]).toBe(f.context);
  expect(f.client.local.composerControls.contexts[f.to]).toBeUndefined();
  expect(f.client.local.selections.remote).toBeUndefined(); expect(f.client.draft).toBe('B original');
  expect(f.calls.some(call => call.op === 'connect')).toBe(false);
});

test('scratch captured-slot rollback preserves later source and destination text/context/selection writes', async () => {
  const f = await scratchRollbackFixture();
  f.client.projectId = 'b';
  f.client.local.drafts[f.from] = 'New source edit'; f.client.local.drafts[f.to] = 'New destination edit';
  const context = { envMode: 'local', branch: 'newer', worktreePath: '' };
  f.client.local.composerControls.contexts[f.to] = context;
  f.client.local.selections.remote = { projectId: 'newer-project', threadId: 'newer-thread' };
  f.release(); await expect(f.pending).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.local.drafts[f.from]).toBe('New source edit'); expect(f.client.local.drafts[f.to]).toBe('New destination edit');
  expect(f.client.local.composerControls.contexts[f.to]).toBe(context);
  expect(f.client.local.composerControls.contexts[f.from]).toBe(f.context);
  expect(f.client.local.selections.remote).toEqual({ projectId: 'newer-project', threadId: 'newer-thread' });
  expect(f.client.draft).toBe('B original');
});

test('a refused Connect reply also rolls back by captured cells without clobbering later edits', async () => {
  const f = await scratchRollbackFixture(true);
  f.client.local.drafts[f.from] = 'Edited source during wait';
  f.client.local.drafts[f.to] = 'Edited destination during wait';
  const context = { envMode: 'local', branch: 'later', worktreePath: '/later' };
  f.client.local.composerControls.contexts[f.from] = context;
  const selected = { projectId: 'other', threadId: '' }; f.client.local.selections.remote = selected;
  f.release(); await expect(f.pending).rejects.toMatchObject({ kind: 'denied', message: 'Connect refused' });
  expect(f.client.local.drafts[f.from]).toBe('Edited source during wait');
  expect(f.client.local.drafts[f.to]).toBe('Edited destination during wait');
  expect(f.client.local.composerControls.contexts[f.from]).toBe(context);
  expect(f.client.local.selections.remote).toBe(selected);
});


test('New Task keyboard offset uses an explicit docked guide fact, never focus-like values', () => {
  expect(mobileLayoutFacts('').keyboardDocked).toBe(false);
  expect(mobileLayoutFacts('{"keyboardDocked":true}').keyboardDocked).toBe(true);
  for (const value of [false, null, 1, "true", {}, []]) {
    expect(mobileLayoutFacts(JSON.stringify({ keyboardDocked: value })).keyboardDocked).toBe(false);
  }
  expect(mobileLayoutFacts('{"safeTop":-1,"safeBottom":34,"keyboardDocked":true,"liquidGlass":true}'))
    .toEqual({ safeTop: 0, safeBottom: 34, liquidGlass: true, keyboardDocked: true });
  expect(mobileLayoutFacts('{"focused":true,"height":300}').keyboardDocked).toBe(false);
});
