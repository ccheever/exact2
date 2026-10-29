// Copy image (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3): MediaActions copyImage over mediaContent.ts readMediaPng,
// and the Browser's screenshot (PreviewView copyImage, Manager.ts copyArtifactToClipboard). The image goes to the
// clipboard as a PNG through @exact/clipboard's `write` (LLP 1133.006), which re-encodes its pixels as Chrome's
// clipboard does. readMediaPng's steps: the media's bytes as readMediaBlob reads them (a failed fetch, an HTTP error
// and a web page refused with its sentences); a PNG as it is; anything else drawn at its own size into a PNG
// (storage.fs.transformImage), refused over 64,000,000 pixels. The bytes stay in files under app:/tmp/copy-image:
// `fetch` writes the reply's body there, and an inline `data:` image, which `fetch` does not take, is decoded here.
// The image codec reads no SVG yet (LLP 1133.006 §9, item 5), so an SVG goes to `svg`, the module's
// (T3MediaActions.swift `mediaCopyImage`; clone/issues/native-copy-image.md).
// The bake binds the package from its exact.json; Bun's tests read the exact.json itself, so they hand in their own.
import * as clipboardPackage from '@exact/clipboard';
import { appFiles } from './save-file';
import { letGo } from '../client/let-go';

export const COPY_IMAGE_DIR = 'app:/tmp/copy-image';
export const FETCH_FAILED = 'The file could not be fetched. The host may block browser access (CORS), or the connection may be unavailable.';
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

/** An inline `data:` URL's media type and bytes (RFC 2397). */
export function dataUrlBytes(url: string): { type: string; bytes: Uint8Array } {
  const comma = url.indexOf(',');
  if (!url.startsWith('data:') || comma < 0) throw new Error(FETCH_FAILED);
  const meta = url.slice(5, comma), body = url.slice(comma + 1), base64 = /;base64$/i.test(meta);
  if ((base64 ? body.length * 0.75 : body.length) > MAX_BYTES) throw new Error(TOO_LARGE);
  let bytes: Uint8Array;
  try { bytes = base64 ? Uint8Array.from(atob(body.replace(/\s+/g, '')), c => c.charCodeAt(0)) : new TextEncoder().encode(decodeURIComponent(body)); }
  catch { throw new Error(FETCH_FAILED); }
  return { type: mimeOf(base64 ? meta.slice(0, -7) : meta) || 'text/plain', bytes };
}

let copies = 0;
/** readMediaPng, then the clipboard: the media at `url` as a PNG. `svg` copies an SVG the codec cannot draw. */
export async function copyMediaImage(url: string, svg: () => Promise<void>): Promise<void> {
  const fs = appFiles().fs as unknown as ImageFiles, directory = `${COPY_IMAGE_DIR}/${++copies}`, source = `${directory}/source`;
  await fs.mkdir(directory, { recursive: true });
  let type: string;
  if (url.startsWith('data:')) {
    const inline = dataUrlBytes(url);
    await fs.writeFile(source, inline.bytes);
    type = inline.type;
  } else type = await fetchTo(url, source);
  if (type === 'text/html') throw new Error(WEB_PAGE);
  if (type === 'image/svg+xml' || (!type.startsWith('image/') && /\.svg(?:[?#]|$)/i.test(url))) return svg();
  if (type === 'image/png') return writePng(source);
  let drawn: { path: string; width: number; height: number };
  try { drawn = await fs.transformImage(source, `${directory}/image.png`, { type: 'image/png' }); }
  catch (error) {
    if (letGo(error)) throw error;
    const code = (error as { code?: string }).code;
    throw new Error(code === 'too-large' ? TOO_LARGE : code === 'undecodable' ? UNDECODABLE : NOT_PNG);
  }
  if (!(drawn.width > 0 && drawn.height > 0) || drawn.width * drawn.height > MAX_PIXELS) throw new Error(TOO_LARGE);
  await writePng(drawn.path || `${directory}/image.png`);
}
