// @ref LLP 1069.010 D1, D2 — documents in place on the web. What the person
// chooses is a FileSystemHandle the page keeps; the app holds only its
// `doc:/<n>/<name>` path and reaches the bytes through `storage.fs` under
// `fs.read doc:/` / `fs.write doc:/` (storage-request.js calls `run`). The
// File System Access API's pickers are the commands of the same names; a
// browser without the open pickers (Safari, Firefox) refuses and fires
// `cancel` (ruled: no fallback to a hidden input's copy), which the app can
// tell from a person's dismissal by `exactPage().canOpenFiles`. Its save
// picker's handle downloads what is written to it instead, as `saveFile`
// does (LLP 1069.010 D2's table; studio diary R31). Under the agent the
// driver's files arrive as bytes and become handles that behave as the
// browser's do.
import { coversPath } from './grant-admission.js';
import { storageKey } from './storage-environment.js';
const handles = new Map(); // n → FileSystemHandle
const names = new Map(); // n → the name its path was given (a kept entry's, the name at the choice)
let next = 0;

// A session handle for what the person chose: none when a keeping page has
// no number reserved to give (`numbered` waits for one).
function mint(handle) {
  for (const [n, h] of handles) if (h === handle) return `doc:/${n}/${names.get(n) ?? handle.name}`;
  const n = number();
  if (n === undefined) return null;
  handles.set(n, handle); names.set(n, handle.name);
  return `doc:/${n}/${handle.name}`;
}
async function numbered(handle) {
  for (;;) {
    const path = mint(handle);
    if (path || !kept?.loaded) return path ?? mint(handle);
    await reserve();
  }
}

// Documents kept across launches (LLP 1069.010.000 D3–D7). For an app
// granted `doc.keep`, the FileSystemHandle the person chose is kept in
// IndexedDB, as any page may keep one: its identity is the browser's (D4),
// and so is its permission, which a reload may withhold until a press asks
// again (`requestPermission`, D7). The database is the app's storage key's
// (none for a drive without a scratch store), so clearing the site's data
// revokes (D6). Each entry is `<n>`, the name at the choice, its kind, the
// mode the browser gave, when it was last chosen, and the handle. Every
// `<n>` a keeping page gives comes from a counter the database keeps,
// reserved a block at a time in a transaction and never given past its
// block, so no reload and no other tab of the app gives one again (D4).
// Choosing and forgetting hold the app's Web Lock, so tabs take turns.
const KEPT_MOST = 20; // the web has no recents list of its own: this one is bounded too
const BLOCK = 256;
let kept = null;
const request = (r) => new Promise((ok, no) => { r.onsuccess = () => ok(r.result); r.onerror = () => no(r.error); });
const committed = (tx) => new Promise((ok, no) => { tx.oncomplete = () => ok(); tx.onabort = tx.onerror = () => no(tx.error ?? new DOMException('aborted', 'AbortError')); });
const keepsDocuments = (scope) => !!scope?.entries?.some((e) => e[1] === 'doc.keep');
// Only the browser's own handles can be kept (the agent's and a dropped copy cannot).
const storable = (handle) => typeof FileSystemHandle === 'function' && handle instanceof FileSystemHandle;

/** Keep what the person chooses, for the app `appId` under `scope`, once
 * a page knows both: the kept entries read back, and a block of numbers
 * reserved, before anything here mints. Nothing for an app without
 * `doc.keep`, or a drive with no scratch store. */
function keeping(appId, scope) {
  if (kept !== null || !appId || !keepsDocuments(scope) || typeof indexedDB === 'undefined') return kept?.ready;
  let key = null;
  try { key = storageKey(appId); } catch { /* a drive's store name it refuses */ }
  if (key == null) return;
  const k = kept = { key, records: new Map(), next: 0, end: 0, reserving: null, loaded: false, turn: Promise.resolve() };
  const open = indexedDB.open(`exact-documents:${key}`, 1);
  open.onupgradeneeded = () => { open.result.createObjectStore('kept', { keyPath: 'n' }); open.result.createObjectStore('meta'); };
  k.db = request(open);
  k.ready = k.db.then(refresh).then(reserve).then(() => { k.loaded = true; }, () => {
    // Keeps nothing this page; numbers stay above every one it read back.
    if (kept === k) kept = null;
    next = Math.max(next, ...handles.keys());
  });
  return k.ready;
}
// The entries as the database holds them now (another tab may have kept or forgotten one).
async function refresh() {
  const db = await kept.db, all = await request(db.transaction('kept').objectStore('kept').getAll());
  kept.records = new Map(all.map((r) => [r.n, r]));
  for (const r of all) if (!handles.has(r.n)) { handles.set(r.n, r.handle); names.set(r.n, r.name); }
}
function reserve() {
  const k = kept;
  return k.reserving ??= (async () => {
    const db = await k.db, tx = db.transaction('meta', 'readwrite'), meta = tx.objectStore('meta');
    const from = Math.max((await request(meta.get('next'))) ?? 1, k.next, ...[...k.records.keys()].map((n) => n + 1));
    meta.put(from + BLOCK, 'next');
    await committed(tx);
    k.next = from; k.end = from + BLOCK;
  })().finally(() => { k.reserving = null; });
}
function number() {
  if (!kept?.loaded) return ++next;
  if (kept.end - kept.next < BLOCK / 2) reserve().catch(() => {});
  return kept.next < kept.end ? kept.next++ : undefined;
}
// Who hears the kept list change (`storage.fs.watch('doc:/')`, D5): the JS
// target's answers, and the wasm host's runner (glue.js `keptChanged`).
const listeners = new Set();
const told = () => { for (const f of listeners) f(); globalThis.exact?.keptChanged?.(); };
// `work` in turn with every other choice and forgetting of this app's, in
// this tab and, under the app's Web Lock, in its others.
function inTurn(work) {
  const k = kept, run = () => navigator.locks ? navigator.locks.request(`exact-documents:${k.key}`, work) : work();
  const done = k.turn.then(run, run);
  k.turn = done.catch(() => {});
  return done;
}

/** What the person chose (D3), kept for an app that keeps documents: the
 * entry it already was when chosen again (the browser's `isSameEntry`),
 * now the most recent; else a new one. Its path, or a session handle's
 * when nothing is kept. */
async function choose(handle) {
  await kept?.ready;
  if (!kept?.loaded || !storable(handle)) return numbered(handle);
  return inTurn(async () => {
    await refresh().catch(() => {});
    let was = null;
    for (const r of kept.records.values()) if (r.kind === handle.kind && await Promise.resolve().then(() => r.handle.isSameEntry(handle)).catch(() => false)) { was = r; break; }
    const n = was?.n ?? Number((await numbered(handle)).split('/')[1]);
    handles.set(n, handle); names.set(n, was?.name ?? handle.name);
    const mode = await handle.queryPermission?.({ mode: 'readwrite' }).catch(() => null) === 'granted' ? 'readwrite' : 'read';
    const entry = { n, name: names.get(n), kind: handle.kind, mode, opened: Date.now(), handle };
    try {
      const db = await kept.db, tx = db.transaction('kept', 'readwrite'), store = tx.objectStore('kept');
      store.put(entry);
      // Past the bound the oldest is no longer reopened after a reload; a page that has it open keeps it.
      const older = [...kept.records.values()].filter((r) => r.n !== n).sort((a, b) => b.opened - a.opened).slice(KEPT_MOST - 1);
      for (const r of older) store.delete(r.n);
      await committed(tx);
      for (const r of older) kept.records.delete(r.n);
      kept.records.set(n, entry);
      told();
    } catch { /* not written is not kept */ }
    return `doc:/${n}/${entry.name}`;
  });
}

const notKept = (path) => failure(`document ENOENT: ${path} is not a document this app keeps`, 'ENOENT');
const needsKeep = () => failure('storage: recentDocuments and forgetDocument need the doc.keep grant');
/** `storage.fs.recentDocuments()` (D5): the kept entries, most recent first, without reading them. */
async function recent(scope) {
  if (!keepsDocuments(scope)) throw needsKeep();
  await kept?.ready;
  if (!kept?.loaded) return [];
  await refresh();
  return [...kept.records.values()].sort((a, b) => b.opened - a.opened)
    .map(({ n, name, kind, mode, opened }) => ({ path: `doc:/${n}/${name}`, name, kind, mode, opened }));
}
/** `storage.fs.forgetDocument(path)` (D5): off the list; the page keeps reaching it until it closes. */
async function forget(path, scope) {
  if (!keepsDocuments(scope)) throw needsKeep();
  await kept?.ready;
  if (!kept?.loaded) throw notKept(path);
  const m = /^doc:\/(\d+)\/([^/]+)\/?$/.exec(String(path));
  if (!m || !await drop(Number(m[1]), m[2])) throw notKept(path);
}
// Entry `n`, named `name` (any name when none), off the database: whether it was kept.
async function drop(n, name) {
  return inTurn(async () => {
    await refresh();
    const r = kept.records.get(n);
    if (!r || (name !== undefined && r.name !== name)) return false;
    const db = await kept.db, tx = db.transaction('kept', 'readwrite');
    tx.objectStore('kept').delete(n);
    await committed(tx);
    kept.records.delete(n);
    told();
    return true;
  });
}
/** `requestPermission(id, path, mode)` (D7): the browser asked, inside the
 * press's activation, for the handle `path` names; `answer` hears
 * `granted` or `denied`. A handle with no permission of its own (the
 * agent's, a dropped copy) answers what it has. A number this page has not
 * read back yet is looked up first, outside the press, and so is denied by
 * a browser that needs one. */
function permission(path, mode, answer) {
  const m = /^doc:\/(\d+)(?:\/([^/]+))?/.exec(String(path)), n = m && Number(m[1]);
  const ask = (handle) => {
    if (!handle || (m[2] !== undefined && m[2] !== names.get(n))) return answer('denied');
    const asked = typeof handle.requestPermission === 'function' ? handle.requestPermission({ mode }) : Promise.resolve(handle.queryPermission?.({ mode }) ?? 'granted');
    return asked.then((s) => answer(s === 'granted' ? 'granted' : 'denied'), () => answer('denied'));
  };
  if (!m) return answer('denied');
  return handles.has(n) ? ask(handles.get(n)) : keptHandle(n).then(ask);
}
// A kept entry another tab kept since this page read the database.
async function keptHandle(n) {
  await kept?.ready;
  if (kept?.loaded && !handles.has(n)) await refresh().catch(() => {});
  return handles.get(n);
}

// One storage operation's failure, coded as every host codes it (kanban
// F28): the DOM's names for the filesystem's (NotFoundError is ENOENT).
const CODES = { NotFoundError: 'ENOENT', TypeMismatchError: 'ENOTDIR', InvalidModificationError: 'ENOTEMPTY' };
function failure(message, code = 'failed') { return Object.assign(new Error(message), { kind: 'Unavailable', code }); }
// A name of the wrong kind at the end of the path: what a native host's
// `read`, `write` or `create_dir_all` says of it.
const MISMATCH = { readFile: 'EISDIR', copyFile: 'EISDIR', writeFile: 'EISDIR', atomicWriteFile: 'EISDIR', appendFile: 'EISDIR', mkdir: 'EEXIST' };
const failed = (op, path, e, last = false) => e?.kind ? e
  : failure(`fs.${op} ${path}: ${e?.message ?? e}`, (last && e?.name === 'TypeMismatchError' && MISMATCH[op]) || (CODES[e?.name] ?? 'failed'));

// Where a `doc:` path leads: `{root}` for the handle's own directory, else
// the handle it names (`entry`) or, for a name that may not exist yet, its
// folder and that name. `.`, `..` and empty segments are refused. With
// `write` (true or false), the browser's permission is checked before
// anything beneath the handle is reached.
async function locate(op, path, create = false, write = undefined) {
  const m = /^doc:\/(\d+)(?:\/(.*))?$/.exec(path), n = m && Number(m[1]), handle = m && (handles.get(n) ?? await keptHandle(n));
  if (!handle) throw failure(`document ENOENT: ${path}: no such document (it was never opened, its page closed, or it is no longer kept)`, 'ENOENT');
  const rest = (m[2] ?? '').replace(/\/$/, ''), parts = rest === '' ? [] : rest.split('/');
  if (parts.some((p) => p === '' || p === '.' || p === '..')) throw failure(`${path}: a document path has no \`.\`, \`..\` or empty segment`);
  const name = names.get(n) ?? handle.name;
  if (!parts.length) return { root: name, handle };
  if (parts[0] !== name) throw failure(`document ENOENT: ${path}: no such document`, 'ENOENT');
  if (write !== undefined) await permitted(op, path, n, handle, write);
  if (parts.length === 1) return { top: handle, entry: handle, n };
  let at = handle;
  try {
    for (const part of parts.slice(1, -1)) {
      if (at.kind !== 'directory') throw new DOMException(part, 'TypeMismatchError');
      at = await at.getDirectoryHandle(part, { create });
    }
    if (at.kind !== 'directory') throw new DOMException(parts.at(-1), 'TypeMismatchError');
  } catch (e) { throw await gone(n, handle, failed(op, path, e)); }
  return { top: handle, folder: at, name: parts.at(-1), n };
}
const entry = (found) => found.entry ?? found.folder.getFileHandle(found.name).catch((e) => e?.name === 'TypeMismatchError' ? found.folder.getDirectoryHandle(found.name) : Promise.reject(e));
// The browser's permission for the chosen handle: a read never prompts; a
// write asks, which the browser grants only inside a press. A kept handle
// after a reload may have none until `requestPermission` (D7): `EACCES`,
// apart from a gone entry's `ENOENT`. The browser's permission is the
// person's answer, so a kept entry chosen for reading that the browser
// now lets the page write is kept as `readwrite` (D3: the app's own grants
// never widen it; the person's answer to the browser does).
async function permitted(op, path, n, top, write) {
  const mode = write ? 'readwrite' : 'read';
  const granted = (await top.queryPermission?.({ mode }).catch(() => 'granted') ?? 'granted') === 'granted'
    || (write && await top.requestPermission?.({ mode }).catch(() => 'denied') === 'granted');
  if (!granted) throw failure(`fs.${op} ${path}: the browser withholds this document's permission; ask again with requestPermission under a press`, 'EACCES');
  const r = write && kept?.records.get(n);
  if (r && r.mode === 'read' && r.handle === top) {
    r.mode = 'readwrite';
    kept.db.then((db) => { db.transaction('kept', 'readwrite').objectStore('kept').put(r); }).catch(() => {});
  }
}
// A failure that found the kept item itself gone: forgotten now, as D4
// forgets a document a read finds gone (its file or folder, not a name
// beneath a folder).
async function gone(n, top, error) {
  if (error?.code !== 'ENOENT' || !kept?.records.has(n)) return error;
  const missing = await (top.kind === 'file' ? top.getFile() : top.entries().next()).then(() => false, (e) => e?.name === 'NotFoundError');
  if (missing) await drop(n).catch(() => {});
  return error;
}
const STAT_DIR = { size: 0, isFile: false, isDirectory: true, modifiedMs: 0 };

/** One `storage.fs` operation on a `doc:` path under `scope` (the app's
 * grants), with the semantics of ibex2's `run_document`, which a native
 * host runs for both languages: `readFile` an ArrayBuffer, `rm` one file
 * or empty folder, `mkdir` with its parents, `rename` and `realpath`
 * refused, and `copyFile` refused except into an app file (`copyFile`
 * below). A Rust source reaches it through storage-request.js, a
 * TypeScript one through `files`. */
async function operate(op, path, bytes, scope, expect = null) {
  if (op === 'copyFile') throw failure('fs.copyFile copies a document only into an app:/ path');
  if (['rename', 'realpath'].includes(op)) throw failure(`fs.${op} is not available on a document; read its bytes and write them`);
  const write = ['writeFile', 'atomicWriteFile', 'appendFile', 'mkdir', 'rm'].includes(op), grant = write ? 'fs.write' : 'fs.read';
  if (!coversPath(scope, grant, path)) throw failure(`denied: fs.${op} ${path}: needs \`${grant} doc:/\``, 'denied');
  // `mkdir` makes the folders above too, as `create_dir_all` does.
  const found = await locate(op, path, op === 'mkdir', write);
  if (found.root !== undefined) {
    if (op === 'readdir') return [found.root];
    if (op === 'stat') return { ...STAT_DIR };
    throw failure(`fs.${op} ${path}: a handle's own directory is read-only`);
  }
  if (op === 'mkdir' && found.entry && found.entry.kind !== 'directory') throw failure(`fs.mkdir ${path}: it is a file`, 'EEXIST');
  try {
    switch (op) {
      case 'readFile': {
        const h = await entry(found);
        if (h.kind === 'directory') throw failure(`fs.readFile ${path}: it is a folder`, 'EISDIR');
        return await (await h.getFile()).arrayBuffer();
      }
      case 'stat': {
        const h = await entry(found);
        if (h.kind === 'directory') return { ...STAT_DIR };
        const file = await h.getFile();
        return { size: file.size, isFile: true, isDirectory: false, modifiedMs: file.lastModified };
      }
      case 'readdir': {
        const h = await entry(found), names = [];
        if (h.kind !== 'directory') throw new DOMException(path, 'TypeMismatchError');
        for await (const [name] of h.entries()) names.push(name);
        return names.sort();
      }
      case 'writeFile': case 'atomicWriteFile': case 'appendFile': {
        // A writable commits on close, which is what makes it atomic here.
        // The chosen folder itself is a folder, as `readFile` says (review B6).
        if (found.entry?.kind === 'directory') throw failure(`fs.${op} ${path}: it is a folder`, 'EISDIR');
        // A write that expects what it read (LLP 1069.010.000 D2a): the
        // writable has no precondition, so the file is compared first.
        if (expect) {
          const now = await (found.entry ?? await found.folder.getFileHandle(found.name)).getFile();
          if (now.lastModified !== expect.modifiedMs || now.size !== expect.size) throw failure(`document ECHANGED: ${path} changed on disk since it was read`, 'ECHANGED');
        }
        const h = found.entry ?? await found.folder.getFileHandle(found.name, { create: true });
        const w = await h.createWritable({ keepExistingData: op === 'appendFile' });
        if (op === 'appendFile') await w.seek((await h.getFile()).size);
        await w.write(bytes); await w.close(); return null;
      }
      case 'mkdir': {
        if (found.entry) return null;
        await found.folder.getDirectoryHandle(found.name, { create: true }); return null;
      }
      case 'rm': {
        if (found.entry) throw failure(`fs.rm ${path}: the document itself is the person's; remove what is in it`);
        await found.folder.removeEntry(found.name); return null;
      }
      default: throw failure(`fs.${op} is not available on a document`);
    }
  } catch (e) { throw await gone(found.n, found.top, failed(op, path, e, true)); }
}

/** `copyFile(from, to)` from a document into an app file (LLP 1069.010 D1,
 * ruled 2026-10-26), as ibex2's `copy_document`: `fs.read` on `from`, then
 * `fs` (the app's storage-fs.js file system) admits `to` under `fs.write`
 * before the document is opened, and keeps its File as the app file, so
 * the bytes never pass through JavaScript. */
async function copyFile(from, to, scope, fs) {
  if (!coversPath(scope, 'fs.read', from)) throw failure(`denied: fs.copyFile ${from}: needs \`fs.read doc:/\``, 'denied');
  return fs.copyIn(to, async () => {
    const found = await locate('copyFile', from, false, false);
    if (found.root !== undefined) throw failure(`fs.copyFile ${from}: a handle's own directory is read-only`);
    try {
      const h = await entry(found);
      if (h.kind === 'directory') throw failure(`fs.copyFile ${from}: it is a folder`, 'EISDIR');
      return await h.getFile();
    } catch (e) { throw failed('copyFile', from, e, true); }
  });
}

function base64(bytes) { let text = ''; for (let i = 0; i < bytes.length; i += 16384) text += String.fromCharCode(...bytes.subarray(i, i + 16384)); return btoa(text); }

/** A storage request's operation (storage-request.js, a Rust source's). */
async function run(op, args, bytes, scope) {
  const value = await operate(op.replace(/^fs\./, ''), args.path, bytes, scope, args.expect ?? null);
  return value instanceof ArrayBuffer ? { base64: base64(new Uint8Array(value)) } : value;
}

/** `storage.fs` over documents for a TypeScript source under `scope`:
 * the methods of an app path, the bytes copied at the call. */
function files(scope) {
  const copy = (data) => ArrayBuffer.isView(data) ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice() : new Uint8Array(ArrayBuffer.prototype.slice.call(data, 0));
  return Object.freeze(Object.fromEntries(['readFile', 'writeFile', 'atomicWriteFile', 'appendFile', 'readdir', 'mkdir', 'rm', 'stat', 'rename', 'copyFile', 'realpath']
    .map((op) => [op, (path, data, options) => {
      const writes = ['writeFile', 'atomicWriteFile', 'appendFile'].includes(op);
      return operate(op, String(path), writes ? copy(data) : null, scope, writes ? options?.expect ?? null : null);
    }])));
}

// Document watches (LLP 1069.010.000 D1, D2): the browser's
// `FileSystemObserver` on the handle a watched path names, one per path and
// scope; each change calls `changed` and the source's answer is asked
// again. A browser without it accepts a watch that never fires, as
// `exactPage().canWatchFiles` says.
const observers = new Map(); // `${recursive} ${path}` -> FileSystemObserver
async function observe(path, recursive, scope, changed) {
  const key = `${recursive} ${path}`;
  if (observers.has(key) || typeof globalThis.FileSystemObserver !== 'function' || !coversPath(scope, 'fs.read', path)) return;
  observers.set(key, null);
  try {
    const found = await locate('watch', path);
    const handle = found.root !== undefined ? found.handle : await entry(found);
    const observer = new globalThis.FileSystemObserver(() => changed());
    await observer.observe(handle, { recursive });
    observers.set(key, observer);
  } catch { observers.delete(key); }
}

// The driver's files as handles the page keeps (LLP 1069.007 D3): a file's
// bytes, a folder's tree, or a name to save to.
// They behave as the File System Access API's do: a name of the other kind
// is TypeMismatchError, a missing one NotFoundError, a non-empty folder
// can't be removed (InvalidModificationError).
function memoryFile(name, data = new Uint8Array()) {
  return { kind: 'file', name, async getFile() { return new File([data], name); },
    async createWritable({ keepExistingData = false } = {}) {
      let at = 0, next = keepExistingData ? data.slice() : new Uint8Array();
      return { async seek(n) { at = n; }, async write(d) {
        const b = new Uint8Array(await new Blob([d]).arrayBuffer()), out = new Uint8Array(Math.max(next.length, at + b.length));
        out.set(next); out.set(b, at); next = out; at += b.length;
      }, async close() { data = next; } };
    },
    async queryPermission() { return 'granted'; } };
}
function memoryFolder(name, files) {
  const children = new Map();
  for (const { path, bytes } of files) {
    const [first, ...rest] = path.split('/');
    if (!rest.length) children.set(first, memoryFile(first, bytes));
    else (children.get(first)?.files ?? children.set(first, { files: [] }).get(first).files).push({ path: rest.join('/'), bytes });
  }
  for (const [n, c] of children) if (c.files) children.set(n, memoryFolder(n, c.files));
  const child = (n, kind, create, make) => {
    const c = children.get(n);
    if (c) { if (c.kind !== kind) throw new DOMException(n, 'TypeMismatchError'); return c; }
    if (!create) throw new DOMException(n, 'NotFoundError');
    children.set(n, make(n)); return children.get(n);
  };
  return { kind: 'directory', name,
    async getFileHandle(n, { create = false } = {}) { return child(n, 'file', create, (m) => memoryFile(m)); },
    async getDirectoryHandle(n, { create = false } = {}) { return child(n, 'directory', create, (m) => memoryFolder(m, [])); },
    async removeEntry(n) {
      const c = children.get(n);
      if (!c) throw new DOMException(n, 'NotFoundError');
      if (c.kind === 'directory') for await (const _ of c.entries()) throw new DOMException(n, 'InvalidModificationError');
      children.delete(n);
    },
    async *entries() { for (const e of [...children]) yield e; },
    async queryPermission() { return 'granted'; } };
}
// A save where the browser has no picker (D2's "else D3's download"): a
// handle whose every write, once closed, downloads under its name.
function downloadFile(name) {
  const file = memoryFile(name), writable = file.createWritable;
  file.createWritable = async (options) => {
    const w = await writable(options);
    return { ...w, async close() { await w.close(); const a = document.createElement('a'); a.href = URL.createObjectURL(await file.getFile()); a.download = name; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 60_000); } };
  };
  return file;
}
const bytesOf = (b64) => { const raw = atob(b64 ?? ''), out = new Uint8Array(raw.length); for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i); return out; };
// The driver's files and folders (`{ name, bytes }`, `{ name, files: [{ path, bytes }] }`) as handles.
const fromDriver = (list) => (list ?? []).map((f) => f.files ? memoryFolder(f.name, f.files.map((c) => ({ path: c.path, bytes: bytesOf(c.bytes) }))) : memoryFile(f.name, bytesOf(f.bytes)));

// What a drop or a paste hands over without the File System Access API: a
// File read-only, and a dropped folder's `FileSystemDirectoryEntry`
// (`webkitGetAsEntry`, every engine) as a read-only folder handle.
const copied = (file) => ({ kind: 'file', name: file.name, async getFile() { return file; }, async queryPermission() { return 'granted'; } });
function entryHandle(entry) {
  if (!entry.isDirectory) return { kind: 'file', name: entry.name, getFile: () => new Promise((ok, no) => entry.file(ok, no)), async queryPermission() { return 'granted'; } };
  const list = async () => { const reader = entry.createReader(), out = []; for (;;) { const batch = await new Promise((ok, no) => reader.readEntries(ok, no)); if (!batch.length) return out; out.push(...batch); } };
  const child = async (n, directory) => {
    const found = (await list()).find((e) => e.name === n);
    if (!found) throw new DOMException(n, 'NotFoundError');
    if (found.isDirectory !== directory) throw new DOMException(n, 'TypeMismatchError');
    return entryHandle(found);
  };
  return { kind: 'directory', name: entry.name,
    async getFileHandle(n, { create = false } = {}) { if (create) throw new DOMException(n, 'NoModificationAllowedError'); return child(n, false); },
    async getDirectoryHandle(n, { create = false } = {}) { if (create) throw new DOMException(n, 'NoModificationAllowedError'); return child(n, true); },
    async *entries() { for (const e of await list()) yield [e.name, entryHandle(e)]; },
    async queryPermission() { return 'granted'; } };
}

// File drags over the elements that hear `dragenter` or `dragleave`, for
// both targets: the chain of those holding the element under the drag,
// kept from the document's own events, so moving between an element's own
// descendants is no leave (what T3 Code's `currentTarget.contains(
// relatedTarget)` test does, without relying on `relatedTarget`, which
// Safari leaves null). HTML fires the new target's dragenter before the old
// one's dragleave (HTML §6.11.5), so a dragleave at the element last entered
// is the drag leaving the page or cancelled. A drop ends the drag with no
// dragleave, as the DOM's does: the drop's own action clears what the enter
// drew. Each hearing element keeps its handlers itself (`exactDrags`), so
// one gone from the page is forgotten with it.
let over = [], last = null, at = null, tracking = false;
const fileDrag = (ev) => ev.dataTransfer?.types?.includes('Files');
function retarget(target) {
  const now = []; // the innermost first, as the DOM's events bubble
  for (let el = target; el; el = el.parentNode) if (el.exactDrags?.allowed()) now.push(el);
  const was = over;
  over = now;
  for (const el of now) if (!was.includes(el)) el.exactDrags.kinds.get('dragenter')?.(at);
  for (const el of was) if (!now.includes(el) && el.isConnected) el.exactDrags?.kinds.get('dragleave')?.(at);
}
function hearDrags(el, kind, allowed, fire) {
  (el.exactDrags ??= { kinds: new Map(), allowed }).kinds.set(kind, fire);
  if (tracking) return;
  tracking = true;
  document.addEventListener('dragenter', (ev) => { if (fileDrag(ev)) { at = ev; last = ev.target; retarget(ev.target); } }, true);
  document.addEventListener('dragover', (ev) => { if (fileDrag(ev)) at = ev; }, true);
  document.addEventListener('dragleave', (ev) => { if (fileDrag(ev) && ev.target === last) { at = ev; last = null; retarget(null); } }, true);
  document.addEventListener('drop', (ev) => { if (fileDrag(ev)) { last = null; over = []; } }, true);
}

// The manifest's `file_handlers`, the only types a picker offers (D2).
let declared;
function types() {
  return declared ??= fetch(new URL('./manifest.json', import.meta.url)).then((r) => r.json()).then((m) =>
    (m.file_handlers ?? []).filter((h) => !Object.keys(h.accept ?? {}).includes('inode/directory'))
      .map((h) => ({ description: h.name, accept: h.accept }))).catch(() => []);
}

// Whether a dropped file is of a type the manifest declares, which a drop
// keeps as a document chosen (D3); any other drop, a folder's included, is
// content for the session, as on a Mac.
async function declaredFile(handle) {
  if (handle?.kind !== 'file') return false;
  const ext = /\.[^.]+$/.exec(handle.name)?.[0].toLowerCase();
  return !!ext && (await types()).some((t) => Object.values(t.accept ?? {}).flat().some((e) => e.toLowerCase() === ext));
}

globalThis.exact.documents = {
  mint, run, files, copyFile, observe, hearDrags, keeping, recent, forget, permission,
  /** `listen` hears the kept list change (`storage.fs.watch('doc:/')`, D5). */
  onList(listen) { listeners.add(listen); },
  /** A paste's files (`clipboardData.files`), each a read-only copy minted
   * at once, so the action hears them in the paste's own event and its
   * `preventDefault()` still cancels the field's paste; a pasted image with
   * no file is the browser's own File (`image.png`). The driver's stand in.
   * None while a keeping page still reads its kept entries back, and none
   * past the numbers it has reserved. */
  pasted(dt, driver) { if (kept && !kept.loaded) return []; return (driver ? fromDriver(driver).map(mint) : [...(dt?.files ?? [])].map((f) => mint(copied(f)))).filter(Boolean); },
  install(host) {
    // host: { dispatch(id, kind, payload), generation() (none on the JS target), log(line), openFile(value) }
    // A view id names a view only in the generation that asked: an answer that
    // outlives a restart, which reuses ids, is dropped. `at(g)` is these
    // operations for generation `g`, the one the glue asked in (the module may
    // load after a restart); each takes its `since()` before its first await.
    let pinned;
    const since = () => { const g = pinned ?? host.generation?.(); return (id, kind, payload) => { if (host.generation?.() === g) host.dispatch(id, kind, payload); }; };
    // The files an installed app was launched with (`file_handlers`,
    // `launchQueue`, LLP 1069.010 slice 4), each opened as a document.
    globalThis.launchQueue?.setConsumer(async (params) => {
      for (const handle of params.files ?? []) {
        if (!host.openFile(await choose(handle))) host.log('open-file: refused: this app has no `open-file` field');
      }
    });
    // At once on a page that keeps nothing; a keeping page writes each choice down first.
    const chosen = (dispatch, view, name, found) => {
      if (!found.length) { host.log(`${name}: cancelled`); return dispatch(view, 27, ''); }
      host.log(`${name}: chosen`);
      if (!kept) return dispatch(view, 1, found.map(mint).join('\n'));
      return Promise.all(found.map(choose)).then((paths) => dispatch(view, 1, paths.join('\n')));
    };
    const api = {
      /** Files and folders dropped on the page (studio diary R19), of any
       * type (the any-file waiver, rules/DEFERRED.md): each handle the
       * browser gave, else a folder's entry, else its File, read-only. */
      async dropped(handles, files, entries = []) {
        await kept?.ready;
        return Promise.all(files.map(async (file, i) => kept?.loaded && handles[i] && await declaredFile(handles[i]) ? choose(handles[i])
          : numbered(handles[i] ?? (entries[i] ? entryHandle(entries[i]) : copied(file)))));
      },
      /** A picker command's ruling `r`: call the browser's picker (the
       * press's activation is still live), then report what was chosen. */
      async show(r, name) {
        const dispatch = since();
        if (!r.present) return dispatch(r.view, 27, '');
        if (name === 'showSaveFilePicker' && typeof globalThis.showSaveFilePicker !== 'function') return chosen(dispatch, r.view, name, [downloadFile(r.suggestedName)]);
        const options = name === 'showSaveFilePicker' ? { suggestedName: r.suggestedName } : name === 'showOpenFilePicker' ? { multiple: r.multiple } : {};
        if (name !== 'showDirectoryPicker') { const t = await types(); if (t.length) options.types = t; }
        try { const found = await globalThis[name](options); await chosen(dispatch, r.view, name, [found].flat()); }
        catch (e) { if (e?.name === 'AbortError') chosen(dispatch, r.view, name, []); else { host.log(`${name}: refused: ${e?.name ?? e}`); dispatch(r.view, 27, ''); } }
      },
      /** The agent's answer to a held picker: the driver's files as handles. */
      answer(r, name, files) {
        const dispatch = since();
        if (r.answered === 'cancel') return chosen(dispatch, r.node, name, []);
        return chosen(dispatch, r.node, name, fromDriver(files));
      },
    };
    return { ...api, at: g => pinnedTo(api, g, v => { pinned = v; }) };
  },
};

function pinnedTo(api, g, pin) {
  return Object.fromEntries(Object.entries(api).map(([name, f]) => [name, (...args) => { pin(g); try { return f(...args); } finally { pin(undefined); } }]));
}
