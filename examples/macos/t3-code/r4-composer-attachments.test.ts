import { describe, expect, test } from 'bun:test';
import { referencedImageIds, removeInlineContextReference, videoMimeType, videoTiles } from './r4-composer-attachments';
import { contextId } from './composer-editor-menu';
import { commandLabel } from './snapshot-shortcut';
import { T3Client } from './client';
import type { Native } from './protocol';
import { composerVideoSnapshot, rememberVideoFrame, removeAttachmentReferences, videoOp } from './r4-composer-attachments';
import { fileChipContexts, setDraftFiles, type DraftFile } from './composer-editor-files';

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
  test('only the draft\'s attached, staged videos become tiles', () => {
    const file = (over: object) => ({ id, name: 'clip.mov', mimeType: 'video/quicktime', contextId: 'file_x', source: 'attached', status: 'staged', draftKey: 'k', ...over });
    expect(videoTiles([file({})], 'k')).toEqual([{ id, name: 'clip.mov', poster: id, contextId: 'file_x' }]);
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
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = input as Record<string, unknown>; this.requests.push(request);
    return { ok: true, generation: 1, value: request.op === 'editorEdit' ? { applied: this.applied } : { removed: true } };
  }
}
const vid = 'b0000000-0000-4000-8000-0000000000aa';
function draftClient(prompt: string): { client: T3Client; native: Recorder; file: DraftFile } {
  const client = new T3Client(); Object.assign(client, { environmentId: 'env', threadId: 't' });
  const file: DraftFile = { id: vid, contextId: `file_${vid}`, draftKey: 'env:t', environmentId: 'env', name: 'clip.mov', mimeType: 'video/quicktime', sizeBytes: 4096, source: 'attached', attachmentId: '', status: 'staged' };
  setDraftFiles(client.local, [file]);
  client.local.drafts['env:t'] = prompt;
  return { client, native: new Recorder(), file };
}

describe('the shelf\'s references and videos in the client', () => {
  test('removing an image strips its chips from the stored draft and the open editor', async () => {
    const { client, native } = draftClient(`look at ${chip} now`);
    expect(await removeAttachmentReferences(client, native, 'env:t', 'image', id)).toBe(true);
    expect(client.local.drafts['env:t']).toBe('look at now');
    expect(native.requests).toEqual([{ op: 'editorEdit', all: true, expect: `look at ${chip} now`, text: 'look at now', focus: false }]);
    expect(await removeAttachmentReferences(client, native, 'env:t', 'image', id)).toBe(false);
  });
  test('a video chip carries the video kind (FilmIcon, its tint)', () => {
    const { client, file } = draftClient('');
    expect(fileChipContexts(client, `see [clip.mov](t3-context://v1/file/${file.contextId})`)).toEqual({ [`file/${file.contextId}`]: 'video\t4 KB' });
  });
  test('Play opens the preview at the first frame\'s size, Close ends it, Remove takes the file, its chips and its staged copy', async () => {
    const { client, native, file } = draftClient(`see [clip.mov](t3-context://v1/file/${file_ctx()}) please`);
    rememberVideoFrame(client, { id: vid, videoWidth: 1080, videoHeight: 1920 });
    expect(composerVideoSnapshot(client)).toEqual({ composerVideos: [{ id: vid, name: 'clip.mov' }], composerVideoPreview: [] });
    await videoOp(client, native, 'r4c-video-play', vid);
    expect(composerVideoSnapshot(client).composerVideoPreview).toEqual([{ open: true, id: vid, name: 'clip.mov', width: 1080, height: 1920 }]);
    await videoOp(client, native, 'r4c-video-close', '');
    expect(composerVideoSnapshot(client).composerVideoPreview).toEqual([]);
    await videoOp(client, native, 'r4c-video-remove', vid);
    expect(composerVideoSnapshot(client).composerVideos).toEqual([]);
    expect(client.local.drafts['env:t']).toBe('see please');
    expect(native.requests.map(request => request.op)).toEqual(['editorEdit', 'composerAttachRemove']);
    await expect(videoOp(client, native, 'r4c-video-play', vid)).rejects.toThrow('That video is no longer attached.');
    expect(file.name).toBe('clip.mov');
  });
});
function file_ctx(): string { return `file_${vid}`; }
