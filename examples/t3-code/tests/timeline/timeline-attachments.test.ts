import { describe, expect, test } from 'bun:test';
import { attachmentUrls, formatBytes, imagePreviewAction, imagePreviewView, messageAttachments, userMessageShape } from '../../src/timeline/timeline-attachments';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
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
    expect(attachFiles).toEqual([{ id: 'f1', name: 'notes.md', icon: 'markdown', label: 'notes.md', detail: 'MD · 0 B', note: '', sending: false }]);
  });
  // ImageChipButton at v0.1.5 (#26306) takes the image kind's blue accent, so no picture is fetched for a tint.
  test('signed inline URLs come from assets.createUrl and are reused until they near expiry; no image is read for an accent', async () => {
    const calls: Obj[] = [], c = client(calls), native = { available: true } as Native;
    const server = serveAssets(() => ({ body: 'image bytes' }));
    try {
      const items = (await attachmentUrls(c, native, 1000)).items;
      expect(items).toEqual([{ id: 'a1', url: 'http://127.0.0.1:1/api/assets/a1' }, { id: 'a2', url: 'http://127.0.0.1:1/api/assets/a2' }]);
      expect(calls[0]).toMatchObject({ method: 'assets.createUrl', resource: { _tag: 'attachment', attachmentId: 'a1', fileName: 'shot.png', mimeType: 'image/png', disposition: 'inline' } });
      await attachmentUrls(c, native, 2000);
      expect(calls).toHaveLength(2);
      expect(server.calls).toHaveLength(0);
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

describe('a sent sound is an audio chip (#21363)', () => {
  test('an audio file plays in its chip; one the server will not serve stays a file row', async () => {
    const { messageAttachments, isSentAudio } = await import('../../src/timeline/timeline-attachments');
    const attached = messageAttachments({ attachments: [
      { type: 'file', id: 'a', name: 'memo.mp3', mimeType: 'audio/mpeg' },
      { type: 'file', id: 'b', name: 'take.m4a', mimeType: 'application/octet-stream' },
      { type: 'file', id: 'c', name: 'locked.wav', mimeType: 'audio/wav', downloadable: false },
      { type: 'file', id: 'd', name: 'notes.md', mimeType: 'text/markdown' },
    ] });
    expect(attached.audio.map(file => file.id)).toEqual(['a', 'b']);
    expect(attached.attachFiles.map(file => file.id)).toEqual(['c', 'd']);
    expect(isSentAudio({ name: 'clip.mp4', mimeType: 'video/mp4' })).toBe(false);
  });
  test('the MIME type is read without its parameters, and text/plain is as generic as none (filePreviewKind)', async () => {
    const { isSentAudio } = await import('../../src/timeline/timeline-attachments');
    expect([
      { name: 'memo.mp3', mimeType: 'text/plain' },
      { name: 'take.wav', mimeType: 'application/octet-stream; charset=binary' },
      { name: 'voice.ogg', mimeType: 'Audio/Ogg; codecs=opus' },
      { name: 'notes.txt', mimeType: 'text/plain' },
      { name: 'memo.mp3', mimeType: 'text/markdown' },
    ].map(isSentAudio)).toEqual([true, true, true, false, false]);
  });
  test('a live or unknown length reads as none, not "Infinity:NaN" (AudioPreview sets 0)', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const chip = contractComponent('timeline-audio.contract', 'AudioPreview'), state = chip.state();
    for (const [seconds, duration] of [[Infinity, 0], [Number.NaN, 0], [-1, 0], [92.5, 92.5]] as const) {
      chip.call('measured', state, {}, [seconds]);
      expect(state.duration).toBe(duration);
    }
  });
  test('the chip tells its row when its sound starts and stops, once each, so the transcript keeps the row mounted', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const chip = contractComponent('timeline-audio.contract', 'AudioPreview'), state = { ...chip.state(), id: 'memo.wav' }, reports: string[] = [];
    const scope = { report: (value: string) => reports.push(value) };
    chip.call('stopped', state, scope, []); // a paused chip's own pause tells nothing
    chip.call('started', state, scope, []);
    chip.call('started', state, scope, []);
    chip.call('stopped', state, scope, []);
    chip.call('started', state, scope, []);
    chip.call('refused', state, scope, ['MEDIA_ERR_DECODE']);
    expect(reports).toEqual(['1memo.wav', '0memo.wav', '1memo.wav', '0memo.wav']);
  });
});

// TimelineUserMessageRow at v0.1.6: what a message holds decides its shape (#26769, #27306, #27324, #27377).
describe('a sent message takes the shape of what it holds', () => {
  const image = (id: string, extra: Obj = {}) => ({ type: 'image', id, name: `${id}.png`, mimeType: 'image/png', ...extra });
  const file = (id: string, name: string, extra: Obj = {}) => ({ type: 'file', id, name, mimeType: 'application/octet-stream', sizeBytes: 1_234, ...extra });
  const ref = (kind: string, contextId: string, label: string) => `${kind === 'image' ? '!' : ''}[${label}](t3-context://v1/${kind}/${contextId})`;
  const shape = (attachments: Obj[], text = '', records: Obj[] = [], chipped: string[] = [], folded = false) => {
    const message = { type: 'user_message', attachments, context: { records } };
    return userMessageShape(message, text, messageAttachments(message), new Set(chipped), folded);
  };
  test('images sent alone show without a bubble: a lone image at its own shape, several as one collage', () => {
    expect(shape([image('a')])).toMatchObject({ shape: 'media', mediaAbove: false, layout: 'lone' });
    expect(shape([image('a'), image('b'), image('c')])).toMatchObject({ shape: 'media', layout: 'collage' });
    expect(shape([image('a'), image('b', { source: { kind: 'snap-shot', appName: 'Safari' } })])).toMatchObject({ shape: 'media', layout: 'stack' });
    expect(shape([image('a'), file('v', 'clip.mp4', { mimeType: 'video/mp4' })])).toMatchObject({ shape: 'media', layout: 'stack' });
  });
  test('media sent with words sits above the bubble; a folded message keeps its grid', () => {
    expect(shape([image('a')], 'Look at this')).toMatchObject({ shape: '', mediaAbove: true, layout: 'lone' });
    expect(shape([image('a')], 'From another agent', [], [], true)).toMatchObject({ shape: '', mediaAbove: false, layout: '' });
  });
  test('files sent without words are cards; with words or beside media they stay rows', () => {
    const records = [{ contextId: 'c1', kind: 'file', attachmentId: 'f1', label: 'notes.md' }];
    expect(shape([file('f1', 'notes.md')], ref('file', 'c1', 'notes.md'), records, ['f1'])).toMatchObject({ shape: 'files' });
    expect(shape([file('f1', 'notes.md')], `Read ${ref('file', 'c1', 'notes.md')}`, records, ['f1'])).toMatchObject({ shape: '' });
    expect(shape([file('f1', 'notes.md')], `${ref('file', 'c1', 'notes.md')}`, records, ['f1']).files.map(entry => entry.id)).toEqual(['f1']);
  });
  test('a voice note alone is a bubble-shaped audio chip', () => {
    expect(shape([file('s1', 'memo.m4a', { mimeType: 'audio/mp4' })])).toMatchObject({ shape: 'audio' });
    expect(shape([file('s1', 'memo.m4a', { mimeType: 'audio/mp4' })], 'Listen')).toMatchObject({ shape: '' });
  });
  test('Copy on a wordless media message takes its media\'s references', () => {
    const records = [{ contextId: 'i1', kind: 'image', attachmentId: 'a', label: 'a.png' }, { contextId: 'i2', kind: 'image', attachmentId: 'b', label: 'b.png' }];
    expect(shape([image('a'), image('b')], '', records).copy).toBe(`${ref('image', 'i1', 'a.png')}\n${ref('image', 'i2', 'b.png')}`);
    expect(shape([image('a')], 'Words', records).copy).toBe('');
  });
  test('pasted text reads "Pasted text" with its size; a file still sending says so', () => {
    const [pasted, sending, plain] = messageAttachments({ attachments: [file('p', 'pasted-text.txt', { source: { _tag: 'pasted-text' }, sizeBytes: 40_960 }),
      file('s', 'big.zip', { downloadable: false }), file('d', 'report.pdf')] }).attachFiles;
    expect(pasted).toMatchObject({ label: 'Pasted text', detail: '40.0 KB', note: '40.0 KB', sending: false });
    expect(sending).toMatchObject({ label: 'big.zip', detail: 'Sending…', note: 'Sending…', sending: true });
    expect(plain).toMatchObject({ label: 'report.pdf', detail: 'PDF · 1.21 KB', note: '' });
  });
  test('byte sizes pick their unit after rounding (lib/formatBytes.ts)', () => {
    expect([500, 1_024, 1_048_000, 1_048_500, 5_000_000].map(formatBytes)).toEqual(['500 B', '1.00 KB', '1023 KB', '1.00 MB', '4.77 MB']);
  });
});
