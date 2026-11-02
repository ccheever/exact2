import { expect, test } from 'bun:test';
import { MobileDraftClient, mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { mobileNewTaskDraftCreate as create, mobileNewTaskDraftBind as bind, mobileNewTaskDraftLookup as lookup } from './mobile-new-task-drafts';
import { mobileComposerAttachmentAction as action } from './composer-attachments';
import { draftFiles } from './shared/composer-editor-files';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
const imageId = '11111111-1111-4111-a111-111111111111';
const fileId = '22222222-2222-4222-a222-222222222222';
async function fixture(kind = 'image') {
  const client = new MobileDraftClient(), writes: Obj[] = [], calls: Obj[] = [];
  const storage: Files = { fs: { async mkdir() {}, async readFile() { return new TextEncoder().encode('{"version":1}').buffer; },
    async atomicWriteFile(_path, bytes) { writes.push(obj(JSON.parse(new TextDecoder().decode(bytes)))); } } };
  const native: Native = { available: true, watch() {}, async later(input) {
    const call = obj(input); calls.push(call);
    return { ok: true, generation: client.generation, value: call.op === 'composerAttachPick' ? { files: [
      { kind, id: kind === 'image' ? imageId : fileId, name: kind === 'image' ? 'photo.jpg' : 'notes.txt',
        mimeType: kind === 'image' ? 'image/jpeg' : 'text/plain', sizeBytes: 50 }] } : { applied: false } };
  } };
  const handles = mobileDraftRecoveryHandles(client, native, storage);
  await client.command('dismiss-error', '', '', 0, handles.native, handles.storage);
  await handles.native!.later({ op: 'devicePresentation' });
  Object.assign(client, { environmentId: 'env', projectId: 'project', origin: 'https://draft.test' });
  for (const id of ['A', 'B']) {
    create(client, { id, environmentId: 'env', projectId: 'project', origin: client.origin, createdAt: '2026-10-08T00:00:00.000Z' });
    client.local.drafts[`new-task:${id}`] = id;
  }
  bind(client, 'new-task:A', 'flow-A'); writes.length = 0; calls.length = 0;
  const run = (source = 'photos', id = '') => action(source, id, native, storage, client);
  return { client, native, storage, writes, calls, run };
}
for (const kind of ['image', 'file']) test(`${kind} picker fallback and first save belong to captured A after editor switches to B`, async () => {
  const f = await fixture(kind), original = f.native.later;
  f.native.later = async input => {
    const reply = await original(input);
    if (obj(input).op === 'editorInsert') bind(f.client, 'new-task:B', 'flow-B');
    return reply;
  };
  expect((await f.run())).toMatchObject({ message: '' });
  expect(f.client.local.drafts['new-task:A']).toContain('t3-context://');
  expect(f.client.draft).toBe('B'); expect(lookup(f.client, 'new-task:B')?.revision).toBe(0);
  expect(lookup(f.client, 'new-task:A')!.revision).toBeGreaterThan(0);
  expect(obj(obj(obj(f.writes[0].mobileNewTaskDrafts).records)['new-task:A']).revision).toBeGreaterThan(0);
  if (kind === 'image') expect(f.client.local.snapshotDrafts['new-task:A'][0]).toMatchObject({ mimeType: 'image/jpeg', name: 'photo.jpg' });
  else expect(draftFiles(f.client.local)[0].draftKey).toBe('new-task:A');
});
test('picker switch before acceptance releases bytes without revision or content changes', async () => {
  const f = await fixture(), original = f.native.later;
  f.native.later = async input => { const reply = await original(input); if (obj(input).op === 'composerAttachPick') bind(f.client, 'new-task:B', 'flow-B'); return reply; };
  expect((await f.run()).message).toContain('draft changed');
  expect(lookup(f.client, 'new-task:A')?.revision).toBe(0); expect(lookup(f.client, 'new-task:B')?.revision).toBe(0);
  expect(f.client.local.drafts).toMatchObject({ 'new-task:A': 'A', 'new-task:B': 'B' });
  expect(f.calls.some(call => call.op === 'snapshotDraftRemove')).toBe(true);
});
for (const kind of ['image', 'file']) test(`${kind} removal saves changed revision before deleting bytes`, async () => {
  const f = await fixture(kind); await f.run(); const before = lookup(f.client, 'new-task:A')!.revision;
  f.writes.length = 0; f.calls.length = 0;
  const original = f.native.later; let deleted = false;
  f.native.later = async input => {
    const op = obj(input).op;
    if (op === 'snapshotDraftRemove' || op === 'composerAttachRemove') {
      deleted = true; expect(f.writes.length).toBeGreaterThan(0);
      expect(obj(obj(obj(f.writes[0].mobileNewTaskDrafts).records)['new-task:A']).revision).toBeGreaterThan(before);
    }
    return original(input);
  };
  expect((await f.run(kind === 'image' ? 'remove-image' : 'remove-file', kind === 'image' ? imageId : fileId)).message).toBe('');
  expect(deleted).toBe(true); expect(f.client.draft.trim()).toBe('A'); expect(lookup(f.client, 'new-task:B')?.revision).toBe(0);
});
test('failed file removal persistence keeps byte cleanup queued without deleting the file', async () => {
  const f = await fixture('file'); await f.run(); f.calls.length = 0;
  f.storage.fs.atomicWriteFile = async () => { throw new Error('disk full'); };
  await f.run('remove-file', fileId);
  expect(f.calls.some(call => call.op === 'composerAttachRemove')).toBe(false);
  expect(f.client.local.drafts['new-task:B']).toBe('B');
});
