// Independent regression witnesses for the file-viewer authorization fixes.
import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import type { Native, Files } from './shared/protocol';
import { mobileAttachmentDocument } from './attachment-document';
import { mobileAttachmentMenu, mobileAttachmentDocumentAction } from './attachment-document-actions';
import { answer } from './app';
function fixture(name = 'notes.md', mimeType = 'text/markdown') {
  const client = new T3Client(); client.environmentId = 'env'; client.threadId = 'thread'; client.projectId = 'project';
  client.connection = 'connected'; client.origin = 'https://example.invalid';
  const attachment: Obj = { id: 'file', type: 'file', name, mimeType };
  const item: Obj = { type: 'user_message', attachments: [attachment] };
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1,
    projection: { visibleTurnItems: [{ sourceThreadId: 'thread', sourceItemId: 'message', item }] } };
  let minted = 0; const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: client.generation, value: request.method === 'assets.createUrl'
      ? { relativeUrl: '/signed?ticket=' + ++minted } : request.op === 'mobileDocumentRead'
        ? { identifier: obj(JSON.parse(String(request.sourceJSON))).identifier, base64: btoa('# Notes') } : {} };
  } };
  const files: Files = { fs: { async mkdir() {}, async readFile() { return new ArrayBuffer(0); }, async atomicWriteFile() {} } };
  return { client, native, files, calls, item };
}
const read = (f: ReturnType<typeof fixture>) => mobileAttachmentDocument('transcript', 'file', 'route', true, false, 1000, false, f.native, f.client);
describe('document action independent review regressions', () => {
  test('file actions group rendered modes and keep source-only operations outside that group', async () => {
    const f = fixture(), preview = await read(f);
    const menu = (data: typeof preview) => obj(JSON.parse(mobileAttachmentMenu(data, false).configuration));
    const rows = (data: typeof preview) => menu(data).actions as Obj[];
    expect(menu(preview).identifier).toBe(preview.identifier);
    expect(rows(preview).filter(row => row.inline)).toMatchObject([
      { id: 'preview', title: 'Preview', selected: true }, { id: 'source', selected: false },
    ]);
    expect(rows(preview).filter(row => !row.inline).map(row => row.id)).toEqual(['copy', 'share', 'open-viewer']);
    const source = await mobileAttachmentDocument('transcript', 'file', 'route', false, false, 1000, false, f.native, f.client);
    expect(rows(source).filter(row => row.inline)).toMatchObject([
      { id: 'preview', selected: false }, { id: 'source', selected: true },
    ]);
    expect(rows(source).filter(row => !row.inline).map(row => row.id)).toEqual(['word-wrap', 'copy', 'share', 'open-viewer']);
    const text = await read(fixture('notes.txt', 'text/plain'));
    expect(rows(text).some(row => row.inline)).toBe(false);
    const table = await read(fixture('notes.csv', 'text/csv'));
    expect(rows(table).find(row => row.id === 'preview')).toMatchObject({ title: 'Table', icon: 'tablecells', inline: true });
  });
  test('explicit viewer gets freshly signed bytes and root consumes that descriptor', async () => {
    const f = fixture(), data = await read(f);
    const opened = await mobileAttachmentDocumentAction(JSON.stringify({ identifier: data.identifier, operation: 'open-viewer' }),
      'transcript', 'file', 'route', data, f.native, f.files, f.client);
    expect(opened).toMatchObject({ operation: 'open-viewer', message: '' });
    expect(JSON.parse(opened.sourceJSON).url).not.toBe(data.uri);
    expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(2);
    expect(await answer('attachmentNativePreview', [opened.sourceJSON])).toMatchObject({ identifier: data.identifier, sourceJSON: opened.sourceJSON, ready: true });
  });
  test('a conversation switch during fresh viewer authorization prevents opening', async () => {
    const f = fixture(), data = await read(f), original = f.native.later;
    f.native.later = async input => { const reply = await original(input); if (obj(input).method === 'assets.createUrl') f.client.threadId = 'other'; return reply; };
    const opened = await mobileAttachmentDocumentAction(JSON.stringify({ identifier: data.identifier, operation: 'open-viewer' }),
      'transcript', 'file', 'route', data, f.native, f.files, f.client);
    expect(opened.operation).toBe(''); expect(opened.sourceJSON).toBe('');
  });
  test('HTML error events from another owner cannot affect the current document', async () => {
    expect(await answer('attachmentNativeEvent', [JSON.stringify({ identifier: 'old', message: 'offline' }), 'new']))
      .toMatchObject({ identifier: '', operation: 'native-error' });
    expect(await answer('attachmentNativeEvent', [JSON.stringify({ identifier: 'new', message: 'offline' }), 'new']))
      .toMatchObject({ identifier: 'new', message: 'offline', operation: 'native-error' });
  });
  test('removed attachments do not trigger copy or viewer operations', async () => {
    const f = fixture(), data = await read(f); f.item.attachments = [];
    const result = await mobileAttachmentDocumentAction(JSON.stringify({ identifier: data.identifier, operation: 'copy' }),
      'transcript', 'file', 'route', data, f.native, f.files, f.client);
    expect(result.operation).toBe(''); expect(f.calls.some(call => call.op === 'copyText')).toBe(false);
  });
});
