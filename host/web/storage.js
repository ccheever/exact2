// Storage completions belong to an answer checkpoint, not an arbitrary browser
// microtask. @ref LLP 1027 D10: host work returns through the data seam.
import { createFileSystem } from './storage-fs.js';
import { createSqlite } from './storage-sqlite.js';

export function createStorage(win, admitted, scope, agent = new URL(location.href).searchParams.has('agent')) {
  const fs = createFileSystem(admitted.appId, admitted.grants);
  const sqlite = createSqlite(admitted.appId, admitted.grants);
  const queues = new Map(), waiters = new Map(), retired = new WeakSet();
  let disposed = false;
  const error = e => Object.assign(new win.Error(e?.message || String(e)), {kind:e?.kind || 'Unavailable'});
  const unavailable = () => error({kind:'Unavailable',message:'storage environment disposed'});
  const clone = value => win.structuredClone(value);
  function enqueue(invoke, convert = clone, discard = () => {}) {
    if (disposed) return win.Promise.reject(unavailable());
    if (agent) return win.Promise.reject(error({message:'storage is unavailable in agent mode'}));
    const owner = scope();
    return new win.Promise((resolve, reject) => {
      const ready = (complete, cleanup = () => {}) => {
        if (disposed || retired.has(owner)) { cleanup(); return; }
        const queue = queues.get(owner) || []; queue.push({complete, cleanup}); queues.set(owner, queue);
        waiters.get(owner)?.(); waiters.delete(owner);
      };
      Promise.resolve().then(invoke).then(
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
  const files = {directories:fs.directories};
  for (const method of ['readFile','writeFile','atomicWriteFile','appendFile','readdir','mkdir','rm','stat','rename','copyFile','realpath']) {
    files[method] = (...args) => {
      // Snapshot input bytes before the caller can mutate its buffers.
      const captured = structuredClone(args);
      return enqueue(() => fs[method](...captured));
    };
  }
  return {
    capability: Object.freeze({fs:Object.freeze(files),sqlite:Object.freeze({open:path => enqueue(() => sqlite.open(path), database, closeDiscarded)})}),
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
      disposed = true; sqlite.dispose(); fs.dispose(); queues.clear();
      for (const wake of waiters.values()) wake(); waiters.clear();
    },
  };
}
