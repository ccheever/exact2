// What an image's first bytes say: its type, its stored size and whether it can hold transparency. T3 Code reads the
// size from the header before it decodes (lib/imageCompression.ts readImageDimensions, 9c161462c9; MIT, see
// LICENSE-T3); staging reads the type too, since a dropped file's name is the only other evidence of it.

export type ImageHeader = { type: 'image/png' | 'image/jpeg' | 'image/gif' | 'image/webp'; width: number; height: number; alpha: boolean };

const ascii = (bytes: Uint8Array, at: number, text: string) => [...text].every((char, index) => bytes[at + index] === char.charCodeAt(0));

function png(bytes: Uint8Array, view: DataView): ImageHeader | null {
  if (bytes.length < 33 || ![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a].every((byte, index) => bytes[index] === byte)) return null;
  // Colour types 4 and 6 carry alpha; a tRNS chunk before the image data makes the others transparent.
  let alpha = bytes[25] === 4 || bytes[25] === 6;
  for (let at = 8; !alpha && at + 8 <= bytes.length;) {
    const length = view.getUint32(at);
    if (ascii(bytes, at + 4, 'IDAT') || ascii(bytes, at + 4, 'IEND')) break;
    if (ascii(bytes, at + 4, 'tRNS')) alpha = true;
    at += 12 + length;
  }
  return { type: 'image/png', width: view.getUint32(16), height: view.getUint32(20), alpha };
}

function jpeg(bytes: Uint8Array, view: DataView): ImageHeader | null {
  if (bytes[0] !== 0xff || bytes[1] !== 0xd8) return null;
  for (let at = 2; at + 9 <= bytes.length;) {
    if (bytes[at] !== 0xff) return null;
    const marker = bytes[at + 1]!;
    if (marker === 0xd8 || (marker >= 0xd0 && marker <= 0xd7) || marker === 0x01) { at += 2; continue; }
    // SOF0–SOF15, less DHT (C4), JPG (C8) and DAC (CC): height then width.
    if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
      return { type: 'image/jpeg', width: view.getUint16(at + 7), height: view.getUint16(at + 5), alpha: false };
    }
    at += 2 + view.getUint16(at + 2);
  }
  return null;
}

function gif(bytes: Uint8Array, view: DataView): ImageHeader | null {
  if (bytes.length < 10 || !ascii(bytes, 0, 'GIF8')) return null;
  return { type: 'image/gif', width: view.getUint16(6, true), height: view.getUint16(8, true), alpha: true };
}

function webp(bytes: Uint8Array, view: DataView): ImageHeader | null {
  if (bytes.length < 30 || !ascii(bytes, 0, 'RIFF') || !ascii(bytes, 8, 'WEBP')) return null;
  const u24 = (at: number) => bytes[at]! | (bytes[at + 1]! << 8) | (bytes[at + 2]! << 16);
  if (ascii(bytes, 12, 'VP8X')) return { type: 'image/webp', width: u24(24) + 1, height: u24(27) + 1, alpha: (bytes[20]! & 0x10) !== 0 };
  if (ascii(bytes, 12, 'VP8L')) {
    const bits = view.getUint32(21, true);
    return { type: 'image/webp', width: (bits & 0x3fff) + 1, height: ((bits >>> 14) & 0x3fff) + 1, alpha: ((bits >>> 28) & 1) === 1 };
  }
  if (ascii(bytes, 12, 'VP8 ')) return { type: 'image/webp', width: view.getUint16(26, true) & 0x3fff, height: view.getUint16(28, true) & 0x3fff, alpha: false };
  return null;
}

/** The header of a PNG, JPEG, GIF or WebP, or null for other bytes (HEIC among them, which only decoding sizes). */
export function imageHeader(bytes: Uint8Array): ImageHeader | null {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  try { return png(bytes, view) ?? jpeg(bytes, view) ?? gif(bytes, view) ?? webp(bytes, view); } catch { return null; }
}
