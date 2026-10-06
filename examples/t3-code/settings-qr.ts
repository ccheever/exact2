// QR codes for General → About → Mobile app (lane r3-settings). A byte-mode port of
// the reference's QR generator (packages/shared/src/qrCode.ts, Project Nayuki's
// library, MIT): QrCode.encodeText for text outside the numeric and alphanumeric
// sets (every link this app encodes has lowercase letters, so encodeText picks
// byte mode), the same version search, ECC boost, Reed-Solomon blocks and mask
// penalty, so the symbol matches the reference module for module.
// Copyright (c) Project Nayuki. (MIT License) https://www.nayuki.io/page/qr-code-generator-library

type Ecc = { ordinal: number; formatBits: number };
export const ECC: Record<'L' | 'M' | 'Q' | 'H', Ecc> = { L: { ordinal: 0, formatBits: 1 }, M: { ordinal: 1, formatBits: 0 }, Q: { ordinal: 2, formatBits: 3 }, H: { ordinal: 3, formatBits: 2 } };
const ECC_CODEWORDS_PER_BLOCK = [
  [-1, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
  [-1, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28],
  [-1, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30, 30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
  [-1, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
];
const NUM_ERROR_CORRECTION_BLOCKS = [
  [-1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25],
  [-1, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49],
  [-1, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29, 34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68],
  [-1, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32, 35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81],
];
const getBit = (x: number, i: number) => ((x >>> i) & 1) !== 0;
function appendBits(value: number, length: number, bits: number[]) { for (let i = length - 1; i >= 0; i--) bits.push((value >>> i) & 1); }
function rawDataModules(version: number): number {
  let result = (16 * version + 128) * version + 64;
  if (version >= 2) { const align = Math.floor(version / 7) + 2; result -= (25 * align - 10) * align - 55; if (version >= 7) result -= 36; }
  return result;
}
const dataCodewords = (version: number, ecc: Ecc) => Math.floor(rawDataModules(version) / 8) - ECC_CODEWORDS_PER_BLOCK[ecc.ordinal]![version]! * NUM_ERROR_CORRECTION_BLOCKS[ecc.ordinal]![version]!;
function rsMultiply(x: number, y: number): number {
  let z = 0;
  for (let i = 7; i >= 0; i--) { z = (z << 1) ^ ((z >>> 7) * 0x11d); z ^= ((y >>> i) & 1) * x; }
  return z;
}
function rsDivisor(degree: number): number[] {
  const result: number[] = new Array(degree - 1).fill(0).concat([1]);
  let root = 1;
  for (let i = 0; i < degree; i++) {
    for (let j = 0; j < result.length; j++) { result[j] = rsMultiply(result[j]!, root); if (j + 1 < result.length) result[j]! ^= result[j + 1]!; }
    root = rsMultiply(root, 0x02);
  }
  return result;
}
function rsRemainder(data: number[], divisor: number[]): number[] {
  const result = divisor.map(() => 0);
  for (const b of data) { const factor = b ^ (result.shift() as number); result.push(0); divisor.forEach((coef, i) => (result[i]! ^= rsMultiply(coef, factor))); }
  return result;
}
function utf8(text: string): number[] {
  const out: number[] = [];
  for (const char of text) {
    const c = char.codePointAt(0)!;
    if (c < 0x80) out.push(c);
    else if (c < 0x800) out.push(0xc0 | (c >> 6), 0x80 | (c & 63));
    else if (c < 0x10000) out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
    else out.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 63), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
  }
  return out;
}

/** QrCode.encodeText(text, ecc) for byte-mode text: the module grid, `modules[y][x]` dark. */
export function encodeQr(text: string, level: keyof typeof ECC = 'M'): boolean[][] {
  const bytes = utf8(text);
  let ecc = ECC[level], version = 1, used = 0;
  for (;; version++) {
    used = 4 + (version <= 9 ? 8 : 16) + bytes.length * 8;
    if (used <= dataCodewords(version, ecc) * 8) break;
    if (version >= 40) throw new RangeError('Data too long');
  }
  for (const boosted of [ECC.M, ECC.Q, ECC.H]) if (used <= dataCodewords(version, boosted) * 8) ecc = boosted;
  const bits: number[] = [];
  appendBits(0x4, 4, bits); appendBits(bytes.length, version <= 9 ? 8 : 16, bits);
  for (const b of bytes) appendBits(b, 8, bits);
  const capacity = dataCodewords(version, ecc) * 8;
  appendBits(0, Math.min(4, capacity - bits.length), bits);
  appendBits(0, (8 - (bits.length % 8)) % 8, bits);
  for (let pad = 0xec; bits.length < capacity; pad ^= 0xec ^ 0x11) appendBits(pad, 8, bits);
  const data: number[] = new Array(bits.length / 8).fill(0);
  bits.forEach((bit, i) => (data[i >>> 3]! |= bit << (7 - (i & 7))));
  return new QrSymbol(version, ecc, data).modules;
}

class QrSymbol {
  readonly size: number; readonly modules: boolean[][]; private isFunction: boolean[][];
  constructor(readonly version: number, readonly ecc: Ecc, data: number[]) {
    this.size = version * 4 + 17;
    this.modules = Array.from({ length: this.size }, () => new Array(this.size).fill(false));
    this.isFunction = Array.from({ length: this.size }, () => new Array(this.size).fill(false));
    this.drawFunctionPatterns();
    this.drawCodewords(this.addEccAndInterleave(data));
    let mask = 0, min = 1e9;
    for (let i = 0; i < 8; i++) {
      this.applyMask(i); this.drawFormatBits(i);
      const penalty = this.penalty();
      if (penalty < min) { mask = i; min = penalty; }
      this.applyMask(i);
    }
    this.applyMask(mask); this.drawFormatBits(mask);
  }
  private set(x: number, y: number, dark: boolean) { this.modules[y]![x] = dark; this.isFunction[y]![x] = true; }
  private drawFunctionPatterns() {
    for (let i = 0; i < this.size; i++) { this.set(6, i, i % 2 === 0); this.set(i, 6, i % 2 === 0); }
    for (const [x, y] of [[3, 3], [this.size - 4, 3], [3, this.size - 4]] as const)
      for (let dy = -4; dy <= 4; dy++) for (let dx = -4; dx <= 4; dx++) {
        const dist = Math.max(Math.abs(dx), Math.abs(dy)), xx = x + dx, yy = y + dy;
        if (xx >= 0 && xx < this.size && yy >= 0 && yy < this.size) this.set(xx, yy, dist !== 2 && dist !== 4);
      }
    const align = this.alignmentPositions(), n = align.length;
    for (let i = 0; i < n; i++) for (let j = 0; j < n; j++) {
      if ((i === 0 && j === 0) || (i === 0 && j === n - 1) || (i === n - 1 && j === 0)) continue;
      for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) this.set(align[i]! + dx, align[j]! + dy, Math.max(Math.abs(dx), Math.abs(dy)) !== 1);
    }
    this.drawFormatBits(0);
    if (this.version >= 7) {
      let rem = this.version;
      for (let i = 0; i < 12; i++) rem = (rem << 1) ^ ((rem >>> 11) * 0x1f25);
      const bits = (this.version << 12) | rem;
      for (let i = 0; i < 18; i++) { const a = this.size - 11 + (i % 3), b = Math.floor(i / 3); this.set(a, b, getBit(bits, i)); this.set(b, a, getBit(bits, i)); }
    }
  }
  private drawFormatBits(mask: number) {
    const data = (this.ecc.formatBits << 3) | mask;
    let rem = data;
    for (let i = 0; i < 10; i++) rem = (rem << 1) ^ ((rem >>> 9) * 0x537);
    const bits = ((data << 10) | rem) ^ 0x5412;
    for (let i = 0; i <= 5; i++) this.set(8, i, getBit(bits, i));
    this.set(8, 7, getBit(bits, 6)); this.set(8, 8, getBit(bits, 7)); this.set(7, 8, getBit(bits, 8));
    for (let i = 9; i < 15; i++) this.set(14 - i, 8, getBit(bits, i));
    for (let i = 0; i < 8; i++) this.set(this.size - 1 - i, 8, getBit(bits, i));
    for (let i = 8; i < 15; i++) this.set(8, this.size - 15 + i, getBit(bits, i));
    this.set(8, this.size - 8, true);
  }
  private alignmentPositions(): number[] {
    if (this.version === 1) return [];
    const n = Math.floor(this.version / 7) + 2, step = this.version === 32 ? 26 : Math.ceil((this.version * 4 + 4) / (n * 2 - 2)) * 2;
    const result = [6];
    for (let pos = this.size - 7; result.length < n; pos -= step) result.splice(1, 0, pos);
    return result;
  }
  private addEccAndInterleave(data: number[]): number[] {
    const blocks = NUM_ERROR_CORRECTION_BLOCKS[this.ecc.ordinal]![this.version]!, eccLen = ECC_CODEWORDS_PER_BLOCK[this.ecc.ordinal]![this.version]!;
    const raw = Math.floor(rawDataModules(this.version) / 8), short = blocks - (raw % blocks), shortLen = Math.floor(raw / blocks);
    const divisor = rsDivisor(eccLen), out: number[][] = [];
    for (let i = 0, k = 0; i < blocks; i++) {
      const dat = data.slice(k, k + shortLen - eccLen + (i < short ? 0 : 1));
      k += dat.length;
      const ecc = rsRemainder(dat, divisor);
      if (i < short) dat.push(0);
      out.push(dat.concat(ecc));
    }
    const result: number[] = [];
    for (let i = 0; i < out[0]!.length; i++) out.forEach((block, j) => { if (i !== shortLen - eccLen || j >= short) result.push(block[i]!); });
    return result;
  }
  private drawCodewords(data: number[]) {
    let i = 0;
    for (let right = this.size - 1; right >= 1; right -= 2) {
      if (right === 6) right = 5;
      for (let vert = 0; vert < this.size; vert++) for (let j = 0; j < 2; j++) {
        const x = right - j, upward = ((right + 1) & 2) === 0, y = upward ? this.size - 1 - vert : vert;
        if (!this.isFunction[y]![x] && i < data.length * 8) { this.modules[y]![x] = getBit(data[i >>> 3]!, 7 - (i & 7)); i++; }
      }
    }
  }
  private applyMask(mask: number) {
    for (let y = 0; y < this.size; y++) for (let x = 0; x < this.size; x++) {
      const invert = [(x + y) % 2 === 0, y % 2 === 0, x % 3 === 0, (x + y) % 3 === 0, (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0,
        ((x * y) % 2) + ((x * y) % 3) === 0, (((x * y) % 2) + ((x * y) % 3)) % 2 === 0, (((x + y) % 2) + ((x * y) % 3)) % 2 === 0][mask]!;
      if (!this.isFunction[y]![x] && invert) this.modules[y]![x] = !this.modules[y]![x];
    }
  }
  private penalty(): number {
    let result = 0;
    const line = (get: (i: number) => boolean) => {
      let color = false, run = 0;
      const history = [0, 0, 0, 0, 0, 0, 0];
      for (let i = 0; i < this.size; i++) {
        if (get(i) === color) { run++; if (run === 5) result += 3; else if (run > 5) result++; }
        else { this.addHistory(run, history); if (!color) result += this.countPatterns(history) * 40; color = get(i); run = 1; }
      }
      if (color) { this.addHistory(run, history); run = 0; }
      run += this.size; this.addHistory(run, history);
      result += this.countPatterns(history) * 40;
    };
    for (let y = 0; y < this.size; y++) line(x => this.modules[y]![x]!);
    for (let x = 0; x < this.size; x++) line(y => this.modules[y]![x]!);
    for (let y = 0; y < this.size - 1; y++) for (let x = 0; x < this.size - 1; x++) {
      const c = this.modules[y]![x];
      if (c === this.modules[y]![x + 1] && c === this.modules[y + 1]![x] && c === this.modules[y + 1]![x + 1]) result += 3;
    }
    const dark = this.modules.reduce((sum, row) => sum + row.filter(Boolean).length, 0), total = this.size * this.size;
    return result + (Math.ceil(Math.abs(dark * 20 - total * 10) / total) - 1) * 10;
  }
  private countPatterns(h: number[]): number {
    const n = h[1]!, core = n > 0 && h[2] === n && h[3] === n * 3 && h[4] === n && h[5] === n;
    return (core && h[0]! >= n * 4 && h[6]! >= n ? 1 : 0) + (core && h[6]! >= n * 4 && h[0]! >= n ? 1 : 0);
  }
  private addHistory(run: number, h: number[]) { if (h[0] === 0) run += this.size; h.pop(); h.unshift(run); }
}

export type QrCell = { x: number; y: number; w: number; h: number };
/**
 * QRCodeSvg's path as boxes: one per horizontal dark run, in points, inside a
 * `size`-point square with `margin` light modules on every side. Edges snap to
 * half points (shapeRendering crispEdges on a 2x display).
 */
export function qrCells(text: string, size = 128, margin = 1, level: keyof typeof ECC = 'M'): QrCell[] {
  const modules = encodeQr(text, level), n = modules.length, unit = size / (n + margin * 2);
  const snap = (v: number) => Math.round(v * 2) / 2;
  const cells: QrCell[] = [];
  modules.forEach((row, y) => {
    let start = -1;
    for (let x = 0; x <= n; x++) {
      if (x < n && row[x]) { if (start < 0) start = x; continue; }
      if (start < 0) continue;
      const left = snap((start + margin) * unit), top = snap((y + margin) * unit);
      cells.push({ x: left, y: top, w: snap((x + margin) * unit) - left, h: snap((y + 1 + margin) * unit) - top });
      start = -1;
    }
  });
  return cells;
}
