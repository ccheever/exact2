// `storage.fs.compressImage`'s search on the web (LLP 1069.002 A1.2/A1.7):
// the trials Bluesky's compress.ts makes, recorded in the same table
// `exact_data::image::search` is checked against. `bun test ./host/web/storage-image.test.mjs`.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contain, headerSize, limits, search } from './storage-image.js';

const recorded = JSON.parse(readFileSync(new URL('../../scripts/fixtures/picker/compress-trials.json', import.meta.url), 'utf8'));
const models = {
  'q*1000': (w, h, q) => q * 1000,
  'floor(w*h/4)+q*1000': (w, h, q) => Math.floor(w * h / 4) + q * 1000,
  'w*h': (w, h) => w * h,
  'floor(w*h*q/1000)': (w, h, q) => Math.floor(w * h * q / 1000),
  'floor(w*h*q/250)': (w, h, q) => Math.floor(w * h * q / 250),
};

test('the search makes Bluesky\'s trials', async () => {
  for (const c of recorded.cases) {
    const trials = [];
    const found = await search(c.width, c.height, c.maxDimension, c.maxBytes, async (w, h, q) => {
      trials.push([w, h, q]);
      const size = models[c.model](w, h, q);
      return { size, value: size };
    });
    expect(trials, c.name).toEqual(c.trials);
    expect(found && { width: found.width, height: found.height, size: found.value }, c.name).toEqual(c.result);
  }
});

test('contain floors, never upscales, and keeps a pixel', () => {
  expect(contain(6000, 4000, 4000)).toEqual([4000, 2666]);
  expect(contain(3024, 4032, 2000)).toEqual([1500, 2000]);
  expect(contain(800, 601, 4000)).toEqual([800, 601]);
  expect(contain(9000, 2, 2048)).toEqual([2048, 1]);
});

test('limits are integers in range, else a TypeError', () => {
  expect(limits({ maxDimension: 4000, maxBytes: 2e6 })).toEqual({ maxDimension: 4000, maxBytes: 2000000 });
  for (const bad of [undefined, null, 7, {}, { maxDimension: 0, maxBytes: 1 }, { maxDimension: 8193, maxBytes: 1 },
    { maxDimension: 1.5, maxBytes: 1 }, { maxDimension: 1, maxBytes: 0 }, { maxDimension: 1, maxBytes: 67108865 }, { maxDimension: '10', maxBytes: 1 }])
    expect(() => limits(bad)).toThrow(TypeError);
});

test('the header gives the stored size before anything is decoded', async () => {
  const file = name => new Blob([readFileSync(new URL(`../../scripts/fixtures/picker/${name}`, import.meta.url))]);
  expect(await headerSize(file('oriented-gps.jpg'))).toEqual([64, 48]);
  expect(await headerSize(file('photo.png'))).toEqual([160, 106]);
  expect(await headerSize(file('photo.heic'))).toEqual([320, 214]); // the coded size; its clean aperture shows 213
  // The fixture's frame header rewritten to claim 9000 × 9000.
  const jpeg = new Uint8Array(readFileSync(new URL('../../scripts/fixtures/picker/oriented-gps.jpg', import.meta.url)));
  const sof = jpeg.findIndex((b, i) => b === 0xff && jpeg[i + 1] === 0xc0);
  new DataView(jpeg.buffer).setUint16(sof + 5, 9000); new DataView(jpeg.buffer).setUint16(sof + 7, 9000);
  expect(await headerSize(new Blob([jpeg]))).toEqual([9000, 9000]);
  const bytes = (...parts) => new Blob([new Uint8Array(parts.flat())]);
  const le16 = n => [n & 255, n >> 8], le32 = n => [n & 255, (n >> 8) & 255, (n >> 16) & 255, n >>> 24];
  const ascii = s => [...s].map(c => c.charCodeAt(0));
  expect(await headerSize(bytes(ascii('GIF89a'), le16(300), le16(200), Array(20).fill(0)))).toEqual([300, 200]);
  expect(await headerSize(bytes(ascii('BM'), Array(16).fill(0), le32(640), le32(-480 >>> 0), Array(8).fill(0)))).toEqual([640, 480]);
  expect(await headerSize(bytes(ascii('RIFF'), le32(0), ascii('WEBPVP8X'), le32(10), [0, 0, 0, 0], [0x3f, 0x1f, 0], [0xff, 0x0f, 0]))).toEqual([8000, 4096]);
  expect(await headerSize(bytes(ascii('not an image at all, not one bit')))).toBe(null);
});
