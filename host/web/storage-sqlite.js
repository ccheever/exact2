// @ref LLP 1027 D10 — worker-owned SQLite capability.
import { authorize } from './storage-fs.js';

export function createSqlite(appId, grants) {
  let worker, sequence = 0, disposed = false;
  const pending = new Map();
  const failure = message => Object.assign(new Error(message), { kind: 'Unavailable' });
  function stop(message) {
    disposed = true;
    worker?.terminate();
    for (const { reject } of pending.values()) reject(failure(message));
    pending.clear();
  }
  function request(op, args) {
    if (disposed) return Promise.reject(failure('storage runtime is unloaded'));
    if (!worker) {
      worker = new Worker(new URL('./storage-worker.js', import.meta.url), { type: 'module' });
      worker.onmessage = ({ data }) => {
        const call = pending.get(data.id);
        if (!call) return;
        pending.delete(data.id);
        if (data.error) call.reject(Object.assign(failure(data.error.message), data.error));
        else call.resolve(data.value);
      };
      worker.onerror = event => { event.preventDefault(); stop('SQLite worker failed'); };
      worker.onmessageerror = () => stop('SQLite worker message failed');
    }
    const id = ++sequence;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      try { worker.postMessage({ id, op, args, appId }); }
      catch (error) { pending.delete(id); reject(failure(error.message)); }
    });
  }
  function statement(database, id) {
    return Object.freeze({
      execute: (params = []) => request('statementExecute', [database, id, params]),
      query: (params = []) => request('statementQuery', [database, id, params]),
      close: () => request('statementClose', [database, id]),
    });
  }
  function database(id) {
    return Object.freeze({
      execute: (sql, params = []) => request('execute', [id, sql, params]),
      query: (sql, params = []) => request('query', [id, sql, params]),
      prepare: sql => request('prepare', [id, sql]).then(s => statement(id, s)),
      transaction: commands => request('transaction', [id, commands]),
      close: () => request('close', [id]),
    });
  }
  return Object.freeze({
    async open(path) {
      path = authorize(grants, 'sqlite.open', path);
      return database(await request('open', [path]));
    },
    dispose() { stop('storage runtime is unloaded'); },
  });
}
