// @ref LLP 1069.010 D1, D2 — documents in place on the web. What the person
// chooses is a FileSystemHandle the page keeps; the app holds only its
// `doc:/<n>/<name>` path and reaches the bytes through `storage.fs` under
// `fs.read doc:/` / `fs.write doc:/` (storage-request.js calls `run`). The
// File System Access API's pickers are the commands of the same names; a
// browser without them (Safari, Firefox) refuses and fires `cancel` (ruled:
// no fallback to a hidden input's copy). Under the agent the driver's files
// arrive as bytes and become handles that behave as the browser's do.
import { coversPath } from './grant-admission.js';
const handles = new Map(); // n → FileSystemHandle
let next = 0;

function mint(handle) {
  for (const [n, h] of handles) if (h === handle) return `doc:/${n}/${handle.name}`;
  handles.set(++next, handle);
  return `doc:/${next}/${handle.name}`;
}

// One storage operation's failure, coded as every host codes it (kanban
// F28): the DOM's names for the filesystem's (NotFoundError is ENOENT).
const CODES = { NotFoundError: 'ENOENT', TypeMismatchError: 'ENOTDIR', InvalidModificationError: 'ENOTEMPTY' };
function failure(message, code = 'failed') { return Object.assign(new Error(message), { kind: 'Unavailable', code }); }
// A name of the wrong kind at the end of the path: what a native host's
// `read`, `write` or `create_dir_all` says of it.
const MISMATCH = { readFile: 'EISDIR', writeFile: 'EISDIR', atomicWriteFile: 'EISDIR', appendFile: 'EISDIR', mkdir: 'EEXIST' };
const failed = (op, path, e, last = false) => e?.kind ? e
  : failure(`fs.${op} ${path}: ${e?.message ?? e}`, (last && e?.name === 'TypeMismatchError' && MISMATCH[op]) || (CODES[e?.name] ?? 'failed'));

// Where a `doc:` path leads: `{root}` for the handle's own directory, else
// the handle it names (`entry`) or, for a name that may not exist yet, its
// folder and that name. `.`, `..` and empty segments are refused.
async function locate(op, path, create = false) {
  const m = /^doc:\/(\d+)(?:\/(.*))?$/.exec(path), handle = m && handles.get(Number(m[1]));
  if (!handle) throw failure(`${path}: no such document (it was never opened, or its page closed)`);
  const rest = (m[2] ?? '').replace(/\/$/, ''), parts = rest === '' ? [] : rest.split('/');
  if (parts.some((p) => p === '' || p === '.' || p === '..')) throw failure(`${path}: a document path has no \`.\`, \`..\` or empty segment`);
  if (!parts.length) return { root: handle.name, handle };
  if (parts[0] !== handle.name) throw failure(`${path}: no such document`);
  if (parts.length === 1) return { top: handle, entry: handle };
  let at = handle;
  try {
    for (const part of parts.slice(1, -1)) {
      if (at.kind !== 'directory') throw new DOMException(part, 'TypeMismatchError');
      at = await at.getDirectoryHandle(part, { create });
    }
    if (at.kind !== 'directory') throw new DOMException(parts.at(-1), 'TypeMismatchError');
  } catch (e) { throw failed(op, path, e); }
  return { top: handle, folder: at, name: parts.at(-1) };
}
const entry = (found) => found.entry ?? found.folder.getFileHandle(found.name).catch((e) => e?.name === 'TypeMismatchError' ? found.folder.getDirectoryHandle(found.name) : Promise.reject(e));
async function writable(op, path, top) {
  if (top.requestPermission && await top.queryPermission?.({ mode: 'readwrite' }) !== 'granted' && await top.requestPermission({ mode: 'readwrite' }) !== 'granted') throw failure(`fs.${op} ${path}: not permitted by the browser`);
}
const STAT_DIR = { size: 0, isFile: false, isDirectory: true, modifiedMs: 0 };

/** One `storage.fs` operation on a `doc:` path under `scope` (the app's
 * grants), with the semantics of ibex2's `run_document`, which a native
 * host runs for both languages: `readFile` an ArrayBuffer, `rm` one file
 * or empty folder, `mkdir` with its parents, `rename`, `copyFile` and
 * `realpath` refused. A Rust source reaches it through storage-request.js,
 * a TypeScript one through `files`. */
async function operate(op, path, bytes, scope) {
  if (['rename', 'copyFile', 'realpath'].includes(op)) throw failure(`fs.${op} is not available on a document; read its bytes and write them`);
  const write = ['writeFile', 'atomicWriteFile', 'appendFile', 'mkdir', 'rm'].includes(op), grant = write ? 'fs.write' : 'fs.read';
  if (!coversPath(scope, grant, path)) throw failure(`denied: fs.${op} ${path}: needs \`${grant} doc:/\``, 'denied');
  // `mkdir` makes the folders above too, as `create_dir_all` does.
  const found = await locate(op, path, op === 'mkdir');
  if (found.root !== undefined) {
    if (op === 'readdir') return [found.root];
    if (op === 'stat') return { ...STAT_DIR };
    throw failure(`fs.${op} ${path}: a handle's own directory is read-only`);
  }
  if (write) await writable(op, path, found.top);
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
  } catch (e) { throw failed(op, path, e, true); }
}

function base64(bytes) { let text = ''; for (let i = 0; i < bytes.length; i += 16384) text += String.fromCharCode(...bytes.subarray(i, i + 16384)); return btoa(text); }

/** A storage request's operation (storage-request.js, a Rust source's). */
async function run(op, args, bytes, scope) {
  const value = await operate(op.replace(/^fs\./, ''), args.path, bytes, scope);
  return value instanceof ArrayBuffer ? { base64: base64(new Uint8Array(value)) } : value;
}

/** `storage.fs` over documents for a TypeScript source under `scope`:
 * the methods of an app path, the bytes copied at the call. */
function files(scope) {
  const copy = (data) => ArrayBuffer.isView(data) ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice() : new Uint8Array(ArrayBuffer.prototype.slice.call(data, 0));
  return Object.freeze(Object.fromEntries(['readFile', 'writeFile', 'atomicWriteFile', 'appendFile', 'readdir', 'mkdir', 'rm', 'stat', 'rename', 'copyFile', 'realpath']
    .map((op) => [op, (path, data) => operate(op, String(path), ['writeFile', 'atomicWriteFile', 'appendFile'].includes(op) ? copy(data) : null, scope)])));
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
const bytesOf = (b64) => { const raw = atob(b64 ?? ''), out = new Uint8Array(raw.length); for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i); return out; };

// The manifest's `file_handlers`, the only types a picker offers (D2).
let declared;
function types() {
  return declared ??= fetch(new URL('./manifest.json', import.meta.url)).then((r) => r.json()).then((m) =>
    (m.file_handlers ?? []).filter((h) => !Object.keys(h.accept ?? {}).includes('inode/directory'))
      .map((h) => ({ description: h.name, accept: h.accept }))).catch(() => []);
}

globalThis.exact.documents = {
  mint, run, files,
  install(host) {
    // host: { dispatch(id, kind, payload), log(line), openFile(value) }
    // The files an installed app was launched with (`file_handlers`,
    // `launchQueue`, LLP 1069.010 slice 4), each opened as a document.
    globalThis.launchQueue?.setConsumer((params) => {
      for (const handle of params.files ?? []) {
        if (!host.openFile(mint(handle))) host.log('open-file: refused: this app has no `open-file` field');
      }
    });
    const chosen = (view, name, found) => {
      if (!found.length) { host.log(`${name}: cancelled`); return host.dispatch(view, 27, ''); }
      host.log(`${name}: chosen`);
      host.dispatch(view, 1, found.map(mint).join('\n'));
    };
    return {
      /** A picker command's ruling `r`: call the browser's picker (the
       * press's activation is still live), then report what was chosen. */
      async show(r, name) {
        if (!r.present) return host.dispatch(r.view, 27, '');
        const options = name === 'showSaveFilePicker' ? { suggestedName: r.suggestedName } : name === 'showOpenFilePicker' ? { multiple: r.multiple } : {};
        if (name !== 'showDirectoryPicker') { const t = await types(); if (t.length) options.types = t; }
        try { const found = await globalThis[name](options); chosen(r.view, name, [found].flat()); }
        catch (e) { if (e?.name === 'AbortError') chosen(r.view, name, []); else { host.log(`${name}: refused: ${e?.name ?? e}`); host.dispatch(r.view, 27, ''); } }
      },
      /** The agent's answer to a held picker: the driver's files as handles. */
      answer(r, name, files) {
        if (r.answered === 'cancel') return chosen(r.node, name, []);
        const made = (files ?? []).map((f) => f.files ? memoryFolder(f.name, f.files.map((c) => ({ path: c.path, bytes: bytesOf(c.bytes) }))) : memoryFile(f.name, bytesOf(f.bytes)));
        chosen(r.node, name, made);
      },
    };
  },
};
