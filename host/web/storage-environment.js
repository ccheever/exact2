// A module realm's environment, captured before a module Worker hardens its
// globals: storage's synchronous namespace and host clock, and under the
// agent its repeatable random stream. The I/O adapters load on first use.
const AGENT_ADMITTED = true; // false in a production bake: host/web/build.mjs rewrites this line (LLP 1069.007 D2)
export const directories = Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' });
export const now = Date.now.bind(Date);
// The URL the page was opened at, which names a drive and its scratch store:
// an app's router rewrites `location` as it navigates (the drive's `?agent&…`
// goes with it), the document's navigation entry keeps it. A page that opens
// its store, or a picker its own, after a navigation opens the same one
// (recipes F9). A worker has no entry and is handed its key.
export const launchHref = () => globalThis.performance?.getEntriesByType?.('navigation')[0]?.name ?? location.href;
// Whose store a page opens: the app's, or for a scripted drive (`?agent`) only
// a scratch store the drive names (`&storage=<name>`, `agent.mjs --storage`),
// apart from the app's own, as on native. null: this drive has no storage.
export function storageKey(appId, href = launchHref()) {
  const params = new URL(href).searchParams, name = params.get('storage');
  if (!(AGENT_ADMITTED && params.has('agent'))) return appId;
  if (name == null) return null;
  if (!/^[A-Za-z0-9._-]+$/.test(name) || name === '.' || name === '..') throw new Error("storage: one name of letters, digits, '.', '-' or '_'");
  return `${appId}/agent/${name}`;
}
export const agentStorageRefusal = 'storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)';

// @ref LLP 1069.005 D2b — the launch seed a drive names (`&seed=`, default
// 1, as `exactTime().seed` reads it), or null outside the agent. Only a
// loopback page: a link that adds `?agent` to a deployed page must not make
// its PKCE verifiers predictable.
export function agentSeed(href = launchHref()) {
  const url = new URL(href);
  if (!AGENT_ADMITTED || !url.searchParams.has('agent') || !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)) return null;
  const seed = Number(url.searchParams.get('seed') ?? 1);
  return Number.isSafeInteger(seed) && seed >= 0 ? seed : 1;
}

// The agent's repeatable random stream: ChaCha20 (RFC 8439 §2.3) keyed by
// the seed's eight little-endian bytes and 24 zeros, the executor's label as
// the zero-padded nonce, the block counter from 0; each draw takes the next
// bytes. `exact_data::crypto::AgentStream` is the same stream in Rust.
export function agentStream(seed, label) {
  const state = new Uint32Array(16), block = new Uint8Array(64), nonce = new Uint8Array(12);
  state.set([0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);
  state[4] = seed % 0x100000000; state[5] = Math.floor(seed / 0x100000000);
  nonce.set(new TextEncoder().encode(label).subarray(0, 12));
  const view = new DataView(nonce.buffer);
  for (let i = 0; i < 3; i++) state[13 + i] = view.getUint32(i * 4, true);
  let used = 64;
  const x = new Uint32Array(16), out = new DataView(block.buffer);
  const quarter = (a, b, c, d) => {
    x[a] += x[b]; x[d] ^= x[a]; x[d] = x[d] << 16 | x[d] >>> 16;
    x[c] += x[d]; x[b] ^= x[c]; x[b] = x[b] << 12 | x[b] >>> 20;
    x[a] += x[b]; x[d] ^= x[a]; x[d] = x[d] << 8 | x[d] >>> 24;
    x[c] += x[d]; x[b] ^= x[c]; x[b] = x[b] << 7 | x[b] >>> 25;
  };
  const next = () => {
    x.set(state);
    for (let i = 0; i < 10; i++) {
      quarter(0, 4, 8, 12); quarter(1, 5, 9, 13); quarter(2, 6, 10, 14); quarter(3, 7, 11, 15);
      quarter(0, 5, 10, 15); quarter(1, 6, 11, 12); quarter(2, 7, 8, 13); quarter(3, 4, 9, 14);
    }
    for (let i = 0; i < 16; i++) out.setUint32(i * 4, (x[i] + state[i]) >>> 0, true);
    state[12]++;
    used = 0;
  };
  return (n) => {
    const bytes = new Uint8Array(n);
    for (let i = 0; i < n; i++) { if (used === 64) next(); bytes[i] = block[used++]; }
    return bytes;
  };
}

// @ref LLP 1069.005 D1b — "keep this key" on the web: the CryptoKeyPair
// itself (its private key non-extractable, so its bytes never reach script)
// in the realm's IndexedDB, under a handle `secret.keep` holds. `name` is
// storageKey's: the app's, a drive's scratch store, or null for a drive
// with none, which keeps its keys in this page's memory only.
const memoryKeys = new Map();
export function keyStore(name, indexedDB) {
  if (name === null || !indexedDB) {
    return { put: async (handle, pair) => { memoryKeys.set(handle, pair); }, get: async handle => memoryKeys.get(handle) ?? null };
  }
  let opened = null;
  const open = () => opened ??= new Promise((resolve, reject) => {
    const request = indexedDB.open(`exact.keys:${name}`, 1);
    request.onupgradeneeded = () => request.result.createObjectStore('keys');
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => { opened = null; reject(request.error); };
  });
  const run = (mode, act) => open().then(db => new Promise((resolve, reject) => {
    const tx = db.transaction('keys', mode), request = act(tx.objectStore('keys'));
    tx.oncomplete = () => resolve(request.result);
    tx.onerror = tx.onabort = () => reject(tx.error ?? request.error);
  }));
  return {
    put: (handle, pair) => run('readwrite', keys => keys.put({ privateKey: pair.privateKey, publicKey: pair.publicKey }, handle)).then(() => {}),
    get: handle => run('readonly', keys => keys.get(handle)).then(pair => pair ?? null),
  };
}
