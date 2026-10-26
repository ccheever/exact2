// An image chip's accent, adapted from T3 Code 74541bab17 (MIT; see LICENSE-T3): contextChipParts.tsx averageImageColor,
// the picture drawn at 16×16 and averaged with each pixel weighted by its alpha, so transparent pixels do not darken it.
// The data module has no canvas: the server's image is downloaded into app:/tmp (the host writes the body, `fetch`'s
// `exactBodyTo`), `storage.fs.compressImage` scales it to a PNG, and png-decode.ts reads the pixels. The mean of an area does not depend on the size or aspect it is drawn at, so
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
  const pixels = decodePng(bytes, ACCENT_SIDE * ACCENT_SIDE);
  return pixels ? averageAccent(pixels.rgba) : '';
}

/** compressImage's refusals that a later read would repeat (docs/reference.md, "Shrink a picked image for upload"). */
const LASTING = new Set(['too-large', 'undecodable', 'unfit', 'unsupported']);

/**
 * The accent of the image at the http(s) `url`, or "" when it has none that a later read would find: a non-2xx reply,
 * bytes compressImage cannot decode, or a fully transparent picture (the chip keeps the image kind's own tone). Null
 * when this read failed for a passing reason (a timeout, the network, a full storage queue), so the caller may ask
 * again. A let-go answer's rejection goes on up (client/let-go.ts).
 */
export async function imageAccent(url: string, isLetGo: (error: unknown) => boolean): Promise<string | null> {
  if (!/^https?:\/\//i.test(url)) return '';
  const fs = files(), base = `${ACCENT_SCRATCH}/${++serial}`, from = `${base}.image`, to = `${base}.png`;
  try {
    await fs.mkdir(ACCENT_SCRATCH, { recursive: true }).catch(() => undefined);
    const response = await fetch(url, { exactTimeout: 5000, exactBodyTo: from, exactIndependentHttp: { maxResponseBytes: MAX_IMAGE_BYTES } } as RequestInit);
    if (!response.ok) return '';
    let scaled: { path: string };
    try { scaled = await fs.compressImage(from, to, { maxDimension: ACCENT_SIDE, maxBytes: 1024 * 1024, type: 'image/png' }); }
    catch (error) { if (LASTING.has(String((error as { code?: unknown } | null)?.code ?? ''))) return ''; throw error; }
    return pngAccent(new Uint8Array(await fs.readFile(scaled.path || to)));
  } catch (error) {
    if (isLetGo(error)) throw error;
    return null;
  } finally {
    await fs.rm(from).catch(() => undefined);
    await fs.rm(to).catch(() => undefined);
  }
}
