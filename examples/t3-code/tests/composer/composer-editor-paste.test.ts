// A drop, a paste and Attach files (ChatComposer.tsx addDroppedFiles, onComposerPaste, the hidden file input → addComposerAttachments)
// stage their files in TypeScript (composer-attach-stage.ts) over the app's storage, held in memory here; the editor's sync
// prunes emptied drafts' files without dropping one attached while that sync was in flight. Run over the client fixture,
// with the native editor's replies stubbed (T3ComposerEditor.swift and T3ComposerAttach.swift answer them on macOS).
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { Backend, storage } from '../support/client-fixture';
import { composerEditorView } from '../../src/composer/composer-editor';
import { acceptAttachedFile, imageChipLink } from '../../src/composer/composer-editor-attach';
import { draftFiles, fileChipContexts } from '../../src/composer/composer-editor-files';
import { obj, str, type Obj } from '../../src/client/domain';
import { acceptComposerEdit, observeComposer } from '../support/composer-field-fixture';
import { fileChipLink } from '../../src/composer/composer-editor-files';
import { toasts } from '../../src/shell/toast';
import { addAttachedFiles, attachIncoming } from '../../src/composer/composer-controls-attach';
import { DRAFTS, STAGED, classifyAttachment, handleName, mimeTypeOfName } from '../../src/composer/composer-attach-stage';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerFieldAcknowledged, composerFieldNative, composerFieldResource } from '../../src/composer/composer-field-bridge';
import { primary, type PrimaryTarget } from '../../src/connections/local-primary';
import { gif, jpeg, memoryFiles, png, text, webp } from '../support/app-files-fixture';

const uuid = (n: number) => `1b4e28ba-2fa1-11d2-883f-0016d3cca42${n}`;

/** The client fixture's server, with the native editor's sync and insert, and the module's staged-copy ops, answered here. */
class EditorBackend extends Backend {
  client: T3Client | null = null;
  state: Obj = { empty: false };
  /** Runs as the native editor reads its state for a sync (before the reply reaches TypeScript). */
  onSync: () => void | Promise<void> = () => {};
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'editorSync') { this.calls.push(request); await this.onSync(); return this.good({ snapshotOwner: request.snapshotOwner, fieldOwner: request.fieldOwner, ...this.state }); }
    if (request.op === 'editorInsert' || request.op === 'editorEdit') { this.calls.push(request); return this.good({ applied: this.client && acceptComposerEdit(this.client, request) }); }
    if (request.op === 'composerAttachMedia') { this.calls.push(request); return this.good(request.video ? { videoWidth: 640, videoHeight: 360 } : {}); }
    if (request.op === 'editorForgetFold') { this.calls.push(request); return this.good({}); }
    return super.later(input);
  }
}

async function opened() {
  const client = new T3Client(), native = new EditorBackend(), disk = storage();
  await client.refresh(native, disk.files);
  await client.command('select-thread', 't1', '', 0, native, disk.files);
  await client.refresh(native, disk.files);
  native.client = client;
  observeComposer(client, client.draft);
  return { client, native, disk };
}
const ops = (native: EditorBackend, op: string) => native.calls.filter(call => call.op === op);
const under = (store: Map<string, Uint8Array>, folder: string) => [...store.keys()].filter(path => path.startsWith(`${folder}/`));
const incoming = (files: string[], extra: Partial<{ picked: Obj[]; asText: boolean }> = {}) => ({ files, picked: [], asText: false, target: '', ...extra });
afterEach(() => { primary.target = null; });

describe('what a dropped or pasted file is (classifyComposerAttachmentFile)', () => {
  test('a handle\'s name and type come from its path; images by type, else by name', () => {
    expect(handleName('doc:/12/my notes.txt')).toBe('my notes.txt');
    expect(mimeTypeOfName('README.MD')).toBe('text/markdown');
    expect(mimeTypeOfName('main.rs')).toBe('');
    expect(classifyAttachment('photo.jpg', mimeTypeOfName('photo.jpg'))).toBe('image');
    expect(classifyAttachment('shot.heic', '')).toBe('image');
    expect(classifyAttachment('picture', 'image/png')).toBe('image');
    expect(classifyAttachment('scan.tiff', mimeTypeOfName('scan.tiff'))).toBe('unsupported-image');
    expect(classifyAttachment('notes.txt', 'text/plain')).toBe('file');
  });
});

describe('a drop, a paste and Attach files (addComposerAttachments)', () => {
  test('a dropped text file becomes a chip with its bytes staged; a PNG within the limits joins the shelf as it is', async () => {
    const { client, native } = await opened();
    const shot = png(4, 3, 64);
    const { store, compressed } = memoryFiles({ 'doc:/1/notes.txt': text('twenty-six bytes of notes\n'), 'doc:/2/small.png': shot });
    expect(await attachIncoming(client, native, 'drop', incoming(['doc:/1/notes.txt', 'doc:/2/small.png']))).toBe('focus:composer');
    const [copy] = under(store, STAGED);
    const file = draftFiles(client.local)[0]!;
    expect(copy).toBe(`${STAGED}/${file.id}`);
    expect(new TextDecoder().decode(store.get(copy!))).toBe('twenty-six bytes of notes\n');
    expect(file).toMatchObject({ name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 26, status: 'staged' });
    // The prompt has no prose, so the image lives on the shelf with no chip (imageAttachmentsGetChips).
    expect(ops(native, 'editorInsert').map(call => call.text)).toEqual([fileChipLink(file)]);
    const [image] = client.local.snapshotDrafts[client.draftKey] ?? [];
    expect(image).toMatchObject({ name: 'small.png', mimeType: 'image/png', sizeBytes: shot.byteLength });
    expect(store.get(`${DRAFTS}/${str(image!.id)}.png`)).toEqual(shot);
    expect(compressed).toEqual([]);
    expect(client.error).toBe('');
  });

  test('an image within its limits keeps its bytes and type; one over them is fitted or re-encoded; the scratch copy goes', async () => {
    const { client, native } = await opened();
    const photo = jpeg(400, 300), anim = gif(30, 20);
    const { store, compressed } = memoryFiles({ 'doc:/1/wide.png': png(3000, 10), 'app:/tmp/picked/1/photo.jpg': photo, 'doc:/3/anim.gif': anim,
      'doc:/4/big.jpg': jpeg(4000, 3000), 'doc:/5/huge.gif': gif(30, 20, 11 * 1024 * 1024), 'doc:/6/shot.heic': text('....ftypheic'),
      'doc:/7/photo.webp': webp(3000, 2000, false, 11 * 1024 * 1024) });
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/wide.png', 'doc:/3/anim.gif']));
    await attachIncoming(client, native, 'pick', incoming([], { picked: [{ path: 'app:/tmp/picked/1/photo.jpg', name: 'photo.jpg', type: 'image/jpeg', size: photo.byteLength }] }));
    await attachIncoming(client, native, 'drop', incoming(['doc:/4/big.jpg', 'doc:/5/huge.gif', 'doc:/6/shot.heic', 'doc:/7/photo.webp']));
    // compressImage reads app files: a dropped image is copied to the scratch folder first.
    expect(compressed.map(call => [String(call.from).replace(/[0-9a-f-]{36}/, 'id'), call.maxDimension, call.maxBytes, call.type])).toEqual([
      ['app:/tmp/composer-stage/id/wide.png', 2000, 10 * 1024 * 1024, 'image/png'],
      ['app:/tmp/composer-stage/id/big.jpg', 2000, 10 * 1024 * 1024, 'image/jpeg'],
      // Every re-encoding is within 2000 px, as reencodeWithinBudget caps it; an opaque WebP becomes a JPEG.
      ['app:/tmp/composer-stage/id/huge.gif', 2000, 10 * 1024 * 1024, 'image/png'],
      ['app:/tmp/composer-stage/id/shot.heic', 2000, 10 * 1024 * 1024, 'image/jpeg'],
      ['app:/tmp/composer-stage/id/photo.webp', 2000, 10 * 1024 * 1024, 'image/jpeg'],
    ]);
    expect(under(store, 'app:/tmp/composer-stage')).toEqual([]);
    const drafts = client.local.snapshotDrafts[client.draftKey] ?? [];
    expect(drafts.map(image => [image.name, image.mimeType, image.downscalable])).toEqual([['wide.png', 'image/png', undefined], ['anim.gif', 'image/gif', false],
      ['photo.jpg', 'image/jpeg', undefined], ['big.jpg', 'image/jpeg', undefined], ['huge.png', 'image/png', undefined], ['shot.jpg', 'image/jpeg', undefined],
      ['photo.jpg', 'image/jpeg', undefined]]);
    expect(store.get(`${DRAFTS}/${str(drafts[1]!.id)}.png`)).toEqual(anim);
    expect(store.get(`${DRAFTS}/${str(drafts[2]!.id)}.png`)).toEqual(photo);
  });

  test('a file past the 100-file limit is not staged and is named', async () => {
    const { client, native } = await opened();
    const entries: Record<string, Uint8Array> = {};
    for (let index = 0; index < 101; index++) entries[`doc:/${index + 1}/f${index}.txt`] = text('x');
    const { store } = memoryFiles(entries);
    await attachIncoming(client, native, 'drop', incoming(Object.keys(entries)));
    expect(under(store, STAGED)).toHaveLength(100);
    expect(client.error).toBe('You can attach up to 100 files per message.');
  });

  test('what cannot be attached is named, as addComposerAttachments reports it', async () => {
    const { client, native } = await opened();
    memoryFiles({ 'doc:/1/scan.tiff': text('II*'), 'doc:/2/broken.png': text('X'), 'doc:/3/empty.txt': new Uint8Array() });
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/scan.tiff']));
    expect(client.error).toBe("'scan.tiff' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.");
    await attachIncoming(client, native, 'drop', incoming(['doc:/2/broken.png']));
    expect(client.error).toBe("'broken.png' could not be read as an image.");
    await attachIncoming(client, native, 'paste', incoming(['doc:/3/empty.txt']));
    expect(client.error).toBe("'empty.txt' is empty or could not be read.");
    expect(ops(native, 'editorInsert')).toEqual([]);
  });

  test('a dropped or pasted folder has no path to mention: the reference\'s refusal for it, or for a remote environment', async () => {
    const { client, native } = await opened();
    memoryFiles({ 'doc:/1/src': 'folder', 'doc:/2/notes.txt': text('notes') });
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/src']));
    expect(toasts(client).map(toast => toast.title)).toEqual(['Folders can only be referenced in the local desktop environment']);
    primary.target = { environmentId: client.environmentId } as PrimaryTarget;
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/src', 'doc:/2/notes.txt']));
    expect(toasts(client).map(toast => [toast.title, toast.description]).at(-1)).toEqual(['Couldn\'t get the path of "src"', 'Type the folder path with @ instead.']);
    expect(draftFiles(client.local).map(file => file.name)).toEqual(['notes.txt']);
    // A pasted folder is a workspace reference too (addComposerFolders), not an empty file.
    client.error = '';
    await attachIncoming(client, native, 'paste', incoming(['doc:/1/src']));
    expect(toasts(client).map(toast => toast.title).at(-1)).toBe('Couldn\'t get the path of "src"');
    expect(client.error).toBe('');
  });

  test('Paste as Text attaches the files too, and its image gets no chip even beside prose (skipImageInlineChip)', async () => {
    const { client, native } = await opened();
    client.local.drafts[client.draftKey] = 'look at this';
    observeComposer(client, client.draft, true);
    memoryFiles({ 'doc:/1/notes.md': text('# Notes\n'), 'doc:/2/image.png': png(4, 3) });
    expect(await attachIncoming(client, native, 'paste', incoming(['doc:/1/notes.md', 'doc:/2/image.png'], { asText: true }))).toBe('');
    const notes = draftFiles(client.local)[0]!;
    expect(ops(native, 'editorInsert').map(call => call.text)).toEqual([fileChipLink(notes)]);
    expect((client.local.snapshotDrafts[client.draftKey] ?? []).map(image => str(image.name))).toEqual(['image.png']);
    // ⌘V of the same image beside prose gives it its chip.
    await attachIncoming(client, native, 'paste', incoming(['doc:/2/image.png']));
    const second = (client.local.snapshotDrafts[client.draftKey] ?? [])[1]!;
    expect(ops(native, 'editorInsert').map(call => call.text).at(-1)).toContain(str(second.id));
  });

  test('a video is staged with its typed link and first frame from the module', async () => {
    const { client, native } = await opened();
    memoryFiles({ 'doc:/1/clip.mov': text('moov') });
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/clip.mov']));
    const clip = draftFiles(client.local)[0]!;
    expect(clip).toMatchObject({ name: 'clip.mov', mimeType: 'video/quicktime' });
    expect(ops(native, 'composerAttachMedia')).toMatchObject([{ id: clip.id, ext: 'mov', video: true }]);
  });

  test('a file pasted into an empty draft keeps its record through the sync that read the editor as empty', async () => {
    const { client, native } = await opened();
    native.state = { empty: true };
    const { released } = memoryFiles({ 'doc:/1/sample.py': text('print("sample")\n') });
    await attachIncoming(client, native, 'paste', incoming(['doc:/1/sample.py']));
    await composerEditorView(client, native);
    expect(draftFiles(client.local).map(file => file.name)).toEqual(['sample.py']);
    expect(released()).toEqual([]);
  });

  test('without a project nothing is staged and the reason is shown, as Attach files says it', async () => {
    const { client, native } = await opened();
    const { store } = memoryFiles({ 'doc:/1/a.txt': text('abc') });
    client.projectId = '';
    await expect(attachIncoming(client, native, 'paste', incoming(['doc:/1/a.txt']))).rejects.toThrow('Choose a project first.');
    expect(under(store, STAGED)).toEqual([]);
  });

  test('files staged after the person moved to another draft stay with the draft they were given to', async () => {
    const { client, native } = await opened();
    client.local.drafts[client.draftKey] = 'look at this';
    observeComposer(client, client.draft, true);
    const { fs, released } = memoryFiles({ 'doc:/1/a.txt': text('abc'), 'doc:/2/image.png': png(4, 3) });
    const stat = fs.stat, first = client.draftKey;
    fs.stat = async (path: string) => { if (path === 'doc:/2/image.png') client.threadId = 't2'; return stat(path); };
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/a.txt', 'doc:/2/image.png']));
    expect(client.draftKey).not.toBe(first);
    expect(released()).toEqual([]);
    expect(client.local.snapshotReleases).toEqual([]);
    const [file] = draftFiles(client.local), [image] = client.local.snapshotDrafts[first] ?? [];
    expect(file).toMatchObject({ name: 'a.txt', draftKey: first });
    expect(image).toMatchObject({ name: 'image.png' });
    // Their chips join the end of that draft's stored prompt (insertAttachmentReferencesForDraft); the open draft gets none.
    expect(client.local.drafts[first]).toBe(`look at this ${fileChipLink(file!)} ${imageChipLink(image!)} `);
    expect(client.local.drafts[client.draftKey] ?? '').toBe('');
    expect(ops(native, 'editorInsert')).toEqual([]);
  });
});

describe('an emptied draft\'s files', () => {
  test.each([{ label: 'file only', withImage: false }, { label: 'file and image', withImage: true }])('keeps staged files through queued insertion and an older empty sync: $label', async ({ withImage }) => {
    const { client, native } = await opened();
    const { released } = memoryFiles({ [`${STAGED}/${uuid(1)}`]: text('twenty-six bytes of notes\n') });
    const owner = composerFieldOwner(client), empty = observeComposer(client, '');
    const fieldNative = composerFieldNative(client, native, () => composerFieldOwner(client));
    const files = [{ kind: 'file', id: uuid(1), name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 26 },
      ...(withImage ? [{ kind: 'image', id: uuid(2), name: 'image.png', sizeBytes: 120 }] : [])];
    await addAttachedFiles(client, fieldNative, files);
    const link = `[notes.txt](t3-context://v1/file/file_${uuid(1)})`;
    expect(client.draft).toBe('');
    await composerEditorView(client, fieldNative);
    expect(draftFiles(client.local)).toMatchObject([{ id: uuid(1), status: 'staged' }]);
    expect(fileChipContexts(client, link)).toEqual({ [`file/file_${uuid(1)}`]: 'file\t1 KB' });
    expect(released()).toEqual([]);

    const read = Promise.withResolvers<void>(), resumed = Promise.withResolvers<void>();
    native.onSync = () => { read.resolve(); return resumed.promise; };
    const syncing = composerEditorView(client, fieldNative);
    await read.promise;
    const edit = composerFieldResource(client, [empty, 0], owner);
    expect(edit.ready).toBe(true);
    const inserted = observeComposer(client, `${link} `, true);
    expect((await composerFieldAcknowledged(client, [inserted, edit.seq, true], owner, native)).applied).toBe(true);
    // The field's input and ACK can arrive before the separate draft persistence command.
    expect(client.draft).toBe('');
    resumed.resolve();
    await syncing;
    expect(draftFiles(client.local)).toHaveLength(1);
    expect(client.snapshotDrafts).toHaveLength(withImage ? 1 : 0);
    expect(released()).toEqual([]);

    native.onSync = () => {};
    observeComposer(client, '', true);
    await composerEditorView(client, fieldNative);
    expect(draftFiles(client.local)).toEqual([]);
    expect(released()).toEqual([uuid(1)]);
  });
  test('a refused queued insertion releases files when its original empty draft is abandoned', async () => {
    const { client, native } = await opened();
    const { released } = memoryFiles({ [`${STAGED}/${uuid(1)}`]: text('notes') });
    const fieldNative = composerFieldNative(client, native, () => composerFieldOwner(client));
    await addAttachedFiles(client, fieldNative, [{ kind: 'file', id: uuid(1), name: 'notes.txt', sizeBytes: 26 }]);
    client.threadId = 't2';
    observeComposer(client, '');
    await composerEditorView(client, composerFieldNative(client, native, () => composerFieldOwner(client)));
    expect(draftFiles(client.local)).toEqual([]);
    expect(released()).toEqual([uuid(1)]);
  });
  test('a file attached while the sync was in flight survives that sync\'s empty editor; one from before goes', async () => {
    const { client, native } = await opened();
    const { released } = memoryFiles({ [`${STAGED}/${uuid(5)}`]: text('old'), [`${STAGED}/${uuid(6)}`]: text('sample') });
    const old = acceptAttachedFile(client, { kind: 'file', id: uuid(5), name: 'old.txt', sizeBytes: 3 }).file!;
    // The editor reads as empty when asked; Attach files' pick lands (its chip on its way into the editor) before the
    // reply comes back, as the panel's completion did on the first file attached to an empty draft.
    native.state = { empty: true };
    let added = null as ReturnType<typeof acceptAttachedFile>['file'];
    native.onSync = () => { added = acceptAttachedFile(client, { kind: 'file', id: uuid(6), name: 'sample.py', mimeType: 'text/x-python', sizeBytes: 67 }).file; };
    await composerEditorView(client, native);
    expect(draftFiles(client.local)).toEqual([added!]);
    expect(released()).toEqual([old.id]);
    // The next sync sees the chip in the editor's text, so the file keeps its record (and its chip its name and size).
    native.state = { empty: false };
    observeComposer(client, fileChipLink(added!) + ' ', true);
    native.onSync = () => {};
    await composerEditorView(client, native);
    expect(draftFiles(client.local)).toEqual([added!]);
  });
});
