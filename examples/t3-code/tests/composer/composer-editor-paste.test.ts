// A drop, a paste and Attach files (ChatComposer.tsx addDroppedFiles, onComposerPaste, the hidden file input → addComposerAttachments)
// stage their files in TypeScript (composer-attach-stage.ts) over the app's storage, held in memory here; the editor view
// prunes emptied drafts' files without dropping one attached while the view was in flight. Run over the client fixture,
// with the shared field's edits answered here.
import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { Backend, storage } from '../support/client-fixture';
import { composerEditorView } from '../../src/composer/composer-editor';
import { acceptAttachedFile, imageChipLink } from '../../src/composer/composer-editor-attach';
import { draftFiles, fileChipContexts, foldPaste } from '../../src/composer/composer-editor-files';
import { obj, str, type Obj } from '../../src/client/domain';
import { acceptComposerEdit, observeComposer } from '../support/composer-field-fixture';
import { fileChipLink } from '../../src/composer/composer-editor-files';
import { toasts } from '../../src/shell/toast';
import { addAttachedFiles, attachIncoming, cancelPreparingImage, preparingImages } from '../../src/composer/composer-controls-attach';
import { DRAFTS, STAGED, classifyAttachment, fileAttachmentMimeType, handleName, mimeTypeOfName, previewSource } from '../../src/composer/composer-attach-stage';
import { contractComponent } from '../support/contract-action-fixture';
import { composerFieldOwner } from '../../src/composer/auto-balance-owner';
import { composerFieldAcknowledged, composerFieldNative, composerFieldResource } from '../../src/composer/composer-field-bridge';
import { primary, type PrimaryTarget } from '../../src/connections/local-primary';
import { gif, jpeg, memoryFiles, png, text, webp } from '../support/app-files-fixture';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';

const uuid = (n: number) => `1b4e28ba-2fa1-11d2-883f-0016d3cca42${n}`;

/** The client fixture's server, with the shared field's insert and edit answered here. */
class EditorBackend extends Backend {
  client: T3Client | null = null;
  /** Edit › Paste as Text chosen with the pointer (T3Menus.swift), until a paste reads it. */
  menuArmed = false;
  /** Runs while the editor view awaits its one read, the file metadata (before the reply reaches TypeScript). */
  onSync: () => void | Promise<void> = () => {};
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'editorInsert' || request.op === 'editorEdit') { this.calls.push(request); return this.good({ applied: this.client && acceptComposerEdit(this.client, request) }); }
    if (request.op === 'composerPasteAsText') { const armed = this.menuArmed; this.menuArmed = false; return this.good({ armed }); }
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
  // Each editor view reads one new path's metadata (a message mentions it), where native.onSync runs.
  if (!client.scopes.includes('filesystem:read')) client.scopes = [...client.scopes, 'filesystem:read'];
  let reads = 0;
  client.restAccess = (() => ({ request: async (_method: string, payload: { paths: string[] }) => { await native.onSync(); return { entries: payload.paths.map(() => null) }; } })) as unknown as T3Client['restAccess'];
  const view = client.projection as Obj;
  const mention = () => { view.messages = [{ id: 'm-read', role: 'user', text: `@/tmp/read-${++reads}.ts` }]; };
  return { client, native, disk, view: (target: Native) => { mention(); return composerEditorView(client, target); } };
}
const ops = (native: EditorBackend, op: string) => native.calls.filter(call => call.op === op);
const under = (store: Map<string, Uint8Array>, folder: string) => [...store.keys()].filter(path => path.startsWith(`${folder}/`));
/** The staging's own compressions, without a preparing tile's thumbnail (composer-attach-stage.ts previewSource). */
const staging = (compressed: Record<string, unknown>[]) => compressed.filter(call => !String(call.to).includes('/composer-stage/preview-'));
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
    // An image the viewer shows but providers refuse (TIFF), a raw DNG and an SVG attach as files.
    expect(classifyAttachment('scan.tiff', mimeTypeOfName('scan.tiff'))).toBe('file');
    expect(classifyAttachment('raw.dng', '')).toBe('file');
    expect(classifyAttachment('vector.svg', 'application/octet-stream')).toBe('file');
    expect(fileAttachmentMimeType('raw.dng', '')).toBe('image/x-adobe-dng');
    expect(classifyAttachment('layers.psd', mimeTypeOfName('layers.psd'))).toBe('unsupported-image');
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
    expect(staging(compressed)).toEqual([]);
    expect(client.error).toBe('');
  });

  test('an image within its limits keeps its bytes and type; one over them is fitted or re-encoded; the scratch copy goes', async () => {
    const { client, native } = await opened();
    const photo = jpeg(400, 300), anim = gif(30, 20);
    const { store, compressed } = memoryFiles({ 'doc:/1/wide.png': png(3000, 10), 'app:/tmp/picked/1/photo.jpg': photo, 'doc:/3/anim.gif': anim,
      'doc:/4/big.jpg': jpeg(4000, 3000), 'doc:/5/huge.gif': gif(30, 20, 10.5 * 1024 * 1024), 'doc:/6/shot.heic': text('....ftypheic'),
      'doc:/7/photo.webp': webp(3000, 2000, false, 10.5 * 1024 * 1024) });
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/wide.png', 'doc:/3/anim.gif']));
    await attachIncoming(client, native, 'pick', incoming([], { picked: [{ path: 'app:/tmp/picked/1/photo.jpg', name: 'photo.jpg', type: 'image/jpeg', size: photo.byteLength }] }));
    await attachIncoming(client, native, 'drop', incoming(['doc:/4/big.jpg', 'doc:/5/huge.gif', 'doc:/6/shot.heic', 'doc:/7/photo.webp']));
    // compressImage reads app files: a dropped image is copied to the scratch folder first.
    expect(staging(compressed).map(call => [String(call.from).replace(/[0-9a-f-]{36}/, 'id'), call.maxDimension, call.maxBytes, call.type])).toEqual([
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

  test('a large dropped or picked file is copied by the host up to the server\'s limit; a large image is re-encoded from its file', async () => {
    const { client, native } = await opened();
    const big = new Uint8Array(13 * 1024 * 1024), huge = png(5000, 4000, 12 * 1024 * 1024);
    const { store, compressed } = memoryFiles({ 'doc:/1/big.bin': big, 'app:/tmp/picked/1/big.bin': big, 'doc:/2/huge.png': huge, 'app:/tmp/picked/2/huge.png': huge });
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/big.bin', 'doc:/2/huge.png']));
    await attachIncoming(client, native, 'pick', incoming([], { picked: [
      { path: 'app:/tmp/picked/1/big.bin', name: 'big.bin', type: 'application/octet-stream', size: big.byteLength },
      { path: 'app:/tmp/picked/2/huge.png', name: 'huge.png', type: 'image/png', size: huge.byteLength }] }));
    expect(draftFiles(client.local)).toMatchObject([{ name: 'big.bin', sizeBytes: big.byteLength, status: 'staged' }, { name: 'big.bin', sizeBytes: big.byteLength, status: 'staged' }]);
    expect(under(store, STAGED).map(path => store.get(path)!.byteLength)).toEqual([big.byteLength, big.byteLength]);
    // Their headers are never read: the dropped image is re-encoded from its scratch copy, the picked one from its own file.
    expect(staging(compressed).map(call => [String(call.from).replace(/[0-9a-f-]{36}/, 'id'), call.maxDimension, call.type])).toEqual([
      ['app:/tmp/composer-stage/id/huge.png', 2000, 'image/png'], ['app:/tmp/picked/2/huge.png', 2000, 'image/png']]);
    expect(under(store, 'app:/tmp/composer-stage')).toEqual([]);
    expect(client.local.snapshotDrafts[client.draftKey]).toMatchObject([{ name: 'huge.png', mimeType: 'image/png' }, { name: 'huge.png', mimeType: 'image/png' }]);
    expect(client.error).toBe('');
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

  test('a pasted image shows its tile at once, under the id its draft takes; removing it while it prepares lets it go', async () => {
    const { client, native } = await opened();
    const { fs, store } = memoryFiles({ 'doc:/1/shot.png': png(4, 3, 64), 'doc:/2/second.png': png(4, 3, 64) });
    let release: () => void = () => {};
    const readFile = fs.readFile;
    fs.readFile = async (path: string) => { if (path.startsWith('doc:')) await new Promise<void>(resolve => { release = resolve; }); return readFile(path); };
    const first = attachIncoming(client, native, 'paste', incoming(['doc:/1/shot.png']));
    await new Promise(resolve => setTimeout(resolve, 0));
    const [tile] = preparingImages(client);
    expect(tile).toMatchObject({ name: 'shot.png' });
    release();
    await first;
    expect(preparingImages(client)).toEqual([]);
    expect((client.local.snapshotDrafts[client.draftKey] ?? []).map(image => image.id)).toEqual([tile!.id]);
    const second = attachIncoming(client, native, 'paste', incoming(['doc:/2/second.png']));
    await new Promise(resolve => setTimeout(resolve, 0));
    const [cancelled] = preparingImages(client);
    cancelPreparingImage(client, cancelled!.id);
    expect(preparingImages(client)).toEqual([]);
    release();
    await second;
    expect((client.local.snapshotDrafts[client.draftKey] ?? []).map(image => image.id)).toEqual([tile!.id]);
    expect(client.local.snapshotReleases).toContain(cancelled!.id);
    void store;
  });
  test('a preparing tile shows a 256 px thumbnail of the image it takes, removed once the image is in', async () => {
    const { client, native } = await opened();
    const { fs, store, compressed } = memoryFiles({ 'doc:/1/shot.png': png(4, 3, 64) });
    let release: () => void = () => {};
    const readFile = fs.readFile;
    fs.readFile = async (path: string) => { if (path.startsWith('doc:')) await new Promise<void>(resolve => { release = resolve; }); return readFile(path); };
    const taking = attachIncoming(client, native, 'paste', incoming(['doc:/1/shot.png']));
    for (let turn = 0; turn < 8; turn++) await new Promise(resolve => setTimeout(resolve, 0));
    const [tile] = preparingImages(client);
    const folder = `app:/tmp/composer-stage/preview-${tile!.id}`;
    expect(tile!.source).toBe(`${folder}/thumbnail.png`);
    // ComposerImageThumbnail: 256 px; compressImage reads an app copy of the pasted file, which goes once it is made.
    expect(compressed.find(entry => entry.to === tile!.source)).toMatchObject({ from: `${folder}/shot.png`, maxDimension: 256, type: 'image/png' });
    expect(store.has(`${folder}/shot.png`)).toBe(false);
    expect(store.has(tile!.source)).toBe(true);
    release();
    await taking;
    expect(preparingImages(client)).toEqual([]);
    expect(store.has(tile!.source)).toBe(false);
  });
  test('a source past 50 MB gets no thumbnail; a tile draws its thumbnail or, without one, the name (ComposerImageThumbnail)', async () => {
    const { fs, compressed } = memoryFiles({ 'doc:/1/huge.png': png(4, 3, 64) });
    const stat = fs.stat;
    fs.stat = async (path: string) => ({ ...(await stat(path)), size: 51 * 1024 * 1024 });
    expect(await previewSource({ path: 'doc:/1/huge.png', name: 'huge.png', type: 'image/png' }, 'big')).toBe('');
    expect(compressed).toEqual([]);
    const tile = contractComponent('src/composer/composer-editor.contract', 'PreparingImageTile');
    for (const source of ['', 'app:/tmp/composer-stage/preview-a/thumbnail.png']) {
      const scope = { id: 'a', name: 'shot.png', source, command: () => {} };
      expect([tile.drawn('testId=`preparing-thumbnail-', {}, scope), tile.drawn('text name ', {}, scope)]).toEqual([source !== '', source === '']);
    }
  });
  test('what cannot be attached is named, as addComposerAttachments reports it', async () => {
    const { client, native } = await opened();
    memoryFiles({ 'doc:/1/layers.psd': text('8BPS'), 'doc:/2/broken.png': text('X'), 'doc:/3/empty.txt': new Uint8Array() });
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/layers.psd']));
    expect(client.error).toBe("'layers.psd' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.");
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

  test('Paste as Text chosen from the menu gives a pasted image no chip, as its key does', async () => {
    const { client, native } = await opened();
    client.local.drafts[client.draftKey] = 'look at this';
    observeComposer(client, client.draft, true);
    memoryFiles({ 'doc:/1/image.png': png(4, 3) });
    native.menuArmed = true;
    await attachIncoming(client, native, 'paste', incoming(['doc:/1/image.png']));
    expect(ops(native, 'editorInsert')).toEqual([]);
    expect((client.local.snapshotDrafts[client.draftKey] ?? []).map(image => str(image.name))).toEqual(['image.png']);
    expect(native.menuArmed).toBe(false);
  });

  test('a video or audio file is staged under its typed name; a video\'s display size comes from its header', async () => {
    const { client, native } = await opened();
    const movie = new Uint8Array(readFileSync(appPath('../../apps/video-player/assets/motion.mp4')));
    const { store } = memoryFiles({ 'doc:/1/clip.MP4': movie, 'doc:/2/memo.m4a': text('ID3'), 'doc:/3/notes.txt': text('notes') });
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/clip.MP4', 'doc:/2/memo.m4a', 'doc:/3/notes.txt']));
    const [clip, memo, notes] = draftFiles(client.local);
    expect(clip).toMatchObject({ name: 'clip.MP4', mimeType: 'video/mp4', videoWidth: 404, videoHeight: 720 });
    expect(memo!.videoWidth).toBeUndefined();
    expect(under(store, STAGED).sort()).toEqual([`${STAGED}/${clip!.id}.mp4`, `${STAGED}/${memo!.id}.m4a`, `${STAGED}/${notes!.id}`].sort());
    expect(store.get(`${STAGED}/${clip!.id}.mp4`)).toEqual(movie);
    expect(native.calls.map(call => String(call.op)).filter(op => op.startsWith('composer'))).toEqual([]); // no module op
  });

  test('a file pasted into an empty draft keeps its record through the view that read the editor as empty', async () => {
    const { client, native, view } = await opened();
    const { released } = memoryFiles({ 'doc:/1/sample.py': text('print("sample")\n') });
    await attachIncoming(client, native, 'paste', incoming(['doc:/1/sample.py']));
    await view(native);
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

  test('the attachment limit counts the draft the files were given to, not the one now open', async () => {
    const { client, native } = await opened();
    const { fs, released } = memoryFiles({ 'doc:/1/a.txt': text('abc'), 'doc:/2/b.txt': text('def') });
    const stat = fs.stat, first = client.draftKey;
    // The draft the person moves to is full; the one that took the drop has room for both.
    client.local.snapshotDrafts[`${client.environmentId}:t2`] = Array.from({ length: 100 }, (_, index) => ({ id: uuid(index + 10), name: `${index}.png` }));
    fs.stat = async (path: string) => { if (path === 'doc:/2/b.txt') client.threadId = 't2'; return stat(path); };
    await attachIncoming(client, native, 'drop', incoming(['doc:/1/a.txt', 'doc:/2/b.txt']));
    expect(client.draftKey).not.toBe(first);
    expect(released()).toEqual([]);
    expect(draftFiles(client.local).map(file => [file.name, file.draftKey])).toEqual([['a.txt', first], ['b.txt', first]]);
  });
});

describe('an emptied draft\'s files', () => {
  test.each([{ label: 'file only', withImage: false }, { label: 'file and image', withImage: true }])('keeps staged files through queued insertion and an older empty view: $label', async ({ withImage }) => {
    const { client, native, view } = await opened();
    const { released } = memoryFiles({ [`${STAGED}/${uuid(1)}`]: text('twenty-six bytes of notes\n') });
    const owner = composerFieldOwner(client), empty = observeComposer(client, '');
    const fieldNative = composerFieldNative(client, native, () => composerFieldOwner(client));
    const files = [{ kind: 'file', id: uuid(1), name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 26 },
      ...(withImage ? [{ kind: 'image', id: uuid(2), name: 'image.png', sizeBytes: 120 }] : [])];
    await addAttachedFiles(client, fieldNative, files);
    const link = `[notes.txt](t3-context://v1/file/file_${uuid(1)})`;
    expect(client.draft).toBe('');
    await view(fieldNative);
    expect(draftFiles(client.local)).toMatchObject([{ id: uuid(1), status: 'staged' }]);
    expect(fileChipContexts(client, link)).toEqual({ [`file/file_${uuid(1)}`]: 'file\t1 KB' });
    expect(released()).toEqual([]);

    const read = Promise.withResolvers<void>(), resumed = Promise.withResolvers<void>();
    native.onSync = () => { read.resolve(); return resumed.promise; };
    const syncing = view(fieldNative);
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
    await view(fieldNative);
    expect(draftFiles(client.local)).toEqual([]);
    expect(released()).toEqual([uuid(1)]);
  });
  test('a large paste folded into an empty draft keeps its file through a view before its chip lands', async () => {
    const { client, native, view } = await opened();
    memoryFiles({});
    const owner = composerFieldOwner(client), empty = observeComposer(client, '');
    const fieldNative = composerFieldNative(client, native, () => composerFieldOwner(client));
    await foldPaste(client, fieldNative, 'fold', { id: 1, snapshotOwner: empty.snapshotOwner, fieldOwner: owner, value: '', start: 0, end: 0, text: 'x'.repeat(40_000) }, uuid(7));
    await view(fieldNative);
    expect(draftFiles(client.local).map(file => file.name)).toEqual(['pasted-text.txt']);
    const edit = composerFieldResource(client, [empty, 0], owner);
    const inserted = observeComposer(client, edit.text, true);
    expect((await composerFieldAcknowledged(client, [inserted, edit.seq, true], owner, native)).applied).toBe(true);
    expect(toasts(client).map(toast => toast.title)).toEqual(['Large paste attached as pasted-text.txt']);
    await view(fieldNative);
    expect(fileChipContexts(client, edit.text)).toEqual({ [`file/file_${uuid(7)}`]: 'file\t40 KB' });
  });
  test('a refused queued insertion releases files when its original empty draft is abandoned', async () => {
    const { client, native, view } = await opened();
    const { released } = memoryFiles({ [`${STAGED}/${uuid(1)}`]: text('notes') });
    const fieldNative = composerFieldNative(client, native, () => composerFieldOwner(client));
    await addAttachedFiles(client, fieldNative, [{ kind: 'file', id: uuid(1), name: 'notes.txt', sizeBytes: 26 }]);
    client.threadId = 't2';
    observeComposer(client, '');
    await view(composerFieldNative(client, native, () => composerFieldOwner(client)));
    expect(draftFiles(client.local)).toEqual([]);
    expect(released()).toEqual([uuid(1)]);
  });
  test('a file attached while the view was in flight survives its empty editor; one from before goes', async () => {
    const { client, native, view } = await opened();
    const { released } = memoryFiles({ [`${STAGED}/${uuid(5)}`]: text('old'), [`${STAGED}/${uuid(6)}`]: text('sample') });
    const old = acceptAttachedFile(client, { kind: 'file', id: uuid(5), name: 'old.txt', sizeBytes: 3 }).file!;
    // The editor reads as empty; Attach files' pick lands (its chip on its way into the editor) while the view awaits,
    // as the panel's completion did on the first file attached to an empty draft.
    let added = null as ReturnType<typeof acceptAttachedFile>['file'];
    native.onSync = () => { added = acceptAttachedFile(client, { kind: 'file', id: uuid(6), name: 'sample.py', mimeType: 'text/x-python', sizeBytes: 67 }).file; };
    await view(native);
    expect(draftFiles(client.local)).toEqual([added!]);
    expect(released()).toEqual([old.id]);
    // The next view sees the chip in the editor's text, so the file keeps its record (and its chip its name and size).
    observeComposer(client, fileChipLink(added!) + ' ', true);
    native.onSync = () => {};
    await view(native);
    expect(draftFiles(client.local)).toEqual([added!]);
  });
});
