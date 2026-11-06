import { expect, test } from 'bun:test';
import { mobileCacheInspect, mobileCacheRead, mobileCacheRemove, mobileCacheWrite, type MobileCacheKey } from './mobile-client-cache';
import { obj } from './shared/domain';
import type { Native } from './shared/protocol';

const key = { environmentId: 'transfer-env', kind: 'thread', key: 'thread-one' } satisfies MobileCacheKey;
const pieceUnits = 48 * 1_024;
const bytes = (text: string) => new TextEncoder().encode(text).length;
const success = (value: unknown) => ({ ok: true, generation: 0, value });

function uploadPeer(options: { staleAt?: string; failAt?: string } = {}) {
  const calls: Record<string, unknown>[] = [], pieces: string[] = [];
  let target: Record<string, unknown> = {}, declared = 0, committed = '';
  const native: Native = { available: true, watch() {}, async later(raw) {
    const input = obj(raw);
    calls.push(input);
    expect(bytes(JSON.stringify(raw))).toBeLessThan(384 * 1_024);
    if (input.action === options.failAt) throw { name: 'FetchError', kind: 'Aborted' };
    if (input.action === options.staleAt) return success({ written: false, stale: true });
    switch (input.action) {
      case 'textBegin':
        target = obj(input.target); declared = Number(input.units);
        expect(target.payload).toBeUndefined(); expect(target.expectedPayload).toBeUndefined();
        return success({ transfer: { kind: 'upload', id: 'upload-one' } });
      case 'textAppend':
        expect(input.id).toBe('upload-one'); expect(input.index).toBe(pieces.length);
        expect(typeof input.text).toBe('string');
        if (typeof input.text !== 'string') throw new Error('Missing piece');
        pieces.push(input.text);
        return success({ next: pieces.length });
      case 'textFinish':
        committed = pieces.join('');
        expect(input.bytes).toBe(bytes(committed)); expect(declared).toBe(committed.length);
        return success(target.action === 'remove' ? { removed: 1 } : { written: true, stale: false });
      case 'textRelease': return success({});
      default: throw new Error('Unexpected cache operation');
    }
  } };
  return { native, calls, get committed() { return committed; }, get target() { return target; } };
}

test('complete escaped cache text crosses only bounded native frames', async () => {
  const payload = JSON.stringify({ environmentId: key.environmentId, text: '"\\\n'.repeat(460_000) });
  expect(bytes(payload)).toBeLessThan(4 * 1_024 * 1_024);
  expect(bytes(JSON.stringify({ payload }))).toBeGreaterThan(4 * 1_024 * 1_024);
  const peer = uploadPeer();
  expect(await mobileCacheWrite(peer.native, key, 'original-ticket', payload)).toBe(true);
  expect(peer.committed).toBe(payload);
  expect(peer.target).toEqual({ action: 'write', ...key, ticket: 'original-ticket', schemaVersion: 1 });
  expect(peer.calls.at(-1)?.action).toBe('textRelease');
});

test('maximum thread payload and conditional expectation preserve exact text', async () => {
  const prefix = '{"environmentId":"transfer-env","text":"', suffix = '"}';
  const payload = prefix + 'x'.repeat(16 * 1_024 * 1_024 - prefix.length - suffix.length) + suffix;
  const writer = uploadPeer();
  expect(await mobileCacheWrite(writer.native, key, 'ticket', payload)).toBe(true);
  expect(bytes(writer.committed)).toBe(16 * 1_024 * 1_024);
  expect(writer.committed).toBe(payload);
  const remover = uploadPeer();
  expect(await mobileCacheRemove(remover.native, key, payload)).toBe(1);
  expect(remover.committed).toBe(payload);
  expect(remover.target).toEqual({ action: 'remove', ...key });
  const refused = uploadPeer();
  await expect(mobileCacheWrite(refused.native, key, 'ticket', payload + 'x')).rejects.toThrow('invalid reply');
  expect(refused.calls).toHaveLength(0);
});

test('chunk boundaries preserve emoji, normalization and literal JSON escape spelling', async () => {
  const payload = 'x'.repeat(pieceUnits - 1) + '🙂e\u0301é\\u00e9' + 'z'.repeat(pieceUnits);
  const peer = uploadPeer();
  expect(await mobileCacheRemove(peer.native, key, payload)).toBe(1);
  expect(peer.committed).toBe(payload);
  expect(obj(peer.calls[1]).text).toBe('x'.repeat(pieceUnits - 1));
  for (const malformed of ['x'.repeat(pieceUnits) + '\ud800', 'x'.repeat(pieceUnits) + '\udc00']) {
    const bad = uploadPeer();
    await expect(mobileCacheRemove(bad.native, key, malformed)).rejects.toThrow('invalid reply');
    expect(bad.calls.some(call => call.action === 'textFinish')).toBe(false);
    expect(bad.calls.at(-1)?.action).toBe('textRelease');
  }
});

test.each(['textBegin', 'textAppend', 'textFinish'])('stale write at %s settles false without retry', async staleAt => {
  const peer = uploadPeer({ staleAt });
  expect(await mobileCacheWrite(peer.native, key, 'old-ticket', 'x'.repeat(pieceUnits + 1))).toBe(false);
  expect(peer.calls.filter(call => call.action === 'textBegin')).toHaveLength(1);
  if (staleAt !== 'textBegin') expect(peer.calls.at(-1)?.action).toBe('textRelease');
});

test.each(['textBegin', 'textAppend', 'textFinish', 'textRelease'])('let-go at %s makes no later native call', async failAt => {
  const peer = uploadPeer({ failAt });
  await expect(mobileCacheWrite(peer.native, key, 'ticket', 'x'.repeat(pieceUnits + 1))).rejects.toMatchObject({ kind: 'superseded' });
  expect(peer.calls.at(-1)?.action).toBe(failAt);
  expect(peer.calls.filter(call => call.action === 'textFinish').length).toBeLessThanOrEqual(1);
});

function downloadPeer(text: string, kind: 'payload' | 'json' = 'payload', patch: Record<string, unknown> = {},
  partPatch: Record<string, unknown> = {}, failAt = '') {
  const calls: Record<string, unknown>[] = [];
  const chunks: string[] = [];
  for (let offset = 0; offset < text.length;) {
    let end = Math.min(offset + 12 * 1_024, text.length);
    const unit = text.charCodeAt(end - 1);
    if (unit >= 0xd800 && unit <= 0xdbff) end--;
    chunks.push(text.slice(offset, end)); offset = end;
  }
  const metadata = { ...key, schemaVersion: 1, updatedAt: 123 };
  const transfer = { id: 'read-one', kind, units: text.length, bytes: bytes(text), parts: chunks.length,
    ...(kind === 'payload' ? { metadata } : {}), ...patch };
  const native: Native = { available: true, watch() {}, async later(raw) {
    const input = obj(raw); calls.push(input);
    if (input.action === failAt) throw { name: 'FetchError', kind: 'Aborted' };
    if (input.action === 'read' || input.action === 'inspect') return success({ transfer });
    if (input.action === 'textPart') return success({ index: input.index, text: chunks[Number(input.index)], ...partPatch });
    if (input.action === 'textReadFinish') return success({ complete: true });
    if (input.action === 'textRelease') return success({});
    throw new Error('Unexpected cache operation');
  } };
  return { native, calls, metadata };
}

test('outgoing payload and oversized metadata reassemble before public validation', async () => {
  const text = JSON.stringify({ environmentId: key.environmentId, text: '🙂é\\u00e9'.repeat(40_000) });
  const peer = downloadPeer(text);
  expect(await mobileCacheRead(peer.native, key)).toEqual({ ...peer.metadata, payload: text });
  expect(peer.calls.at(-2)?.action).toBe('textReadFinish');
  expect(peer.calls.at(-1)?.action).toBe('textRelease');
  const summary = { rows: [{ environmentId: key.environmentId, kind: 'thread', recordCount: 1, payloadBytes: bytes(text) }],
    recordCount: 1, payloadBytes: bytes(text) };
  expect(await mobileCacheInspect(downloadPeer(JSON.stringify(summary), 'json').native)).toEqual(summary);
});

test('malformed downloads never publish partial records', async () => {
  const text = 'x'.repeat(pieceUnits + 1);
  for (const patch of [{ units: text.length - 1 }, { bytes: bytes(text) + 1 }, { parts: 0 }, { parts: 1_025 },
    { id: '' }, { bytes: 32 * 1_024 * 1_024 + 1 }, { metadata: { ...key, schemaVersion: 2, updatedAt: 1 } }]) {
    await expect(mobileCacheRead(downloadPeer(text, 'payload', patch).native, key)).rejects.toThrow('invalid reply');
  }
  for (const partPatch of [{ index: 100 }, { text: '' }, { text: '\ud800' }, { text: 'x'.repeat(pieceUnits + 1) }]) {
    const peer = downloadPeer(text, 'payload', {}, partPatch);
    await expect(mobileCacheRead(peer.native, key)).rejects.toThrow('invalid reply');
    expect(peer.calls.at(-1)?.action).toBe('textRelease');
    expect(peer.calls.some(call => call.action === 'textReadFinish')).toBe(false);
  }
});

test.each(['textPart', 'textReadFinish', 'textRelease'])('download let-go at %s leaves no further IO', async failAt => {
  const peer = downloadPeer('x'.repeat(pieceUnits + 1), 'payload', {}, {}, failAt);
  await expect(mobileCacheRead(peer.native, key)).rejects.toMatchObject({ kind: 'superseded' });
  expect(peer.calls.at(-1)?.action).toBe(failAt);
});
