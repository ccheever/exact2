// @ref LLP 1069.010 D1, D2 — documents in place on the web. What the person
// chooses is a FileSystemHandle the page keeps; the app holds only its
// `doc:/<n>/<name>` path and reaches the bytes through `storage.fs` under
// `fs.read doc:/` / `fs.write doc:/` (storage-request.js calls `run`). The
// File System Access API's pickers are the commands of the same names; a
// browser without them (Safari, Firefox) refuses and fires `cancel` (ruled:
// no fallback to a hidden input's copy). Under the agent the driver's files
// arrive as bytes and become handles that behave as the browser's do.
const handles = new Map(); // n → FileSystemHandle
let next = 0;

function mint(handle) {
  for (const [n, h] of handles) if (h === handle) return `doc:/${n}/${handle.name}`;
  handles.set(++next, handle);
  return `doc:/${next}/${handle.name}`;
}

// The handle a `doc:` path leads to, or `{root}` for the handle's own
// directory; `.`, `..` and empty segments are refused.
async function resolve(path) {
  const m = /^doc:\/(\d+)(?:\/(.*))?$/.exec(path), handle = m && handles.get(Number(m[1]));
  if (!handle) throw new Error(`${path}: no such document (it was never opened, or its page closed)`);
  const rest = (m[2] ?? '').replace(/\/$/, ''), parts = rest === '' ? [] : rest.split('/');
  if (parts.some((p) => p === '' || p === '.' || p === '..')) throw new Error(`${path}: a document path has no \`.\`, \`..\` or empty segment`);
  if (!parts.length) return { root: handle.name };
  if (parts[0] !== handle.name) throw new Error(`${path}: no such document`);
  let at = handle;
  for (const [i, part] of parts.slice(1).entries()) {
    if (at.kind !== 'directory') throw new Error(`${path}: no such document`);
    const last = i === parts.length - 2;
    at = last ? await at.getFileHandle(part).catch(() => at.getDirectoryHandle(part)) : await at.getDirectoryHandle(part);
  }
  return { handle: at };
}

// Whether a scope's `<capability> <prefix>` lines cover the path, a
// component at a time (as `PathPrefix::covers` reads it).
function covered(scope, capability, path) {
  const parts = (p) => { const [ns, rest] = p.includes(':/') ? p.split(/:\/(.*)/s) : ['', p]; return [ns, ...rest.split('/').filter(Boolean)]; };
  const target = parts(path);
  return scope.split('\n').map((l) => l.trim().split(/\s+/)).some(([cap, prefix]) => cap === capability && prefix && parts(prefix).every((c, i) => target[i] === c));
}

function base64(bytes) { let text = ''; for (let i = 0; i < bytes.length; i += 16384) text += String.fromCharCode(...bytes.subarray(i, i + 16384)); return btoa(text); }

/** One storage operation on a `doc:` path under `scope` (the app's grants). */
async function run(op, args, bytes, scope) {
  const path = args.path, write = ['fs.writeFile', 'fs.atomicWriteFile', 'fs.appendFile', 'fs.mkdir', 'fs.rm'].includes(op);
  const grant = write ? 'fs.write' : 'fs.read';
  if (!covered(scope, grant, path)) throw new Error(`${op} ${path}: not granted (needs \`${grant} doc:/\`)`);
  const found = await resolve(path);
  if (found.root !== undefined) {
    if (op === 'fs.readdir') return [found.root];
    if (op === 'fs.stat') return { size: 0, isFile: false, isDirectory: true, modifiedMs: 0 };
    throw new Error(`${op} ${path}: a handle's own directory is read-only`);
  }
  const h = found.handle;
  switch (op) {
    case 'fs.readFile': return { base64: base64(new Uint8Array(await (await h.getFile()).arrayBuffer())) };
    case 'fs.stat': {
      if (h.kind === 'directory') return { size: 0, isFile: false, isDirectory: true, modifiedMs: 0 };
      const file = await h.getFile();
      return { size: file.size, isFile: true, isDirectory: false, modifiedMs: file.lastModified };
    }
    case 'fs.readdir': { const names = []; for await (const [name] of h.entries()) names.push(name); return names.sort(); }
    case 'fs.writeFile': case 'fs.atomicWriteFile': {
      // A writable commits on close, which is what makes it atomic here.
      if (h.requestPermission && await h.queryPermission?.({ mode: 'readwrite' }) !== 'granted' && await h.requestPermission({ mode: 'readwrite' }) !== 'granted') throw new Error(`${op} ${path}: not permitted by the browser`);
      const w = await h.createWritable(); await w.write(bytes); await w.close(); return null;
    }
    default: throw new Error(`${op} is not available on a document`);
  }
}

// The driver's files as handles the page keeps (LLP 1069.007 D3): a file's
// bytes, a folder's tree, or a name to save to.
function memoryFile(name, data = new Uint8Array()) {
  return { kind: 'file', name, async getFile() { return new File([data], name); },
    async createWritable() { const parts = []; return { async write(d) { parts.push(d); }, async close() { data = new Uint8Array(await new Blob(parts).arrayBuffer()); } }; },
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
  return { kind: 'directory', name,
    async getFileHandle(n) { const c = children.get(n); if (c?.kind !== 'file') throw new DOMException(n, 'NotFoundError'); return c; },
    async getDirectoryHandle(n) { const c = children.get(n); if (c?.kind !== 'directory') throw new DOMException(n, 'NotFoundError'); return c; },
    async *entries() { for (const e of children) yield e; } };
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
  mint, run,
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
