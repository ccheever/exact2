// A PNG's pixels as 8-bit RGBA, for the image chip's accent (image-accent.ts), which reads the small PNG that
// `storage.fs.compressImage` writes: the data module has no image decode. The zlib stream is inflated here (RFC 1950,
// RFC 1951); the PNG is read as the W3C PNG specification (third edition) lays it out: every colour type and bit
// depth, `tRNS`, and the five filters. An interlaced image, a bad stream or one over `maxPixels` reads as null.
// Chunk CRCs and the Adler-32 checksum are not verified, nor a stored block's NLEN, nor that a Huffman code is complete.

export interface Pixels { width: number; height: number; rgba: Uint8Array }

const LENGTH_BASE = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DISTANCE_BASE = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DISTANCE_EXTRA = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
/** The order a dynamic block lists its code-length code lengths in (RFC 1951 §3.2.7). */
const CODE_LENGTH_ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/** A canonical Huffman code: how many codes each length has, and the symbols in code order. */
interface Code { counts: Uint16Array; symbols: Uint16Array }
function code(lengths: ArrayLike<number>): Code {
  const counts = new Uint16Array(16), offsets = new Uint16Array(16), symbols = new Uint16Array(lengths.length);
  for (let i = 0; i < lengths.length; i++) counts[lengths[i]!]!++;
  counts[0] = 0;
  for (let length = 1; length < 16; length++) offsets[length] = offsets[length - 1]! + counts[length - 1]!;
  for (let i = 0; i < lengths.length; i++) if (lengths[i]) symbols[offsets[lengths[i]!]!++] = i;
  return { counts, symbols };
}
let fixed: { literals: Code; distances: Code } | null = null;
function fixedCodes() {
  if (fixed) return fixed;
  const lengths = new Uint8Array(288);
  lengths.fill(8, 0, 144); lengths.fill(9, 144, 256); lengths.fill(7, 256, 280); lengths.fill(8, 280, 288);
  return fixed = { literals: code(lengths), distances: code(new Uint8Array(30).fill(5)) };
}

class Malformed extends Error {}

/** Inflates the zlib stream `input` into exactly `size` bytes. */
export function inflate(input: Uint8Array, size: number): Uint8Array {
  if (input.length < 2 || (input[0]! & 15) !== 8 || ((input[0]! << 8) | input[1]!) % 31 !== 0 || input[1]! & 32) throw new Malformed('not a zlib stream');
  const out = new Uint8Array(size);
  let at = 2, bit = 0, written = 0;
  const bits = (count: number): number => {
    let value = 0;
    for (let i = 0; i < count; i++) {
      if (at >= input.length) throw new Malformed('the stream ends early');
      value |= ((input[at]! >> bit) & 1) << i;
      if (++bit === 8) { bit = 0; at++; }
    }
    return value;
  };
  const decode = ({ counts, symbols }: Code): number => {
    let value = 0, first = 0, index = 0;
    for (let length = 1; length < 16; length++) {
      value |= bits(1);
      const count = counts[length]!;
      if (value - first < count) return symbols[index + value - first]!;
      index += count; first = (first + count) << 1; value <<= 1;
    }
    throw new Malformed('a bad code');
  };
  const put = (byte: number) => { if (written >= size) throw new Malformed('more data than the image holds'); out[written++] = byte; };
  for (let last = 0; !last;) {
    last = bits(1);
    const type = bits(2);
    if (type === 0) {
      if (bit) { bit = 0; at++; }
      if (at + 4 > input.length) throw new Malformed('the stream ends early');
      const length = input[at]! | (input[at + 1]! << 8);
      at += 4;
      if (at + length > input.length) throw new Malformed('the stream ends early');
      for (let i = 0; i < length; i++) put(input[at++]!);
      continue;
    }
    if (type === 3) throw new Malformed('a reserved block type');
    let literals: Code, distances: Code;
    if (type === 1) ({ literals, distances } = fixedCodes());
    else {
      const literalCount = bits(5) + 257, distanceCount = bits(5) + 1, lengthCount = bits(4) + 4;
      const lengthLengths = new Uint8Array(19);
      for (let i = 0; i < lengthCount; i++) lengthLengths[CODE_LENGTH_ORDER[i]!] = bits(3);
      const lengthCode = code(lengthLengths), lengths = new Uint8Array(literalCount + distanceCount);
      for (let i = 0; i < lengths.length;) {
        const symbol = decode(lengthCode);
        if (symbol < 16) { lengths[i++] = symbol; continue; }
        const repeat = symbol === 16 ? 3 + bits(2) : symbol === 17 ? 3 + bits(3) : 11 + bits(7);
        if (symbol === 16 && i === 0) throw new Malformed('a repeat with nothing before it');
        const value = symbol === 16 ? lengths[i - 1]! : 0;
        if (i + repeat > lengths.length) throw new Malformed('too many code lengths');
        lengths.fill(value, i, i + repeat); i += repeat;
      }
      literals = code(lengths.subarray(0, literalCount)); distances = code(lengths.subarray(literalCount));
    }
    for (;;) {
      const symbol = decode(literals);
      if (symbol < 256) { put(symbol); continue; }
      if (symbol === 256) break;
      const index = symbol - 257;
      if (index >= 29) throw new Malformed('a bad length');
      const length = LENGTH_BASE[index]! + bits(LENGTH_EXTRA[index]!);
      const distanceSymbol = decode(distances);
      if (distanceSymbol >= 30) throw new Malformed('a bad distance');
      const distance = DISTANCE_BASE[distanceSymbol]! + bits(DISTANCE_EXTRA[distanceSymbol]!);
      if (distance > written) throw new Malformed('a distance before the start');
      for (let i = 0; i < length; i++) put(out[written - distance]!);
    }
  }
  if (written !== size) throw new Malformed('less data than the image holds');
  return out;
}

const SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
/** Channels per colour type: greyscale, truecolour, indexed, greyscale with alpha, truecolour with alpha. */
const CHANNELS: Record<number, number> = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 };
const DEPTHS: Record<number, number[]> = { 0: [1, 2, 4, 8, 16], 2: [8, 16], 3: [1, 2, 4, 8], 4: [8, 16], 6: [8, 16] };

/** The PNG `bytes` as 8-bit RGBA, or null when it is not one this reads (see the header). */
export function decodePng(bytes: Uint8Array, maxPixels = 1 << 20): Pixels | null {
  try { return decode(bytes, maxPixels); } catch (error) { if (error instanceof Malformed || error instanceof RangeError) return null; throw error; }
}

function decode(bytes: Uint8Array, maxPixels: number): Pixels | null {
  if (bytes.length < 8 || SIGNATURE.some((byte, i) => bytes[i] !== byte)) return null;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let width = 0, height = 0, depth = 0, type = -1, palette: Uint8Array | null = null, transparency: Uint8Array | null = null;
  const data: Uint8Array[] = [];
  for (let at = 8; at + 8 <= bytes.length;) {
    const start = at + 8, length = view.getUint32(at), name = String.fromCharCode(...bytes.subarray(at + 4, start)), body = bytes.subarray(start, start + length);
    if (body.length !== length) return null;
    at = start + length + 4;
    if (name === 'IHDR') {
      if (length < 13) return null;
      width = view.getUint32(start); height = view.getUint32(start + 4);
      [depth, type] = [body[8]!, body[9]!];
      if (body[10] !== 0 || body[11] !== 0 || body[12] !== 0) return null; // compression, filter method, interlace
    } else if (name === 'PLTE') palette = body;
    else if (name === 'tRNS') transparency = body;
    else if (name === 'IDAT') data.push(body);
    else if (name === 'IEND') break;
  }
  const channels = CHANNELS[type];
  if (!channels || !DEPTHS[type]!.includes(depth) || !width || !height || width * height > maxPixels || (type === 3 && !palette)) return null;
  const pixelBits = channels * depth, stride = Math.ceil((width * pixelBits) / 8), step = Math.max(1, pixelBits >> 3);
  const joined = new Uint8Array(data.reduce((total, part) => total + part.length, 0));
  data.reduce((at, part) => { joined.set(part, at); return at + part.length; }, 0);
  const raw = inflate(joined, height * (stride + 1));
  // Unfilter each row in place (PNG §7.3, §9): the filtered bytes follow each row's filter type.
  const rows = new Uint8Array(height * stride);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)]!, from = y * (stride + 1) + 1, row = y * stride;
    if (filter > 4) return null;
    for (let x = 0; x < stride; x++) {
      const left = x >= step ? rows[row + x - step]! : 0, up = y ? rows[row - stride + x]! : 0, corner = y && x >= step ? rows[row - stride + x - step]! : 0;
      let predicted = 0;
      if (filter === 1) predicted = left;
      else if (filter === 2) predicted = up;
      else if (filter === 3) predicted = (left + up) >> 1;
      else if (filter === 4) {
        const estimate = left + up - corner, toLeft = Math.abs(estimate - left), toUp = Math.abs(estimate - up), toCorner = Math.abs(estimate - corner);
        predicted = toLeft <= toUp && toLeft <= toCorner ? left : toUp <= toCorner ? up : corner;
      }
      rows[row + x] = (raw[from + x]! + predicted) & 255;
    }
  }
  const rgba = new Uint8Array(width * height * 4), max = (1 << Math.min(depth, 8)) - 1;
  // Sample `index` of row `y`: its full value at the image's depth.
  const sample = (y: number, index: number): number => {
    if (depth === 8) return rows[y * stride + index]!;
    if (depth === 16) return (rows[y * stride + index * 2]! << 8) | rows[y * stride + index * 2 + 1]!;
    const bitAt = index * depth;
    return (rows[y * stride + (bitAt >> 3)]! >> (8 - depth - (bitAt & 7))) & max;
  };
  const eight = (value: number) => depth === 16 ? value >> 8 : depth === 8 ? value : Math.round((value * 255) / max);
  const key = transparency && (type === 0 || type === 2) && transparency.length >= (type === 0 ? 2 : 6)
    ? Array.from({ length: type === 0 ? 1 : 3 }, (_, i) => (transparency![i * 2]! << 8) | transparency![i * 2 + 1]!) : null;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const out = (y * width + x) * 4, first = x * channels;
      let r: number, g: number, b: number, a = 255;
      if (type === 3) {
        const index = sample(y, first);
        if (index * 3 + 2 >= palette!.length) return null;
        [r, g, b] = [palette![index * 3]!, palette![index * 3 + 1]!, palette![index * 3 + 2]!];
        if (transparency && index < transparency.length) a = transparency[index]!;
      } else {
        const values = Array.from({ length: channels }, (_, i) => sample(y, first + i));
        if (key && key.every((value, i) => value === values[i])) a = 0;
        if (type === 0 || type === 4) { r = g = b = eight(values[0]!); if (type === 4) a = eight(values[1]!); }
        else { [r, g, b] = [eight(values[0]!), eight(values[1]!), eight(values[2]!)]; if (type === 6) a = eight(values[3]!); }
      }
      rgba[out] = r; rgba[out + 1] = g; rgba[out + 2] = b; rgba[out + 3] = a;
    }
  }
  return { width, height, rgba };
}
