// A composer file chip's press against T3 Code 1e2ecbd975 (MIT, LICENSE-T3): composerContextPresentation.tsx
// FileContextChip (`openFile`), ChatComposer.tsx's `previewFileId` Dialog and files/AttachmentFilePreview.tsx with
// origin "Draft", onRemove and onClose, over the draft's own bytes.
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { performChipAct } from '../../src/composer/composer-chip-details';
import { composerFileLocal, composerFileView, pastedPreview } from '../../src/composer/composer-chip-file';
import { setDraftFiles, takeFold, type DraftFile } from '../../src/composer/composer-editor-files';
import { attachmentView } from '../../src/panel/attachment-surface';
import { lastSave, memoryAppFiles, serveAssets } from '../support/asset-server';

let server: ReturnType<typeof serveAssets> | null = null;
afterEach(() => { server?.restore(); server = null; });

const ID = '11111111-2222-3333-4444-555555555555';
const file = (patch: Partial<DraftFile>): DraftFile => ({ id: ID, contextId: `file_${ID}`, draftKey: 'env:t1', environmentId: 'env', name: 'notes.js', mimeType: 'text/javascript',
  sizeBytes: 40, source: 'attached', attachmentId: '', status: 'staged', ...patch });

function harness(files: DraftFile[], prompt = `see [notes.js](t3-context://v1/file/file_${ID}) now`) {
  const calls: Obj[] = [];
  const reply = (_request: Obj): Obj => ({ ok: true, generation: 0, value: {} });
  // The staged copy under the app's data (T3ComposerAttach.swift writes it), and the server's asset once sent.
  const appFiles = memoryAppFiles(new Map([[`app:/data/composer-files/${ID}`, new TextEncoder().encode('const a = 1;\nexport { a };')]]));
  server?.restore();
  server = serveAssets(() => ({ body: 'const a = 1;\nexport { a };' }));
  const native = { available: true, watch() {}, later: async (request: Obj) => { calls.push(request); return { ok: true, generation: 0, value: {} }; } } as unknown as Native;
  const client = { draftKey: 'env:t1', ready: true, origin: 'http://127.0.0.1:16600', local: { drafts: { 'env:t1': prompt }, clientSettings: {} },
    rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ op: method, ...payload }); return { relativeUrl: '/api/assets/token/notes.js' }; },
    restAccess: () => ({ call: async (request: Obj) => { calls.push(request); return reply(request); } }) } as unknown as T3Client;
  setDraftFiles(client.local, files);
  return { calls, native, client, appFiles };
}

describe('a file chip\'s press opens AttachmentFilePreview over the draft\'s bytes', () => {
  test('a staged file\'s text is read from its staged copy, with the Draft crumb, Remove from draft and Close', async () => {
    const { calls, native, client, appFiles } = harness([file({})]);
    expect((await composerFileView(client, native, 1)).open).toBe(false);
    await performChipAct(client, native, { act: 'file', context: `file_${ID}` });
    const { open, file: view } = await composerFileView(client, native, 1);
    expect(open).toBe(true);
    expect(view).toMatchObject({ id: ID, origin: 'Draft', name: 'notes.js', size: '1 KB', preview: 'code', canRemove: true, canClose: true, canCopy: true, canSave: true, showWrap: true });
    expect(view.lines.map(line => line.runs.map(run => run.text).join(''))).toEqual(['const a = 1;', 'export { a };']);
    // The staged copy, never the server: nothing is uploaded or fetched to preview it.
    expect([calls, server!.calls]).toEqual([[], []]);
    // Save file offers the staged copy as it is to the window's Save panel.
    await composerFileLocal(client, native, 'surface-linked-att-save', ID);
    expect(lastSave(client, appFiles)).toMatchObject({ from: `app:/data/composer-files/${ID}`, name: 'notes.js', text: 'const a = 1;\nexport { a };' });
    // Close (and Escape, the backdrop's) ends it.
    await composerFileLocal(client, native, 'close', ID);
    expect((await composerFileView(client, native, 1)).open).toBe(false);
  });

  test('a PDF names its staged copy to the media body; another type is "No preview" with its actions', async () => {
    const { native, client } = harness([file({ name: 'spec.pdf', mimeType: 'application/pdf' })]);
    await performChipAct(client, native, { act: 'file', context: `file_${ID}` });
    expect((await composerFileView(client, native, 1)).file).toMatchObject({ preview: 'pdf', url: `t3-staged:${ID}`, canSave: true, canCopy: false, canRemove: true });
    const other = harness([file({ name: 'build.bin', mimeType: 'application/octet-stream' })]);
    await performChipAct(other.client, other.native, { act: 'file', context: `file_${ID}` });
    expect((await composerFileView(other.client, other.native, 1)).file).toMatchObject({ preview: 'none', noPreview: 'Save it to open in an app that supports bin files.', canSave: true });
  });

  test('a folded paste is read from memory; a file the send uploaded is the server\'s', async () => {
    const { calls, native, client } = harness([]);
    const fold = harness([]);
    await takeFold(fold.client, fold.native, { id: 1, text: 'pasted words', start: 0, end: 0, expect: '' }, ID);
    const pasted = fold.client.local.composerFiles as DraftFile[];
    await performChipAct(fold.client, fold.native, { act: 'file', context: pasted[0]!.contextId });
    expect((await composerFileView(fold.client, fold.native, 1)).file).toMatchObject({ origin: 'Draft', name: 'pasted-text.txt', preview: 'code' });
    expect(server!.calls).toEqual([]);
    expect(pastedPreview('a\0b')).toEqual({ ok: false, message: 'This file contains binary data and cannot be shown as text.' });
    expect(pastedPreview('é'.repeat(600_000))).toMatchObject({ ok: true, truncated: true });
    // A cut inside a two-byte sequence drops its half.
    const split = pastedPreview(`a${'é'.repeat(600_000)}`);
    expect(split).toMatchObject({ ok: true, truncated: true });
    expect(String(split.text).endsWith('é')).toBe(true);
    expect(new TextEncoder().encode(String(split.text)).length).toBe(1024 * 1024 - 1);

    setDraftFiles(client.local, [file({ status: 'ready', attachmentId: 'att-9' })]);
    await performChipAct(client, native, { act: 'file', context: `file_${ID}` });
    const view = (await composerFileView(client, native, 1)).file;
    expect(view).toMatchObject({ id: ID, origin: 'Draft' });
    expect(calls.filter(call => call.op === 'assets.createUrl').map(call => (call.resource as Obj).attachmentId)).toEqual(['att-9']);
    expect(server!.calls.map(call => call.url)).toEqual(['http://127.0.0.1:16600/api/assets/token/notes.js']);
  });

  test('Remove from draft removes the chip and its staged copy, and closes', async () => {
    const { calls, native, client } = harness([file({})]);
    await performChipAct(client, native, { act: 'file', context: `file_${ID}` });
    await composerFileLocal(client, native, 'surface-linked-att-remove', ID);
    expect(client.local.drafts['env:t1']).toBe('see now');
    expect(client.local.composerFiles).toEqual([]);
    expect(calls.some(call => call.op === 'composerAttachRemove' && call.id === ID)).toBe(true);
    expect((await composerFileView(client, native, 1)).open).toBe(false);
  });

  test('leaving the draft closes it; a late control of another preview leaves it open', async () => {
    const { native, client } = harness([file({})]);
    await performChipAct(client, native, { act: 'file', context: `file_${ID}` });
    await composerFileLocal(client, native, 'surface-linked-att-media-error', 'another-file');
    expect((await composerFileView(client, native, 1)).open).toBe(true);
    (client as unknown as { draftKey: string }).draftKey = 'env:t2';
    expect((await composerFileView(client, native, 1)).open).toBe(false);
    (client as unknown as { draftKey: string }).draftKey = 'env:t1';
    expect((await composerFileView(client, native, 1)).open).toBe(false);
  });

  test('a sent attachment\'s panel keeps its "Attachment" crumb and no Remove or Close', async () => {
    const { native, client } = harness([]);
    const view = await attachmentView(client, native, { id: 'att-1', name: 'a.txt', mimeType: 'text/plain', sizeBytes: 3 }, 1);
    expect(view).toMatchObject({ id: 'att-1', origin: 'Attachment', canRemove: false, canClose: false });
  });

  test('a file no longer in the draft, or another draft\'s, opens nothing', async () => {
    const { native, client } = harness([file({ draftKey: 'env:other' })]);
    await expect(performChipAct(client, native, { act: 'file', context: `file_${ID}` })).rejects.toThrow('That file is no longer attached.');
    expect((await composerFileView(client, native, 1)).open).toBe(false);
  });
});
