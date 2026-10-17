// The editor's sync takes the files a paste held (ChatComposer.tsx onComposerPaste → addComposerAttachments) and prunes
// emptied drafts' files without dropping one attached while that sync was in flight. Run through composerEditorView
// over the client fixture, with the native editor's replies stubbed (T3ComposerEditor.swift answers them on macOS).
import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { Backend, storage } from './client-fixture';
import { composerEditorView } from './composer-editor';
import { acceptAttachedFile } from './composer-editor-attach';
import { draftFiles } from './composer-editor-files';
import { obj, str, type Obj } from './domain';

const uuid = (n: number) => `1b4e28ba-2fa1-11d2-883f-0016d3cca42${n}`;

/** The client fixture's server, with the native editor's sync, paste take and insert answered here. */
class EditorBackend extends Backend {
  state: Obj = { empty: false };
  pasted: Obj[] = [];
  /** Runs as the native editor reads its state for a sync (before the reply reaches TypeScript). */
  onSync: () => void = () => {};
  onTake: () => void = () => {};
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'editorSync') { this.calls.push(request); this.onSync(); return this.good({ owner: '', ...this.state, pastes: this.pasted.length }); }
    if (request.op === 'composerPasteTake') { this.calls.push(request); this.onTake(); const files = this.pasted; this.pasted = []; return this.good({ files }); }
    if (request.op === 'editorInsert') { this.calls.push(request); return this.good({ applied: true }); }
    if (request.op === 'composerAttachRemove') { this.calls.push(request); return this.good({ removed: true }); }
    return super.later(input);
  }
}

async function opened() {
  const client = new T3Client(), native = new EditorBackend(), disk = storage();
  await client.refresh(native, disk.files);
  await client.command('select-thread', 't1', '', 0, native, disk.files);
  await client.refresh(native, disk.files);
  return { client, native, disk };
}
const ops = (native: EditorBackend, op: string) => native.calls.filter(call => call.op === op);

describe('a paste with files on the pasteboard (onComposerPaste)', () => {
  test('the sync takes the held files: a plain file becomes a chip at the caret, an image joins the shelf', async () => {
    const { client, native } = await opened();
    native.pasted = [
      { kind: 'file', id: uuid(1), name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 8 },
      { kind: 'image', id: uuid(2), name: 'image.png', sizeBytes: 120 },
    ];
    await composerEditorView(client, native);
    // Staged as Attach stages a picked file: within the server's staging limit (unknown capabilities: 50 MB).
    expect(ops(native, 'composerPasteTake')).toMatchObject([{ fileLimit: 50 * 1024 * 1024 }]);
    // The prompt has no prose, so the image lives on the shelf with no chip (imageAttachmentsGetChips).
    expect(ops(native, 'editorInsert').map(call => call.text)).toEqual([`[notes.md](t3-context://v1/file/file_${uuid(1)})`]);
    expect(draftFiles(client.local).map(file => [file.name, file.draftKey, file.status])).toEqual([['notes.md', client.draftKey, 'staged']]);
    expect((client.local.snapshotDrafts[client.draftKey] ?? []).map(image => str(image.name))).toEqual(['image.png']);
    expect(client.error).toBe('');
    // Nothing is held any more: the next sync asks for none.
    await composerEditorView(client, native);
    expect(ops(native, 'composerPasteTake')).toHaveLength(1);
  });

  test('Paste as Text attaches the files too, and its image gets no chip even beside prose (skipImageInlineChip)', async () => {
    const { client, native } = await opened();
    client.local.drafts[client.draftKey] = 'look at this';
    native.pasted = [
      { kind: 'file', id: uuid(1), name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 8, asText: true },
      { kind: 'image', id: uuid(2), name: 'image.png', sizeBytes: 120, asText: true },
    ];
    await composerEditorView(client, native);
    expect(ops(native, 'editorInsert').map(call => call.text)).toEqual([`[notes.md](t3-context://v1/file/file_${uuid(1)})`]);
    expect((client.local.snapshotDrafts[client.draftKey] ?? []).map(image => str(image.name))).toEqual(['image.png']);
    // ⌘V of the same image beside prose gives it its chip.
    native.pasted = [{ kind: 'image', id: uuid(3), name: 'image.png', sizeBytes: 120 }];
    await composerEditorView(client, native);
    expect(ops(native, 'editorInsert').map(call => call.text).at(-1)).toContain(uuid(3));
  });

  test('what cannot be attached is named, as addComposerAttachments reports it', async () => {
    const { client, native } = await opened();
    native.pasted = [{ kind: 'unsupported-image', name: 'diagram.svg', sizeBytes: 40 }];
    await composerEditorView(client, native);
    expect(client.error).toBe("'diagram.svg' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.");
    expect(ops(native, 'editorInsert')).toEqual([]);
  });

  test('a file pasted into an empty draft keeps its record through the sync that read the editor as empty', async () => {
    const { client, native } = await opened();
    native.state = { empty: true };
    native.pasted = [{ kind: 'file', id: uuid(7), name: 'sample.py', mimeType: 'text/x-python', sizeBytes: 67 }];
    await composerEditorView(client, native);
    expect(draftFiles(client.local).map(file => file.name)).toEqual(['sample.py']);
    expect(ops(native, 'composerAttachRemove')).toEqual([]);
  });

  test('without a project the files are let go and the reason is shown, as Attach files says it', async () => {
    const { client, native } = await opened();
    client.projectId = '';
    native.pasted = [{ kind: 'file', id: uuid(8), name: 'a.txt', sizeBytes: 3 }];
    await composerEditorView(client, native);
    expect(ops(native, 'composerAttachRemove')).toMatchObject([{ id: uuid(8) }]);
    expect(client.error).toBe('Choose a project first.');
  });

  test('files taken after the draft changed are let go', async () => {
    const { client, native } = await opened();
    native.pasted = [{ kind: 'file', id: uuid(3), name: 'a.txt', sizeBytes: 3 }, { kind: 'image', id: uuid(4), name: 'image.png', sizeBytes: 9 }];
    native.onTake = () => { client.threadId = 't2'; };
    await composerEditorView(client, native);
    expect(ops(native, 'composerAttachRemove')).toMatchObject([{ id: uuid(3) }]);
    expect(client.local.snapshotReleases).toContain(uuid(4));
    expect(draftFiles(client.local)).toEqual([]);
  });
});

describe('an emptied draft\'s files', () => {
  test('a file attached while the sync was in flight survives that sync\'s empty editor; one from before goes', async () => {
    const { client, native } = await opened();
    const old = acceptAttachedFile(client, { kind: 'file', id: uuid(5), name: 'old.txt', sizeBytes: 3 }).file!;
    // The editor reads as empty when asked; Attach files' pick lands (its chip on its way into the editor) before the
    // reply comes back, as the panel's completion did on the first file attached to an empty draft.
    native.state = { empty: true };
    let added = null as ReturnType<typeof acceptAttachedFile>['file'];
    native.onSync = () => { added = acceptAttachedFile(client, { kind: 'file', id: uuid(6), name: 'sample.py', mimeType: 'text/x-python', sizeBytes: 67 }).file; };
    await composerEditorView(client, native);
    expect(draftFiles(client.local)).toEqual([added!]);
    expect(ops(native, 'composerAttachRemove')).toMatchObject([{ id: old.id }]);
    // The next sync sees the chip in the editor's text, so the file keeps its record (and its chip its name and size).
    native.state = { empty: false };
    native.onSync = () => {};
    await composerEditorView(client, native);
    expect(draftFiles(client.local)).toEqual([added!]);
  });
});
