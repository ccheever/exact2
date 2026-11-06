import { observeComposer } from '../support/composer-field-fixture';
import { memoryFiles, text } from '../support/app-files-fixture';
import { DRAFTS, STAGED } from '../../src/composer/composer-staged-copies';
import { describe, expect, test } from 'bun:test';
import { referencedImageIds, removeInlineContextReference, videoMimeType, videoTiles } from '../../src/composer/composer-attachments';
import { contextId } from '../../src/composer/composer-editor-menu';
import { commandLabel } from '../../src/snapshot/snapshot-shortcut';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { composerVideoSnapshot, rememberVideoFrame, removeAttachmentReferences, videoOp } from '../../src/composer/composer-attachments';
import { fileChipContexts, setDraftFiles, type DraftFile } from '../../src/composer/composer-editor-files';

const id = 'a1b2c3d4-0000-4000-8000-000000000001';
const chip = `![shot.png](t3-context://v1/image/${contextId('image', id)})`;

describe('removing a referenced shelf image (removeComposerImage, removeInlineContextReference)', () => {
  test('the shelf knows which images the prompt references', () => {
    expect([...referencedImageIds(`look at ${chip} please`, [id, 'b'])]).toEqual([id]);
    expect(referencedImageIds('no chips', [id]).size).toBe(0);
  });
  test('every reference goes, with one adjoining space each', () => {
    const cid = contextId('image', id);
    expect(removeInlineContextReference(`look at ${chip} and ${chip} please`, cid).prompt).toBe('look at and please');
    expect(removeInlineContextReference(`${chip} first`, cid).prompt).toBe('first');
    expect(removeInlineContextReference(`end ${chip}`, cid)).toEqual({ prompt: 'end', cursor: 3 });
    expect(removeInlineContextReference(`keep [file](t3-context://v1/file/file_x) ${chip}`, cid).prompt).toBe('keep [file](t3-context://v1/file/file_x)');
    expect(removeInlineContextReference('untouched', cid)).toEqual({ prompt: 'untouched', cursor: 9 });
  });
});

describe('video tiles (isPreviewableComposerVideo, videoMimeType)', () => {
  test('a definite type answers; the extension speaks only for a generic one', () => {
    expect(videoMimeType({ name: 'clip.mov', mimeType: 'video/quicktime' })).toBe('video/quicktime');
    expect(videoMimeType({ name: 'clip.MP4', mimeType: 'application/octet-stream' })).toBe('video/mp4');
    expect(videoMimeType({ name: 'clip.webm', mimeType: '' })).toBe('video/webm');
    expect(videoMimeType({ name: 'notes.mp4', mimeType: 'application/pdf' })).toBeNull();
    expect(videoMimeType({ name: 'notes.md', mimeType: 'text/markdown' })).toBeNull();
  });
  test('a TypeScript file labelled as an MPEG transport stream is not a video', () => {
    expect(videoMimeType({ name: 'index.ts', mimeType: 'video/mp2t' })).toBeNull();
    expect(videoMimeType({ name: 'index.ts', mimeType: 'video/vnd.dlna.mpeg-tts' })).toBeNull();
    expect(videoMimeType({ name: 'clip.mp4', mimeType: 'video/mp2t' })).toBe('video/mp4');
  });
  test('only the draft\'s attached, staged videos become tiles', () => {
    const file = (over: object) => ({ id, name: 'clip.mov', mimeType: 'video/quicktime', contextId: 'file_x', source: 'attached', status: 'staged', draftKey: 'k', ...over });
    expect(videoTiles([file({})], 'k')).toEqual([{ id, name: 'clip.mov', src: `app:/data/composer-files/${id}.mov`, contextId: 'file_x' }]);
    expect(videoTiles([file({ draftKey: 'other' })], 'k')).toEqual([]);
    expect(videoTiles([file({ source: 'paste' })], 'k')).toEqual([]);
    expect(videoTiles([file({ name: 'a.md', mimeType: 'text/markdown' })], 'k')).toEqual([]);
  });
});

describe('keybinding labels', () => {
  test('the SnapShot conflict message names composer.sendAndNewThread as the reference does', () => {
    expect(commandLabel('composer.sendAndNewThread')).toBe('Composer: Send and Start New Thread');
    expect(commandLabel('composer.sendBackground')).toBe('Composer: Start in Background');
  });
});

class Recorder implements Native {
  available = true; requests: Record<string, unknown>[] = []; applied = true;
  input?: (request: Record<string, unknown>) => void;
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = input as Record<string, unknown>; this.requests.push(request);
    if (request.op === 'editorEdit' && this.applied) this.input?.(request);
    return { ok: true, generation: 1, value: request.op === 'editorEdit' ? { applied: this.applied } : { removed: true } };
  }
}
const vid = 'b0000000-0000-4000-8000-0000000000aa';
function draftClient(prompt: string): { client: T3Client; native: Recorder; file: DraftFile } {
  const client = new T3Client(); Object.assign(client, { environmentId: 'env', threadId: 't' });
  const file: DraftFile = { id: vid, contextId: `file_${vid}`, draftKey: 'env:t', environmentId: 'env', name: 'clip.mov', mimeType: 'video/quicktime', sizeBytes: 4096, source: 'attached', attachmentId: '', status: 'staged' };
  setDraftFiles(client.local, [file]);
  client.local.drafts['env:t'] = prompt; observeComposer(client, prompt);
  const native = new Recorder();
  native.input = request => {
    expect(client.local.drafts['env:t']).toBe(request.expect as string);
    client.local.drafts['env:t'] = request.text as string; observeComposer(client, String(request.text), true);
  };
  return { client, native, file };
}

describe('the shelf\'s references and videos in the client', () => {
  test('removing an image strips its chips from the stored draft and the open editor', async () => {
    const { client, native } = draftClient(`look at ${chip} now`);
    expect(await removeAttachmentReferences(client, native, 'env:t', 'image', id)).toBe(true);
    expect(client.local.drafts['env:t']).toBe('look at now');
    expect(native.requests).toEqual([{ op: 'editorEdit', all: true, expect: `look at ${chip} now`, text: 'look at now', focus: false }]);
    expect(await removeAttachmentReferences(client, native, 'env:t', 'image', id)).toBe(false);
  });
  test('a refused live edit preserves the current prompt', async () => {
    const prompt = `look at ${chip} now`, { client, native } = draftClient(prompt);
    native.applied = false;
    await expect(removeAttachmentReferences(client, native, 'env:t', 'image', id)).rejects.toThrow('The prompt changed before attachment references could be removed.');
    expect(client.local.drafts['env:t']).toBe(prompt);
  });
  test('a background draft loses references without touching the open field', async () => {
    const { client, native } = draftClient('open prompt');
    client.local.drafts['env:background'] = `look at ${chip} now`;
    expect(await removeAttachmentReferences(client, native, 'env:background', 'image', id)).toBe(true);
    expect(client.local.drafts['env:background']).toBe('look at now');
    expect(client.local.drafts['env:t']).toBe('open prompt');
    expect(native.requests).toEqual([]);
  });
  test('a video chip carries the video kind (FilmIcon, its tint)', () => {
    const { client, file } = draftClient('');
    expect(fileChipContexts(client, `see [clip.mov](t3-context://v1/file/${file.contextId})`)).toEqual({ [`file/${file.contextId}`]: 'video\t4 KB' });
  });
  test('Play opens the preview at the video\'s size, Close ends it, Remove takes the file, its chips and its staged copy', async () => {
    const { client, native, file } = draftClient(`see [clip.mov](t3-context://v1/file/${file_ctx()}) please`);
    // An older build's copy, typed link and first-frame poster go too.
    const { removed, store } = memoryFiles({ [`${STAGED}/${vid}`]: text('moov'), [`${STAGED}/${vid}.mov`]: text('moov'), [`${DRAFTS}/${vid}.png`]: text('poster') });
    rememberVideoFrame(client, { id: vid, videoWidth: 1080, videoHeight: 1920 });
    expect(composerVideoSnapshot(client)).toEqual({ composerVideos: [{ id: vid, name: 'clip.mov', src: `app:/data/composer-files/${vid}.mov` }], composerVideoPreview: [] });
    await videoOp(client, native, 'shelf-video-play', vid);
    expect(composerVideoSnapshot(client).composerVideoPreview).toEqual([{ open: true, id: vid, name: 'clip.mov', width: 1080, height: 1920, src: `app:/data/composer-files/${vid}.mov` }]);
    await videoOp(client, native, 'shelf-video-close', '');
    expect(composerVideoSnapshot(client).composerVideoPreview).toEqual([]);
    await videoOp(client, native, 'shelf-video-remove', vid);
    expect(composerVideoSnapshot(client).composerVideos).toEqual([]);
    expect(client.local.drafts['env:t']).toBe('see please');
    expect(native.requests.map(request => request.op)).toEqual(['editorEdit']);
    expect(JSON.parse(new TextDecoder().decode(store.get('app:/data/t3-code.json'))).composerFiles).toEqual([]); // the draft's files, saved without it
    // Every name its staged copy has had (composer-staged-copies.ts removeStaged).
    expect(removed.sort()).toEqual([`${STAGED}/${vid}`, `${STAGED}/${vid}.mov`, `${DRAFTS}/${vid}.png`].sort());
    await expect(videoOp(client, native, 'shelf-video-play', vid)).rejects.toThrow('That video is no longer attached.');
    expect(file.name).toBe('clip.mov');
  });
});
function file_ctx(): string { return `file_${vid}`; }
