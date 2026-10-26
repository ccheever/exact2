import { describe, expect, test } from 'bun:test';
import { attachmentUrls, imagePreviewAction, imagePreviewView, messageAttachments } from '../../src/timeline/timeline-attachments';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { memoryFiles, scaledPhoto } from '../support/image-accent-fixture';
import { serveAssets } from '../support/asset-server';

const item = { type: 'user_message', attachments: [
  { type: 'image', id: 'a1', name: 'shot.png', mimeType: 'image/png', source: { kind: 'snap-shot', appName: 'Safari', windowTitle: '', accessibleText: 'Hello' } },
  { type: 'image', id: 'a2', name: 'diagram.png', mimeType: 'image/png' },
  { type: 'image', id: 'a3', name: 'preview-annotation-1.png', mimeType: 'image/png' },
  { type: 'file', id: 'f1', name: 'notes.md', mimeType: 'text/markdown' }] };
function client(calls: Obj[] = []) {
  return { threadId: 't1', generation: 1, environmentId: 'e', origin: 'http://127.0.0.1:1', connection: 'connected',
    projection: { visibleTurnItems: [{ sourceThreadId: 't1', sourceItemId: 'm1', item }] },
    rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ method, ...payload }); return { relativeUrl: `/api/assets/${(payload.resource as Obj).attachmentId}`, expiresAt: 10_000_000 }; } } as unknown as T3Client;
}
describe('sent attachments', () => {
  test('images keep SnapShot details and skip annotation previews; files take their Pierre icon', () => {
    const { images, attachFiles } = messageAttachments(item);
    expect(images.map(image => [image.id, image.snapshot, image.appName, image.appInitial, image.windowTitle, image.accessible])).toEqual([
      ['a1', true, 'Safari', 'S', 'Captured window', true], ['a2', false, '', '', '', false]]);
    expect(attachFiles).toEqual([{ id: 'f1', name: 'notes.md', icon: 'markdown' }]);
  });
  test('signed inline URLs come from assets.createUrl and are reused until they near expiry; each image is read once for its accent', async () => {
    const calls: Obj[] = [], c = client(calls), native = { available: true } as Native;
    // image-accent.ts: a1 reads as rgb(129 140 168), a2 cannot be fetched.
    memoryFiles(scaledPhoto());
    const server = serveAssets(url => url.endsWith('/a1') ? { body: 'image bytes' } : { status: 404 });
    try {
      const items = (await attachmentUrls(c, native, 1000)).items;
      expect(items.map(entry => [entry.id, entry.url, entry.fill])).toEqual([
        ['a1', 'http://127.0.0.1:1/api/assets/a1', '#818ca81c'], ['a2', 'http://127.0.0.1:1/api/assets/a2', '']]);
      expect(calls[0]).toMatchObject({ method: 'assets.createUrl', resource: { _tag: 'attachment', attachmentId: 'a1', fileName: 'shot.png', mimeType: 'image/png', disposition: 'inline' } });
      expect(server.calls.map(call => call.url.split('/').pop())).toEqual(['a1', 'a2']);
      await attachmentUrls(c, native, 2000);
      expect(calls).toHaveLength(2);
      expect(server.calls).toHaveLength(2);
    } finally { server.restore(); }
  });
  test('an accent read that failed for a passing reason is asked again by the next answer', async () => {
    const c = { ...client(), environmentId: 'e-retry' } as T3Client, native = { available: true } as Native;
    memoryFiles(scaledPhoto());
    let online = false;
    const server = serveAssets(url => !online ? new Error('timed out') : url.endsWith('/a1') ? { body: 'image bytes' } : { status: 404 });
    try {
      expect((await attachmentUrls(c, native, 1000)).items.map(entry => entry.fill)).toEqual(['', '']);
      online = true;
      expect((await attachmentUrls(c, native, 2000)).items.map(entry => entry.fill)).toEqual(['#818ca81c', '']);
      await attachmentUrls(c, native, 3000);
      expect(server.calls.map(call => call.url.split('/').pop())).toEqual(['a1', 'a2', 'a1', 'a2']);
    } finally { server.restore(); }
  });
  test('the expanded preview opens on an image and steps through the message', () => {
    const c = client(), id = JSON.stringify(['t1', 'm1']);
    imagePreviewAction(c, 'image-open', 'a1', id);
    expect(imagePreviewView(c)).toEqual({ imagePreviewId: 'a1', imagePreviewName: 'shot.png', imagePreviewPosition: '(1/2)', imagePreviewPrevious: true, imagePreviewNext: true, imagePreviewVideo: false, imagePreviewSource: 'attachment:a1', imagePreviewTip: 'shot.png', imagePreviewReturn: '' });
    imagePreviewAction(c, 'image-step', '', 'next');
    expect(imagePreviewView(c)).toMatchObject({ imagePreviewId: 'a2', imagePreviewPosition: '(2/2)' });
    imagePreviewAction(c, 'image-step', '', 'next');
    expect(imagePreviewView(c).imagePreviewId).toBe('a1');
    imagePreviewAction(c, 'image-close', '', '');
    expect(imagePreviewView(c).imagePreviewId).toBe('');
    expect(() => imagePreviewAction(c, 'image-open', 'zz', id)).toThrow('no longer available');
  });
});
