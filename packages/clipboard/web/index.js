// @exact/clipboard on the web (LLP 1133.006): `navigator.clipboard.write`
// with a `ClipboardItem` (https://www.w3.org/TR/clipboard-apis/).
//
// - The image is sanitized as Apple's is (D1): decoded, frame 0 of an
//   animation chosen with `ImageDecoder` where the browser has it, and drawn
//   to a canvas whose PNG is what the browser is given, so both hosts hold
//   the same pixels to the same bounds before the browser sanitizes again.
// - When a write may happen is the browser's (D3): Chrome writes while the
//   document has focus; Safari and Firefox within a person's activation. A
//   refusal is the browser's `NotAllowedError`.
// - Under the agent it is its own substitute (D8 item 4, §7): the item is
//   kept in memory, never given to the browser.
// - A later write supersedes one still decoding, which fails `AbortError` (D2).

const MOST_FILE = 64 * 1024 * 1024, MOST_BYTES = 2 * 1024 * 1024, MOST_PIXELS = 64_000_000, MOST_TEXT = 1024 * 1024;
const TYPES = ['text/plain', 'image/png'];
const SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/** `{base64}` as bytes. */
const unwired = b64 => Uint8Array.from(atob(b64), c => c.charCodeAt(0));

/** A PNG's size from its `IHDR`, or null for what is not a PNG: a side past the specification's 2³¹ − 1, as on Apple. */
export function pngSize(bytes) {
  if (bytes.length < 24 || SIGNATURE.some((b, i) => bytes[i] !== b)) return null;
  if (String.fromCharCode(...bytes.subarray(12, 16)) !== 'IHDR') return null;
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), width = v.getUint32(16), height = v.getUint32(20);
  return width <= 0x7fffffff && height <= 0x7fffffff ? { width, height } : null;
}

/** Frame 0 of `bytes` drawn: the first frame of its animation, as Chromium's clipboard writer decodes it. */
async function firstFrame(bytes) {
  if (typeof globalThis.ImageDecoder === 'function') {
    const decoder = new ImageDecoder({ data: bytes, type: 'image/png' });
    try { return (await decoder.decode({ frameIndex: 0 })).image; } finally { decoder.close(); }
  }
  return createImageBitmap(new Blob([bytes], { type: 'image/png' }));
}

const hex = buffer => [...new Uint8Array(buffer)].map(b => b.toString(16).padStart(2, '0')).join('');

export function implementation({ agent, log, files }) {
  const say = line => log?.(`clipboard: ${line}`);
  let latest = 0, inFlight = null, writes = 0, last = null, shownNumber = 0;
  const refused = (error, message) => { say(`write refused: ${error} (${message})`); return { error, message }; };

  /** A new PNG of `bytes`' pixels, or a refusal. */
  async function sanitized(bytes) {
    const size = pngSize(bytes);
    if (!size) return refused('DataError', 'not a PNG');
    if (size.width * size.height > MOST_PIXELS) return refused('DataError', `too large: ${size.width}×${size.height} is over 64,000,000 pixels`);
    let frame;
    try { frame = await firstFrame(bytes); } catch { return refused('DataError', 'not a PNG: it does not decode'); }
    try {
      const width = frame.displayWidth ?? frame.width, height = frame.displayHeight ?? frame.height;
      const canvas = new OffscreenCanvas(width, height);
      canvas.getContext('2d').drawImage(frame, 0, 0);
      const blob = await canvas.convertToBlob({ type: 'image/png' });
      const png = new Uint8Array(await blob.arrayBuffer());
      return { blob, png, width, height };
    } finally { frame.close?.(); }
  }

  async function write(items, told = {}) {
    // Chrome resolves an empty write and leaves the clipboard as it was.
    if (!Array.isArray(items)) return refused('TypeError', 'write takes a list of ClipboardItem');
    if (!items.length) return null;
    if (items.length > 1) return refused('NotAllowedError', 'Support for multiple ClipboardItems is not implemented.');
    const record = items[0];
    if (!record || typeof record !== 'object' || !Object.keys(record).length) return refused('TypeError', 'a ClipboardItem has at least one type');
    const other = Object.keys(record).sort().find(t => !TYPES.includes(t));
    if (other) return refused('NotAllowedError', `Type ${other} not supported on write.`);
    const text = record['text/plain'];
    if (text != null && typeof text !== 'string') return refused('TypeError', 'text/plain is a string');
    if (text != null && new TextEncoder().encode(text).length > MOST_TEXT) return refused('DataError', 'text/plain is over 1 MiB');
    let source = null;
    const image = record['image/png'];
    if (typeof image === 'string') {
      try { source = new Uint8Array(await files.read(image)); } catch (e) { return refused(/ENOENT|not found|no such/i.test(e?.message ?? '') ? 'NotFoundError' : 'NotReadableError', `${image}: ${e?.message ?? e}`); }
      if (source.length > MOST_FILE) return refused('DataError', `too large: ${image} is over 64 MiB`);
    } else if (image && typeof image.base64 === 'string') {
      source = unwired(image.base64);
      if (source.length > MOST_BYTES) return refused('DataError', 'image/png is over 2 MiB as bytes: pass an app:/ path');
    } else if (image != null) return refused('TypeError', 'image/png is an app:/ path or bytes');

    // This write supersedes any still decoding (D2).
    const number = ++latest;
    inFlight?.supersede();
    let superseded = null;
    const mine = { request: told.request, supersede: () => { superseded ??= refused('AbortError', 'a later write superseded it'); } };
    inFlight = mine;
    const done = () => { if (inFlight === mine) inFlight = null; };
    const png = source ? await sanitized(source) : null;
    if (superseded || number !== latest) { done(); return superseded ?? refused('AbortError', 'a later write superseded it'); }
    done();
    if (png?.error) return png;
    if (!agent) {
      const data = {};
      if (png) data['image/png'] = png.blob;
      if (text != null) data['text/plain'] = new Blob([text], { type: 'text/plain' });
      try { await navigator.clipboard.write([new ClipboardItem(data)]); } catch (e) { return refused(e?.name ?? 'NotAllowedError', e?.message ?? String(e)); }
    }
    writes++;
    const shown = { types: [...(text != null ? ['text/plain'] : []), ...(png ? ['image/png'] : [])], pasteboard: agent ? 'memory' : 'navigator.clipboard' };
    if (text != null) shown.text = text.slice(0, 200);
    if (png) {
      shown.image = { width: png.width, height: png.height, bytes: png.png.length, sha256: hex(await crypto.subtle.digest('SHA-256', png.png)) };
      say(`wrote image/png ${png.width}×${png.height} (${png.png.length.toLocaleString('en-US')} bytes)${text != null ? ' and text/plain' : ''}`);
    } else say(`wrote text/plain (${new TextEncoder().encode(text).length} bytes)`);
    // What `state()` shows is the newest write's, though an older one's hash, awaited after its write, lands later.
    if (number > shownNumber) { shownNumber = number; last = shown; }
    return null;
  }
  const disclosed = () => ({ writes, ...(last ? { lastWrite: last } : {}) });

  return {
    write: ({ data }, told) => write(data, told),
    writeText: ({ data }, told) => typeof data === 'string' ? write([{ 'text/plain': data }], told) : refused('TypeError', 'writeText takes a string'),
    cancel(request) { if (inFlight && inFlight.request === request) { latest++; inFlight.supersede(); } },
    destroy() { inFlight?.supersede(); },
    /** `state.extensions.clipboard.implementation` (§7 item 3). */
    state: disclosed,
  };
}
