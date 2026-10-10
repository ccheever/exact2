// @ref llp/1109.005-composer-and-transcript.decision.md#interrupted-transfer-controls-and-pending-editor-ownership
// Sequencing regressions use controlled native replies. Native durability is checked separately.
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftPresentation, mobileNewTaskDraftChanged, mobileNewTaskDraftBind } from './mobile-new-task-drafts';
import { mobileNewTaskTransferRecoveryRead as read, mobileNewTaskTransferRecoveryAction as act, mobileNewTaskTransferRecoveryPresentation as present } from './mobile-new-task-transfer-recovery';
import { mobileCaptureNewTaskOutbox, type MobileOutboxCaptureFacts } from './mobile-outbox-capture';
import { noteNow } from './shared/composer-controls';
import { type Files, type Native } from './shared/protocol';
import type { MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';

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
  let pendingMarker = true, missingStatus = false;
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
        ownerEpoch: 'epoch', sequenceFloor: 1, complete, errors: [], records: [], outcomes: [], mutations: pendingMarker ? [...claims.values()].filter(c => c.state === 'prepared').map(c => ({
          messageId: c.messageId, state: 'pending', mutation: { messageId: c.messageId, mutationId: c.mutationId,
            operation: 'enqueue', record: c.record, transfer: c } })) : [],
        revisions: {}, tokens: {}, transfers: [...claims.values()],
      };
      case 'transferLookup': return {
        complete, fingerprint: request.capture ? 'a'.repeat(64) : null,
        claims: [...claims.values()].filter(c => c.draftKey === request.draftKey),
      };
      case 'transferStatus': {
        const c = missingStatus ? undefined : claims.get(request.transferId);
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
    setComplete(v: boolean) { complete = v; }, setPending(v: boolean) { pendingMarker = v; }, setMissing(v: boolean) { missingStatus = v; }, setActive(v: boolean) { active = v; },
    setMarkerFailure(v: string) { failMarker = v; }, disk: () => disk,
  };
}
const actions = (f: Awaited<ReturnType<typeof fixture>>) => f.calls.map(c => c.action ?? c.op);

const request = (f: Awaited<ReturnType<typeof fixture>>) => ({ draftKey: key, current: f.input.current });
async function button(f: Awaited<ReturnType<typeof fixture>>, kind: string) {
  const snapshot = await read(f.client, f.native, request(f));
  const action = snapshot.items.flatMap(item => item.actions).find(action => action.kind === kind);
  if (!action) throw Error('Missing action ' + kind + ': ' + JSON.stringify(snapshot));
  return action.key;
}
test('empty and retired-only histories keep recovery UI hidden without weakening duplicate authority', async () => {
  const f = await fixture();
  expect(present(await read(f.client, f.native, request(f)))).toEqual({ visible: false, message: '', actions: [] });
  f.seed('completed');
  const complete = await read(f.client, f.native, request(f));
  expect(complete).toMatchObject({ complete: true, blocksSend: false, items: [{ state: 'completed', actions: [] }] });
  expect(present(complete).visible).toBe(false); expect(f.claims.get('old-message')?.state).toBe('completed');
});
test('exact prepared enqueue offers commit and rollback without allocating or sending', async () => {
  const f = await fixture(); f.seed('prepared');
  const snapshot = await read(f.client, f.native, request(f));
  expect(snapshot).toMatchObject({ complete: true, blocksSend: true, items: [{ state: 'prepared' }] });
  expect(snapshot.items[0]!.actions.map(action => action.kind)).toEqual(['commit', 'rollback']);
  expect(present(snapshot).actions).toHaveLength(2); expect(f.writes).toEqual([]);
  expect(actions(f)).toEqual(['read', 'transferLookup', 'transferStatus']);
});
test('prepared terminal durability uncertainty offers retry rather than commit or rollback', async () => {
  const f = await fixture(); f.seed('prepared'); f.setPending(false);
  const snapshot = await read(f.client, f.native, request(f));
  expect(snapshot.items[0]!.actions.map(action => action.kind)).toEqual(['retry']);
  const result = await act(f.client, f.native, f.storage, request(f), snapshot.items[0]!.actions[0]!.key);
  expect(result.submit).toMatchObject({ status: 'completed', messageId: 'old-message', commandId: 'old-command' });
  expect(f.calls.find(call => call.action === 'recover')).toMatchObject({ messageId: 'old-message', mutationId: 'epoch:1', decision: 'retry' });
  expect(actions(f)).not.toContain('ids');
});
test('explicit commit finishes captured cleanup using original identity and preserves another draft', async () => {
  const f = await fixture(); f.seed('prepared');
  const key = await button(f, 'commit'), result = await act(f.client, f.native, f.storage, request(f), key);
  expect(result).toMatchObject({ released: false, submit: { status: 'completed', threadId: 'old-thread', messageId: 'old-message' } });
  expect(f.client.local.drafts['new-task:B']).toBe(' original ');
  expect(f.client.local.drafts[f.input.draftKey]).toBeUndefined();
  expect(actions(f)).not.toContain('ids'); expect(f.calls.some(call => call.method)).toBe(false);
  expect(f.client.threadId).toBe('');
});
test('explicit rollback keeps draft then separate release retires failed claim without resubmission', async () => {
  const f = await fixture(); f.seed('prepared');
  const rolled = await act(f.client, f.native, f.storage, request(f), await button(f, 'rollback'));
  expect(rolled.submit).toMatchObject({ status: 'failed', claim: { state: 'failed' } });
  expect(rolled.message).toBe('');
  expect(f.client.local.drafts[key]).toBe(' original ');
  const released = await act(f.client, f.native, f.storage, request(f), await button(f, 'release'));
  expect(released).toMatchObject({ released: true, submit: null, message: '' });
  expect(f.claims.get('old-message')?.state).toBe('released'); expect(f.client.local.drafts[key]).toBe(' original ');
  expect(actions(f)).not.toContain('ids'); expect(actions(f)).not.toContain('enqueueTransfer');
  expect(present(await read(f.client, f.native, request(f))).visible).toBe(false);
});
test('queued cleanup retains newer same-draft content and completes only its captured marker', async () => {
  const f = await fixture(); f.seed('queued');
  f.client.local.drafts[key] = 'newer'; mobileNewTaskDraftChanged(f.client, key);
  const result = await act(f.client, f.native, f.storage, request(f), await button(f, 'finish'));
  expect(result.submit).toMatchObject({ status: 'completed', draftRetained: true, disposition: 'stay' });
  expect(f.client.local.drafts[key]).toBe('newer'); expect(f.disk().mobileOutboxTransferCompletions).toHaveProperty('old-message');
});
test('partial inventory and missing native claim expose refresh-only state', async () => {
  const f = await fixture(); f.seed('prepared'); f.setComplete(false);
  const partial = await read(f.client, f.native, request(f));
  expect(partial).toMatchObject({ complete: false, blocksSend: true, items: [{ actions: [] }] });
  expect(present(partial)).toMatchObject({ visible: true, actions: [] });
  f.setComplete(true); f.setMissing(true);
  expect(await read(f.client, f.native, request(f))).toMatchObject({ complete: false, blocksSend: true, items: [{ state: 'unknown', actions: [] }] });
});
test('conflicting active claims are never independently recoverable from one draft control', async () => {
  const f = await fixture(), first = f.seed('queued');
  const duplicate = copy(first); Object.assign(duplicate, { transferId: 'second', messageId: 'second', commandId: 'second-command', threadId: 'second-thread' });
  Object.assign(duplicate.record!, { messageId: 'second', commandId: 'second-command', threadId: 'second-thread' });
  f.claims.set('second', duplicate);
  const snapshot = await read(f.client, f.native, request(f));
  expect(snapshot.message).toContain('More than one'); expect(snapshot.items.every(item => item.actions.length === 0)).toBe(true);
});
test('old action state and mutation evidence cannot silently select a newer recovery policy', async () => {
  const f = await fixture(); f.seed('prepared'); const selected = await button(f, 'rollback');
  f.setPending(false); f.calls.length = 0;
  expect(await act(f.client, f.native, f.storage, request(f), selected)).toMatchObject({ submit: null, released: false });
  expect(actions(f)).not.toContain('recover'); expect(f.writes).toEqual([]);
});
test('changed native status inside resume refuses before cleanup or recovery mutation', async () => {
  const f = await fixture(); f.seed('prepared'); const selected = await button(f, 'rollback');
  const base = f.native.later; let reads = 0;
  f.native.later = async raw => {
    const r = raw as any;
    if (r.action === 'transferStatus' && ++reads === 2) f.claims.get('old-message')!.state = 'queued';
    return base(raw);
  };
  await expect(act(f.client, f.native, f.storage, request(f), selected)).rejects.toMatchObject({ kind: 'superseded' });
  expect(actions(f)).not.toContain('recover'); expect(actions(f)).not.toContain('completeTransfer'); expect(f.writes).toEqual([]);
});
test('stale route before action refuses all native work', async () => {
  const f = await fixture(); f.seed('queued'); const selected = await button(f, 'finish');
  f.setActive(false); f.calls.length = 0;
  await expect(act(f.client, f.native, f.storage, request(f), selected)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls).toEqual([]); expect(f.writes).toEqual([]);
});
test('route change after native recovery admission leaves original queue evidence and stops cleanup', async () => {
  const f = await fixture(); f.seed('prepared'); const selected = await button(f, 'commit');
  f.hooks.set('recover', async () => { f.setActive(false); });
  await expect(act(f.client, f.native, f.storage, request(f), selected)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.claims.get('old-message')?.state).toBe('queued'); expect(f.client.local.drafts[key]).toBe(' original ');
  expect(actions(f)).not.toContain('completeTransfer'); expect(f.writes).toEqual([]);
});
test('failed cleanup is recoverable without losing its original capture', async () => {
  const f = await fixture(); f.seed('queued'); f.setMarkerFailure('before');
  const result = await act(f.client, f.native, f.storage, request(f), await button(f, 'finish'));
  expect(result.submit?.status).toBe('cleanup-pending'); expect(f.claims.get('old-message')?.state).toBe('queued');
  expect((await read(f.client, f.native, request(f))).items[0]!.actions.map(action => action.kind)).toEqual(['finish']);
});
test('tampered or foreign action key never releases or recovers any claim', async () => {
  const f = await fixture(); f.seed('failed'); const selected = await button(f, 'release');
  for (const key of ['null', '[]', '{', JSON.stringify({ ...JSON.parse(selected), draftKey: 'new-task:B' }),
    JSON.stringify({ ...JSON.parse(selected), fingerprint: 'b'.repeat(64) })]) {
    expect(await act(f.client, f.native, f.storage, request(f), key)).toMatchObject({ released: false, submit: null });
  }
  expect(actions(f)).not.toContain('releaseFailedTransfer'); expect(actions(f)).not.toContain('recover');
});

test('a concurrent recovery exposes no second action and admits only one failed release', async () => {
  const f = await fixture(); f.seed('failed'); const selected = await button(f, 'release');
  const gate = f.gate('releaseFailedTransfer');
  const first = act(f.client, f.native, f.storage, request(f), selected);
  await gate.reached;
  const snapshot = await read(f.client, f.native, request(f));
  expect(snapshot.busy).toBe(true); expect(present(snapshot).actions).toEqual([]);
  expect(await act(f.client, f.native, f.storage, request(f), selected)).toMatchObject({ released: false, submit: null });
  gate.release();
  expect((await first).released).toBe(true);
  expect(actions(f).filter(action => action === 'releaseFailedTransfer')).toHaveLength(1);
});
test('a pending marker for another fingerprint grants neither recovery interpretation', async () => {
  const f = await fixture(); f.seed('prepared');
  const base = f.native.later;
  f.native.later = async raw => {
    const reply = await base(raw) as any;
    if ((raw as any).action === 'read') {
      reply.value = copy(reply.value);
      reply.value.mutations[0].mutation.transfer.fingerprint = 'b'.repeat(64);
    }
    return reply;
  };
  const snapshot = await read(f.client, f.native, request(f));
  expect(snapshot.items[0]!.actions).toEqual([]);
  expect(snapshot.items[0]!.message).toContain('does not match');
  expect(present(snapshot).visible).toBe(true);
});
