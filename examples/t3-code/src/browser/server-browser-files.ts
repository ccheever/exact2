// Server stream downloads and file choosers use the media endpoint's own access,
// never a page URL or the environment bearer in app state.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { str } from '../client/domain';
import { appFiles, saveDownload, saveName } from '../media/save-file';
export type StreamPicked = { path: string; name: string; type: string; size: number };
export async function streamMediaUrl(client: T3Client, native: Native, threadId: string, tabId: string, operation: 'download' | 'upload', id: string): Promise<string> {
  const access = await client.call(native, { op: 'previewStreamAccess' });
  const url = new URL(`${str(access.httpBase)}/${operation}`);
  for (const [key, value] of Object.entries({ threadId, tabId, [operation === 'upload' ? 'chooser' : 'id']: id, wsTicket: str(access.ticket) })) url.searchParams.set(key, value);
  return url.href;
}
export async function downloadStreamFile(client: T3Client, native: Native, threadId: string, tabId: string, download: { id: string; fileName: string; sizeBytes: number }, current: () => boolean): Promise<void> {
  const url = await streamMediaUrl(client, native, threadId, tabId, 'download', download.id);
  try { await saveDownload(client, url, saveName(download.fileName, 'download'), { init: { redirect: 'error' }, maxBytes: Math.max(1, download.sizeBytes), current }); }
  catch { throw new Error('The browser download was refused.'); }
}
/** Multipart is bytes here: the data module has fetch and typed arrays, without
 * relying on a DOM FormData implementation. A picked file is app-owned storage. */
export async function uploadStreamFiles(client: T3Client, native: Native, threadId: string, tabId: string, chooser: string, picked: StreamPicked[], current: () => boolean): Promise<void> {
  if (picked.length > 20 || picked.reduce((total, file) => total + file.size, 0) > 100 * 1024 * 1024) throw new Error('Choose up to 20 files totaling 100 MB.');
  const contents: Uint8Array[] = [];
  for (const file of picked) {
    if (!file.path.startsWith('app:/tmp/picked/')) throw new Error('Choose a file with the browser file picker.');
    contents.push(new Uint8Array(await appFiles().fs.readFile(file.path)));
  }
  let boundary = '----ExactBrowserStreamUpload';
  while (contents.some(bytes => containsBytes(bytes, utf8(boundary)))) boundary += 'X';
  const parts: Uint8Array[] = [];
  for (const [index, file] of picked.entries()) {
    if (!file.path.startsWith('app:/tmp/picked/')) throw new Error('Choose a file with the browser file picker.');
    const name = saveName(file.name, 'file').replace(/["\r\n]/g, '_'), type = /^[\w.+-]+\/[\w.+-]+$/.test(file.type) ? file.type : 'application/octet-stream';
    parts.push(utf8(`--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="${name}"\r\nContent-Type: ${type}\r\n\r\n`), contents[index]!, utf8('\r\n'));
  }
  parts.push(utf8(`--${boundary}--\r\n`));
  const body = new Uint8Array(parts.reduce((size, bytes) => size + bytes.byteLength, 0)); let offset = 0;
  for (const part of parts) { body.set(part, offset); offset += part.byteLength; }
  const url = await streamMediaUrl(client, native, threadId, tabId, 'upload', chooser);
  if (!current()) return;
  const reply = await fetch(url, { method: 'POST', headers: { 'Content-Type': `multipart/form-data; boundary=${boundary}` }, body, redirect: 'error', exactIndependentHttp: { maxResponseBytes: 1024 * 1024 } } as RequestInit);
  if (!reply.ok) throw new Error('The browser file upload was refused.');
}

function containsBytes(bytes: Uint8Array, needle: Uint8Array): boolean {
  for (let at = bytes.indexOf(needle[0]!); at >= 0; at = bytes.indexOf(needle[0]!, at + 1)) {
    if (needle.every((value, index) => bytes[at + index] === value)) return true;
  }
  return false;
}
/** Native JS supplies typed arrays without a DOM TextEncoder. */
function utf8(text: string): Uint8Array {
  const bytes: number[] = [];
  for (const char of text) {
    let point = char.codePointAt(0)!; if (point >= 0xd800 && point <= 0xdfff) point = 0xfffd;
    if (point < 0x80) bytes.push(point);
    else if (point < 0x800) bytes.push(0xc0 | point >> 6, 0x80 | point & 63);
    else if (point < 0x10000) bytes.push(0xe0 | point >> 12, 0x80 | point >> 6 & 63, 0x80 | point & 63);
    else bytes.push(0xf0 | point >> 18, 0x80 | point >> 12 & 63, 0x80 | point >> 6 & 63, 0x80 | point & 63);
  }
  return new Uint8Array(bytes);
}
