// Sequencing regressions use controlled native replies. Native durability is checked separately.
import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftPresentation, mobileNewTaskDraftChanged, mobileNewTaskDraftStore } from './mobile-new-task-drafts';
import { mobileNewTaskTransferSubmit as submit, mobileNewTaskTransferResume as resume,
  mobileNewTaskTransferBusy as busy } from './new-task-transfer';
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
    if (request.op !== 'mobileOutbox') return request.op === 'status' ? { phase: 'disconnected' } : {};
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
  Object.assign(client, { origin, environmentId: 'env', projectId: 'project', threadId: '', generation: 1 });
  for (const id of ['A', 'B']) {
    mobileNewTaskDraftCreate(client, { id, origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' });
    client.local.drafts['new-task:' + id] = ' original ';
  }
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

test('inactive returning caller cannot allocate IDs or admit a new transfer', async () => {
  const f = await fixture(); f.setActive(false);
  await expect(submit(f.client, f.native, f.storage, f.input)).rejects.toMatchObject({ kind: 'superseded' });
  expect(actions(f)).toEqual(['read', 'transferLookup']); expect(f.writes).toHaveLength(0);
  expect(busy(f.client, key)).toBe(false);
});
test('full draft changes at read, persistence and IDs refuse before admission', async () => {
  for (const point of ['read', 'persist:1', 'ids']) {
    const f = await fixture(), g = f.gate(point);
    const task = submit(f.client, f.native, f.storage, f.input);
    await g.reached;
    f.client.local.drafts[key] = 'newer'; mobileNewTaskDraftChanged(f.client, key); g.release();
    await expect(task).rejects.toMatchObject({ kind: 'superseded' });
    expect(actions(f)).not.toContain('enqueueTransfer'); expect(f.client.local.drafts[key]).toBe('newer');
    expect(busy(f.client, key)).toBe(false);
  }
});
test('facts changing during ID allocation refuse without rewriting draft', async () => {
  const f = await fixture(), g = f.gate('ids');
  const task = submit(f.client, f.native, f.storage, f.input);
  await g.reached; f.facts.defaultRuntimeMode = 'full-access'; g.release();
  await expect(task).rejects.toMatchObject({ kind: 'superseded' });
  expect(actions(f)).not.toContain('enqueueTransfer'); expect(f.client.local.drafts[key]).toBe(' original ');
});
test('after accepted enqueue selection departure and newer A preserve both drafts', async () => {
  const f = await fixture(), g = f.gate('enqueueTransfer');
  const task = submit(f.client, f.native, f.storage, f.input);
  await g.reached;
  f.setActive(false); f.client.threadId = 'thread-B'; f.client.local.drafts[key] = 'newer A';
  mobileNewTaskDraftChanged(f.client, key); g.release();
  const result = await task;
  expect(result.status).toBe('completed'); expect(result.claim?.messageId).toBe('new-message');
  expect(f.client.local.drafts[key]).toBe('newer A'); expect(f.client.local.drafts['new-task:B']).toBe(' original ');
  expect(f.client.threadId).toBe('thread-B');
});
test('lost enqueue reply leaves recovery required without cleanup or hidden retry', async () => {
  const f = await fixture();
  f.hooks.set('enqueueTransfer', async () => { throw Error('lost response'); });
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result.status).toBe('recovery-required');
  expect(actions(f).filter(x => x === 'enqueueTransfer')).toHaveLength(1);
  expect(actions(f)).not.toContain('completeTransfer'); expect(f.client.local.drafts[key]).toBe(' original ');
  expect(busy(f.client, key)).toBe(false);
});
test('LetGo enqueue stops all later native calls and releases the latch', async () => {
  const f = await fixture();
  f.hooks.set('enqueueTransfer', async () => { throw { name: 'FetchError', kind: 'Aborted' }; });
  await expect(submit(f.client, f.native, f.storage, f.input)).rejects.toMatchObject({ kind: 'superseded' });
  expect(actions(f).at(-1)).toBe('enqueueTransfer'); expect(f.client.local.drafts[key]).toBe(' original ');
  expect(busy(f.client, key)).toBe(false);
});
test('existing completed fingerprint recovers original IDs before allocating any', async () => {
  const f = await fixture(); f.seed('completed');
  const result = await submit(f.client, f.native, f.storage, f.input);
  expect(result.status).toBe('completed'); expect(result.claim?.messageId).toBe('old-message');
  expect(actions(f)).not.toContain('ids'); expect(actions(f)).not.toContain('enqueueTransfer');
  expect(f.client.local.drafts[key]).toBe(' original ');
});
test('prepared ownership never guesses commit and explicit resume uses original mutation', async () => {
  const f = await fixture(); f.seed('prepared');
  expect((await submit(f.client, f.native, f.storage, f.input)).status).toBe('recovery-required');
  expect(actions(f)).not.toContain('recover'); expect(actions(f)).not.toContain('ids');
  const result = await resume(f.client, f.native, f.storage, 'old-message', 'commit');
  expect(result.status).toBe('completed');
  expect(f.calls.find(c => c.action === 'recover')).toMatchObject({
    messageId: 'old-message', mutationId: 'epoch:1', decision: 'commit',
  });
  expect(actions(f)).not.toContain('ids');
});
test('failed retry is explicit and releases old identity before new allocation', async () => {
  const f = await fixture(); f.seed('failed');
  expect((await submit(f.client, f.native, f.storage, f.input)).status).toBe('failed');
  expect(actions(f)).not.toContain('ids');
  const result = await submit(f.client, f.native, f.storage, { ...f.input, retryFailed: true });
  expect(result.status).toBe('completed');
  expect(actions(f).indexOf('releaseFailedTransfer')).toBeLessThan(actions(f).indexOf('ids'));
  expect(result.claim?.messageId).toBe('new-message');
});
test('overlapping resume cannot clean while submit owns the same draft latch', async () => {
  const f = await fixture(); f.seed('queued');
  const g = f.gate('persist:1'), a = submit(f.client, f.native, f.storage, f.input);
  await g.reached;
  const b = await resume(f.client, f.native, f.storage, 'old-message');
  expect(b.status).toBe('busy'); expect(f.writes).toHaveLength(1); g.release();
  expect((await a).status).toBe('completed'); expect(busy(f.client, key)).toBe(false);
});
test('marker save failure remains cleanup pending and next resume never clears newer content twice', async () => {
  for (const failure of ['before', 'after']) {
    const f = await fixture(); f.seed('queued'); f.setMarkerFailure(failure);
    expect((await submit(f.client, f.native, f.storage, f.input)).status).toBe('cleanup-pending');
    expect(actions(f)).not.toContain('completeTransfer');
    f.client.local.drafts[key] = 'later text'; f.setMarkerFailure('');
    expect((await resume(f.client, f.native, f.storage, 'old-message')).status).toBe('completed');
    expect(f.client.local.drafts[key]).toBe('later text'); expect(actions(f)).not.toContain('ids');
  }
});
test('duplicate submit is busy before another native invocation', async () => {
  const f = await fixture(), g = f.gate('read'), a = submit(f.client, f.native, f.storage, f.input);
  await g.reached;
  const before = f.calls.length;
  expect((await submit(f.client, f.native, f.storage, f.input)).status).toBe('busy');
  expect(f.calls).toHaveLength(before); g.release();
  expect((await a).status).toBe('completed');
});

test('incomplete native inventory preserves the draft without allocating task IDs', async () => {
  const f = await fixture(); f.setComplete(false);
  expect((await submit(f.client, f.native, f.storage, f.input)).status).toBe('blocked');
  expect(actions(f)).not.toContain('ids'); expect(f.writes).toHaveLength(0);
  expect(f.client.local.drafts[key]).toBe(' original ');
});
test('failed initial preference write prevents enqueue and releases the draft latch', async () => {
  const f = await fixture(); f.hooks.set('persist:1', async () => { throw Error('disk unavailable'); });
  expect((await submit(f.client, f.native, f.storage, f.input)).status).toBe('blocked');
  expect(actions(f)).not.toContain('ids'); expect(actions(f)).not.toContain('enqueueTransfer');
  expect(f.client.local.drafts[key]).toBe(' original '); expect(busy(f.client, key)).toBe(false);
});
test('another draft has its own latch while A waits for native storage', async () => {
  const f = await fixture(), gate = f.gate('read');
  const first = submit(f.client, f.native, f.storage, f.input); await gate.reached;
  const secondKey = 'new-task:B';
  const second = await submit(f.client, f.native, f.storage, {
    ...f.input, draftKey: secondKey, facts: () => ({ ...f.facts, key: secondKey }),
  });
  expect(second.status).toBe('completed'); expect(busy(f.client, key)).toBe(true);
  expect(busy(f.client, secondKey)).toBe(false); expect(f.client.local.drafts[key]).toBe(' original ');
  // Stop A before allocating the deterministic IDs this sequencing fixture already used for B.
  f.setActive(false); gate.release(); await expect(first).rejects.toMatchObject({ kind: 'superseded' });
  expect(busy(f.client, key)).toBe(false);
});
test('LetGo while reading prevents lookup and all later native calls', async () => {
  const f = await fixture(); f.hooks.set('read', async () => { throw { name: 'FetchError', kind: 'Aborted' }; });
  await expect(submit(f.client, f.native, f.storage, f.input)).rejects.toMatchObject({ kind: 'superseded' });
  expect(actions(f)).toEqual(['read']); expect(f.writes).toHaveLength(0); expect(busy(f.client, key)).toBe(false);
});


test('new-admission facts prepare after exact recovery lookup and before persistence or IDs', async () => {
  const f = await fixture(); let prepared = false;
  noteNow(f.client, Date.parse('2026-10-07T01:00:00.000Z')); // The prior root sample is intentionally stale.
  const result = await submit(f.client, f.native, f.storage, { ...f.input,
    async prepareFacts(native) {
      expect(actions(f)).toEqual(['read', 'transferLookup', 'transferLookup']);
      expect(f.writes).toHaveLength(0);
      await native.later({ op: 'prepare-facts' }); prepared = true;
    }, facts() { expect(prepared).toBe(true); return f.facts; },
  });
  expect(result.status).toBe('completed');
  expect(actions(f).indexOf('prepare-facts')).toBeLessThan(actions(f).indexOf('ids'));
  const record = f.calls.find(call => call.action === 'enqueueTransfer').record;
  expect(record.createdAt).toBe('2026-10-08T01:00:00.000Z');
});
test('existing claims recover without resolving present-day facts or reading action time', async () => {
  for (const state of ['prepared', 'queued', 'completed', 'failed'] as const) {
    const f = await fixture(); f.seed(state);
    const result = await submit(f.client, f.native, f.storage, { ...f.input, now: NaN,
      async prepareFacts() { throw Error('must not prepare'); }, facts() { throw Error('must not read facts'); } });
    expect(result.claim?.messageId).toBe('old-message'); expect(actions(f)).not.toContain('ids');
    expect(result.message).not.toContain('must not');
  }
});
test('fact preparation checks ownership before native calls and after every native reply', async () => {
  for (const when of ['before', 'during']) {
    const f = await fixture();
    if (when === 'during') f.hooks.set('prepare-facts', async () => { f.setActive(false); });
    let returned = false;
    await expect(submit(f.client, f.native, f.storage, { ...f.input, async prepareFacts(native) {
      if (when === 'before') f.setActive(false);
      await native.later({ op: 'prepare-facts' }); returned = true;
    } })).rejects.toMatchObject({ kind: 'superseded' });
    expect(returned).toBe(false); expect(f.writes).toHaveLength(0); expect(actions(f)).not.toContain('ids');
    expect(actions(f).includes('prepare-facts')).toBe(when === 'during'); expect(busy(f.client, key)).toBe(false);
  }
});
test('swallowed cancellation in fact preparation cannot allocate or persist', async () => {
  const f = await fixture(); f.hooks.set('prepare-facts', async () => { throw { name: 'FetchError', kind: 'Aborted' }; });
  await expect(submit(f.client, f.native, f.storage, { ...f.input, async prepareFacts(native) {
    try { await native.later({ op: 'prepare-facts' }); } catch { /* Tolerated preference failure must not hide cancellation. */ }
  } })).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.writes).toHaveLength(0); expect(actions(f)).not.toContain('ids'); expect(busy(f.client, key)).toBe(false);
});
test('preparation failure keeps the draft and invalid action time never uses cached clock', async () => {
  for (const failedPrepare of [true, false]) {
    const f = await fixture(); const before = mobileNewTaskDraftPresentation(f.client, key);
    const result = await submit(f.client, f.native, f.storage, { ...f.input, now: failedPrepare ? f.input.now : NaN,
      async prepareFacts() { if (failedPrepare) throw Error('file read unavailable'); } });
    expect(result.status).toBe('blocked'); expect(result.message).toContain(failedPrepare ? 'file read unavailable' : 'app clock');
    expect(f.writes).toHaveLength(0); expect(actions(f)).not.toContain('ids');
    expect(mobileNewTaskDraftPresentation(f.client, key)).toEqual(before); expect(busy(f.client, key)).toBe(false);
  }
});


test('known source admission refusals precede preference writes and ID allocation', async () => {
  for (const reason of ['busy', 'permission', 'text', 'model', 'worktree', 'context']) {
    const f = await fixture();
    if (reason === 'busy') f.facts.blockReason = 'Wait for attachment import.';
    if (reason === 'permission') { f.facts.connected = true; f.facts.canOperate = false; }
    if (reason === 'text') f.client.local.drafts[key] = '  ';
    if (reason === 'model') f.facts.selectedModel = null;
    if (reason === 'worktree') { f.facts.workspace.mode = 'worktree'; f.facts.workspace.explicitBranch = null; }
    if (reason === 'context') mobileNewTaskDraftStore(f.client).records[key]!.context = { version: 1, records: [{ kind: 'unsupported' }] };
    f.hooks.set('persist:1', async () => { throw Error('must not mask admission with disk failure'); });
    const before = mobileNewTaskDraftPresentation(f.client, key), result = await submit(f.client, f.native, f.storage, f.input);
    expect(result.status).toBe('blocked'); expect(result.message).not.toContain('mask');
    expect(f.writes).toHaveLength(0); expect(actions(f)).not.toContain('ids');
    expect(mobileNewTaskDraftPresentation(f.client, key)).toEqual(before); expect(busy(f.client, key)).toBe(false);
  }
});
