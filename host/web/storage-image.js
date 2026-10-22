// `storage.fs.compressImage` on the web (LLP 1069.002 A1.3): the browser
// decodes (`createImageBitmap`, orientation applied), a canvas scales (onto
// white for a JPEG, clear for a PNG, A1.11), and `convertToBlob` encodes each
// trial. Loaded by storage-fs.js on the first call, never before first
// pixel. A JPEG's search is Bluesky's (A1.2), the same trials as
// `exact_data::image::search` (scripts/fixtures/picker/compress-trials.json);
// a PNG's only shrinks, as `exact_data::image::search_lossless` does.

// The platform's clock as storage-environment.js captured it, before a
// worker realm refuses ambient time to app code (LLP 1027.002 D2).
import { now } from './storage-environment.js';

export const MAX_DIMENSION = 8192;
export const MAX_BYTES = 64 * 1024 * 1024; // also the largest source
export const MAX_SOURCE_PIXELS = 64 * 1024 * 1024;
// A PNG with no `maxDimension` keeps its size, at most this a side (LLP
// 1133.006 D1; the native codec service's bound).
export const KEEP_SIZE = 0, MAX_KEPT_DIMENSION = 16384;
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
  const { maxBytes, maxPixels } = options;
  const type = options.type === undefined ? 'image/jpeg' : options.type;
  if (type !== 'image/jpeg' && type !== 'image/png') throw new TypeError(`${api}: type must be "image/jpeg" or "image/png"`);
  const keep = options.maxDimension === undefined && type === 'image/png', maxDimension = keep ? KEEP_SIZE : options.maxDimension;
  if (!keep && (!Number.isInteger(maxDimension) || maxDimension < 1 || maxDimension > MAX_DIMENSION))
    throw new TypeError(`${api}: maxDimension must be an integer from 1 to ${MAX_DIMENSION}${type === 'image/png' ? ', or left out to keep the size' : ''}`);
  if (!Number.isInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_BYTES)
    throw new TypeError(`${api}: maxBytes must be an integer from 1 to ${MAX_BYTES}`);
  if (maxPixels !== undefined && (!Number.isInteger(maxPixels) || maxPixels < 1 || maxPixels > MAX_SOURCE_PIXELS))
    throw new TypeError(`${api}: maxPixels must be an integer from 1 to ${MAX_SOURCE_PIXELS}`);
  return { maxDimension, maxBytes, type, maxPixels: maxPixels ?? MAX_SOURCE_PIXELS };
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

/** A PNG's search (A1.11): the fitted size, then each 0.8 shrink, at most
 * four of them. `encode(w, h)` resolves `{size, value}`. */
export async function searchLossless(width, height, maxDimension, maxBytes, encode) {
  // Kept at its own size: one trial, and no shrinking.
  if (maxDimension === KEEP_SIZE) {
    const { size, value } = await encode(width, height);
    return size <= maxBytes ? { value, width, height } : null;
  }
  let dimension = maxDimension;
  for (let shrinks = 0; shrinks <= MAX_SHRINKS; shrinks++) {
    const [w, h] = contain(width, height, dimension);
    const { size, value } = await encode(w, h);
    if (size <= maxBytes) return { value, width: w, height: h };
    if (dimension === 1) break;
    dimension = Math.max(1, Math.floor(dimension * 0.8));
  }
  return null;
}

// The pixel size from the header, read before anything is decoded (A1.5),
// or null: the browser never decodes a file whose size is unknown here or
// over the limit. JPEG (its frame header), PNG, GIF (of its screen and
// first image, the one of more pixels), WebP (of its canvas and first
// frame, the one of more pixels), BMP. Not
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
  // Of two rectangles (a screen or canvas and a frame), the one of more pixels.
  const larger = (a, b) => a && b ? (a[0] * a[1] >= b[0] * b[1] ? a : b) : null;
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
    // ones, a negative height meaning top-down (a negative width is no
    // image).
    if (v.getUint32(14, true) === 12) return positive([v.getUint16(18, true), v.getUint16(20, true)]);
    return positive([v.getInt32(18, true), Math.abs(v.getInt32(22, true))]);
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

// An SVG (LLP 1133.006 D1, "SVG input"): sniffed as a browser does, sized as
// an `img` with no CSS size is (exact-svg-file's `Natural::size`: the file's
// width and height, one with the viewBox's ratio, the viewBox's own size
// when it gives none, else 300 × 150), and drawn by an `img` element, which
// only a page has: in a worker it is `undecodable`.
export function svgText(bytes) {
  let rest = new TextDecoder().decode(bytes.subarray(0, 4096)).replace(/^\ufeff/, '').trimStart();
  for (;;) {
    if (/^<svg[\s>/:]/.test(rest)) return rest;
    const skip = rest.startsWith('<!--') ? rest.indexOf('-->', 4) + 3 : rest.startsWith('<?') || /^<!doctype/i.test(rest) ? rest.indexOf('>') + 1 : 0;
    if (skip <= 0) return null;
    rest = rest.slice(skip).trimStart();
  }
}
export function svgSize(text) {
  const tag = /^<svg(?::svg)?\b([^>]*)>/.exec(text)?.[1] ?? '';
  const attr = name => new RegExp(`\\s${name}\\s*=\\s*("([^"]*)"|'([^']*)')`).exec(tag)?.slice(2).find(v => v !== undefined);
  const length = value => {
    const m = /^\s*([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)\s*(px|pt|pc|mm|cm|in|em|rem)?\s*$/i.exec(value ?? '');
    const n = m ? Number(m[1]) * ({ pt: 4 / 3, pc: 16, mm: 96 / 25.4, cm: 96 / 2.54, in: 96, em: 16, rem: 16 }[m[2]?.toLowerCase()] ?? 1) : NaN;
    return n > 0 && isFinite(n) ? n : null;
  };
  const box = (attr('viewBox') ?? '').split(/[\s,]+/).filter(Boolean).map(Number);
  const view = box.length === 4 && box[2] > 0 && box[3] > 0 && box.every(isFinite) ? box : null;
  const w = length(attr('width')), h = length(attr('height')), r = view ? view[2] / view[3] : null;
  if (w && h) return [w, h];
  if (w && r) return [w, w / r];
  if (h && r) return [h * r, h];
  if (view) return [view[2], view[3]];
  return [w ?? 300, h ?? 150];
}
async function decodeSvg(blob, maxPixels) {
  const text = svgText(new Uint8Array(await blob.slice(0, 4096).arrayBuffer()));
  const [w, h] = svgSize(text).map(n => Math.max(1, Math.round(n)));
  if (w * h > maxPixels) throw failure('too-large', `${w}×${h} pixels is over ${maxPixels}`);
  if (typeof Image !== 'function' || typeof URL?.createObjectURL !== 'function') throw failure('undecodable', 'an SVG is drawn by an img element, which this realm has not');
  const url = URL.createObjectURL(new Blob([blob], { type: 'image/svg+xml' }));
  try {
    const img = new Image();
    img.src = url;
    await img.decode().catch(() => { throw failure('undecodable', 'not an SVG this browser draws'); });
    const { context, encode } = surface(w, h, 'image/png');
    context.drawImage(img, 0, 0, w, h);
    return await createImageBitmap(await encode());
  } finally { URL.revokeObjectURL(url); }
}

// An animated PNG's first frame as a still PNG (LLP 1133.006 D1), as the
// native codec cuts it: Chromium writes that frame, and WebKit's decoder,
// ImageIO, does not select it. A still PNG is itself; null when malformed.
const PNG_SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const CRC = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
export function pngFirstFrame(bytes) {
  if (bytes.length < 8 || PNG_SIGNATURE.some((b, i) => bytes[i] !== b)) return bytes;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), chunks = [];
  for (let at = 8; at + 12 <= bytes.length;) {
    const length = view.getUint32(at), type = String.fromCharCode(...bytes.subarray(at + 4, at + 8));
    if (at + 12 + length > bytes.length) return null;
    chunks.push({ type, data: bytes.subarray(at + 8, at + 8 + length) });
    at += 12 + length;
    if (type === 'IEND') break;
  }
  const image = chunks.findIndex(c => c.type === 'IDAT');
  if (image < 0 || !chunks.slice(0, image).some(c => c.type === 'acTL')) return bytes;
  const run = (list, type) => { const out = []; for (const c of list) { if (c.type !== type) break; out.push(c); } return out; };
  let data;
  if (chunks.slice(0, image).some(c => c.type === 'fcTL')) data = run(chunks.slice(image), 'IDAT').map(c => c.data);
  else {
    const control = chunks.findIndex(c => c.type === 'fcTL');
    if (control < 0) return null;
    data = run(chunks.slice(control + 1), 'fdAT').filter(c => c.data.length > 4).map(c => c.data.subarray(4));
  }
  if (!data.length) return null;
  const chunk = (type, d) => {
    const out = new Uint8Array(12 + d.length), v = new DataView(out.buffer);
    v.setUint32(0, d.length);
    out.set([...type].map(c => c.charCodeAt(0)), 4);
    out.set(d, 8);
    let crc = 0xffffffff;
    for (const b of out.subarray(4, 8 + d.length)) crc = CRC[(crc ^ b) & 0xff] ^ (crc >>> 8);
    v.setUint32(8 + d.length, (crc ^ 0xffffffff) >>> 0);
    return out;
  };
  const parts = [new Uint8Array(PNG_SIGNATURE), ...chunks.slice(0, image).filter(c => c.type !== 'acTL' && c.type !== 'fcTL').map(c => chunk(c.type, c.data)), ...data.map(d => chunk('IDAT', d)), chunk('IEND', new Uint8Array())];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  parts.reduce((at, p) => (out.set(p, at), at + p.length), 0);
  return out;
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

function surface(w, h, type) {
  const alpha = type === 'image/png';
  if (typeof OffscreenCanvas === 'function') {
    const canvas = new OffscreenCanvas(w, h);
    return { context: canvas.getContext('2d', { alpha }), encode: quality => canvas.convertToBlob({ type, quality }) };
  }
  if (typeof document === 'object' && document?.createElement) {
    const canvas = document.createElement('canvas');
    canvas.width = w; canvas.height = h;
    return { context: canvas.getContext('2d', { alpha }), encode: quality => new Promise((resolve, reject) =>
      canvas.toBlob(blob => blob ? resolve(blob) : reject(failure('unsupported', 'the canvas encoded nothing')), type, quality)) };
  }
  throw failure('unsupported', 'no canvas in this realm');
}

/** `blob` as a JPEG, or a PNG (`type`), within the budget: `{bytes:
 * ArrayBuffer, width, height}`. Nothing is decoded or tried at or after
 * `deadline`. */
export async function compress(blob, maxDimension, maxBytes, deadline = now() + DEADLINE_MS, type = 'image/jpeg', maxPixels = MAX_SOURCE_PIXELS) {
  const late = () => { if (now() >= deadline) throw failure('timeout', `the search ran past ${DEADLINE_MS / 1000} s`); };
  if (blob.size > MAX_BYTES) throw failure('too-large', `${blob.size} bytes is over ${MAX_BYTES}`);
  const pixels = Math.min(maxPixels, MAX_SOURCE_PIXELS);
  const head = new Uint8Array(await blob.slice(0, 4096).arrayBuffer());
  let bitmap;
  if (svgText(head)) bitmap = await decodeSvg(blob, pixels);
  else {
    const stored = await headerSize(blob);
    if (!stored) throw failure('undecodable', 'not an image whose size the web reads before decoding (JPEG, PNG, GIF, WebP, BMP, SVG)');
    if (stored[0] * stored[1] > pixels) throw failure('too-large', `${stored[0]}×${stored[1]} pixels is over ${pixels}`);
    late(); // the header's read may have taken the time
    // An animated PNG decodes as its first frame.
    if (head[0] === 0x89 && head[1] === 0x50) {
      const still = pngFirstFrame(new Uint8Array(await blob.arrayBuffer()));
      if (!still) throw failure('undecodable', 'an animated PNG whose first frame is malformed');
      blob = new Blob([still], { type: 'image/png' });
    }
    bitmap = await decode(blob);
  }
  try {
    const { width, height } = bitmap;
    if (!width || !height) throw failure('undecodable', 'the image has no pixels');
    // Again after decoding: the header may not be the frame the browser drew.
    if (width * height > pixels) throw failure('too-large', `${width}×${height} pixels is over ${pixels}`);
    if (maxDimension === KEEP_SIZE && (width > MAX_KEPT_DIMENSION || height > MAX_KEPT_DIMENSION)) throw failure('too-large', `${width}×${height} keeps more than ${MAX_KEPT_DIMENSION} pixels a side`);
    let drawn = null;
    const name = type === 'image/png' ? 'PNG' : 'JPEG';
    const trial = async (w, h, quality) => {
      late();
      if (!drawn || drawn.w !== w || drawn.h !== h) {
        drawn = null; // the last size's canvas goes before the next is made
        const { context, encode } = surface(w, h, type);
        if (!context) throw failure('unsupported', 'no 2D canvas context');
        if (type === 'image/jpeg') {
          context.fillStyle = '#fff';
          context.fillRect(0, 0, w, h);
        }
        context.imageSmoothingEnabled = true;
        context.imageSmoothingQuality = 'high';
        context.drawImage(bitmap, 0, 0, w, h);
        drawn = { w, h, encode };
      }
      const out = await drawn.encode(quality === undefined ? undefined : quality / 100);
      if (out.type !== type) throw failure('unsupported', `this browser encodes ${out.type || 'nothing'}, not ${type}`);
      return { size: out.size, value: out };
    };
    const best = type === 'image/png'
      ? await searchLossless(width, height, maxDimension, maxBytes, (w, h) => trial(w, h))
      : await search(width, height, maxDimension, maxBytes, trial);
    if (!best) throw failure('unfit', `no ${name} of this image fits ${maxBytes} bytes`);
    return { bytes: await best.value.arrayBuffer(), width: best.width, height: best.height };
  } finally {
    bitmap.close?.();
  }
}
