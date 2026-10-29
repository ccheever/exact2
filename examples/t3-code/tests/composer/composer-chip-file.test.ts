// A composer file chip's press against T3 Code 1e2ecbd975 (MIT, LICENSE-T3): composerContextPresentation.tsx
// FileContextChip (`openFile`), ChatComposer.tsx's `previewFileId` Dialog and files/AttachmentFilePreview.tsx with
// origin "Draft", onRemove and onClose, over the draft's own bytes.
import { memoryFiles, text } from '../support/app-files-fixture';
import { STAGED } from '../../src/composer/composer-staged-copies';
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { performChipAct } from '../../src/composer/composer-chip-details';
import { composerFileLocal, composerFileView, pastedPreview } from '../../src/composer/composer-chip-file';
import { observeComposer, acceptComposerEdit } from '../support/composer-field-fixture';
import { setDraftFiles, foldPaste, type DraftFile } from '../../src/composer/composer-editor-files';
import { attachmentView } from '../../src/panel/attachment-surface';
import { lastSave, memoryAppFiles, serveAssets } from '../support/asset-server';
import { forgetMediaFrames } from '../../src/media/media-frame';

let server: ReturnType<typeof serveAssets> | null = null;
afterEach(() => { server?.restore(); server = null; });

const ID = '11111111-2222-3333-4444-555555555555';
const file = (patch: Partial<DraftFile>): DraftFile => ({ id: ID, contextId: `file_${ID}`, draftKey: 'env:t1', environmentId: 'env', name: 'notes.js', mimeType: 'text/javascript',
  sizeBytes: 40, source: 'attached', attachmentId: '', status: 'staged', ...patch });

function harness(files: DraftFile[], prompt = `see [notes.js](t3-context://v1/file/file_${ID}) now`) {
  const calls: Obj[] = [];
  const reply = (_request: Obj): Obj => ({ ok: true, generation: 0, value: {} });
  // The staged copy under the app's data (composer-attach-stage.ts writes it), and the server's asset once sent.
  const appFiles = memoryAppFiles(new Map([[`app:/data/composer-files/${ID}`, new TextEncoder().encode('const a = 1;\nexport { a };')]]));
  server?.restore();
  server = serveAssets(() => ({ body: 'const a = 1;\nexport { a };' }));
  const native = { available: true, watch() {}, later: async (request: Obj) => {
    calls.push(request);
    if (request.op === 'editorEdit' && request.all === true) {
      expect(client.local.drafts['env:t1']).toBe(request.expect as string);
      client.local.drafts['env:t1'] = request.text as string; observeComposer(client, String(request.text), true);
      return { ok: true, generation: 0, value: { applied: true } };
    }
    if (request.op === 'editorEdit') return { ok: true, generation: 0, value: { applied: acceptComposerEdit(client, request) } };
    return { ok: true, generation: 0, value: {} };
  } } as unknown as Native;
  const client = { persist: async () => {}, draftKey: 'env:t1', environmentId: 'env', snapshotOwner: 'env:t1', generation: 1, projection: { turnItems: [], runtimeRequests: [] }, ready: true, origin: 'http://127.0.0.1:16600', local: { drafts: { 'env:t1': prompt }, clientSettings: {} },
    rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ op: method, ...payload }); return { relativeUrl: '/api/assets/token/notes.js' }; },
    restAccess: () => ({ call: async (request: Obj) => { calls.push(request); return reply(request); } }) } as unknown as T3Client;
  setDraftFiles(client.local, files); observeComposer(client, prompt);
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

  test('a PDF opens in the viewer page, its staged copy posted to it; another type is "No preview" with its actions', async () => {
    forgetMediaFrames(); // another file's test may have shown this staged name
    const { native, client, appFiles } = harness([file({ name: 'spec.pdf', mimeType: 'application/pdf' })]);
    await performChipAct(client, native, { act: 'file', context: `file_${ID}` });
    const view = (await composerFileView(client, native, 1)).file;
    expect(view).toMatchObject({ preview: 'viewer', canSave: true, canCopy: false, canRemove: true });
    expect(JSON.parse(view.viewer.config)).toMatchObject({ view: 'pdf', src: '' });
    // Once the page has booted, the staged copy's bytes go to it by message (the reference's Blob URL).
    await composerFileLocal(client, native, 'surface-linked-att-viewer:{"type":"boot"}', ID);
    const posted = JSON.parse((await composerFileView(client, native, 1)).file.viewer.posts[0]!.text) as Obj;
    expect([posted.type, posted.done, Buffer.from(String(posted.data), 'base64')]).toEqual(['bytes', true, Buffer.from(appFiles.get(`app:/data/composer-files/${ID}`)!)]);
    // Nothing is uploaded or fetched to preview it.
    expect(server!.calls).toEqual([]);
    const other = harness([file({ name: 'build.bin', mimeType: 'application/octet-stream' })]);
    await performChipAct(other.client, other.native, { act: 'file', context: `file_${ID}` });
    expect((await composerFileView(other.client, other.native, 1)).file).toMatchObject({ preview: 'none', noPreview: 'Save it to open in an app that supports bin files.', canSave: true });
  });

  test('a folded paste is read from memory; a file the send uploaded is the server\'s', async () => {
    const { calls, native, client } = harness([]);
    const fold = harness([]);
    const current = observeComposer(fold.client, '');
    await foldPaste(fold.client, fold.native, 'fold', { id: 1, snapshotOwner: current.snapshotOwner, fieldOwner: current.owner, value: '', text: 'pasted words', start: 0, end: 0 }, ID);
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
    const { native, client } = harness([file({})]);
    const { released } = memoryFiles({ [`${STAGED}/${ID}`]: text('notes') });
    await performChipAct(client, native, { act: 'file', context: `file_${ID}` });
    await composerFileLocal(client, native, 'surface-linked-att-remove', ID);
    expect(client.local.drafts['env:t1']).toBe('see now');
    expect(client.local.composerFiles).toEqual([]);
    expect(released()).toEqual([ID.toLowerCase()]);
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
