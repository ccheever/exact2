// @ref LLP 1027 D10 — worker-owned SQLite capability.
import { authorize } from './storage-fs.js';

// Owners exist before their first query, but workers still start lazily.
const workers = new Map();
let nextClient = 0;
const failure = message => Object.assign(new Error(message), { kind: 'Unavailable' });

export function createSqlite(appId, grants) {
  let shared = workers.get(appId);
  if (!shared) {
    shared = { worker: null, clients: new Map() };
    workers.set(appId, shared);
  }
  const client = ++nextClient, pending = new Map();
  let sequence = 0, disposed = false;
  function stop(message) {
    if (disposed) return;
    disposed = true;
    shared.clients.delete(client);
    for (const { reject } of pending.values()) reject(failure(message));
    pending.clear();
    if (!shared.clients.size) {
      shared.worker?.terminate();
      if (workers.get(appId) === shared) workers.delete(appId);
    } else shared.worker?.postMessage({ client, op: 'dispose' });
  }
  shared.clients.set(client, { pending, stop });
  function request(op, args) {
    if (disposed) return Promise.reject(failure('storage runtime is unloaded'));
    if (!shared.worker) {
      const worker = shared.worker = new Worker(new URL('./storage-worker.js', import.meta.url), { type: 'module' });
      worker.onmessage = ({ data }) => {
        const calls = shared.clients.get(data.client)?.pending;
        const call = calls?.get(data.id);
        if (!call) return;
        calls.delete(data.id);
        if (data.error) call.reject(Object.assign(failure(data.error.message), data.error));
        else call.resolve(data.value);
      };
      const failed = message => {
        for (const owner of [...shared.clients.values()]) owner.stop(message);
      };
      worker.onerror = event => { event.preventDefault(); failed('SQLite worker failed'); };
      worker.onmessageerror = () => failed('SQLite worker message failed');
    }
    const id = ++sequence;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      try { shared.worker.postMessage({ client, id, op, args, appId }); }
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
