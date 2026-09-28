// @ref LLP 1069.002 — the file picker on the web, loaded on the first
// `showPicker` or file input `change`. The element is the picker (D8);
// each chosen File becomes an `app:/tmp/picked/…` entry backed by the File
// itself (D5), described by the browser (D3: pixel size from
// createImageBitmap, which applies EXIF orientation; a video's from its
// metadata), and delivered as `change` (host kind 26). Under the agent the
// driver's bytes arrive as Files and take the same path (D9).
const picked = new Map(); // app:/tmp/picked/… → File
const urls = new Map(); // app:/tmp/picked/… → its blob URL, for `image`/`video` (D7)
let store; // the app's file store, or null when it has none (a drive without --storage)

globalThis.exact.picker = function install(host) {
  // host: { dispatch(id, kind, payload), pickedPath(name), appId, log(line) }
  async function fileStore() {
    if (store !== undefined) return store;
    try {
      const { storageKey } = await import('./storage-environment.js');
      const key = storageKey(host.appId, location.href);
      if (key == null) return store = null;
      const { createFileStore } = await import('./storage-fs.js');
      const s = createFileStore(key);
      // What the last page picked is gone (D4).
      await s.rm('app:/tmp/picked').catch(() => {});
      await s.mkdir('app:/tmp/picked');
      return store = s;
    } catch (error) {
      console.warn('exact: picked files are not stored', String(error));
      return store = null;
    }
  }
  async function describe(file) {
    const size = { width: '', height: '', duration: '' };
    try {
      if (file.type.startsWith('image/')) {
        const bitmap = await createImageBitmap(file);
        size.width = bitmap.width; size.height = bitmap.height; bitmap.close();
      } else if (file.type.startsWith('video/')) {
        const video = document.createElement('video'), url = URL.createObjectURL(file);
        video.preload = 'metadata'; video.muted = true;
        await new Promise((resolve, reject) => { video.onloadedmetadata = resolve; video.onerror = reject; video.src = url; });
        size.width = video.videoWidth; size.height = video.videoHeight;
        if (Number.isFinite(video.duration)) size.duration = video.duration;
        URL.revokeObjectURL(url);
      }
    } catch { /* delivered without the size fields (D3) */ }
    return size;
  }
  const clean = (s) => String(s).replace(/[\t\r\n]/g, ' ');
  async function deliver(id, files) {
    const s = await fileStore(), lines = [];
    for (const file of files) {
      const path = await host.pickedPath(file.name);
      picked.set(path, file);
      if (s) await s.putBlob(path, file);
      const { width, height, duration } = await describe(file);
      lines.push([path, clean(file.name), clean(file.type || 'application/octet-stream'), file.size, width, height, duration].join('\t'));
    }
    host.dispatch(id, 26, lines.join('\n'));
  }
  // `saveFile` (LLP 1069.010 D3): the `app:/` file's bytes, from the store.
  async function appBytes(from) {
    const s = await fileStore();
    if (!s) throw new Error('this page has no app files');
    return new Uint8Array(await s.readFile(from));
  }
  const saved = (view, name) => { host.log('saveFile: saved'); host.dispatch(view, 1, name); };
  const unsaved = (view, line) => { host.log(line); host.dispatch(view, 27, ''); };
  return {
    /** `saveFile`'s ruling `r` (refused, or present) and, where the browser
     * has one, the save picker's pending handle: write the copy there, or
     * download it under the suggested name (Safari, Firefox). */
    async save(r, chosen) {
      if (!r.present) return host.dispatch(r.view, 27, '');
      let bytes;
      try { bytes = await appBytes(r.from); } catch (e) { return unsaved(r.view, `saveFile: refused: ${e.message ?? e}`); }
      if (!chosen) {
        const url = URL.createObjectURL(new Blob([bytes])), a = document.createElement('a');
        a.href = url; a.download = r.suggestedName; a.click();
        setTimeout(() => URL.revokeObjectURL(url), 60_000);
        return saved(r.view, r.suggestedName);
      }
      try {
        const handle = await chosen, w = await handle.createWritable();
        await w.write(bytes); await w.close();
        saved(r.view, handle.name);
      } catch (e) { unsaved(r.view, e?.name === 'AbortError' ? 'saveFile: cancelled' : `saveFile: refused: ${e?.name ?? e}`); }
    },
    /** The agent's answer to a held export: the bytes go back to the driver,
     * which writes them at its path; `change` carries that path's name. */
    async answerSave(r, path) {
      if (r.answered === 'cancel') { unsaved(r.node, 'saveFile: cancelled'); return {}; }
      let bytes;
      try { bytes = await appBytes(r.request.from); } catch (e) { unsaved(r.node, `saveFile: refused: ${e.message ?? e}`); return { error: `saveFile: ${e.message ?? e}` }; }
      let raw = '';
      for (let i = 0; i < bytes.length; i += 0x8000) raw += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      saved(r.node, String(path).trim().split(/[\\/]/).filter(Boolean).pop());
      return { bytes: btoa(raw) };
    },
    /** The element's own `change`: what the person chose. */
    change: (el, id) => el.files?.length ? deliver(id, [...el.files]).finally(() => { el.value = ''; }) : Promise.resolve(),
    cancel: (id) => host.dispatch(id, 27, ''),
    /** The agent's answer (D9): `files` as the driver sent them, or null for `tap @t cancel`. */
    answer(id, files) {
      if (!files) { host.dispatch(id, 27, ''); return Promise.resolve(); }
      const made = files.map(({ name, bytes }) => {
        const raw = atob(bytes), data = new Uint8Array(raw.length);
        for (let i = 0; i < raw.length; i++) data[i] = raw.charCodeAt(i);
        return new File([data], name, { type: typeFor(name) });
      });
      return deliver(id, made);
    },
  };
};

/** A blob URL for an `app:/tmp/picked/…` source this page picked (D7), or
 * an empty string (nothing to show) for any other `app:/` path. */
globalThis.exact.pickedURL = function (path) {
  const file = picked.get(path);
  if (!file) return '';
  if (!urls.has(path)) urls.set(path, URL.createObjectURL(file));
  return urls.get(path);
};

// A browser's guess from the name, for the agent's files.
function typeFor(name) {
  const ext = name.toLowerCase().split('.').pop();
  return { jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', gif: 'image/gif', webp: 'image/webp', heic: 'image/heic', heif: 'image/heif', avif: 'image/avif',
    mp4: 'video/mp4', mov: 'video/quicktime', m4v: 'video/x-m4v', webm: 'video/webm', json: 'application/json', md: 'text/markdown', txt: 'text/plain' }[ext] ?? 'application/octet-stream';
}
