import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';
import { acceptAttachedFile, attachStagingLimit, fileTooLargeMessage, imageChipContexts, imageChipLink, imageContextRecords, imagesGetChips, insertFileChips, reservedAttachments } from './composer-editor-attach';
import { adoptComposerFiles, composerFileAttachments, composerFileRecords, draftFiles, fileChipContexts, pruneFiles } from './composer-editor-files';

const uuid = (n: number) => `1b4e28ba-2fa1-11d2-883f-0016d3cca42${n}`;
function harness(capabilities: Obj = { attachmentUploads: true, fileAttachments: { maxUploadBytes: 26_214_400 } }, applied = true) {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input) as Obj; calls.push(request);
    if (request.op === 'composerAttachRead') return { ok: true, generation: 1, value: { base64: 'aGVsbG8=', sizeBytes: 5 } };
    return { ok: true, generation: 1, value: request.op === 'editorInsert' ? { applied } : {} };
  } };
  const rpcs: Obj[] = [];
  const local = { drafts: {} as Record<string, string> };
  const client = { local, draftKey: 'env:t1', environmentId: 'env', generation: 1, snapshotDrafts: [] as Obj[], draft: '',
    config: { environment: { capabilities } },
    async rpc(_native: Native, method: string, payload: Obj) { rpcs.push({ method, payload }); return { attachmentId: 'att-7', relativeUrl: '/api/attachments/upload/y' }; } } as unknown as T3Client;
  return { native, client, calls, rpcs, local };
}

describe('Attach files: plain files (addComposerAttachments)', () => {
  test('the staging limit follows fileAttachmentStagingLimit', () => {
    expect(attachStagingLimit(harness({}).client)).toBe(50 * 1024 * 1024);
    expect(attachStagingLimit(harness().client)).toBe(26_214_400);
    expect(attachStagingLimit(harness({ attachmentUploads: false }).client)).toBe(0);
  });
  test('refusals read as the reference names them', () => {
    const { client } = harness();
    expect(acceptAttachedFile(client, { kind: 'file', name: 'a.zip', sizeBytes: 10 }, 0).error).toBe('This server does not support file attachments.');
    expect(acceptAttachedFile(client, { kind: 'file', name: 'empty.txt', sizeBytes: 0 }).error).toBe("'empty.txt' is empty or could not be read.");
    expect(acceptAttachedFile(client, { kind: 'file', name: 'big.bin', sizeBytes: 30_000_000 }).error).toBe("'big.bin' exceeds the 25 MB attachment limit.");
    expect(fileTooLargeMessage('x', 1536)).toBe("'x' exceeds the 1536 bytes attachment limit.");
    expect(fileTooLargeMessage('x', 2048)).toBe("'x' exceeds the 2 KB attachment limit.");
    expect(draftFiles(client.local)).toEqual([]);
  });
  test('an accepted file is a chip at the caret that uploads under its own type and binds to the send', async () => {
    const { client, native, calls, rpcs } = harness();
    const staged = acceptAttachedFile(client, { kind: 'file', id: uuid(1), name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 5 });
    expect(staged.error).toBe('');
    await insertFileChips(client, native, [staged.file!]);
    const insert = calls.find(call => call.op === 'editorInsert')!;
    expect(insert.text).toBe(`[notes.md](t3-context://v1/file/file_${uuid(1)})`);
    const prompt = `see ${String(insert.text)} `;
    // Its bytes are on this device, so the chip resolves (name and size) before any upload.
    expect(fileChipContexts(client, prompt)).toEqual({ [`file/file_${uuid(1)}`]: 'file\t1 KB' });
    const attachments = await composerFileAttachments(client, native, prompt);
    expect(rpcs).toEqual([{ method: 'attachments.createUploadUrl', payload: { type: 'file', name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 5 } }]);
    expect(calls.find(call => call.op === 'composerAttachRead')).toMatchObject({ id: uuid(1) });
    expect(calls.find(call => call.op === 'uploadAttachment')).toMatchObject({ path: '/api/attachments/upload/y', base64: 'aGVsbG8=', contentType: 'text/markdown' });
    expect(calls.find(call => call.op === 'composerAttachRemove')).toMatchObject({ id: uuid(1) });
    expect(attachments).toEqual([{ type: 'file', id: 'att-7', name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 5 }]);
    expect(composerFileRecords(client, prompt)).toMatchObject([{ kind: 'file', contextId: `file_${uuid(1)}`, attachmentId: 'att-7', name: 'notes.md' }]);
  });
  test('several files share one insert; without an editor the chips join the stored draft', async () => {
    const { client, native, calls } = harness(undefined, false);
    client.local.drafts['env:t1'] = 'hello';
    const files = [1, 2].map(n => acceptAttachedFile(client, { kind: 'file', id: uuid(n), name: `f${n}.txt`, sizeBytes: 3 }).file!);
    await insertFileChips(client, native, files);
    expect(calls.filter(call => call.op === 'editorInsert')).toHaveLength(1);
    expect(client.local.drafts['env:t1']).toBe(`hello [f1.txt](t3-context://v1/file/file_${uuid(1)}) [f2.txt](t3-context://v1/file/file_${uuid(2)}) `);
  });
  test('the draft counts its referenced files toward the 100-attachment limit', () => {
    const { client } = harness();
    const file = acceptAttachedFile(client, { kind: 'file', id: uuid(3), name: 'c.txt', sizeBytes: 3 }).file!;
    (client as unknown as { draft: string }).draft = `[c.txt](t3-context://v1/file/${file.contextId}) `;
    expect(reservedAttachments(client)).toBe(1);
  });
  test('a staged attached file survives a restart; a folded paste does not', () => {
    const next = {};
    adoptComposerFiles(next, { composerFiles: [
      { id: uuid(4), contextId: `file_${uuid(4)}`, draftKey: 'env:t1', environmentId: 'env', name: 'd.pdf', mimeType: 'application/pdf', sizeBytes: 9, source: 'attached', attachmentId: '', status: 'staged' },
      { id: 'p1', contextId: 'file_p1', draftKey: 'env:t1', environmentId: 'env', name: 'pasted-text.txt', sizeBytes: 9, source: 'pasted-text', attachmentId: '', status: 'staged' },
    ] });
    expect(draftFiles(next).map(file => [file.name, file.status])).toEqual([['d.pdf', 'staged']]);
  });
  test('a lost staged copy refuses the send with the reattach message', async () => {
    const { client, calls } = harness();
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = obj(input) as Obj; calls.push(request);
      return request.op === 'composerAttachRead' ? { ok: false, generation: 1, error: { kind: 'Attach', message: 'The attached file was not saved.' } } : { ok: true, generation: 1, value: {} };
    } };
    const file = acceptAttachedFile(client, { kind: 'file', id: uuid(5), name: 'e.csv', sizeBytes: 3 }).file!;
    await expect(composerFileAttachments(client, native, `[e.csv](t3-context://v1/file/${file.contextId}) `)).rejects.toThrow('e.csv was not saved with this draft. Attach it again to send it.');
  });
  test('pruning an emptied draft reports the files it dropped', () => {
    const { client } = harness();
    const file = acceptAttachedFile(client, { kind: 'file', id: uuid(6), name: 'g.txt', sizeBytes: 3 }).file!;
    expect(pruneFiles(client.local, { 'env:t1': '' }, '')).toEqual([file]);
    expect(draftFiles(client.local)).toEqual([]);
  });
});

describe('Attach files: image chips (imageAttachmentsGetChips)', () => {
  const image = { id: '1b4e28ba-2fa1-11d2-883f-0016d3cca427', name: 'photo.png', mimeType: 'image/png', sizeBytes: 1042 };
  test('an image gets an inline chip only when the prompt has prose', async () => {
    const { client, native, calls } = harness();
    expect(imagesGetChips(client)).toBe(false);
    (client as unknown as { draft: string }).draft = '[notes.md](t3-context://v1/file/file_x) ';
    expect(imagesGetChips(client)).toBe(false);
    (client as unknown as { draft: string }).draft = 'look at this';
    expect(imagesGetChips(client)).toBe(true);
    const file = acceptAttachedFile(client, { kind: 'file', id: uuid(7), name: 'n.txt', sizeBytes: 3 }).file!;
    await insertFileChips(client, native, [file], [image]);
    expect(calls.find(call => call.op === 'editorInsert')!.text).toBe(`[n.txt](t3-context://v1/file/file_${uuid(7)}) ![photo.png](t3-context://v1/image/image_${image.id})`);
  });
  test('the chip resolves to the shelf image and binds to its upload', () => {
    const { client } = harness();
    const uploaded = { ...image, uploadId: 'att-img' };
    (client as unknown as { snapshotDrafts: Obj[] }).snapshotDrafts = [uploaded];
    const prompt = `see ${imageChipLink(image)} `;
    expect(imageChipContexts(client, prompt)).toEqual({ [`image/image_${image.id}`]: `image\t2 KB\t${image.id}` });
    expect(imageContextRecords(client, prompt)).toEqual([{ version: 1, kind: 'image', contextId: `image_${image.id}`, label: 'photo.png', attachmentId: 'att-img', name: 'photo.png', mimeType: 'image/png', sizeBytes: 1042 }]);
    // A chip whose image left the shelf draws unresolved and binds nothing.
    (client as unknown as { snapshotDrafts: Obj[] }).snapshotDrafts = [];
    expect(imageChipContexts(client, prompt)).toEqual({});
    expect(imageContextRecords(client, prompt)).toEqual([]);
  });
});
