// @ref LLP 1027 D10 — app-owned browser storage, installed after first pixel.
// Each operation is one IndexedDB transaction: readers never see partial writes,
// and directory moves/removals cannot race another tab's filesystem operation.
// Modification times come from the platform's clock, captured when this
// module evaluates: a realm that later refuses ambient time to app code (the
// module Worker, LLP 1027.002 D2) still stamps files with the real one.
import { directories as roots, now } from './storage-environment.js';
import { coversPath } from './grant-admission.js';
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
  if (coversPath(grants, operation, normalized)) return normalized;
  const error = new Error(deniedPath(operation, normalized));
  error.kind = 'Unavailable';
  throw error;
}

/** A path refusal names the path and the grant lines that would admit it (app farm round 2: a bare
 * `denied: sqlite.open` left builds guessing which per-persona file it wanted). As js/src/prelude.js says it natively. */
export function deniedPath(operation, path) {
  const dir = path.slice(0, path.lastIndexOf('/'));
  const directory = /^[a-z]+:\/[^/]/.test(dir) ? `, or \`${operation} ${dir}\` for every file there` : '';
  return `denied: ${operation} ${path}: no grant covers it; grant \`${operation} ${path}\`${directory} (a grant covers its path and what is below it, by whole names)`;
}

function parentOf(path) { return path.slice(0, path.lastIndexOf('/')); }
function below(path, directory) { return path.startsWith(`${directory}/`); }
function descendants(path) {
  // '/' sorts immediately before '0': include all descendant names (even
  // U+FFFF) but no sibling path. The directory itself is a separate point read.
  return IDBKeyRange.bound(`${path}/`, `${path}0`, false, true);
}

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
  // The applied marker a mutation's own transaction writes (`unloadJournal`): taken here,
  // before the first await, from the one mutation `mutate` lets run at a time.
  let pendingMark = null;
  async function run(write, operation, path, list, subtree) {
    const mark = write ? pendingMark : null;
    pendingMark = null;
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
      const requests = [...new Set(Array.isArray(path) ? path : [path])].map(key => store.get(key));
      if (subtree) requests.push(store.getAll(descendants(subtree)));
      const keys = list ? [store.getKey(list), store.getAllKeys(descendants(list))] : [];
      // The session's applied set, read and written in this transaction (`unloadJournal`).
      const applied = mark ? store.get(APPLIED + mark.session) : null;
      let pending = requests.length + keys.length + (applied ? 1 : 0);
      const ready = () => {
        if (--pending) return;
        const values = requests.flatMap(request => request.result ?? []);
        const records = new Map(values.map(value => [value.path, value]));
        const changes = {
          put(value) { records.set(value.path, value); store.put(value); },
          remove(path) { records.delete(path); store.delete(path); },
          touch(path) { const value = directory(records, path); this.put({ ...value, modifiedMs: now() }); },
        };
        try {
          result = operation(records, changes, keys.flatMap(request => request.result ?? []));
          if (mark) store.put({ path: APPLIED + mark.session, kind: 'marker', session: mark.session,
            applied: [...(applied.result?.applied ?? []).slice(1 - APPLIED_KEPT), mark.seq] });
        }
        catch (cause) { error = cause; transaction.abort(); }
      };
      for (const request of [...requests, ...keys, ...(applied ? [applied] : [])]) request.onsuccess = ready;
    });
  }
  async function write(path, data, append, owned = false) {
    path = normalizePath(path);
    requireBelowRoot(path);
    // Snapshot before yielding. A private owned buffer is transferred, not copied.
    const copied = owned ? structuredClone(data, { transfer: [data] }) : bytes(data);
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
    }, [path, parentOf(path)]);
  }
  return Object.freeze({
    directories: roots,
    /** The next mutation's applied marker ({session, seq}), written in its transaction. */
    mark(value) { pendingMark = value; },
    /** A session's applied set ({session, applied: [seq…]}), or null; a failed read rejects. */
    async applied(session) {
      const db = await open();
      return new Promise((resolve, reject) => {
        const request = db.transaction('files', 'readonly').objectStore('files').get(APPLIED + session);
        request.onsuccess = () => resolve(request.result ?? null);
        request.onerror = () => reject(failure(request.error?.message || 'IndexedDB read failed'));
      });
    },
    /** Drops a replayed session's marker. */
    async forget(session) {
      const db = await open();
      return new Promise((resolve, reject) => {
        const transaction = db.transaction('files', 'readwrite');
        transaction.objectStore('files').delete(APPLIED + session);
        transaction.oncomplete = () => resolve();
        transaction.onabort = () => reject(failure(transaction.error?.message || 'transaction aborted'));
      });
    },
    async readFile(path) {
      path = normalizePath(path);
      // IndexedDB already returns an independent structured clone for this read.
      // A picked file's entry holds the browser's own File (LLP 1069.002 D5).
      const contents = await run(false, records => file(records, path).contents, path);
      return contents instanceof Blob ? contents.arrayBuffer() : contents;
    },
    // Trusted host only: a file as a Blob, for an `image` or `video` source
    // (LLP 1069.002 D7), and when it last changed. A picked entry is its File.
    async blob(path, type, maxBytes = Infinity) {
      path = normalizePath(path);
      const { contents, modifiedMs } = await run(false, records => file(records, path), path);
      const size = contents.byteLength ?? contents.size;
      if (size > maxBytes) throw failure(`compressImage: too-large: ${size} bytes is over ${maxBytes}`, 'too-large');
      return { blob: contents instanceof Blob ? contents : new Blob([contents], { type }), modifiedMs };
    },
    // Trusted host only: a picked file's entry, backed by the browser's File
    // rather than bytes read into memory (LLP 1069.002 D5).
    putBlob(path, blob) {
      path = normalizePath(path);
      requireBelowRoot(path);
      return run(true, (records, changes) => {
        directory(records, parentOf(path));
        const previous = records.get(path);
        if (previous && previous.kind !== 'file') throw failure('write needs a regular file', 'EISDIR');
        changes.put({ path, kind: 'file', contents: blob, modifiedMs: now() });
        if (!previous) changes.touch(parentOf(path));
      }, [path, parentOf(path)]);
    },
    writeFile(path, data) { return write(path, data, false); },
    atomicWriteFile(path, data) { return write(path, data, false); },
    // Trusted host only: takes an ArrayBuffer and detaches it from its caller.
    atomicWriteOwnedFile(path, data) { return write(path, data, false, true); },
    appendFile(path, data) { return write(path, data, true); },
    async readdir(path) {
      path = normalizePath(path);
      return run(false, (records, _changes, keys) => {
        directory(records, path);
        return keys.filter(key => parentOf(key) === path)
          .map(key => key.slice(path.length + 1)).sort();
      }, path, path);
    },
    async mkdir(path) {
      path = normalizePath(path);
      const ancestors = [path];
      while (!rootPaths.includes(ancestors[ancestors.length - 1])) {
        ancestors.push(parentOf(ancestors[ancestors.length - 1]));
      }
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
      }, ancestors);
    },
    async rm(path) {
      path = normalizePath(path);
      requireBelowRoot(path);
      return run(true, (records, changes, keys) => {
        directory(records, parentOf(path));
        if (!keys.includes(path)) return;
        for (const key of keys) changes.remove(key);
        changes.touch(parentOf(path));
      }, parentOf(path), path);
    },
    async stat(path) {
      path = normalizePath(path);
      return run(false, records => {
        const value = entry(records, path);
        return { size: value.kind === 'file' ? value.contents.byteLength ?? value.contents.size : 0,
          isFile: value.kind === 'file', isDirectory: value.kind === 'directory', modifiedMs: value.modifiedMs };
      }, path);
    },
    async rename(from, to) {
      from = normalizePath(from);
      to = normalizePath(to);
      requireBelowRoot(from);
      requireBelowRoot(to);
      return run(true, (records, changes, keys) => {
        const source = entry(records, from);
        directory(records, parentOf(to));
        if (from === to) return;
        if (below(to, from)) throw failure('cannot move a directory into itself');
        const target = records.get(to);
        if (target) {
          if (source.kind !== target.kind) throw failure('rename requires matching file kinds');
          if (target.kind === 'directory' && keys.some(key => below(key, to))) {
            throw failure('destination directory is not empty', 'ENOTEMPTY');
          }
          changes.remove(to);
        }
        const moving = [...records.values()].filter(value => value.path === from || below(value.path, from));
        for (const value of moving) changes.remove(value.path);
        for (const value of moving) changes.put({ ...value, path: to + value.path.slice(from.length) });
        changes.touch(parentOf(from));
        changes.touch(parentOf(to));
      }, [from, to, parentOf(from), parentOf(to)], to, from);
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
      }, [from, to, parentOf(to)]);
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

// @ref LLP 1097 D10 — a write in flight when the tab closes. A mutation is one IndexedDB
// transaction, and the browser drops a transaction still running when the page goes away.
// Each realm's filesystem keeps a journal of its own (a session): a mutation is an entry
// from the moment it is accepted (by this module, or by the JS target's storage queue
// above it: `admit`) until it commits, and its transaction adds its sequence number to the
// session's applied set. While the page is hidden, and from `pagehide`, the session's
// entries are in localStorage under its own key, rewritten synchronously as entries come
// and go. The session holds a Web Lock for its realm's life. A later realm, holding the
// app's recovery lock (so recoveries run one at a time and a new page waits for one in
// progress), replays the entries of the sessions whose lock it can take, interleaved by
// the time each was accepted, skipping any its session applied: so a replay cut short
// resumes and an append lands once. A window realm only: a worker has neither event nor
// synchronous storage, and a killed browser fires no event.
const APPLIED = '\u0000exact-applied:';
const APPLIED_KEPT = 1024;
const journals = new Map();
const MAX_JOURNAL = 2_000_000;
const journalPrefix = appId => `exact-storage-journal:${appId}:`;
const lockName = (appId, session) => `exact-storage-session:${encodeURIComponent(appId)}:${session}`;
const recoveryLock = appId => `exact-storage-recovery:${encodeURIComponent(appId)}`;
// A replayed mutation that fails this way never will: it is reported and skipped. Any other
// failure (busy, quota, an unloaded store) stops the recovery and keeps the rest for later.
const TERMINAL = new Set(['ENOENT', 'EISDIR', 'ENOTDIR', 'EEXIST', 'ENOTEMPTY']);
const terminal = error => TERMINAL.has(error?.code) || /^denied/.test(error?.message ?? '');
function windowRealm() {
  try { return typeof document !== 'undefined' && typeof addEventListener === 'function' && !!globalThis.localStorage && !!globalThis.navigator?.locks; }
  catch { return false; }
}
const isBuffer = value => Object.prototype.toString.call(value) === '[object ArrayBuffer]';
function encodeArg(value) {
  if (isBuffer(value) || ArrayBuffer.isView(value)) {
    const view = isBuffer(value) ? new Uint8Array(value) : new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
    let binary = '';
    for (let i = 0; i < view.length; i += 0x8000) binary += String.fromCharCode(...view.subarray(i, i + 0x8000));
    return { bytes: btoa(binary) };
  }
  return value;
}
function decodeArg(value) {
  if (value && typeof value === 'object' && typeof value.bytes === 'string') {
    const binary = atob(value.bytes);
    const out = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
    return out.buffer;
  }
  return value;
}
function mutationPaths(method, args) {
  return method === 'rename' ? [args[0], args[1]] : method === 'copyFile' ? [args[1]] : [args[0]];
}
// The grants a mutation needs, checked when it is accepted and again when it is replayed.
function authorizeMutation(grants, method, args) {
  if (method === 'rename' || method === 'copyFile') {
    const from = authorize(grants, 'fs.read', args[0]), to = authorize(grants, 'fs.write', args[1]);
    if (method === 'rename') authorize(grants, 'fs.write', from);
    return [from, to, ...args.slice(2)];
  }
  return [authorize(grants, 'fs.write', args[0]), ...args.slice(1)];
}
function readJournal(key) {
  try { return JSON.parse(localStorage.getItem(key) ?? '{}').entries ?? []; } catch { return []; }
}
function writeJournal(key, entries) {
  if (!entries.length) { localStorage.removeItem(key); return; }
  // The longest prefix that fits: replay keeps order, and what does not fit is the lost suffix.
  let count = entries.length, text = JSON.stringify({ entries });
  while (text.length > MAX_JOURNAL && count > 1) {
    count = Math.floor(count / 2);
    text = JSON.stringify({ entries: entries.slice(0, count) });
  }
  if (text.length > MAX_JOURNAL) throw new Error(`one entry is ${text.length} characters`);
  localStorage.setItem(key, text);
}
function unloadJournal(appId) {
  if (!windowRealm()) return null;
  if (journals.has(appId)) return journals.get(appId);
  const random = new Uint32Array(2);
  globalThis.crypto?.getRandomValues?.(random);
  const session = `${random[0].toString(36)}${random[1].toString(36)}${now().toString(36)}`;
  const key = journalPrefix(appId) + session;
  let seq = 0, written = false, warned = false, gone = false;
  const pending = new Map();
  // Held for this realm's life: while it is, no other realm replays this session.
  navigator.locks.request(lockName(appId, session), () => new Promise(() => {})).catch(() => {});
  function write() {
    const entries = [...pending].map(([n, e]) => ({ seq: n, at: e.at, method: e.method, args: e.args }));
    try { writeJournal(key, entries); written = entries.length > 0; }
    catch (error) {
      if (!warned) { warned = true; console.warn(`storage: could not journal ${entries.length} pending write(s) for a closed tab: ${error.message}`); }
    }
  }
  const publish = () => gone || written || document.visibilityState === 'hidden';
  const journal = {
    session,
    add(method, args) {
      const n = ++seq;
      pending.set(n, { at: now(), method, args: args.map(encodeArg) });
      if (publish()) write();
      return n;
    },
    // A mutation that committed, or failed while its page was still there, leaves the
    // journal; one that failed once the page is going (its transaction dropped) stays.
    settle(n, ok = true) {
      if (n === undefined || (!ok && gone) || !pending.delete(n)) return;
      if (publish()) write();
    },
    flush() { if (pending.size || written) write(); },
    leave() { gone = true; journal.flush(); },
    // Every entry other sessions left, each with its session, in the order they were accepted.
    left() {
      const entries = [];
      for (let i = 0; i < localStorage.length; i++) {
        const k = localStorage.key(i);
        if (!k?.startsWith(journalPrefix(appId)) || k === key) continue;
        const from = k.slice(journalPrefix(appId).length);
        for (const entry of readJournal(k)) entries.push({ ...entry, session: from, key: k });
      }
      return entries.sort((a, b) => a.at - b.at || (a.session < b.session ? -1 : a.session > b.session ? 1 : a.seq - b.seq));
    },
  };
  journals.set(appId, journal);
  if (!windowRealm.hooked) {
    windowRealm.hooked = true;
    addEventListener('pagehide', () => { for (const j of journals.values()) j.leave(); });
    addEventListener('visibilitychange', () => { if (document.visibilityState === 'hidden') for (const j of journals.values()) j.flush(); });
  }
  return journal;
}
// Under the app's recovery lock: the gone sessions (their lock is free) and their entries,
// replayed in acceptance order, each skipped if its session applied it. Resolves to the
// failure lines (D8). A failure that is not terminal stops the recovery; what is left stays.
async function recover(appId, store, journal, runOne) {
  return navigator.locks.request(recoveryLock(appId), async () => {
    const lines = [];
    const releases = [];
    try {
      // Take each gone session's lock for the length of the recovery.
      const gone = new Set();
      for (const entry of journal.left()) {
        if (gone.has(entry.session) || releases.some(r => r.session === entry.session)) continue;
        let release;
        const held = new Promise(resolve => { release = resolve; });
        const taken = await new Promise(resolve => {
          navigator.locks.request(lockName(appId, entry.session), { ifAvailable: true }, lock => {
            resolve(!!lock);
            return lock ? held : undefined;
          }).catch(() => resolve(false));
        });
        releases.push({ session: entry.session, release });
        if (taken) gone.add(entry.session);
      }
      const entries = journal.left().filter(e => gone.has(e.session));
      const applied = new Map();
      for (const session of gone) applied.set(session, new Set((await store.applied(session))?.applied ?? []));
      const done = new Map([...gone].map(session => [session, new Set()]));
      let stopped = false;
      for (const entry of entries) {
        if (applied.get(entry.session).has(entry.seq)) { done.get(entry.session).add(entry.seq); continue; }
        let error = null;
        for (let attempt = 0; attempt < 5; attempt++) {
          try { await runOne(entry.method, entry.args.map(decodeArg), { session: entry.session, seq: entry.seq }); error = null; break; }
          catch (cause) {
            error = cause;
            if (cause?.code !== 'EBUSY') break;
            await new Promise(resolve => setTimeout(resolve, 50 * (attempt + 1)));
          }
        }
        if (error && !terminal(error)) {
          const kept = entries.length - [...done.values()].reduce((n, set) => n + set.size, 0);
          lines.push(`replay of ${entry.method} ${entry.args[0]} from a closed tab stopped (${kept} kept for the next launch): ${error.code ?? 'failed'} ${error.message}`);
          stopped = true;
          break;
        }
        if (error) lines.push(`replay of ${entry.method} ${entry.args[0]} from a closed tab: ${error.code ?? 'failed'} ${error.message}`);
        done.get(entry.session).add(entry.seq);
      }
      for (const session of gone) {
        const k = journalPrefix(appId) + session;
        const rest = readJournal(k).filter(e => !done.get(session).has(e.seq));
        writeJournal(k, rest);
        if (!rest.length && !stopped) await store.forget(session).catch(() => {});
      }
      return lines;
    } finally {
      for (const { release } of releases) release?.();
    }
  });
}

/** Public capability. Every operation is admitted before opening IndexedDB. */
export function createFileSystem(appId, grants) {
  const store = createFileStore(appId);
  const fs = { directories: roots, dispose() { store.close(); } };
  const journal = unloadJournal(appId);
  function chain(step) {
    const previous = mutationTails.get(appId) || Promise.resolve();
    const result = previous.then(step);
    const settled = result.then(() => {}, () => {});
    mutationTails.set(appId, settled);
    void settled.then(() => {
      if (mutationTails.get(appId) === settled) mutationTails.delete(appId);
    });
    return result;
  }
  async function locked(paths, operation) {
    const release = await lockPaths(appId, paths);
    try { return await operation(); }
    finally { await release(); }
  }
  // Each mutation is journaled from its acceptance (here, or `admit` above) until it
  // commits; its transaction adds it to its session's applied set.
  let preset;
  function mutate(paths, method, args, operation) {
    const seq = preset !== undefined ? preset : journal?.add(method, args);
    preset = undefined;
    return chain(() => locked(paths, () => {
      if (seq !== undefined) store.mark({ session: journal.session, seq });
      return operation();
    })).then(value => { journal?.settle(seq, true); return value; }, error => { journal?.settle(seq, false); throw error; });
  }
  // @ref LLP 1097 D10 — what closed tabs left uncommitted runs first, in order, before any
  // read or mutation of this filesystem (`createFileStore`'s direct users do not wait).
  const replayed = journal ? chain(() => recover(appId, store, journal, (method, args, mark) => {
    const allowed = authorizeMutation(grants, method, args); // the grants of this launch
    return locked(mutationPaths(method, allowed), () => { store.mark(mark); return store[method](...allowed); });
  })) : null;
  const ready = () => replayed?.catch(() => {});
  // The failure lines of that replay, for the runtime's journal (D8).
  fs.recovery = replayed ? replayed.catch(error => [`replay from a closed tab failed: ${error.message}`]) : Promise.resolve([]);
  // A storage queue above this one (the JS target's, ts-data.js) journals a write when it
  // accepts it, not when it reaches here, so one queued behind others is not lost.
  // Only a mutation its grants admit is journaled; one they refuse is refused when it runs.
  fs.admit = (method, args) => {
    if (!journal) return undefined;
    try { authorizeMutation(grants, method, args); } catch { return undefined; }
    return journal.add(method, args.map(a => typeof a === 'string' ? a : bytes(a)));
  };
  fs.release = seq => journal?.settle(seq, true);
  fs.withSeq = (seq, call) => {
    preset = seq;
    try { return call(); }
    finally { if (preset !== undefined) { preset = undefined; journal?.settle(seq, true); } }
  };
  for (const method of ['readFile', 'readdir', 'stat', 'realpath']) {
    fs[method] = async path => { const at = authorize(grants, 'fs.read', path); await ready(); return store[method](at); };
  }
  for (const method of ['writeFile', 'atomicWriteFile', 'appendFile', 'mkdir', 'rm']) {
    fs[method] = async (path, data) => {
      path = authorize(grants, 'fs.write', path);
      // Preserve call-time bytes even when lock acquisition crosses a task.
      if (['writeFile', 'atomicWriteFile', 'appendFile'].includes(method)) data = bytes(data);
      return mutate([path], method, [path, data], () => store[method](path, data));
    };
  }
  for (const method of ['rename', 'copyFile']) {
    fs[method] = async (from, to) => {
      from = authorize(grants, 'fs.read', from);
      to = authorize(grants, 'fs.write', to);
      if (method === 'rename') authorize(grants, 'fs.write', from);
      return mutate(method === 'rename' ? [from, to] : [to], method, [from, to], () => store[method](from, to));
    };
  }
  // `compressImage(from, to, {maxDimension, maxBytes})` (LLP 1069.002 A1):
  // both grants are checked and the options too before anything is read;
  // the codec runs outside the mutation lock, and only the write takes it.
  fs.compressImage = async (from, to, options) => {
    if (typeof from !== 'string' || typeof to !== 'string' || !from.startsWith('app:/') || !to.startsWith('app:/'))
      throw failure('compressImage: needs app:/ paths', 'failed');
    const image = await import('./storage-image.js');
    const { maxDimension, maxBytes } = image.limits(options);
    from = authorize(grants, 'fs.read', from);
    to = authorize(grants, 'fs.write', to);
    requireBelowRoot(to);
    // An entry is one IndexedDB record, read whole by any operation (`stat`
    // too); its size is checked before it becomes a Blob or is decoded.
    await ready();
    const { blob } = await store.blob(from, undefined, image.MAX_BYTES);
    const out = await image.compress(blob, maxDimension, maxBytes);
    const size = out.bytes.byteLength;
    await mutate([to], 'atomicWriteFile', [to, out.bytes.slice(0)], () => store.atomicWriteOwnedFile(to, out.bytes));
    return { path: to, type: 'image/jpeg', size, width: out.width, height: out.height };
  };
  return Object.freeze(fs);
}
