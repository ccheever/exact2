import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileNative } from './client';
import { mobileGitRead, mobileGitSnapshot } from './git-overview';
import { mobileGitBranchAction, mobileGitBranchesRead, mobileGitBranchesSnapshot } from './git-branches';

const operations = ['create', 'checkout', 'worktree'] as const;
type Operation = typeof operations[number];
const permissions = ['orchestration:read', 'source-control:write', 'orchestration:operate'];
const metadataMethod = 'orchestration.dispatchCommand';
const mutationMethod = (op: Operation) => op === 'create' ? 'vcs.createRef' : op === 'checkout' ? 'vcs.switchRef' : 'vcs.createWorktree';
type Stage = 'refs' | 'git' | 'ids' | 'persistence' | 'metadata';

async function fixture(op: Operation, bindNative = true) {
  const client = new T3Client();
  Object.assign(client, { origin: 'https://example.test', environmentId: 'e', projectId: 'p', threadId: 't',
    connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 9 });
  client.shell.projects = [{ id: 'p', title: 'Repo', workspaceRoot: '/repo' }];
  client.shell.threads = [{ id: 't', projectId: 'p', branch: 'main', worktreePath: null }];
  client.config = { environment: { capabilities: { threadPullRequests: true, serverResolvedCommandContext: true } } };
  const status: Obj = { isRepo: true, refName: 'main', isDefaultRef: true, hasWorkingTreeChanges: false,
    hasUpstream: true, hasPrimaryRemote: true, aheadCount: 0, behindCount: 0, pr: null, workingTree: { files: [] } };
  const refs = [{ name: 'main', isRemote: false, current: true, worktreePath: null },
    { name: 'next', isRemote: false, current: false, worktreePath: null }];
  const calls: Obj[] = []; let count = 0, grants = [...permissions], failure = '';
  let hold: { matches(request: Obj): boolean; started(): void; pending: Promise<void> } | null = null;
  const raw: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (hold?.matches(request)) { const gate = hold; hold = null; gate.started(); await gate.pending; }
    if (request.method === metadataMethod && failure) return { ok: false, generation: 9, error: { kind: 'server', message: failure } };
    const value = request.path === '/api/auth/session' ? { authenticated: true, permissions: [...grants] }
      : request.method === 'vcs.refreshStatus' ? status : request.method === 'vcs.listRefs' ? { refs }
      : request.method === 'vcs.createRef' ? { refName: 'feature/next' }
      : request.method === 'vcs.switchRef' ? { refName: 'next' }
      : request.method === 'vcs.createWorktree' ? { worktree: { refName: 'feature/next', path: '/worktrees/next' } }
      : request.op === 'subscribe' ? { id: `subscription-${++count}` }
      : request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `id-${++count}`) : {};
    return { ok: true, generation: 9, value };
  } };
  const observedNative = mobileNative(raw, client), native = bindNative ? observedNative : raw;
  const owner = () => mobileGitSnapshot(10, client).owner;
  await mobileGitRead(owner(), 10, native, client);
  await mobileGitBranchesRead(owner(), 10, native, client);
  calls.length = 0;
  return { client, calls, owner,
    run: () => mobileGitBranchAction(owner(), op, 'next', 'main', 10, native, client),
    read: () => mobileGitRead(owner(), 10, observedNative, client),
    async observeWithout(permission: string) {
      grants = permissions.filter(value => value !== permission);
      await client.http(observedNative, '/api/auth/session');
      expect(client.scopes).toEqual(grants);
    },
    denyMetadata(message: string) { failure = message; },
    holdAt(stage: Stage) {
      let release!: () => void, began!: () => void;
      const pending = new Promise<void>(resolve => release = resolve), started = new Promise<void>(resolve => began = resolve);
      hold = { pending, started: began, matches: request => stage === 'refs' ? request.method === 'vcs.listRefs' : stage === 'git' ? request.method === mutationMethod(op)
        : stage === 'ids' ? request.op === 'ids' : stage === 'persistence' ? request.op === 'writePreferences' : request.method === metadataMethod };
      return { started, release };
    },
  };
}
function expectCalls(f: Awaited<ReturnType<typeof fixture>>, op: Operation, metadata: number, refresh = true) {
  expect(f.calls.filter(request => request.method === mutationMethod(op))).toHaveLength(1);
  expect(f.calls.filter(request => request.method === metadataMethod)).toHaveLength(metadata);
  expect(f.calls.filter(request => request.method === 'vcs.refreshStatus')).toHaveLength(refresh ? 1 : 0);
}

for (const op of operations) {
  for (const transition of ['unchanged', 'operate-removed', 'source-write-removed'] as const) {
    test(`${op}: observed ${transition} while Git is held follows the pinned metadata permission`, async () => {
      const f = await fixture(op), gate = f.holdAt('git');
      expect(f.client.scopes).toEqual(permissions);
      const pending = f.run(); await gate.started;
      if (transition !== 'unchanged') await f.observeWithout(transition === 'operate-removed' ? 'orchestration:operate' : 'source-control:write');
      gate.release(); const result = await pending;
      expectCalls(f, op, transition === 'operate-removed' ? 0 : 1);
      expect(result.changed).toBe(transition !== 'operate-removed');
      expect(result.message).toBe(transition === 'operate-removed' ? "This connection cannot update the thread's branch." : '');
      if (transition !== 'operate-removed') {
        expect(f.calls.find(request => request.method === metadataMethod)?.payload).toMatchObject({
          type: 'thread.metadata.update', threadId: 't', branch: op === 'checkout' ? 'next' : 'feature/next',
          worktreePath: op === 'worktree' ? '/worktrees/next' : null,
        });
      }
      const refresh = f.calls.find(request => request.method === 'vcs.refreshStatus');
      expect(refresh?.payload).toEqual({ cwd: op === 'worktree' ? '/worktrees/next' : '/repo' });
      expect(f.client.pending).toBeUndefined();
    });
  }
  for (const stage of ['ids', 'persistence'] as const) for (const permission of ['orchestration:operate', 'source-control:write']) {
    test(`${op}: observed ${permission} removal during ${stage} revalidates the actual metadata send`, async () => {
      const f = await fixture(op), gate = f.holdAt(stage), pending = f.run(); await gate.started;
      if (stage === 'persistence') expect(f.client.pending?.payload.type).toBe('thread.metadata.update');
      await f.observeWithout(permission); gate.release(); const result = await pending;
      const accepted = permission === 'source-control:write';
      expectCalls(f, op, accepted ? 1 : 0);
      expect(result.changed).toBe(accepted);
      expect(result.message).toBe(accepted ? '' : "This connection cannot update the thread's branch.");
      expect(f.client.pending).toBeUndefined();
      if (stage === 'persistence' && !accepted) {
        const persisted = f.calls.filter(request => request.op === 'writePreferences');
        expect(persisted.length).toBeGreaterThanOrEqual(2);
        expect(obj(obj(JSON.parse(String(persisted.at(-1)!.text))).pending).e).toBeUndefined();
      }
    });
  }
  test(`${op}: metadata accepted before operate removal remains successful`, async () => {
    const f = await fixture(op), gate = f.holdAt('metadata'), pending = f.run(); await gate.started;
    await f.observeWithout('orchestration:operate'); gate.release(); const result = await pending;
    expectCalls(f, op, 1); expect(result.changed).toBe(true); expect(result.message).toBe('');
    expect(f.client.pending).toBeUndefined();
  });
  test(`${op}: metadata server denial still refreshes the landed Git mutation`, async () => {
    const f = await fixture(op), before = mobileGitBranchesSnapshot(10, f.client).readRevision;
    f.denyMetadata('Metadata was denied'); const result = await f.run();
    expectCalls(f, op, 1); expect(result.changed).toBe(false); expect(result.message).toBe('Metadata was denied');
    expect(mobileGitBranchesSnapshot(10, f.client).readRevision).toBe(before + 1);
    expect(f.client.pending).toBeUndefined();
  });
  for (const permission of ['orchestration:operate', 'source-control:write']) {
    test(`${op}: a concurrent Git read observes ${permission} removal after Git was sent`, async () => {
      const f = await fixture(op), gate = f.holdAt('git'), pending = f.run(); await gate.started;
      await f.observeWithout(permission); await f.read(); gate.release(); const result = await pending;
      const accepted = permission === 'source-control:write';
      expectCalls(f, op, accepted ? 1 : 0);
      expect(result.changed).toBe(accepted);
      expect(result.message).toBe(accepted ? '' : "This connection cannot update the thread's branch.");
    });
  }
}

for (const stage of ['git', 'ids', 'persistence'] as const) for (const change of ['owner', 'cwd', 'selection'] as const) {
  test(`stale ${change} during ${stage} cannot send metadata or refresh another workspace`, async () => {
    const f = await fixture('create'), gate = f.holdAt(stage);
    const pending = f.run().then(result => ({ result, error: null }), error => ({ result: null, error })); await gate.started;
    if (change === 'owner') f.client.environmentId = 'other';
    if (change === 'cwd') f.client.shell.projects[0]!.workspaceRoot = '/other';
    if (change === 'selection') f.client.threadEpoch++;
    gate.release(); const answer = await pending;
    expect(answer.error).toMatchObject({ kind: 'superseded' }); expect(answer.result).toBeNull();
    expectCalls(f, 'create', 0, false);
  });
}
test('a raw Native binds branch authorization and refresh to the supplied session client', async () => {
  const f = await fixture('create', false);
  expect(f.client.scopes).toEqual([]);
  const result = await f.run();
  expect(f.client.scopes).toEqual(permissions);
  expectCalls(f, 'create', 1); expect(result.changed).toBe(true); expect(result.message).toBe('');
});
test('revocation observed by a concurrent read before checkout is sent still cancels it', async () => {
  const f = await fixture('checkout'), gate = f.holdAt('refs');
  const pending = f.run().then(result => ({ result, error: null }), error => ({ result: null, error })); await gate.started;
  await f.observeWithout('source-control:write'); await f.read(); gate.release(); const answer = await pending;
  expect(answer.error).toMatchObject({ kind: 'superseded' }); expect(answer.result).toBeNull();
  expect(f.calls.filter(request => request.method === mutationMethod('checkout'))).toHaveLength(0);
  expect(f.calls.filter(request => request.method === metadataMethod)).toHaveLength(0);
  expect(f.calls.filter(request => request.method === 'vcs.refreshStatus')).toHaveLength(0);
});
