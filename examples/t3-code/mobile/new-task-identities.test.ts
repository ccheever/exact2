import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskFlowView as view, mobileNewTaskFlowAction as action } from './new-task-flow';
import { mobileNewTaskDraftLookup as lookup, mobileNewTaskDraftList as list, mobileNewTaskDraftCreate as create } from './mobile-new-task-drafts';
import { composerNow, noteNow } from './shared/composer-controls';
import { mobileNewTaskEnvironmentMatch, mobileNewTaskEnvironmentSources } from './new-task-selection';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import type { MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
import { mobileNewTaskSubmitRecover } from './new-task-submit';
const time = 1791420000000, origin = 'https://test.invalid';
function config(environmentId = 'env'): Obj {
  return { environment: { environmentId, orchestrationProtocolVersion: 2, capabilities: { serverResolvedCommandContext: true } },
    providers: [{ instanceId: 'provider', driver: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' },
      models: [{ slug: 'model', name: 'Model', isDefault: true }] }], settings: { providerInstances: { provider: { driver: 'codex', enabled: true } } } };
}
async function fixture(saved: Obj = { version: 1 }, withClock = true) {
  const client = new MobileDraftClient(), fleet = new EnvironmentFleet(), calls: Obj[] = [];
  let disk = JSON.stringify(saved), serial = 0;
  const claims = new Map<string, MobileOutboxTransferClaim>();
  const outboxRows = new Map<string, Obj>();
  const outcome = (claim: MobileOutboxTransferClaim) => ({ mutationId: claim.mutationId, messageId: claim.messageId,
    status: 'committed', revision: 1, record: claim.record, removed: null, message: '', ownerEpoch: 'epoch', sequenceFloor: 1,
    current: outboxRows.get(claim.messageId) });
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { disk = new TextDecoder().decode(bytes); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'mobileOutbox') {
      let value: unknown;
      if (request.action === 'read') value = { ownerEpoch: 'epoch', sequenceFloor: claims.size, complete: true, errors: [],
        records: [...outboxRows.values()], outcomes: [], mutations: [], transfers: [...claims.values()],
        revisions: Object.fromEntries([...outboxRows].map(([id, row]) => [id, row.revision])),
        tokens: Object.fromEntries([...outboxRows].map(([id, row]) => [id, row.token])) };
      else if (request.action === 'transferLookup') value = { complete: true, fingerprint: request.capture ? 'a'.repeat(64) : null,
        claims: [...claims.values()].filter(claim => claim.draftKey === request.draftKey) };
      else if (request.action === 'transferStatus') {
        const claim = claims.get(String(request.transferId)); value = { claim, outcome: claim?.record ? outcome(claim) : null };
      } else if (request.action === 'enqueueTransfer') {
        const record = request.record as NonNullable<MobileOutboxTransferClaim['record']>;
        const capture = request.capture as NonNullable<MobileOutboxTransferClaim['capture']>;
        const claim: MobileOutboxTransferClaim = { transferId: record.messageId, draftKey: capture.draft.key, fingerprint: 'a'.repeat(64),
          messageId: record.messageId, threadId: record.threadId, commandId: record.commandId, mutationId: String(request.mutationId),
          state: 'queued', record, capture };
        claims.set(claim.transferId, claim);
        outboxRows.set(record.messageId, { record, revision: 1, token: request.mutationId, pending: false, held: false });
        value = { disposition: 'created', claim, outcome: outcome(claim) };
      } else if (request.action === 'completeTransfer') {
        const claim = claims.get(String(request.transferId))!;
        const completed = { ...claim, state: 'completed' as const, capture: null, record: null };
        claims.set(claim.transferId, completed); value = { completed: true, claim: completed };
      } else throw Error('Unexpected outbox action ' + request.action);
      return { ok: true, generation: client.generation, value };
    }
    return { ok: true, generation: client.generation, value: request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `allocated-${++serial}`)
      : request.op === 'status' ? { origin: client.origin, homeOrigin: origin, environmentId: client.environmentId }
      : request.op === 'http' ? { authenticated: true, permissions: ['orchestration:operate'], scopes: ['orchestration:operate'] }
        : request.method === 'server.getConfig' ? config(client.environmentId) : {} };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage);
  await client.command('dismiss-error', '', '', 0, handles.native, handles.storage);
  Object.assign(client, { environmentId: 'env', origin, projectId: 'a', threadId: '', generation: 1, connection: 'connected',
    shellLoaded: true, configLive: true, shellLive: true, threadLive: true, scopes: ['orchestration:operate'] });
  client.config = config(); client.shell.projects = [{ id: 'a', title: 'A', workspaceRoot: '/a' }, { id: 'b', title: 'B', workspaceRoot: '/b' }];
  client.local.drafts['env:new:a'] = 'old A'; client.local.drafts['env:new:b'] = 'old B';
  if (withClock) noteNow(client, time); calls.length = 0;
  const snapshot = (session: string, visit = 'visit', location = '/new/draft', ready = true) => view(session, visit, location, true, ready, client, fleet);
  const act = (owner: string, kind: string, id = '', value = '', visit = 'visit') => action(owner, visit, kind, id, value, native, storage, client, fleet, time);
  const choose = async (session: string, project = 'a') => { const flow = snapshot(session, 'visit', '/new'); const result = await act(flow.owner, 'project', JSON.stringify(['env', project]));
    expect(result.message).toBe(''); return snapshot(session); };
  return { client, fleet, calls, native, storage, snapshot, act, choose, claims, disk: () => obj(JSON.parse(disk)) };
}
test('two containing flows create independent IDs for one project and never adopt legacy content', async () => {
  const f = await fixture(); const a = await f.choose('one'), keyA = f.client.draftKey;
  expect(keyA).toBe('new-task:allocated-1'); expect(f.client.draft).toBe(''); expect(lookup(f.client, keyA)?.createdAt).toBe(new Date(time).toISOString());
  await f.act(a.owner, 'draft', a.draftOwner, 'draft A');
  const b = await f.choose('two'), keyB = f.client.draftKey; expect(keyB).not.toBe(keyA);
  await f.act(b.owner, 'draft', b.draftOwner, 'draft B');
  expect(list(f.client).map(record => record.key).sort()).toEqual([keyA, keyB]);
  expect(f.client.local.drafts[keyA]).toBe('draft A'); expect(f.client.local.drafts['env:new:a']).toBe('old A');
  expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(2);
});
test('same-flow project retarget keeps identity/content through child routes', async () => {
  const f = await fixture(), flow = await f.choose('one'), key = f.client.draftKey;
  await f.act(flow.owner, 'draft', flow.draftOwner, 'kept');
  f.client.local.snapshotDrafts[key] = [{ id: 'local', uploadId: 'remote' }];
  expect((await f.act(flow.owner, 'project', '["env","b"]')).message).toBe('');
  expect(f.client.draftKey).toBe(key); expect(lookup(f.client, key)?.projectId).toBe('b'); expect(f.client.draft).toBe('kept');
  expect(f.client.snapshotDrafts).toEqual([{ id: 'local', uploadId: 'remote' }]);
  for (const path of ['/new/draft/environment', '/new/draft/files/a.ts', '/new/draft/attachments/local']) {
    expect(f.snapshot('one', path, path)).toMatchObject({ ready: true, draftOwner: flow.draftOwner });
    expect(f.client.draftKey).toBe(key);
  }
  expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(1); expect(f.client.local.drafts['env:new:b']).toBe('old B');
});
test('full draftId resume uses hydrated stamped project, ignores competing query project and creates no new ID', async () => {
  const f = await fixture(), first = await f.choose('one'), key = f.client.draftKey; await f.act(first.owner, 'draft', first.draftOwner, 'saved');
  const r = await fixture(f.disk()), location = `/new/draft?draftId=${encodeURIComponent(key)}&environmentId=wrong&projectId=b`;
  expect(r.snapshot('resume', 'visit', location, false).status).toBe('loading');
  const loading = r.snapshot('resume', 'visit', location); expect(loading.needsPrepare).toBe(true);
  expect((await r.act(loading.owner, 'prepare')).message).toBe(''); expect(r.snapshot('resume', 'visit', location).ready).toBe(true);
  expect(r.client.draftKey).toBe(key); expect(r.client.draft).toBe('saved'); expect(r.client.projectId).toBe('a');
  expect(r.calls.filter(call => call.op === 'ids')).toEqual([]);
});
test('missing or archived stamped draft never falls back; outbox/share keep precedence', async () => {
  const f = await fixture(); create(f.client, { id: 'saved', origin, environmentId: 'env', projectId: 'b', createdAt: new Date(time).toISOString() });
  f.client.shell.projects[1]!.archivedAt = '2026-10-01';
  for (const tail of ['draftId=new-task:absent', 'draftId=new-task:saved', 'draftId=new-task:saved&pendingTaskId=p', 'draftId=new-task:saved&incomingShareId=s']) {
    expect(f.snapshot('flow', 'visit', `/new/draft?environmentId=env&projectId=a&${tail}`)).toMatchObject({ ready: false, nextLocation: '/new', needsPrepare: false });
  }
  expect(f.client.projectId).toBe('a'); expect(f.calls).toEqual([]);
});
test('clock is required before IDs and the observed wall clock owns creation timestamp', async () => {
  const f = await fixture(undefined, false), initial = f.snapshot('one', 'visit', '/new');
  expect(composerNow(f.client)).toBe(0); expect((await f.act(initial.owner, 'project', '["env","a"]')).message).toContain('clock');
  expect(f.calls.filter(call => call.op === 'ids')).toEqual([]);
  noteNow(f.client, time); const next = f.snapshot('two', 'visit', '/new');
  expect((await f.act(next.owner, 'project', '["env","a"]')).message).toBe(''); expect(lookup(f.client, f.client.draftKey)?.createdAt).toBe(new Date(time).toISOString());
});
test('late ID allocation after flow replacement creates no competing draft', async () => {
  const f = await fixture(); let reply!: (value: unknown) => void, entered!: () => void;
  const started = new Promise<void>(resolve => { entered = resolve; }), previous = f.native.later;
  f.native.later = async input => obj(input).op === 'ids' ? (entered(), await new Promise(resolve => { reply = resolve; })) : previous(input);
  const first = f.snapshot('one', 'visit', '/new'), choosing = f.act(first.owner, 'project', '["env","a"]');
  await started; f.snapshot('two', 'other', '/new'); reply({ ok: true, generation: 1, value: ['late-id'] });
  await expect(choosing).rejects.toMatchObject({ kind: 'superseded' }); expect(lookup(f.client, 'new-task:late-id')).toBeNull();
});
test('delayed local enqueue reply preserves A claim, never selects A over B, and recovers only A', async () => {
  const f = await fixture(), a = await f.choose('one'), keyA = f.client.draftKey;
  await f.act(a.owner, 'draft', a.draftOwner, 'same');
  let finish!: (value: unknown) => void, entered!: () => void;
  const started = new Promise<void>(resolve => { entered = resolve; }), previous = f.native.later;
  f.native.later = async input => {
    if (obj(input).op === 'mobileOutbox' && obj(input).action === 'enqueueTransfer') {
      const saved = await previous(input); entered(); return new Promise(resolve => { finish = () => resolve(saved); });
    }
    return previous(input);
  };
  const sending = f.act(a.owner, 'send'); await started;
  const claimA = [...f.claims.values()][0]!, b = await f.choose('two', 'b'), keyB = f.client.draftKey;
  await f.act(b.owner, 'draft', b.draftOwner, 'same');
  expect(keyB).not.toBe(keyA); expect(f.client.pending).toBeUndefined();
  expect(f.client.local.drafts[keyA]).toBe('same');
  finish(undefined);
  expect(await sending).toMatchObject({ message: '', nextLocation: '', submitted: false });
  expect(f.client.threadId).toBe(''); expect(f.client.projectId).toBe('b'); expect(f.client.draftKey).toBe(keyB); expect(f.client.pending).toBeUndefined();
  const recovered = await mobileNewTaskSubmitRecover(f.client, f.native, f.storage, claimA.transferId, () => false);
  expect(recovered).toMatchObject({ status: 'completed', disposition: 'stay', messageId: claimA.messageId });
  expect(lookup(f.client, keyA)).toBeNull(); expect(f.client.local.drafts[keyB]).toBe('same'); expect(f.client.threadId).toBe('');
  expect(f.client.local.drafts['env:new:a']).toBe('old A'); expect(f.client.projectId).toBe('b');
});
test('source environment match prefers fork origin identity and excludes known-different weak matches', () => {
  const selected: Obj = { title: 'Same', workspaceRoot: '/source/repo', repositoryIdentity: { canonicalKey: 'upstream', origin: { canonicalKey: 'fork' } } };
  const projects: Obj[] = [{ id: 'other', title: 'Same', workspaceRoot: '/repo', repositoryIdentity: { canonicalKey: 'different' } },
    { id: 'unknown', title: 'Same', workspaceRoot: '/else/repo' }, { id: 'fork', title: 'Other', workspaceRoot: '/checkout', repositoryIdentity: { canonicalKey: 'fork' } }];
  expect(mobileNewTaskEnvironmentMatch(projects, selected)?.id).toBe('fork');
  expect(mobileNewTaskEnvironmentMatch(projects.slice(0, 2), selected)?.id).toBe('unknown');
  expect(mobileNewTaskEnvironmentSources([{ environmentId: 'target', label: '', machine: '', config: {}, shell: { projects: [projects[0]!], threads: [], sequence: 0 }, focused: false }], selected)).toEqual([]);
});

test('real connect and shared synchronization retarget the same draft across repository and scratch environments', async () => {
  for (const scratch of [false, true]) {
    const f = await fixture();
    if (scratch) { f.client.config.scratchWorkspaceRoot = '/a'; f.client.shell.projects[0]!.workspaceRoot = '/a'; }
    const started = await f.choose('flow'), key = f.client.draftKey;
    await f.act(started.owner, 'draft', started.draftOwner, 'local content');
    f.client.local.snapshotDrafts[key] = [{ id: 'local-image', uploadId: 'old-upload' }];
    f.client.local.composerControls.contexts[key] = { envMode: 'worktree', branch: 'old', worktreePath: '/old' };
    const remoteProject = { id: 'remote-project', title: 'A', workspaceRoot: '/remote/a' }, remoteConfig = config('remote');
    if (scratch) remoteConfig.scratchWorkspaceRoot = '/remote/a';
    const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const, generation: 7, synchronized: 7,
      config: remoteConfig, shell: { projects: [remoteProject], threads: [], sequence: 0 }, scopes: ['orchestration:operate'],
      error: '', message: '', traceId: '', lastEvent: 0, subscriptions: {}, requested: true };
    f.fleet.entries.set(entry.key, entry); f.client.local.drafts['remote:new:remote-project'] = 'unrelated';
    const previous = f.native.later;
    f.native.later = async input => {
      const request = obj(input);
      if (request.fleet === entry.key) {
        f.calls.push(request);
        return { ok: true, generation: 7, value: request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate'] }
          : request.method === 'projects.ensureScratch' ? { projectId: 'remote-project' } : {} };
      }
      if (request.op === 'connect') { f.calls.push(request); return { ok: true, generation: 2,
        value: { state: 'connected', origin: entry.origin, environmentId: 'remote', message: '' } }; }
      if (f.client.environmentId === 'remote') {
        f.calls.push(request);
        return { ok: true, generation: 2, value: request.method === 'server.getConfig' ? remoteConfig
          : request.path === '/api/orchestration/shell' ? { snapshotSequence: 1, projects: [remoteProject], threads: [] }
            : request.path === '/api/auth/session' ? { authenticated: true, permissions: ['orchestration:operate'], scopes: ['orchestration:operate'] }
              : request.op === 'subscribe' ? { id: String(request.key) } : {} };
      }
      return previous(input);
    };
    const screen = f.snapshot('flow', 'environment', '/new/draft/environment');
    expect((await f.act(screen.owner, 'environment', 'remote', '', 'environment')).message).toBe('');
    expect(f.client.draftKey).toBe(key); expect(f.client.draft).toBe('local content');
    expect(lookup(f.client, key)).toMatchObject({ environmentId: 'remote', projectId: 'remote-project', origin: entry.origin });
    expect(f.client.snapshotDrafts).toEqual([{ id: 'local-image' }]);
    expect(f.client.local.drafts['remote:new:remote-project']).toBe('unrelated'); expect(f.client.local.drafts['env:new:a']).toBe('old A');
    expect(f.calls.filter(call => call.op === 'connect')).toHaveLength(1);
    expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(1);
    expect(f.calls.some(call => call.method === 'projects.ensureScratch')).toBe(scratch);
    expect(f.snapshot('flow', 'draft', '/new/draft').ready).toBe(true);
  }
});

test('lost local enqueue reply refuses captured draft retargeting without shared Pending or server dispatch', async () => {
  const f = await fixture(), flow = await f.choose('one'), key = f.client.draftKey;
  await f.act(flow.owner, 'draft', flow.draftOwner, 'captured'); const previous = f.native.later;
  f.native.later = async input => {
    const response = await previous(input);
    return obj(input).op === 'mobileOutbox' && obj(input).action === 'enqueueTransfer'
      ? { ok: false, generation: 1, error: { kind: 'storage', uncertain: true, message: 'reply lost' } } : response;
  };
  expect((await f.act(flow.owner, 'send')).message).toContain('Recover');
  f.snapshot('one'); const before = f.calls.length;
  expect((await f.act(flow.owner, 'project', '["env","b"]')).message).toContain('captured task transfer');
  expect(f.client.projectId).toBe('a'); expect(f.client.draftKey).toBe(key); expect(lookup(f.client, key)?.projectId).toBe('a');
  expect(f.calls.slice(before).some(call => call.op === 'ids' || call.method)).toBe(false);
  expect(f.client.pending).toBeUndefined(); expect(f.client.local.drafts[key]).toBe('captured');
  expect([...f.claims.values()][0]?.state).toBe('queued');
  expect(f.calls.some(call => call.method === 'orchestration.launchThread')).toBe(false);
});
test('a saved foreign-origin draft refuses resume without adopting another key or allocating IDs', async () => {
  const f = await fixture(); const record = create(f.client, { id: 'foreign', origin: 'https://foreign.invalid', environmentId: 'env', projectId: 'a', createdAt: new Date(time).toISOString() });
  f.client.local.drafts[record.key] = 'foreign content';
  const location = `/new/draft?draftId=${encodeURIComponent(record.key)}`, flow = f.snapshot('resume', 'visit', location);
  expect(flow.needsPrepare).toBe(true);
  expect((await f.act(flow.owner, 'prepare')).message).toContain('different environment');
  expect(f.snapshot('resume', 'visit', location).ready).toBe(false); expect(f.calls.some(call => call.op === 'ids')).toBe(false);
  expect(f.client.local.drafts[record.key]).toBe('foreign content'); expect(f.client.local.drafts['env:new:a']).toBe('old A');
});
test('a late connection reply after flow departure cannot retarget or bind the newer flow', async () => {
  const f = await fixture(), flow = await f.choose('one'), key = f.client.draftKey;
  await f.act(flow.owner, 'draft', flow.draftOwner, 'kept');
  const entry = { key: 'remote-key', origin: 'https://remote.test', environmentId: 'remote', phase: 'connected' as const, generation: 7, synchronized: 7,
    config: config('remote'), shell: { projects: [{ id: 'remote-a', title: 'A', workspaceRoot: '/remote/a' }], threads: [], sequence: 0 }, scopes: ['orchestration:operate'],
    error: '', message: '', traceId: '', lastEvent: 0, subscriptions: {}, requested: true };
  f.fleet.entries.set(entry.key, entry);
  let reply!: (value: unknown) => void, entered!: () => void; const started = new Promise<void>(resolve => { entered = resolve; }), previous = f.native.later;
  f.native.later = async input => obj(input).op === 'connect' ? (entered(), await new Promise(resolve => { reply = resolve; })) : previous(input);
  f.snapshot('one', 'environment', '/new/draft/environment');
  const moving = f.act(flow.owner, 'environment', 'remote', '', 'environment'); await started;
  f.snapshot('two', 'new', '/new'); reply({ ok: true, generation: 2, value: { state: 'connected', origin: entry.origin, environmentId: 'remote', message: '' } });
  await expect(moving).rejects.toMatchObject({ kind: 'superseded' });
  expect(lookup(f.client, key)).toMatchObject({ environmentId: 'env', projectId: 'a', origin }); expect(f.client.local.drafts[key]).toBe('kept');
  expect(f.snapshot('two', 'new', '/new').draftOwner).toBe('');
});
