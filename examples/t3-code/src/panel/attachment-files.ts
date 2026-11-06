// An attachment's bytes for its preview and Save file (MIT reference, see LICENSE-T3:
// components/files/AttachmentFilePreview.tsx readFilePreviewResponse and decodeFilePreviewText,
// FILE_TEXT_PREVIEW_MAX_BYTES; MediaActions.tsx and mediaContent.ts readMediaBlob). A sent attachment is read from its
// signed asset URL (`fetch`, app.ts grants); a draft's from its staged copy under the app's data
// (composer-files/, composer-attach-stage.ts stages it under composer-staged-copies.ts stagedName).
import { appFiles, requestSave, saveBytes, saveDownload, saveName } from '../media/save-file';

export const PREVIEW_BYTES = 1024 * 1024;
export type TextPreview = { ok: true; text: string; truncated: boolean } | { ok: false; message: string };

/** A draft file's staged copy as an app file, by its staged name (composer-staged-copies.ts stagedName). */
export const stagedPath = (name: string) => `app:/data/composer-files/${name}`;

/** Only a signed asset route of an http(s) server: never a file URL or another path. */
export function assetUrl(raw: string): string | null {
  try {
    const url = new URL(raw);
    return (url.protocol === 'http:' || url.protocol === 'https:') && url.hostname && !url.username && url.pathname.startsWith('/api/assets/') ? url.href : null;
  } catch { return null; }
}

/** decodeFilePreviewText: binary data and non-UTF-8 text are refused, not shown as replacement characters. */
export function decodePreview(bytes: Uint8Array, truncated: boolean): TextPreview {
  const bounded = bytes.subarray(0, PREVIEW_BYTES);
  if (bounded.includes(0)) return { ok: false, message: 'This file contains binary data and cannot be shown as text.' };
  const decoder = new TextDecoder('utf-8', { fatal: true });
  // A cut can split one UTF-8 sequence; drop at most its three leading bytes.
  for (let drop = 0; drop <= (truncated ? 3 : 0); drop++) {
    try { return { ok: true, text: decoder.decode(bounded.subarray(0, bounded.length - drop)), truncated }; } catch { /* try a shorter cut */ }
  }
  return { ok: false, message: 'This file is not UTF-8 text. Open it in another app to view its contents.' };
}

/** `Content-Range: bytes 0-1048576/4194304` → 4194304. */
function totalBytes(range: string | null): number | null {
  const total = range?.split('/').pop();
  return total && /^\d+$/.test(total) ? Number(total) : null;
}

/** A sent attachment's first megabyte as text (a `Range` request). */
export async function attachmentText(url: string): Promise<TextPreview> {
  const asset = assetUrl(url);
  if (!asset) return { ok: false, message: 'The attachment is unavailable.' };
  let response: Response;
  try { response = await fetch(asset, { headers: { Range: `bytes=0-${PREVIEW_BYTES}` } }); } catch { return { ok: false, message: 'Could not load this file.' }; }
  if (!response.ok) return { ok: false, message: 'Could not load this file.' };
  const bytes = new Uint8Array(await response.arrayBuffer());
  const total = totalBytes(response.headers.get('Content-Range'));
  return decodePreview(bytes, bytes.length > PREVIEW_BYTES || (response.status === 206 && total !== null && total > PREVIEW_BYTES));
}

/** A draft file's first megabyte as text, from its staged copy. */
export async function stagedText(name: string): Promise<TextPreview> {
  let bytes: Uint8Array;
  try { bytes = new Uint8Array(await appFiles().fs.readFile(stagedPath(name))); } catch { return { ok: false, message: 'Could not load this file.' }; }
  return decodePreview(bytes, bytes.length > PREVIEW_BYTES);
}

/** Save file of a sent attachment: its bytes from the asset URL, then the window's Save panel. */
export async function saveAttachment(client: object, url: string, name: string): Promise<void> {
  const asset = assetUrl(url);
  if (!asset) throw new Error('The attachment is unavailable.');
  try { await saveDownload(client, asset, saveName(name, 'attachment')); } catch { throw new Error('The file could not be loaded. Try again.'); }
}

/** Save file of a draft's staged copy, as it is. */
export function saveStaged(client: object, staged: string, name: string): void {
  requestSave(client, stagedPath(staged), saveName(name, 'attachment'));
}

/** Save file of a folded paste: its text as the file. */
export async function saveText(client: object, text: string, name: string): Promise<void> {
  await saveBytes(client, new TextEncoder().encode(text), saveName(name, 'attachment'));
}
