// r5-composer: a shelf video's first-frame size survives a relaunch (saved with the draft's file entry).
import { describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { composerVideoSnapshot, rememberVideoFrame, videoOp } from './r4-composer-attachments';
import { adoptComposerFiles, draftFiles, setDraftFiles, type DraftFile } from './composer-editor-files';
import type { Native } from './protocol';

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
    await videoOp(second, native, 'r4c-video-play', vid);
    expect(composerVideoSnapshot(second).composerVideoPreview).toEqual([{ open: true, id: vid, name: 'clip.mov', width: 1080, height: 1920 }]);
  });
  test('an unknown or invalid size stays unknown (16:9 in the view)', async () => {
    const saved = { composerFiles: [{ ...file, videoWidth: 0, videoHeight: 'x' }] };
    const next = client();
    adoptComposerFiles(next.local, saved);
    expect(draftFiles(next.local)[0]!.videoWidth).toBeUndefined();
    await videoOp(next, native, 'r4c-video-play', vid);
    expect(composerVideoSnapshot(next).composerVideoPreview[0]).toMatchObject({ width: 0, height: 0 });
  });
});
