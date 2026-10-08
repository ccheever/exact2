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

// The stored pixel size from the header, read before anything is decoded
// (A1.5), or null when this is not a format whose header is read here:
// JPEG, PNG, GIF, WebP, BMP, HEIF/AVIF. The browser never decodes a file
// whose size is unknown or over the limit.
const HEADER_BYTES = 512 * 1024;
export async function headerSize(blob) {
  const at = async (start, length) => new DataView(await blob.slice(start, start + length).arrayBuffer());
  const head = await at(0, 32);
  if (head.byteLength < 12) return null;
  const u8 = i => head.getUint8(i), ascii = (i, n) => String.fromCharCode(...Array.from({ length: n }, (_, k) => u8(i + k)));
  if (u8(0) === 0x89 && ascii(1, 3) === 'PNG' && head.byteLength >= 24) return [head.getUint32(16), head.getUint32(20)];
  if (ascii(0, 6) === 'GIF87a' || ascii(0, 6) === 'GIF89a') return [head.getUint16(6, true), head.getUint16(8, true)];
  if (ascii(0, 2) === 'BM' && head.byteLength >= 26) return [Math.abs(head.getInt32(18, true)), Math.abs(head.getInt32(22, true))];
  if (ascii(0, 4) === 'RIFF' && ascii(8, 4) === 'WEBP' && head.byteLength >= 30) {
    const kind = ascii(12, 4), le24 = i => u8(i) | u8(i + 1) << 8 | u8(i + 2) << 16;
    if (kind === 'VP8 ') return [head.getUint16(26, true) & 0x3fff, head.getUint16(28, true) & 0x3fff];
    if (kind === 'VP8L') { const bits = head.getUint32(21, true); return [(bits & 0x3fff) + 1, ((bits >>> 14) & 0x3fff) + 1]; }
    if (kind === 'VP8X') return [le24(24) + 1, le24(27) + 1];
    return null;
  }
  if (u8(0) === 0xff && u8(1) === 0xd8) {
    // Segment by segment to the frame header; each read is a few bytes.
    const sof = new Set([0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf]);
    let pos = 2;
    for (let segments = 0; segments < 256 && pos + 9 <= blob.size; segments++) {
      const v = await at(pos, 9);
      if (v.getUint8(0) !== 0xff) return null;
      const marker = v.getUint8(1);
      if (marker === 0xff) { pos += 1; continue; }
      if (sof.has(marker)) return [v.getUint16(7), v.getUint16(5)];
      if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd8)) { pos += 2; continue; }
      if (marker === 0xd9 || marker === 0xda) return null; // image data before any frame header
      pos += 2 + v.getUint16(2);
    }
    return null;
  }
  if (ascii(4, 4) === 'ftyp') {
    // ISO BMFF (HEIF, AVIF): the largest `ispe` under meta/iprp/ipco.
    const v = await at(0, Math.min(blob.size, HEADER_BYTES));
    let best = null;
    const walk = (start, end, path) => {
      for (let pos = start; pos + 8 <= end;) {
        let size = v.getUint32(pos), header = 8;
        const type = String.fromCharCode(v.getUint8(pos + 4), v.getUint8(pos + 5), v.getUint8(pos + 6), v.getUint8(pos + 7));
        if (size === 1 && pos + 16 <= end) { size = Number(v.getBigUint64(pos + 8)); header = 16; }
        else if (size === 0) size = end - pos;
        if (size < header) return;
        const inner = pos + header, stop = Math.min(pos + size, end);
        if (type === 'meta') walk(inner + 4, stop, path + '/meta'); // a full box
        else if (type === 'iprp' && path === '/meta') walk(inner, stop, path + '/iprp');
        else if (type === 'ipco' && path === '/meta/iprp') walk(inner, stop, path + '/ipco');
        else if (type === 'ispe' && path === '/meta/iprp/ipco' && inner + 12 <= stop) {
          const size2 = [v.getUint32(inner + 4), v.getUint32(inner + 8)];
          if (!best || size2[0] * size2[1] > best[0] * best[1]) best = size2;
        }
        pos += size;
      }
    };
    walk(0, v.byteLength, '');
    return best;
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
  if (!stored || !stored[0] || !stored[1]) throw failure('undecodable', 'not an image whose size this host reads (JPEG, PNG, GIF, WebP, BMP, HEIF, AVIF)');
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
