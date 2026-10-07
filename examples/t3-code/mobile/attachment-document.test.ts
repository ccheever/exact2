import { describe, test, expect } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { mobileAttachmentDocument, decodeAttachmentPrefix } from './attachment-document';
import { filePreviewKind, FILE_TEXT_PREVIEW_MAX_BYTES } from './attachment-document-kind';
import { parseDelimitedPreview } from './attachment-document-table';

function fixture(name = 'notes.md', mimeType = 'text/markdown', content = '# A note') {
  const client = new T3Client(); client.environmentId = 'env'; client.threadId = 'thread'; client.projectId = 'project';
  client.origin = 'https://env.example'; client.connection = 'connected';
  const item: Obj = { type: 'user_message', attachments: [{ id: 'file', type: 'file', name, mimeType, sizeBytes: 1025 }] };
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1,
    projection: { visibleTurnItems: [{ sourceThreadId: 'thread', sourceItemId: 'message', item }] } };
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    const value = request.method === 'assets.createUrl' ? { relativeUrl: '/signed?fresh=' + calls.length }
      : { identifier: obj(JSON.parse(String(request.sourceJSON))).identifier, base64: btoa(content) };
    return { ok: true, generation: client.generation, value };
  } };
  return { client, native, calls, item };
}
function read(f: ReturnType<typeof fixture>, rendered = true, retry = false, now = 1000) {
  return mobileAttachmentDocument('transcript', 'file', 'route', rendered, false, now, retry, f.native, f.client);
}
describe('attachment document reads', () => {
  test('audio uses signed original bytes without a text read', async () => {
    const f = fixture('note.m4a', 'audio/mp4'), preview = await read(f);
    expect(preview).toMatchObject({ kind: 'audio', ready: true, hasContent: false, error: '' });
    expect(f.calls.map(call => call.op)).toEqual(['request']);
    expect(JSON.parse(preview.sourceJSON).url).toContain('/signed?');
  });
  test('Markdown and source share one bounded native read; MIME wins and size rounds up', async () => {
    const f = fixture(), preview = await read(f);
    expect(preview).toMatchObject({ ready: true, activeMode: 'markdown', text: '# A note', subtitle: 'Attachment · 2 KB' });
    expect((await read(f, false)).activeMode).toBe('source');
    expect(f.calls.map(call => call.op)).toEqual(['request', 'mobileDocumentRead']);
    expect(filePreviewKind({ name: 'table.csv', mimeType: 'application/pdf' })).toBe('pdf');
  });
  test('HTML preview keeps its initial URL, while source reauthorizes after five minutes', async () => {
    const f = fixture('index.html', 'text/html', '<p>Hello</p>');
    const preview = await read(f); expect(preview.activeMode).toBe('html'); expect(f.calls).toHaveLength(1);
    const source = await read(f, false, false, 302_000);
    expect(source.uri).toBe(preview.uri); expect(source.text).toBe('<p>Hello</p>');
    expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(2);
    expect(JSON.parse(String(f.calls.at(-1)?.sourceJSON)).url).not.toBe(preview.uri);
  });
  test('retry remints URL and clears a failed text read', async () => {
    const f = fixture(), original = f.native.later;
    let broken = true;
    f.native.later = async input => obj(input).op === 'mobileDocumentRead' && broken
      ? { ok: false, generation: 0, error: { kind: 'Attachment', message: 'offline' } } : original(input);
    expect((await read(f)).error).toBe('offline'); broken = false;
    expect((await read(f, true, true)).text).toBe('# A note');
    expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(2);
  });
  test('removing an attachment during the read discards the result', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => { const result = await original(input); if (obj(input).op === 'mobileDocumentRead') f.item.attachments = []; return result; };
    await expect(read(f)).rejects.toMatchObject({ kind: 'superseded' });
  });
  test('changing thread during the read discards the result', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => { const result = await original(input); if (obj(input).op === 'mobileDocumentRead') f.client.threadId = 'other'; return result; };
    await expect(read(f)).rejects.toMatchObject({ kind: 'superseded' });
  });
  test('a newer retry cannot be overwritten by an older completion', async () => {
    const f = fixture(), original = f.native.later;
    let release!: () => void, started!: () => void;
    const pending = new Promise<void>(resolve => { release = resolve; });
    const began = new Promise<void>(resolve => { started = resolve; });
    let reads = 0;
    f.native.later = async input => { const result = await original(input); if (obj(input).op === 'mobileDocumentRead' && ++reads === 1) { started(); await pending; } return result; };
    const older = read(f); await began;
    expect((await read(f, true, true)).error).toBe(''); release();
    await expect(older).rejects.toMatchObject({ kind: 'superseded' });
  });
  test('abandonment propagates and does not poison the document cache', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => { if (obj(input).op === 'mobileDocumentRead') throw { name: 'FetchError', kind: 'Aborted' }; return original(input); };
    await expect(read(f)).rejects.toMatchObject({ kind: 'superseded' });
    f.native.later = original; expect((await read(f)).error).toBe('');
  });
  test('UTF-8 boundaries, binary and oversized bridges use the pinned policy', () => {
    expect(decodeAttachmentPrefix({ base64: '' })).toEqual({ text: '', truncated: false });
    expect(() => decodeAttachmentPrefix({ base64: btoa('a\0b') })).toThrow('binary');
    expect(() => decodeAttachmentPrefix({ base64: btoa('\xff') })).toThrow('UTF-8');
    const prefix = 'a'.repeat(FILE_TEXT_PREVIEW_MAX_BYTES - 1) + '\xe2\x82';
    const result = decodeAttachmentPrefix({ base64: btoa(prefix) });
    expect(result.truncated).toBe(true); expect(result.text.length).toBe(FILE_TEXT_PREVIEW_MAX_BYTES - 1);
    expect(() => decodeAttachmentPrefix({ base64: 'a'.repeat(Math.ceil((FILE_TEXT_PREVIEW_MAX_BYTES + 1) / 3) * 4 + 1) })).toThrow('byte limit');
  });
  test('CSV is quoted correctly and independently limited by rows, columns and cells', async () => {
    const f = fixture('items.csv', 'text/csv', 'Name,Note\r\n"a,b","line one\nline two"');
    const result = await read(f); expect(result.activeMode).toBe('table');
    expect(result.table[1]?.cells.map(cell => cell.text)).toEqual(['a,b', 'line one\nline two']);
    const rows = parseDelimitedPreview(Array(102).fill(Array(32).fill('x'.repeat(2001)).join(',')).join('\n'), ',');
    expect(rows.truncated).toBe(true); expect(rows.rows.length).toBe(100);
    expect(rows.rows[0]?.length).toBe(30); expect(rows.rows[0]?.[0]?.length).toBe(2000);
  });
});
