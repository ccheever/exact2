// `storage.fs.compressImage`'s search on the web (LLP 1069.002 A1.2/A1.7):
// the trials Bluesky's compress.ts makes, recorded in the same table
// `exact_data::image::search` is checked against. `bun test ./host/web/storage-image.test.mjs`.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { compress, contain, headerSize, limits, search, searchLossless, KEEP_SIZE, MAX_SOURCE_PIXELS, pngFirstFrame, svgSize, svgText } from './storage-image.js';

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

test('a PNG shrinks by 0.8 until it fits, at most four times (A1.11)', async () => {
  const trials = [];
  const found = await searchLossless(1000, 800, 1000, 300_000, async (w, h) => { trials.push([w, h]); return { size: w * h / 2, value: w }; });
  expect(trials).toEqual([[1000, 800], [800, 640]]);
  expect(found).toEqual({ value: 800, width: 800, height: 640 });
  trials.length = 0;
  expect(await searchLossless(1000, 800, 1000, 1, async (w, h) => { trials.push([w, h]); return { size: 2, value: null }; })).toBe(null);
  expect(trials).toEqual([[1000, 800], [800, 640], [640, 512], [512, 409], [409, 327]]);
});

test('limits are integers in range, else a TypeError', () => {
  expect(limits({ maxDimension: 4000, maxBytes: 2e6 })).toEqual({ maxDimension: 4000, maxBytes: 2000000, type: 'image/jpeg', maxPixels: MAX_SOURCE_PIXELS });
  expect(limits({ maxDimension: 8, maxBytes: 9, type: 'image/png' })).toEqual({ maxDimension: 8, maxBytes: 9, type: 'image/png', maxPixels: MAX_SOURCE_PIXELS });
  // LLP 1133.006 D1: a PNG with no maxDimension keeps its size; maxPixels bounds the source.
  expect(limits({ maxBytes: 9, type: 'image/png', maxPixels: 64e6 })).toEqual({ maxDimension: KEEP_SIZE, maxBytes: 9, type: 'image/png', maxPixels: 64e6 });
  for (const bad of [{ maxBytes: 9 }, { maxBytes: 9, type: 'image/png', maxPixels: 0 }, { maxBytes: 9, type: 'image/png', maxPixels: 67108865 }, { maxDimension: 0, maxBytes: 9, type: 'image/png' }])
    expect(() => limits(bad)).toThrow(TypeError);
  for (const type of ['image/webp', 'png', null, 7]) expect(() => limits({ maxDimension: 8, maxBytes: 9, type })).toThrow(TypeError);
  for (const bad of [undefined, null, 7, {}, { maxDimension: 0, maxBytes: 1 }, { maxDimension: 8193, maxBytes: 1 },
    { maxDimension: 1.5, maxBytes: 1 }, { maxDimension: 1, maxBytes: 0 }, { maxDimension: 1, maxBytes: 67108865 }, { maxDimension: '10', maxBytes: 1 }])
    expect(() => limits(bad)).toThrow(TypeError);
});

test('the header gives the stored size before anything is decoded', async () => {
  const file = name => new Blob([readFileSync(new URL(`../../scripts/fixtures/picker/${name}`, import.meta.url))]);
  expect(await headerSize(file('oriented-gps.jpg'))).toEqual([64, 48]);
  expect(await headerSize(file('photo.png'))).toEqual([160, 106]);
  expect(await headerSize(file('photo.heic'))).toBe(null); // HEIF: not bounded before decoding on the web
  // The fixture's frame header rewritten to claim 9000 × 9000.
  const jpeg = new Uint8Array(readFileSync(new URL('../../scripts/fixtures/picker/oriented-gps.jpg', import.meta.url)));
  const sof = jpeg.findIndex((b, i) => b === 0xff && jpeg[i + 1] === 0xc0);
  new DataView(jpeg.buffer).setUint16(sof + 5, 9000); new DataView(jpeg.buffer).setUint16(sof + 7, 9000);
  expect(await headerSize(new Blob([jpeg]))).toEqual([9000, 9000]);
  const bytes = (...parts) => new Blob([new Uint8Array(parts.flat())]);
  const le16 = n => [n & 255, n >> 8], le32 = n => [n & 255, (n >> 8) & 255, (n >> 16) & 255, n >>> 24];
  const ascii = s => [...s].map(c => c.charCodeAt(0));
  // GIF: a 1×1 screen whose first image is 9000×9000 is 9000×9000; an
  // extension block before it is skipped.
  const gif = (screen, image) => bytes(ascii('GIF89a'), le16(screen[0]), le16(screen[1]), [0, 0, 0],
    [0x21, 0xf9, 4, 0, 0, 0, 0, 0], [0x2c, 0, 0, 0, 0], le16(image[0]), le16(image[1]), [0, 2, 0]);
  expect(await headerSize(gif([300, 200], [300, 200]))).toEqual([300, 200]);
  expect(await headerSize(gif([1, 1], [9000, 9000]))).toEqual([9000, 9000]);
  expect(await headerSize(gif([0, 0], [0, 0]))).toBe(null);
  // The rectangle of more pixels, not a side from each.
  expect(await headerSize(gif([50000, 2], [2, 40000]))).toEqual([50000, 2]);
  // BMP: signed 32-bit sides, top-down negative; and the 12-byte core header.
  expect(await headerSize(bytes(ascii('BM'), Array(12).fill(0), le32(40), le32(640), le32(-480 >>> 0), Array(8).fill(0)))).toEqual([640, 480]);
  expect(await headerSize(bytes(ascii('BM'), Array(12).fill(0), le32(12), le16(20000), le16(20000), Array(8).fill(0)))).toEqual([20000, 20000]);
  expect(await headerSize(bytes(ascii('BM'), Array(12).fill(0), le32(40), le32(-8 >>> 0), le32(8), Array(8).fill(0)))).toBe(null);
  // WebP: a VP8X canvas and its first frame, the larger of the two.
  const vp8l = (w, h) => { const bits = (w - 1) | ((h - 1) << 14); return [ascii('VP8L'), le32(5), [0x2f], le32(bits)]; };
  const vp8x = (w, h) => [ascii('VP8X'), le32(10), [0, 0, 0, 0], [(w - 1) & 255, ((w - 1) >> 8) & 255, (w - 1) >> 16], [(h - 1) & 255, ((h - 1) >> 8) & 255, (h - 1) >> 16]];
  expect(await headerSize(bytes(ascii('RIFF'), le32(0), ascii('WEBP'), vp8x(8000, 4096).flat(), vp8l(8000, 4096).flat()))).toEqual([8000, 4096]);
  expect(await headerSize(bytes(ascii('RIFF'), le32(0), ascii('WEBP'), vp8x(16, 16).flat(), vp8l(9000, 9000).flat()))).toEqual([9000, 9000]);
  expect(await headerSize(bytes(ascii('RIFF'), le32(0), ascii('WEBP'), vp8l(300, 200).flat(), [0, 0, 0]))).toEqual([300, 200]);
  // An animation: its first frame's own size, not its duration or flags.
  const le24 = n => [n & 255, (n >> 8) & 255, (n >> 16) & 255];
  const anmf = (w, h, duration) => [ascii('ANMF'), le32(16), le24(0), le24(0), le24(w - 1), le24(h - 1), le24(duration), [0]].flat();
  const anim = [ascii('ANIM'), le32(6), [0, 0, 0, 0, 0, 0]].flat();
  expect(await headerSize(bytes(ascii('RIFF'), le32(0), ascii('WEBP'), vp8x(100, 100).flat(), anim, anmf(32, 32, 0xfffff)))).toEqual([100, 100]);
  expect(await headerSize(bytes(ascii('RIFF'), le32(0), ascii('WEBP'), vp8x(16, 16).flat(), anim, anmf(9000, 9000, 100)))).toEqual([9000, 9000]);
  expect(await headerSize(bytes(ascii('not an image at all, not one bit')))).toBe(null);
});

test('a deadline already past decodes nothing', async () => {
  const fixture = new Blob([readFileSync(new URL('../../scripts/fixtures/picker/oriented-gps.jpg', import.meta.url))]);
  let decodes = 0;
  const before = globalThis.createImageBitmap;
  globalThis.createImageBitmap = async () => { decodes++; throw new Error('decoded'); };
  try {
    const error = await compress(fixture, 4000, 2_000_000, 0).then(() => null, e => e);
    expect([error?.code, decodes]).toEqual(['timeout', 0]);
  } finally { globalThis.createImageBitmap = before; }
});

// LLP 1133.006 D1: a kept PNG is tried once at its own size; an APNG is cut to its frame 0; an SVG is sized as an img is.
test('a kept PNG is tried once at its own size, and refuses rather than shrinks', async () => {
  const sizes = [];
  const fit = await searchLossless(16000, 1000, KEEP_SIZE, 100, async (w, h) => { sizes.push([w, h]); return { size: 10, value: 'v' }; });
  expect(sizes).toEqual([[16000, 1000]]);
  expect(fit).toEqual({ value: 'v', width: 16000, height: 1000 });
  expect(await searchLossless(800, 600, KEEP_SIZE, 1, async () => ({ size: 2, value: 'v' }))).toBeNull();
});
test('an APNG whose default image is not part of its animation is cut to its frame 0', () => {
  const b64 = readFileSync(new URL('../../packages/clipboard/test/fixture/images.ts', import.meta.url), 'utf8').match(/apng = '([^']+)'/)[1];
  const apng = Uint8Array.from(atob(b64), c => c.charCodeAt(0)), still = pngFirstFrame(apng);
  const text = String.fromCharCode(...still);
  expect(text.includes('acTL') || text.includes('fcTL') || text.includes('fdAT')).toBe(false);
  expect(text.split('IDAT').length - 1).toBe(1);
  const plain = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  expect(pngFirstFrame(plain)).toBe(plain);
});
test('an SVG is sniffed as a browser does and sized as an img with no CSS size', () => {
  const enc = t => new TextEncoder().encode(t);
  expect(svgText(enc('﻿<?xml version="1.0"?>\n<!-- c --><!DOCTYPE svg><svg viewBox="0 0 4 2">'))).not.toBeNull();
  expect(svgText(enc('<svgx>'))).toBeNull();
  expect(svgSize('<svg width="40" height="20">')).toEqual([40, 20]);
  expect(svgSize('<svg width="1in" viewBox="0 0 2 1">')).toEqual([96, 48]);
  expect(svgSize('<svg viewBox="0 0 30 10">')).toEqual([30, 10]);
  expect(svgSize('<svg width="50%">')).toEqual([300, 150]);
});
