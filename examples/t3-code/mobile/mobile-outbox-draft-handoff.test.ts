import { expect, test } from 'bun:test';
import { mobileOutboxDraftHandoffProjection as projection, mobileOutboxDraftHandoffDecode as decode,
  mobileOutboxDraftHandoffComplete as complete, mobileOutboxDraftHandoffStatus as status,
  mobileOutboxDraftHandoffsHydrate as hydrate, mobileOutboxDraftHandoffsPersisted as persisted,
  type MobileOutboxDraftHandoff } from './mobile-outbox-draft-handoff';
import type { MobileOutboxRecord } from './mobile-outbox-model';
import type { MobileOutboxDeliveryReceipt } from './mobile-outbox-delivery';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const asset = '11111111-1111-4111-a111-111111111111';
function fixture(rejected = true) {
  const record: MobileOutboxRecord = { schemaVersion: 1, origin: 'https://handoff.test', environmentId: 'env',
    threadId: 'thread', messageId: 'message', commandId: 'command', text: 'saved edits', attachments: [],
    createdAt: '2026-10-08T00:00:00.000Z', creation: { projectId: 'project', workspaceMode: 'local', branch: null, worktreePath: null } };
  const receipt: MobileOutboxDeliveryReceipt = { kind: 'outbox', operationId: 'command', revision: 3,
    origin: record.origin, environmentId: 'env', threadId: 'thread', messageId: 'message', rowToken: 'epoch:1', rowRevision: 1,
    record: clone(record), stage: 'start-turn', method: 'orchestration.launchThread', payload: { commandId: 'command', threadId: 'thread', initialMessage: { messageId: 'message' } },
    attachmentIDs: [], state: 'rejected', attemptRevision: 2, attemptPreviousState: 'reserved', error: { kind: 'EnvironmentAuthorizationError' } };
  if (!rejected) {
    receipt.state = 'acknowledged'; receipt.revision = 5; receipt.result = { sequence: 42 }; delete receipt.error;
    receipt.cleanup = { ackRevision: 3, intentRevision: 4, phase: 'edited', mutationId: 'epoch:3', ownerEpoch: 'epoch', outcome: {
      mutationId: 'epoch:3', messageId: 'message', status: 'stale', revision: 2, record: null, removed: null, message: '',
      ownerEpoch: 'epoch', sequenceFloor: 3, current: { record: clone(record), revision: 2, token: 'epoch:2', pending: false } } };
  }
  const key = rejected ? 'new-task:restored-message' : 'env:thread';
  const document: Obj = { version: 1, drafts: { [key]: record.text, unrelated: 'keep' }, snapshotDrafts: {}, composerFiles: [],
    composerControls: { staged: {}, contexts: {} }, mobileAttachmentOrder: {}, mobileRecoveredDrafts: {},
    mobileNewTaskDrafts: { version: 1, records: rejected ? { [key]: { key, origin: record.origin, environmentId: 'env', projectId: 'project',
      createdAt: record.createdAt, revision: 0, choices: null } } : {}, claims: {}, receipts: {}, fileReleases: [] } };
  const handoff: MobileOutboxDraftHandoff = { version: 1, operationId: 'command', receiptRevision: receipt.revision,
    record, request: { ownerEpoch: 'epoch', mutationId: 'epoch:4', messageId: 'message', operation: 'remove',
      expectedToken: 'epoch:2', expectedRevision: 2, requireUnheld: false }, draftKey: key, draft: projection(document, key) };
  document.mobileOutboxDraftHandoffs = { command: handoff as unknown as Obj };
  return { record, receipt, key, document, handoff };
}
test('handoff captures only the exact destination and detaches every nested value', () => {
  for (const rejected of [true, false]) {
    const f = fixture(rejected), decoded = decode(f.handoff, f.receipt);
    expect(decoded).toEqual(f.handoff); expect(decoded.draft.text).toBe('saved edits');
    decoded.record.creation!.projectId = 'mutated'; decoded.draft.text = 'mutated';
    expect(f.record.creation!.projectId).toBe('project'); expect(obj(f.document.drafts)[f.key]).toBe('saved edits');
    expect(JSON.stringify(decoded.draft)).not.toContain('unrelated');
  }
});
test('wrong destinations, owner changes, incomplete removal identities and stale receipt revisions are refused', () => {
  const f = fixture();
  for (const patch of [{ draftKey: 'env:thread' }, { receiptRevision: 4 }, { operationId: 'other' },
    { record: { ...f.record, origin: 'https://other.test' } }, { request: { ...f.handoff.request, mutationId: 'epoch:04' } },
    { request: { ...f.handoff.request, record: f.record } }, { request: { ...f.handoff.request, requireUnheld: true } },
    { request: { ...f.handoff.request, expectedRevision: 0 } }, { draft: { ...f.handoff.draft, metadata: null } }])
    expect(() => decode({ ...f.handoff, ...patch }, f.receipt)).toThrow('invalid');
});
test('only final rejection or edited creation ACK can end the old owner', () => {
  const f = fixture();
  for (const state of ['reserved', 'issued', 'uncertain', 'retired'] as const)
    expect(() => decode(f.handoff, { ...f.receipt, state })).toThrow();
  const accepted = fixture(false);
  const withoutCleanup = { ...accepted.receipt, revision: 3 }; delete withoutCleanup.cleanup;
  expect(() => decode({ ...accepted.handoff, receiptRevision: 3 }, withoutCleanup)).toThrow('invalid');
});
test('all local assets must be in the target and malformed JSON cannot lose evidence', () => {
  const f = fixture();
  f.record.attachments = [{ id: asset, kind: 'image', name: 'a.png', mimeType: 'image/png', sizeBytes: 1, uploadId: '', status: 'staged' }];
  expect(() => decode(f.handoff, f.receipt)).toThrow('invalid');
  f.handoff.draft.images = [{ id: asset, name: 'existing name' }]; expect(decode(f.handoff, f.receipt).record.attachments).toHaveLength(1);
  for (const bad of [NaN, undefined, Infinity]) {
    const malformed = { ...f.handoff, draft: { ...f.handoff.draft, recovered: { existing: { bad } } } };
    expect(() => decode(malformed, f.receipt)).toThrow('invalid');
  }
});
test('actual client persistence retains handoff metadata and invalid markers without silent deletion', async () => {
  for (const marker of [fixture().document.mobileOutboxDraftHandoffs, { malformed: { version: 99 } }, null]) {
    const client = new MobileDraftClient();
    let disk = '{"version":1}';
    const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; },
      async atomicWriteFile(_path, bytes) { disk = new TextDecoder().decode(bytes); } } };
    const native: Native = { available: true, watch() {}, async later() {
      return { ok: true, generation: 0, value: { phase: 'disconnected' } };
    } };
    const initial = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(initial.native, initial.storage);
    hydrate(client, { mobileOutboxDraftHandoffs: marker }); await client.persist(storage);
    expect(obj(JSON.parse(disk)).mobileOutboxDraftHandoffs).toEqual(marker);
    const cold = new MobileDraftClient();
    const handles = mobileDraftRecoveryHandles(cold, native, storage); await cold.refresh(handles.native, handles.storage);
    expect(persisted(cold)).toEqual(marker);
  }
});
test('native completion submits a detached proof and refuses changed or non-durable acknowledgments', async () => {
  const f = fixture(), calls: Obj[] = []; let answer: unknown = { handoff: f.handoff, durable: true };
  const native: Native = { available: true, watch() {}, async later(request) { calls.push(obj(request)); return { ok: true, generation: 0, value: clone(answer) }; } };
  expect((await complete(native, f.receipt, f.handoff)).durable).toBe(true);
  expect(calls[0]).toMatchObject({ op: 'mobileOutboxDelivery', action: 'draftHandoffComplete', operationId: 'command', handoff: f.handoff });
  f.handoff.draft.text = 'caller mutation'; expect(obj(obj(calls[0].handoff).draft).text).toBe('saved edits');
  answer = { handoff: f.handoff, durable: false }; await expect(complete(native, f.receipt, f.handoff)).rejects.toThrow('invalid');
  answer = { handoff: null, durable: false }; expect(await status(native, f.receipt)).toEqual(answer);
  answer = { handoff: null, durable: true }; await expect(status(native, f.receipt)).rejects.toThrow('invalid');
  const failing: Native = { ...native, async later() { return { ok: false, generation: 0, error: { kind: 'Persistence', message: 'disk failed', uncertain: true } }; } };
  await expect(complete(failing, f.receipt, f.handoff)).rejects.toThrow('disk failed');
});
