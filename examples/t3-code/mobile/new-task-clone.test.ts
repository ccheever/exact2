import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { liveEvent } from './shared/live-streams';
import type { Native, Files } from './shared/protocol';
import { liveEnvironment } from './shared/live-streams';
import { mobileNewTaskCloneObserve as observe, mobileNewTaskCloneSnapshot as snapshot,
  mobileNewTaskCloneBlocks as blocks, mobileNewTaskCloneAction as action } from './new-task-clone';
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
function fixture(hook?: (request: Obj) => unknown | Promise<unknown>) {
  const client = new T3Client(), calls: Obj[] = []; let serial = 0;
  Object.assign(client, { origin: 'https://clone.test', environmentId: 'env', projectId: 'project', connection: 'connected',
    generation: 4, configLive: true, shellLive: true, shellLoaded: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.config = { environment: { capabilities: { projectCloneTracking: true } } };
  client.shell.projects = [{ id: 'project', title: 'Repo', workspaceRoot: '/work/repo' }, { id: 'another', title: 'Other', workspaceRoot: '/other' }];
  client.local.drafts[client.draftKey] = 'Keep this draft';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); const override = await hook?.(request); if (override !== undefined) return override;
    const value = request.op === 'ids' ? [`command-${++serial}`] : request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate', 'source-control:write'] }
      : request.path === '/api/orchestration/shell' ? { projects: client.shell.projects.filter(project => project.id !== 'project'), threads: [], snapshotSequence: 2 } : {};
    return { ok: true, generation: 4, value };
  } };
  const clone = (phase: string, extra: Obj = {}) => ({ projectId: 'project', phase, stage: 'receiving', percent: 45, detail: '12.3 MiB', destinationPath: '/work/repo', ...extra });
  const emit = (value: unknown) => liveEvent(client, { key: 'project-clones', subscriptionId: 'clones-1', value: value as Obj });
  const view = (location = '/new/draft?environmentId=env&projectId=project&cloning=1', visit = 'visit', ready = true) => observe('owner', visit, location, ready, client);
  const run = (kind: string) => action('owner', 'visit', kind, native, client);
  return { client, calls, native, clone, emit, view, run };
}

test('known initial clone blocks Start before first event without locking draft text or showing a fake banner', () => {
  const f = fixture(); expect(f.view()).toMatchObject({ blocked: true, pending: true, visible: false, busy: false }); expect(blocks(f.client)).toBe(true);
  f.client.local.drafts[f.client.draftKey] = 'Typed while cloning'; expect(f.client.draft).toBe('Typed while cloning');
  expect(f.calls).toHaveLength(0);
});

test('pending known clone is qualified by environment and project, with first query value semantics', () => {
  const f = fixture(); f.view('/new/draft?environmentId=other&projectId=project&cloning=1'); expect(blocks(f.client)).toBe(false);
  f.view('/new/draft?environmentId=env&projectId=another&cloning=1'); expect(blocks(f.client)).toBe(false);
  f.view('/new/draft?environmentId=env&projectId=project&cloning=0&cloning=1'); expect(blocks(f.client)).toBe(false);
  f.view(); f.client.projectId = 'another'; expect(blocks(f.client)).toBe(false); f.client.projectId = 'project'; expect(blocks(f.client)).toBe(true);
});

test('authoritative empty list and failed subscription differ from the not-yet-loaded list', () => {
  for (const reply of [[], { _transportError: { message: 'Stream disconnected' } }]) {
    const f = fixture(); expect(f.view().pending).toBe(true); f.emit(reply);
    expect(snapshot(f.client)).toMatchObject({ blocked: false, pending: false, visible: false });
  }
  const f = fixture(); f.client.config = {}; expect(f.view().pending).toBe(false); expect(blocks(f.client)).toBe(false);
});

test('running, failed, cancelled and done use the pinned mobile banner text and action set', () => {
  const f = fixture(); f.view();
  f.emit([f.clone('running', { repository: { nameWithOwner: 'owner/repo' } })]);
  expect(snapshot(f.client)).toMatchObject({ blocked: true, visible: true, title: 'Cloning owner/repo', description: 'Receiving objects · 45% · 12.3 MiB', cancel: true, retry: false, remove: false });
  f.emit([f.clone('failed', { error: 'Git authentication failed' })]);
  expect(snapshot(f.client)).toMatchObject({ blocked: true, title: 'Failed to clone repo', error: 'Git authentication failed', cancel: false, retry: true, remove: true });
  f.emit([f.clone('cancelled')]); expect(snapshot(f.client)).toMatchObject({ blocked: true, title: 'Cancelled cloning repo', description: '', error: '', retry: true, remove: true });
  f.emit([f.clone('done')]); expect(snapshot(f.client)).toMatchObject({ blocked: false, pending: false, visible: false, cancel: false, retry: false, remove: false });
});

test('route children retain the initial clone while fresh owner and explicit replacement do not adopt it', () => {
  const f = fixture(); f.view(); f.view('/new/draft/branch', 'branch'); expect(blocks(f.client)).toBe(true);
  f.view(); expect(blocks(f.client)).toBe(true);
  observe('new-owner', 'fresh', '/new/draft', true, f.client); expect(blocks(f.client)).toBe(false);
  f.view(); f.view('/new/draft?environmentId=env&projectId=project'); expect(blocks(f.client)).toBe(false);
  f.client.threadId = 'thread'; f.emit([f.clone('running')]); expect(snapshot(f.client).visible).toBe(false); expect(blocks(f.client)).toBe(false);
});

test('cancel and retry send real project-scoped RPCs with fresh source-control grants', async () => {
  const f = fixture(); f.view(); f.emit([f.clone('running')]); const cancelled = await f.run('cancel'); expect(cancelled.message).toBe('');
  expect(f.calls.find(call => call.method === 'projectClone.cancel')).toMatchObject({ generation: 4, payload: { projectId: 'project' } });
  expect(snapshot(f.client).phase).toBe('running');
  f.emit([f.clone('cancelled')]); await f.run('retry'); expect(f.calls.find(call => call.method === 'projectClone.retry')?.payload).toEqual({ projectId: 'project' });
  expect(f.calls.filter(call => call.path === '/api/auth/session')).toHaveLength(2); expect(f.calls.some(call => call.op === 'subscribe')).toBe(false);
});

test('fresh grant refusal reports the exact action alert and dispatches no clone or project write', async () => {
  const f = fixture(request => request.path === '/api/auth/session' ? { ok: true, generation: 4, value: { authenticated: true, permissions: [] } } : undefined);
  f.view(); f.emit([f.clone('running')]); expect(await f.run('cancel')).toMatchObject({ message: 'This connection cannot change project clones.', alertTitle: 'Failed to cancel clone' });
  f.emit([f.clone('failed')]); expect(await f.run('remove')).toMatchObject({ message: 'This connection cannot remove projects.', alertTitle: 'Failed to remove project' });
  expect(f.calls.some(call => call.op === 'request')).toBe(false);
});

test('successful remove uses a non-force delete and returns Home without selecting another project', async () => {
  const f = fixture(); f.view(); f.emit([f.clone('failed')]);
  expect(await f.run('remove')).toMatchObject({ removed: true, nextLocation: '/', message: '' });
  expect(f.calls.find(call => call.method === 'projects.mutate')?.payload).toEqual({ type: 'project.delete', commandId: 'command-1', projectId: 'project' });
  expect(f.client.projectId).toBe('project'); expect(f.client.shell.projects.map(project => project.id)).toEqual(['another']);
  expect(f.client.local.drafts['env:new:project']).toBe('Keep this draft');
});

test('changing the selected project during delete suppresses Home navigation and further reads', async () => {
  const gate = deferred<unknown>(), started = deferred<void>();
  const f = fixture(request => { if (request.method === 'projects.mutate') { started.resolve(); return gate.promise; } });
  f.view(); f.emit([f.clone('failed')]); const removing = f.run('remove'); await started.promise;
  f.client.projectId = 'another'; gate.resolve({ ok: true, generation: 4, value: {} });
  expect(await removing).toMatchObject({ removed: true, nextLocation: '', message: '' }); expect(f.client.projectId).toBe('another');
  expect(f.calls.some(call => call.path === '/api/orchestration/shell')).toBe(false); expect(snapshot(f.client).busy).toBe(false);
});

test('route exit while checking grants refuses the write, and a double tap holds one in-flight operation', async () => {
  const gate = deferred<unknown>(), started = deferred<void>();
  const f = fixture(request => { if (request.path === '/api/auth/session') { started.resolve(); return gate.promise; } });
  f.view(); f.emit([f.clone('running')]); const cancel = f.run('cancel'); await started.promise; await f.run('cancel');
  expect(f.calls.filter(call => call.path === '/api/auth/session')).toHaveLength(1);
  f.view('/new', 'chooser', false); gate.resolve({ ok: true, generation: 4, value: { authenticated: true, permissions: ['source-control:write'] } });
  await expect(cancel).rejects.toMatchObject({ kind: 'superseded' }); expect(f.calls.some(call => call.method === 'projectClone.cancel')).toBe(false); expect(snapshot(f.client).busy).toBe(false);
});

test('a generation change during grant lookup prevents dispatch; failures are left for the native alert owner', async () => {
  const f = fixture(request => { if (request.path === '/api/auth/session') { f.client.generation++; return { ok: true, generation: 4, value: { authenticated: true, permissions: ['source-control:write'] } }; } });
  f.view(); f.emit([f.clone('running')]); await f.run('cancel'); expect(f.calls.some(call => call.method === 'projectClone.cancel')).toBe(false);
  const failed = fixture(request => request.method === 'projectClone.retry' ? { ok: false, generation: 4, error: { kind: 'server', message: 'Repository denied', uncertain: false } } : undefined);
  failed.view(); failed.emit([failed.clone('failed')]); expect(await failed.run('retry')).toMatchObject({ message: 'Repository denied', alertTitle: 'Failed to retry clone' });
});

for (const phase of ['running', 'failed']) for (const streamFailed of [false, true]) {
  test(`actual sender ${streamFailed ? 'defers to the server after a failed stream' : 'refuses a tracked clone'} following ${phase}`, async () => {
    const f = fixture(request => request.op === 'ids'
      ? { ok: true, generation: 4, value: Array.from({ length: Number(request.count) }, (_, index) => `id-${index}`) }
      : request.method === 'orchestration.launchThread'
        ? { ok: false, generation: 4, error: { kind: 'server', message: 'Server admission reached', uncertain: false } } : undefined);
    f.client.providerId = 'provider'; f.client.modelId = 'model';
    f.client.config = { environment: { capabilities: { projectCloneTracking: true, serverResolvedCommandContext: true } }, providers: [
      { instanceId: 'provider', driver: 'codex', enabled: true, installed: true, status: 'ready', models: [{ slug: 'model', name: 'Model' }] }] };
    const storage: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
    f.view(); f.emit([f.clone(phase)]);
    if (streamFailed) f.emit({ _transportError: { message: 'Stream disconnected' } });
    expect(blocks(f.client)).toBe(!streamFailed);
    const reply = await f.client.command('send', '', '', 0, f.native, storage);
    const launches = f.calls.filter(call => call.method === 'orchestration.launchThread');
    if (streamFailed) {
      expect(reply.message).toBe('Server admission reached'); expect(launches).toHaveLength(1);
      expect(obj(obj(launches[0]!.payload).initialMessage).text).toBe('Keep this draft');
    } else { expect(reply.message).toContain('Send once the repository is cloned.'); expect(launches).toHaveLength(0); }
    expect(f.client.draft).toBe('Keep this draft');
    expect(liveEnvironment(f.client, null, 'env')?.clones.value?.[0]?.phase).toBe(phase);
  });
}
