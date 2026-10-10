import { afterEach, expect, test } from 'bun:test';
import { mobileClient } from './client';
import { obj, str, initialShell, type Obj } from './shared/domain';
import { fleet, environmentKey, type FleetEntry } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';
import { mobileAddProjectObserve, mobileAddProjectSnapshot, mobileAddProjectPrepare, mobileAddProjectEdit,
  mobileAddProjectBrowse, mobileAddProjectAction, mobileAddProjectDeadline, type AddProjectRoute } from './add-project';
const original = { origin: mobileClient.origin, environmentId: mobileClient.environmentId, connection: mobileClient.connection,
  config: mobileClient.config, generation: mobileClient.generation, shell: mobileClient.shell, shellLoaded: mobileClient.shellLoaded };
afterEach(() => { Object.assign(mobileClient, original); fleet.entries.clear(); mobileAddProjectObserve('', [], ''); });
const permissions = ['orchestration:operate', 'orchestration:read', 'source-control:write', 'filesystem:read'];
const configuration = (tracking = true): Obj => ({ environment: { label: 'Alpha', platform: { os: 'linux', machine: 'server' }, capabilities: { projectCloneTracking: tracking } },
  newProjectsRoot: '/projects', settings: { addProjectBaseDirectory: '/work' } });
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
interface FixtureOptions {
  permissions?: string[]; tracking?: boolean; delayedShell?: boolean; saved?: Obj[]; discovery?: Obj;
  hook?: (request: Obj) => Promise<unknown> | unknown;
}
let visit = 0;
function fixture(options: FixtureOptions = {}) {
  const owner = `add-project-test-${++visit}`, calls: Obj[] = [], rows: Obj[] = [], bRows: Obj[] = [];
  let serial = 0;
  Object.assign(mobileClient, { origin: 'https://a.test', environmentId: 'a', connection: 'connected', generation: 31,
    config: configuration(options.tracking), shell: { ...initialShell(), projects: rows }, shellLoaded: true });
  const bKey = environmentKey('https://b.test', 'b');
  const entry: FleetEntry = { key: bKey, origin: 'https://b.test', environmentId: 'b', phase: 'connected', generation: 32,
    synchronized: 32, config: configuration(options.tracking), shell: { ...initialShell(), projects: bRows },
    message: '', traceId: '', lastEvent: 0, subscriptions: {}, scopes: [], error: '', requested: true };
  fleet.entries.set(bKey, entry);
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const overridden = await options.hook?.(request); if (overridden !== undefined) return overridden;
    const second = request.fleet === bKey, projects = second ? bRows : rows, generation = second ? 32 : 31;
    let value: unknown = {};
    if (request.op === 'environments') value = { saved: options.saved ?? [{ environmentId: 'a', origin: 'https://a.test', label: 'Alpha' }, { environmentId: 'b', origin: 'https://b.test', label: 'Beta', mobileLabel: 'Beta' }] };
    else if (request.op === 'ids') value = Array.from({ length: Number(request.count) }, () => `id-${++serial}`);
    else if (request.path === '/api/auth/session') value = { authenticated: true, permissions: options.permissions ?? permissions };
    else if (request.path === '/api/orchestration/shell') value = { projects: options.delayedShell ? [] : projects, threads: [], snapshotSequence: 2 };
    else if (request.method === 'server.discoverSourceControl') value = options.discovery ?? { sourceControlProviders: [] };
    else if (request.method === 'filesystem.browse') value = { entries: [{ name: 'repo', fullPath: `${str(obj(request.payload).partialPath)}repo/` }, { name: '.hidden', fullPath: '/work/.hidden/' }] };
    else if (request.method === 'projects.mutate') { const payload = obj(request.payload); projects.push({ id: payload.projectId, title: payload.title, workspaceRoot: payload.workspaceRoot }); }
    else if (request.method === 'projects.createNew') { const payload = obj(request.payload), id = `new-${++serial}`; const workspaceRoot = `/projects/${str(payload.name).toLowerCase()}`;
      projects.push({ id, title: payload.name, workspaceRoot }); value = { projectId: id, workspaceRoot }; }
    else if (request.method === 'projectClone.start') { const payload = obj(request.payload); projects.push({ id: payload.projectId, title: payload.title, workspaceRoot: payload.destinationPath }); }
    else if (request.method === 'sourceControl.cloneRepository') value = { cwd: '/actual/cloned' };
    else if (request.method === 'sourceControl.lookupRepository') value = { provider: obj(request.payload).provider, url: 'https://provider.test/team/project', sshUrl: 'git@provider.test:team/project.git', nameWithOwner: 'team/project' };
    return { ok: true, generation, value };
  } };
  const root: AddProjectRoute = { id: 'source', name: 'addProject', url: '/new/add-project' };
  const route = (name: string, query = '?environmentId=a'): AddProjectRoute => ({ id: name, name, url: `/new/add-project/${name}${query}` });
  const open = async (name: string, query = '?environmentId=a') => { const active = route(name, query); mobileAddProjectObserve(owner, [root, active], active.id); await mobileAddProjectPrepare(owner, active.id, native); return active; };
  const view = (id?: string) => mobileAddProjectSnapshot().screens.find(screen => screen.id === (id ?? mobileAddProjectSnapshot().activeId))!;
  const submit = (id: string) => mobileAddProjectAction(owner, id, 'submit', '', native);
  return { owner, calls, rows, bRows, entry, native, root, route, open, view, submit };
}

test('connected saved environments sort by label; explicit missing child does not use another machine', async () => {
  const f = fixture(); mobileAddProjectObserve(f.owner, [f.root], f.root.id); await mobileAddProjectPrepare(f.owner, f.root.id, f.native);
  expect(f.view().environments.map(item => item.label)).toEqual(['Alpha', 'Beta']);
  const absent = await f.open('addProjectLocal', '?environmentId=missing');
  expect(f.view(absent.id).available).toBe(false); expect(f.view().error).toBe('');
  expect(f.calls.filter(call => call.method === 'filesystem.browse')).toHaveLength(0);
  await f.open('addProjectNew', '?environmentId=missing'); expect(f.view().showMachines).toBe(true);
});

test('route query uses first values and retained inputs survive child Back but not a fresh visit', async () => {
  const f = fixture(), repository = await f.open('addProjectRepository', '?environmentId=a&environmentId=b&source=bogus&source=gitlab');
  expect(f.view().source).toBe('url'); expect(f.view().environmentId).toBe('a');
  mobileAddProjectEdit(f.owner, repository.id, 'input', 'owner/repo'); await f.submit(repository.id);
  expect(mobileAddProjectSnapshot().navigation).toMatchObject({ kind: 'destination', remoteUrl: 'https://github.com/owner/repo.git' });
  const destination = f.route('addProjectDestination', '?environmentId=a&repositoryName=repo&remoteUrl=https://github.com/owner/repo.git');
  mobileAddProjectObserve(f.owner, [f.root, repository, destination], destination.id); await mobileAddProjectPrepare(f.owner, destination.id, f.native);
  expect(f.view(repository.id).input).toBe('owner/repo'); expect(f.view(destination.id).input).toBe('/work/repo');
  mobileAddProjectObserve(f.owner, [f.root, repository], repository.id); expect(f.view().input).toBe('owner/repo');
  mobileAddProjectObserve(`${f.owner}-new`, [f.root, repository], repository.id); expect(f.view().input).toBe('');
});

test('folder reads require live filesystem permission, and explicit empty permissions deny all writes', async () => {
  const f = fixture({ permissions: [] }), local = await f.open('addProjectLocal');
  expect(f.view().canRead).toBe(false); expect(f.view().browseError).toContain('cannot browse'); expect(f.view().primaryDisabled).toBe(true);
  await f.submit(local.id); expect(f.view().error).toContain('cannot add projects');
  await mobileAddProjectBrowse(f.owner, local.id, '/work/', 'repo', f.native);
  expect(f.calls.some(call => call.method === 'filesystem.browse' || call.method === 'projects.mutate')).toBe(false);
});

test('typing during preload invalidates the folder answer without replacing the typed path', async () => {
  const gate = deferred<unknown>(), started = deferred<void>(); let hold = false;
  const f = fixture({ hook: request => { if (hold && request.method === 'filesystem.browse') { started.resolve(); return gate.promise; } } });
  const local = await f.open('addProjectLocal'); hold = true;
  const browse = mobileAddProjectBrowse(f.owner, local.id, '/work/', 'repo', f.native); await started.promise;
  mobileAddProjectEdit(f.owner, local.id, 'input', '/typed/path'); gate.resolve({ ok: true, generation: 31, value: { entries: [] } });
  await expect(browse).rejects.toMatchObject({ kind: 'superseded' }); expect(f.view().input).toBe('/typed/path'); expect(f.view().browsing).toBe(false);
});

test('a later folder tap wins reversed replies and back invalidates a prepare permanently', async () => {
  const gate = deferred<unknown>(), started = deferred<void>(); let hold = false;
  const f = fixture({ hook: request => { if (hold && request.method === 'filesystem.browse' && obj(request.payload).partialPath === '/work/first/') { started.resolve(); return gate.promise; } } });
  const local = await f.open('addProjectLocal'); hold = true;
  const first = mobileAddProjectBrowse(f.owner, local.id, '/work/', 'first', f.native); await started.promise;
  await mobileAddProjectBrowse(f.owner, local.id, '/work/', 'second', f.native);
  gate.resolve({ ok: true, generation: 31, value: { entries: [{ name: 'old', fullPath: '/old' }] } });
  await expect(first).rejects.toMatchObject({ kind: 'superseded' }); expect(f.view().input).toBe('/work/second/'); expect(f.view().browsing).toBe(false);
  mobileAddProjectObserve(f.owner, [f.root], f.root.id); await expect(f.submit(local.id)).rejects.toMatchObject({ kind: 'superseded' });
});

test('local create sends the pinned payload and adopts an actual qualified shell project', async () => {
  const f = fixture(), local = await f.open('addProjectLocal', '?environmentId=b');
  mobileAddProjectEdit(f.owner, local.id, 'input', '/work/new-project'); const result = await f.submit(local.id);
  const call = f.calls.find(call => call.method === 'projects.mutate')!;
  expect(call.fleet).toBe(f.entry.key); expect(call.generation).toBe(32);
  expect(call.payload).toEqual({ type: 'project.create', commandId: 'id-1', projectId: 'id-2', title: 'new-project', workspaceRoot: '/work/new-project', createWorkspaceRootIfMissing: true, defaultModelSelection: null });
  expect(result.navigation).toMatchObject({ kind: 'draft', environmentId: 'b', projectId: 'id-2', cloning: false });
  expect(mobileClient.environmentId).toBe('a'); expect(f.entry.shell.projects[0]?.id).toBe('id-2');
});

test('existing project reuse is environment-qualified and opens its draft with an alert', async () => {
  const f = fixture(); f.rows.push({ id: 'a-project', title: 'Alpha project', workspaceRoot: '/work/repo' }); f.bRows.push({ id: 'b-project', title: 'Beta project', workspaceRoot: '/work/repo' });
  const local = await f.open('addProjectLocal', '?environmentId=b'); mobileAddProjectEdit(f.owner, local.id, 'input', '/work/repo');
  await f.submit(local.id); expect(mobileAddProjectSnapshot().navigation.kind).toBe('');
  await mobileAddProjectAction(f.owner, local.id, 'alert', f.view().alerts[0]!.id, f.native);
  expect(mobileAddProjectSnapshot().navigation).toMatchObject({ kind: 'draft', projectId: 'b-project', environmentId: 'b' });
  expect(f.calls.find(call => call.op === 'mobileAlert')).toMatchObject({ title: 'Project already exists', message: 'Beta project' }); expect(f.calls.some(call => call.method === 'projects.mutate')).toBe(false);
});

test('fresh grant denial after prepare prevents creation and ordinary errors allow explicit retry', async () => {
  let denied = false;
  const f = fixture({ hook: request => denied && request.path === '/api/auth/session' ? { ok: true, generation: 31, value: { authenticated: true, permissions: [] } } : undefined });
  const local = await f.open('addProjectLocal'); denied = true; await f.submit(local.id);
  expect(f.calls.some(call => call.method === 'projects.mutate')).toBe(false); expect(f.view().busy).toBe(false); expect(f.view().uncertain).toBe(false);
  denied = false; await f.submit(local.id); expect(f.calls.filter(call => call.method === 'projects.mutate')).toHaveLength(1);
});

test('immediate submitting lock stops a double tap; a generation change refuses the stale write result', async () => {
  const gate = deferred<unknown>(), started = deferred<void>();
  const f = fixture({ hook: request => { if (request.method === 'projects.createNew') { started.resolve(); return gate.promise; } } });
  const page = await f.open('addProjectNew'); mobileAddProjectEdit(f.owner, page.id, 'input', 'Name');
  const create = f.submit(page.id); await started.promise; await f.submit(page.id);
  expect(f.calls.filter(call => call.method === 'projects.createNew')).toHaveLength(1);
  mobileClient.generation++; gate.resolve({ ok: true, generation: 31, value: { projectId: 'created', workspaceRoot: '/created' } });
  await create; expect(mobileAddProjectSnapshot().navigation.kind).toBe(''); expect(f.view().error).toContain('connection changed');
  expect(f.view().uncertain).toBe(true); await f.submit(page.id); expect(f.calls.filter(call => call.method === 'projects.createNew')).toHaveLength(1);
});

test('an acknowledged create waits for the existing event stream; root deadline never repeats the write', async () => {
  const f = fixture({ delayedShell: true }), page = await f.open('addProjectNew'); mobileAddProjectEdit(f.owner, page.id, 'input', 'Delayed');
  const result = await f.submit(page.id); expect(result.waitOperation).not.toBe(''); expect(result.navigation.kind).toBe(''); expect(f.view().busy).toBe(true);
  await f.submit(page.id); expect(f.calls.filter(call => call.method === 'projects.createNew')).toHaveLength(1);
  const deadline = mobileAddProjectDeadline(f.owner, page.id, result.waitOperation); expect(deadline.waitOperation).toBe(''); expect(f.view().input).toBe(''); expect(f.view().error).toContain('was created');
  expect(f.calls.filter(call => call.path === '/api/orchestration/shell')).toHaveLength(1);
});

test('actual project event completes delayed creation without a timer or another request', async () => {
  const f = fixture({ delayedShell: true }), page = await f.open('addProjectNew'); mobileAddProjectEdit(f.owner, page.id, 'input', 'Delayed');
  await f.submit(page.id); const calls = f.calls.length; mobileClient.shell.projects.push({ id: 'new-1', title: 'Delayed', workspaceRoot: '/projects/delayed' });
  const result = mobileAddProjectObserve(f.owner, [f.root, page], page.id); expect(result.navigation).toMatchObject({ kind: 'draft', projectId: 'new-1' }); expect(result.waitOperation).toBe(''); expect(f.calls).toHaveLength(calls);
});

test('tracked clone and older blocking clone use separate real payloads and actual returned cwd', async () => {
  for (const tracking of [true, false]) {
    const f = fixture({ tracking }), page = await f.open('addProjectDestination', '?environmentId=a&remoteUrl=https://host.test/repo.git&repositoryName=repo');
    const result = await f.submit(page.id);
    if (tracking) {
      expect(f.calls.find(call => call.method === 'projectClone.start')?.payload).toMatchObject({ projectId: 'id-1', title: 'repo', remoteUrl: 'https://host.test/repo.git', destinationPath: '/work/repo' });
      expect(result.navigation).toMatchObject({ kind: 'draft', projectId: 'id-1', cloning: true }); expect(f.calls.some(call => call.method === 'projects.mutate')).toBe(false);
    } else {
      expect(f.calls.find(call => call.method === 'sourceControl.cloneRepository')?.payload).toEqual({ remoteUrl: 'https://host.test/repo.git', destinationPath: '/work/repo' });
      expect(obj(f.calls.find(call => call.method === 'projects.mutate')?.payload).workspaceRoot).toBe('/actual/cloned'); expect(result.navigation.cloning).toBe(false);
    }
  }
});

test('provider lookup uses its requested source and source-specific clone URL', async () => {
  const f = fixture(), page = await f.open('addProjectRepository', '?environmentId=b&source=gitlab'); mobileAddProjectEdit(f.owner, page.id, 'input', ' team/project ');
  const result = await f.submit(page.id), lookup = f.calls.find(call => call.method === 'sourceControl.lookupRepository')!;
  expect(lookup.fleet).toBe(f.entry.key); expect(lookup.payload).toEqual({ provider: 'gitlab', repository: 'team/project' });
  expect(result.navigation).toMatchObject({ kind: 'destination', source: 'gitlab', remoteUrl: 'git@provider.test:team/project.git', title: 'team/project', repositoryName: 'project' });
});

test('creation remains successful after commit and publication errors, with no automatic duplicate', async () => {
  const discovery: Obj = { sourceControlProviders: [{ kind: 'github', status: 'available', auth: { status: 'authenticated', account: 'octo' } }] };
  const f = fixture({ discovery, hook: request => {
    if (request.method === 'projects.createNew') { f.rows.push({ id: 'created', title: 'Name', workspaceRoot: '/actual/server-name-2' }); return { ok: true, generation: 31, value: { projectId: 'created', workspaceRoot: '/actual/server-name-2', commitError: 'Git identity missing' } }; }
    if (request.method === 'sourceControl.publishRepository') return { ok: false, generation: 31, error: { kind: 'server', message: 'Repository creation denied', uncertain: false } };
  } });
  const page = await f.open('addProjectNew'); expect(f.view().showPublish).toBe(true);
  mobileAddProjectEdit(f.owner, page.id, 'input', 'Name'); mobileAddProjectEdit(f.owner, page.id, 'publish', 'true');
  await f.submit(page.id); expect(mobileAddProjectSnapshot().navigation.kind).toBe('');
  expect(f.view().alerts.map(alert => alert.title)).toEqual(['Created without a first commit', 'Could not create the GitHub repository']);
  for (const notice of [...f.view().alerts]) await mobileAddProjectAction(f.owner, page.id, 'alert', notice.id, f.native);
  expect(mobileAddProjectSnapshot().navigation).toMatchObject({ kind: 'draft', projectId: 'created' });
  expect(f.calls.find(call => call.method === 'sourceControl.publishRepository')?.payload).toEqual({ cwd: '/actual/server-name-2', provider: 'github', repository: 'octo/server-name-2', visibility: 'private' });
  expect(f.calls.filter(call => call.method === 'projects.createNew')).toHaveLength(1); expect(f.view().uncertain).toBe(false);
});

test('leaving during a dispatched create suppresses late navigation and never sends a second write', async () => {
  const gate = deferred<unknown>(), started = deferred<void>();
  const f = fixture({ hook: request => { if (request.method === 'projects.createNew') { started.resolve(); return gate.promise; } } });
  const page = await f.open('addProjectNew'); mobileAddProjectEdit(f.owner, page.id, 'input', 'Name'); const create = f.submit(page.id); await started.promise;
  mobileAddProjectObserve(f.owner, [f.root], f.root.id); gate.resolve({ ok: true, generation: 31, value: { projectId: 'created', workspaceRoot: '/created' } });
  await expect(create).rejects.toMatchObject({ kind: 'superseded' }); expect(mobileAddProjectSnapshot().navigation.kind).toBe(''); expect(f.calls.filter(call => call.method === 'projects.createNew')).toHaveLength(1);
});

test('a generic post-dispatch rejection remains uncertain and cannot duplicate project creation', async () => {
  const f = fixture({ hook: request => { if (request.method === 'projects.createNew') throw new Error('Lost connection after send'); } });
  const page = await f.open('addProjectNew'); mobileAddProjectEdit(f.owner, page.id, 'input', 'Name');
  await f.submit(page.id); expect(f.view().uncertain).toBe(true); expect(f.view().busy).toBe(false);
  await f.submit(page.id); expect(f.calls.filter(call => call.method === 'projects.createNew')).toHaveLength(1);
});

test('a folder tap cancels preparation before its delayed session can issue a stale listing', async () => {
  const sessionGate = deferred<unknown>(), sessionStarted = deferred<void>(); let racing = false, sessions = 0;
  const f = fixture({ hook: request => {
    if (racing && request.path === '/api/auth/session' && ++sessions === 1) { sessionStarted.resolve(); return sessionGate.promise; }
    if (racing && request.method === 'filesystem.browse') return { ok: true, generation: 31, value: { entries: [{ name: 'new', fullPath: '/work/child/new' }] } };
  } });
  const page = await f.open('addProjectLocal'); racing = true; mobileAddProjectEdit(f.owner, page.id, 'refresh', '');
  const preparing = mobileAddProjectPrepare(f.owner, page.id, f.native); await sessionStarted.promise;
  await mobileAddProjectBrowse(f.owner, page.id, '/work/', 'child', f.native);
  sessionGate.resolve({ ok: true, generation: 31, value: { authenticated: true, permissions } });
  await expect(preparing).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.view().input).toBe('/work/child/'); expect(f.view().folders.map(folder => folder.id)).toEqual(['/work/child/new']);
  expect(f.calls.filter(call => call.method === 'filesystem.browse' && obj(call.payload).partialPath === '/work/')).toHaveLength(1);
});

test('offline catalog recovery requests new preparation and clears only its stale read error', async () => {
  const f = fixture(); mobileClient.connection = 'disconnected'; fleet.entries.clear(); mobileAddProjectObserve(f.owner, [f.root], f.root.id);
  await mobileAddProjectPrepare(f.owner, f.root.id, f.native); expect(f.view().available).toBe(false); expect(f.view().error).toBe('');
  mobileClient.connection = 'connected'; mobileClient.revision++; expect(mobileAddProjectObserve(f.owner, [f.root], f.root.id).needsPrepare).toBe(true);
  await mobileAddProjectPrepare(f.owner, f.root.id, f.native); expect(f.view().available).toBe(true); expect(f.view().error).toBe(''); expect(mobileAddProjectSnapshot().needsPrepare).toBe(false);
});

test('lost publication response keeps the acknowledged create and emits its partial-success alert', async () => {
  const f = fixture({ discovery: { sourceControlProviders: [{ kind: 'github', status: 'available', auth: { status: 'authenticated', account: 'octo' } }] },
    hook: request => { if (request.method === 'sourceControl.publishRepository') throw new Error('Lost publication response'); } });
  const page = await f.open('addProjectNew'); mobileAddProjectEdit(f.owner, page.id, 'input', 'Name'); mobileAddProjectEdit(f.owner, page.id, 'publish', 'true');
  await f.submit(page.id); expect(mobileAddProjectSnapshot().navigation.kind).toBe(''); expect(f.view().uncertain).toBe(false);
  expect(f.view().alerts[0]?.title).toBe('Could not create the GitHub repository');
  await mobileAddProjectAction(f.owner, page.id, 'alert', f.view().alerts[0]!.id, f.native); expect(mobileAddProjectSnapshot().navigation.kind).toBe('draft');
  await f.submit(page.id); expect(f.calls.filter(call => call.method === 'projects.createNew')).toHaveLength(1);
});

test('navigation query retains reserved characters and source labels match the pinned screens', async () => {
  const f = fixture(), page = await f.open('addProjectRepository'); mobileAddProjectEdit(f.owner, page.id, 'input', 'https://host.test/repo.git?x=a&y=b');
  const result = await f.submit(page.id), query = new URLSearchParams(result.navigation.query.slice(1));
  expect(query.get('remoteUrl')).toBe('https://host.test/repo.git?x=a&y=b'); expect(query.get('environmentId')).toBe('a');
  await f.open('addProjectDestination'); expect(f.view()).toMatchObject({ title: 'Clone destination', primaryLabel: 'Clone project', placeholder: '~/projects/my-app' });
});

test('local keystrokes preserve fresh catalog state without masking an earlier external invalidation', async () => {
  const f = fixture(), page = await f.open('addProjectNew');
  for (const text of ['d', 'de', 'dem', 'demo']) { mobileAddProjectEdit(f.owner, page.id, 'input', text); expect(mobileAddProjectSnapshot().needsPrepare).toBe(false); }
  mobileClient.revision++; mobileAddProjectEdit(f.owner, page.id, 'input', 'external'); expect(mobileAddProjectSnapshot().needsPrepare).toBe(true);
});

test('fresh flow initializes from current configuration while retained inputs remain unchanged', async () => {
  const f = fixture(), page = await f.open('addProjectLocal'); mobileAddProjectEdit(f.owner, page.id, 'input', '/retained/');
  mobileClient.config = { ...mobileClient.config, settings: { addProjectBaseDirectory: '/current' } }; mobileClient.revision++;
  mobileAddProjectObserve(f.owner, [f.root, page], page.id); await mobileAddProjectPrepare(f.owner, page.id, f.native); expect(f.view().input).toBe('/retained/');
  const nextOwner = `${f.owner}-fresh`; mobileAddProjectObserve(nextOwner, [f.root, page], page.id); await mobileAddProjectPrepare(nextOwner, page.id, f.native); expect(f.view().input).toBe('/current/');
});

test('a notice is presented exactly once and defers draft navigation until dismissal', async () => {
  const gate = deferred<unknown>(), started = deferred<void>();
  const f = fixture({ hook: request => { if (request.op === 'mobileAlert') { started.resolve(); return gate.promise; } } });
  f.rows.push({ id: 'existing', title: 'Exists', workspaceRoot: '/work/repo' });
  const page = await f.open('addProjectLocal'); mobileAddProjectEdit(f.owner, page.id, 'input', '/work/repo'); await f.submit(page.id);
  const token = f.view().alerts[0]!.id, notice = mobileAddProjectAction(f.owner, page.id, 'alert', token, f.native); await started.promise;
  expect(mobileAddProjectSnapshot().navigation.kind).toBe(''); await mobileAddProjectAction(f.owner, page.id, 'alert', token, f.native);
  gate.resolve({ ok: true, generation: 31, value: { choice: 'ok' } }); await notice;
  expect(mobileAddProjectSnapshot().navigation.kind).toBe('draft'); expect(f.calls.filter(call => call.op === 'mobileAlert')).toHaveLength(1);
});

test('an explicitly uncertain negative bridge reply cannot unlock a second creation', async () => {
  const f = fixture({ hook: request => request.method === 'projects.createNew'
    ? { ok: false, generation: 31, error: { kind: 'transport', message: 'Request outcome unknown', uncertain: true } } : undefined });
  const page = await f.open('addProjectNew'); mobileAddProjectEdit(f.owner, page.id, 'input', 'Name');
  await f.submit(page.id); expect(f.view().uncertain).toBe(true); await f.submit(page.id);
  expect(f.calls.filter(call => call.method === 'projects.createNew')).toHaveLength(1);
});
