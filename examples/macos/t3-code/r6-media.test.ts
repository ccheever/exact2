import { describe, expect, test } from 'bun:test';
import { attachmentLocal, attachmentView, previewKind, type AttachmentMeta } from './r5-panels-attach';
import { isMediaPreview, mediaBody, mediaErrorMessage } from './r6-media-preview';
import type { T3Client } from './client';
import type { Native } from './protocol';
import type { Obj } from './domain';

const files: AttachmentMeta[] = [
  { id: 'p1', name: 'report.pdf', mimeType: 'application/pdf', sizeBytes: 956 },
  { id: 'h1', name: 'page.html', mimeType: 'text/html', sizeBytes: 491 },
  { id: 'a1', name: 'tone.wav', mimeType: 'audio/wav', sizeBytes: 132_344 },
  { id: 'v1', name: 'clip.mp4', mimeType: 'video/mp4', sizeBytes: 5452 },
  { id: 'z1', name: 'archive.zip', mimeType: 'application/zip', sizeBytes: 10 },
];
function client(calls: Obj[] = []) {
  const item = { type: 'user_message', attachments: files.map(file => ({ type: 'file', ...file })) };
  return {
    ready: true, generation: 1, origin: 'http://127.0.0.1:1', local: { clientSettings: {} },
    projection: { visibleTurnItems: [{ item }] },
    rpc: async (_native: Native, method: string, payload: Obj) => {
      calls.push({ method, ...payload });
      return { relativeUrl: `/api/assets/${(payload.resource as Obj).attachmentId}?sig=1` };
    },
    restAccess: () => ({ call: async (request: Obj) => {
      calls.push({ op: request.op });
      return request.op === 'attachmentText' ? { ok: true, text: '<!doctype html><p>Hi</p>', truncated: false } : { ok: true };
    } }),
  } as unknown as T3Client;
}
const native = { available: true } as Native;

describe('attachment media bodies (lane r6-media)', () => {
  test('filePreviewKind classifies the four media kinds by MIME type and by name', () => {
    expect(files.map(file => previewKind(file))).toEqual(['pdf', 'html', 'audio', 'video', 'unsupported']);
    expect(['x.pdf', 'x.htm', 'x.mp3', 'x.m4a', 'x.mov', 'x.webm'].map(name => previewKind({ name, mimeType: 'application/octet-stream' })))
      .toEqual(['pdf', 'html', 'audio', 'audio', 'video', 'video']);
    expect([isMediaPreview('pdf'), isMediaPreview('image'), mediaBody('html', false), mediaBody('audio', true), mediaBody('text', true)])
      .toEqual([true, false, '', 'audio', '']);
  });

  test('PDF, audio and video draw their body from the inline signed URL; nothing reads them as text', async () => {
    const calls: Obj[] = [], c = client(calls);
    for (const [meta, preview] of [[files[0]!, 'pdf'], [files[2]!, 'audio'], [files[3]!, 'video']] as const) {
      const view = await attachmentView(c, native, meta, 1000);
      expect([view.preview, view.url, view.error, view.canRender, view.canCopy, view.canSave, view.noPreview])
        .toEqual([preview, `http://127.0.0.1:1/api/assets/${meta.id}?sig=1`, '', false, false, true, '']);
    }
    expect(calls.filter(call => call.op === 'attachmentText')).toEqual([]);
    expect(calls.map(call => (call.resource as Obj | undefined)?.disposition)).toEqual(['inline', 'inline', 'inline']);
  });

  test('HTML opens rendered with "Show HTML source"; source reads the text and offers copy and wrap', async () => {
    const calls: Obj[] = [], c = client(calls), meta = files[1]!;
    const rendered = await attachmentView(c, native, meta, 1000);
    expect([rendered.preview, rendered.canRender, rendered.rendered, rendered.renderLabel, rendered.renderIcon, rendered.url !== '', rendered.canCopy])
      .toEqual(['html', true, true, 'Show HTML source', 'code', true, false]);
    await attachmentLocal(c, native, 'render', meta.id, 0);
    const source = await attachmentView(c, native, meta, 2000);
    expect([source.preview, source.renderLabel, source.renderIcon, source.showWrap, source.canCopy, source.url, source.lines.length])
      .toEqual(['code', 'Show rendered page', 'eye', true, true, '', 1]);
    await attachmentLocal(c, native, 'render', meta.id, 0);
    const back = await attachmentView(c, native, meta, 3000);
    // As in the reference, the source read stays: Copy contents remains once the text has loaded.
    expect([back.preview, back.canCopy, back.showWrap]).toEqual(['html', true, false]);
  });

  test("a player's error reads as the reference's message; Try again re-mints and remounts it", async () => {
    const calls: Obj[] = [], c = client(calls), meta = files[2]!;
    await attachmentView(c, native, meta, 1000);
    await attachmentLocal(c, native, 'media-error', meta.id, 0);
    const failed = await attachmentView(c, native, meta, 1000);
    expect([failed.preview, failed.error, failed.url]).toEqual(['error', 'Unable to load audio.', '']);
    await attachmentLocal(c, native, 'retry', meta.id, 0);
    const again = await attachmentView(c, native, meta, 2000);
    expect([again.preview, again.error]).toEqual(['audio', '']);
    expect(calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(2);
    expect([mediaErrorMessage('video'), mediaErrorMessage('pdf')]).toEqual(['Unable to load video.', '']);
  });

  test('an unsupported file keeps "No preview for this file" and Save file', async () => {
    const view = await attachmentView(client(), native, files[4]!, 1000);
    expect([view.preview, view.noPreview, view.canSave]).toEqual(['none', 'Save it to open in an app that supports zip files.', true]);
  });
});

import { attachmentUrls, imagePreviewAction, imagePreviewView, messageAttachments } from './timeline-attachments';
describe('sent videos in the transcript (lane r6-media)', () => {
  const item = { type: 'user_message', attachments: [
    { type: 'file', id: 'v1', name: 'clip.mp4', mimeType: 'application/octet-stream' },
    { type: 'image', id: 'i1', name: 'a.png', mimeType: 'image/png' },
    { type: 'file', id: 'f1', name: 'notes.md', mimeType: 'text/markdown' },
    { type: 'image', id: 'i2', name: 'b.png', mimeType: 'image/png' }] };
  const messageId = JSON.stringify(['t1', 'm1']);
  function timeline(calls: Obj[] = []) {
    return { threadId: 't1', generation: 1, environmentId: 'e', origin: 'http://127.0.0.1:1', connection: 'connected',
      projection: { visibleTurnItems: [{ sourceThreadId: 't1', sourceItemId: 'm1', item }] },
      rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ method, ...(payload.resource as Obj) }); return { relativeUrl: `/api/assets/${(payload.resource as Obj).attachmentId}`, expiresAt: 10_000_000 }; },
      restAccess: () => ({ call: async (request: Obj) => { calls.push({ op: request.op, url: request.url }); return { accent: '' }; } }) } as unknown as T3Client;
  }
  test('a video is a grid tile after the images, never a file row', () => {
    const { images, attachFiles } = messageAttachments(item);
    expect(images.map(image => [image.id, image.video])).toEqual([['i1', false], ['i2', false], ['v1', true]]);
    expect(attachFiles.map(file => file.id)).toEqual(['f1']);
  });
  test('its inline URL is signed under its video type and never read for an accent', async () => {
    const calls: Obj[] = [];
    const { items } = await attachmentUrls(timeline(calls), { available: true } as Native, 1000);
    expect(items.map(entry => entry.id)).toEqual(['i1', 'i2', 'v1']);
    expect(calls.filter(call => call.method === 'assets.createUrl' && call.attachmentId === 'v1').map(call => [call.mimeType, call.disposition])).toEqual([['video/mp4', 'inline']]);
    expect(calls.filter(call => call.op === 'imageAccent').map(call => String(call.url).split('/').pop())).toEqual(['i1', 'i2']);
  });
  test('a video opens alone in the media dialog; images step among the images only', () => {
    const c = timeline();
    imagePreviewAction(c, 'image-open', 'v1', messageId);
    expect(imagePreviewView(c)).toEqual({ imagePreviewId: 'v1', imagePreviewName: 'clip.mp4', imagePreviewPosition: '', imagePreviewPrevious: false, imagePreviewNext: false, imagePreviewVideo: true });
    imagePreviewAction(c, 'image-open', 'i2', messageId);
    imagePreviewAction(c, 'image-step', '', 'next');
    expect(imagePreviewView(c)).toEqual({ imagePreviewId: 'i1', imagePreviewName: 'a.png', imagePreviewPosition: '(1/2)', imagePreviewPrevious: true, imagePreviewNext: true, imagePreviewVideo: false });
    imagePreviewAction(c, 'image-close', '', '');
    expect(imagePreviewView(c).imagePreviewId).toBe('');
  });
});

import { codeLines } from './r4-surfaces-files';
describe('HTML source lines (lane r6-media)', () => {
  test('every line keeps its own text: tags no longer swallow the text before them', () => {
    const text = '<!doctype html>\n<html><head><title>T</title></head>\n<body>\n<h1>Hi</h1>\n</body></html>';
    expect(codeLines('page.html', text).map(line => line.runs.map(run => run.text).join(''))).toEqual(text.split('\n'));
    expect(codeLines('page.html', text)[3]!.runs.map(run => [run.text, run.syntax])).toEqual([['<', 'punct'], ['h1', 'tag'], ['>', 'punct'], ['Hi', ''], ['</', 'punct'], ['h1', 'tag'], ['>', 'punct']]);
  });
});
