import { expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { queuedEditState, type MobileQueuedEditSession } from './queued-edit-state';
import { prepareComposerAttachmentPreviews, composerAttachmentPreview } from './composer-attachment-previews';
const item = (id: string, retained = true) => ({ id, name: `${id}.jpg`, mimeType: 'image/jpeg', kind: 'image', removeOperation: retained ? 'remove-retained' : 'remove-snapshot' });
function fixture() {
  const client = new T3Client(); client.environmentId = 'env'; client.origin = 'http://example.test'; client.threadId = 'thread';
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(value) {
    const request = obj(value); calls.push(request);
    return { ok: true, generation: client.generation, value: request.op === 'request'
      ? { relativeUrl: '/asset', expiresAt: 400_000 } : { dataUrl: 'data:image/jpeg;base64,thumb' } };
  } };
  const edit: MobileQueuedEditSession = { owner: 'edit1', draftKey: 'env:thread~queued-edit~run', session: 'one',
    environmentId: 'env', origin: client.origin, threadId: 'thread', projectId: '', generation: client.generation,
    revision: 1, runId: 'run', messageId: 'message', text: 'edit', attachments: [], existingAttachments: [], saving: false };
  queuedEditState(client).sessions.set(edit.owner, edit); queuedEditState(client).active.set('env:thread', edit.owner);
  return { client, native, calls, edit };
}
test('retained images sign remote metadata; newly picked images preview local bytes', async () => {
  const f = fixture(), retained = item('retained'), local = item('local', false);
  await prepareComposerAttachmentPreviews(f.native, f.client, () => [retained, local], 100);
  expect(f.calls.find(call => call.method === 'assets.createUrl')?.payload).toEqual({ resource: { _tag: 'attachment', attachmentId: 'retained', fileName: 'retained.jpg', mimeType: 'image/jpeg', disposition: 'inline' } });
  expect(f.calls.filter(call => call.op === 'mobileAttachmentPreview').map(call => call.id)).toEqual(['local']);
  expect(composerAttachmentPreview(f.client, retained, 100)).toBe('http://example.test/asset');
  expect(composerAttachmentPreview(f.client, local, 100)).toContain('data:image/jpeg');
  await prepareComposerAttachmentPreviews(f.native, f.client, () => [retained, local], 200);
  expect(f.calls).toHaveLength(2);
  await prepareComposerAttachmentPreviews(f.native, f.client, () => [retained, local], 300_101);
  expect(f.calls).toHaveLength(3);
  expect(composerAttachmentPreview(f.client, retained, 400_000)).toBe('');
});
test('a replacement edit and changed metadata cannot adopt an earlier image answer', async () => {
  const f = fixture(), image = item('same');
  let release!: () => void;
  const original = f.native.later;
  f.native.later = async input => { await new Promise<void>(resolve => { release = resolve; }); return original(input); };
  const read = prepareComposerAttachmentPreviews(f.native, f.client, () => [image], 100);
  await Promise.resolve();
  queuedEditState(f.client).active.set('env:thread', 'edit2');
  queuedEditState(f.client).sessions.set('edit2', { ...f.edit, owner: 'edit2', session: 'two' });
  release(); await expect(read).rejects.toMatchObject({ kind: 'superseded' });
  expect(composerAttachmentPreview(f.client, image, 100)).toBe('');
  f.native.later = original;
  await prepareComposerAttachmentPreviews(f.native, f.client, () => [image], 100);
  expect(composerAttachmentPreview(f.client, { ...image, name: 'different.jpg' }, 100)).toBe('');
});
test('all started workers settle before the answer returns after ownership changes', async () => {
  const f = fixture(), images = [item('a'), item('b'), item('c'), item('d'), item('e')];
  const releases: (() => void)[] = []; let returned = false;
  const original = f.native.later;
  f.native.later = async value => { await new Promise<void>(resolve => { releases.push(resolve); }); return original(value); };
  const read = prepareComposerAttachmentPreviews(f.native, f.client, () => images, 100).then(() => { returned = true; return null; }, error => { returned = true; return error; });
  await Promise.resolve(); expect(releases).toHaveLength(4);
  f.client.threadId = 'other'; releases[0]!(); await Promise.resolve(); await Promise.resolve();
  expect(returned).toBe(false);
  for (const release of releases.slice(1)) release(); expect(await read).toMatchObject({ kind: 'superseded' });
  expect(f.calls).toHaveLength(4); expect(returned).toBe(true);
});
