// composer-fidelity G12a. Ports of T3 Code 1e2ecbd975 (MIT; see LICENSE-T3)
// apps/web/src/components/chat/queuedMessageEdit.test.ts (6), the editQueuedRun
// case of packages/client-runtime/src/operations/commands.test.ts, and agent-side
// mirrors of QueuedRunsControl.test.tsx ("renders an attachment thumbnail on the
// queued row", "keeps the original queued message visible while editing"),
// against the composer-controls fake backend.
import { describe, expect, test } from 'bun:test';
import { arr, type Obj } from './domain';
import { opened, running } from './composer-controls-fixture';
import { snapshot } from './presentation';
import { toasts } from './toast';
import { ATTACHMENT_ONLY_PROMPT, mergedEditContext, prepareQueuedEditAttachments, recoverQueuedMessageEdit } from './queued-edit-attachments';
import type { T3Client } from './client';

const file = { type: 'file', id: 'file:report', name: 'report.pdf', mimeType: 'application/pdf', sizeBytes: 6 };
const image = { type: 'image', id: 'image:screen', name: 'screen.png', mimeType: 'image/png', sizeBytes: 5 };
const uploadedFile = { type: 'file', id: 'upload:report', name: 'report.pdf', mimeType: 'application/pdf', sizeBytes: 6 };
const uploadedImage = { type: 'image', id: 'upload:screen', name: 'screen.png', mimeType: 'image/png', sizeBytes: 5 };

describe('queued message file edits (queuedMessageEdit.test.ts)', () => {
  test.each([{ images: [] as Obj[], label: 'file-only' }, { images: [image], label: 'mixed' }])('preserves a generic file in a $label save', async ({ images }) => {
    const attachments = await prepareQueuedEditAttachments({ existing: [], images, files: [file], uploadFiles: async () => [uploadedFile], uploadImages: async () => [uploadedImage] });
    expect(attachments.at(-1)).toEqual(uploadedFile);
    expect(attachments.length).toBe(images.length + 1);
    if (images.length) expect(attachments[0]).toMatchObject({ type: 'image', id: 'upload:screen' });
  });
  test('retains saved attachments alongside newly uploaded files', async () => {
    const saved = { ...uploadedFile, id: 'saved:earlier' };
    expect(await prepareQueuedEditAttachments({ existing: [saved], images: [], files: [file], uploadFiles: async () => [uploadedFile], uploadImages: async () => [] })).toEqual([saved, uploadedFile]);
  });
  test('fails a save instead of dropping a file whose upload is missing', async () => {
    await expect(prepareQueuedEditAttachments({ existing: [], images: [image], files: [file], uploadFiles: async () => [], uploadImages: async () => [uploadedImage] })).rejects.toThrow('Retry or remove');
  });
  test('keeps file-only edits when another client starts the queued run', () => {
    expect(recoverQueuedMessageEdit({ prompt: 'Original message', images: 0, files: 1, originalText: 'Original message', threadHasContent: false })).toBe('kept');
  });
  test('preserves an edit with only new attachments when the queue advances remotely', () => {
    expect(recoverQueuedMessageEdit({ prompt: '', images: 1, files: 0, originalText: '', threadHasContent: false })).toBe('kept');
  });
  test('does not overwrite a separate draft when the queued run leaves', () => {
    expect(recoverQueuedMessageEdit({ prompt: '', images: 0, files: 1, originalText: '', threadHasContent: true })).toBe('discarded');
    expect(recoverQueuedMessageEdit({ prompt: 'same', images: 0, files: 0, originalText: 'same', threadHasContent: true })).toBe('clean');
  });
  test('the context keeps records of kept attachments and adds the new prompt\'s', () => {
    const original = { version: 1, records: [{ contextId: 'a', attachmentId: 'kept' }, { contextId: 'b', attachmentId: 'gone' }, { contextId: 'c', kind: 'thread' }] };
    expect(mergedEditContext(original, [{ id: 'kept' }], { version: 1, records: [{ contextId: 'd', attachmentId: 'new' }] }).records)
      .toEqual([{ contextId: 'a', attachmentId: 'kept' }, { contextId: 'c', kind: 'thread' }, { contextId: 'd', attachmentId: 'new' }]);
  });
});

async function queuedWithAttachments() {
  const context = await opened();
  const { client } = context;
  running(client, { activeAttemptId: 'a1' });
  client.thread!.projection.runs = [...arr(client.thread!.projection.runs), { id: 'q1', ordinal: 2, status: 'queued', queuePosition: 1, userMessageId: 'm1' }];
  client.thread!.projection.messages = [{ id: 'm1', text: 'Queued with a screenshot', attachments: [
    { type: 'image', id: 'attachment-1', name: 'kept.png', mimeType: 'image/png', sizeBytes: 64 },
    { type: 'file', id: 'attachment-2', name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 12 }],
    context: { version: 1, records: [{ version: 1, kind: 'file', contextId: 'ctx-2', attachmentId: 'attachment-2' }] } }];
  return context;
}
const edits = (committed: Obj[]) => committed.filter(entry => entry.type === 'queued-run.edit');
const lastToast = (client: T3Client) => toasts(client).at(-1);

describe('editing a queued message with its attachments (G12a)', () => {
  test('the row shows its image thumbnail, and stays visible while edited', async () => {
    const { client, command } = await queuedWithAttachments();
    expect(snapshot(client).composer.queued[0]).toMatchObject({ text: 'Queued with a screenshot', thumbnails: [{ id: 'attachment-1', name: 'kept.png' }], pending: false });
    await command('cclocal:queued-edit', 'q1');
    const composer = snapshot(client).composer;
    expect(composer.queued[0]).toMatchObject({ text: 'Queued with a screenshot', editing: true });
    expect(composer.queueEditAttachments).toEqual([{ id: 'attachment-1', name: 'kept.png', image: true }, { id: 'attachment-2', name: 'notes.txt', image: false }]);
    expect(client.draft).toBe('Queued with a screenshot');
  });
  test('removing a kept attachment and saving sends the full remaining list with its context', async () => {
    const { client, native, command } = await queuedWithAttachments();
    await command('draft', '', 'Before editing');
    await command('cclocal:queued-edit', 'q1');
    await command('cclocal:queued-attachment-remove', 'attachment-2');
    await command('draft', '', 'Edited text');
    const result = await command('send', '', 'Edited text');
    expect(result.message).toBe('');
    expect(edits(native.committed).at(-1)).toMatchObject({ type: 'queued-run.edit', runId: 'q1', text: 'Edited text', attachments: [{ id: 'attachment-1' }] });
    expect(edits(native.committed).at(-1)?.context).toBeUndefined();
    expect(client.draft).toBe('Before editing');
    expect(snapshot(client).composer.queueEditing).toBe('');
  });
  test('an empty text with attachments sends the attachment-only prompt; nothing at all is a no-op', async () => {
    const { client, native, command } = await queuedWithAttachments();
    await command('cclocal:queued-edit', 'q1');
    await command('draft', '', '');
    await command('send', '', '');
    expect(edits(native.committed).at(-1)).toMatchObject({ text: ATTACHMENT_ONLY_PROMPT, attachments: [{ id: 'attachment-1' }, { id: 'attachment-2' }],
      context: { version: 1, records: [{ contextId: 'ctx-2', attachmentId: 'attachment-2' }] } });
    await command('cclocal:queued-edit', 'q1');
    await command('cclocal:queued-attachment-remove', 'attachment-1');
    await command('cclocal:queued-attachment-remove', 'attachment-2');
    await command('draft', '', '');
    const before = native.committed.length;
    await command('send', '', '');
    expect(native.committed.length).toBe(before);
    expect(client.error).toBe('');
  });
  test('more than 100 attachments are refused with the reference wording', async () => {
    const { client, native, command } = await queuedWithAttachments();
    client.thread!.projection.messages = [{ id: 'm1', text: 'many', attachments: Array.from({ length: 101 }, (_, index) => ({ type: 'file', id: `f${index}`, name: `f${index}.txt` })) }];
    await command('cclocal:queued-edit', 'q1');
    const before = native.committed.length;
    const result = await command('send', '', 'many');
    expect(result.message).toBe('A message can have at most 100 attachments.');
    expect(native.committed.length).toBe(before);
  });
  test('a refused write reads "Could not save the edited queued message."', async () => {
    const { client, native, command } = await queuedWithAttachments();
    await command('cclocal:queued-edit', 'q1');
    const later = native.later.bind(native);
    native.later = async (input: unknown) => {
      const request = input as Obj;
      if (request.op === 'request' && (request.payload as Obj)?.type === 'queued-run.edit') return { ok: false, generation: native.generation, error: { kind: 'server', message: 'refused', uncertain: false } };
      return later(input);
    };
    const result = await command('send', '', 'Edited');
    expect(result.message).toBe('Could not save the edited queued message.');
    expect(snapshot(client).composer.queueEditing).toBe('q1');
  });
  test('the run leaving the queue keeps a dirty edit in an empty draft, or discards it with the reference wording', async () => {
    const kept = await queuedWithAttachments();
    await kept.command('cclocal:queued-edit', 'q1');
    await kept.command('draft', '', 'Changed while queued');
    kept.client.thread!.projection.runs = arr(kept.client.thread!.projection.runs).filter(run => run.id !== 'q1');
    snapshot(kept.client);
    expect(lastToast(kept.client)).toMatchObject({ title: 'Queued message is no longer queued', description: 'Your unsaved edit was kept in the composer.' });
    expect(kept.client.draft).toBe('Changed while queued');

    const dropped = await queuedWithAttachments();
    await dropped.command('draft', '', 'My own draft');
    await dropped.command('cclocal:queued-edit', 'q1');
    await dropped.command('draft', '', 'Changed while queued');
    dropped.client.thread!.projection.runs = arr(dropped.client.thread!.projection.runs).filter(run => run.id !== 'q1');
    snapshot(dropped.client);
    expect(lastToast(dropped.client)).toMatchObject({ title: 'Queued message is no longer queued', description: 'Your unsaved edit was discarded.' });
    expect(dropped.client.draft).toBe('My own draft');
  });
});
