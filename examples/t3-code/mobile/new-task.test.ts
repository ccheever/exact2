import { noteNow } from './shared/composer-controls';
import { liveEvent } from './shared/live-streams';
import { mobileNewTaskCloneObserve } from './new-task-clone';
import { mobileLayoutFacts } from './root-presentation';
import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTask } from './new-task';
import { mobileNewTaskFileSnapshot, mobileNewTaskFileRead } from './new-task-file';
import { mobileAppLink } from './navigation-links';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { draftContext } from './shared/composer-controls-branch';
import { mobileComposerTarget } from './composer-target';
import { mobileComposerSettingsAction } from './composer-settings';
import { mobileNewTaskFlowView as view, mobileNewTaskFlowAction as act, mobileNewTaskFlowOwns as owns, mobileNewTaskRoute } from './new-task-flow';

async function fixture() {
  const client = new MobileDraftClient(), fleet = new EnvironmentFleet(), calls: Obj[] = [];
  const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('not saved'); }, async atomicWriteFile() {} } };
  await client.command('dismiss-error', '', '', 0, { available: true, watch() {}, async later() { return { ok: true, generation: 0, value: {} }; } }, files);
  Object.assign(client, { environmentId: 'env', origin: 'https://example.test', projectId: 'a', connection: 'connected',
    configLive: true, shellLive: true, shellLoaded: true, threadLive: true, scopes: ['orchestration:operate'] });
  noteNow(client, 1791420000000);
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, providers: [] };
  client.shell.projects = [{ id: 'a', title: 'A', workspaceRoot: '/a' }, { id: 'b', title: 'B', workspaceRoot: '/b' }];
  client.local.drafts['env:new:a'] = 'A original'; client.local.drafts['env:new:b'] = 'B original';
  let serial = 0;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'mobileOutbox') return { ok: true, generation: client.generation, value: request.action === 'read'
      ? { ownerEpoch: 'epoch', sequenceFloor: 0, complete: true, errors: [], records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers: [] }
      : { complete: true, fingerprint: null, claims: [] } };
    const value = request.op === 'ids' ? Array.from({length: Number(request.count)}, () => `id-${++serial}`) : request.op === 'http' ? { authenticated: true, permissions: ['source-control:write'] }
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
  expect(f.client.projectId).toBe('b'); expect(f.client.draft).toBe(''); expect(f.client.local.drafts['env:new:a']).toBe('A original');
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

test('unknown draft and unavailable outbox/share identity never fall through to an existing project', async () => {
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
  expect(f.client.draft).toBe(''); expect(f.snapshot('/new/draft?environmentId=env&projectId=b&branch=feature').ready).toBe(true);
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
  const draft = f.snapshot('/new/draft'), key = f.client.draftKey; let release!: () => void, entered!: () => void, writes = 0;
  const started = new Promise<void>(resolve => { entered = resolve; }), gate = new Promise<void>(resolve => { release = resolve; });
  f.files.fs.atomicWriteFile = async () => { if (++writes === 1) { entered(); await gate; } };
  const edit = f.action(draft.owner, 'draft', '', 'A changed'); await started;
  const latest = f.action(draft.owner, 'draft', '', 'A latest'); await latest;
  f.client.projectId = 'b'; release(); await edit;
  expect(f.client.local.drafts[key]).toBe('A latest'); expect(f.client.local.drafts['env:new:a']).toBe('A original'); expect(f.client.draft).toBe('A latest');
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
  expect(f.snapshot('/new/draft', 'draft').ready).toBe(true); expect(f.client.draft).toBe('');
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
  expect(f.snapshot(url).ready).toBe(true); expect(f.client.draft).toBe('');
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
  expect((await f.action(chooser.owner, 'environment', 'other', '', 'environment')).message).toContain('no longer available');
  expect(f.calls.slice(before).map(call => [call.op, call.action])).toEqual([['mobileOutbox', 'read'], ['mobileOutbox', 'transferLookup']]);
  expect(f.client.snapshotDrafts[0]?.id).toBe('owned-file');
});

test('scratch Run on attempts the actual connection without treating legacy content as a collision', async () => {
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
  expect((await f.action(chooser.owner, 'environment', 'remote', '', 'environment')).message).not.toContain('already has a saved draft');
  expect(f.calls.some(call => call.method === 'projects.ensureScratch' && call.fleet === 'remote-key')).toBe(true);
  expect(f.calls.some(call => call.op === 'connect')).toBe(true);
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

function modelConfig(client: T3Client) {
  client.config = { environment: { capabilities: { serverResolvedCommandContext: true } }, settings: { projectSettingsFolded: true, defaultModelSelection: null,
    providerInstances: { codex: { driver: 'codex', enabled: true, config: {} } },
    projectSettingsOverrides: { a: { defaultModelSelection: { instanceId: 'codex', model: 'configured', options: [{ id: 'reasoning', value: 'high' }] } } } },
    providers: [{ instanceId: 'codex', driver: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' },
      models: [{ slug: 'catalog', name: 'Catalog', isDefault: true }] }] };
}

test('actual New Task flow preserves configured noncatalog model and options through default recalculation', async () => {
  const f = await fixture(); modelConfig(f.client);
  f.client.local.composerControls.stickyProvider = 'codex';
  f.client.local.composerControls.stickyByProvider.codex = { model: 'catalog', options: [] };
  const chooser = f.snapshot('/new');
  expect(await f.action(chooser.owner, 'project', '["env","a"]')).toMatchObject({ message: '', submitted: false });
  f.client.local.drafts[f.client.draftKey] = 'Fresh content';
  expect(f.client.providerId).toBe('codex'); expect(f.client.modelId).toBe('configured');
  expect(f.client.modelOptions).toEqual([{ id: 'reasoning', value: 'high' }]);
  expect(mobileNewTask('', f.client, f.fleet).composer).toMatchObject({ modelLabel: 'configured', canSend: true });
  f.client.chooseDefaults();
  expect(f.client.modelId).toBe('configured'); expect(f.client.modelOptions).toEqual([{ id: 'reasoning', value: 'high' }]);
  expect(f.snapshot('/new/draft', 'draft').ready).toBe(true);
  expect(f.client.draft).toBe('Fresh content');
  expect(f.calls.some(call => ['orchestration.launchThread', 'orchestration.dispatchCommand'].includes(String(call.method)))).toBe(false);
  f.client.scopes = [];
  expect(mobileNewTask('', f.client, f.fleet).composer.canSend).toBe(false);
});

test('fresh defaults respect folded projects, explicit null and provider disable flags', async () => {
  const f = await fixture(); modelConfig(f.client);
  const settings = obj(f.client.config.settings), overrides = obj(settings.projectSettingsOverrides);
  f.client.shell.projects[0]!.defaultModelSelection = { instanceId: 'codex', model: 'legacy-project' };
  settings.defaultModelSelection = { instanceId: 'codex', model: 'environment' };
  overrides.a = {}; f.client.chooseDefaults(); expect(f.client.modelId).toBe('environment');
  settings.projectSettingsFolded = false;
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('legacy-project');
  overrides.a = { defaultModelSelection: null };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('catalog');
  overrides.a = { defaultModelSelection: undefined };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('environment');
  overrides.a = { defaultModelSelection: { instanceId: 'codex', model: 'project' } };
  obj(obj(settings.providerInstances).codex).config = { enabled: false };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('environment');
});

test('sticky noncatalog choices retain options after implicit legacy defaults fall through', async () => {
  const f = await fixture(); modelConfig(f.client);
  const provider = obj((f.client.config.providers as Obj[])[0]);
  (provider.models as Obj[]).push({ slug: 'configured', name: 'Legacy', isLegacy: true });
  f.client.local.composerControls.stickyProvider = 'codex';
  f.client.local.composerControls.stickyByProvider.codex = { model: 'remembered', options: [{ id: 'fast', value: true }] };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('remembered');
  expect(f.client.modelOptions).toEqual([{ id: 'fast', value: true }]);
  provider.auth = { status: 'unauthenticated' };
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('');
  expect(mobileNewTask('', f.client, f.fleet).composer).toMatchObject({ modelLabel: 'Choose model', canSend: false });
});

test('Antigravity keeps unavailable configured choice while send stays blocked', async () => {
  const f = await fixture(); modelConfig(f.client);
  obj(obj(f.client.config.settings).projectSettingsOverrides).a = { defaultModelSelection: { instanceId: 'anti', model: 'retained' } };
  obj(obj(f.client.config.settings).providerInstances).anti = { driver: 'antigravity', enabled: true };
  f.client.chooseDefaults(); expect(f.client.providerId).toBe('anti'); expect(f.client.modelId).toBe('retained');
  expect(mobileNewTask('', f.client, f.fleet).composer).toMatchObject({ modelLabel: 'retained', modelUnavailable: true, canSend: false });
});

test('automatic choice follows catalog order and defaults without treating model display as send readiness', async () => {
  const f = await fixture(); modelConfig(f.client);
  obj(obj(f.client.config.settings).projectSettingsOverrides).a = {};
  const provider = obj((f.client.config.providers as Obj[])[0]);
  provider.status = 'error'; provider.models = [{ slug: 'first', name: 'First' }, { slug: 'default', name: 'Default', isDefault: true }];
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('default');
  expect(mobileNewTask('', f.client, f.fleet).composer.canSend).toBe(false);
  provider.models = [{ slug: 'first', name: 'First' }];
  f.client.chooseDefaults(); expect(f.client.modelId).toBe('first');
  provider.enabled = false; f.client.chooseDefaults(); expect(f.client.modelId).toBe('');
});


test('draft file reads use explicit worktree cwd and shared source rendering', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/src%2Fa.ts?environmentId=env&cwd=%2Fworktree&projectName=Worktree&line=2';
  const flow = f.snapshot(location, 'file'), calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: f.client.generation, value: request.op === 'http'
      ? { authenticated: true, permissions: ['filesystem:read'] } : { contents: 'one\r\n\ttwo', truncated: true } };
  } };
  const file = await mobileNewTaskFileRead('src/a.ts', location, 'file', flow.owner, false, native, f.client);
  expect(file).toMatchObject({ title: 'a.ts', subtitle: 'Worktree · src', contents: 'one\n\ttwo', truncated: true, initialRowId: 'source-line:1' });
  expect(file.rows[1]).toMatchObject({ text: '    two', selected: true });
  expect(calls.find(call => call.method === 'projects.readFile')?.payload).toEqual({ cwd: '/worktree', relativePath: 'src/a.ts' });
  expect(f.client.projectId).toBe('a'); expect(f.client.threadId).toBe(''); expect(f.client.draft).toBe('');
});

test('draft file permission downgrade clears content and cross-environment routes never read', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file'); let allowed = true, reads = 0;
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); if (request.method) reads++;
    return { ok: true, generation: f.client.generation, value: request.op === 'http'
      ? { authenticated: true, permissions: allowed ? ['filesystem:read'] : [], scopes: ['filesystem:read'] } : { contents: 'private' } };
  } };
  expect((await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client)).contents).toBe('private');
  allowed = false;
  const denied = await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client);
  expect(denied.contents).toBe(''); expect(denied.error).toContain('cannot read'); expect(reads).toBe(1);
  const foreign = location + '?environmentId=other&cwd=/private'; f.snapshot(foreign, 'foreign');
  await mobileNewTaskFileRead('a.ts', foreign, 'foreign', flow.owner, false, native, f.client);
  expect(reads).toBe(1);
});

test('draft file replies cannot cross a route change or a newer request', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file'); let release!: (reply: unknown) => void;
  const slow: Native = { available: true, watch() {}, async later() { return new Promise(resolve => { release = resolve; }); } };
  const old = mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, slow, f.client);
  f.snapshot('/new/draft', 'draft');
  release({ ok: true, generation: f.client.generation, value: { authenticated: true, permissions: ['filesystem:read'] } });
  await expect(old).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('');
  f.snapshot(location, 'file');
  const older = mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, slow, f.client);
  const ready: Native = { available: true, watch() {}, async later(input) {
    return { ok: true, generation: f.client.generation, value: obj(input).op === 'http'
      ? { authenticated: true, permissions: ['filesystem:read'] } : { contents: 'newest' } };
  } };
  await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, ready, f.client);
  release({ ok: true, generation: f.client.generation, value: { authenticated: true, permissions: ['filesystem:read'] } });
  await expect(older).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('newest');
});


test('draft file query replacement invalidates the old read without clearing the current cache', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const oldLocation = '/new/draft/files/a.ts?cwd=/old', location = '/new/draft/files/a.ts?cwd=/new';
  const flow = f.snapshot(oldLocation, 'file'); let release!: (reply: unknown) => void;
  const slow: Native = { available: true, watch() {}, async later() { return new Promise(resolve => { release = resolve; }); } };
  const pending = mobileNewTaskFileRead('a.ts', oldLocation, 'file', flow.owner, false, slow, f.client);
  f.snapshot(location, 'file');
  const ready: Native = { available: true, watch() {}, async later(input) {
    return { ok: true, generation: f.client.generation, value: obj(input).op === 'http'
      ? { authenticated: true, permissions: ['filesystem:read'] } : { contents: 'current workspace' } };
  } };
  await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, ready, f.client);
  release({ ok: true, generation: f.client.generation, value: { authenticated: true, permissions: ['filesystem:read'] } });
  await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
  await expect(mobileNewTaskFileRead('a.ts', oldLocation, 'file', flow.owner, false, ready, f.client)).rejects.toMatchObject({ kind: 'superseded' });
  expect(mobileNewTaskFileSnapshot('a.ts', oldLocation, 'file', flow.owner, false, f.client).contents).toBe('');
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('current workspace');
  expect(f.client.draft).toBe('');
});


function fileNative(f: Awaited<ReturnType<typeof fixture>>, calls: Obj[], content = 'answer') {
  return { available: true, watch() {}, async later(input: unknown) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: f.client.generation, value: request.op === 'http'
      ? { authenticated: true, permissions: ['filesystem:read'] } : { contents: content } };
  } };
}
test('draft file legal raw question mark must not erase a later environmentId', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = mobileAppLink('/new/draft/files/a.ts?cwd=/work?tree&environmentId=other', 'file').location;
  const flow = f.snapshot(location, 'file'), calls: Obj[] = [];
  expect(new URLSearchParams(location.slice(location.indexOf('?') + 1)).get('environmentId')).toBe('other');
  await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, fileNative(f, calls), f.client);
  expect(calls.filter(call => call.method === 'projects.readFile')).toHaveLength(0);
});
test('draft file valid trailing space in Unix cwd must survive route decoding', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts?cwd=%2Fwork%20&environmentId=env', flow = f.snapshot(location, 'file'), calls: Obj[] = [];
  await mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, fileNative(f, calls), f.client);
  expect(calls.find(call => call.method === 'projects.readFile')?.payload).toEqual({ cwd: '/work ', relativePath: 'a.ts' });
});
test('draft file read RPC completion after route exit is refused', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file');
  let release!: (reply: unknown) => void, reached!: () => void;
  const requested = new Promise<void>(resolve => { reached = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    if (obj(input).op === 'http') return {ok: true, generation: f.client.generation, value: {authenticated: true, permissions: ['filesystem:read']}};
    reached(); return new Promise(resolve => { release = resolve; });
  }};
  const reading = mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client);
  await requested; f.snapshot('/new/draft', 'draft');
  release({ok: true, generation: f.client.generation, value: {contents: 'old'}});
  await expect(reading).rejects.toMatchObject({kind: 'superseded'});
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('');
});
test('draft file read RPC completion after reconnect is refused', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file');
  let release!: (reply: unknown) => void, reached!: () => void;
  const requested = new Promise<void>(resolve => { reached = resolve; });
  const native: Native = { available: true, watch() {}, async later(input) {
    if (obj(input).op === 'http') return {ok: true, generation: f.client.generation, value: {authenticated: true, permissions: ['filesystem:read']}};
    reached(); return new Promise(resolve => { release = resolve; });
  }};
  const reading = mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client);
  await requested; f.client.generation++;
  release({ok: true, generation: f.client.generation - 1, value: {contents: 'old'}});
  await expect(reading).rejects.toMatchObject({kind: 'superseded'});
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client).contents).toBe('');
});
test('draft file runtime abandonment produces no content or sticky spinner', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const location = '/new/draft/files/a.ts', flow = f.snapshot(location, 'file');
  const native: Native = {available: true, watch() {}, async later() {throw {name:'FetchError',kind:'Aborted'};}};
  await expect(mobileNewTaskFileRead('a.ts', location, 'file', flow.owner, false, native, f.client)).rejects.toMatchObject({kind:'superseded'});
  expect(mobileNewTaskFileSnapshot('a.ts', location, 'file', flow.owner, false, f.client)).toMatchObject({contents:'', loading:false, error:''});
});


test('Add Project routes retain the containing draft while refusing composer actions', async () => {
  const f = await fixture(), chooser = f.snapshot('/new');
  await f.action(chooser.owner, 'project', '["env","b"]');
  const draft = f.snapshot('/new/draft', 'draft');
  for (const suffix of ['', '/repository', '/destination', '/local', '/new']) {
    const added = f.snapshot(`/new/add-project${suffix}?environmentId=env`, 'add');
    expect(added).toMatchObject({ owner: draft.owner, status: 'add-project', ready: false, needsPrepare: false, nextLocation: '', draftOwner: draft.draftOwner });
    expect(owns(draft.owner, 'add', f.client)).toBe(false);
    const calls = f.calls.length;
    for (const kind of ['draft', 'project', 'scratch', 'prepare']) {
      expect((await f.action(added.owner, kind, '["env","a"]', 'changed', 'add')).message).toContain('Return to the new task');
    }
    expect(f.calls).toHaveLength(calls);
    expect(f.client.projectId).toBe('b'); expect(f.client.draft).toBe('');
    expect(f.snapshot('/new/draft', 'draft')).toMatchObject({ owner: draft.owner, ready: true, draftOwner: draft.draftOwner });
  }
  expect(mobileNewTaskRoute('/new/add-project/unknown').context).toBe('');
});


test('chooser Add Project uses connection availability and retains unfiltered project presence', async () => {
  const f = await fixture();
  expect(mobileNewTask('unmatched', f.client, f.fleet)).toMatchObject({ canAddProject: true, hasProjects: true, projects: [] });
  f.client.connection = 'disconnected';
  expect(mobileNewTask('', f.client, f.fleet).canAddProject).toBe(false);
  f.client.shell.projects = [];
  expect(mobileNewTask('', f.client, f.fleet)).toMatchObject({ canAddProject: false, hasProjects: false });
});


test('initial clone draft admits route but guards actual Send until authoritative stream answer', async () => {
  const f = await fixture();
  f.client.config = { ...f.client.config, environment: { capabilities: { projectCloneTracking: true, serverResolvedCommandContext: true } } };
  const location = '/new/draft?environmentId=env&projectId=a&cloning=1';
  expect(mobileNewTaskRoute(location).unsupported).toBe('');
  let flow = f.snapshot(location);
  await f.action(flow.owner, 'prepare'); flow = f.snapshot(location);
  expect(flow.ready).toBe(true);
  mobileNewTaskCloneObserve(flow.owner, 'visit', location, true, f.client);
  expect(mobileNewTask('', f.client, f.fleet).composer).toMatchObject({ canSend: false, blockedReason: 'Cloning repository' });
  f.calls.length = 0;
  expect(await f.action(flow.owner, 'send')).toMatchObject({ message: 'Cloning repository', submitted: false });
  expect(f.calls).toHaveLength(0); expect(f.client.draft).toBe('');
  liveEvent(f.client, { key: 'project-clones', subscriptionId: 'clone-1', value: [{ projectId: 'a', phase: 'failed' }] });
  expect(await f.action(flow.owner, 'send')).toMatchObject({ message: 'Repository not cloned', submitted: false });
  expect(f.calls).toHaveLength(0);
  liveEvent(f.client, { key: 'project-clones', subscriptionId: 'clone-1', value: [] });
  expect(mobileNewTask('', f.client, f.fleet).composer.blockedReason).not.toBe('Cloning repository');
});


test('acknowledged clone removal survives shell fallback without racing the draft redirect', async () => {
  for (const leave of [false, true, 'same-visit']) {
    const f = await fixture(), location = '/new/draft?environmentId=env&projectId=a&cloning=1';
    f.client.config = { environment: { capabilities: { projectCloneTracking: true, serverResolvedCommandContext: true } }, providers: [] };
    let flow = f.snapshot(location); await f.action(flow.owner, 'prepare'); flow = f.snapshot(location);
    liveEvent(f.client, { key: 'project-clones', subscriptionId: 'clone-1', value: [{ projectId: 'a', phase: 'failed' }] });
    const previous = f.native.later;
    f.native.later = async input => {
      const request = obj(input);
      if (request.path === '/api/auth/session') return { ok: true, generation: f.client.generation,
        value: { authenticated: true, permissions: ['orchestration:operate'] } };
      if (request.op === 'ids') return { ok: true, generation: f.client.generation, value: ['delete-project'] };
      if (request.method === 'projects.mutate') {
        f.client.shell.projects = f.client.shell.projects.filter(project => project.id !== 'a'); f.client.projectId = 'b';
        const during = f.snapshot(leave ? '/new' : location, leave === true ? 'other' : 'visit');
        if (!leave) expect(during).toMatchObject({ busy: true, nextLocation: '' });
        mobileNewTaskCloneObserve(during.owner, during.requestRoute, leave ? '/new' : location, during.ready || during.busy, f.client);
        return { ok: true, generation: f.client.generation, value: {} };
      }
      return previous(input);
    };
    const removed = await f.action(flow.owner, 'clone-remove', 'a', 'env');
    expect(removed).toMatchObject({ nextLocation: leave ? '' : '/', message: '' });
    // Exact mutation result records are closed. The helper-only removed flag must not leak.
    expect(Object.keys(removed).sort()).toEqual(['alertTitle', 'environmentId', 'message', 'nextLocation', 'projectId', 'requestRoute', 'revision', 'submitted', 'threadId']);
    expect(f.client.local.drafts['env:new:a']).toBe('A original');
  }
});


import { mobileNewTaskAction } from './new-task';
import { mobileNewTaskDraftPresentation, mobileNewTaskDraftSelectedBranch } from './mobile-new-task-drafts';
test('actual branch picker distinguishes untouched checkout from selecting that same branch', async () => {
  const f = await fixture(), chooser = f.snapshot('/new'); await f.action(chooser.owner, 'project', '["env","a"]');
  const original = f.native.later;
  f.native.later = async input => {
    const call = obj(input);
    if (call.method === 'vcs.refreshStatus') return { ok: true, generation: f.client.generation, value: { isRepo: true, refName: 'main' } };
    if (call.method === 'vcs.listRefs') return { ok: true, generation: f.client.generation, value: { refs: [{ name: 'main', current: true, isDefault: true }], total: 1, nextCursor: null } };
    return original(input);
  };
  await mobileNewTaskPrepare('', f.native, f.client);
  expect(draftContext(f.client).branch).toBe('main');
  expect(mobileNewTaskDraftSelectedBranch(mobileNewTaskDraftPresentation(f.client, f.client.draftKey)!)).toBeNull();
  expect((await mobileNewTaskAction('branch', 'main', '', f.native, f.files, f.client, f.fleet)).message).toBe('');
  expect(mobileNewTaskDraftSelectedBranch(mobileNewTaskDraftPresentation(f.client, f.client.draftKey)!)).toBe('main');
  await mobileNewTaskPrepare('', f.native, f.client);
  expect(mobileNewTaskDraftSelectedBranch(mobileNewTaskDraftPresentation(f.client, f.client.draftKey)!)).toBe('main');
});
