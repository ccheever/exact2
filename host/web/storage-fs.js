// @ref LLP 1027 D10 — app-owned browser storage, installed after first pixel.
// Each operation is one IndexedDB transaction: readers never see partial writes,
// and directory moves/removals cannot race another tab's filesystem operation.
// Modification times come from the platform's clock, captured when this
// module evaluates: a realm that later refuses ambient time to app code (the
// module Worker, LLP 1027.002 D2) still stamps files with the real one.
const now = Date.now.bind(Date);
const roots = Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' });
const rootPaths = Object.values(roots);
// Calls from one host module retain invocation order. Web Locks still reject
// conflicts with a live SQLite handle or a mutation in another browser tab.
const mutationTails = new Map();

function failure(message, code) {
  const error = new Error(`filesystem: ${message}`);
  error.kind = 'Unavailable';
  if (code) error.code = code;
  return error;
}

export function normalizePath(path) {
  if (typeof path !== 'string' || !path.startsWith('app:/')) {
    throw failure('browser storage needs an app:/ path');
  }
  const parts = path.slice(5).split('/').filter(Boolean);
  if (!['data', 'cache', 'tmp'].includes(parts[0])) throw failure('unknown app directory');
  if (parts.some(part => part === '.' || part === '..' || part.includes('\0'))) {
    throw failure('app path contains traversal or NUL');
  }
  return `app:/${parts.join('/')}`;
}

// Grants are the host-admitted declaration text, never module exports. Compare
// components (including namespace); app:/data does not grant app:/database.
export function authorize(grants, operation, path) {
  const normalized = normalizePath(path);
  const parts = normalized.slice(5).split('/');
  const lines = typeof grants === 'string' ? grants.split('\n') : [];
  for (const line of lines) {
    const [op, prefix, ...extra] = line.trim().split(/\s+/);
    if (op !== operation || extra.length || !prefix?.startsWith('app:/')) continue;
    const allowed = prefix.slice(5).split('/').filter(Boolean);
    if (allowed.some(part => part === '.' || part === '..' || part.includes('\0'))) continue;
    if (allowed.every((part, index) => parts[index] === part)) return normalized;
  }
  const error = new Error(`denied: ${operation}`);
  error.kind = 'Unavailable';
  throw error;
}

function parentOf(path) { return path.slice(0, path.lastIndexOf('/')); }
function below(path, directory) { return path.startsWith(`${directory}/`); }

// A database owns its file exclusively and pins each ancestor with a shared
// lock. Unrelated files remain usable; replacing an open database or moving its
// parent fails instead of silently discarding SQLite's later commit.
export async function lockPaths(appId, paths) {
  if (!globalThis.navigator?.locks) throw failure('Web Locks are unavailable');
  const requested = new Map();
  for (let path of paths) {
    path = normalizePath(path);
    requested.set(path, 'exclusive');
    while (!rootPaths.includes(path)) {
      path = parentOf(path);
      if (!requested.has(path)) requested.set(path, 'shared');
    }
  }
  const held = [];
  async function release() {
    for (const lock of held) lock.release();
    await Promise.all(held.map(lock => lock.done));
  }
  try {
    for (const [path, mode] of [...requested].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) {
      let unlock;
      const lifetime = new Promise(resolve => { unlock = resolve; });
      let acquired;
      let refused;
      const ready = new Promise((resolve, reject) => { acquired = resolve; refused = reject; });
      const done = navigator.locks.request(`exact-storage:${encodeURIComponent(appId)}:${path}`,
        { mode, ifAvailable: true }, async lock => {
          if (!lock) { refused(failure(`storage is busy: ${path}`, 'EBUSY')); return; }
          acquired();
          await lifetime;
        });
      void done.catch(error => refused(failure(error.message)));
      await ready;
      held.push({ release: unlock, done });
    }
    return release;
  } catch (error) {
    await release();
    throw error;
  }
}
function requireBelowRoot(path) {
  if (rootPaths.includes(path)) throw failure('operation needs a path below the app root');
}
function entry(records, path) {
  const value = records.get(path);
  if (!value) throw failure(`no such file or directory: ${path}`, 'ENOENT');
  return value;
}
function directory(records, path) {
  const value = entry(records, path);
  if (value.kind !== 'directory') throw failure(`not a directory: ${path}`, 'ENOTDIR');
  return value;
}
function file(records, path) {
  const value = entry(records, path);
  if (value.kind !== 'file') throw failure('operation needs a regular file', 'EISDIR');
  return value;
}
function bytes(data) {
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength).slice().buffer;
  }
  // The argument can belong to the app iframe, so instanceof is insufficient.
  try { return ArrayBuffer.prototype.slice.call(data, 0); }
  catch { throw failure('file data must be an ArrayBuffer or typed array'); }
}

/** Trusted host backend, also used by SQLite after sqlite.open admission.
 * Never expose this grant-free object to app code. No database opens at creation.
 */
export function createFileStore(appId) {
  if (typeof appId !== 'string' || !appId) throw failure('missing admitted app identity');
  let opening;
  let closed = false;
  function open() {
    if (closed) return Promise.reject(failure('storage was unloaded'));
    if (!opening) {
      opening = new Promise((resolve, reject) => {
        let request;
        try { request = indexedDB.open(`exact-storage:${encodeURIComponent(appId)}`, 1); }
        catch (error) { reject(failure(error.message)); return; }
        request.onupgradeneeded = () => {
          const store = request.result.createObjectStore('files', { keyPath: 'path' });
          for (const path of rootPaths) store.put({ path, kind: 'directory', modifiedMs: now() });
        };
        request.onerror = () => reject(failure(request.error?.message || 'IndexedDB open failed'));
        request.onblocked = () => reject(failure('IndexedDB upgrade blocked'));
        request.onsuccess = () => {
          const db = request.result;
          if (closed) { db.close(); reject(failure('storage was unloaded')); return; }
          db.onversionchange = () => db.close();
          resolve(db);
        };
      });
    }
    return opening;
  }
  async function run(write, operation, path, list = false) {
    const db = await open();
    if (closed) throw failure('storage was unloaded');
    return new Promise((resolve, reject) => {
      let transaction;
      try { transaction = db.transaction('files', write ? 'readwrite' : 'readonly'); }
      catch (error) { reject(failure(error.message)); return; }
      const store = transaction.objectStore('files');
      let result;
      let error;
      transaction.oncomplete = () => resolve(result);
      transaction.onabort = () => reject(error || failure(transaction.error?.message || 'transaction aborted'));
      transaction.onerror = () => {}; // onabort reports once, including quota failures.
      const request = path === undefined ? store.getAll() : store.get(path);
      // '/' sorts immediately before '0': the exclusive upper bound includes
      // every descendant name, even one containing U+FFFF, but no sibling path.
      const keys = list ? store.getAllKeys(IDBKeyRange.bound(`${path}/`, `${path}0`, false, true)) : null;
      let pending = keys ? 2 : 1;
      const ready = () => {
        if (--pending) return;
        const values = path === undefined ? request.result : request.result ? [request.result] : [];
        const records = new Map(values.map(value => [value.path, value]));
        const changes = {
          put(value) { records.set(value.path, value); store.put(value); },
          remove(path) { records.delete(path); store.delete(path); },
          touch(path) { const value = directory(records, path); this.put({ ...value, modifiedMs: now() }); },
        };
        try { result = operation(records, changes, keys?.result); }
        catch (cause) { error = cause; transaction.abort(); }
      };
      request.onsuccess = ready;
      if (keys) keys.onsuccess = ready;
    });
  }
  async function write(path, data, append) {
    path = normalizePath(path);
    requireBelowRoot(path);
    const copied = bytes(data); // Snapshot before yielding to the IndexedDB open.
    return run(true, (records, changes) => {
      directory(records, parentOf(path));
      const previous = records.get(path);
      if (previous && previous.kind !== 'file') throw failure('write needs a regular file', 'EISDIR');
      let contents = copied;
      if (append && previous) {
        const joined = new Uint8Array(previous.contents.byteLength + copied.byteLength);
        joined.set(new Uint8Array(previous.contents));
        joined.set(new Uint8Array(copied), previous.contents.byteLength);
        contents = joined.buffer;
      }
      changes.put({ path, kind: 'file', contents, modifiedMs: now() });
      if (!previous) changes.touch(parentOf(path));
    });
  }
  return Object.freeze({
    directories: roots,
    async readFile(path) {
      path = normalizePath(path);
      // IndexedDB already returns an independent structured clone for this read.
      return run(false, records => file(records, path).contents, path);
    },
    writeFile(path, data) { return write(path, data, false); },
    atomicWriteFile(path, data) { return write(path, data, false); },
    appendFile(path, data) { return write(path, data, true); },
    async readdir(path) {
      path = normalizePath(path);
      return run(false, (records, _changes, keys) => {
        directory(records, path);
        return keys.filter(key => parentOf(key) === path)
          .map(key => key.slice(path.length + 1)).sort();
      }, path, true);
    },
    async mkdir(path) {
      path = normalizePath(path);
      return run(true, (records, changes) => {
        const parts = path.slice(5).split('/');
        let current = `app:/${parts.shift()}`;
        for (const part of parts) {
          const parent = current;
          current += `/${part}`;
          if (records.has(current)) directory(records, current);
          else {
            changes.put({ path: current, kind: 'directory', modifiedMs: now() });
            changes.touch(parent);
          }
        }
      });
    },
    async rm(path) {
      path = normalizePath(path);
      requireBelowRoot(path);
      return run(true, (records, changes) => {
        directory(records, parentOf(path));
        if (!records.has(path)) return;
        for (const key of records.keys()) if (key === path || below(key, path)) changes.remove(key);
        changes.touch(parentOf(path));
      });
    },
    async stat(path) {
      path = normalizePath(path);
      return run(false, records => {
        const value = entry(records, path);
        return { size: value.kind === 'file' ? value.contents.byteLength : 0,
          isFile: value.kind === 'file', isDirectory: value.kind === 'directory', modifiedMs: value.modifiedMs };
      }, path);
    },
    async rename(from, to) {
      from = normalizePath(from);
      to = normalizePath(to);
      requireBelowRoot(from);
      requireBelowRoot(to);
      return run(true, (records, changes) => {
        const source = entry(records, from);
        directory(records, parentOf(to));
        if (from === to) return;
        if (below(to, from)) throw failure('cannot move a directory into itself');
        const target = records.get(to);
        if (target) {
          if (source.kind !== target.kind) throw failure('rename requires matching file kinds');
          if (target.kind === 'directory' && [...records.keys()].some(key => below(key, to))) {
            throw failure('destination directory is not empty', 'ENOTEMPTY');
          }
          changes.remove(to);
        }
        const moving = [...records.values()].filter(value => value.path === from || below(value.path, from));
        for (const value of moving) changes.remove(value.path);
        for (const value of moving) changes.put({ ...value, path: to + value.path.slice(from.length) });
        changes.touch(parentOf(from));
        changes.touch(parentOf(to));
      });
    },
    async copyFile(from, to) {
      from = normalizePath(from);
      to = normalizePath(to);
      requireBelowRoot(to);
      return run(true, (records, changes) => {
        const source = file(records, from);
        directory(records, parentOf(to));
        if (from === to) throw failure('copy requires distinct regular files');
        if (records.has(to)) file(records, to);
        changes.put({ path: to, kind: 'file', contents: source.contents.slice(0), modifiedMs: now() });
        changes.touch(parentOf(to));
      });
    },
    async realpath(path) {
      path = normalizePath(path);
      return run(false, records => { entry(records, path); return path; }, path);
    },
    close() {
      closed = true;
      if (opening) void opening.then(db => db.close(), () => {});
    },
  });
}

/** Public capability. Every operation is admitted before opening IndexedDB. */
export function createFileSystem(appId, grants) {
  const store = createFileStore(appId);
  const fs = { directories: roots, dispose() { store.close(); } };
  function mutate(paths, operation) {
    const previous = mutationTails.get(appId) || Promise.resolve();
    const result = previous.then(async () => {
      const release = await lockPaths(appId, paths);
      try { return await operation(); }
      finally { await release(); }
    });
    const settled = result.then(() => {}, () => {});
    mutationTails.set(appId, settled);
    void settled.then(() => {
      if (mutationTails.get(appId) === settled) mutationTails.delete(appId);
    });
    return result;
  }
  for (const method of ['readFile', 'readdir', 'stat', 'realpath']) {
    fs[method] = async path => store[method](authorize(grants, 'fs.read', path));
  }
  for (const method of ['writeFile', 'atomicWriteFile', 'appendFile', 'mkdir', 'rm']) {
    fs[method] = async (path, data) => {
      path = authorize(grants, 'fs.write', path);
      // Preserve call-time bytes even when lock acquisition crosses a task.
      if (['writeFile', 'atomicWriteFile', 'appendFile'].includes(method)) data = bytes(data);
      return mutate([path], () => store[method](path, data));
    };
  }
  for (const method of ['rename', 'copyFile']) {
    fs[method] = async (from, to) => {
      from = authorize(grants, 'fs.read', from);
      to = authorize(grants, 'fs.write', to);
      if (method === 'rename') authorize(grants, 'fs.write', from);
      return mutate(method === 'rename' ? [from, to] : [to], () => store[method](from, to));
    };
  }
  return Object.freeze(fs);
}
