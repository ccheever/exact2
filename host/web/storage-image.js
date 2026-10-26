// `storage.fs.compressImage` on the web (LLP 1069.002 A1.3): the browser
// decodes (`createImageBitmap`, orientation applied), a canvas scales (onto
// white for a JPEG, clear for a PNG, A1.11), and `convertToBlob` encodes each
// trial. Loaded by storage-fs.js on the first call, never before first
// pixel. A JPEG's search is Bluesky's (A1.2), the same trials as
// `exact_data::image::search` (scripts/fixtures/picker/compress-trials.json);
// a PNG's only shrinks, as `exact_data::image::search_lossless` does.
// `transformImage` (LLP 1133.019 D14) and `compressImage`'s edit (D15) draw
// the bitmap once through one transform, `geometry`'s, the same map as
// `exact_data::image::edit::Geometry`. A canvas has no metadata, no colour
// space but sRGB and no gain map, so a transform's result says `srgb`,
// `hdr: false` and `metadata: "none"` whatever was asked (D16).

// The platform's clock as storage-environment.js captured it, before a
// worker realm refuses ambient time to app code (LLP 1027.002 D2).
import { now } from './storage-environment.js';

export const MAX_DIMENSION = 8192;
export const MAX_BYTES = 64 * 1024 * 1024; // also the largest source
export const MAX_SOURCE_PIXELS = 64 * 1024 * 1024;
const DEADLINE_MS = 20_000;
const MAX_SHRINKS = 4;

export const MAX_SIDE = 16384;

export function failure(code, detail, call = 'compressImage') {
  const error = new Error(`filesystem: ${call}: ${code}: ${detail}`);
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
  const type = options.type === undefined ? 'image/jpeg' : options.type;
  if (type !== 'image/jpeg' && type !== 'image/png') throw new TypeError(`${api}: type must be "image/jpeg" or "image/png"`);
  return { maxDimension, maxBytes, type };
}

/** Where `edit` ({rotate, flip, crop}, as the prelude checked them) of a
 * `width × height` upright picture lands: the turned frame, the crop in it
 * (clipped, its edges rounded, LLP 1114.003 D2), and the map from upright
 * pixels to the crop's as a canvas `setTransform` takes it. A crop that
 * misses fails `outside`. */
export function geometry(width, height, edit, call) {
  const degrees = (((edit.rotate ?? 0) % 360) + 360) % 360;
  const quarter = { 0: [0, 1], 90: [1, 0], 180: [0, -1], 270: [-1, 0] }[degrees];
  const [sin, cos] = quarter ?? [Math.sin(degrees * Math.PI / 180), Math.cos(degrees * Math.PI / 180)];
  // A hair under a whole pixel is that pixel: a turn's rounding adds no row.
  const side = n => Math.max(1, Math.ceil(n - 1e-6));
  const tw = side(width * Math.abs(cos) + height * Math.abs(sin)), th = side(width * Math.abs(sin) + height * Math.abs(cos));
  // [a, b, c, d, e, f]: x' = a x + c y + e, y' = b x + d y + f.
  const then = ([a, b, c, d, e, f], [A, B, C, D, E, F]) => [A * a + C * b, B * a + D * b, A * c + C * d, B * c + D * d, A * e + C * f + E, B * e + D * f + F];
  let map = then(then([1, 0, 0, 1, -width / 2, -height / 2], [cos, sin, -sin, cos, 0, 0]), [1, 0, 0, 1, tw / 2, th / 2]);
  if (edit.flip === 'horizontal') map = then(map, [-1, 0, 0, 1, tw, 0]);
  if (edit.flip === 'vertical') map = then(map, [1, 0, 0, -1, 0, th]);
  let crop = [0, 0, tw, th];
  const r = edit.crop;
  if (r) {
    if (r.x >= tw || r.y >= th || r.x + r.width <= 0 || r.y + r.height <= 0) throw failure('outside', `the crop misses the ${tw}×${th} image`, call);
    const edges = (at, length, max) => {
      const lo = Math.min(Math.round(Math.max(at, 0)), max - 1), hi = Math.min(Math.round(Math.min(at + length, max)), max);
      return [lo, Math.max(hi, lo + 1) - lo];
    };
    const [x, w] = edges(r.x, r.width, tw), [y, h] = edges(r.y, r.height, th);
    crop = [x, y, w, h];
  }
  map = then(map, [1, 0, 0, 1, -crop[0], -crop[1]]);
  const toOutput = (w, h) => then(map, [w / crop[2], 0, 0, h / crop[3], 0, 0]);
  return { turned: [tw, th], crop, size: [crop[2], crop[3]], toOutput, free: !quarter };
}

/** `createImageBitmap`'s size: one side keeps the aspect, rounded up; both set it; neither keeps the crop's. */
export function resized(width, height, resizeWidth, resizeHeight) {
  const ratio = (n, num, den) => Math.min(MAX_SIDE, Math.max(1, Math.ceil(n * num / den)));
  if (resizeWidth && resizeHeight) return [resizeWidth, resizeHeight];
  if (resizeWidth) return [resizeWidth, ratio(height, resizeWidth, width)];
  if (resizeHeight) return [ratio(width, resizeHeight, height), resizeHeight];
  return [width, height];
}

const EXTENSIONS = { 'image/jpeg': 'jpg', 'image/png': 'png', 'image/webp': 'webp' };
/** The extension a `to` without one gains for `type`. */
export const extension = type => EXTENSIONS[type];

const WORDS = { flip: ['horizontal', 'vertical'], resizeQuality: ['pixelated', 'low', 'medium', 'high'], metadata: ['keep', 'no-location', 'none'] };
/** D14's options checked, as the prelude's binary.js `imageOptions` checks
 * them, for a Rust source's request that did not pass it: `rotate`, `flip`
 * and `crop`, and with `transform` the rest. A bad one is a TypeError. */
export function editOptions(api, options, transform) {
  if (options == null) options = {};
  if (typeof options !== 'object') throw new TypeError(`${api}: options must be an object`);
  const out = {}, finite = n => typeof n === 'number' && Number.isFinite(n), side = n => Number.isInteger(n) && n >= 1 && n <= MAX_SIDE;
  const take = (name, ok, says) => {
    if (options[name] === undefined) return;
    if (!ok(options[name])) throw new TypeError(`${api}: ${name} must be ${says}`);
    out[name] = options[name];
  };
  const word = name => take(name, w => WORDS[name].includes(w), WORDS[name].map(w => JSON.stringify(w)).join(' or '));
  take('rotate', finite, 'a finite number of degrees');
  word('flip');
  take('crop', c => c && typeof c === 'object' && [c.x, c.y, c.width, c.height].every(finite) && c.width > 0 && c.height > 0, '{x, y, width, height}, finite, with a positive width and height');
  if (!transform) return out;
  take('resizeWidth', side, `an integer from 1 to ${MAX_SIDE}`);
  take('resizeHeight', side, `an integer from 1 to ${MAX_SIDE}`);
  word('resizeQuality');
  take('type', t => typeof t === 'string' && /^image\/[a-z0-9.+-]+$/.test(t), 'a media type such as "image/jpeg"');
  take('quality', q => typeof q === 'number' && q >= 0 && q <= 1, 'a number from 0 to 1');
  word('metadata');
  return out;
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

async function decode(blob, call) {
  try { return await createImageBitmap(blob, { imageOrientation: 'from-image' }); }
  catch (error) {
    // An engine that predates the `from-image` value refuses the option
    // itself; its default applies EXIF orientation.
    if (error?.name === 'TypeError') {
      try { return await createImageBitmap(blob); } catch { /* reported below */ }
    }
    throw failure('undecodable', 'not an image this browser decodes', call);
  }
}

function surface(w, h, type, call = 'compressImage') {
  const alpha = type !== 'image/jpeg';
  if (typeof OffscreenCanvas === 'function') {
    const canvas = new OffscreenCanvas(w, h);
    return { context: canvas.getContext('2d', { alpha }), encode: quality => canvas.convertToBlob({ type, quality }) };
  }
  if (typeof document === 'object' && document?.createElement) {
    const canvas = document.createElement('canvas');
    canvas.width = w; canvas.height = h;
    return { context: canvas.getContext('2d', { alpha }), encode: quality => new Promise((resolve, reject) =>
      canvas.toBlob(blob => blob ? resolve(blob) : reject(failure('unsupported', 'the canvas encoded nothing', call)), type, quality)) };
  }
  throw failure('unsupported', 'no canvas in this realm', call);
}

/** `bitmap` drawn through `matrix` into a new `w × h` surface for `type`:
 * on white for a JPEG, which keeps no alpha (A1.3). */
function draw(bitmap, w, h, type, matrix, smoothing, call) {
  const { context, encode } = surface(w, h, type, call);
  if (!context) throw failure('unsupported', 'no 2D canvas context', call);
  if (type === 'image/jpeg') {
    context.fillStyle = '#fff';
    context.fillRect(0, 0, w, h);
  }
  context.imageSmoothingEnabled = smoothing !== 'pixelated';
  if (smoothing !== 'pixelated') context.imageSmoothingQuality = smoothing;
  if (matrix) context.setTransform(...matrix);
  if (matrix) context.drawImage(bitmap, 0, 0);
  else context.drawImage(bitmap, 0, 0, w, h);
  return encode;
}

/** The size from the header, and the decoded upright bitmap, bounded (A1.5). */
async function open(blob, late, call) {
  if (blob.size > MAX_BYTES) throw failure('too-large', `${blob.size} bytes is over ${MAX_BYTES}`, call);
  const stored = await headerSize(blob);
  if (!stored) throw failure('undecodable', 'not an image whose size the web reads before decoding (JPEG, PNG, GIF, WebP, BMP)', call);
  if (stored[0] * stored[1] > MAX_SOURCE_PIXELS) throw failure('too-large', `${stored[0]}×${stored[1]} pixels is over ${MAX_SOURCE_PIXELS}`, call);
  late(); // the header's read may have taken the time
  const bitmap = await decode(blob, call);
  const { width, height } = bitmap;
  // Again after decoding: the header may not be the frame the browser drew.
  const bad = !width || !height ? failure('undecodable', 'the image has no pixels', call)
    : width * height > MAX_SOURCE_PIXELS ? failure('too-large', `${width}×${height} pixels is over ${MAX_SOURCE_PIXELS}`, call) : null;
  if (bad) { bitmap.close?.(); throw bad; }
  return bitmap;
}

// The source's own type, where a canvas may write it: its first bytes.
async function sourceType(blob) {
  const b = new Uint8Array(await blob.slice(0, 12).arrayBuffer());
  const ascii = (i, k) => String.fromCharCode(...b.slice(i, i + k));
  if (b[0] === 0xff && b[1] === 0xd8) return 'image/jpeg';
  if (b[0] === 0x89 && ascii(1, 3) === 'PNG') return 'image/png';
  if (ascii(0, 4) === 'RIFF' && ascii(8, 4) === 'WEBP') return 'image/webp';
  return null;
}

/** `transformImage` of `blob` with `options` (as the prelude checked them):
 * `{bytes, width, height, type, colorSpace, hdr, metadata}`. Nothing is
 * decoded or drawn at or after `deadline`. */
export async function transform(blob, options = {}, deadline = now() + DEADLINE_MS) {
  const call = 'transformImage';
  const late = () => { if (now() >= deadline) throw failure('timeout', `the transform ran past ${DEADLINE_MS / 1000} s`, call); };
  const bitmap = await open(blob, late, call);
  try {
    const g = geometry(bitmap.width, bitmap.height, options, call);
    const [w, h] = resized(...g.size, options.resizeWidth, options.resizeHeight);
    if (w * h > MAX_SOURCE_PIXELS) throw failure('too-large', `${w}×${h} pixels is over ${MAX_SOURCE_PIXELS}`, call);
    // Unsaid, the source's type where the browser writes it, else JPEG (D14).
    const asked = options.type, own = asked === undefined ? await sourceType(blob) : null;
    for (const type of asked !== undefined ? [asked] : own && own !== 'image/jpeg' ? [own, 'image/jpeg'] : ['image/jpeg']) {
      late();
      const encode = draw(bitmap, w, h, type, g.toOutput(w, h), options.resizeQuality ?? 'high', call);
      const out = await encode(type === 'image/png' ? undefined : options.quality ?? 0.9);
      // convertToBlob writes a PNG for a type it does not know: refuse it, never another silently.
      if (out.type !== type) { if (asked === undefined && type !== 'image/jpeg') continue; throw failure('unsupported', `this browser encodes ${out.type || 'nothing'}, not ${type}`, call); }
      if (out.size > MAX_BYTES) throw failure('too-large', `the ${type} is over ${MAX_BYTES} bytes`, call);
      return { bytes: await out.arrayBuffer(), width: w, height: h, type, colorSpace: 'srgb', hdr: false, metadata: 'none' };
    }
    throw failure('unsupported', 'this browser encodes no JPEG', call);
  } finally {
    bitmap.close?.();
  }
}

/** `blob` as a JPEG, or a PNG (`type`), within the budget: `{bytes:
 * ArrayBuffer, width, height}`. With `edit` ({rotate, flip, crop}) the
 * search runs on the edited picture (LLP 1133.019 D15). Nothing is decoded
 * or tried at or after `deadline`. */
export async function compress(blob, maxDimension, maxBytes, deadline = now() + DEADLINE_MS, type = 'image/jpeg', edit = null) {
  const late = () => { if (now() >= deadline) throw failure('timeout', `the search ran past ${DEADLINE_MS / 1000} s`); };
  const bitmap = await open(blob, late, 'compressImage');
  try {
    const g = edit && (edit.rotate !== undefined || edit.flip !== undefined || edit.crop !== undefined)
      ? geometry(bitmap.width, bitmap.height, edit, 'compressImage') : null;
    const [width, height] = g ? g.size : [bitmap.width, bitmap.height];
    let drawn = null;
    const name = type === 'image/png' ? 'PNG' : 'JPEG';
    const trial = async (w, h, quality) => {
      late();
      if (!drawn || drawn.w !== w || drawn.h !== h) {
        drawn = null; // the last size's canvas goes before the next is made
        drawn = { w, h, encode: draw(bitmap, w, h, type, g?.toOutput(w, h), 'high', 'compressImage') };
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
