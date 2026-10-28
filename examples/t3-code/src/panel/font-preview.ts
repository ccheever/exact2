// A font's specimen (MIT reference, see LICENSE-T3: components/files/FontViewer.tsx and FontFilePreview.tsx; T3 Code
// 7c3b3b3605ef). The reference reads at most 10 MB of the font, checks the bytes with `FontFace` and frames its
// specimen document (`srcDoc`, `sandbox=""`) with the font as a data URL. Here TypeScript reads the bytes (a sent
// file's signed URL, or a draft's staged copy) and writes the same document to app:/cache/media-preview, which the
// preview frames with scripts allowed (font-preview.contract): FontFace is the frame's, so the document's own script
// loads the font, shows the specimen once it has loaded, and tells the app whether it did.
import { appFiles } from '../media/save-file';
import { MEDIA_CACHE, stagedCopyName } from '../media/media-frame';
import { STAGED } from '../composer/composer-staged-copies';
import { assetUrl } from './attachment-files';

const MAX_FONT_BYTES = 10 * 1024 * 1024;
/** FontViewer's reasons, each shown as "<reason> Save it to open in another app." under "No preview for this file". */
export const FONT_TOO_LARGE = 'Font previews are limited to 10 MB.';
export const FONT_UNLOADED = 'The font could not be loaded.';
export const FONT_TIMEOUT = 'The font took too long to load.';
/** Chromium's message when `new FontFace(name, bytes).load()` rejects bytes its sanitizer refuses; the reference shows it. */
export const FONT_REFUSED = 'Invalid font data in ArrayBuffer.';
/** What the specimen's script posts. */
export const FONT_LOADED_MESSAGE = 'font-loaded';
export const FONT_FAILED_MESSAGE = 'font-failed';

export type FontPage = { page: string; error: string };

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
/** RFC 4648 base64, in chunks a multiple of three bytes long, so only the last one is padded. */
export function base64(bytes: Uint8Array): string {
  const parts: string[] = [];
  for (let start = 0; start < bytes.length; start += 12_288) {
    const end = Math.min(bytes.length, start + 12_288);
    let out = '';
    for (let at = start; at < end; at += 3) {
      const word = (bytes[at]! << 16) | ((bytes[at + 1] ?? 0) << 8) | (bytes[at + 2] ?? 0);
      out += BASE64[(word >> 18) & 63]! + BASE64[(word >> 12) & 63]! + (at + 1 < end ? BASE64[(word >> 6) & 63]! : '=') + (at + 2 < end ? BASE64[word & 63]! : '=');
    }
    parts.push(out);
  }
  return parts.join('');
}

/**
 * FontViewer's srcDoc for the font at `dataUrl`: "Font preview", the alphabets and the 12–64 px samples, in the
 * surface's colours (`prefers-color-scheme` follows the frame's `color-scheme`). Its script is the reference's FontFace
 * check: the body stays hidden until the face has loaded.
 */
export function fontSpecimen(dataUrl: string): string {
  const check = `document.fonts.load("16px preview").then(function(){return Array.from(document.fonts).some(function(face){return face.family.replace(/["']/g,"")==="preview"&&face.status==="loaded"})},function(){return false}).then(function(ok){if(ok)document.body.style.visibility="visible";parent.postMessage(ok?"${FONT_LOADED_MESSAGE}":"${FONT_FAILED_MESSAGE}","*")})`;
  const samples = [12, 16, 24, 40, 64].map(size => `<p style="font-size:${size}px"><small>${size} px</small>The quick brown fox jumps over the lazy dog.</p>`).join('');
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; font-src data:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><style>@font-face{font-family:preview;src:url('${dataUrl}')}html{color-scheme:light dark;background:#fcfcfc;color:#18181b}@media (prefers-color-scheme:dark){html{background:#000000;color:#f5f5f5}}body{margin:0;padding:32px;font-family:preview;overflow-wrap:anywhere;visibility:hidden}h1{font-size:40px;font-weight:normal}p{line-height:1.4}small{font:12px monospace;display:block;opacity:.6;margin-bottom:8px}</style></head><body><h1>Font preview</h1><p style="font-size:24px">ABCDEFGHIJKLMNOPQRSTUVWXYZ<br>abcdefghijklmnopqrstuvwxyz<br>0123456789 !?&amp;@#%(){}[]</p>${samples}<script>${check}</script></body></html>`;
}

class FontReadError extends Error {}

/** readFont: the font's bytes, at most 10 MB: a signed URL's reply, or a draft's staged copy. */
async function readFont(url: string, sizeBytes: number): Promise<Uint8Array> {
  if (sizeBytes > MAX_FONT_BYTES) throw new FontReadError(FONT_TOO_LARGE);
  const staged = stagedCopyName(url);
  if (staged) {
    let bytes: Uint8Array;
    try { bytes = new Uint8Array(await appFiles().fs.readFile(`${STAGED}/${staged}`)); } catch { throw new FontReadError(FONT_UNLOADED); }
    if (bytes.length > MAX_FONT_BYTES) throw new FontReadError(FONT_TOO_LARGE);
    return bytes;
  }
  const asset = assetUrl(url);
  if (!asset) throw new FontReadError(FONT_UNLOADED);
  let response: Response;
  try {
    response = await fetch(asset, { exactTimeout: 30_000, exactIndependentHttp: { maxResponseBytes: MAX_FONT_BYTES } } as RequestInit);
  } catch (error) {
    const kind = (error as { kind?: string } | null)?.kind;
    throw new FontReadError(kind === 'Timeout' ? FONT_TIMEOUT : kind === 'Refused' ? FONT_TOO_LARGE : FONT_UNLOADED);
  }
  if (!response.ok) throw new FontReadError(FONT_UNLOADED);
  if (Number(response.headers.get('content-length')) > MAX_FONT_BYTES) throw new FontReadError(FONT_TOO_LARGE);
  return new Uint8Array(await response.arrayBuffer());
}

type SpecimenFiles = {
  mkdir(path: string, options?: { recursive?: boolean }): Promise<unknown>; writeFile(path: string, bytes: Uint8Array): Promise<unknown>;
  readdir?(path: string): Promise<unknown>; rm?(path: string): Promise<unknown>;
};
const files = (): SpecimenFiles => appFiles().fs as unknown as SpecimenFiles;

/** The specimens written, by preview, the most recently shown last. */
const kept = new Map<string, Promise<FontPage>>();
let serial = 0;
/** A specimen page's name. The serial starts again with the app, so the pages an earlier run left go first. */
const FONT_PAGE = /^font-\d+\.html$/;
let swept: Promise<void> | null = null;
/** Deletes the specimen pages an earlier run left, once, before the first page of this run is written. */
function sweepPages(): Promise<void> {
  swept ??= (async () => {
    const names = await files().readdir?.(MEDIA_CACHE);
    if (!Array.isArray(names)) return;
    await Promise.all(names.filter((name): name is string => typeof name === 'string' && FONT_PAGE.test(name)).map(name => files().rm?.(`${MEDIA_CACHE}/${name}`).catch(() => undefined)));
  })().catch(() => undefined);
  return swept;
}
/** Specimens kept at once: each holds its font, so the least recently shown one's file goes when another arrives. */
const KEPT = 4;

/**
 * The specimen page of preview `key` for the font at `url` (a signed asset URL or a draft's `t3-staged:<name>`), or the
 * reason there is none. The page is written once and kept while it is among the four most recently shown; a preview
 * whose page went is written again from the font, as FontViewer reads it again when it mounts. A failure is not kept,
 * so Try again reads the font afresh.
 */
export function fontPage(key: string, url: string, sizeBytes: number): Promise<FontPage> {
  const known = kept.get(key);
  if (known) { kept.delete(key); kept.set(key, known); return known; }
  const made = (async (): Promise<FontPage> => {
    try {
      const bytes = await readFont(url, sizeBytes);
      await files().mkdir(MEDIA_CACHE, { recursive: true }).catch(() => undefined);
      await sweepPages();
      const page = `${MEDIA_CACHE}/font-${++serial}.html`;
      // The native host writes bytes only: a string is refused.
      await files().writeFile(page, new TextEncoder().encode(fontSpecimen(`data:application/octet-stream;base64,${base64(bytes)}`)));
      return { page, error: '' };
    } catch (error) {
      return { page: '', error: error instanceof FontReadError ? error.message : FONT_UNLOADED };
    }
  })();
  kept.set(key, made);
  void made.then(result => { if (result.error && kept.get(key) === made) kept.delete(key); });
  while (kept.size > KEPT) {
    const [oldest, gone] = kept.entries().next().value!;
    kept.delete(oldest);
    removePage(gone);
  }
  return made;
}

const removePage = (made: Promise<FontPage>) => void made.then(result => { if (result.page) return files().rm?.(result.page); }).catch(() => undefined);

/** Whether preview `key`'s specimen is kept: one that is not reads the font again when it is next shown. */
export function fontPageKept(key: string): boolean { return kept.has(key); }
/** Forgets preview `key`'s specimen and deletes its page (a retry, a draft's preview closed). */
export function forgetFontPage(key: string): void {
  const gone = kept.get(key);
  kept.delete(key);
  if (gone) removePage(gone);
}
/** Forgets every specimen and deletes their pages, and the sweep made (tests). */
export function forgetFontPages(): void {
  for (const gone of kept.values()) removePage(gone);
  kept.clear(); swept = null;
}
