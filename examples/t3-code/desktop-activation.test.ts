// 20261005-app-activation: `t3 app <dir>`. The first two describes port T3 Code's tests with their
// original names (MIT, see LICENSE-T3; reference 1e2ecbd975: apps/web/src/desktopAppActivation.test.ts,
// packages/shared/src/desktopAppControl.test.ts; the Windows named-pipe case stays out, macOS only).
// The rest cover the clone's window side: when it is ready, how a handed request becomes the
// window's open request, and the request's round trip on a focused or a background primary.
import { afterEach, describe, expect, it, test, vi } from 'bun:test';
import { createHash } from 'node:crypto';
import {
  handleDesktopAppActivationRequest, resolveDesktopAppControlAddress, activationTarget, activationPrepare, activationOpen, activationDependencies,
  openActivation, parseActivationRequest, activationOps, ACTIVATION_OPEN, type DesktopAppActivationDependencies,
} from './desktop-activation';
import { primaryAt, primaryOff, resetPrimary } from './local-primary-fixture';
import { EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { READ_OPS } from './client-ops';
import { T3Client } from './client';
import { initialShell, obj, str, type Obj } from './domain';
import type { Native } from './protocol';

afterEach(() => resetPrimary());

const environmentId = 'primary';
const existingProjectId = 'project-existing';
const createdProjectId = 'project-created';
const threadId = 'thread-1';
const request = { version: 1, requestId: 'request-1', type: 'open-workspace', workspaceRoot: '/workspace/project', platform: 'linux' } as const;

function dependencies(overrides: Partial<DesktopAppActivationDependencies> = {}): DesktopAppActivationDependencies {
  return {
    getTarget: () => ({ environmentId, platform: 'linux' }),
    findProject: () => ({ id: existingProjectId, environmentId, workspaceRoot: request.workspaceRoot }),
    createProject: vi.fn(async () => createdProjectId),
    waitForProject: vi.fn(async () => undefined),
    openThread: vi.fn(async () => ({ threadId })),
    ...overrides,
  };
}

describe('desktop app activation', () => {
  it('reuses an existing project and opens a new thread', async () => {
    const deps = dependencies();
    const response = await handleDesktopAppActivationRequest(request, deps);
    expect(deps.createProject).not.toHaveBeenCalled();
    expect(deps.openThread).toHaveBeenCalledWith({ environmentId, projectId: existingProjectId });
    expect(response).toEqual({ version: 1, requestId: request.requestId, ok: true, projectId: existingProjectId, threadId });
  });

  it('waits for a created project before it opens the thread', async () => {
    const order: string[] = [];
    const deps = dependencies({
      findProject: () => null,
      createProject: vi.fn(async () => { order.push('create'); return createdProjectId; }),
      waitForProject: vi.fn(async () => { order.push('project-event'); }),
      openThread: vi.fn(async () => { order.push('open-thread'); return { threadId }; }),
    });
    const response = await handleDesktopAppActivationRequest(request, deps);
    expect(order).toEqual(['create', 'project-event', 'open-thread']);
    expect(response).toMatchObject({ ok: true, projectId: createdProjectId });
  });

  it('rejects a Windows path when the primary environment is WSL', async () => {
    const response = await handleDesktopAppActivationRequest({ ...request, platform: 'win32' }, dependencies({ getTarget: () => ({ environmentId, platform: 'linux' }) }));
    expect(response).toMatchObject({ ok: false, code: 'platform-mismatch' });
  });

  it('returns a project error without opening a thread', async () => {
    const openThread = vi.fn(async () => ({ threadId }));
    const response = await handleDesktopAppActivationRequest(request, dependencies({
      findProject: () => null,
      createProject: vi.fn(async () => { throw new Error('Project path is not available.'); }),
      openThread,
    }));
    expect(response).toMatchObject({ ok: false, code: 'project-create-failed', message: 'Project path is not available.' });
    expect(openThread).not.toHaveBeenCalled();
  });
});

const sha256 = (text: string) => createHash('sha256').update(text).digest('hex');
describe('resolveDesktopAppControlAddress', () => {
  it('keeps Unix socket paths short and separates desktop state directories', () => {
    const first = resolveDesktopAppControlAddress({ stateDir: `/home/user/${'long/'.repeat(40)}userdata`, platform: 'linux', tempDir: '/tmp', userId: 1000,
      joinPath: (...segments) => segments.join('/'), sha256 });
    const second = resolveDesktopAppControlAddress({ stateDir: '/home/user/.t3/other/userdata', platform: 'linux', tempDir: '/tmp', userId: 1000,
      joinPath: (...segments) => segments.join('/'), sha256 });
    expect(first.directory).toBe('/tmp/t3code-1000');
    expect(first.address.length).toBeLessThan(108);
    expect(first.address).not.toBe(second.address);
  });

  // The vectors macos/tests/app-control holds T3AppControlAddress to (the app's native half).
  test('the macOS vectors the native address shares, under the 104-byte sun_path limit', () => {
    const join = (...segments: string[]) => segments.join('/');
    const lane = resolveDesktopAppControlAddress({ stateDir: '/Users/someone/.t3/userdata', platform: 'darwin', tempDir: '/var/folders/zz/abcdefghijklmnopqrstuvwxyz0000gn/T', userId: 501, joinPath: join, sha256 });
    expect(lane).toEqual({ directory: '/var/folders/zz/abcdefghijklmnopqrstuvwxyz0000gn/T/t3code-501', address: `/var/folders/zz/abcdefghijklmnopqrstuvwxyz0000gn/T/t3code-501/${sha256('/Users/someone/.t3/userdata').slice(0, 24)}.sock` });
    expect(sha256('/Users/someone/.t3/userdata').slice(0, 24)).toBe('073e69d4582c50be74c1c2d0');
    expect(lane.address.length).toBeLessThan(104);
    const noUser = resolveDesktopAppControlAddress({ stateDir: '/Users/someone/.t3/userdata', platform: 'darwin', tempDir: '/tmp', userId: undefined, joinPath: join, sha256 });
    expect(noUser.directory).toBe('/tmp/t3code-073e69d4582c');
  });
});

describe('the activation request', () => {
  test('parses only a whole open-workspace request of version 1', () => {
    expect(parseActivationRequest(request)).toEqual(request);
    expect(parseActivationRequest({ ...request, version: 2 })).toBeNull();
    expect(parseActivationRequest({ ...request, requestId: '' })).toBeNull();
    expect(parseActivationRequest({ ...request, type: 'open-file' })).toBeNull();
    expect(parseActivationRequest({ ...request, platform: 'freebsd' })).toBeNull();
    expect(parseActivationRequest({ ...request, workspaceRoot: 7 })).toBeNull();
  });
});

// ── The clone's window side ─────────────────────────────────────────────
const ORIGIN = 'http://127.0.0.1:16901';
const config = (os = 'darwin'): Obj => ({ environment: { environmentId: 'env-primary', label: 'This Mac', platform: { os, arch: 'arm64' } } });
function focusedClient(overrides: Partial<T3Client> = {}): T3Client {
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env-primary', origin: ORIGIN, connection: 'connected', config: config(), shellLoaded: true, generation: 3,
    shell: { ...initialShell(), projects: [{ id: 'p-1', title: 'one', workspaceRoot: '/Users/me/one' }] } }, overrides);
  return client;
}
function backgroundEntry(overrides: Partial<FleetEntry> = {}): FleetEntry {
  return { key: `${ORIGIN}\nenv-primary`, origin: ORIGIN, environmentId: 'env-primary', phase: 'connected', message: '', traceId: '', generation: 2, synchronized: 2,
    lastEvent: 0, subscriptions: {}, config: config(), shell: { ...initialShell(), projects: [] }, scopes: [], error: '', requested: true, primary: true, ...overrides };
}

describe('when the window can open a project (DesktopAppActivationCoordinator ready)', () => {
  test('the focused primary, connected with its config and shell snapshot', () => {
    primaryAt(ORIGIN, 'env-primary');
    expect(activationTarget(focusedClient(), new EnvironmentFleet())).toMatchObject({ environmentId: 'env-primary', platform: 'darwin', entry: null });
    expect(activationTarget(focusedClient({ shellLoaded: false }), new EnvironmentFleet())).toBeNull();
    expect(activationTarget(focusedClient({ connection: 'reconnecting' }), new EnvironmentFleet())).toBeNull();
    expect(activationTarget(focusedClient({ config: {} }), new EnvironmentFleet())).toBeNull();
  });

  test('the primary in the background while another environment has the focus', () => {
    primaryAt(ORIGIN, 'env-primary');
    const fleet = new EnvironmentFleet(), entry = backgroundEntry();
    fleet.entries.set(entry.key, entry);
    const remote = focusedClient({ environmentId: 'env-remote', origin: 'https://remote.example' });
    expect(activationTarget(remote, fleet)).toMatchObject({ environmentId: 'env-primary', entry });
    entry.synchronized = 1;
    expect(activationTarget(remote, fleet)).toBeNull();
  });

  test('never while the Local environment is switched off or no primary is known', () => {
    primaryOff();
    expect(activationTarget(focusedClient(), new EnvironmentFleet())).toBeNull();
    resetPrimary();
    expect(activationTarget(focusedClient(), new EnvironmentFleet())).toBeNull();
  });

  test('reports ready once, again on each change, with one token per client (a reload is a new renderer)', async () => {
    primaryAt(ORIGIN, 'env-primary');
    const sent: Obj[] = [];
    const native: Native = { available: true, watch: () => {}, later: async request => { sent.push(obj(request)); return { ok: true, generation: 0, value: {} }; } };
    const client = focusedClient();
    await activationPrepare(client, native, new EnvironmentFleet());
    await activationPrepare(client, native, new EnvironmentFleet());
    client.connection = 'reconnecting';
    await activationPrepare(client, native, new EnvironmentFleet());
    expect(sent.map(entry => entry.ready)).toEqual([true, false]);
    expect(str(sent[0]!.token)).not.toBe('');
    expect(sent[1]!.token).toBe(sent[0]!.token);
    await activationPrepare(focusedClient(), native, new EnvironmentFleet());
    expect(sent[2]!.token).not.toBe(sent[0]!.token);
  });
});

describe('the window open request', () => {
  const nativeHanding = (handed: { id: string }): Native & { topics: string[] } => {
    const topics: string[] = [];
    return { topics, available: true, watch: topic => { topics.push(topic); }, later: async () => ({ ok: true, generation: 0, value: { dispatched: handed.id } }) };
  };
  test('a handed request and a clicked notification each open once, the latest one winning', async () => {
    const owner = {}, handed = { id: '' }, native = nativeHanding(handed);
    const none = { opened: '', openedThread: '' };
    expect(await activationOpen(owner, native, none)).toEqual({ openRequest: '', openThreadId: '' });
    expect(native.topics).toContain('t3.activation');
    handed.id = 'req-1';
    expect(await activationOpen(owner, native, none)).toEqual({ openRequest: 'open:1', openThreadId: 'activation:req-1' });
    // Asked again (a tick, another revision): the same request, not a new one.
    expect(await activationOpen(owner, native, none)).toEqual({ openRequest: 'open:1', openThreadId: 'activation:req-1' });
    handed.id = '';
    expect(await activationOpen(owner, native, none)).toEqual({ openRequest: 'open:1', openThreadId: 'activation:req-1' });
    expect(await activationOpen(owner, native, { opened: '1:thread-9', openedThread: 'thread-9' })).toEqual({ openRequest: 'open:2', openThreadId: 'thread-9' });
    handed.id = 'req-2';
    expect(await activationOpen(owner, native, { opened: '1:thread-9', openedThread: 'thread-9' })).toEqual({ openRequest: 'open:3', openThreadId: 'activation:req-2' });
  });
  test('without the native module a notification opens as before', async () => {
    expect(await activationOpen({}, undefined, { opened: '2:thread-a', openedThread: 'thread-a' })).toEqual({ openRequest: 'open:1', openThreadId: 'thread-a' });
  });
});

/** A native module double: the activation ops, ids, the focused transport's requests and the fleet's. */
function moduleDouble(handed: Obj | null, server: { shells: Obj[][]; fail?: string } = { shells: [] }) {
  const calls: Obj[] = [];
  let ids = 0;
  const native: Native = { available: true, watch: () => {}, later: async raw => {
    const request = obj(raw), op = str(request.op);
    calls.push(request);
    const generation = typeof request.generation === 'number' ? request.generation : 0;
    const ok = (value: unknown) => ({ ok: true, generation, value });
    if (op === 'activationRequest') return handed && request.requestId === handed.requestId ? ok(handed) : { ok: false, generation, error: { kind: 'Activation', message: 'withdrawn' } };
    if (op === 'activationComplete') return ok({});
    if (op === 'ids') return ok(Array.from({ length: Number(request.count) || 1 }, () => `id-${++ids}`));
    if (op === 'request' && request.method === 'projects.mutate') return server.fail ? { ok: false, generation, error: { kind: 'server', message: server.fail } } : ok({ sequence: 1 });
    if (op === 'http') return ok({ projects: server.shells.shift() ?? [], threads: [], snapshotSequence: 2 });
    if (op === 'connect') return ok({ state: 'connecting', origin: str(request.origin), environmentId: '', message: '' });
    return ok({});
  } };
  return { native, calls };
}
const handedRequest = (workspaceRoot: string, platform = 'darwin') => ({ version: 1, requestId: 'req-7', type: 'open-workspace', workspaceRoot, platform });

describe('a handed request on this client', () => {
  test('an existing project on the focused primary: no project.create, its draft opens with a thread id, the answer goes back', async () => {
    primaryAt(ORIGIN, 'env-primary');
    const client = focusedClient({ threadId: 'some-thread', projectId: '' });
    const { native, calls } = moduleDouble(handedRequest('/Users/me/one/'));
    const response = await openActivation(client, native, 'req-7', activationDependencies(client, native, new EnvironmentFleet()));
    expect(response).toEqual({ version: 1, requestId: 'req-7', ok: true, projectId: 'p-1', threadId: 'id-1' });
    expect(calls.some(call => call.method === 'projects.mutate')).toBe(false);
    expect([client.projectId, client.threadId]).toEqual(['p-1', '']);
    expect(client.local.composerControls.draftThreads?.['env-primary:new:p-1']).toBe('id-1');
    expect(obj(calls.find(call => call.op === 'activationComplete')?.response)).toEqual(response!);
  });

  test('a new folder: project.create (not creating the folder), then the draft once the shell has it', async () => {
    primaryAt(ORIGIN, 'env-primary');
    const client = focusedClient();
    const created = { id: 'id-2', title: 'two words', workspaceRoot: '/Users/me/two words' };
    const { native, calls } = moduleDouble(handedRequest('/Users/me/two words'), { shells: [[...client.shell.projects, created]] });
    const response = await openActivation(client, native, 'req-7', activationDependencies(client, native, new EnvironmentFleet()));
    const create = calls.find(call => call.method === 'projects.mutate');
    expect(obj(create?.payload)).toEqual({ type: 'project.create', commandId: 'id-1', projectId: 'id-2', title: 'two words', workspaceRoot: '/Users/me/two words',
      createWorkspaceRootIfMissing: false, defaultModelSelection: null });
    expect(create?.generation).toBe(3);
    expect(response).toMatchObject({ ok: true, projectId: 'id-2' });
    expect(client.projectId).toBe('id-2');
  });

  test("the server's refusal is project-create-failed with its message, no draft opens and no error banner", async () => {
    primaryAt(ORIGIN, 'env-primary');
    const client = focusedClient({ projectId: 'p-1' });
    const { native } = moduleDouble(handedRequest('/Users/me/missing'), { shells: [], fail: 'Workspace root does not exist: /Users/me/missing' });
    const response = await openActivation(client, native, 'req-7', activationDependencies(client, native, new EnvironmentFleet()));
    expect(response).toEqual({ version: 1, requestId: 'req-7', ok: false, code: 'project-create-failed', message: 'Workspace root does not exist: /Users/me/missing' });
    expect([client.projectId, client.error]).toEqual(['p-1', '']);
  });

  test('a disconnected primary is environment-unavailable; a Linux path on this Mac is platform-mismatch', async () => {
    primaryAt(ORIGIN, 'env-primary');
    const client = focusedClient({ connection: 'reconnecting' });
    const { native } = moduleDouble(handedRequest('/Users/me/one'));
    expect(await openActivation(client, native, 'req-7', activationDependencies(client, native, new EnvironmentFleet())))
      .toMatchObject({ ok: false, code: 'environment-unavailable', message: "The desktop app's primary local environment is not connected." });
    const linux = focusedClient(), other = moduleDouble(handedRequest('/home/me/one', 'linux'));
    expect(await openActivation(linux, other.native, 'req-7', activationDependencies(linux, other.native, new EnvironmentFleet())))
      .toMatchObject({ ok: false, code: 'platform-mismatch', message: "The command path is for linux, but the desktop app's primary environment uses darwin. Cross-platform path mapping is not supported." });
  });

  test('a request that is gone (cancelled, expired) opens nothing and answers nothing', async () => {
    primaryAt(ORIGIN, 'env-primary');
    const client = focusedClient({ projectId: 'p-1', threadId: 't-1' });
    const { native, calls } = moduleDouble(null);
    expect(await openActivation(client, native, 'req-7', activationDependencies(client, native, new EnvironmentFleet()))).toBeNull();
    expect(calls.map(call => call.op)).toEqual(['activationRequest']);
    expect([client.projectId, client.threadId]).toEqual(['p-1', 't-1']);
  });

  test('a background primary gets the project on its own transport, then becomes the focus on the new draft', async () => {
    primaryAt(ORIGIN, 'env-primary');
    const fleet = new EnvironmentFleet(), entry = backgroundEntry();
    fleet.entries.set(entry.key, entry);
    const client = focusedClient({ environmentId: 'env-remote', origin: 'https://remote.example', projectId: 'r-1' });
    const { native, calls } = moduleDouble(handedRequest('/Users/me/new'), { shells: [[{ id: 'id-2', title: 'new', workspaceRoot: '/Users/me/new' }]] });
    const response = await openActivation(client, native, 'req-7', activationDependencies(client, native, fleet));
    expect(response).toMatchObject({ ok: true, projectId: 'id-2', threadId: 'id-3' });
    const create = calls.find(call => call.method === 'projects.mutate');
    expect([create?.fleet, create?.generation]).toEqual([entry.key, 2]);
    expect(calls.filter(call => call.op === 'fleetStop' || call.op === 'connect').map(call => [call.op, call.fleet ?? call.origin, call.primary ?? null]))
      .toEqual([['fleetStop', entry.key, null], ['connect', ORIGIN, true]]);
    expect(client.local.selections['env-primary']).toEqual({ projectId: 'id-2', threadId: '' });
    expect(fleet.entries.has(entry.key)).toBe(false);
  });

  test('`activation:open` is an op of its own, run before the write check', async () => {
    expect(READ_OPS).toContain(activationOps);
    const out = { message: 'x', id: '', value: '' };
    const client = focusedClient();
    const { native } = moduleDouble(null);
    expect(await activationOps.call(client, 'select-thread', 'activation:req-7', '', 0, native, undefined as never, out)).toBe(false);
    expect(await activationOps.call(client, ACTIVATION_OPEN, 'activation:req-7', '', 0, native, undefined as never, out)).toBe(true);
    expect(out.message).toBe('');
  });
});
