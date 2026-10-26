// An image chip's accent (image-accent.ts, png-decode.ts): the alpha-weighted mean of the picture, read from the PNG
// compressImage scales it to, within one unit per channel of what the former native module read.
import { describe, expect, test } from 'bun:test';
import { deflateSync } from 'node:zlib';
import { averageAccent, imageAccent, pngAccent, ACCENT_SCRATCH } from '../../src/media/image-accent';
import { decodePng, inflate } from '../../src/media/png-decode';
import { adoptAppFiles } from '../../src/media/save-file';
import { letGo } from '../../src/client/let-go';
import { ACCENT_FIXTURES, PHOTO_PNG, memoryFiles } from '../support/image-accent-fixture';
import { serveAssets } from '../support/asset-server';

const bytes = (base64: string) => new Uint8Array(Buffer.from(base64, 'base64'));
const channels = (accent: string) => accent.split(' ').map(Number);

/** A PNG of `rows` (each row's filter type, then its bytes), deflated at `level`; CRCs are zero (they are not read). */
function png(width: number, height: number, depth: number, type: number, rows: number[][], options: { level?: number; chunks?: [string, number[]][]; interlace?: number } = {}): Uint8Array {
  const chunk = (name: string, body: Uint8Array | number[]) => {
    const data = new Uint8Array(body), out = new Uint8Array(12 + data.length), view = new DataView(out.buffer);
    view.setUint32(0, data.length); out.set([...name].map(char => char.charCodeAt(0)), 4); out.set(data, 8);
    return out;
  };
  const header = new Uint8Array(13), view = new DataView(header.buffer);
  view.setUint32(0, width); view.setUint32(4, height); header.set([depth, type, 0, 0, options.interlace ?? 0], 8);
  const idat = deflateSync(new Uint8Array(rows.flat()), { level: options.level ?? 6 });
  const parts = [new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', header),
    ...(options.chunks ?? []).map(([name, body]) => chunk(name, body)), chunk('IDAT', idat), chunk('IEND', [])];
  const out = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  parts.reduce((at, part) => { out.set(part, at); return at + part.length; }, 0);
  return out;
}
const pixels = (data: Uint8Array) => Array.from(data);

describe('the accent', () => {
  test('of each test image is within one unit per channel of the native one; a transparent image has none', () => {
    for (const fixture of ACCENT_FIXTURES) {
      const accent = pngAccent(bytes(fixture.scaled));
      if (!fixture.native) { expect(accent).toBe(''); continue; }
      const got = channels(accent), want = channels(fixture.native);
      expect({ name: fixture.name, within: got.every((value, i) => Math.abs(value - want[i]!) <= 1) }).toEqual({ name: fixture.name, within: true });
    }
  });

  test('of photo.png decoded whole is Chrome\'s rgb(129 140 168)', () => {
    const decoded = decodePng(bytes(PHOTO_PNG))!;
    expect([decoded.width, decoded.height]).toEqual([320, 200]);
    expect(averageAccent(decoded.rgba)).toBe('129 140 168');
  });

  test('weights each pixel by its alpha, so transparent pixels do not darken it', () => {
    expect(averageAccent(new Uint8Array([255, 0, 0, 255, 0, 0, 0, 0]))).toBe('255 0 0');
    expect(averageAccent(new Uint8Array([255, 0, 0, 255, 0, 0, 255, 85]))).toBe('191 0 64');
    expect(averageAccent(new Uint8Array([10, 20, 30, 0]))).toBe('');
  });
});

describe('the PNG decoder', () => {
  test('reads every colour type and depth, with tRNS, as 8-bit RGBA', () => {
    expect(pixels(decodePng(png(2, 1, 8, 2, [[0, 255, 0, 0, 0, 0, 255]]))!.rgba)).toEqual([255, 0, 0, 255, 0, 0, 255, 255]);
    expect(pixels(decodePng(png(2, 1, 8, 2, [[0, 255, 0, 0, 0, 0, 255]], { chunks: [['tRNS', [0, 0, 0, 0, 0, 255]]] }))!.rgba)).toEqual([255, 0, 0, 255, 0, 0, 255, 0]);
    expect(pixels(decodePng(png(1, 1, 16, 6, [[0, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0x80, 0]]))!.rgba)).toEqual([0x12, 0x56, 0x9a, 0x80]);
    expect(pixels(decodePng(png(3, 1, 2, 0, [[0, 0b00011011]]))!.rgba)).toEqual([0, 0, 0, 255, 85, 85, 85, 255, 170, 170, 170, 255]);
    expect(pixels(decodePng(png(2, 1, 8, 4, [[0, 200, 255, 100, 0]]))!.rgba)).toEqual([200, 200, 200, 255, 100, 100, 100, 0]);
    expect(pixels(decodePng(png(1, 1, 16, 0, [[0, 0x00, 0x07]], { chunks: [['tRNS', [0x00, 0x07]]] }))!.rgba)).toEqual([0, 0, 0, 0]);
    const palette: [string, number[]][] = [['PLTE', [255, 0, 0, 0, 255, 0]], ['tRNS', [128]]];
    expect(pixels(decodePng(png(3, 1, 1, 3, [[0, 0b01000000]], { chunks: palette }))!.rgba)).toEqual([255, 0, 0, 128, 0, 255, 0, 255, 255, 0, 0, 128]);
  });

  test('undoes all five filters, from stored, fixed and dynamic blocks', () => {
    // Two rows of two RGB pixels, (10 20 30) (40 50 60) over (70 80 90) (100 110 120), filtered each way.
    const want = [10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255];
    const first = [10, 20, 30, 40, 50, 60], second = [70, 80, 90, 100, 110, 120];
    const sub = (row: number[]) => row.map((value, i) => (value - (i >= 3 ? row[i - 3]! : 0)) & 255);
    const up = second.map((value, i) => (value - first[i]!) & 255);
    const average = second.map((value, i) => (value - (((i >= 3 ? second[i - 3]! : 0) + first[i]!) >> 1)) & 255);
    const paeth = second.map((value, i) => {
      const a = i >= 3 ? second[i - 3]! : 0, b = first[i]!, c = i >= 3 ? first[i - 3]! : 0, p = a + b - c;
      const predicted = Math.abs(p - a) <= Math.abs(p - b) && Math.abs(p - a) <= Math.abs(p - c) ? a : Math.abs(p - b) <= Math.abs(p - c) ? b : c;
      return (value - predicted) & 255;
    });
    for (const [filter, row] of [[1, sub(second)], [2, up], [3, average], [4, paeth]] as [number, number[]][]) {
      for (const level of [0, 1, 9]) expect(pixels(decodePng(png(2, 2, 8, 2, [[1, ...sub(first)], [filter, ...row]], { level }))!.rgba)).toEqual(want);
    }
  });

  test('reads a long run, whose copies reach back across the stream', () => {
    const row = Array.from({ length: 3 * 64 }, (_, i) => i % 3 === 0 ? 200 : 7);
    const decoded = decodePng(png(64, 64, 8, 2, Array.from({ length: 64 }, () => [0, ...row]), { level: 9 }))!;
    expect(averageAccent(decoded.rgba)).toBe('200 7 7');
    expect(inflate(deflateSync(new Uint8Array(70000).fill(3)), 70000).every(byte => byte === 3)).toBe(true);
  });

  test('refuses what it does not read: not a PNG, an interlaced or truncated one, a bad stream, too many pixels', () => {
    const good = png(2, 1, 8, 2, [[0, 255, 0, 0, 0, 0, 255]]);
    expect(decodePng(new TextEncoder().encode('not an image'))).toBeNull();
    expect(decodePng(png(2, 1, 8, 2, [[0, 255, 0, 0, 0, 0, 255]], { interlace: 1 }))).toBeNull();
    expect(decodePng(good.subarray(0, good.length - 20))).toBeNull();
    expect(decodePng(png(2, 1, 8, 2, [[0, 255, 0, 0]]))).toBeNull();
    expect(decodePng(png(2, 1, 8, 2, [[5, 255, 0, 0, 0, 0, 255]]))).toBeNull();
    expect(decodePng(png(2, 1, 3, 2, [[0, 255, 0, 0, 0, 0, 255]]))).toBeNull();
    expect(decodePng(png(2, 1, 8, 2, [[0, 255, 0, 0, 0, 0, 255]]), 1)).toBeNull();
    // The accent reads at most compressImage's 128 × 128.
    expect(pngAccent(png(129, 128, 8, 0, Array.from({ length: 128 }, () => [0, ...new Array(129).fill(9)])))).toBe('');
    expect(pngAccent(png(128, 128, 8, 0, Array.from({ length: 128 }, () => [0, ...new Array(128).fill(9)])))).toBe('9 9 9');
    expect(() => inflate(new Uint8Array([0x78, 0x9c, 0xff, 0xff]), 4)).toThrow();
  });
});

describe('imageAccent', () => {
  test('fetches the image, scales it to a 128 px PNG under app:/tmp, reads its accent and leaves nothing behind', async () => {
    const { files, scaledFrom } = memoryFiles(bytes(ACCENT_FIXTURES[0]!.scaled));
    const server = serveAssets(() => ({ body: bytes(PHOTO_PNG) }));
    try {
      expect(await imageAccent('http://127.0.0.1:1/api/assets/a1?token=x', letGo)).toBe('129 140 168');
      expect(server.calls.map(call => call.url)).toEqual(['http://127.0.0.1:1/api/assets/a1?token=x']);
      expect(scaledFrom).toEqual(['128 image/png']);
      expect([...files.keys()]).toEqual([]);
    } finally { server.restore(); }
  });

  test('an image that cannot be fetched or read, or is fully transparent, keeps the kind\'s own tone for good', async () => {
    const transparent = bytes(ACCENT_FIXTURES.find(fixture => fixture.name === 'transparent')!.scaled);
    for (const [scaled, answer] of [[transparent, { body: bytes(PHOTO_PNG) }], [null, { body: 'not an image' }], [transparent, { status: 404 }]] as const) {
      const { files } = memoryFiles(scaled);
      const server = serveAssets(() => answer);
      try {
        expect(await imageAccent('https://example.com/a.png', letGo)).toBe('');
        expect([...files.keys()]).toEqual([]);
      } finally { server.restore(); }
    }
    memoryFiles(transparent);
    const server = serveAssets(() => ({ body: bytes(PHOTO_PNG) }));
    try {
      expect(await imageAccent('file:///tmp/a.png', letGo)).toBe('');
      expect(server.calls).toEqual([]);
    } finally { server.restore(); }
  });

  test('a passing failure (the network, a timeout, a full storage queue) answers null, so the read is asked again', async () => {
    const photo = bytes(ACCENT_FIXTURES[0]!.scaled);
    const offline = serveAssets(() => new Error('offline'));
    try { memoryFiles(photo); expect(await imageAccent('https://example.com/a.png', letGo)).toBeNull(); } finally { offline.restore(); }
    const server = serveAssets(() => ({ body: bytes(PHOTO_PNG) }));
    try {
      const { files } = memoryFiles(null, 'timeout');
      expect(await imageAccent('https://example.com/a.png', letGo)).toBeNull();
      expect([...files.keys()]).toEqual([]);
      adoptAppFiles({ fs: { async mkdir() {}, async rm() {}, async readFile() { throw new Error('ENOENT'); },
        async writeFile() { throw Object.assign(new Error('the storage queue is full'), { kind: 'Unavailable', code: 'full' }); } } });
      expect(await imageAccent('https://example.com/a.png', letGo)).toBeNull();
    } finally { server.restore(); }
  });

  test('a let-go answer\'s fetch goes on up, not cached as no accent', async () => {
    memoryFiles(null);
    const server = serveAssets(() => Object.assign(new Error('the answer was let go before this reply'), { name: 'FetchError', kind: 'Aborted' }));
    try {
      await expect(imageAccent('https://example.com/a.png', letGo)).rejects.toMatchObject({ kind: 'Aborted' });
    } finally { server.restore(); }
  });

  test('writes only where app.ts grants', async () => {
    const { grants } = await import('../../app');
    expect(grants.split('\n')).toEqual(expect.arrayContaining([`fs.read ${ACCENT_SCRATCH}`, `fs.write ${ACCENT_SCRATCH}`]));
  });
});
