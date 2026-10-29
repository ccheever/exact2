// A fake Web NFC for Chromium's test pages (LLP 1133.035 §12 stage 1): only
// Chrome on Android ships `NDEFReader`. `page.addInitScript(fake)` installs
// it before the page loads; `__nfc.tap(records, serial)` brings a tag into
// the field, which settles the waiting write or lock and is read by every
// scanning reader.

/** Web NFC's reader, as far as the implementation uses it (https://w3c.github.io/web-nfc/#the-ndefreader-object). */
export function fake() {
  const scanning = new Set(), waiting = [], written = [];
  const aborted = () => new DOMException('The operation was aborted.', 'AbortError');
  // A tag's record as Chrome hands it over: its data a DataView, a string given as its UTF-8.
  const record = r => ({ ...r, data: r.data == null ? null : new DataView((typeof r.data === 'string' ? new TextEncoder().encode(r.data) : new Uint8Array(r.data)).buffer), toRecords: () => null });
  class NDEFReader extends EventTarget {
    onreading = null;
    onreadingerror = null;
    scan({ signal } = {}) {
      if (signal?.aborted) return Promise.reject(aborted());
      scanning.add(this);
      signal?.addEventListener('abort', () => scanning.delete(this));
      return Promise.resolve();
    }
    write(message, { signal } = {}) { return this.#wait('write', message, signal); }
    makeReadOnly({ signal } = {}) { return this.#wait('lock', null, signal); }
    #wait(kind, message, signal) {
      if (signal?.aborted) return Promise.reject(aborted());
      return new Promise((resolve, reject) => {
        const w = { kind, message, resolve };
        waiting.push(w);
        signal?.addEventListener('abort', () => { const at = waiting.indexOf(w); if (at >= 0) waiting.splice(at, 1); reject(aborted()); });
      });
    }
  }
  const encode = r => ({ ...r, data: typeof r.data === 'string' ? [...new TextEncoder().encode(r.data)] : r.data ? [...new Uint8Array(r.data.buffer ?? r.data)] : null });
  globalThis.NDEFReader = NDEFReader;
  const query = navigator.permissions.query.bind(navigator.permissions);
  navigator.permissions.query = d => d?.name === 'nfc' ? Promise.resolve({ state: 'granted', onchange: null }) : query(d);
  globalThis.__nfc = {
    tap(records, serialNumber) {
      const w = waiting.shift();
      if (w) {
        written.push({ kind: w.kind, records: (w.message?.records ?? []).map(encode) });
        w.resolve();
      }
      const event = Object.assign(new Event('reading'), { serialNumber, message: { records: records.map(record) } });
      for (const reader of scanning) reader.onreading?.(event);
    },
    unreadable() {
      for (const reader of scanning) reader.onreadingerror?.(new Event('readingerror'));
    },
    written: () => written,
    scanning: () => scanning.size,
  };
}
