import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import type { Native, Files } from './shared/protocol';
import { obj, type Obj } from './shared/domain';
import { EnvironmentFleet, environmentKey, type FleetEntry } from './shared/settings-b-fleet';
import { mobileNewTaskFileRead, mobileNewTaskFileSnapshot } from './new-task-file';
import { mobileNewTaskFlowView, mobileNewTaskFlowAction, mobileNewTaskFlowOwns, mobileNewTaskFileRouteCurrent } from './new-task-flow';

function fixture(target = 'focused') {
  const client = new T3Client(), background = new EnvironmentFleet(), calls: Obj[] = [];
  Object.assign(client, { origin: 'https://focused.test', environmentId: 'focused', generation: 7, connection: 'connected' });
  const key = environmentKey('https://remote.test', 'remote');
  const entry: FleetEntry = { key, origin: 'https://remote.test', environmentId: 'remote', phase: 'connected', generation: 19,
    synchronized: 19, message: '', traceId: '', lastEvent: 0, subscriptions: {}, config: {}, shell: new T3Client().shell, scopes: [], error: '', requested: true };
  background.entries.set(key, entry);
  background.saved = [{ origin: client.origin, environmentId: 'focused', enabled: true }, { origin: entry.origin, environmentId: 'remote', enabled: true }];
  const location = `/new/draft/files/src%2Fa.ts?environmentId=${target}&cwd=%2Fexplicit&projectName=Project&line=2`;
  const view = (url = location, visit = 'file', session = 'session') => mobileNewTaskFlowView(session, visit, url, true, false, client, background);
  const flow = view();
  let permitted = true, contents = 'first\nsecond';
  const response = (request: Obj) => ({ ok: true, generation: request.fleet ? entry.generation : client.generation,
    value: request.op === 'environments' ? { saved: background.saved } : request.op === 'http'
      ? { authenticated: true, permissions: permitted ? ['filesystem:read'] : [], scopes: ['filesystem:read'] }
      : { contents, truncated: true } });
  const native: Native = { available: true, watch() {}, async later(input) { const request = obj(input); calls.push(request); return response(request); } };
  const read = (input: Native = native, url = location, visit = 'file', owner = flow.owner) => mobileNewTaskFileRead('src/a.ts', url, visit, owner, false, input, client, background);
  const snapshot = (url = location, visit = 'file', owner = flow.owner) => mobileNewTaskFileSnapshot('src/a.ts', url, visit, owner, false, client, background);
  const files: Files = { fs: { async mkdir() { throw new Error('unexpected write'); }, async readFile() { throw new Error('unexpected draft read'); }, async atomicWriteFile() { throw new Error('unexpected draft write'); } } };
  return { client, background, entry, calls, location, flow, view, response, native, read, snapshot, files,
    setAllowed(value: boolean) { permitted = value; }, setContents(value: string) { contents = value; } };
}

function deferredNative(f: ReturnType<typeof fixture>, at: string) {
  let release!: (value: unknown) => void, reached!: () => void;
  const pending = new Promise<void>(resolve => { reached = resolve; });
  const native: Native = { ...f.native, async later(input) {
    const request = obj(input); f.calls.push(request);
    if ((request.method || request.op) === at) { reached(); return new Promise(resolve => { release = resolve; }); }
    return f.response(request);
  } };
  return { native, pending, release: (value: unknown) => release(value) };
}

test('standalone focused file needs no project, thread, shell or preferences and keeps a stable owner', async () => {
  const f = fixture();
  expect(f.client.preferencesLoaded).toBe(false); expect(f.client.shell.projects).toHaveLength(0);
  expect(f.flow).toMatchObject({ status: 'file', fileReady: true, ready: false, needsPrepare: false, nextLocation: '', draftOwner: '', busy: false });
  expect(mobileNewTaskFlowOwns(f.flow.owner, 'file', f.client)).toBe(false);
  const owner = f.snapshot().owner;
  const result = await f.read();
  expect(result).toMatchObject({ owner, contents: 'first\nsecond', title: 'a.ts', subtitle: 'Project · src', initialRowId: 'source-line:1', truncated: true });
  expect(f.snapshot().owner).toBe(owner);
  expect(f.calls.map(call => call.method || call.op)).toEqual(['environments', 'http', 'projects.readFile']);
  expect(f.calls[2]).toMatchObject({ generation: 7, payload: { cwd: '/explicit', relativePath: 'src/a.ts' } });
  expect(f.client.projectId).toBe(''); expect(f.client.threadId).toBe('');
});

test('standalone background file uses the existing fleet channel and does not select it', async () => {
  const f = fixture('remote');
  Object.assign(f.client, { projectId: 'owned-project', threadId: 'owned-thread', threadEpoch: 42, providerId: 'provider', modelId: 'model' });
  f.client.local.drafts['focused:new:owned-project'] = 'preserved';
  const before = JSON.stringify([f.client.origin, f.client.environmentId, f.client.projectId, f.client.threadId, f.client.threadEpoch, f.client.providerId, f.client.modelId, f.client.local]);
  expect((await f.read()).contents).toBe('first\nsecond');
  expect(f.calls[1]).toMatchObject({ fleet: f.entry.key, generation: 19, op: 'http', path: '/api/auth/session' });
  expect(f.calls[2]).toMatchObject({ fleet: f.entry.key, generation: 19, method: 'projects.readFile', payload: { cwd: '/explicit', relativePath: 'src/a.ts' } });
  expect(JSON.stringify([f.client.origin, f.client.environmentId, f.client.projectId, f.client.threadId, f.client.threadEpoch, f.client.providerId, f.client.modelId, f.client.local])).toBe(before);
});

test('standalone route refuses every draft action including project and prepare exceptions', async () => {
  const f = fixture();
  for (const kind of ['project', 'prepare', 'scratch', 'draft', 'send', 'clone-retry']) {
    const result = await mobileNewTaskFlowAction(f.flow.owner, 'file', kind, '["focused","project"]', 'replacement', f.native, f.files, f.client, f.background);
    expect(result.message).toContain('Return to the new task');
  }
  expect(f.calls).toHaveLength(0);
});

test('standalone fileBack preserves an adopted draft and its complete local state', async () => {
  const f = fixture('remote');
  Object.assign(f.client, { shellLoaded: true, shellLive: true, configLive: true, threadLive: true, scopes: ['orchestration:operate'] });
  f.client.config = { environment: { capabilities: { serverResolvedCommandContext: true } } };
  f.client.shell.projects = [{ id: 'project', title: 'Project', workspaceRoot: '/project' }];
  f.client.local.drafts['focused:new:project'] = 'unsent draft';
  const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('absent'); }, async atomicWriteFile() {} } };
  await f.client.command('dismiss-error', '', '', 0, f.native, files);
  const chooser = f.view('/new', 'choose');
  await mobileNewTaskFlowAction(chooser.owner, 'choose', 'project', '["focused","project"]', '', f.native, files, f.client, f.background);
  const draft = f.view('/new/draft', 'draft'); expect(draft.ready).toBe(true);
  const before = JSON.stringify([f.client.local, f.client.projectId, f.client.threadId, f.client.threadEpoch, f.client.providerId, f.client.modelId]);
  const file = f.view(); expect(file.fileReady).toBe(true); expect(file.ready).toBe(false);
  await f.read(f.native, f.location, 'file', file.owner);
  expect(f.view('/new/draft', 'draft')).toMatchObject({ ready: true, draftOwner: draft.draftOwner });
  expect(JSON.stringify([f.client.local, f.client.projectId, f.client.threadId, f.client.threadEpoch, f.client.providerId, f.client.modelId])).toBe(before);
  expect(f.client.draft).toBe('unsent draft');
});

test('standalone file does not wait on an unrelated environment, selected draft or preferences', async () => {
  const f = fixture('remote'), delayed = deferredNative(f, 'projects.readFile');
  const reading = f.read(delayed.native); await delayed.pending;
  Object.assign(f.client, { projectId: 'other', threadId: 'other-thread', threadEpoch: 33, generation: 8, connection: 'disconnected' });
  delayed.release({ ok: true, generation: 19, value: { contents: 'remote survives unrelated selection' } });
  expect((await reading).contents).toBe('remote survives unrelated selection');
});

test('unknown, disabled and disconnected standalone environments never send a file request', async () => {
  for (const state of ['unknown', 'disabled', 'disconnected']) {
    const f = fixture(state === 'unknown' ? 'missing' : 'remote');
    if (state === 'disabled') f.background.saved[1]!.enabled = false;
    if (state === 'disconnected') f.entry.phase = 'available';
    expect((await f.read()).error).toContain('Connect to this workspace');
    expect(f.calls.filter(call => call.method || call.op === 'http')).toHaveLength(0);
  }
});

test('fresh explicit empty permission denies and removes standalone cached contents', async () => {
  const f = fixture('remote'); expect((await f.read()).contents).toBe('first\nsecond');
  f.setAllowed(false);
  expect(await f.read()).toMatchObject({ contents: '', loading: false, error: 'This connection cannot read host files.' });
  expect(f.snapshot().contents).toBe('');
  expect(f.calls.filter(call => call.method === 'projects.readFile')).toHaveLength(1);
});

test('standalone reads abandon replies after route departure during catalog, permission or file request', async () => {
  for (const stage of ['environments', 'http', 'projects.readFile']) {
    const f = fixture('remote'), delayed = deferredNative(f, stage), reading = f.read(delayed.native);
    await delayed.pending; f.view('/new', 'back');
    delayed.release({ ok: true, generation: 19, value: { saved: f.background.saved, authenticated: true, permissions: ['filesystem:read'], contents: 'stale' } });
    await expect(reading).rejects.toMatchObject({ kind: 'superseded' });
    expect(f.snapshot().contents).toBe('');
  }
});

test('standalone replies cannot cross generation, origin, phase, role or active-route changes', async () => {
  for (const change of ['generation', 'origin', 'phase', 'role', 'route', 'disabled']) {
    const f = fixture('remote'), delayed = deferredNative(f, 'projects.readFile'), reading = f.read(delayed.native);
    await delayed.pending;
    if (change === 'generation') f.entry.generation++;
    if (change === 'origin') f.entry.origin = 'https://replacement.test';
    if (change === 'phase') f.entry.phase = 'reconnecting';
    if (change === 'role') { f.client.environmentId = 'remote'; f.client.origin = f.entry.origin; }
    if (change === 'route') f.entry.activeRouteId = 'new-route';
    if (change === 'disabled') f.background.saved[1]!.enabled = false;
    delayed.release({ ok: true, generation: 19, value: { contents: 'stale' } });
    await expect(reading).rejects.toMatchObject({ kind: 'superseded' }); expect(f.snapshot().contents).toBe('');
  }
});

test('focused same-generation origin replacement abandons file read', async () => {
  const f = fixture(), delayed = deferredNative(f, 'http'), reading = f.read(delayed.native);
  await delayed.pending; f.client.origin = 'https://replacement.test';
  delayed.release({ ok: true, generation: 7, value: { authenticated: true, permissions: ['filesystem:read'] } });
  await expect(reading).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.filter(call => call.method)).toHaveLength(0);
});

test('same visit query replacement and newer refresh preserve only the latest standalone file', async () => {
  const f = fixture(), delayed = deferredNative(f, 'projects.readFile'), reading = f.read(delayed.native);
  await delayed.pending;
  const newer = f.location.replace('%2Fexplicit', '%2Fnew'); f.view(newer); f.setContents('new');
  await f.read(f.native, newer);
  delayed.release({ ok: true, generation: 7, value: { contents: 'old' } });
  await expect(reading).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot(newer).contents).toBe('new'); expect(f.snapshot().contents).toBe('');
  await expect(f.read()).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot(newer).contents).toBe('new');
  const slow = deferredNative(f, 'projects.readFile'), older = f.read(slow.native, newer); await slow.pending;
  f.setContents('newest'); await f.read(f.native, newer);
  slow.release({ ok: true, generation: 7, value: { contents: 'late' } });
  await expect(older).rejects.toMatchObject({ kind: 'superseded' }); expect(f.snapshot(newer).contents).toBe('newest');
});

test('same file visit reused by a new flow cannot display the old cached bytes', async () => {
  const f = fixture(); await f.read();
  const next = f.view(f.location, 'file', 'replacement');
  expect(f.snapshot(f.location, 'file', next.owner).contents).toBe('');
  expect(mobileNewTaskFileRouteCurrent(f.flow.owner, 'file', f.location, f.client)).toBe(false);
});

test('standalone parser preserves literal question marks and nonblank path bytes', async () => {
  const f = fixture('remote');
  const url = '/new/draft/files/src%2Fa.ts?cwd=/work?tree%20&environmentId=remote&line=2&projectName=%20Name%20';
  const flow = f.view(url); const result = await f.read(f.native, url, 'file', flow.owner);
  expect(f.calls.find(call => call.method)?.payload).toEqual({ cwd: '/work?tree ', relativePath: 'src/a.ts' });
  expect(result).toMatchObject({ subtitle: ' Name  · src', initialRowId: 'source-line:1' });
  for (const query of ['cwd=/work', 'environmentId=remote', 'environmentId=%20&cwd=/work', 'environmentId=remote&cwd=%20', 'environmentId=remote&cwd=/work&draftId=unowned']) {
    const malformed = '/new/draft/files/a.ts?' + query; const view = f.view(malformed);
    expect(view.fileReady).toBe(false); expect(mobileNewTaskFileRouteCurrent(view.owner, 'file', malformed, f.client)).toBe(false);
  }
});

test('runtime abandonment leaves no cached contents or spinner', async () => {
  const f = fixture('remote');
  const native: Native = { ...f.native, async later() { throw { name: 'FetchError', kind: 'Aborted' }; } };
  await expect(f.read(native)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.snapshot()).toMatchObject({ loading: false, error: '', contents: '' });
});
