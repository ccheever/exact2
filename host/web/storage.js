// Storage completions belong to an answer checkpoint, not an arbitrary browser
// microtask. @ref LLP 1027 D10: host work returns through the data seam.
import { agentStorageRefusal, directories, storageKey } from './storage-environment.js';
let fileFactory, sqliteFactory;

export function createStorage(win, admitted, scope, key = storageKey(admitted.appId)) {
  // Once storage has been used, a replacement reserves its owner before the
  // old realm is disposed, retaining the shared SQLite worker across reloads.
  let fs = key && fileFactory?.(key, admitted.grantSet);
  let sqlite = key && sqliteFactory?.(key, admitted.grantSet);
  let fsLoading, sqliteLoading;
  const queues = new Map(), waiters = new Map(), retired = new WeakSet();
  let disposed = false;
  const error = e => Object.assign(new win.Error(e?.message || String(e)), {kind:e?.kind || 'Unavailable'});
  const unavailable = () => error({kind:'Unavailable',message:'storage environment disposed'});
  const clone = value => win.structuredClone(value);
  // Keep adapters as shared modules: the Rust request path uses the same
  // filesystem mutation queues and SQLite owners. A module that never calls
  // storage downloads neither adapter.
  const fileSystem = () => fs ? Promise.resolve(fs) : fsLoading ??= import('./storage-fs.js').then(({ createFileSystem }) => {
    fileFactory = createFileSystem;
    if (disposed) throw unavailable();
    return fs = createFileSystem(key, admitted.grantSet);
  });
  const databaseSystem = () => sqlite ? Promise.resolve(sqlite) : sqliteLoading ??= import('./storage-sqlite.js').then(({ createSqlite }) => {
    sqliteFactory = createSqlite;
    if (disposed) throw unavailable();
    return sqlite = createSqlite(key, admitted.grantSet);
  });
  // A completion waits in its answer's queue until the answer's checkpoint.
  const completion = owner => (complete, cleanup = () => {}) => {
    if (disposed || retired.has(owner)) { cleanup(); return; }
    const queue = queues.get(owner) || []; queue.push({complete, cleanup}); queues.set(owner, queue);
    waiters.get(owner)?.(); waiters.delete(owner);
  };
  // Host work an answer waits on that is not storage (a browser digest, LLP
  // 1069.005 D1) completes the same way; it needs no store, so the agent's
  // storage refusal does not apply.
  function work(promise) {
    if (disposed) return win.Promise.reject(unavailable());
    const ready = completion(scope());
    return new win.Promise((resolve, reject) => {
      Promise.resolve(promise).then(value => ready(() => resolve(value)), e => ready(() => reject(e)));
    });
  }
  function enqueue(invoke, convert = clone, discard = () => {}) {
    if (disposed) return win.Promise.reject(unavailable());
    if (key == null) return win.Promise.reject(error({message:agentStorageRefusal}));
    const owner = scope();
    const active = () => { if (disposed || retired.has(owner)) throw unavailable(); };
    return new win.Promise((resolve, reject) => {
      const ready = completion(owner);
      Promise.resolve().then(() => { active(); return invoke(active); }).then(
        value => ready(() => { try { resolve(convert(value)); } catch(e) { discard(value); reject(error(e)); } }, () => discard(value)),
        e => ready(() => reject(error(e))),
      );
    });
  }
  function method(raw, name, args, convert, discard) {
    const captured = structuredClone(args);
    return enqueue(() => raw[name](...captured), convert, discard);
  }
  const closeDiscarded = raw => { void raw.close().catch(() => {}); };
  const statement = raw => Object.freeze({
    execute: params => method(raw, 'execute', [params]),
    query: params => method(raw, 'query', [params]),
    close: () => method(raw, 'close', []),
  });
  const database = raw => Object.freeze({
    execute: (sql, params) => method(raw, 'execute', [sql, params]),
    query: (sql, params) => method(raw, 'query', [sql, params]),
    prepare: sql => method(raw, 'prepare', [sql], statement, closeDiscarded),
    transaction: commands => method(raw, 'transaction', [commands]),
    close: () => method(raw, 'close', []),
  });
  const files = {directories};
  for (const method of ['readFile','writeFile','atomicWriteFile','appendFile','readdir','mkdir','rm','stat','rename','copyFile','realpath']) {
    files[method] = (...args) => {
      // Snapshot input bytes before the caller can mutate its buffers.
      const captured = structuredClone(args);
      return enqueue(async active => { const backend = await fileSystem(); active(); return backend[method](...captured); });
    };
  }
  return {
    capability: Object.freeze({fs:Object.freeze(files),sqlite:Object.freeze({open:path => enqueue(async active => {
      const backend = await databaseSystem(); active(); return backend.open(path);
    }, database, closeDiscarded)}),work}),
    async deliver(owner) {
      if (disposed || retired.has(owner)) throw unavailable();
      if (!queues.get(owner)?.length) await new Promise(resolve => waiters.set(owner,resolve));
      if (disposed || retired.has(owner)) throw unavailable();
      const queue = queues.get(owner), complete = queue?.shift();
      if (!queue?.length) queues.delete(owner);
      if (complete) complete.complete();
    },
    retire(owner) {
      if (!owner) return;
      retired.add(owner);
      for (const entry of queues.get(owner) || []) entry.cleanup();
      queues.delete(owner);
      waiters.get(owner)?.(); waiters.delete(owner);
    },
    dispose() {
      disposed = true; sqlite?.dispose(); fs?.dispose(); queues.clear();
      for (const wake of waiters.values()) wake(); waiters.clear();
    },
  };
}
