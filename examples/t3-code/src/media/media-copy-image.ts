// Copy image (T3 Code 5d3d78ab93, MIT; see LICENSE-T3): MediaActions copyImage over mediaContent.ts readMediaPng,
// and the Browser's screenshot (PreviewView copyImage, Manager.ts copyArtifactToClipboard). The image goes to the
// clipboard as a PNG through @exact/clipboard's `write` (LLP 1133.006), which re-encodes its pixels as Chrome's
// clipboard does. readMediaPng's steps: the media's bytes as readMediaBlob reads them (a failed fetch, an HTTP error
// and a web page refused with its sentences); a PNG as it is; anything else drawn at its own size into a PNG
// (storage.fs.transformImage), refused over 64,000,000 pixels. The bytes stay in files under app:/tmp/copy-image
// until the copy ends: `fetch` writes the reply's body there, and an inline `data:` image, which `fetch` does not
// take, is decoded here. The image codec reads no SVG yet (LLP 1133.006 §9, item 5), so an SVG goes to `svg`, which
// draws it in the module (T3MediaActions.swift `mediaCopyImage`) until the codec can.
// The bake binds the package from its exact.json; Bun's tests read the exact.json itself, so they hand in their own.
import * as clipboardPackage from '@exact/clipboard';
import { appFiles } from './save-file';
import { letGo } from '../client/let-go';

export const COPY_IMAGE_DIR = 'app:/tmp/copy-image';
/** The desktop app's words: its fetch has no CORS, so only the connection can stop it. */
export const FETCH_FAILED = 'The file could not be fetched. Check the connection and try again.';
export const INLINE_UNDECODABLE = 'This inline media could not be decoded.';
export const WEB_PAGE = 'This link returned a web page instead of media. Open the original URL.';
export const UNDECODABLE = 'The browser could not decode this image for copying. Try saving it instead.';
export const TOO_LARGE = 'This image is too large or has no usable dimensions. Try saving it instead.';
export const NOT_PNG = 'The image could not be converted to PNG.';
const MAX_PIXELS = 64_000_000;
/** The largest file `fetch` writes for a copy, and the largest inline image decoded: the image codec's bound. */
const MAX_BYTES = 64 * 1024 * 1024;

export type ClipboardCalls = { write(items: object[]): Promise<void>; ClipboardItem: new (items: Record<string, unknown>) => object };
let clipboard = clipboardPackage as unknown as ClipboardCalls;
/** A test's own `@exact/clipboard` (null puts the package back). */
export function useClipboardCalls(fake: ClipboardCalls | null): void { clipboard = fake ?? clipboardPackage as unknown as ClipboardCalls; }

/** The PNG at `path` (an app:/ file) on the clipboard; a refusal in the reference's words where it has them. */
export async function writePng(path: string): Promise<void> {
  try { await clipboard.write([new clipboard.ClipboardItem({ 'image/png': path })]); }
  catch (error) {
    if (letGo(error)) throw error;
    const { name, message } = error as { name?: string; message?: string };
    if (name === 'DataError') throw new Error(/too large/i.test(message ?? '') ? TOO_LARGE : NOT_PNG);
    throw error;
  }
}

type ImageFiles = {
  mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>;
  writeFile(path: string, bytes: Uint8Array): Promise<unknown>;
  rm(path: string): Promise<unknown>;
  transformImage(from: string, to: string, options: { type: 'image/png' }): Promise<{ path: string; width: number; height: number }>;
};
const mimeOf = (value: string | null | undefined) => (value ?? '').split(';', 1)[0]!.trim().toLowerCase();

/** readMediaBlob over `fetch`: the body is written to `path`; answers the reply's type. */
async function fetchTo(url: string, path: string): Promise<string> {
  let reply: Response;
  try { reply = await fetch(url, { exactTimeout: 60_000, exactBodyTo: path, exactIndependentHttp: { maxResponseBytes: MAX_BYTES } } as RequestInit); }
  catch (error) { if (letGo(error)) throw error; throw new Error(FETCH_FAILED); }
  if (!reply.ok) throw new Error(`The file could not be fetched (HTTP ${reply.status}).`);
  return mimeOf(reply.headers.get('content-type'));
}

/**
 * readDataUrlBlob: an inline `data:` URL's media type and bytes (RFC 2397). A fragment is never part of the payload, and
 * a `%XX` escape is one raw byte, not UTF-8.
 */
export function dataUrlBytes(src: string): { type: string; bytes: Uint8Array } {
  const url = src.split('#', 1)[0]!, comma = url.indexOf(',');
  if (!url.startsWith('data:') || comma < 0) throw new Error(INLINE_UNDECODABLE);
  const [type = '', ...parameters] = url.slice(5, comma).split(';'), body = url.slice(comma + 1);
  const base64 = parameters.some(parameter => parameter.trim().toLowerCase() === 'base64');
  if ((base64 ? body.length * 0.75 : body.length) > MAX_BYTES) throw new Error(TOO_LARGE);
  let bytes: Uint8Array;
  try {
    bytes = base64
      ? Uint8Array.from(atob(decodeURIComponent(body)), c => c.charCodeAt(0))
      : Uint8Array.from(body.split(/(%[0-9a-f]{2})/i).flatMap(part => /^%[0-9a-f]{2}$/i.test(part) ? [Number.parseInt(part.slice(1), 16)] : [...new TextEncoder().encode(part)]));
  } catch { throw new Error(INLINE_UNDECODABLE); }
  return { type: mimeOf(type) || 'text/plain', bytes };
}

let copies = 0;
/** One copy's files under app:/tmp/copy-image, removed when the copy ends (`done`). */
async function copyFiles() {
  const fs = appFiles().fs as unknown as ImageFiles, n = ++copies;
  await fs.mkdir(COPY_IMAGE_DIR, { recursive: true });
  const source = `${COPY_IMAGE_DIR}/${n}-source`, png = `${COPY_IMAGE_DIR}/${n}.png`;
  return { fs, source, png, done: async (...more: string[]) => { for (const path of [source, png, ...more]) await fs.rm(path).catch(() => undefined); } };
}
/** The image at `source` drawn into a PNG at `to` at its own size, or the reference's refusal. */
async function drawPng(fs: ImageFiles, source: string, to: string): Promise<string> {
  let drawn: { path: string; width: number; height: number };
  try { drawn = await fs.transformImage(source, to, { type: 'image/png' }); }
  catch (error) {
    if (letGo(error)) throw error;
    const code = (error as { code?: string }).code;
    throw new Error(code === 'too-large' ? TOO_LARGE : code === 'undecodable' ? UNDECODABLE : NOT_PNG);
  }
  if (!(drawn.width > 0 && drawn.height > 0) || drawn.width * drawn.height > MAX_PIXELS) throw new Error(TOO_LARGE);
  return drawn.path || to;
}

/** readMediaPng, then the clipboard: the media at `url` as a PNG. `svg` copies an SVG the codec cannot draw. */
export async function copyMediaImage(url: string, svg: () => Promise<void>): Promise<void> {
  const { fs, source, png, done } = await copyFiles();
  try {
    let type: string;
    if (url.startsWith('data:')) {
      const inline = dataUrlBytes(url);
      await fs.writeFile(source, inline.bytes);
      type = inline.type;
    } else type = await fetchTo(url, source);
    if (type === 'text/html') throw new Error(WEB_PAGE);
    if (type === 'image/svg+xml' || (!type.startsWith('image/') && /\.svg(?:[?#]|$)/i.test(url))) return await svg();
    await writePng(type === 'image/png' ? source : await drawPng(fs, source, png));
  } finally { await done(); }
}

/**
 * copyArtifactToClipboard (Manager.ts): an image file's pixels as a PNG on the clipboard (nativeImage.createFromPath,
 * toPNG). `staged` is a copy under app:/tmp/copy-image, which this removes.
 */
export async function copyImageFile(staged: string): Promise<void> {
  const { fs, png, done } = await copyFiles();
  try { await writePng(await drawPng(fs, staged, png)); }
  finally { await done(staged); }
}
