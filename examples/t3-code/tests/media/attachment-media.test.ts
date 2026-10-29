import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { memoryAppFiles, serveAssets } from '../support/asset-server';
import { memoryFiles } from '../support/image-accent-fixture';
import { attachmentLocal, attachmentView, previewKind, type AttachmentMeta } from '../../src/panel/attachment-surface';
import { isMediaPreview, mediaBody, mediaErrorMessage } from '../../src/media/attachment-media';
import { forgetMediaFrames } from '../../src/media/media-frame';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';

const files: AttachmentMeta[] = [
  // A stored attachment's id names its extension (attachmentStore.ts): a PDF's ends in `-pdf`.
  { id: 'p1-pdf', name: 'report.pdf', mimeType: 'application/pdf', sizeBytes: 956 },
  { id: 'h1', name: 'page.html', mimeType: 'text/html', sizeBytes: 491 },
  { id: 'a1', name: 'tone.wav', mimeType: 'audio/wav', sizeBytes: 132_344 },
  { id: 'v1', name: 'clip.mp4', mimeType: 'video/mp4', sizeBytes: 5452 },
  { id: 'z1', name: 'archive.zip', mimeType: 'application/zip', sizeBytes: 10 },
];
function client(calls: Obj[] = [], list: AttachmentMeta[] = files) {
  const item = { type: 'user_message', attachments: list.map(file => ({ type: 'file', ...file })) };
  return {
    ready: true, generation: 1, origin: 'http://127.0.0.1:1', local: { clientSettings: {} },
    projection: { visibleTurnItems: [{ item }] },
    rpc: async (_native: Native, method: string, payload: Obj) => {
      calls.push({ method, ...payload });
      return { relativeUrl: `/api/assets/${(payload.resource as Obj).attachmentId}?sig=1` };
    },
    restAccess: () => ({ call: async (request: Obj) => {
      calls.push({ op: request.op });
      return { ok: true };
    } }),
  } as unknown as T3Client;
}
const native = { available: true } as Native;
// The asset server answers an HTML attachment's text (attachment-files.ts reads it).
let server: ReturnType<typeof serveAssets>;
beforeEach(() => { server = serveAssets(() => ({ body: '<!doctype html><p>Hi</p>' })); });
afterEach(() => server.restore());

describe('attachment media bodies', () => {
  test('filePreviewKind classifies the four media kinds by MIME type and by name', () => {
    // An archive is classified as one, and opens in the viewer page (attachment-surface.ts previewBody).
    expect(files.map(file => previewKind(file))).toEqual(['pdf', 'html', 'audio', 'video', 'archive']);
    expect(['x.pdf', 'x.htm', 'x.mp3', 'x.m4a', 'x.mov', 'x.webm'].map(name => previewKind({ name, mimeType: 'application/octet-stream' })))
      .toEqual(['pdf', 'html', 'audio', 'audio', 'video', 'video']);
    expect([isMediaPreview('pdf'), isMediaPreview('image'), mediaBody('html', false), mediaBody('audio', true), mediaBody('text', true)])
      .toEqual([true, false, '', 'audio', '']);
  });

  test('audio and video play the inline signed URL and a PDF opens it in PDF.js; nothing reads them as text', async () => {
    const calls: Obj[] = [], c = client(calls), written = memoryAppFiles();
    server.restore();
    server = serveAssets(() => ({ body: '%PDF-1.4 pages' }));
    for (const [meta, preview] of [[files[2]!, 'audio'], [files[3]!, 'video']] as const) {
      const view = await attachmentView(c, native, meta, 1000);
      expect([view.preview, view.url, view.error, view.canRender, view.canCopy, view.canSave, view.noPreview])
        .toEqual([preview, `http://127.0.0.1:1/api/assets/${meta.id}?sig=1`, '', false, false, true, '']);
    }
    expect(server.calls).toEqual([]);
    // A PDF opens in the viewer page, which fetches the signed URL for PDF.js (PdfViewer): the app downloads and writes nothing.
    const pdf = await attachmentView(c, native, files[0]!, 1000);
    expect([pdf.preview, pdf.url, pdf.error, pdf.canRender, pdf.canCopy, pdf.canSave, pdf.noPreview]).toEqual(['viewer', '', '', false, false, true, '']);
    expect(JSON.parse(pdf.viewer.config)).toMatchObject({ type: 'config', view: 'pdf', src: 'http://127.0.0.1:1/api/assets/p1-pdf?sig=1', name: 'report.pdf' });
    expect(pdf.viewer.src).toBe('assets/file-preview/index.html');
    expect([server.calls, written.size]).toEqual([[], 0]);
    expect(calls.map(call => (call.resource as Obj | undefined)?.disposition)).toEqual(['inline', 'inline', 'inline']);
  });

  test('a PDF whose URL the server will not sign says so; Try again signs it afresh', async () => {
    const c = client();
    forgetMediaFrames();
    const sign = c.rpc;
    let refused = true;
    (c as unknown as { rpc: typeof sign }).rpc = async (...args: Parameters<typeof sign>) => { if (refused) throw new Error('The attachment is unavailable.'); return sign(...args); };
    const failed = await attachmentView(c, native, files[0]!, 1000);
    expect([failed.preview, failed.error, failed.url]).toEqual(['error', 'The attachment is unavailable.', '']);
    refused = false;
    await attachmentLocal(c, native, 'retry', files[0]!.id, 1000);
    const again = await attachmentView(c, native, files[0]!, 1000);
    expect([again.preview, again.error, JSON.parse(again.viewer.config).src]).toEqual(['viewer', '', 'http://127.0.0.1:1/api/assets/p1-pdf?sig=1']);
  });

  test('a rendered page frames its signed asset URL itself', async () => {
    const view = await attachmentView(client(), native, files[1]!, 1000);
    expect([view.preview, view.url]).toEqual(['html', 'http://127.0.0.1:1/api/assets/h1?sig=1']);
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
    expect([mediaErrorMessage('video'), mediaErrorMessage('image'), mediaErrorMessage('pdf')]).toEqual(['Unable to load video.', 'Unable to load image.', '']);
  });

  test("an image's error reads \"Unable to load image.\" (its `error` event, exact2 #121); Try again loads it again", async () => {
    const meta: AttachmentMeta = { id: 'i2', name: 'shot.png', mimeType: 'image/png', sizeBytes: 9 }, c = client([], [...files, meta]);
    expect((await attachmentView(c, native, meta, 1000)).preview).toBe('image');
    await attachmentLocal(c, native, 'media-error', meta.id, 0);
    const failed = await attachmentView(c, native, meta, 1000);
    expect([failed.preview, failed.error, failed.url]).toEqual(['error', 'Unable to load image.', '']);
    await attachmentLocal(c, native, 'retry', meta.id, 0);
    expect((await attachmentView(c, native, meta, 2000)).preview).toBe('image');
  });

  test('an archive opens in the viewer page; an unsupported file keeps "No preview for this file" and Save file', async () => {
    const zip = await attachmentView(client(), native, files[4]!, 1000);
    expect([zip.preview, zip.noPreview, zip.canSave, JSON.parse(zip.viewer.config).view, JSON.parse(zip.viewer.config).kind]).toEqual(['viewer', '', true, 'binary', 'archive']);
    const blob: AttachmentMeta = { id: 'b1', name: 'blob.bin', mimeType: 'application/octet-stream', sizeBytes: 10 };
    const view = await attachmentView(client([], [...files, blob]), native, blob, 1000);
    expect([view.preview, view.noPreview, view.canSave]).toEqual(['none', 'Save it to open in an app that supports bin files.', true]);
  });
});

import { attachmentUrls, imagePreviewAction, imagePreviewView, messageAttachments } from '../../src/timeline/timeline-attachments';
describe('sent videos in the transcript', () => {
  const item = { type: 'user_message', attachments: [
    { type: 'file', id: 'v1', name: 'clip.mp4', mimeType: 'application/octet-stream' },
    { type: 'image', id: 'i1', name: 'a.png', mimeType: 'image/png' },
    { type: 'file', id: 'f1', name: 'notes.md', mimeType: 'text/markdown' },
    { type: 'image', id: 'i2', name: 'b.png', mimeType: 'image/png' }] };
  const messageId = JSON.stringify(['t1', 'm1']);
  function timeline(calls: Obj[] = []) {
    return { threadId: 't1', generation: 1, environmentId: 'e', origin: 'http://127.0.0.1:1', connection: 'connected',
      projection: { visibleTurnItems: [{ sourceThreadId: 't1', sourceItemId: 'm1', item }] },
      rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ method, ...(payload.resource as Obj) }); return { relativeUrl: `/api/assets/${(payload.resource as Obj).attachmentId}`, expiresAt: 10_000_000 }; } } as unknown as T3Client;
  }
  test('a video is a grid tile after the images, never a file row', () => {
    const { images, attachFiles } = messageAttachments(item);
    expect(images.map(image => [image.id, image.video])).toEqual([['i1', false], ['i2', false], ['v1', true]]);
    expect(attachFiles.map(file => file.id)).toEqual(['f1']);
  });
  test('its inline URL is signed under its video type and never read for an accent', async () => {
    const calls: Obj[] = [];
    memoryFiles(null);
    const accents = serveAssets(() => ({ status: 404 }));
    try {
      const { items } = await attachmentUrls(timeline(calls), { available: true } as Native, 1000);
      expect(items.map(entry => entry.id)).toEqual(['i1', 'i2', 'v1']);
      expect(calls.filter(call => call.method === 'assets.createUrl' && call.attachmentId === 'v1').map(call => [call.mimeType, call.disposition])).toEqual([['video/mp4', 'inline']]);
      expect(accents.calls.map(call => call.url.split('/').pop())).toEqual(['i1', 'i2']);
    } finally { accents.restore(); }
  });
  test('a sent SVG is signed as a picture and read for its accent; one the server does not hand out is not signed', async () => {
    const svgItem = { type: 'user_message', attachments: [{ type: 'file', id: 's1', name: 'badge.svg', mimeType: 'image/svg+xml' },
      { type: 'file', id: 's2', name: 'logo.svg', mimeType: 'application/octet-stream' }, { type: 'file', id: 's3', name: 'gone.svg', mimeType: 'image/svg+xml', downloadable: false }] };
    const calls: Obj[] = [], c = { ...timeline(calls), environmentId: 'e-svg', projection: { visibleTurnItems: [{ sourceThreadId: 't1', sourceItemId: 'm1', item: svgItem }] } } as T3Client;
    memoryFiles(null);
    const accents = serveAssets(() => ({ status: 404 }));
    try {
      const { items } = await attachmentUrls(c, { available: true } as Native, 1000);
      expect(items.map(entry => entry.id)).toEqual(['s1', 's2']);
      expect(calls.filter(call => call.method === 'assets.createUrl').map(call => [call.attachmentId, call.mimeType, call.disposition])).toEqual([
        ['s1', 'image/svg+xml', 'inline'], ['s2', 'image/svg+xml', 'inline']]);
      // Unlike a video's, its picture is read for the chip's accent.
      expect(accents.calls.map(call => call.url.split('/').pop())).toEqual(['s1', 's2']);
    } finally { accents.restore(); }
  });
  test('a video opens alone in the media dialog; images step among the images only', () => {
    const c = timeline();
    imagePreviewAction(c, 'image-open', 'v1', messageId);
    expect(imagePreviewView(c)).toEqual({ imagePreviewId: 'v1', imagePreviewName: 'clip.mp4', imagePreviewPosition: '', imagePreviewPrevious: false, imagePreviewNext: false, imagePreviewVideo: true, imagePreviewSource: 'attachment:v1', imagePreviewTip: 'clip.mp4', imagePreviewReturn: '' });
    imagePreviewAction(c, 'image-open', 'i2', messageId);
    imagePreviewAction(c, 'image-step', '', 'next');
    expect(imagePreviewView(c)).toEqual({ imagePreviewId: 'i1', imagePreviewName: 'a.png', imagePreviewPosition: '(1/2)', imagePreviewPrevious: true, imagePreviewNext: true, imagePreviewVideo: false, imagePreviewSource: 'attachment:i1', imagePreviewTip: 'a.png', imagePreviewReturn: '' });
    imagePreviewAction(c, 'image-close', '', '');
    expect(imagePreviewView(c).imagePreviewId).toBe('');
  });
});

import { codeLines } from '../../src/panel/files-surface';
import { settleHighlights } from '../support/highlights-fixture';
describe('HTML source lines', () => {
  test('every line keeps its own text: tags no longer swallow the text before them', () => {
    const text = '<!doctype html>\n<html><head><title>T</title></head>\n<body>\n<h1>Hi</h1>\n</body></html>';
    expect(codeLines('page.html', text).map(line => line.runs.map(run => run.text).join(''))).toEqual(text.split('\n'));
    settleHighlights(); // the page waits for the lexers (highlights.ts)
    expect(codeLines('page.html', text)[3]!.runs.map(run => [run.text, run.syntax])).toEqual([['<', 'punct'], ['h1', 'tag'], ['>', 'punct'], ['Hi', ''], ['</', 'punct'], ['h1', 'tag'], ['>', 'punct']]);
  });
});
