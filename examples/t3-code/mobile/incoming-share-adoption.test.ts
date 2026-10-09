import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate, mobileNewTaskDraftBind } from './mobile-new-task-drafts';
import { mobileIncomingShareAdopt } from './incoming-share-adoption';
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
async function fixture(saved: Obj = { version: 1 }) {
  const client = new MobileDraftClient(), calls: Obj[] = [], writes: Obj[] = []; let disk = JSON.stringify(saved);
  let writeFailure = '', loseConsumeReply = false, current = true, hook = async (_call: Obj) => {};
  let content = entry(), consumed = false, selected: string[] = [], owner: unknown;
  const adoptionId = uuid(91);
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode(disk).buffer; }, async atomicWriteFile(_path, bytes) {
    if (writeFailure === 'before') throw new Error('write failed');
    disk = new TextDecoder().decode(bytes); writes.push(JSON.parse(disk));
    if (writeFailure === 'after') throw new Error('lost save reply');
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
        owner = call.destination;
      }
      if (call.action === 'stage') selected = call.attachmentIds as string[];
      if (call.action === 'consume') {
        const receipt = obj(obj(obj(JSON.parse(disk)).mobileIncomingShareImports)[key])[adoptionId];
        if (!receipt) return { ok: false, generation: 0, error: { kind: 'Share', message: 'Save the draft first' } };
        consumed = true;
        if (loseConsumeReply) { loseConsumeReply = false; throw new Error('lost consume reply'); }
      }
      value = { adoptionId, consumed, entry: consumed ? null : content, attachments: selected.map(id => content.attachments.find(file => file.id === id)!) };
    }
    return { ok: true, generation: client.generation, value };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage); await client.refresh(handles.native, handles.storage);
  Object.assign(client, { environmentId: 'env', projectId: 'project', origin, generation: 1, threadId: '', configLive: true,
    config: { environment: { capabilities: { attachmentUploads: true, fileAttachments: { maxUploadBytes: 50 * 1024 * 1024 } } } } });
  if (!obj(obj(saved.mobileNewTaskDrafts).records)[key]) mobileNewTaskDraftCreate(client, { id: 'share', environmentId: 'env', projectId: 'project', origin, createdAt: '2026-10-08T09:00:00Z' });
  expect(mobileNewTaskDraftBind(client, key, 'flow')).toBe(true); calls.length = 0; writes.length = 0;
  return { client, calls, writes, native, storage, adoptionId, disk: () => JSON.parse(disk) as Obj,
    adopt: () => mobileIncomingShareAdopt(client, native, storage, { entry: content, current: () => current }),
    setEntry(value: IncomingShareEntry) { content = value; }, setHook(value: typeof hook) { hook = value; },
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
    expect(f.calls.some(call => call.action === 'consume')).toBe(false); expect(f.writes).toEqual([]);
  }
});
test('server policy changing while staging cannot silently adopt newly disallowed files', async () => {
  const f = await fixture(); f.setHook(async call => { if (call.action === 'stage') f.client.config = { environment: { capabilities: { attachmentUploads: false } } }; });
  expect((await f.adopt()).message).toContain('support changed'); expect(f.client.draft).toBe(''); expect(f.writes).toEqual([]);
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
