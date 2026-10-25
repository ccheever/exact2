// The fixture's stand-ins for Discord's avatar CDN: a 128-pixel PNG of a head and shoulders on a
// colour picked from the path, so each user keeps one picture. Discord's default avatars
// (`/embed/avatars/<0-5>.png`) take its six default colours.
import { deflateSync } from 'node:zlib';

const DEFAULTS = [0x5865f2, 0x757e8a, 0x3ba55c, 0xfaa61a, 0xed4245, 0xeb459f];
const PALETTE = [0x1abc9c, 0x3498db, 0x9b59b6, 0xe67e22, 0xe91e63, 0x2ecc71, 0xf1c40f, 0x607d8b, 0x795548, 0x00bcd4];
const SIZE = 128;

const CRC = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length), view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(new TextEncoder().encode(type), 4);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

function png(rgb: number): Uint8Array {
  const raw = new Uint8Array(SIZE * (1 + SIZE * 3));
  const [r, g, b] = [rgb >> 16 & 0xff, rgb >> 8 & 0xff, rgb & 0xff];
  const light = (v: number) => Math.round(v + (255 - v) * 0.85);
  for (let y = 0; y < SIZE; y++) {
    const row = y * (1 + SIZE * 3);
    for (let x = 0; x < SIZE; x++) {
      const head = (x - 64) ** 2 + (y - 52) ** 2 < 22 ** 2;
      const body = y > 84 && (x - 64) ** 2 / 40 ** 2 + (y - 124) ** 2 / 36 ** 2 < 1;
      const on = head || body;
      raw.set(on ? [light(r), light(g), light(b)] : [r, g, b], row + 1 + x * 3);
    }
  }
  const header = new Uint8Array(13), view = new DataView(header.buffer);
  view.setUint32(0, SIZE);
  view.setUint32(4, SIZE);
  header.set([8, 2, 0, 0, 0], 8);
  const parts = [new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw)), chunk('IEND', new Uint8Array())];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) { out.set(p, at); at += p.length; }
  return out;
}

const made = new Map<string, Uint8Array>();
/** The picture for a CDN path (a user's avatar, a server's or a group's icon), or null for any other path. */
export function avatarPng(path: string): Uint8Array | null {
  const fallback = /^\/embed\/avatars\/([0-5])\.png$/.exec(path);
  const own = /^\/(?:avatars|icons|channel-icons)\/(\d{1,20})\/([0-9a-z_]{1,40})\.png$/.exec(path);
  if (!fallback && !own) return null;
  const color = fallback ? DEFAULTS[Number(fallback[1])]
    : PALETTE[[...own![2]].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 7) % PALETTE.length];
  let found = made.get(path);
  if (!found) made.set(path, found = png(color));
  return found;
}

/** A PNG's, GIF's or JPEG's pixel size from its header, as Discord reports an image attachment's; {} for
 * anything else. */
export function imageSize(b: Uint8Array): { width?: number; height?: number } {
  const view = new DataView(b.buffer, b.byteOffset, b.byteLength);
  if (b.length >= 24 && b[0] === 0x89 && b[1] === 0x50) return { width: view.getUint32(16), height: view.getUint32(20) };
  if (b.length >= 10 && b[0] === 0x47 && b[1] === 0x49) return { width: view.getUint16(6, true), height: view.getUint16(8, true) };
  if (b.length >= 4 && b[0] === 0xff && b[1] === 0xd8) {
    for (let i = 2; i + 9 < b.length;) {
      if (b[i] !== 0xff) return {};
      const marker = b[i + 1], length = view.getUint16(i + 2);
      if (marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker)) {
        return { height: view.getUint16(i + 5), width: view.getUint16(i + 7) };
      }
      i += 2 + length;
    }
  }
  return {};
}
