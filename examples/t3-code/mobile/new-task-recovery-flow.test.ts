// @ref llp/1109.005-composer-and-transcript.decision.md#interrupted-transfer-controls-and-pending-editor-ownership
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskFlowView as view, mobileNewTaskFlowAction as action } from './new-task-flow';
import { mobileNewTaskDraftLookup as lookup } from './mobile-new-task-drafts';
import { noteNow } from './shared/composer-controls';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import type { MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
const time = 1791420000000, origin = 'https://test.invalid';
function config(environmentId = 'env'): Obj {
  return { environment: { environmentId, orchestrationProtocolVersion: 2, capabilities: { serverResolvedCommandContext: true } },
    providers: [{ instanceId: 'provider', driver: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' },
      models: [{ slug: 'model', name: 'Model', isDefault: true }] }], settings: { providerInstances: { provider: { driver: 'codex', enabled: true } } } };
}
async function fixture(saved: Obj = { version: 1 }, withClock = true) {
  const client = new MobileDraftClient(), fleet = new EnvironmentFleet(), calls: Obj[] = [];
  let disk = JSON.stringify(saved), serial = 0, interrupt: MobileOutboxTransferClaim['state'] | null = null;
  const claims = new Map<string, MobileOutboxTransferClaim>();
  const outboxRows = new Map<string, Obj>();
  const outcome = (claim: MobileOutboxTransferClaim) => claim.state === 'prepared' ? null : ({ mutationId: claim.mutationId, messageId: claim.messageId,
    status: claim.state === 'failed' ? 'failed' : 'committed', revision: 1, record: claim.state === 'failed' ? null : claim.record, removed: null, message: '', ownerEpoch: 'epoch', sequenceFloor: 1,
    current: outboxRows.get(claim.messageId) });
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
    async atomicWriteFile(_path, bytes) { disk = new TextDecoder().decode(bytes); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'mobileOutbox') {
      let value: unknown;
      if (request.action === 'read') value = { ownerEpoch: 'epoch', sequenceFloor: claims.size, complete: true, errors: [],
        records: [...outboxRows.values()], outcomes: [], mutations: [...claims.values()].filter(c => c.state === 'prepared').map(c => ({ messageId: c.messageId, state: 'pending', mutation: { operation: 'enqueue', messageId: c.messageId, mutationId: c.mutationId, record: c.record, transfer: c } })), transfers: [...claims.values()],
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
          state: interrupt ?? 'queued', record, capture };
        claims.set(claim.transferId, claim);
        outboxRows.set(record.messageId, { record, revision: 1, token: request.mutationId, pending: interrupt === 'prepared', held: false });
        value = { disposition: 'created', claim, outcome: outcome(claim) };
        if (interrupt) return { ok: false, generation: client.generation, error: { kind: 'storage', uncertain: true, message: 'Interrupted enqueue reply' } };
      } else if (request.action === 'completeTransfer') {
        const claim = claims.get(String(request.transferId))!;
        const completed = { ...claim, state: 'completed' as const, capture: null, record: null };
        claims.set(claim.transferId, completed); value = { completed: true, claim: completed };
      } else if (request.action === 'recover') {
        const claim = claims.get(String(request.messageId))!;
        claim.state = request.decision === 'rollback' ? 'failed' : 'queued';
        outboxRows.set(claim.messageId, { record: claim.record, revision: 1, token: claim.mutationId, pending: false, held: false });
        value = outcome(claim);
      } else if (request.action === 'releaseFailedTransfer') {
        const claim = claims.get(String(request.transferId))!;
        const released = { ...claim, state: 'released' as const, capture: null, record: null };
        claims.set(claim.transferId, released); value = { released: true, claim: released };
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
  return { client, fleet, calls, native, storage, snapshot, act, choose, claims, interrupt(state: MobileOutboxTransferClaim['state']) { interrupt = state; }, disk: () => obj(JSON.parse(disk)) };
}

async function interrupted(state: MobileOutboxTransferClaim['state']) {
  const f = await fixture(), initial = await f.choose('one');
  await f.act(initial.owner, 'draft', initial.draftOwner, 'captured'); f.interrupt(state);
  const result = await f.act(initial.owner, 'send');
  expect(result).toMatchObject({ submitted: false, nextLocation: '' });
  return { ...f, owner: initial.owner, key: f.client.draftKey, screen: f.snapshot('one') };
}
const recoveryKey = (snapshot: ReturnType<typeof view>, kind: string) => {
  const found = snapshot.recovery.actions.find(item => JSON.parse(item.key).kind === kind);
  if (!found) throw Error('Missing ' + kind + ': ' + JSON.stringify(snapshot));
  return found.key;
};
test('post-Send prepared recovery offers exact commit and rollback; commit navigates only after cleanup', async () => {
  const f = await interrupted('prepared');
  expect(f.screen).toMatchObject({ ready: true, recovery: { visible: true, blocked: true } });
  expect(f.screen.recovery.actions.map(item => JSON.parse(item.key).kind)).toEqual(['commit', 'rollback']);
  const original = [...f.claims.values()][0]!;
  const result = await f.act(f.owner, 'transfer-recovery', recoveryKey(f.screen, 'commit'));
  expect(result).toMatchObject({ submitted: false, nextLocation: '/', threadId: original.threadId });
  expect(lookup(f.client, f.key)).toBeNull(); expect(f.claims.get(original.transferId)?.state).toBe('completed');
  expect(f.calls.filter(call => call.action === 'enqueueTransfer')).toHaveLength(1);
  expect(f.calls.some(call => call.method === 'orchestration.launchThread')).toBe(false);
});
test('queued recovery survives reopening original draftId and completes without generating another task', async () => {
  const f = await interrupted('queued'), original = [...f.claims.values()][0]!;
  const location = '/new/draft?draftId=' + encodeURIComponent(f.key);
  const loading = f.snapshot('resume', 'resume', location);
  expect(loading.needsPrepare).toBe(true);
  expect((await f.act(loading.owner, 'prepare', '', '', 'resume')).message).toBe('');
  const ready = f.snapshot('resume', 'resume', location);
  expect(ready.recovery).toMatchObject({ visible: true, blocked: true });
  const before = f.calls.filter(call => call.op === 'ids').length;
  expect(await f.act(ready.owner, 'transfer-recovery', recoveryKey(ready, 'finish'), '', 'resume'))
    .toMatchObject({ nextLocation: '/', submitted: false, threadId: original.threadId });
  expect(f.calls.filter(call => call.op === 'ids')).toHaveLength(before);
});
test('failed release keeps draft selected, removes blocking notice and never resends', async () => {
  const f = await interrupted('failed');
  expect(await f.act(f.owner, 'transfer-recovery', recoveryKey(f.screen, 'release'))).toMatchObject({ nextLocation: '', submitted: false, message: '' });
  expect(f.snapshot('one')).toMatchObject({ ready: true, recovery: { visible: false, blocked: false } });
  expect(f.client.draftKey).toBe(f.key); expect(f.client.draft).toBe('captured');
  expect(f.calls.filter(call => call.action === 'enqueueTransfer')).toHaveLength(1);
  expect(f.calls.some(call => call.method === 'orchestration.launchThread')).toBe(false);
});
test('rollback requires a separate explicit release and stale original commit control cannot resend', async () => {
  const f = await interrupted('prepared'), oldCommit = recoveryKey(f.screen, 'commit');
  expect(await f.act(f.owner, 'transfer-recovery', recoveryKey(f.screen, 'rollback'))).toMatchObject({ nextLocation: '', submitted: false });
  const failed = f.snapshot('one');
  expect(failed.recovery.actions.map(item => JSON.parse(item.key).kind)).toEqual(['release']);
  const rejected = await f.act(f.owner, 'transfer-recovery', oldCommit);
  expect(rejected.message).toContain('changed'); expect(f.client.draft).toBe('captured');
  expect(f.calls.filter(call => call.action === 'recover')).toHaveLength(1);
});
test('late recovery completion never navigates or cleans up a replacement flow', async () => {
  const f = await interrupted('prepared');
  let entered!: () => void, release!: () => void;
  const started = new Promise<void>(resolve => entered = resolve), wait = new Promise<void>(resolve => release = resolve);
  const previous = f.native.later;
  f.native.later = async input => {
    const response = await previous(input);
    if (obj(input).action === 'recover') { entered(); await wait; }
    return response;
  };
  const recovering = f.act(f.owner, 'transfer-recovery', recoveryKey(f.screen, 'commit'));
  await started;
  const other = await f.choose('two', 'b'), otherKey = f.client.draftKey;
  await f.act(other.owner, 'draft', other.draftOwner, 'new task'); release();
  await expect(recovering).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.client.draftKey).toBe(otherKey); expect(f.client.draft).toBe('new task');
  expect(f.client.local.drafts[f.key]).toBe('captured'); expect([...f.claims.values()][0]?.state).toBe('queued');
  expect(f.snapshot('two')).toMatchObject({ ready: true, recovery: { visible: false, blocked: false } });
  expect(f.calls.filter(call => call.action === 'enqueueTransfer')).toHaveLength(1);
});
test('resuming a captured draft with branch route cannot retarget its workspace before recovery', async () => {
  const f = await interrupted('prepared');
  const before = JSON.stringify(f.client.local.composerControls.contexts[f.key]);
  const location = '/new/draft?draftId=' + encodeURIComponent(f.key) + '&branch=other&worktreePath=%2Fother';
  const loading = f.snapshot('resume-branch', 'branch', location);
  const result = await f.act(loading.owner, 'prepare', '', '', 'branch');
  expect(result.message).toContain('captured task transfer');
  expect(JSON.stringify(f.client.local.composerControls.contexts[f.key])).toBe(before);
  expect([...f.claims.values()][0]?.state).toBe('prepared');
});
