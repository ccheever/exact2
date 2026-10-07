import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { type Native } from './shared/protocol';
import { mobileMediaPrepare, mobileMediaShare, mobileMediaForget, mobileMediaURL } from './media-preview';
const imageId = '11111111-1111-4111-a111-111111111111';
function fixture() {
  const client = new T3Client(); client.environmentId = 'env'; client.threadId = 'thread'; client.projectId = 'project';
  client.origin = 'https://environment.example'; client.connection = 'connected';
  const item: Obj = { type: 'user_message', attachments: [{ id: 'pdf', type: 'file', name: 'report.pdf', mimeType: 'application/pdf' },
    { id: 'video', type: 'file', name: 'film.mov', mimeType: 'application/octet-stream' }] };
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1,
    projection: { visibleTurnItems: [{ sourceThreadId: 'thread', sourceItemId: 'message', item }] } };
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: client.generation, value: request.method === 'assets.createUrl' ? { relativeUrl: '/api/assets/signed?cap=actual-server-result' } : {} };
  } };
  return { client, native, calls };
}
const prepare = (f: ReturnType<typeof fixture>, id: string, route = 'route', scope = 'transcript') => mobileMediaPrepare(scope, id, route, f.native, f.client);

describe('mobile media preview ownership', () => {
  test('fresh server authorization on open, first URL kept across resource refresh, explicit retry re-mints', async () => {
    const f = fixture(); const first = await prepare(f, 'pdf');
    expect(first).toMatchObject({ name: 'report.pdf', kind: 'file', ready: true, error: '' });
    expect(JSON.parse(first.sourceJSON)).toMatchObject({ source: 'remote', url: 'https://environment.example/api/assets/signed?cap=actual-server-result' });
    expect(f.calls[0]?.payload).toEqual({ resource: { _tag: 'attachment', attachmentId: 'pdf', fileName: 'report.pdf', mimeType: 'application/pdf', disposition: 'inline' } });
    expect(await prepare(f, 'pdf')).toEqual(first); expect(f.calls).toHaveLength(1);
    mobileMediaForget(first.identifier, f.client); await prepare(f, 'pdf'); expect(f.calls).toHaveLength(2);
    await prepare(f, 'pdf', 'another-route'); expect(f.calls).toHaveLength(3);
  });
  test('composer originals use owned ids and no path/base64/network substitute', async () => {
    const f = fixture(); f.client.connection = 'disconnected';
    f.client.local.snapshotDrafts[f.client.draftKey] = [{ id: imageId, name: 'animation.gif', mimeType: 'image/gif', sizeBytes: 120 }];
    const value = await prepare(f, imageId, 'draft-preview', 'composer');
    expect(value.ready).toBe(true);
    expect(JSON.parse(value.sourceJSON)).toMatchObject({ id: imageId, source: 'draft-image', kind: 'image', url: '' });
    expect(value.sourceJSON).not.toContain('file:'); expect(f.calls).toHaveLength(0);
    f.client.projectId = 'another'; f.client.threadId = '';
    expect((await prepare(f, imageId, 'draft-preview', 'composer')).error).toContain('no longer available');
  });
  test('removed, disconnected and route-owner mismatches do not mint URLs', async () => {
    const f = fixture(); expect((await prepare(f, 'missing')).ready).toBe(false);
    f.client.connection = 'disconnected'; expect((await prepare(f, 'pdf')).error).toContain('Reconnect');
    expect((await mobileMediaPrepare('transcript', 'pdf', 'route', f.native, f.client, 'another', 'thread')).error).toContain('changed');
    expect(f.calls).toHaveLength(0);
  });
  test('late authorization cannot open media after selected thread changes', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => { const result = await original(input); f.client.threadId = 'other'; return result; };
    expect((await prepare(f, 'pdf')).error).toContain('changed');
    expect(f.calls.some(call => call.op === 'mobileMediaShare')).toBe(false);
  });
  test('video type fallback follows shared source helper; share uses prepared original descriptor', async () => {
    const f = fixture(); expect((await prepare(f, 'video')).kind).toBe('video');
    expect(await mobileMediaShare('transcript', 'pdf', 'share-route', f.native, f.client)).toMatchObject({ message: '' });
    const share = f.calls.find(call => call.op === 'mobileMediaShare');
    expect(JSON.parse(String(share?.sourceJSON))).toMatchObject({ name: 'report.pdf', id: 'pdf', source: 'remote' });
  });
  test('local share rechecks the active draft before opening a native sheet', async () => {
    const f = fixture(); f.client.local.snapshotDrafts[f.client.draftKey] = [{ id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg' }];
    const result = mobileMediaShare('composer', imageId, 'route', f.native, f.client);
    f.client.threadId = 'other'; expect((await result).message).toContain('changed'); expect(f.calls).toHaveLength(0);
  });
  test('abandoned native answers propagate without a misleading error snapshot', async () => {
    const f = fixture(); f.native.later = async () => { throw { name: 'FetchError', kind: 'Aborted' }; };
    await expect(prepare(f, 'pdf')).rejects.toMatchObject({ kind: 'superseded' });
  });
  test('URLs refuse embedded credentials and accept actual separately hosted signed assets', () => {
    expect(() => mobileMediaURL('https://environment.example', 'https://user:secret@example.test/media')).toThrow('invalid');
    expect(mobileMediaURL('https://environment.example', 'https://assets.example/file?cap=value')).toBe('https://assets.example/file?cap=value');
    expect(() => mobileMediaURL('https://environment.example', '')).toThrow('invalid');
  });
});
