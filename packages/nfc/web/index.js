// @exact/nfc on the web (LLP 1133.035 stage 1): Web NFC's `NDEFReader`,
// which only Chrome on Android ships; every other browser reads `available`
// false. The page runtime (host/web-js/modules.js) calls it outside the
// agent, and under the agent when the fixture binds it (`--bind nfc`).
//
// - One reader serves the session's scans (D6): it scans while any scan
//   claim lives and is aborted when the last ends. Each tag it reads is one
//   `reading` (or `readingerror`) on the root, whatever the number of scans,
//   and ends each `once` scan with `read`.
// - A scan answers when it ends (D4): `abortScan`, a call let go
//   (`cancel(request)`), or the reader failing after it started
//   (`interrupted`). A failure to start rejects at once.
// - One write at a time (D6): a newer `write` or `makeReadOnly` aborts the
//   waiting one, which fails `AbortError`, as Web NFC's replace does. While a
//   write waits, the reader's `reading` for the tag being written is the
//   write's, not the app's (D4: "A written tag is not also read").
// - The page leaving the foreground fails a waiting write `AbortError`; the
//   browser suspends a scan itself and resumes it on return (D6's table).
// - Records reach Contract as `NDEFRecordInfo`, with `text` decoded for
//   `url`, `absolute-url`, `text` and a smart poster's URL (D2); the last 16
//   readings' records, bytes and all, answer `ndefRecords`.

const KEPT = 16;

/** A DOMException's name as D3's error, with the platform's message. */
function failure(e, fallback = 'NetworkError') {
  const name = typeof e?.name === 'string' && e.name !== 'Error' ? e.name : fallback;
  return { error: name, message: typeof e?.message === 'string' && e.message ? e.message : null };
}

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

function base64(bytes) {
  let s = '';
  for (let i = 0; i < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | ((bytes[i + 1] ?? 0) << 8) | (bytes[i + 2] ?? 0);
    s += B64[(n >> 18) & 63] + B64[(n >> 12) & 63] + (i + 1 < bytes.length ? B64[(n >> 6) & 63] : '=') + (i + 2 < bytes.length ? B64[n & 63] : '=');
  }
  return s;
}

function unbase64(text) {
  const s = String(text).replace(/=+$/, ''), out = new Uint8Array(Math.floor((s.length * 3) / 4));
  let bits = 0, n = 0, at = 0;
  for (const c of s) { n = (n << 6) | B64.indexOf(c); bits += 6; if (bits >= 8) { bits -= 8; out[at++] = (n >> bits) & 255; } }
  return out;
}

/** A record's payload as bytes. */
function bytesOf(record) {
  const d = record?.data;
  if (!d) return new Uint8Array(0);
  if (d instanceof Uint8Array) return d;
  if (ArrayBuffer.isView(d)) return new Uint8Array(d.buffer, d.byteOffset, d.byteLength);
  if (d instanceof ArrayBuffer) return new Uint8Array(d);
  if (typeof d === 'string') return new TextEncoder().encode(d);
  return new Uint8Array(0);
}

function decode(bytes, encoding) {
  try { return new TextDecoder(encoding || 'utf-8').decode(bytes); } catch { return new TextDecoder().decode(bytes); }
}

/** A record's own records (a smart poster's, a well-formed external record's), as the web's `toRecords()`. */
function inner(record) {
  try { return typeof record.toRecords === 'function' ? record.toRecords() ?? null : null; } catch { return null; }
}

/** `text` (D2): the URL of a `url` or `absolute-url` record and of a smart poster, a `text` record's text. */
function textOf(record) {
  switch (record.recordType) {
    case 'url': case 'absolute-url': return decode(bytesOf(record), 'utf-8');
    case 'text': return decode(bytesOf(record), record.encoding);
    case 'smart-poster': {
      const url = (inner(record) ?? []).find(r => r.recordType === 'url');
      return url ? decode(bytesOf(url), 'utf-8') : null;
    }
    default: return null;
  }
}

/** A record as Contract reads it (`NDEFRecordInfo`). */
function info(record) {
  return {
    recordType: record.recordType, mediaType: record.mediaType ?? null, id: record.id ?? null,
    encoding: record.encoding ?? null, lang: record.lang ?? null, text: textOf(record), size: bytesOf(record).length,
  };
}

/** A record as data code reads it (`NDEFRecord`), its bytes as they cross the seam. A record's own records go one
 * level deep (`NDEFInnerRecord`): plan values are finite trees, and a deeper record is in its parent's bytes. */
function full(record) {
  const own = inner(record), bytes = r => ({ ...info(r), data: { base64: base64(bytesOf(r)) } });
  return { ...bytes(record), records: own ? own.map(bytes) : null };
}

/** An `NDEFRecordInit` from the seam, its bytes back to a buffer. */
function recordInit(r) {
  const out = { recordType: r.recordType };
  for (const k of ['mediaType', 'id', 'encoding', 'lang']) if (r[k] != null) out[k] = r[k];
  if (r.data != null) out.data = typeof r.data === 'object' && typeof r.data.base64 === 'string' ? unbase64(r.data.base64) : r.data;
  return out;
}

export function implementation({ changed, emit, foreground }) {
  const Reader = globalThis.NDEFReader;
  const has = typeof Reader === 'function';
  let permission = null;
  if (has && typeof navigator === 'object' && navigator.permissions?.query) {
    navigator.permissions.query({ name: 'nfc' }).then(p => {
      permission = p;
      p.onchange = () => changed('nfc.nfcStatus');
      changed('nfc.nfcStatus');
    }, () => {});
  }

  // The session's scans by request, the reader serving them, and its abort.
  const scans = new Map();
  let reader = null, scanning = null, starting = null;
  // The waiting write: its request, its abort and how it settles.
  let writing = null;
  const readings = [];
  let last = 0;

  /** Scan claim `id` ends with `answer`; the reader stops when none is left. */
  function end(id, answer) {
    const claim = scans.get(id);
    if (!claim) return;
    scans.delete(id);
    claim.resolve(answer);
    if (scans.size === 0 && scanning) { scanning.abort(); scanning = null; reader = null; starting = null; }
  }

  function heard(event) {
    if (writing) return;
    const records = [...(event.message?.records ?? [])];
    const id = `r${++last}`;
    readings.push({ id, records: records.map(full) });
    while (readings.length > KEPT) readings.shift();
    emit('reading', [{ id, serialNumber: event.serialNumber ?? '', apple: null, android: null, records: records.map(info) }]);
    for (const [request, claim] of [...scans]) if (claim.once) end(request, { reason: 'read', reading: id, message: null });
  }

  /** The session's reader scanning, started once for every scan claim. */
  function started() {
    if (starting) return starting;
    reader = new Reader();
    scanning = new AbortController();
    reader.onreading = heard;
    reader.onreadingerror = () => emit('readingerror', [{ id: `t${++last}`, serialNumber: '', apple: null, android: null }]);
    const signal = scanning.signal;
    starting = reader.scan({ signal }).catch(e => {
      if (scanning?.signal === signal) { scanning = null; reader = null; starting = null; }
      throw e;
    });
    return starting;
  }

  /** A write or a lock, replacing any that waits. */
  function waitForTag(request, run) {
    if (writing) { const w = writing; writing = null; w.abort.abort(); w.resolve({ error: 'AbortError', message: 'replaced' }); }
    const abort = new AbortController();
    return new Promise(resolve => {
      const mine = { request, abort, resolve };
      writing = mine;
      const w = new Reader();
      run(w, abort.signal).then(
        () => { if (writing === mine) { writing = null; resolve({ serialNumber: '' }); } },
        e => { if (writing === mine) { writing = null; resolve(failure(e)); } },
      );
    });
  }

  return {
    nfcStatus() {
      const state = permission?.state ?? (has ? 'prompt' : 'denied');
      return { available: has, radio: 'unknown', state, readerUI: 'app' };
    },
    async scan(args, told = {}) {
      if (!has) return { error: 'NotSupportedError' };
      try {
        await started();
      } catch (e) {
        return failure(e, 'NotReadableError');
      }
      return new Promise(resolve => {
        scans.set(told.request ?? `s${++last}`, { once: args?.once === true, resolve });
        // A reader that fails after it started ends every scan.
        reader.addEventListener?.('error', () => { for (const id of [...scans.keys()]) end(id, { reason: 'interrupted', reading: null, message: null }); }, { once: true });
      });
    },
    async abortScan() {
      for (const id of [...scans.keys()]) end(id, { reason: 'stopped', reading: null, message: null });
    },
    async write(args, told = {}) {
      if (!has) return { error: 'NotSupportedError' };
      const records = (args?.message?.records ?? []).map(recordInit);
      if (records.length === 0) return { error: 'TypeError', message: 'a message has a record' };
      const options = { overwrite: args?.overwrite !== false };
      return waitForTag(told.request, (w, signal) => w.write({ records }, { ...options, signal }));
    },
    async makeReadOnly(args, told = {}) {
      if (!has) return { error: 'NotSupportedError' };
      return waitForTag(told.request, (w, signal) => typeof w.makeReadOnly === 'function' ? w.makeReadOnly({ signal }) : Promise.reject(new DOMException('makeReadOnly', 'NotSupportedError')));
    },
    async abortWrite() {
      if (!writing) return;
      const w = writing;
      writing = null;
      w.abort.abort();
      w.resolve({ error: 'AbortError', message: null });
    },
    async ndefRecords(args) {
      const kept = readings.find(r => r.id === args?.reading);
      return kept ? kept.records : { error: 'NotFoundError', message: String(args?.reading ?? '') };
    },
    /** A call let go (LLP 1133.016 D15 item 2): a scan stops, a write aborts. */
    cancel(request) {
      if (scans.has(request)) end(request, { reason: 'stopped', reading: null, message: null });
      else if (writing?.request === request) {
        const w = writing;
        writing = null;
        w.abort.abort();
        w.resolve({ error: 'AbortError', message: 'let go' });
      }
    },
    foreground(shown) {
      if (shown || !writing) return;
      const w = writing;
      writing = null;
      w.abort.abort();
      w.resolve({ error: 'AbortError', message: 'background' });
    },
  };
}
