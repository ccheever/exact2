// An image chip's accent, adapted from T3 Code 74541bab17 (MIT; see LICENSE-T3): contextChipParts.tsx averageImageColor,
// the picture drawn at 16×16 and averaged with each pixel weighted by its alpha, so transparent pixels do not darken it.
// The data module has no canvas: the server's image is fetched into app:/tmp, `storage.fs.compressImage` scales it to a
// PNG, and png-decode.ts reads the pixels. The mean of an area does not depend on the size or aspect it is drawn at, so
// the PNG is 128 px on its longer side: at 16 px, compressImage's two scalings (ImageIO's thumbnail, then the bitmap)
// move an accent by up to 7 units per channel from the whole picture's; at 128 px it is within one.
import { appFiles } from './save-file';
import { decodePng } from './png-decode';

/** Where the fetched image and its scaled PNG are written for the moment of the read (app.ts grants). */
export const ACCENT_SCRATCH = 'app:/tmp/image-accent';
/** PROVIDER_SEND_TURN_MAX_IMAGE_BYTES, with room: a larger reply is refused by the fetch, and the chip keeps its tone. */
const MAX_IMAGE_BYTES = 12 * 1024 * 1024;
const ACCENT_SIDE = 128;

interface AccentFiles {
  writeFile(path: string, bytes: Uint8Array): Promise<unknown>;
  readFile(path: string): Promise<ArrayBuffer>;
  mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>;
  rm(path: string): Promise<unknown>;
  compressImage(from: string, to: string, options: { maxDimension: number; maxBytes: number; type: 'image/png' }): Promise<{ path: string }>;
}
const files = (): AccentFiles => (appFiles() as unknown as { fs: AccentFiles }).fs;
let serial = 0;

/** The alpha-weighted mean of 8-bit RGBA pixels as "r g b", or "" when every pixel is transparent. */
export function averageAccent(rgba: Uint8Array): string {
  let red = 0, green = 0, blue = 0, alpha = 0;
  for (let index = 0; index < rgba.length; index += 4) {
    const weight = rgba[index + 3]!;
    red += rgba[index]! * weight; green += rgba[index + 1]! * weight; blue += rgba[index + 2]! * weight; alpha += weight;
  }
  return alpha === 0 ? '' : `${Math.round(red / alpha)} ${Math.round(green / alpha)} ${Math.round(blue / alpha)}`;
}

/** The accent of the PNG `bytes`, "" when they are not one or are fully transparent. */
export function pngAccent(bytes: Uint8Array): string {
  const pixels = decodePng(bytes);
  return pixels ? averageAccent(pixels.rgba) : '';
}

/**
 * The accent of the image at the http(s) `url`: "" when it cannot be fetched or read, so the chip keeps the image kind's
 * own tone. A let-go answer's rejection goes on up (client/let-go.ts).
 */
export async function imageAccent(url: string, isLetGo: (error: unknown) => boolean): Promise<string> {
  if (!/^https?:\/\//i.test(url)) return '';
  const fs = files(), base = `${ACCENT_SCRATCH}/${++serial}`, from = `${base}.image`, to = `${base}.png`;
  try {
    const response = await fetch(url, { exactTimeout: 5000, exactIndependentHttp: { maxResponseBytes: MAX_IMAGE_BYTES } } as RequestInit);
    if (!response.ok) return '';
    const bytes = new Uint8Array(await response.arrayBuffer());
    await fs.mkdir(ACCENT_SCRATCH, { recursive: true }).catch(() => undefined);
    await fs.writeFile(from, bytes);
    const scaled = await fs.compressImage(from, to, { maxDimension: ACCENT_SIDE, maxBytes: 1024 * 1024, type: 'image/png' });
    return pngAccent(new Uint8Array(await fs.readFile(scaled.path || to)));
  } catch (error) {
    if (isLetGo(error)) throw error;
    return '';
  } finally {
    await fs.rm(from).catch(() => undefined);
    await fs.rm(to).catch(() => undefined);
  }
}
