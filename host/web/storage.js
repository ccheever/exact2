// Storage completions belong to an answer checkpoint, not an arbitrary browser
// microtask. @ref LLP 1027 D10: host work returns through the data seam.
import { agentStorageRefusal, directories, storageKey } from './storage-environment.js';
let fileFactory, sqliteFactory, toldAgent = false;

export function createStorage(win, admitted, scope, key = storageKey(admitted.appId)) {
  // Once storage has been used, a replacement reserves its owner before the
  // old realm is disposed, retaining the shared SQLite worker across reloads.
  let fs = key && fileFactory?.(key, admitted.grantSet);
  let sqlite = key && sqliteFactory?.(key, admitted.grantSet);
  let fsLoading, sqliteLoading;
  const queues = new Map(), waiters = new Map(), retired = new WeakSet();
  // Each issued operation's owner, in a cell its completion reads when it
  // lands, not at issue (LLP 1097 D7): an answer that replies before its
  // storage lands hands it to the background (`rehome`), so a completion
  // still in flight finds its new owner and is never dropped as retired.
  const issued = new Map();
  // Cells accepted while the answer is still current, before the adapter
  // runs. Issue order is accept order, so the operation in flight takes the
  // oldest one: a read queued behind a background write keeps the answer
  // that asked, even though the background is current when it is issued.
  const reserved = [];
  const track = owner => {
    const cell = {owner};
    let cells = issued.get(owner);
    if (!cells) issued.set(owner, cells = new Set());
    cells.add(cell);
    return cell;
  };
  const untrack = cell => {
    const cells = issued.get(cell.owner);
    cells?.delete(cell);
    if (cells && !cells.size) issued.delete(cell.owner);
  };
  const reserve = owner => { const cell = track(owner); reserved.push(cell); return cell; };
  const abandon = cell => {
    const at = reserved.indexOf(cell);
    if (at < 0) return;
    reserved.splice(at, 1);
    untrack(cell);
  };
  // A reserved cell, or null when the caller did not reserve (a direct call,
  // or a host with no hook). Early returns drop a reserved cell so the next
  // operation does not take it.
  const claim = () => reserved.shift() || null;
  let disposed = false;
  const error = e => Object.assign(new win.Error(e?.message || String(e)), {kind:e?.kind || 'Unavailable', code:e?.code});
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
  // A completion waits in its owner's queue until that owner's checkpoint.
  // One that arrives after the owner was retired rejects the caller, so the
  // prelude's head clears and the next operation can be issued.
  const completion = cell => (complete, cleanup = () => {}, fail = () => {}) => {
    untrack(cell);
    const owner = cell.owner;
    if (disposed || retired.has(owner)) { cleanup(); fail(); return; }
    const queue = queues.get(owner) || []; queue.push({complete, cleanup, fail}); queues.set(owner, queue);
    waiters.get(owner)?.(); waiters.delete(owner);
  };
  // Host work an answer waits on that is not storage (a browser digest, LLP
  // 1069.005 D1) completes the same way; it needs no store, so the agent's
  // storage refusal does not apply.
  function work(promise) {
    if (disposed) return win.Promise.reject(unavailable());
    const ready = completion(track(scope()));
    return new win.Promise((resolve, reject) => {
      const fail = () => reject(unavailable());
      Promise.resolve(promise).then(
        value => ready(() => resolve(value), () => {}, fail),
        e => ready(() => reject(e), () => {}, fail),
      );
    });
  }
  function enqueue(invoke, convert = clone, discard = () => {}, keyed = true) {
    const reservedCell = claim();
    const drop = () => { if (reservedCell) untrack(reservedCell); };
    if (disposed) { drop(); return win.Promise.reject(unavailable()); }
    if (keyed && key == null) {
      drop();
      // Said once, as on every host (trivia F7): the page's console reaches the driver's logs.
      if (!toldAgent) { toldAgent = true; console.warn(`storage refused (agent): ${agentStorageRefusal}`); }
      return win.Promise.reject(error({message:agentStorageRefusal, code:'agent'}));
    }
    const cell = reservedCell || track(scope());
    const active = () => { if (disposed || retired.has(cell.owner)) throw unavailable(); };
    return new win.Promise((resolve, reject) => {
      const ready = completion(cell);
      const fail = () => reject(unavailable());
      Promise.resolve().then(() => { active(); return invoke(active); }).then(
        value => ready(() => { try { resolve(convert(value)); } catch(e) { discard(value); reject(error(e)); } }, () => discard(value), fail),
        e => ready(() => reject(error(e)), () => {}, fail),
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
  // A document the person chose (`doc:/`, LLP 1069.010 D1) is a handle the
  // page keeps (documents-glue.js), reached as a Rust source's storage
  // request reaches it: under the same grants, with no store key. A module
  // on a worker (LLP 1027.002) has no page to keep them: it is refused.
  let documents;
  const documentFiles = () => documents ??= (typeof window === 'undefined'
    ? Promise.reject(error({message:'storage: the documents the person chose live in the page; a worker-placed module cannot reach them (placement main reaches them)', code:'unsupported'}))
    : import('./documents-glue.js').then(() => globalThis.exact.documents.files(admitted.grantSet)));
  const files = {directories};
  for (const method of ['readFile','writeFile','atomicWriteFile','appendFile','readdir','mkdir','rm','stat','rename','copyFile','realpath']) {
    files[method] = (...args) => {
      // Snapshot input bytes before the caller can mutate its buffers.
      const captured = structuredClone(args);
      if (captured.slice(0, 2).some(p => typeof p === 'string' && p.startsWith('doc:/')))
        return enqueue(async active => { const backend = await documentFiles(); active(); return backend[method](...captured); }, clone, () => {}, false);
      return enqueue(async active => { const backend = await fileSystem(); active(); return backend[method](...captured); });
    };
  }
  // The prelude hands `compressImage` its options as two numbers (LLP
  // 1069.002 A1); app files only, so never the documents' backend.
  files.compressImage = (from, to, maxDimension, maxBytes) =>
    enqueue(async active => { const backend = await fileSystem(); active(); return backend.compressImage(from, to, {maxDimension, maxBytes}); });
  return {
    reserve, abandon,
    capability: Object.freeze({fs:Object.freeze(files),sqlite:Object.freeze({open:path => enqueue(async active => {
      const backend = await databaseSystem(); active(); return backend.open(path);
    }, database, closeDiscarded)}),work}),
    async deliver(owner) {
      await this.ready(owner);
      this.deliverNow(owner);
    },
    // Until `owner` has a completion waiting (a background round waits here,
    // outside the realm's turns, LLP 1097 D7).
    async ready(owner) {
      if (disposed || retired.has(owner)) throw unavailable();
      if (!queues.get(owner)?.length) await new Promise(resolve => waiters.set(owner,resolve));
      if (disposed || retired.has(owner)) throw unavailable();
    },
    // The first of `owners` that has a completion waiting. Losers do not keep
    // a waiter, so a later `ready` is not resolved by this race.
    async waitFor(owners) {
      if (disposed) throw unavailable();
      const queued = () => owners.find(owner => !retired.has(owner) && queues.get(owner)?.length);
      const hit = queued();
      if (hit) return hit;
      let done = false;
      const picked = await new Promise(resolve => {
        const wrapped = new Map();
        const finish = owner => {
          if (done) return;
          done = true;
          for (const [other, fn] of wrapped) if (waiters.get(other) === fn) waiters.delete(other);
          wrapped.clear();
          resolve(owner);
        };
        for (const owner of owners) {
          const prev = waiters.get(owner);
          const fn = () => { prev?.(); finish(owner); };
          wrapped.set(owner, fn);
          waiters.set(owner, fn);
        }
        const raced = queued();
        if (raced) finish(raced);
      });
      if (disposed || retired.has(picked)) throw unavailable();
      return picked;
    },
    // Its first waiting completion, now; whether there was one.
    deliverNow(owner) {
      const queue = queues.get(owner), complete = queue?.shift();
      if (!queue?.length) queues.delete(owner);
      if (complete) complete.complete();
      return !!complete;
    },
    // An answer that replied with storage in flight hands it to `to` (the
    // background), before its owner is retired: every cell still in flight
    // and every completion queued (LLP 1097 D7).
    rehome(owner, to) {
      const cells = issued.get(owner);
      if (cells) {
        issued.delete(owner);
        let into = issued.get(to);
        if (!into) issued.set(to, into = new Set());
        for (const cell of cells) { cell.owner = to; into.add(cell); }
      }
      const queued = queues.get(owner);
      if (queued?.length) {
        queues.delete(owner);
        queues.set(to, [...(queues.get(to) || []), ...queued]);
        waiters.get(to)?.(); waiters.delete(to);
      }
    },
    retire(owner) {
      if (!owner) return;
      retired.add(owner);
      for (const entry of queues.get(owner) || []) { entry.cleanup(); entry.fail?.(); }
      queues.delete(owner);
      waiters.get(owner)?.(); waiters.delete(owner);
    },
    dispose() {
      disposed = true; sqlite?.dispose(); fs?.dispose(); queues.clear();
      for (const wake of waiters.values()) wake(); waiters.clear();
    },
  };
}

// The prelude reserves a cell when the app calls storage, while the answer
// is still current, and abandons it if the call throws before `enqueue`
// takes the cell. Hermes has no such hook; its adapter runs at the call.
export function bindAnswerStorage(target, storage, ownerOf) {
  target.__exact_reserve_storage = () => storage.reserve(ownerOf());
  target.__exact_abandon_storage = cell => storage.abandon(cell);
}

// Deliver a let-go chain until `__exact_let_go` says nothing is left, then
// retire its owners. `"queued"` means the head is someone else's operation:
// stop, and leave the owners in `owed` for the delivery that makes this
// chain the head. Calling `letGo` when it returns `""` drops the calls, so
// this does not call it again.
export async function finishLetGo(storage, owed, hooks) {
  let status;
  while ((status = hooks.letGo()) === 'storage') {
    const owners = [...owed.values()];
    if (!owners.length) break;
    const owner = owners.length === 1 ? owners[0] : await storage.waitFor(owners);
    await hooks.deliver(owner);
    if (hooks.disposed()) return;
  }
  if (status === '') {
    for (const owner of owed.values()) storage.retire(owner);
    owed.clear();
  }
}
