// `storage.fs.compressImage` on the web (LLP 1069.002 A1.3): the browser
// decodes (`createImageBitmap`, orientation applied), a canvas scales onto
// white, and `convertToBlob` encodes each quality trial. Loaded by
// storage-fs.js on the first call, never before first pixel. The search is
// Bluesky's (A1.2), the same trials as `exact_data::image::search`
// (scripts/fixtures/picker/compress-trials.json).

// The platform's clock as storage-environment.js captured it, before a
// worker realm refuses ambient time to app code (LLP 1027.002 D2).
import { now } from './storage-environment.js';

export const MAX_DIMENSION = 8192;
export const MAX_BYTES = 64 * 1024 * 1024; // also the largest source
export const MAX_SOURCE_PIXELS = 64 * 1024 * 1024;
const DEADLINE_MS = 20_000;
const MAX_SHRINKS = 4;

export function failure(code, detail) {
  const error = new Error(`filesystem: compressImage: ${code}: ${detail}`);
  error.kind = 'Unavailable';
  error.code = code;
  return error;
}

/** The options as A1.1 bounds them, or a TypeError. */
export function limits(options) {
  const api = 'storage.fs.compressImage()';
  if (!options || typeof options !== 'object') throw new TypeError(`${api}: options must be {maxDimension, maxBytes}`);
  const { maxDimension, maxBytes } = options;
  if (!Number.isInteger(maxDimension) || maxDimension < 1 || maxDimension > MAX_DIMENSION)
    throw new TypeError(`${api}: maxDimension must be an integer from 1 to ${MAX_DIMENSION}`);
  if (!Number.isInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_BYTES)
    throw new TypeError(`${api}: maxBytes must be an integer from 1 to ${MAX_BYTES}`);
  return { maxDimension, maxBytes };
}

/** `containImageRes`: never larger than itself, each side floored, at least 1. */
export function contain(width, height, max) {
  if (width <= max && height <= max) return [width, height];
  const scale = width > height ? max / width : max / height;
  return [Math.max(1, Math.floor(width * scale)), Math.max(1, Math.floor(height * scale))];
}

/** Bluesky's search. `encode(w, h, quality)` resolves `{size, value}`; the
 * best fit is `{value, width, height}`, or null when nothing fits. */
export async function search(width, height, maxDimension, maxBytes, encode) {
  let dimension = maxDimension, lo = 0, hi = 101, shrinks = 0, best = null;
  while (hi - lo > 1) {
    if (shrinks >= MAX_SHRINKS) break;
    const [w, h] = contain(width, height, dimension);
    const quality = Math.round((hi + lo) / 2);
    if (quality <= 13) {
      lo = 0; hi = 101; shrinks++;
      dimension = Math.floor(dimension * 0.8);
      continue;
    }
    const { size, value } = await encode(w, h, quality);
    if (size <= maxBytes) { lo = quality; best = { value, width: w, height: h }; }
    else hi = quality;
  }
  return best;
}

// The pixel size from the header, read before anything is decoded (A1.5),
// or null: the browser never decodes a file whose size is unknown here or
// over the limit. JPEG (its frame header), PNG, GIF (the larger of its
// screen and first image), WebP (its canvas and first frame), BMP. Not
// HEIF or AVIF: their container's size need not be the coded frame's, and
// a browser may decode the frame before checking (Firefox), so their size
// cannot be bounded before decoding.
const HEADER_BYTES = 256 * 1024; // exact_raster::MAX_HEADER_BYTES
export async function headerSize(blob) {
  const v = new DataView(await blob.slice(0, HEADER_BYTES).arrayBuffer());
  const n = v.byteLength, u8 = i => v.getUint8(i);
  const ascii = (i, k) => i + k <= n ? String.fromCharCode(...Array.from({ length: k }, (_, j) => u8(i + j))) : '';
  const le24 = i => u8(i) | u8(i + 1) << 8 | u8(i + 2) << 16;
  const positive = size => size && size.every(side => Number.isInteger(side) && side > 0) ? size : null;
  const larger = (a, b) => a && b ? [Math.max(a[0], b[0]), Math.max(a[1], b[1])] : null;
  if (n < 12) return null;
  if (u8(0) === 0x89 && ascii(1, 3) === 'PNG' && ascii(12, 4) === 'IHDR' && n >= 24) return positive([v.getUint32(16), v.getUint32(20)]);
  if (ascii(0, 6) === 'GIF87a' || ascii(0, 6) === 'GIF89a') {
    const screen = [v.getUint16(6, true), v.getUint16(8, true)];
    let pos = 13 + (u8(10) & 0x80 ? 3 * (2 << (u8(10) & 7)) : 0);
    while (pos < n) {
      const block = u8(pos);
      if (block === 0x2c && pos + 9 < n) return positive(larger(screen, [v.getUint16(pos + 5, true), v.getUint16(pos + 7, true)]));
      if (block !== 0x21 || pos + 2 >= n) return null;
      pos += 2; // an extension's introducer and label, then its sub-blocks
      while (pos < n && u8(pos)) pos += 1 + u8(pos);
      pos += 1;
    }
    return null;
  }
  if (ascii(0, 2) === 'BM' && n >= 26) {
    // A 12-byte core header has 16-bit sides; the others signed 32-bit
    // ones, a negative height meaning top-down.
    if (v.getUint32(14, true) === 12) return positive([v.getUint16(18, true), v.getUint16(20, true)]);
    return positive([Math.abs(v.getInt32(18, true)), Math.abs(v.getInt32(22, true))]);
  }
  if (ascii(0, 4) === 'RIFF' && ascii(8, 4) === 'WEBP') {
    const frame = pos => {
      const kind = ascii(pos, 4);
      if (kind === 'VP8 ' && pos + 18 <= n) return [v.getUint16(pos + 14, true) & 0x3fff, v.getUint16(pos + 16, true) & 0x3fff];
      if (kind === 'VP8L' && pos + 13 <= n) { const bits = v.getUint32(pos + 9, true); return [(bits & 0x3fff) + 1, ((bits >>> 14) & 0x3fff) + 1]; }
      return null;
    };
    if (ascii(12, 4) !== 'VP8X') return positive(frame(12));
    if (n < 30) return null;
    const canvas = [le24(24) + 1, le24(27) + 1];
    // The first frame, still or animated, must be read too.
    for (let pos = 30; pos + 8 <= n;) {
      const kind = ascii(pos, 4), size = v.getUint32(pos + 4, true);
      if (kind === 'VP8 ' || kind === 'VP8L') return positive(larger(canvas, frame(pos)));
      // ANMF: X, Y, width - 1, height - 1, duration (24 bits each), flags.
      if (kind === 'ANMF' && pos + 24 <= n) return positive(larger(canvas, [le24(pos + 14) + 1, le24(pos + 17) + 1]));
      pos += 8 + size + (size & 1);
    }
    return null;
  }
  if (u8(0) === 0xff && u8(1) === 0xd8) {
    const sof = new Set([0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf]);
    for (let pos = 2; pos + 9 <= n;) {
      if (u8(pos) !== 0xff) return null;
      const marker = u8(pos + 1);
      if (marker === 0xff) { pos += 1; continue; }
      if (sof.has(marker)) return positive([v.getUint16(pos + 7), v.getUint16(pos + 5)]);
      if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd8)) { pos += 2; continue; }
      if (marker === 0xd9 || marker === 0xda) return null; // image data before any frame header
      pos += 2 + v.getUint16(pos + 2);
    }
    return null;
  }
  return null;
}

async function decode(blob) {
  try { return await createImageBitmap(blob, { imageOrientation: 'from-image' }); }
  catch (error) {
    // An engine that predates the `from-image` value refuses the option
    // itself; its default applies EXIF orientation.
    if (error?.name === 'TypeError') {
      try { return await createImageBitmap(blob); } catch { /* reported below */ }
    }
    throw failure('undecodable', 'not an image this browser decodes');
  }
}

function surface(w, h) {
  if (typeof OffscreenCanvas === 'function') {
    const canvas = new OffscreenCanvas(w, h);
    return { context: canvas.getContext('2d', { alpha: false }), encode: quality => canvas.convertToBlob({ type: 'image/jpeg', quality }) };
  }
  if (typeof document === 'object' && document?.createElement) {
    const canvas = document.createElement('canvas');
    canvas.width = w; canvas.height = h;
    return { context: canvas.getContext('2d', { alpha: false }), encode: quality => new Promise((resolve, reject) =>
      canvas.toBlob(blob => blob ? resolve(blob) : reject(failure('unsupported', 'the canvas encoded nothing')), 'image/jpeg', quality)) };
  }
  throw failure('unsupported', 'no canvas in this realm');
}

/** `blob` as a JPEG within the budget: `{bytes: ArrayBuffer, width, height}`. */
export async function compress(blob, maxDimension, maxBytes) {
  const deadline = now() + DEADLINE_MS;
  if (blob.size > MAX_BYTES) throw failure('too-large', `${blob.size} bytes is over ${MAX_BYTES}`);
  const stored = await headerSize(blob);
  if (!stored) throw failure('undecodable', 'not an image whose size the web reads before decoding (JPEG, PNG, GIF, WebP, BMP)');
  if (stored[0] * stored[1] > MAX_SOURCE_PIXELS) throw failure('too-large', `${stored[0]}×${stored[1]} pixels is over ${MAX_SOURCE_PIXELS}`);
  const bitmap = await decode(blob);
  try {
    const { width, height } = bitmap;
    if (!width || !height) throw failure('undecodable', 'the image has no pixels');
    // Again after decoding: the header may not be the frame the browser drew.
    if (width * height > MAX_SOURCE_PIXELS) throw failure('too-large', `${width}×${height} pixels is over ${MAX_SOURCE_PIXELS}`);
    let drawn = null;
    const best = await search(width, height, maxDimension, maxBytes, async (w, h, quality) => {
      if (now() >= deadline) throw failure('timeout', `the search ran past ${DEADLINE_MS / 1000} s`);
      if (!drawn || drawn.w !== w || drawn.h !== h) {
        drawn = null; // the last size's canvas goes before the next is made
        const { context, encode } = surface(w, h);
        if (!context) throw failure('unsupported', 'no 2D canvas context');
        context.fillStyle = '#fff';
        context.fillRect(0, 0, w, h);
        context.imageSmoothingEnabled = true;
        context.imageSmoothingQuality = 'high';
        context.drawImage(bitmap, 0, 0, w, h);
        drawn = { w, h, encode };
      }
      const out = await drawn.encode(quality / 100);
      if (out.type !== 'image/jpeg') throw failure('unsupported', `this browser encodes ${out.type || 'nothing'}, not image/jpeg`);
      return { size: out.size, value: out };
    });
    if (!best) throw failure('unfit', `no JPEG of this image fits ${maxBytes} bytes`);
    return { bytes: await best.value.arrayBuffer(), width: best.width, height: best.height };
  } finally {
    bitmap.close?.();
  }
}
