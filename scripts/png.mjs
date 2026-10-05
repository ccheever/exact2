// PNG, the little the fixtures and the agent's film need and nothing more:
// decode an 8-bit, non-interlaced RGB or RGBA image to RGBA bytes; encode RGBA
// bytes as an unfiltered RGBA image, as an animated PNG of equal frames, or as
// a contact sheet of them. Node's zlib does the compression; this does the
// chunks, the filters, and the CRC. No dependency (rules/RULES.md: none).
import { deflateSync, inflateSync } from 'node:zlib';

/** {width, height, data: Uint8Array of RGBA} from a PNG buffer. */
export function decodePng(buf) {
  const sig = [137, 80, 78, 71, 13, 10, 26, 10];
  for (let i = 0; i < 8; i++) if (buf[i] !== sig[i]) throw new Error('not a PNG');
  let pos = 8, width = 0, height = 0, depth = 0, color = 0, interlace = 0;
  const idat = [];
  while (pos + 8 <= buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString('latin1', pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === 'IHDR') { width = data.readUInt32BE(0); height = data.readUInt32BE(4); depth = data[8]; color = data[9]; interlace = data[12]; }
    else if (type === 'IDAT') idat.push(data);
    else if (type === 'IEND') break;
    pos += 12 + len;
  }
  if (depth !== 8 || interlace !== 0 || (color !== 2 && color !== 6)) throw new Error(`unsupported PNG: depth ${depth}, color type ${color}, interlace ${interlace}`);
  const bpp = color === 6 ? 4 : 3, stride = width * bpp;
  const raw = inflateSync(Buffer.concat(idat));
  const out = new Uint8Array(width * height * 4);
  let prev = new Uint8Array(stride), cur = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    const f = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let i = 0; i < stride; i++) {
      const a = i >= bpp ? cur[i - bpp] : 0, b = prev[i], c = i >= bpp ? prev[i - bpp] : 0;
      let x = line[i];
      switch (f) {
        case 1: x += a; break;
        case 2: x += b; break;
        case 3: x += (a + b) >> 1; break;
        case 4: { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); x += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; break; }
      }
      cur[i] = x & 255;
    }
    for (let x = 0; x < width; x++) {
      const s = x * bpp, d = (y * width + x) * 4;
      out[d] = cur[s]; out[d + 1] = cur[s + 1]; out[d + 2] = cur[s + 2]; out[d + 3] = bpp === 4 ? cur[s + 3] : 255;
    }
    [prev, cur] = [cur, prev];
  }
  return { width, height, data: out };
}

/** An image's compressed, unfiltered scanlines. */
function scanlines({ width, height, data }) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) { raw[y * (stride + 1)] = 0; Buffer.from(data.buffer, data.byteOffset + y * stride, stride).copy(raw, y * (stride + 1) + 1); }
  return deflateSync(raw);
}
const SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
function header(width, height, color = 6) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0); ihdr.writeUInt32BE(height, 4); ihdr[8] = 8; ihdr[9] = color; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return chunk('IHDR', ihdr);
}

/** An image's RGB scanlines, each with the filter whose bytes sum smallest (the usual heuristic), compressed. */
function filteredRgb({ width, height, data }) {
  const stride = width * 3, raw = Buffer.alloc((stride + 1) * height), best = new Uint8Array(stride), trial = new Uint8Array(stride);
  let prev = new Uint8Array(stride), cur = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) for (let c = 0; c < 3; c++) cur[x * 3 + c] = data[(y * width + x) * 4 + c];
    let least = Infinity;
    for (let f = 0; f < 5; f++) {
      let sum = 0;
      for (let i = 0; i < stride; i++) {
        const a = i >= 3 ? cur[i - 3] : 0, b = prev[i], c = i >= 3 ? prev[i - 3] : 0;
        const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        const predicted = f === 0 ? 0 : f === 1 ? a : f === 2 ? b : f === 3 ? (a + b) >> 1 : pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
        const v = (cur[i] - predicted) & 255;
        trial[i] = v; sum += v < 128 ? v : 256 - v;
      }
      if (sum < least) { least = sum; raw[y * (stride + 1)] = f; best.set(trial); }
    }
    raw.set(best, y * (stride + 1) + 1);
    [prev, cur] = [cur, prev];
  }
  return deflateSync(raw, { level: 9 });
}

/** A PNG buffer from {width, height, data: RGBA bytes}. `rgb` drops the alpha and
 * filters each row: a smaller file, for a picture kept in the repository. */
export function encodePng(image, { rgb = false } = {}) {
  const pixels = rgb ? filteredRgb(image) : scanlines(image);
  return Buffer.concat([SIGNATURE, header(image.width, image.height, rgb ? 2 : 6), chunk('IDAT', pixels), chunk('IEND', Buffer.alloc(0))]);
}

/** An animated PNG of equal-size RGBA frames, each shown `delayMs`, looping. A viewer without APNG shows the first. */
export function encodeApng(frames, delayMs) {
  const { width, height } = frames[0];
  if (frames.some(f => f.width !== width || f.height !== height)) throw new Error('animated PNG: every frame must be one size');
  const actl = Buffer.alloc(8);
  actl.writeUInt32BE(frames.length, 0); actl.writeUInt32BE(0, 4);
  const parts = [SIGNATURE, header(width, height), chunk('acTL', actl)];
  let seq = 0;
  frames.forEach((frame, i) => {
    const fctl = Buffer.alloc(26);
    fctl.writeUInt32BE(seq++, 0); fctl.writeUInt32BE(width, 4); fctl.writeUInt32BE(height, 8);
    fctl.writeUInt16BE(Math.min(65535, Math.round(delayMs)), 20); fctl.writeUInt16BE(1000, 22);
    parts.push(chunk('fcTL', fctl));
    const data = scanlines(frame);
    if (i === 0) parts.push(chunk('IDAT', data));
    else { const fdat = Buffer.alloc(4 + data.length); fdat.writeUInt32BE(seq++, 0); data.copy(fdat, 4); parts.push(chunk('fdAT', fdat)); }
  });
  parts.push(chunk('IEND', Buffer.alloc(0)));
  return Buffer.concat(parts);
}

/** Equal-size RGBA frames in a grid, left to right, at most `columns` wide, a one-pixel gray gap between cells; shrunk by a whole factor (box average) until the sheet is at most `maxWidth` wide. */
export function contactSheet(frames, { columns = 6, maxWidth = 2048 } = {}) {
  const cols = Math.min(columns, frames.length), rows = Math.ceil(frames.length / cols);
  const k = Math.max(1, Math.ceil((cols * frames[0].width + cols - 1) / maxWidth));
  const w = Math.floor(frames[0].width / k), h = Math.floor(frames[0].height / k);
  const width = cols * w + cols - 1, height = rows * h + rows - 1;
  const data = new Uint8Array(width * height * 4).fill(128);
  for (let i = 3; i < data.length; i += 4) data[i] = 255;
  frames.forEach((f, i) => {
    const ox = (i % cols) * (w + 1), oy = Math.floor(i / cols) * (h + 1);
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const d = ((oy + y) * width + ox + x) * 4;
      for (let c = 0; c < 4; c++) {
        let sum = 0;
        for (let dy = 0; dy < k; dy++) for (let dx = 0; dx < k; dx++) sum += f.data[((y * k + dy) * f.width + x * k + dx) * 4 + c];
        data[d + c] = Math.round(sum / (k * k));
      }
    }
  });
  return { width, height, data };
}

/** An image shrunk by a whole factor `k`: each pixel the box average of k × k (an edge short of k is dropped). */
export function shrink({ width, height, data }, k) {
  const w = Math.floor(width / k), h = Math.floor(height / k), out = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) for (let c = 0; c < 4; c++) {
    let sum = 0;
    for (let dy = 0; dy < k; dy++) for (let dx = 0; dx < k; dx++) sum += data[((y * k + dy) * width + x * k + dx) * 4 + c];
    out[(y * w + x) * 4 + c] = Math.round(sum / (k * k));
  }
  return { width: w, height: h, data: out };
}

/** Two images of one size side by side, then their difference: the reference
 * dimmed to gray, each pixel whose largest channel differs by more than `band`
 * in red, brighter for a larger difference; each pixel drawn `zoom` × `zoom`. */
export function diffPicture(reference, actual, band = 8, zoom = 1) {
  const { width: w, height: h } = reference, W = w * 3 * zoom, out = new Uint8Array(W * h * zoom * 4);
  const put = (x, y, rgba) => { for (let dy = 0; dy < zoom; dy++) for (let dx = 0; dx < zoom; dx++) out.set(rgba, ((y * zoom + dy) * W + x * zoom + dx) * 4); };
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 4;
    put(x, y, reference.data.subarray(i, i + 4));
    put(w + x, y, actual.data.subarray(i, i + 4));
    let m = 0;
    for (let c = 0; c < 3; c++) m = Math.max(m, Math.abs(reference.data[i + c] - actual.data[i + c]));
    const gray = (reference.data[i] + reference.data[i + 1] + reference.data[i + 2]) / 9;
    put(2 * w + x, y, m > band ? [128 + Math.min(127, m), 0, 0, 255] : [gray, gray, gray, 255]);
  }
  return { width: W, height: h * zoom, data: out };
}

/** The RGBA bytes of a rectangle of an image, as an image. */
export function crop({ width, data }, x, y, w, h) {
  const out = new Uint8Array(w * h * 4);
  for (let row = 0; row < h; row++) out.set(data.subarray(((y + row) * width + x) * 4, ((y + row) * width + x + w) * 4), row * w * 4);
  return { width: w, height: h, data: out };
}

/** Two images of one size compared: the share of pixels whose largest channel difference exceeds `band`, and the mean absolute difference. */
export function diff(a, b, band = 8) {
  if (a.width !== b.width || a.height !== b.height) return { differing: 1, mean: 255, size: `${a.width}×${a.height} vs ${b.width}×${b.height}` };
  let over = 0, sum = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    let m = 0;
    for (let c = 0; c < 3; c++) { const d = Math.abs(a.data[i + c] - b.data[i + c]); sum += d; if (d > m) m = d; }
    if (m > band) over++;
  }
  const n = a.data.length / 4;
  return { differing: over / n, mean: sum / (n * 3), size: `${a.width}×${a.height}` };
}

const CRC = new Int32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c; });
function crc32(buf) { let c = -1; for (const b of buf) c = CRC[(c ^ b) & 255] ^ (c >>> 8); return (c ^ -1) >>> 0; }
function chunk(type, data) {
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0); out.write(type, 4, 'latin1'); data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}
