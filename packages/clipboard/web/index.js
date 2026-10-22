// @exact/clipboard on the web (LLP 1133.006): the browser's
// `navigator.clipboard.write` with a `ClipboardItem`. The page runtime calls
// it for data code (host/web-js/direct.js), outside the agent and, as its
// own substitute, under it (D8 item 4).
//
// - **Sanitized** (D1), before the browser sees it: the PNG is decoded and
//   drawn, and a new PNG of its pixels written; an animated PNG writes its
//   first frame, cut out of its chunks as the Apple implementation does, so
//   a browser whose decoder shows the default image (WebKit's) writes what
//   Chromium writes.
// - **Bounds and order** (D2) as on Apple: at most 64,000,000 pixels by the
//   header, text at most 1 MiB; an older write, by sequence number, or one a
//   later write overtook while it decoded, is superseded (`AbortError`).
// - **When** (D3): the browser decides. Chrome writes while the document
//   has focus; Safari and Firefox within a transient activation. Its
//   refusal (`NotAllowedError`) is the write's.
// - **Under the agent** nothing reaches the person's clipboard: the item is
//   kept in memory (`globalThis.exact.agentClipboard`, also where
//   `copyText` keeps its text, rt.js). The answer's `$state` is what
//   `state.extensions.clipboard` shows of the last write.

const MAX_PIXELS = 64_000_000, MAX_TEXT = 1 << 20;
const SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const UNREADABLE = 'Failed to read or decode ClipboardItemData for type image/png.';

/** The PNG's chunks, or null when it is not one. */
function chunks(bytes) {
  if (bytes.length < 8 || SIGNATURE.some((b, i) => bytes[i] !== b)) return null;
  const out = [], view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let at = 8; at + 12 <= bytes.length;) {
    const length = view.getUint32(at), type = String.fromCharCode(...bytes.subarray(at + 4, at + 8));
    if (at + 12 + length > bytes.length) return null;
    out.push({ type, data: bytes.subarray(at + 8, at + 8 + length) });
    at += 12 + length;
    if (type === 'IEND') break;
  }
  return out;
}

const TABLE = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
function chunk(type, data) {
  const out = new Uint8Array(12 + data.length), view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set([...type].map(c => c.charCodeAt(0)), 4);
  out.set(data, 8);
  let crc = 0xffffffff;
  for (const b of out.subarray(4, 8 + data.length)) crc = TABLE[(crc ^ b) & 0xff] ^ (crc >>> 8);
  view.setUint32(8 + data.length, (crc ^ 0xffffffff) >>> 0);
  return out;
}

/** An animated PNG's first frame as a still PNG; a still one as it is; null when malformed. */
export function firstFrame(bytes) {
  const all = chunks(bytes);
  if (!all) return null;
  const image = all.findIndex(c => c.type === 'IDAT');
  if (image < 0) return null;
  const before = all.slice(0, image);
  if (!before.some(c => c.type === 'acTL')) return bytes;
  let data;
  if (before.some(c => c.type === 'fcTL')) data = takeWhile(all.slice(image), 'IDAT').map(c => c.data);
  else {
    const control = all.findIndex(c => c.type === 'fcTL');
    if (control < 0) return null;
    data = takeWhile(all.slice(control + 1), 'fdAT').filter(c => c.data.length > 4).map(c => c.data.subarray(4));
  }
  if (!data.length) return null;
  const parts = [new Uint8Array(SIGNATURE), ...before.filter(c => c.type !== 'acTL' && c.type !== 'fcTL').map(c => chunk(c.type, c.data)), ...data.map(d => chunk('IDAT', d)), chunk('IEND', new Uint8Array())];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  parts.reduce((at, p) => (out.set(p, at), at + p.length), 0);
  return out;
}
const takeWhile = (list, type) => { const out = []; for (const c of list) { if (c.type !== type) break; out.push(c); } return out; };

/** The header's width and height. */
export function size(bytes) {
  const header = chunks(bytes)?.[0];
  if (header?.type !== 'IHDR' || header.data.length < 8) return null;
  const view = new DataView(header.data.buffer, header.data.byteOffset, 8);
  return { width: view.getUint32(0), height: view.getUint32(4) };
}

const refusal = (name, message) => Object.assign(new Error(message), { refusal: name });

/** The item's PNG decoded and written again: its bytes, width and height. */
async function sanitize(value) {
  let bytes;
  if (value?.$bytes !== undefined) bytes = Uint8Array.from(atob(value.$bytes), c => c.charCodeAt(0));
  else if (value?.$file) bytes = new Uint8Array(await value.$file.arrayBuffer());
  else throw refusal('DataError', UNREADABLE);
  const dims = size(bytes);
  if (!dims) throw refusal('DataError', UNREADABLE);
  if (!dims.width || !dims.height || dims.width * dims.height > MAX_PIXELS) throw refusal('DataError', `image/png: too large, ${dims.width} × ${dims.height} is more than 64,000,000 pixels`);
  const still = firstFrame(bytes);
  if (!still) throw refusal('DataError', UNREADABLE);
  let bitmap;
  try { bitmap = await createImageBitmap(new Blob([still], { type: 'image/png' })); }
  catch { throw refusal('DataError', UNREADABLE); }
  const canvas = typeof OffscreenCanvas === 'function' ? new OffscreenCanvas(bitmap.width, bitmap.height) : Object.assign(document.createElement('canvas'), { width: bitmap.width, height: bitmap.height });
  canvas.getContext('2d').drawImage(bitmap, 0, 0);
  bitmap.close?.();
  const blob = canvas.convertToBlob ? await canvas.convertToBlob({ type: 'image/png' }) : await new Promise(r => canvas.toBlob(r, 'image/png'));
  return { blob, width: canvas.width, height: canvas.height };
}

async function sha256(blob) {
  if (!globalThis.crypto?.subtle) return null;
  const digest = await crypto.subtle.digest('SHA-256', await blob.arrayBuffer());
  return [...new Uint8Array(digest)].map(b => b.toString(16).padStart(2, '0')).join('');
}

export function implementation({ agent, say = () => {} }) {
  let newest = 0;
  const refuse = (name, message) => { say(`clipboard: write refused: ${name} (${message})`); return { error: name, message: `clipboard.write: ${message}` }; };
  const describe = async (text, image) => {
    const last = { types: [...(image ? ['image/png'] : []), ...(text != null ? ['text/plain'] : [])] };
    if (text != null) last.text = text.slice(0, 200);
    if (image) last.image = { width: image.width, height: image.height, bytes: image.blob.size, sha256: await sha256(image.blob) };
    return last;
  };
  const write = async (seq, items) => {
    if (seq === 1) newest = 0; // a binding's sequence starts again when its runtime reloads
    if (seq <= newest && seq !== 0) return refuse('AbortError', 'a later write superseded it');
    newest = seq;
    // Chrome's refusals, in its words (measured: HeadlessChrome 156).
    if (!items.length) return { $state: { substituting: agent, last: { types: [] } } };
    if (items.length > 1) return refuse('NotAllowedError', 'Support for multiple ClipboardItems is not implemented.');
    const item = items[0], other = Object.keys(item).sort().find(t => t !== 'text/plain' && t !== 'image/png');
    if (other) return refuse('NotAllowedError', `Type ${other} not supported on write.`);
    const text = item['text/plain'] ?? null;
    if (text != null && new TextEncoder().encode(text).length > MAX_TEXT) return refuse('DataError', 'text/plain: more than 1 MiB');
    let image = null;
    if (item['image/png'] !== undefined) {
      try { image = await sanitize(item['image/png']); }
      catch (e) { return refuse(e.refusal ?? 'DataError', e.message); }
    }
    if (seq !== newest && seq !== 0) return refuse('AbortError', 'a later write superseded it');
    const last = await describe(text, image);
    if (agent) {
      (globalThis.exact ??= {}).agentClipboard = { ...last };
    } else {
      const data = {};
      if (image) data['image/png'] = image.blob;
      if (text != null) data['text/plain'] = new Blob([text], { type: 'text/plain' });
      try { await navigator.clipboard.write([new ClipboardItem(data)]); }
      catch (e) { return refuse(e?.name ?? 'NotAllowedError', e?.message ?? String(e)); }
    }
    say(image ? `clipboard: wrote image/png ${image.width}×${image.height} (${image.blob.size.toLocaleString('en-US')} bytes)${agent ? ' (kept for the agent)' : ''}`
      : `clipboard: wrote text/plain (${text.length} characters)${agent ? ' (kept for the agent)' : ''}`);
    return { $state: { substituting: agent, last } };
  };
  return {
    write: ({ items, $sequence }) => write($sequence?.seq ?? 0, items ?? []),
    writeText: ({ text, $sequence }) => write($sequence?.seq ?? 0, [{ 'text/plain': text }]),
  };
}
