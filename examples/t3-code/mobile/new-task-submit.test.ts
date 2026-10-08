// Sequencing regressions use controlled native replies. Native durability is checked separately.
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftPresentation, mobileNewTaskDraftChanged, mobileNewTaskDraftBind } from './mobile-new-task-drafts';
import { mobileNewTaskSubmit as submit, mobileNewTaskSubmitRecover as recover } from './new-task-submit';
import { mobileCaptureNewTaskOutbox, type MobileOutboxCaptureFacts } from './mobile-outbox-capture';
import { noteNow } from './shared/composer-controls';
import { type Files, type Native } from './shared/protocol';
import type { MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
import { mobileDraftAttachmentRecord } from './draft-attachment-order';

const key = 'new-task:A', origin = 'https://server.test';
const copy = <T>(x: T): T => JSON.parse(JSON.stringify(x));
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>(r => resolve = r);
  return { promise, resolve };
}
async function fixture() {
  const client = new MobileDraftClient(), calls: any[] = [], writes: any[] = [];
  let disk: any = { version: 1 }, ready = false, writeCount = 0, active = true, failMarker = '', complete = true;
  const hooks = new Map<string, () => Promise<void>>(), claims = new Map<string, MobileOutboxTransferClaim>();
  const facts: MobileOutboxCaptureFacts = {
    key, origin, environmentId: 'env', projectId: 'project', config: null,
    selectedModel: { instanceId: 'p', model: 'm' }, defaultRuntimeMode: 'auto', connected: false, canOperate: true,
    planPreferenceLoaded: true, planModeEnabled: true,
    workspace: { canChoose: true, mode: 'local', worktreePath: null, explicitBranch: null,
      currentCheckoutBranch: 'main', startFromOrigin: false },
  };
  const runHook = async (name: string) => {
    const h = hooks.get(name);
    if (h) { hooks.delete(name); await h(); }
  };
  const current = (claim: MobileOutboxTransferClaim) => ({
    record: claim.record, revision: 1, token: claim.mutationId, pending: claim.state === 'prepared',
  });
  const outcome = (claim: MobileOutboxTransferClaim): any => claim.state === 'prepared' ? null : ({
    mutationId: claim.mutationId, messageId: claim.messageId, status: claim.state === 'failed' ? 'failed' : 'committed',
    revision: 1, record: claim.state === 'failed' ? null : claim.record, removed: null, message: '',
    ownerEpoch: 'epoch', sequenceFloor: 1, current: current(claim),
  });
  const value = async (request: any): Promise<any> => {
    if (request.op === 'ids') return ['new-thread', 'new-message', 'new-command'];
    if (request.op === 'status') return { phase: client.connection, origin, homeOrigin: origin, environmentId: 'env' };
    if (request.op === 'mobilePreferences') return { planModeEnabled: true };
    if (request.op === 'http') return { authenticated: true, permissions: ['orchestration:operate'] };
    if (request.op === 'subscribeVcsStatus') return { id: 'subscription' };
    if (request.op !== 'mobileOutbox') throw Error('unexpected operation ' + request.op);
    switch (request.action) {
      case 'read': return {
        ownerEpoch: 'epoch', sequenceFloor: 1, complete, errors: [], records: [], outcomes: [], mutations: [],
        revisions: {}, tokens: {}, transfers: [...claims.values()],
      };
      case 'transferLookup': return {
        complete, fingerprint: request.capture ? 'a'.repeat(64) : null,
        claims: [...claims.values()].filter(c => c.draftKey === request.draftKey),
      };
      case 'transferStatus': {
        const c = claims.get(request.transferId);
        return { claim: c ?? null, outcome: c ? outcome(c) : null };
      };
      case 'enqueueTransfer': {
        const c: MobileOutboxTransferClaim = {
          transferId: request.record.messageId, draftKey: request.capture.draft.key, fingerprint: 'a'.repeat(64),
          messageId: request.record.messageId, threadId: request.record.threadId, commandId: request.record.commandId,
          mutationId: request.mutationId, state: 'queued', record: copy(request.record), capture: copy(request.capture),
        };
        claims.set(c.transferId, c);
        return { disposition: 'created', claim: c, outcome: outcome(c) };
      }
      case 'completeTransfer': {
        const c = claims.get(request.transferId)!;
        if (!disk.mobileOutboxTransferCompletions?.[c.transferId]) throw Error('marker absent');
        const done = { ...c, state: 'completed' as const, record: null, capture: null };
        claims.set(c.transferId, done);
        return { completed: true, claim: done };
      };
      case 'releaseFailedTransfer': {
        const c = claims.get(request.transferId)!;
        const next = { ...c, state: 'released' as const, record: null, capture: null };
        claims.set(c.transferId, next);
        return { released: true, claim: next };
      };
      case 'recover': {
        const c = claims.get(request.messageId)!;
        const next = { ...c, state: request.decision === 'rollback' ? 'failed' as const : 'queued' as const };
        claims.set(c.transferId, next);
        return outcome(next);
      };
      default: throw Error('unexpected action ' + request.action);
    }
  };
  const native: Native = {
    available: true, watch() {},
    async later(raw) {
      const request = raw as any;
      if (ready) {
        calls.push(copy(request));
        await runHook(request.action === 'transferLookup'
          ? (request.capture ? 'lookup-capture' : 'lookup-key') : request.action ?? request.op);
      }
      return { ok: true, generation: 1, value: await value(request) };
    },
  };
  const storage: Files = { fs: {
    async mkdir() {},
    async readFile() { return new TextEncoder().encode(JSON.stringify(disk)).buffer; },
    async atomicWriteFile(_p, bytes) {
      const document = JSON.parse(new TextDecoder().decode(bytes));
      if (ready) {
        writeCount++; writes.push(copy(document));
        await runHook('persist:' + writeCount);
      }
      if (document.mobileOutboxTransferCompletions && failMarker === 'before') throw Error('marker save failed');
      disk = document;
      if (document.mobileOutboxTransferCompletions && failMarker === 'after') throw Error('marker save reply lost');
    },
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage);
  await client.refresh(handles.native, handles.storage);
  Object.assign(client, { origin, environmentId: 'env', projectId: 'project', threadId: '', generation: 1,
    connection: 'connected', configLive: true, shellLoaded: true, shellLive: true });
  client.config = { settings: { defaultModelSelection: { instanceId: 'p', model: 'm' },
    providerInstances: { p: { driver: 'codex', enabled: true } } }, environment: { capabilities: { attachmentUploads: true } },
    providers: [{ instanceId: 'p', driver: 'codex', enabled: true, installed: true,
      auth: { status: 'authenticated' }, models: [{ slug: 'm', isDefault: true }] }] };
  client.shell.projects = [{ id: 'project', title: 'Project', workspaceRoot: '/repo', archivedAt: null }];
  for (const id of ['A', 'B']) {
    mobileNewTaskDraftCreate(client, { id, origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' });
    client.local.drafts['new-task:' + id] = ' original ';
  }
  mobileNewTaskDraftBind(client, key, 'flow');
  noteNow(client, Date.parse('2026-10-08T01:00:00.000Z')); ready = true;
  const input = { draftKey: key, now: Date.parse('2026-10-08T01:00:00.000Z'), current: () => active, facts: () => facts };
  const seed = (state: MobileOutboxTransferClaim['state']) => {
    const draft = mobileNewTaskDraftPresentation(client, key)!;
    const prepared = mobileCaptureNewTaskOutbox(draft, facts, {
      threadId: 'old-thread', messageId: 'old-message', commandId: 'old-command', createdAt: '2026-10-08T00:30:00.000Z',
    });
    if (prepared.status !== 'ready') throw Error(prepared.reason);
    const c: MobileOutboxTransferClaim = {
      transferId: 'old-message', messageId: 'old-message', threadId: 'old-thread', commandId: 'old-command',
      mutationId: 'epoch:1', draftKey: key, fingerprint: 'a'.repeat(64), state,
      record: ['completed', 'released'].includes(state) ? null : prepared.record,
      capture: ['completed', 'released'].includes(state) ? null : { version: 1, draft },
    };
    claims.set(c.transferId, c);
    return c;
  };
  const gate = (name: string) => {
    const reached = deferred(), release = deferred();
    hooks.set(name, async () => { reached.resolve(); await release.promise; });
    return { reached: reached.promise, release: release.resolve };
  };
  return {
    client, native, storage, input, calls, writes, facts, claims, seed, gate, hooks,
    setComplete(v: boolean) { complete = v; }, setActive(v: boolean) { active = v; },
    setMarkerFailure(v: string) { failMarker = v; }, disk: () => disk,
  };
}
const actions = (f: Awaited<ReturnType<typeof fixture>>) => f.calls.map(c => c.action ?? c.op);

test('online submit completes local transfer and returns queued thread without server writes or selection', async () => {
  const f = await fixture();
  const before = [f.client.environmentId, f.client.projectId, f.client.threadId, f.client.threadEpoch];
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'completed', disposition: 'thread', draftRetained: false,
    owner: { origin, environmentId: 'env', threadId: 'new-thread', messageId: 'new-message', commandId: 'new-command' } });
  expect([f.client.environmentId, f.client.projectId, f.client.threadId, f.client.threadEpoch]).toEqual(before);
  expect(actions(f).indexOf('enqueueTransfer')).toBeGreaterThan(actions(f).indexOf('ids'));
  expect(actions(f)).toContain('completeTransfer');
  expect(f.calls.some(call => ['orchestration.launchThread', 'orchestration.dispatchCommand'].includes(call.method))).toBe(false);
  expect(f.client.local.drafts['new-task:B']).toBe(' original ');
});
test('offline capture queues and returns pending disposition without auth or server requests', async () => {
  const f = await fixture(); f.client.connection = 'disconnected';
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'completed', disposition: 'pending', threadId: 'new-thread' });
  expect(actions(f)).not.toContain('http'); expect(actions(f)).not.toContain('request');
});
test('modern local attachment keeps the source pending navigation while upload is outstanding', async () => {
  const f = await fixture(), id = '11111111-1111-4111-a111-111111111111';
  f.client.local.snapshotDrafts[key] = [{ id, name: 'image.png', mimeType: 'image/png', sizeBytes: 12 }];
  mobileDraftAttachmentRecord(f.client, key, [id]);
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'completed', disposition: 'pending', draftRetained: false });
  expect(actions(f)).not.toContain('upload');
});
test('connected denied operate grant cannot enqueue even though offline capture is allowed', async () => {
  const f = await fixture(), base = f.native.later;
  f.native.later = async raw => (raw as any).op === 'http'
    ? { ok: true, generation: 1, value: { authenticated: true, permissions: [] } } : base(raw);
  expect(await submit(f.client, f.native, f.storage, f.input)).toMatchObject({ status: 'blocked', disposition: 'stay',
    message: 'This connection cannot start tasks.' });
  expect(actions(f)).not.toContain('enqueueTransfer'); expect(f.client.local.drafts[key]).toBe(' original ');
});
test('failed capture leaves draft and makes no enqueue or navigation', async () => {
  const f = await fixture(); f.client.local.drafts[key] = '';
  expect(await submit(f.client, f.native, f.storage, f.input)).toMatchObject({ status: 'blocked', disposition: 'stay',
    owner: null, threadId: '', draftRetained: true });
  expect(actions(f)).not.toContain('enqueueTransfer'); expect(f.writes).toHaveLength(0);
});
test('newer draft text during enqueue remains bound and prevents automatic navigation', async () => {
  const f = await fixture();
  f.hooks.set('enqueueTransfer', async () => { f.client.local.drafts[key] = 'newer'; mobileNewTaskDraftChanged(f.client, key); });
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'completed', disposition: 'stay', draftRetained: true });
  expect(f.client.local.drafts[key]).toBe('newer'); expect(f.client.draftKey).toBe(key);
  expect(result.owner?.messageId).toBe('new-message');
});
test('route departure after admission keeps durable identity and refuses navigation', async () => {
  const f = await fixture(); f.hooks.set('enqueueTransfer', async () => { f.setActive(false); });
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'completed', disposition: 'stay', messageId: 'new-message' });
  expect(f.claims.get('new-message')?.state).toBe('completed');
});
test('destination change after admission cannot navigate to the old task', async () => {
  const f = await fixture(); f.hooks.set('enqueueTransfer', async () => { f.client.environmentId = 'other'; });
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'completed', disposition: 'stay', owner: { origin, environmentId: 'env' } });
  expect(f.client.environmentId).toBe('other');
});
test('queued transfer recovery keeps persisted IDs and skips fresh capture reads', async () => {
  const f = await fixture(); f.seed('queued');
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'completed', disposition: 'pending', threadId: 'old-thread',
    messageId: 'old-message', commandId: 'old-command', owner: { origin, environmentId: 'env' } });
  expect(actions(f)).not.toContain('ids'); expect(actions(f)).not.toContain('mobilePreferences');
});
test('interrupted enqueue remains recovery-required without a new ID or navigation', async () => {
  const f = await fixture(); f.seed('prepared');
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'recovery-required', disposition: 'stay', messageId: 'old-message', draftRetained: true });
  expect(actions(f)).not.toContain('ids'); expect(actions(f)).not.toContain('enqueueTransfer');
});
test('explicit transfer commit uses original identity and pending disposition', async () => {
  const f = await fixture(); f.seed('prepared');
  const result = await recover(f.client, f.native, f.storage, 'old-message', f.input.current, 'commit');
  expect(result).toMatchObject({ status: 'completed', disposition: 'pending', messageId: 'old-message' });
  expect(actions(f)).toContain('recover'); expect(actions(f)).not.toContain('ids');
});
test('cleanup failure is still a queued claim but never a successful navigation', async () => {
  const f = await fixture(); f.setMarkerFailure('before');
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'cleanup-pending', disposition: 'stay', owner: { messageId: 'new-message' } });
  expect(f.claims.get('new-message')?.state).toBe('queued');
});
test('completed claim with no surviving queue row cannot invent its original origin', async () => {
  const f = await fixture(); f.seed('completed');
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result).toMatchObject({ status: 'completed', disposition: 'stay', owner: null,
    messageId: 'old-message', threadId: 'old-thread', commandId: 'old-command' });
  expect(actions(f)).not.toContain('ids');
});
