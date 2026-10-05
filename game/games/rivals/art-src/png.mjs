// PNG writing and the procedural texture toolkit: value noise, fbm, a tiny
// pixel font, and RGBA canvases drawn in linear light and stored as sRGB.
import { deflateSync } from 'node:zlib';

const CRC = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (bytes) => {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(new TextEncoder().encode(type), 4);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
};
/** Encode 8-bit RGBA pixels as a PNG file. */
export function png(width, height, rgba) {
  const raw = new Uint8Array(height * (width * 4 + 1));
  for (let y = 0; y < height; y++) raw.set(rgba.subarray(y * width * 4, (y + 1) * width * 4), y * (width * 4 + 1) + 1);
  const header = new Uint8Array(13);
  const view = new DataView(header.buffer);
  view.setUint32(0, width);
  view.setUint32(4, height);
  header.set([8, 6, 0, 0, 0], 8);
  const parts = [new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw, { level: 9 })), chunk('IEND', new Uint8Array())];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) { out.set(p, at); at += p.length; }
  return out;
}

// Deterministic hash noise: the generator's output depends only on this file.
const hash = (x, y, s) => {
  let h = (x * 374761393 + y * 668265263 + s * 2147483647) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967295;
};
const smooth = (t) => t * t * (3 - 2 * t);
/** Tileable value noise with period `p` cells. */
export function noise(x, y, p, s = 0) {
  const xi = Math.floor(x), yi = Math.floor(y), fx = smooth(x - xi), fy = smooth(y - yi);
  const w = (a) => ((a % p) + p) % p;
  const v = (i, j) => hash(w(xi + i), w(yi + j), s);
  return (v(0, 0) * (1 - fx) + v(1, 0) * fx) * (1 - fy) + (v(0, 1) * (1 - fx) + v(1, 1) * fx) * fy;
}
/** Tileable fbm in [0,1]: u, v in [0,1), `base` cells at the first octave. */
export function fbm(u, v, base = 4, octaves = 4, s = 0) {
  let sum = 0, amp = 0.5, norm = 0, p = base;
  for (let o = 0; o < octaves; o++) {
    sum += amp * noise(u * p, v * p, p, s + o * 17);
    norm += amp; amp *= 0.5; p *= 2;
  }
  return sum / norm;
}
export const clamp = (x, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, x));
export const mix = (a, b, t) => a + (b - a) * t;
export const mix3 = (a, b, t) => a.map((x, i) => mix(x, b[i], t));
const toSrgb = (c) => {
  c = clamp(c);
  return c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055;
};
export const srgb = (c) => Math.round(toSrgb(c) * 255);

/** A canvas of linear RGBA floats; `bytes(srgbColour)` packs it for a PNG. */
export class Canvas {
  constructor(width, height) {
    this.width = width; this.height = height;
    this.data = new Float32Array(width * height * 4).fill(1);
  }
  set(x, y, rgba) {
    if (x < 0 || y < 0 || x >= this.width || y >= this.height) return;
    this.data.set(rgba, (y * this.width + x) * 4);
  }
  get(x, y) {
    const w = this.width, h = this.height;
    const i = ((((y % h) + h) % h) * w + (((x % w) + w) % w)) * 4;
    return this.data.subarray(i, i + 4);
  }
  each(fn) {
    for (let y = 0; y < this.height; y++) for (let x = 0; x < this.width; x++) {
      const out = fn(x, y, (x + 0.5) / this.width, (y + 0.5) / this.height);
      if (out) this.set(x, y, out.length === 3 ? [...out, 1] : out);
    }
    return this;
  }
  /** Colour textures are sRGB-encoded; data textures (normal, MR) are stored linearly. */
  bytes(colour = true) {
    const out = new Uint8Array(this.width * this.height * 4);
    for (let i = 0; i < out.length; i++) {
      const v = this.data[i];
      out[i] = colour && i % 4 !== 3 ? srgb(v) : Math.round(clamp(v) * 255);
    }
    return out;
  }
  png(colour = true) { return png(this.width, this.height, this.bytes(colour)); }
}

/** A tangent-space normal map from a height function h(u, v) in [0,1]. */
export function normalMap(size, height, strength = 4) {
  const hs = new Float32Array(size * size);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) hs[y * size + x] = height((x + 0.5) / size, (y + 0.5) / size);
  const at = (x, y) => hs[(((y % size) + size) % size) * size + (((x % size) + size) % size)];
  return new Canvas(size, size).each((x, y) => {
    const dx = (at(x + 1, y) - at(x - 1, y)) * strength, dy = (at(x, y + 1) - at(x, y - 1)) * strength;
    const n = [-dx, dy, 1], l = Math.hypot(...n);
    return [n[0] / l * 0.5 + 0.5, n[1] / l * 0.5 + 0.5, n[2] / l * 0.5 + 0.5, 1];
  });
}

// A 5×7 pixel font for stencils and logos (only the letters the arena uses).
const FONT = {
  A: ['01110', '10001', '10001', '11111', '10001', '10001', '10001'],
  B: ['11110', '10001', '10001', '11110', '10001', '10001', '11110'],
  E: ['11111', '10000', '10000', '11110', '10000', '10000', '11111'],
  I: ['11111', '00100', '00100', '00100', '00100', '00100', '11111'],
  L: ['10000', '10000', '10000', '10000', '10000', '10000', '11111'],
  N: ['10001', '11001', '10101', '10011', '10001', '10001', '10001'],
  O: ['01110', '10001', '10001', '10001', '10001', '10001', '01110'],
  R: ['11110', '10001', '10001', '11110', '10100', '10010', '10001'],
  S: ['01111', '10000', '10000', '01110', '00001', '00001', '11110'],
  T: ['11111', '00100', '00100', '00100', '00100', '00100', '00100'],
  V: ['10001', '10001', '10001', '10001', '10001', '01010', '00100'],
  X: ['10001', '10001', '01010', '00100', '01010', '10001', '10001'],
  0: ['01110', '10011', '10101', '10101', '10101', '11001', '01110'],
  1: ['00100', '01100', '00100', '00100', '00100', '00100', '01110'],
  2: ['01110', '10001', '00001', '00110', '01000', '10000', '11111'],
  3: ['11110', '00001', '00001', '01110', '00001', '00001', '11110'],
  4: ['00010', '00110', '01010', '10010', '11111', '00010', '00010'],
  '-': ['00000', '00000', '00000', '11111', '00000', '00000', '00000'],
  ' ': ['00000', '00000', '00000', '00000', '00000', '00000', '00000'],
};
/** Whether text drawn at (x0, y0) with `scale`-pixel cells covers pixel (x, y). */
export function textCovers(text, x0, y0, scale, x, y) {
  const cx = Math.floor((x - x0) / scale), cy = Math.floor((y - y0) / scale);
  if (cy < 0 || cy >= 7 || cx < 0) return false;
  const glyph = FONT[text[Math.floor(cx / 6)]];
  return !!glyph && cx % 6 < 5 && glyph[cy][cx % 6] === '1';
}
export const textWidth = (text, scale) => (text.length * 6 - 1) * scale;
