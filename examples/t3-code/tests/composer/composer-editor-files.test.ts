import { describe, expect, test } from 'bun:test';
import { toasts } from '../../src/shell/toast';
import { memoryAppFiles } from '../support/asset-server';
import { observeComposer, acceptComposerEdit } from '../support/composer-field-fixture';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { T3Client } from '../../src/client/client';
import {
  adoptComposerFiles, base64Utf8, composerFileAttachments, composerFileRecords, draftFiles, fileAttachments, fileChipContexts, fileChipLink,
  fileContextRecords, fileSendBlock, fileStagingLimit, formatAttachmentSize, nextPastedTextFileName, pruneFiles, referencedFiles, stageFold, foldPaste, heldPaste,
} from '../../src/composer/composer-editor-files';

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
  function harness(armed = { menu: false }) {
    const calls: Obj[] = [];
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = obj(input) as Obj; calls.push(request);
      if (request.op === 'composerPasteAsText') { const value = { armed: armed.menu }; armed.menu = false; return { ok: true, generation: 1, value }; }
      return { ok: true, generation: 1, value: request.op === 'editorEdit' ? { applied: acceptComposerEdit(client, request) } : {} };
    } };
    const rpcs: Obj[] = [], client = new T3Client();
    Object.defineProperties(client, { draftKey: { value: 'env:t1' }, environmentId: { value: 'env' } });
    client.generation = 1;
    client.rpc = async (_native: Native, method: string, payload: Obj) => { rpcs.push({ method, payload }); return { attachmentId: 'att-9', relativeUrl: '/api/attachments/upload/x' }; };
    const local = client.local;
    observeComposer(client, 'before ');
    return { native, client, calls, rpcs, local };
  }
  test('a held paste stages once, chips where it went, and uploads with the send', async () => {
    const { native, client, calls, rpcs } = harness();
    const current = observeComposer(client, 'before ');
    const fold = { id: 3, snapshotOwner: current.snapshotOwner, fieldOwner: current.owner, value: current.value, text: 'x'.repeat(40_000), start: 7, end: 7 };
    await foldPaste(client, native, 'fold', fold, 'f1');
    await foldPaste(client, native, 'fold', fold, 'f2'); // the same record again: no second file
    const edits = calls.filter(call => call.op === 'editorEdit');
    expect(edits).toHaveLength(1);
    expect(edits[0]).toMatchObject({ start: 7, end: 7, pad: true, text: '[pasted-text.txt](t3-context://v1/file/file_f1) ' });
    expect(draftFiles(client.local)).toHaveLength(1);
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
  test('a paste over 2 MiB uploads from an app file, removed once sent', async () => {
    const { native, client, calls } = harness();
    const written = new Map<string, number>();
    const files = memoryAppFiles(new class extends Map<string, Uint8Array> {
      override set(path: string, bytes: Uint8Array) { written.set(path, bytes.byteLength); return super.set(path, bytes); }
    }());
    const current = observeComposer(client, 'before ');
    const size = 2 * 1024 * 1024 + 1;
    await foldPaste(client, native, 'fold', { id: 6, snapshotOwner: current.snapshotOwner, fieldOwner: current.owner, value: current.value, text: 'x'.repeat(size), start: 7, end: 7 }, 'f3');
    const prompt = `before ${String(calls.find(call => call.op === 'editorEdit')!.text)}`;
    await composerFileAttachments(client, native, prompt);
    const upload = calls.find(call => call.op === 'uploadAttachment')!;
    expect(upload).toMatchObject({ bodyFrom: 'app:/tmp/composer-stage/folds/1.txt', sizeBytes: size, contentType: 'text/plain;charset=utf-8' });
    expect(upload.base64).toBeUndefined();
    expect(written.get('app:/tmp/composer-stage/folds/1.txt')).toBe(size);
    expect([...files.keys()]).toEqual([]);
  });
  test('a refused paste is the reference\'s toast and stages nothing', async () => {
    const { native, client, calls } = harness();
    const current = observeComposer(client, 'before ');
    const held = { id: 4, snapshotOwner: current.snapshotOwner, fieldOwner: current.owner, value: current.value, text: 'x'.repeat(130_000), start: 7, end: 7 };
    await foldPaste(client, native, 'too-large-message', held, 'f1');
    await foldPaste(client, native, 'too-large-attach', { ...held, id: 5 }, 'f2');
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([
      ['error', 'Pasted text is too large for this message', 'Remove some text or an attachment, then paste again.'],
      ['error', 'Pasted text is too large to attach', 'Reduce the clipboard contents or save a smaller excerpt as a file.']]);
    expect(draftFiles(client.local)).toEqual([]);
    expect(calls.filter(call => call.op !== 'composerPasteAsText')).toEqual([]);
  });
  test('Paste as Text chosen from the menu, or a held paste its bytes keep under the threshold, goes in as the field\'s paste', async () => {
    const armed = { menu: true }, { native, client, calls } = harness(armed);
    const current = observeComposer(client, 'before ');
    const held = { id: 6, snapshotOwner: current.snapshotOwner, fieldOwner: current.owner, value: current.value, text: 'x'.repeat(130_000), start: 7, end: 7 };
    await foldPaste(client, native, 'too-large-message', held, 'f1');
    expect(toasts(client)).toEqual([]);
    expect(calls.filter(call => call.op === 'editorEdit')).toMatchObject([{ start: 7, end: 7, expect: '', text: held.text }]);
    expect(armed.menu).toBe(false);
    // "measure": 20,000 ASCII units are 20,000 bytes, under 32 KB.
    calls.length = 0;
    const after = observeComposer(client, 'before ');
    await foldPaste(client, native, 'measure', { ...held, id: 7, value: after.value, text: 'y'.repeat(20_000) }, 'f2');
    expect(calls.filter(call => call.op === 'editorEdit')).toMatchObject([{ text: 'y'.repeat(20_000) }]);
    expect(draftFiles(client.local)).toEqual([]);
  });
  test('a held record reads back from attachFiles\' files', () => {
    expect(heldPaste(['fold\tsnap\tfield\t2\t5\t123', 'value', 'text'])).toEqual({ disposition: 'fold', snapshotOwner: 'snap', fieldOwner: 'field', start: 2, end: 5, id: 123, value: 'value', text: 'text' });
    expect(heldPaste(['fold\tsnap', 'value', 'text'])).toBeNull();
    expect(heldPaste(['doc:/1/a.txt'])).toBeNull();
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
