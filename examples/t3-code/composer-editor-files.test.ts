import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import type { T3Client } from './client';
import {
  adoptComposerFiles, base64Utf8, composerFileAttachments, composerFileRecords, draftFiles, fileAttachments, fileChipContexts, fileChipLink,
  fileContextRecords, fileSendBlock, fileStagingLimit, formatAttachmentSize, nextPastedTextFileName, pruneFiles, referencedFiles, stageFold, takeFold,
} from './composer-editor-files';

describe('large paste folding (textPaste.ts, foldPastedText)', () => {
  test('file names count up per draft', () => {
    expect(nextPastedTextFileName([])).toBe('pasted-text.txt');
    expect(nextPastedTextFileName(['Pasted-Text.txt'])).toBe('pasted-text-2.txt');
    expect(nextPastedTextFileName(['pasted-text.txt', 'pasted-text-2.txt'])).toBe('pasted-text-3.txt');
  });
  test('sizes read like formatAttachmentSize', () => {
    expect(formatAttachmentSize(40_000)).toBe('40 KB');
    expect(formatAttachmentSize(1)).toBe('1 KB');
    expect(formatAttachmentSize(3.2 * 1024 * 1024)).toBe('3.2 MB');
  });
  test('the staging limit follows the server capabilities, clamped to 50 MB', () => {
    expect(fileStagingLimit({})).toBe(0);
    expect(fileStagingLimit({ attachmentUploads: true })).toBe(0);
    expect(fileStagingLimit({ attachmentUploads: true, fileAttachments: { maxUploadBytes: 1_000_000 } })).toBe(1_000_000);
    expect(fileStagingLimit({ attachmentUploads: true, fileAttachments: { maxUploadBytes: 1e12 } })).toBe(50 * 1024 * 1024);
    expect(fileStagingLimit({ attachmentUploads: false, fileAttachments: { maxUploadBytes: 1_000 } })).toBe(0);
  });
  test('a staged paste becomes a file chip the send binds to its upload', () => {
    const local = {};
    const file = stageFold(local, 'env:t1', 'env', 'é'.repeat(20_000), '1b4e28ba-2fa1-11d2-883f-0016d3cca427');
    expect(file).toMatchObject({ name: 'pasted-text.txt', sizeBytes: 40_000, contextId: 'file_1b4e28ba-2fa1-11d2-883f-0016d3cca427', status: 'staged' });
    const link = fileChipLink(file);
    expect(link).toBe('[pasted-text.txt](t3-context://v1/file/file_1b4e28ba-2fa1-11d2-883f-0016d3cca427)');
    const second = stageFold(local, 'env:t1', 'env', 'x', 'id2');
    expect(second.name).toBe('pasted-text-2.txt');
    expect(referencedFiles(local, 'env:t1', `see ${link} now`)).toEqual([file]);
    expect(referencedFiles(local, 'env:t2', `see ${link} now`)).toEqual([]);
    expect(fileSendBlock([file])).toBe('Wait for attachments to finish uploading, or remove failed uploads.');
    file.status = 'ready'; file.attachmentId = 'att-1';
    expect(fileSendBlock([file])).toBe('');
    expect(fileAttachments([file])).toEqual([{ type: 'file', id: 'att-1', name: 'pasted-text.txt', mimeType: 'text/plain;charset=utf-8', sizeBytes: 40_000, source: { _tag: 'pasted-text' } }]);
    expect(fileContextRecords([file])).toEqual([{ version: 1, kind: 'file', contextId: file.contextId, label: 'pasted-text.txt', attachmentId: 'att-1',
      name: 'pasted-text.txt', mimeType: 'text/plain;charset=utf-8', sizeBytes: 40_000 }]);
  });
  test('only uploaded files persist, and empty drafts release theirs', () => {
    const local = {};
    const ready = stageFold(local, 'env:t1', 'env', 'a', 'r');
    ready.status = 'ready'; ready.attachmentId = 'att';
    stageFold(local, 'env:t1', 'env', 'b', 'u');
    const next = {};
    adoptComposerFiles(next, JSON.parse(JSON.stringify({ composerFiles: draftFiles(local) })));
    expect(draftFiles(next).map(file => file.id)).toEqual(['r']);
    pruneFiles(local, { 'env:t1': '' }, 'env:t1');
    expect(draftFiles(local)).toHaveLength(2);
    pruneFiles(local, { 'env:t1': 'still here' }, '');
    expect(draftFiles(local)).toHaveLength(2);
    pruneFiles(local, { 'env:t1': '' }, '');
    expect(draftFiles(local)).toEqual([]);
  });
  test('base64 of UTF-8 matches the platform encoder', () => {
    for (const text of ['', 'a', 'ab', 'abc', 'héllo wörld ✓', 'lorem ipsum '.repeat(999)]) expect(base64Utf8(text)).toBe(Buffer.from(text, 'utf8').toString('base64'));
  });
});

describe('fold, chip and send', () => {
  function harness() {
    const calls: Obj[] = [];
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = obj(input) as Obj; calls.push(request);
      return { ok: true, generation: 1, value: request.op === 'editorEdit' ? { applied: true } : {} };
    } };
    const local = { drafts: {} as Record<string, string> };
    const rpcs: Obj[] = [];
    const client = { local, draftKey: 'env:t1', environmentId: 'env', generation: 1,
      async rpc(_native: Native, method: string, payload: Obj) { rpcs.push({ method, payload }); return { attachmentId: 'att-9', relativeUrl: '/api/attachments/upload/x' }; } } as unknown as T3Client;
    return { native, client, calls, rpcs, local };
  }
  test('a held paste stages once, chips where it went, and uploads with the send', async () => {
    const { native, client, calls, rpcs } = harness();
    const fold = { id: 3, text: 'x'.repeat(40_000), start: 7, end: 7, expect: '' };
    await takeFold(client, native, fold, 'f1');
    await takeFold(client, native, fold, 'f2'); // the view re-asked: same fold, same file
    const edits = calls.filter(call => call.op === 'editorEdit');
    expect(edits).toHaveLength(2);
    expect(edits[0]).toMatchObject({ start: 7, end: 7, expect: '', pad: true, fold: 3, text: '[pasted-text.txt](t3-context://v1/file/file_f1) ' });
    expect(edits[1]!.text).toBe(edits[0]!.text);
    const prompt = `before ${String(edits[0]!.text)}`;
    expect(fileChipContexts(client, prompt)).toEqual({ 'file/file_f1': 'file\t40 KB' });
    expect(composerFileRecords(client, prompt)).toEqual([]);
    const attachments = await composerFileAttachments(client, native, prompt);
    expect(rpcs).toEqual([{ method: 'attachments.createUploadUrl', payload: { type: 'file', name: 'pasted-text.txt', mimeType: 'text/plain;charset=utf-8', sizeBytes: 40_000 } }]);
    expect(calls.find(call => call.op === 'uploadAttachment')).toMatchObject({ path: '/api/attachments/upload/x', contentType: 'text/plain;charset=utf-8' });
    expect(attachments).toEqual([{ type: 'file', id: 'att-9', name: 'pasted-text.txt', mimeType: 'text/plain;charset=utf-8', sizeBytes: 40_000, source: { _tag: 'pasted-text' } }]);
    expect(composerFileRecords(client, prompt)).toMatchObject([{ kind: 'file', contextId: 'file_f1', attachmentId: 'att-9' }]);
    // A retried send reuses the upload.
    await composerFileAttachments(client, native, prompt);
    expect(rpcs).toHaveLength(1);
  });
  test('a staged file whose bytes were lost cannot send and draws unresolved', async () => {
    const { native, client } = harness();
    const local = (client as unknown as { local: object }).local;
    stageFold(local, 'env:t1', 'env', 'abc', 'gone');
    const prompt = '[pasted-text.txt](t3-context://v1/file/file_gone) ';
    expect(fileChipContexts(client, prompt)).toEqual({});
    await expect(composerFileAttachments(client, native, prompt)).rejects.toThrow('pasted-text.txt was not saved with this draft. Attach it again to send it.');
  });
});
