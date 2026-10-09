import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftBind } from './mobile-new-task-drafts';
import { mobileIncomingShareAdopt } from './incoming-share-adoption';
import { mobileIncomingShareCancel } from './incoming-share-cancellation';
import { mobileOutboxDraftHandoffProjection } from './mobile-outbox-draft-handoff';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { incomingShareEntry, incomingShareSelection, incomingShareMergedText, type IncomingShareEntry } from './incoming-share-model';
import { mobileIncomingShareImports } from './incoming-share-imports';
import { mobileDraftAttachmentIds } from './draft-attachment-order';
import { mobileNewTaskTransferGuardBusy } from './new-task-transfer-guard';
import { draftFiles } from './shared/composer-editor-files';
import { obj, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
const key = 'new-task:share', origin = 'https://share.test';
const uuid = (n: number) => `00000000-0000-4000-8000-${n.toString().padStart(12, '0')}`;
const entry = (): IncomingShareEntry => ({ schemaVersion: 1, id: `share-${'a'.repeat(64)}`, instanceId: uuid(90), createdAt: '2026-10-08T10:00:00Z', text: 'Shared task', warnings: [],
  attachments: [ { id: uuid(1), kind: 'file', name: 'first.txt', mimeType: 'text/plain', sizeBytes: 12 },
    { id: uuid(2), kind: 'image', name: 'second.jpg', mimeType: 'image/jpeg', sizeBytes: 22 },
    { id: uuid(3), kind: 'file', name: 'third.mp4', mimeType: 'video/mp4', sizeBytes: 32 } ] });
async function fixture(saved: Obj = { version: 1 }, journal: { baseline?: Obj; adopted?: Obj; restored?: Obj } = {}) {
  const client = new MobileDraftClient(), calls: Obj[] = [], writes: Obj[] = []; let disk = JSON.stringify(saved);
  let writeFailure = '', loseConsumeReply = false, consumeFailure = false, loseCancelReply = false, current = true, hook = async (_call: Obj) => {};
  let content = entry(), consumed = false, selected: string[] = [], owner: unknown;
  const adoptionId = uuid(91);
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; }, async atomicWriteFile(_path, bytes) {
    const document = JSON.parse(new TextDecoder().decode(bytes)), imported = !!obj(obj(document.mobileIncomingShareImports)[key])[adoptionId];
    if (writeFailure === 'baseline' || imported && writeFailure === 'before') throw new Error('write failed');
    if (imported) journal.adopted ??= mobileOutboxDraftHandoffProjection(document, key);
    disk = JSON.stringify(document); writes.push(document);
    if (imported && writeFailure === 'after') throw new Error('lost save reply');
  } } };
  const native: Native = { available: true, watch() {}, async later(raw) {
    const call = obj(raw); calls.push(call); await hook(call); let value: unknown = {};
    if (call.op === 'status') value = { phase: 'disconnected' };
    if (call.op === 'mobileOutbox') value = call.action === 'read'
      ? { ownerEpoch: 'epoch', sequenceFloor: 1, complete: true, errors: [], records: [], outcomes: [], mutations: [], revisions: {}, tokens: {}, transfers: [] }
      : { complete: true, fingerprint: null, claims: [] };
    if (call.op === 'mobileIncomingShares') {
      if (call.action === 'reserve') {
        if (owner && JSON.stringify(owner) !== JSON.stringify(call.destination)) return { ok: false, generation: 0, error: { kind: 'Share', message: 'Already reserved' } };
        owner = call.destination; journal.baseline ??= mobileOutboxDraftHandoffProjection(JSON.parse(disk), key);
      }
      if (call.action === 'stage') selected = call.attachmentIds as string[];
      if (call.action === 'consume') {
        if (consumeFailure) throw new Error('consume unavailable');
        const receipt = obj(obj(obj(JSON.parse(disk)).mobileIncomingShareImports)[key])[adoptionId];
        if (!receipt) return { ok: false, generation: 0, error: { kind: 'Share', message: 'Save the draft first' } };
        consumed = true;
        if (loseConsumeReply) { loseConsumeReply = false; throw new Error('lost consume reply'); }
      }
      if (call.action === 'cancel') {
        const document = JSON.parse(disk), current = mobileOutboxDraftHandoffProjection(document, key);
        const equal = (a: unknown, b: unknown) => a !== undefined && canonical(a) === canonical(b);
        if (consumed || ![journal.baseline, journal.adopted, journal.restored].some(value => equal(value, call.expectedDraft))
          || !(journal.restored ? equal(journal.restored, current) : [journal.baseline, journal.adopted].some(value => equal(value, current))))
          return { ok: false, generation: 0, error: { kind: 'Share', message: 'The saved draft changed or was consumed' } };
        if (!journal.restored) {
          const restored = JSON.parse(JSON.stringify(journal.baseline));
          restored.metadata.revision = Math.max(Number(obj(current.metadata).revision), Number(restored.metadata.revision)) + 1;
          document.drafts = { ...document.drafts, [key]: restored.text };
          document.snapshotDrafts = { ...document.snapshotDrafts, [key]: restored.images };
          document.composerFiles = [...(document.composerFiles ?? []).filter((file: Obj) => file.draftKey !== key), ...restored.files];
          document.mobileNewTaskDrafts.records[key] = restored.metadata;
          document.composerControls.staged = { ...document.composerControls.staged, [key]: restored.staged };
          document.composerControls.contexts = { ...document.composerControls.contexts, [key]: restored.workspace };
          document.mobileAttachmentOrder = { ...document.mobileAttachmentOrder, [key]: restored.order };
          document.mobileRecoveredDrafts = { ...Object.fromEntries(Object.entries(obj(document.mobileRecoveredDrafts)).filter(([, value]) => obj(value).key !== key)), ...restored.recovered };
          delete obj(obj(document.mobileIncomingShareImports)[key])[adoptionId];
          journal.restored = restored; disk = JSON.stringify(document);
        }
        if (loseCancelReply) { loseCancelReply = false; throw new Error('lost cancel reply'); }
      }
      value = { adoptionId, consumed, entry: consumed ? null : content, attachments: selected.map(id => content.attachments.find(file => file.id === id)!),
        cancelled: call.action === 'cancel', restoredDraft: call.action === 'cancel' ? journal.restored : null };
    }
    return { ok: true, generation: client.generation, value };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { environmentId: 'env', projectId: 'project', origin, generation: 1, threadId: '', configLive: true,
    config: { environment: { capabilities: { attachmentUploads: true, fileAttachments: { maxUploadBytes: 50 * 1024 * 1024 } } } } });
  if (!obj(obj(saved.mobileNewTaskDrafts).records)[key]) mobileNewTaskDraftCreate(client, { id: 'share', environmentId: 'env', projectId: 'project', origin, createdAt: '2026-10-08T09:00:00Z' });
  expect(mobileNewTaskDraftBind(client, key, 'flow')).toBe(true); calls.length = 0; writes.length = 0;
  return { client, calls, writes, native, storage, adoptionId, journal, disk: () => JSON.parse(disk) as Obj,
    cancel: () => mobileIncomingShareCancel(client, native, storage, { entry: content, adoptionId, current: () => current }),
    adopt: () => mobileIncomingShareAdopt(client, native, storage, { entry: content, current: () => current }),
    setEntry(value: IncomingShareEntry) { content = value; }, setHook(value: typeof hook) { hook = value; },
    failConsume(value = true) { consumeFailure = value; }, loseCancel() { loseCancelReply = true; },
    failWrite(value: string) { writeFailure = value; }, depart() { current = false; }, loseConsume() { loseConsumeReply = true; } };
}
test('adoption persists shared text, original metadata, mixed order and receipt before native consumption', async () => {
  const f = await fixture(); f.client.local.drafts[key] = 'Existing';
  const result = await f.adopt(); expect(result.status).toBe('imported');
  expect(f.client.local.drafts[key]).toStartWith('Existing\n\nShared task');
  expect(f.client.local.snapshotDrafts[key]?.[0]).toEqual({ id: uuid(2), name: 'second.jpg', mimeType: 'image/jpeg', sizeBytes: 22 });
  expect(draftFiles(f.client.local).map(file => file.id)).toEqual([uuid(1), uuid(3)]);
  expect(mobileDraftAttachmentIds(f.client, key)).toEqual([uuid(1), uuid(2), uuid(3)]);
  expect(obj(obj(f.disk().mobileIncomingShareImports)[key])[f.adoptionId]).toMatchObject({ shareId: entry().id, instanceId: entry().instanceId, attachmentIds: [uuid(1), uuid(2), uuid(3)] });
  expect(f.calls.filter(call => call.op === 'mobileIncomingShares').map(call => call.action)).toEqual(['reserve', 'stage', 'consume']);
  expect(f.calls.some(call => String(call.method).startsWith('orchestration.'))).toBe(false);
  expect(mobileNewTaskTransferGuardBusy(f.client, key)).toBe(false);
  const cold = await fixture(f.disk()); expect(mobileIncomingShareImports(cold.client)[key]).toEqual(mobileIncomingShareImports(f.client)[key]);
});
test('unknown generic-file support waits without reserving; unsupported files warn while images and text import', async () => {
  const f = await fixture(); f.client.configLive = false;
  expect((await f.adopt()).status).toBe('pending'); expect(f.calls).toEqual([]);
  f.client.configLive = true; f.client.config = { environment: { capabilities: { attachmentUploads: false } } };
  const result = await f.adopt(); expect(result.status).toBe('imported'); expect(result.warnings).toHaveLength(2);
  expect(draftFiles(f.client.local)).toEqual([]); expect(f.client.snapshotDrafts).toHaveLength(1);
});
test('image-only import does not need a generic file capability', async () => {
  const f = await fixture(); f.client.configLive = false; f.setEntry({ ...entry(), attachments: [entry().attachments[1]!] });
  expect((await f.adopt()).status).toBe('imported');
});
test('failed writes and lost replies retry without duplicating content or staging byte IDs', async () => {
  for (const failure of ['before', 'after', 'consume']) {
    const f = await fixture(); if (failure === 'consume') f.loseConsume(); else f.failWrite(failure);
    expect((await f.adopt()).status).toBe('retained'); const text = f.client.draft;
    f.failWrite(''); expect((await f.adopt()).status).toBe('imported');
    expect(f.client.draft).toBe(text); expect(f.client.snapshotDrafts).toHaveLength(1); expect(draftFiles(f.client.local)).toHaveLength(2);
    expect(f.calls.filter(call => call.action === 'stage')).toHaveLength(1);
    expect(f.calls.filter(call => call.action === 'reserve')).toHaveLength(1);
  }
});
test('route, draft, endpoint and input changes after a native await preserve both owners', async () => {
  for (const mode of ['route', 'project', 'generation', 'text']) {
    const f = await fixture(); f.client.local.drafts[key] = 'Original';
    f.setHook(async call => { if (call.action !== 'stage') return;
      if (mode === 'route') f.depart(); if (mode === 'project') f.client.projectId = 'other';
      if (mode === 'generation') f.client.generation++; if (mode === 'text') f.client.local.drafts[key] = 'New typing';
    });
    await expect(f.adopt()).rejects.toThrow('draft changed'); expect(f.client.local.drafts[key]).toBe(mode === 'text' ? 'New typing' : 'Original');
    expect(f.client.local.snapshotDrafts[key] ?? []).toEqual([]); expect(draftFiles(f.client.local)).toEqual([]);
    expect(f.calls.some(call => call.action === 'consume')).toBe(false); expect(f.writes).toHaveLength(1);
  }
});
test('server policy changing while staging cannot silently adopt newly disallowed files', async () => {
  const f = await fixture(); f.setHook(async call => { if (call.action === 'stage') f.client.config = { environment: { capabilities: { attachmentUploads: false } } }; });
  expect((await f.adopt()).message).toContain('support changed'); expect(f.client.draft).toBe(''); expect(f.writes).toHaveLength(1);
});
test('attachment capacity retains existing order and imports text with a bounded warning', async () => {
  const f = await fixture(); f.client.local.snapshotDrafts[key] = Array.from({ length: 100 }, (_, i) => ({ id: uuid(100 + i), name: 'old.png', mimeType: 'image/png', sizeBytes: 1 }));
  const result = await f.adopt(); expect(result.status).toBe('imported'); expect(result.warnings).toEqual(['3 shared files were skipped because this draft reached the attachment limit.']);
  expect(f.client.snapshotDrafts).toHaveLength(100); expect(f.client.draft).toBe('Shared task');
  expect(f.calls.find(call => call.action === 'stage')?.attachmentIds).toEqual([]);
});
test('malformed receipt ownership blocks importing instead of being pruned', async () => {
  const f = await fixture(); Object.assign(f.client.local, { mobileIncomingShareImports: { [key]: { broken: {} } } });
  expect((await f.adopt()).status).toBe('retained'); expect(f.calls).toEqual([]); expect(f.writes).toEqual([]);
});
test('native metadata decoder and server selection reject malformed or excessive references', () => {
  for (const value of [{ ...entry(), instanceId: 'file:///tmp/a' }, { ...entry(), attachments: [entry().attachments[0], entry().attachments[0]] },
    { ...entry(), attachments: [{ ...entry().attachments[0], sizeBytes: -1 }] }, { ...entry(), attachments: [{ ...entry().attachments[1], mimeType: 'image/heic' }] }])
    expect(() => incomingShareEntry(value)).toThrow();
  const selected = incomingShareSelection(entry(), { environment: { capabilities: { attachmentUploads: true, fileAttachments: { maxUploadBytes: 20 } } } }, []);
  expect(selected.status).toBe('ready'); if (selected.status === 'ready') expect(selected.attachments.map(file => file.id)).toEqual([uuid(1), uuid(2)]);
  expect(incomingShareMergedText('a\n\nb', 'b')).toBe('a\n\nb'); expect(incomingShareMergedText('a', 'b')).toBe('a\n\nb');
});

test('reserve sees a durable baseline including a selected empty draft and failed baseline save never reserves', async () => {
  const f = await fixture();
  f.setHook(async call => { if (call.action === 'reserve') {
    expect(f.writes).toHaveLength(1);
    expect(obj(obj(f.disk().mobileNewTaskDrafts).records)[key]).toMatchObject({ key, revision: 0 });
    expect(obj(obj(f.disk().mobileIncomingShareImports)[key])).toEqual({});
  } });
  expect((await f.adopt()).status).toBe('imported');
  const failed = await fixture(); failed.failWrite('baseline');
  expect((await failed.adopt()).status).toBe('retained');
  expect(failed.calls.some(call => call.action === 'reserve')).toBe(false);
});
test('cancel restores baseline after uncertain import persistence, without writing stale preferences', async () => {
  for (const failure of ['before', 'after', 'consume']) {
    const f = await fixture(); f.client.local.drafts[key] = 'Before';
    f.client.local.drafts.unrelated = 'Keep me';
    if (failure === 'consume') f.failConsume(); else f.failWrite(failure);
    expect((await f.adopt()).status).toBe('retained');
    expect(f.client.draft).toStartWith('Before\n\nShared task');
    const writes = f.writes.length;
    expect((await f.cancel()).status).toBe('cancelled');
    expect(f.writes).toHaveLength(writes); expect(f.client.draft).toBe('Before');
    expect(f.client.local.drafts.unrelated).toBe('Keep me');
    expect(f.client.snapshotDrafts).toEqual([]); expect(draftFiles(f.client.local)).toEqual([]);
    expect(mobileDraftAttachmentIds(f.client, key)).toEqual([]);
    expect(mobileIncomingShareImports(f.client)[key]).toBeUndefined();
    expect(mobileNewTaskTransferGuardBusy(f.client, key)).toBe(false);
    const cold = await fixture(f.disk()); expect(cold.client.draft).toBe('Before');
  }
});
test('lost cancellation reply retries durable native result without duplicate restore or merge', async () => {
  const f = await fixture(); f.client.local.drafts[key] = 'Before'; f.failConsume();
  await f.adopt(); const imported = f.client.draft; f.loseCancel();
  expect((await f.cancel()).status).toBe('retained'); expect(f.client.draft).toBe(imported);
  const disk = f.disk(); expect(obj(disk.drafts)[key]).toBe('Before');
  expect((await f.cancel()).status).toBe('cancelled'); expect(f.client.draft).toBe('Before');
  expect(f.disk()).toEqual(disk);
});
test('hot cancellation refuses edits after import and never calls native cancellation', async () => {
  const f = await fixture(); f.failConsume(); await f.adopt();
  f.client.local.drafts[key] = 'Later typing';
  expect((await f.cancel()).status).toBe('retained'); expect(f.client.draft).toBe('Later typing');
  expect(f.calls.some(call => call.action === 'cancel')).toBe(false);
});
test('a cancellation reply cannot overwrite edits or a different route made during native await', async () => {
  for (const mode of ['typing', 'route']) {
    const f = await fixture(); f.failConsume(); await f.adopt(); const imported = f.client.draft;
    f.setHook(async call => { if (call.action === 'cancel') {
      if (mode === 'typing') f.client.local.drafts[key] = 'Later typing'; else f.depart();
    } });
    await expect(f.cancel()).rejects.toThrow('draft changed');
    expect(f.client.draft).toBe(mode === 'typing' ? 'Later typing' : imported);
    expect(mobileIncomingShareImports(f.client)[key]?.[f.adoptionId]).toBeDefined();
    expect(mobileNewTaskTransferGuardBusy(f.client, key)).toBe(false);
  }
});
test('cold reservation cancellation restores exact saved draft but refuses unsaved new typing', async () => {
  const initial = await fixture(); initial.client.local.drafts[key] = 'Before'; initial.failConsume(); await initial.adopt();
  const cold = await fixture(initial.disk(), initial.journal);
  expect(obj(obj(obj(cold.client.local).mobileNewTaskDrafts).records)[key]).toMatchObject({ choices: null });
  const result = await cold.cancel();
  expect(result.status).toBe('cancelled'); expect(cold.client.draft).toBe('Before');
  const other = await fixture(); other.failConsume(); await other.adopt();
  const newer = await fixture(other.disk(), other.journal); newer.client.local.drafts[key] = 'Unsaved cold typing';
  expect((await newer.cancel()).status).toBe('retained'); expect(newer.client.draft).toBe('Unsaved cold typing');
});
test('consume winning retains imported draft', async () => {
  const f = await fixture(); await f.adopt(); const imported = f.client.draft;
  expect((await f.cancel()).status).toBe('retained'); expect(f.client.draft).toBe(imported);
  expect(mobileIncomingShareImports(f.client)[key]?.[f.adoptionId]).toBeDefined();
});

test('invalid native cancellation projection cannot mutate the local draft', async () => {
  const f = await fixture(); f.failConsume(); await f.adopt(); const imported = f.client.draft;
  const native: Native = { ...f.native, async later(request) {
    const answer = obj(await f.native.later(request));
    if (obj(request).action === 'cancel') return { ...answer, value: { ...obj(answer.value), restoredDraft: { text: 'Bad' } } };
    return answer;
  } };
  const result = await mobileIncomingShareCancel(f.client, native, f.storage, { entry: entry(), adoptionId: f.adoptionId, current: () => true });
  expect(result.status).toBe('retained'); expect(result.message).toContain('invalid restored draft');
  expect(f.client.draft).toBe(imported); expect(mobileIncomingShareImports(f.client)[key]?.[f.adoptionId]).toBeDefined();
});
test('cancel after staging failure restores the empty saved draft without manufacturing a receipt', async () => {
  const f = await fixture(); f.setHook(async call => { if (call.action === 'stage') throw new Error('Stage failed'); });
  expect((await f.adopt()).status).toBe('retained');
  expect((await f.cancel()).status).toBe('cancelled'); expect(f.client.draft).toBe('');
  expect(mobileIncomingShareImports(f.client)[key]).toBeUndefined();
  expect(obj(obj(obj(f.disk().mobileNewTaskDrafts).records)[key]).revision).toBe(1);
});

test('an empty share with only unsupported files preserves draft metadata through import persistence', async () => {
  const f = await fixture(); f.client.config = { environment: { capabilities: { attachmentUploads: false } } };
  f.setEntry({ ...entry(), text: '', attachments: [entry().attachments[0]!] }); f.failConsume();
  expect((await f.adopt()).status).toBe('retained');
  expect(obj(obj(f.disk().mobileNewTaskDrafts).records)[key]).toMatchObject({ key, revision: 1 });
  expect((await f.cancel()).status).toBe('cancelled'); expect(f.client.draft).toBe('');
});
