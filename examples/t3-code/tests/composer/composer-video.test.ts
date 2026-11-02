// A shelf video's display size, read from its header, survives a relaunch (saved with the draft's file entry); a staged
// video or audio copy keeps its extension.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';
import { videoFrameSize } from '../../src/composer/composer-video';
import { stagedName } from '../../src/composer/composer-staged-copies';
import { T3Client } from '../../src/client/client';
import { composerVideoSnapshot, rememberVideoFrame, videoOp } from '../../src/composer/composer-attachments';
import { adoptComposerFiles, draftFiles, setDraftFiles, type DraftFile } from '../../src/composer/composer-editor-files';
import type { Native } from '../../src/client/protocol';

const vid = 'b0000000-0000-4000-8000-0000000000bb';
const native: Native = { available: true, watch() {}, async later() { return { ok: true, generation: 1, value: {} }; } } as unknown as Native;
function client(): T3Client {
  const next = new T3Client(); Object.assign(next, { environmentId: 'env', threadId: 't' });
  return next;
}
const file: DraftFile = { id: vid, contextId: `file_${vid}`, draftKey: 'env:t', environmentId: 'env', name: 'clip.mov', mimeType: 'video/quicktime', sizeBytes: 4096, source: 'attached', attachmentId: '', status: 'staged' };

describe('video preview aspect after relaunch', () => {
  test('the pick\'s frame size is written to the draft file and read back from the saved preferences', async () => {
    const first = client();
    setDraftFiles(first.local, [{ ...file }]);
    rememberVideoFrame(first, { id: vid, videoWidth: 1080, videoHeight: 1920 });
    expect(draftFiles(first.local)[0]).toMatchObject({ videoWidth: 1080, videoHeight: 1920 });
    // The preference file is the JSON of `local`; a relaunch adopts it into a fresh client.
    const saved = JSON.parse(JSON.stringify({ version: 1, ...first.local }));
    const second = client();
    adoptComposerFiles(second.local, saved);
    await videoOp(second, native, 'shelf-video-play', vid);
    expect(composerVideoSnapshot(second).composerVideoPreview).toEqual([{ open: true, id: vid, name: 'clip.mov', width: 1080, height: 1920, src: `app:/data/composer-files/${vid}.mov` }]);
  });
  test('an unknown or invalid size stays unknown (16:9 in the view)', async () => {
    const saved = { composerFiles: [{ ...file, videoWidth: 0, videoHeight: 'x' }] };
    const next = client();
    adoptComposerFiles(next.local, saved);
    expect(draftFiles(next.local)[0]!.videoWidth).toBeUndefined();
    await videoOp(next, native, 'shelf-video-play', vid);
    expect(composerVideoSnapshot(next).composerVideoPreview[0]).toMatchObject({ width: 0, height: 0 });
  });
});

/** A `moov` holding one track header (ISO/IEC 14496-12 tkhd), of `version`, with `width` × `height` under `matrix`'s a and b. */
function movie(width: number, height: number, a: number, b: number, version = 0): Uint8Array {
  const body = version === 1 ? 96 : 84, tkhd = new Uint8Array(8 + body), view = new DataView(tkhd.buffer);
  view.setUint32(0, tkhd.length); tkhd.set([0x74, 0x6b, 0x68, 0x64], 4); tkhd[8] = version;
  const matrix = 8 + (version === 1 ? 52 : 40);
  view.setInt32(matrix, a * 65536); view.setInt32(matrix + 4, b * 65536); view.setInt32(matrix + 12, -b * 65536); view.setInt32(matrix + 16, a * 65536);
  view.setUint32(matrix + 36, width * 65536); view.setUint32(matrix + 40, height * 65536);
  const box = (type: string, inner: Uint8Array) => { const out = new Uint8Array(8 + inner.length); new DataView(out.buffer).setUint32(0, out.length); out.set([...type].map(c => c.charCodeAt(0)), 4); out.set(inner, 8); return out; };
  const ftyp = box('ftyp', new Uint8Array(8));
  const moov = box('moov', box('trak', tkhd));
  const out = new Uint8Array(ftyp.length + moov.length); out.set(ftyp); out.set(moov, ftyp.length);
  return out;
}

describe('a video\'s display size (videoFrameSize)', () => {
  test('the first picture track\'s size, turned by a quarter-turn matrix; null for what it cannot read', () => {
    expect(videoFrameSize(new Uint8Array(readFileSync(appPath('../../apps/video-player/assets/motion.mp4'))))).toEqual({ width: 404, height: 720 });
    expect(videoFrameSize(movie(1920, 1080, 1, 0))).toEqual({ width: 1920, height: 1080 });
    expect(videoFrameSize(movie(1920, 1080, 0, 1))).toEqual({ width: 1080, height: 1920 });
    expect(videoFrameSize(movie(1920, 1080, 0, -1, 1))).toEqual({ width: 1080, height: 1920 });
    expect(videoFrameSize(movie(0, 0, 1, 0))).toBeNull(); // an audio track
    expect(videoFrameSize(new TextEncoder().encode('\x1aE\xdf\xa3 webm'))).toBeNull();
    expect(videoFrameSize(movie(640, 480, 1, 0).subarray(0, 40))).toBeNull();
  });
});

describe('a staged copy\'s name (stagedName)', () => {
  test('a video or audio file keeps its extension, letters and digits only; any other file is its id', () => {
    expect(stagedName({ id: vid.toUpperCase(), name: 'clip.MOV', mimeType: 'video/quicktime' })).toBe(`${vid}.mov`);
    expect(stagedName({ id: vid, name: 'memo.m4a', mimeType: 'audio/x-m4a' })).toBe(`${vid}.m4a`);
    expect(stagedName({ id: vid, name: 'odd.M-4 A', mimeType: 'audio/x-m4a' })).toBe(`${vid}.m4a`);
    expect(stagedName({ id: vid, name: 'clip', mimeType: 'video/mp4' })).toBe(vid);
    expect(stagedName({ id: vid, name: 'spec.pdf', mimeType: 'application/pdf' })).toBe(vid);
  });
});
