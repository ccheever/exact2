import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskTransferGuardAcquire as acquire, mobileNewTaskTransferGuardRelease as release,
  mobileNewTaskTransferGuardRead as read, mobileNewTaskTransferGuardAssert as assert, mobileNewTaskTransferGuardBusy as busy } from './new-task-transfer-guard';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxTransferLookup } from './mobile-outbox';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftPresentation as presentation } from './mobile-new-task-drafts';
import { mobileHomeDraftAction } from './home-draft-actions';
import { mobileNewTaskTransferSubmit } from './new-task-transfer';
import { mobileBindNewTaskDraft } from './new-task-draft-binding';
import { mobileNewTaskFlowView, mobileNewTaskFlowAction } from './new-task-flow';
import { EnvironmentFleet } from './shared/settings-b-fleet';
import { noteNow } from './shared/composer-controls';
import type { MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
const A = 'new-task:A', B = 'new-task:B', origin = 'https://guard.test';
async function fixture() {
  const client = new MobileDraftClient(); let claims: MobileOutboxTransferClaim[] = [], complete = true;
  let hook = async (_request: Obj) => {}; const calls: Obj[] = []; let ids = 0;
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}').buffer; }, async atomicWriteFile() {} } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request); await hook(request);
    const value = request.op === 'mobileOutbox' ? request.action === 'read'
      ? { ownerEpoch: 'epoch', sequenceFloor: 1, complete, errors: [], records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers: claims }
      : { complete, fingerprint: null, claims: claims.filter(claim => claim.draftKey === request.draftKey) }
      : request.op === 'mobileAlert' ? { choice: 'discard' } : request.op === 'ids' ? Array.from({ length: Number(request.count) }, () => `id-${++ids}`)
      : request.op === 'status' ? { phase: 'disconnected' } : {};
    return { ok: true, generation: client.generation, value };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { origin, environmentId: 'env', projectId: 'project', threadId: '', generation: 1, connection: 'connected',
    shellLoaded: true, configLive: true, shellLive: true, threadLive: true });
  client.shell.projects = [{ id: 'project', title: 'P', workspaceRoot: '/p' }, { id: 'other', title: 'Other', workspaceRoot: '/other' }];
  client.config = { environment: { capabilities: {} }, providers: [] }; noteNow(client, 1791420000000);
  for (const id of ['A', 'B']) { create(client, { id, origin, environmentId: 'env', projectId: 'project', createdAt: '2026-10-08T00:00:00.000Z' }); client.local.drafts[`new-task:${id}`] = 'text'; }
  const claim = (state: MobileOutboxTransferClaim['state'], key = A): MobileOutboxTransferClaim => ({
    transferId: 'message', draftKey: key, fingerprint: 'a'.repeat(64), messageId: 'message', threadId: 'thread', commandId: 'command', mutationId: 'epoch:1', state,
    capture: ['completed', 'released'].includes(state) ? null : { version: 1, draft: presentation(client, key)! },
    record: ['completed', 'released'].includes(state) ? null : { schemaVersion: 1, origin, environmentId: 'env', threadId: 'thread', messageId: 'message', commandId: 'command',
      createdAt: '2026-10-08T01:00:00.000Z', text: 'text', attachments: [], creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } } });
  calls.length = 0;
  return { client, native, storage, calls, claim, claims(value: MobileOutboxTransferClaim[]) { claims = value; }, complete(value: boolean) { complete = value; }, hook(value: typeof hook) { hook = value; } };
}
test('lease excludes same-key transfer preparation, preserves B and stale release cannot unlock replacement', async () => {
  const f = await fixture(), a = acquire(f.client, A)!, b = acquire(f.client, B)!;
  expect(acquire(f.client, A)).toBeNull(); expect(busy(f.client, B)).toBe(true);
  expect(acquire(f.client, '')).toBeNull(); expect(acquire(f.client, 'env:thread')).toBeNull(); expect(f.calls).toEqual([]);
  release(f.client, a); const next = acquire(f.client, A)!; release(f.client, a); expect(busy(f.client, A)).toBe(true);
  release(f.client, next); release(f.client, b); expect(busy(f.client, B)).toBe(false);
});
test('cold destructive check reads inventory before full-key lookup; terminal claims permit mutation', async () => {
  for (const state of ['completed', 'released'] as const) {
    const f = await fixture(); f.claims([f.claim(state)]); const lease = acquire(f.client, A)!;
    await read(f.client, f.native, lease); expect(() => assert(f.client, lease)).not.toThrow();
    expect(f.calls.map(call => [call.action, call.draftKey])).toEqual([['read', undefined], ['transferLookup', A]]); release(f.client, lease);
  }
});
test('prepared queued and failed claims block only their captured draft with explicit recovery', async () => {
  for (const state of ['prepared', 'queued', 'failed'] as const) {
    const f = await fixture(); f.claims([f.claim(state)]); const a = acquire(f.client, A)!, b = acquire(f.client, B)!;
    await expect(read(f.client, f.native, a)).rejects.toThrow('Resolve'); await read(f.client, f.native, b); expect(() => assert(f.client, b)).not.toThrow();
    release(f.client, a); release(f.client, b);
  }
});
test('partial inventory never proves absence and stale route or endpoint interrupts before lookup', async () => {
  for (const mode of ['partial', 'route', 'endpoint']) {
    const f = await fixture(), lease = acquire(f.client, A)!; let current = true;
    f.hook(async request => { if (request.action === 'read') { if (mode === 'partial') f.complete(false); if (mode === 'route') current = false; if (mode === 'endpoint') f.client.generation++; } });
    await expect(read(f.client, f.native, lease, () => current)).rejects.toThrow(); expect(f.calls.some(call => call.action === 'transferLookup')).toBe(false); release(f.client, lease);
  }
});
test('final synchronous assertion sees a claim or partial inventory adopted during confirmation', async () => {
  for (const mode of ['claim', 'partial']) {
    const f = await fixture(), lease = acquire(f.client, A)!; await read(f.client, f.native, lease);
    if (mode === 'claim') { f.claims([f.claim('queued')]); await mobileOutboxTransferLookup(f.client, f.native, A); }
    else { f.complete(false); await mobileOutboxRead(f.client, f.native); }
    expect(() => assert(f.client, lease)).toThrow(); release(f.client, lease);
  }
});
test('Home discard blocks before alert for active claim and final claim admission preserves content', async () => {
  for (const mode of ['before', 'during']) {
    const f = await fixture(); if (mode === 'before') f.claims([f.claim('prepared')]);
    f.hook(async request => { if (request.op === 'mobileAlert') {
      expect(acquire(f.client, A)).toBeNull(); f.claims([f.claim('queued')]); await mobileOutboxTransferLookup(f.client, f.native, A);
    } });
    const result = await mobileHomeDraftAction('home', 'env', '', 'draft-discard', A, () => true, f.client, f.native, f.storage);
    expect(result.message).toContain('Resolve'); expect(presentation(f.client, A)?.text).toBe('text'); expect(busy(f.client, A)).toBe(false);
    expect(f.calls.filter(call => call.op === 'mobileAlert')).toHaveLength(mode === 'before' ? 0 : 1);
  }
});
test('direct binding retarget cannot bypass lookup and releases its lease after refusal', async () => {
  const f = await fixture(); f.claims([f.claim('failed')]); f.client.projectId = 'other';
  await expect(mobileBindNewTaskDraft(f.client, 'flow', A, '', f.native, f.storage, () => true)).rejects.toThrow('Resolve');
  expect(presentation(f.client, A)?.projectId).toBe('project'); expect(busy(f.client, A)).toBe(false);
});
test('flow refuses captured retarget before project selection or scratch RPC', async () => {
  const f = await fixture(), fleet = new EnvironmentFleet();
  let view = mobileNewTaskFlowView('flow', 'visit', `/new/draft?draftId=${encodeURIComponent(A)}`, true, true, f.client, fleet);
  const ready = await mobileNewTaskFlowAction(view.owner, 'visit', 'prepare', '', '', f.native, f.storage, f.client, fleet); expect(ready.message).toBe('');
  view = mobileNewTaskFlowView('flow', 'visit', '/new/draft', true, true, f.client, fleet); expect(view.ready).toBe(true);
  f.claims([f.claim('queued')]); f.calls.length = 0;
  const result = await mobileNewTaskFlowAction(view.owner, 'visit', 'project', '["env","other"]', '', f.native, f.storage, f.client, fleet);
  expect(result.message).toContain('Resolve'); expect(f.client.projectId).toBe('project'); expect(presentation(f.client, A)?.projectId).toBe('project');
  expect(f.calls.every(call => call.op === 'mobileOutbox')).toBe(true); expect(busy(f.client, A)).toBe(false);
});


test('Home inventory cancellation preserves the last complete projection without an alert or write', async () => {
  const f = await fixture();
  await mobileOutboxRead(f.client, f.native);
  const before = mobileOutboxSnapshot(f.client);
  let writes = 0;
  f.storage.fs.atomicWriteFile = async () => { writes++; };
  f.calls.length = 0;
  f.hook(async request => {
    if (request.action === 'read') throw { name: 'FetchError', kind: 'Aborted' };
  });
  await expect(mobileHomeDraftAction('home', 'env', '', 'draft-discard', A, () => true,
    f.client, f.native, f.storage)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.map(call => call.action ?? call.op)).toEqual(['read']);
  expect(mobileOutboxSnapshot(f.client)).toEqual(before);
  expect(presentation(f.client, A)?.text).toBe('text');
  expect(writes).toBe(0);
  expect(busy(f.client, A)).toBe(false);
});

test('actual Submit cannot begin while a Home discard confirmation owns the draft', async () => {
  const f = await fixture();
  let entered!: () => void, finish!: () => void;
  const started = new Promise<void>(resolve => { entered = resolve; });
  f.hook(async request => {
    if (request.op === 'mobileAlert') {
      entered();
      await new Promise<void>(resolve => { finish = resolve; });
    }
  });
  const removing = mobileHomeDraftAction('home', 'env', '', 'draft-discard', A, () => true,
    f.client, f.native, f.storage);
  await started;
  const result = await mobileNewTaskTransferSubmit(f.client, f.native, f.storage, {
    draftKey: A, now: 1791420000000, current: () => true, facts: () => { throw Error('busy caller must not capture'); },
  });
  expect(result.status).toBe('busy');
  expect(f.calls.some(call => call.op === 'ids')).toBe(false);
  finish();
  expect((await removing).message).toBe('');
  expect(presentation(f.client, A)).toBeNull();
  expect(busy(f.client, A)).toBe(false);
});

async function resumedFlow(f: Awaited<ReturnType<typeof fixture>>, session: string) {
  const fleet = new EnvironmentFleet();
  const view = mobileNewTaskFlowView(session, 'visit', `/new/draft?draftId=${encodeURIComponent(A)}`,
    true, true, f.client, fleet);
  const prepared = await mobileNewTaskFlowAction(view.owner, 'visit', 'prepare', '', '',
    f.native, f.storage, f.client, fleet);
  expect(prepared.message).toBe('');
  return { fleet, view };
}

test('matching saved-draft child resumes through an active claim while explicit branch change remains guarded', async () => {
  for (const branch of ['', '&branch=feature']) {
    const f = await fixture(), { fleet } = await resumedFlow(f, 'resume');
    f.claims([f.claim('queued')]); f.calls.length = 0;
    const location = `/new/draft/files/a.ts?draftId=${encodeURIComponent(A)}${branch}`;
    const child = mobileNewTaskFlowView('resume', 'child', location, true, true, f.client, fleet);
    const result = await mobileNewTaskFlowAction(child.owner, 'child', 'prepare', '', '',
      f.native, f.storage, f.client, fleet);
    if (branch) {
      expect(result.message).toContain('Resolve');
      expect(f.calls.some(call => call.method === 'vcs.switchRef')).toBe(false);
    } else {
      expect(result.message).toBe('');
      expect(f.calls.some(call => call.op === 'mobileOutbox')).toBe(false);
      expect(mobileNewTaskFlowView('resume', 'child', location, true, true, f.client, fleet).ready).toBe(true);
    }
    expect(f.client.draftKey).toBe(A);
    expect(presentation(f.client, A)?.projectId).toBe('project');
  }
});

test('flow inventory cancellation releases ownership without retaining a UI failure or retargeting', async () => {
  const f = await fixture(), { fleet, view } = await resumedFlow(f, 'cancel');
  f.calls.length = 0;
  f.hook(async request => {
    if (request.action === 'read') throw { name: 'FetchError', kind: 'Aborted' };
  });
  await expect(mobileNewTaskFlowAction(view.owner, 'visit', 'project', '["env","other"]', '',
    f.native, f.storage, f.client, fleet)).rejects.toMatchObject({ kind: 'superseded' });
  expect(f.calls.map(call => call.action ?? call.op)).toEqual(['read']);
  expect(f.client.projectId).toBe('project');
  expect(presentation(f.client, A)?.projectId).toBe('project');
  expect(mobileNewTaskFlowView('cancel', 'visit', '/new/draft', true, true, f.client, fleet).message).toBe('');
  expect(busy(f.client, A)).toBe(false);
});

test('failed project selection save keeps old metadata and blocks foreground Send', async () => {
  const f = await fixture(), { fleet, view } = await resumedFlow(f, 'failed-selection');
  f.storage.fs.atomicWriteFile = async () => { throw Error('disk full'); };
  const result = await mobileNewTaskFlowAction(view.owner, 'visit', 'project', '["env","other"]', '',
    f.native, f.storage, f.client, fleet);
  expect(result.message).toContain('disk full');
  expect(presentation(f.client, A)?.projectId).toBe('project');
  expect(presentation(f.client, A)?.text).toBe('text');
  expect(mobileNewTaskFlowView('failed-selection', 'visit', '/new/draft', true, true, f.client, fleet).ready).toBe(false);
  expect(busy(f.client, A)).toBe(false);
});

test('failed binding save retains optimistic draft content for an explicit retry without new IDs', async () => {
  const f = await fixture(); f.client.projectId = 'other';
  let failed = true, saved = '';
  f.storage.fs.atomicWriteFile = async (_path, bytes) => {
    if (failed) throw Error('disk full');
    saved = new TextDecoder().decode(bytes);
  };
  await expect(mobileBindNewTaskDraft(f.client, 'flow', A, '', f.native, f.storage, () => true))
    .rejects.toThrow('disk full');
  expect(presentation(f.client, A)?.projectId).toBe('other');
  expect(presentation(f.client, A)?.text).toBe('text');
  expect(busy(f.client, A)).toBe(false);
  failed = false;
  expect(await mobileBindNewTaskDraft(f.client, 'flow', A, '', f.native, f.storage, () => true)).toBe(A);
  expect(JSON.parse(saved).mobileNewTaskDrafts.records[A].projectId).toBe('other');
  expect(f.calls.some(call => call.op === 'ids')).toBe(false);
});
