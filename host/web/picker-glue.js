// @ref LLP 1069.002 — the file picker on the web, loaded on the first
// `showPicker` or file input `change`. The element is the picker (D8);
// each chosen File becomes an `app:/tmp/picked/…` entry backed by the File
// itself (D5), described by the browser (D3: pixel size from
// createImageBitmap, which applies EXIF orientation; a video's from its
// metadata), and delivered as `change` (host kind 26). Under the agent the
// driver's bytes arrive as Files and take the same path (D9). Also loaded
// by the first `app:/` source an `image` or `video` shows (D7): `appURL`,
// and by a `share` with `files=` (LLP 1125 D12): `share`.
const picked = new Map(); // app:/tmp/picked/… → File
const urls = new Map(); // app:/… → [its object URL, the version it was made from], for `image`/`video` (D7)
let opened; // the app's file store, or null when it has none (a drive without --storage)
let emptied; // the store once `app:/tmp/picked/` is emptied, at this page's first pick (D4)

// The store the app's own sources open (storage-environment.js `storageKey`,
// the page's launch URL's), so what a pick puts there the data module reads.
function fileStore(appId) {
  return opened ??= (async () => {
    try {
      const { storageKey } = await import('./storage-environment.js');
      const key = storageKey(appId);
      if (key == null) return null;
      const { createFileStore } = await import('./storage-fs.js');
      return createFileStore(key);
    } catch (error) {
      console.warn('exact: the app\'s files are not available', String(error));
      return null;
    }
  })();
}

/** An `app:/` source for an `image` or `video` (D7): an object URL for the
 * app's own file — one this page picked, or one in its store (a pick the
 * data module kept in `app:/data`, after a reload too) — or '' when there is
 * none. A file that changed gets a new URL; the last one is revoked. */
async function appURL(path, appId) {
  const file = picked.get(path);
  if (file) return made(path, file, file);
  try {
    const s = await fileStore(appId);
    if (!s) return '';
    const { blob, modifiedMs } = await s.blob(path, typeFor(path));
    return made(path, blob, `${modifiedMs}:${blob.size}`);
  } catch {
    return ''; // no such file, or not an app:/data|cache|tmp path: nothing to show
  }
}
function made(path, blob, version) {
  const was = urls.get(path);
  if (was?.[1] === version) return was[0];
  if (was) URL.revokeObjectURL(was[0]);
  const url = URL.createObjectURL(blob);
  urls.set(path, [url, version]);
  return url;
}
globalThis.exact.appURL = appURL;

// A bundled page's given files (LLP 1020.001 D7): for a page under `assets/`
// or `deck/` (an .html or .htm file), each query value that begins `app:/`,
// read as URLSearchParams reads it; null for any other source. WebModule.swift
// `givenPaths` reads a source the same way.
function pageFiles(src) {
  if (/^([a-z][a-z\d+.-]*:|[\\/]{2})/i.test(src)) return null;
  let url;
  try { url = new URL(src, 'https://app.invalid/'); } catch { return null; }
  const query = src.indexOf('?'), hash = src.indexOf('#'), end = hash < 0 ? src.length : hash;
  if (!/^\/(assets|deck)\/[^?#]*\.html?$/i.test(url.pathname) || query < 0 || hash >= 0 && hash < query) return null;
  const pairs = src.slice(query + 1, end).split('&').map(pair => {
    const at = pair.indexOf('=');
    let value = null;
    try { value = at < 0 ? null : decodeURIComponent(pair.slice(at + 1).replace(/\+/g, ' ')); } catch { /* a malformed escape names no file */ }
    return { name: at < 0 ? pair : pair.slice(0, at), path: value?.startsWith('app:/') ? value : null, pair };
  });
  if (!pairs.some(p => p.path)) return null;
  return { paths: [...new Set(pairs.filter(p => p.path).map(p => p.path))],
    rewrite: urlFor => src.slice(0, query + 1) + pairs.map(p => p.path ? `${p.name}=${encodeURIComponent(urlFor(p.path))}` : p.pair).join('&') + src.slice(end) };
}
const GIVEN = /^app:\/(cache|tmp)(\/(?!\.\.?(\/|$))[^/\0]+)+$/;

// Each file the page's query names becomes an object URL the frame owns, made
// from the store as the source commits and revoked with it.
async function framePage(el, state, appId, given, committed) {
  const store = await fileStore(appId), blobs = new Map();
  for (const path of given.paths) {
    if (!GIVEN.test(path)) throw new Error(`${path}: a page is given app:/cache and app:/tmp files only`);
    try { blobs.set(path, picked.get(path) ?? (await store.blob(path, typeFor(path))).blob); } catch { throw new Error(`${path}: file unavailable`); }
  }
  if (!state.live) return;
  const urls = new Map([...blobs].map(([path, blob]) => [path, URL.createObjectURL(blob)])), dispose = state.dispose;
  state.dispose = function () { dispose.call(this); for (const url of urls.values()) URL.revokeObjectURL(url); };
  state.url = given.rewrite(path => urls.get(path));
  el.setAttribute('src', state.url);
  committed();
}

// An iframe owns its URL: changing its source or ending its node releases
// it without revoking another element's view of the same file (LLP 1020 §10).
export async function appFrame(el, state, appId, committed = () => {}) {
  if (!state.path.startsWith('app:/')) {
    const given = pageFiles(state.path);
    // The boot glue's test is looser: a source it sent here that gives no
    // page a file (one outside assets/ and deck/, say) loads as written.
    if (!given) { state.url = state.path; el.setAttribute('src', state.path); return committed(); }
    return framePage(el, state, appId, given, committed).catch(error => {
      if (!state.live) return;
      state.failed = true;
      el.dispatchEvent(new CustomEvent('error', { detail: `iframe: ${state.path}: ${error.message}` }));
    });
  }
  try {
    const path = state.path.split(/[?#]/, 1)[0];
    const store = await fileStore(appId);
    const blob = picked.get(path) ?? (await store.blob(path, typeFor(path))).blob;
    if (!state.live) return;
    // Chrome refuses PDF viewers in every sandbox, including allow-same-origin.
    if (blob.type.split(';', 1)[0].toLowerCase() === 'application/pdf' && el.hasAttribute('sandbox'))
      throw new Error('sandbox blocks PDF documents');
    state.url = URL.createObjectURL(blob);
    el.setAttribute('src', state.url + (state.path.includes('#') ? '#' + state.path.split('#').slice(1).join('#') : ''));
    committed();
  } catch (error) {
    if (!state.live) return;
    state.failed = true;
    el.dispatchEvent(new CustomEvent('error', { detail: `iframe: ${state.path}: ${error.message === 'sandbox blocks PDF documents' ? error.message : 'file unavailable'}` }));
  }
}

globalThis.exact.picker = function install(host) {
  // host: { dispatch(id, kind, payload), generation() (none on the JS target), pickedPath(name), appId, log(line) }
  // A view id names a view only in the generation that asked: an answer that
  // outlives a restart, which reuses ids, is dropped. `at(g)` is these
  // operations for generation `g`, the one the glue asked in (the module may
  // load after a restart); each takes its `since()` before its first await.
  let pinned;
  const since = () => { const g = pinned ?? host.generation?.(); return (id, kind, payload) => { if (host.generation?.() === g) host.dispatch(id, kind, payload); }; };
  // What the last page picked is gone (D4).
  const pickStore = () => emptied ??= fileStore(host.appId).then(async s => {
    if (!s) return null;
    await s.rm('app:/tmp/picked').catch(() => {});
    await s.mkdir('app:/tmp/picked');
    return s;
  }).catch(error => {
    console.warn('exact: picked files are not stored', String(error));
    return null;
  });
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
  async function deliver(id, files, dispatch) {
    const s = await pickStore(), lines = [];
    for (const file of files) {
      const path = await host.pickedPath(file.name);
      picked.set(path, file);
      if (s) await s.putBlob(path, file);
      const { width, height, duration } = await describe(file);
      lines.push([path, clean(file.name), clean(file.type || typeFor(file.name)), file.size, width, height, duration].join('\t'));
    }
    dispatch(id, 26, lines.join('\n'));
  }
  // `saveFile` (LLP 1069.010 D3): the `app:/` file's bytes, from the store,
  // or the `text=` itself, as UTF-8.
  async function appBytes(from, text) {
    if (text != null) return new TextEncoder().encode(text);
    const s = await fileStore(host.appId);
    if (!s) throw new Error('this page has no app files');
    return new Uint8Array(await s.readFile(from));
  }
  const saved = (dispatch, view, name) => { host.log('saveFile: saved'); dispatch(view, 1, name); };
  const unsaved = (dispatch, view, line) => { host.log(line); dispatch(view, 27, ''); };
  const api = {
    /** `saveFile`'s ruling `r` (refused, or present) and, where the browser
     * has one, the save picker's pending handle: write the copy there, or
     * download it under the suggested name (Safari, Firefox). */
    async save(r, chosen) {
      const dispatch = since();
      if (!r.present) return dispatch(r.view, 27, '');
      let bytes;
      try { bytes = await appBytes(r.from, r.text); } catch (e) { return unsaved(dispatch, r.view, `saveFile: refused: ${e.message ?? e}`); }
      if (!chosen) {
        const url = URL.createObjectURL(new Blob([bytes])), a = document.createElement('a');
        a.href = url; a.download = r.suggestedName; a.click();
        setTimeout(() => URL.revokeObjectURL(url), 60_000);
        return saved(dispatch, r.view, r.suggestedName);
      }
      try {
        const handle = await chosen, w = await handle.createWritable();
        await w.write(bytes); await w.close();
        saved(dispatch, r.view, handle.name);
      } catch (e) { unsaved(dispatch, r.view, e?.name === 'AbortError' ? 'saveFile: cancelled' : `saveFile: refused: ${e?.name ?? e}`); }
    },
    /** The agent's answer to a held export: the bytes go back to the driver,
     * which writes them at its path; `change` carries that path's name. */
    async answerSave(r, path) {
      const dispatch = since();
      if (r.answered === 'cancel') { unsaved(dispatch, r.node, 'saveFile: cancelled'); return {}; }
      let bytes;
      try { bytes = await appBytes(r.request.from, r.request.text); } catch (e) { unsaved(dispatch, r.node, `saveFile: refused: ${e.message ?? e}`); return { error: `saveFile: ${e.message ?? e}` }; }
      let raw = '';
      for (let i = 0; i < bytes.length; i += 0x8000) raw += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      saved(dispatch, r.node, String(path).trim().split(/[\\/]/).filter(Boolean).pop());
      return { bytes: btoa(raw) };
    },
    /** `share(files=…)` (LLP 1125 D12): the share data `data` with each
     * `app:/` file the runner allowed (`{path}`) as a File named by its last
     * segment and typed by its extension, to the browser's sheet when
     * `navigator.canShare` takes them. Resolves to the journal line. The
     * reads are awaited inside the press's transient activation, which
     * `navigator.share` still needs when they end. */
    async share(data, files) {
      let made;
      try {
        made = await Promise.all(files.map(async ({ path }) => {
          const name = path.split('/').pop(), type = typeFor(name), file = picked.get(path);
          if (file) return new File([file], name, { type });
          const s = await fileStore(host.appId);
          if (!s) throw new Error('this page has no app files');
          return new File([(await s.blob(path, type)).blob], name, { type });
        }));
      } catch (e) { return `share: refused: ${e?.message ?? e}`; }
      const members = { ...Object.fromEntries(Object.entries(data).filter(([, v]) => v != null)), files: made };
      if (typeof navigator.canShare !== 'function' || !navigator.canShare(members)) return 'share: refused: this browser cannot share these files';
      return navigator.share(members).then(() => 'share: shared', e => e?.name === 'AbortError' ? 'share: dismissed' : `share: refused: ${e?.name ?? e}`);
    },
    /** The element's own `change`: what the person chose. */
    change: (el, id) => el.files?.length ? deliver(id, [...el.files], since()).finally(() => { el.value = ''; }) : Promise.resolve(),
    cancel: (id) => since()(id, 27, ''),
    appURL: (path) => appURL(path, host.appId),
    appFrame: (el, state, committed) => appFrame(el, state, host.appId, committed),
    /** The agent's answer (D9): `files` as the driver sent them, or null for `tap @t cancel`. */
    answer(id, files) {
      const dispatch = since();
      if (!files) { dispatch(id, 27, ''); return Promise.resolve(); }
      const made = files.map(({ name, bytes }) => {
        const raw = atob(bytes), data = new Uint8Array(raw.length);
        for (let i = 0; i < raw.length; i++) data[i] = raw.charCodeAt(i);
        return new File([data], name, { type: typeFor(name) });
      });
      return deliver(id, made, dispatch);
    },
  };
  return { ...api, at: g => pinnedTo(api, g, v => { pinned = v; }) };
};

function pinnedTo(api, g, pin) {
  return Object.fromEntries(Object.entries(api).map(([name, f]) => [name, (...args) => { pin(g); try { return f(...args); } finally { pin(undefined); } }]));
}

// A browser's guess from the name, for the agent's files.
function typeFor(name) {
  const ext = name.toLowerCase().split('.').pop();
  return { jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', gif: 'image/gif', webp: 'image/webp', heic: 'image/heic', heif: 'image/heif', avif: 'image/avif',
    svg: 'image/svg+xml', mp3: 'audio/mpeg', wav: 'audio/wav', m4a: 'audio/mp4', ogg: 'audio/ogg', mp4: 'video/mp4', mov: 'video/quicktime', m4v: 'video/x-m4v', webm: 'video/webm', pdf: 'application/pdf', html: 'text/html', htm: 'text/html', xhtml: 'application/xhtml+xml', xml: 'application/xml', json: 'application/json', md: 'text/markdown', txt: 'text/plain', log: 'text/plain', csv: 'text/csv', tsv: 'text/tab-separated-values', css: 'text/css', yaml: 'application/yaml', yml: 'application/yaml' }[ext] ?? 'application/octet-stream';
}
