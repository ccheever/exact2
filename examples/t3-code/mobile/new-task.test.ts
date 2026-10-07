import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { draftContext } from './shared/composer-controls-branch';
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
  expect(changed.needsPrepare).toBe(true); expect(owns(changed.owner, 'visit', f.client)).toBe(false);
  expect((await f.action(changed.owner, 'draft', '', 'wrong owner')).message).toContain('ready');
  expect(f.client.local.drafts['env:new:a']).toBe('A original');
  const settings = f.snapshot('/new/draft/settings/runtime', 'runtime');
  expect(settings.needsPrepare).toBe(true); expect(owns(settings.owner, 'runtime', f.client)).toBe(false);
  await f.action(settings.owner, 'prepare', '', '', 'runtime');
  expect(f.snapshot('/new/draft/settings/runtime', 'runtime').ready).toBe(true);
});
