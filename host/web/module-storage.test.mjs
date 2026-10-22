// The page's module realm (module-glue.js) over the real prelude, in a vm
// context standing in for its iframe, and its worker placement on a Worker,
// each with a scripted file store.
import { expect, test } from 'bun:test';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import vm from 'node:vm';

const ROOT = resolve(import.meta.dir, '../..');
// The runner's normalized set for `net.fetch https://api.castle.test` (as tests/it/request.rs's batch carries it).
const GRANTS = {version:1,entries:[[1,'net.fetch https://api.castle.test',['fetch','https','api.castle.test',443],null]],error:null,seal:'825ec7a8503217b3'};

// An answer's storage operation queued behind another answer's parks; when
// that answer delivers its own (after its fetch came back), the parked one's
// operation is in flight and it is asked again (prelude.js `settle`, LLP 1097
// D3). Before, only a background delivery asked parked answers again, and the
// Bluesky clone's device settings never answered on the web.
test('a parked answer is asked again after another answer delivers its storage', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-module-storage-'));
  const saved = Object.fromEntries(['fetch', 'document', 'location'].map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]));
  try {
    for (const name of ['module-glue.js', 'storage.js', 'grant-admission.js', 'faults.js', 'navigation.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
    writeFileSync(resolve(dir, 'storage-environment.js'), `export const directories = Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' });
export const storageKey = () => 'k'; export const agentStorageRefusal = 'no store'; export const agentReads = []; export const emptyRead = () => []; export const agentSeed = () => null;
export const agentStream = () => null; export const keyStore = () => ({}); export const now = () => 0;`);
    // Each read lands when the test says, in the order issued.
    writeFileSync(resolve(dir, 'storage-fs.js'), `export const createFileSystem = () => ({
  readFile: path => new Promise(ok => globalThis.reads.push(() => ok(new TextEncoder().encode(path)))), dispose() {} });`);
    const prelude = readFileSync(resolve(ROOT, 'js/src/prelude.js'), 'utf8');
    const binary = readFileSync(resolve(ROOT, 'js/src/binary.js'), 'utf8');
    const script = new TextEncoder().encode(`exact = { abi: 1, appId: 'test.storage', grants: 'net.fetch https://api.castle.test', answer(source, args, store, storage) {
  const read = path => storage.fs.readFile(path).then(bytes => new TextDecoder().decode(bytes));
  if (source === 'feed') { const file = read('app:/data/feed'); return fetch('https://api.castle.test/feed').then(r => r.text()).then(async text => text + ' ' + await file); }
  return read('app:/data/device');
} };`);
    const sha = new Bun.CryptoHasher('sha256').update(script).digest('hex');
    globalThis.reads = [];
    const win = vm.createContext({ console: { log() {}, info() {}, warn() {}, error() {}, debug() {} }, TextEncoder, TextDecoder, structuredClone, setTimeout, clearTimeout,
      queueMicrotask, URL, Headers, crypto, indexedDB: null, MessageChannel, Uint8Array, ArrayBuffer, addEventListener() {} });
    win.window = win;
    for (const name of ['Promise', 'Error', 'Object', 'Array', 'JSON', 'Symbol']) win[name] = vm.runInContext(name, win); // what storage.js builds in the realm
    win.document = { createElement: () => ({}), head: { append: el => vm.runInContext(el.textContent, win) } };
    const frame = { contentWindow: win, setAttribute() {}, remove() {} };
    Object.defineProperties(globalThis, {
      fetch: { configurable: true, writable: true, value: async url => { const r = new Response(String(url).endsWith('module-prelude.js') ? prelude : String(url).endsWith('module-binary.js') ? binary : 'never'); Object.defineProperty(r, 'url', { value: 'http://127.0.0.1/x' }); return r; } },
      document: { configurable: true, value: { createElement: () => frame, body: { append() {} } } },
      location: { configurable: true, value: { href: 'http://127.0.0.1/', origin: 'http://127.0.0.1' } },
    });
    globalThis.exact = {};
    const { createGrantSet } = await import(resolve(dir, 'grant-admission.js'));
    const glue = await import(resolve(dir, 'module-glue.js'));
    const grants = 'net.fetch https://api.castle.test';
    const receipt = new TextEncoder().encode(JSON.stringify({ version: 1, abi: 1, appId: 'test.storage', grants, web: { file: 'app.js', bytes: script.length, sha256: sha }, module: { sha256: 'b'.repeat(64) } }));
    const realm = await glue.prepare({ receipt, script }, { appId: 'test.storage', grants, grantSet: createGrantSet(GRANTS), placement: 'main' });
    const ask = (source, extra = {}) => glue.run(glue.call({ op: 'answer', id: realm.id, target: source, source, args: [], store: [], grants: [], ...extra }).continuation);
    const feed = ask('feed');
    const fetched = await feed; // the fetch: the turn goes, its read in flight
    expect(fetched.tag).toBe(1);
    const device = ask('device');
    await new Promise(r => setTimeout(r, 20)); // its read queued behind feed's: it parks
    const resumed = glue.run(glue.call({ op: 'resume', id: realm.id, target: 'feed', source: 'feed', args: [], store: [], grants: [], outcome: { response: { status: 200, headers: [], body: 'fresh' } } }).continuation);
    globalThis.reads.shift()();
    expect((await resumed).value).toBe('fresh app:/data/feed');
    await new Promise(r => setTimeout(r, 20));
    globalThis.reads.shift()?.();
    const answered = await Promise.race([device, new Promise(r => setTimeout(() => r('still parked'), 500))]);
    expect(answered.value).toBe('app:/data/device');
    realm.dispose();
  } finally {
    for (const [name, descriptor] of Object.entries(saved)) { if (descriptor) Object.defineProperty(globalThis, name, descriptor); else delete globalThis[name]; }
    delete globalThis.exact; delete globalThis.reads;
    rmSync(dir, { recursive: true, force: true });
  }
});

// The real prelude and host turns, with storage backends that land at once.
// `done` closes either placement even when an answer is still parked.
async function moduleRealm(script, { placement = 'worker', key = 'k' } = {}) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-worker-storage-'));
  const saved = Object.fromEntries(['fetch', 'document', 'location', 'exact'].map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]));
  let realm;
  const done = () => {
    realm?.dispose();
    for (const [name, descriptor] of Object.entries(saved)) { if (descriptor) Object.defineProperty(globalThis, name, descriptor); else delete globalThis[name]; }
    rmSync(dir, { recursive: true, force: true });
  };
  try {
    for (const name of ['module-glue.js', 'module-worker.js', 'storage.js', 'grant-admission.js', 'faults.js', 'navigation.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
    writeFileSync(resolve(dir, 'storage-environment.js'), `export const directories = Object.freeze({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' });
export const storageKey = () => ${JSON.stringify(key)}; export const agentStorageRefusal = 'no store'; export const agentReads = ['readdir']; export const emptyRead = () => []; export const agentSeed = () => null;
export const agentStream = () => null; export const keyStore = () => ({}); export const now = () => 0;`);
    writeFileSync(resolve(dir, 'storage-fs.js'), `export const createFileSystem = () => {
  const files = new Map();
  return { readFile: async path => files.get(path) ?? new TextEncoder().encode(path), readdir: async () => [],
    writeFile: async (path, bytes) => { files.set(path, bytes); }, dispose() {} };
};`);
    writeFileSync(resolve(dir, 'storage-sqlite.js'), `export const createSqlite = () => ({
  open: async () => ({ query: async (_sql, params) => params,
    prepare: async () => ({ query: async params => params, close: async () => {} }), close: async () => {} }), dispose() {} });`);
    const prelude = readFileSync(resolve(ROOT, 'js/src/prelude.js'), 'utf8');
    const binary = readFileSync(resolve(ROOT, 'js/src/binary.js'), 'utf8');
    const bytes = new TextEncoder().encode(`exact = { abi: 1, appId: 'test.storage', grants: 'net.fetch https://api.castle.test', answer(source, args, store, storage) {
  const read = path => storage.fs.readFile(path).then(bytes => new TextDecoder().decode(bytes));
  ${script}
} };`);
    const sha = new Bun.CryptoHasher('sha256').update(bytes).digest('hex');
    globalThis.fetch = async url => { const r = new Response(String(url).endsWith('module-prelude.js') ? prelude : String(url).endsWith('module-binary.js') ? binary : 'never'); Object.defineProperty(r, 'url', { value: 'http://127.0.0.1/x' }); return r; };
    globalThis.location = { href: 'http://127.0.0.1/', origin: 'http://127.0.0.1' };
    globalThis.exact = {};
    if (placement === 'main') {
      const win = vm.createContext({ console: { log() {}, info() {}, warn() {}, error() {}, debug() {} }, TextEncoder, TextDecoder, structuredClone, setTimeout, clearTimeout,
        queueMicrotask, URL, Headers, crypto, indexedDB: null, MessageChannel, Uint8Array, ArrayBuffer, addEventListener() {} });
      win.window = win;
      for (const name of ['Promise', 'Error', 'Object', 'Array', 'JSON', 'Symbol']) win[name] = vm.runInContext(name, win);
      win.document = { createElement: () => ({}), head: { append: el => vm.runInContext(el.textContent, win) } };
      const frame = { contentWindow: win, setAttribute() {}, remove() {} };
      Object.defineProperty(globalThis, 'document', { configurable: true, value: { createElement: () => frame, body: { append() {} } } });
    }
    const { createGrantSet } = await import(resolve(dir, 'grant-admission.js'));
    const glue = await import(resolve(dir, 'module-glue.js'));
    const grants = 'net.fetch https://api.castle.test';
    const receipt = new TextEncoder().encode(JSON.stringify({ version: 1, abi: 1, appId: 'test.storage', grants, web: { file: 'app.js', bytes: bytes.length, sha256: sha }, module: { sha256: 'b'.repeat(64) } }));
    realm = await glue.prepare({ receipt, script: bytes }, { appId: 'test.storage', grants, grantSet: createGrantSet(GRANTS), placement });
    const request = (op, source, extra = {}) => ({ op, id: realm.id, target: source, source, args: [], store: [['recovery-owner', source]], grants: [], ...extra });
    return {
      done,
      ask: source => glue.run(glue.call(request('answer', source)).continuation),
      resume: source => glue.run(glue.call(request('resume', source, { outcome: { response: { status: 200, headers: [], body: 'fresh' } } })).continuation),
      forget: inFlight => glue.call({ op: 'forget', id: realm.id, inFlight: inFlight.map(s => request('answer', s)) }),
    };
  } catch (error) { done(); throw error; }
}
const race = p => Promise.race([p.then(v => v, e => `rejected: ${e.message}`), new Promise(r => setTimeout(() => r('STILL WAITING'), 1500))]);

// The sequence above, on the worker. An answer waiting there held the
// worker's turn on an operation only the other answer's turn could deliver,
// and both waited forever; and an operation issued in another answer's turn
// was charged to that answer.
test('a worker-placed answer queued behind another answer\'s storage parks and answers', async () => {
  const w = await moduleRealm(`if (source === 'feed') { const file = read('app:/data/feed'); return fetch('https://api.castle.test/feed').then(r => r.text()).then(async text => text + ' ' + await file); }
  return read('app:/data/' + source);`);
  try {
    expect((await w.ask('feed')).tag).toBe(1);
    const device = w.ask('device');
    await Bun.sleep(50);
    expect((await race(w.resume('feed'))).value).toBe('fresh app:/data/feed');
    expect((await race(device)).value).toBe('app:/data/device');
  } finally { w.done(); }
}, 20000);

// The runner lets go of a parked answer: its turn is refused, and storage
// goes on for the rest; and a let-go chain a parked answer's delivery made
// the head is finished, so the answers after it are not stuck behind it.
test('a worker-placed parked answer the runner lets go is dropped, and a let-go chain finishes', async () => {
  const w = await moduleRealm(`if (source === 'feed') { const file = read('app:/data/feed').then(() => read('app:/data/feed2')); return fetch('https://api.castle.test/feed').then(r => r.text()).then(async text => text + ' ' + await file); }
  return read('app:/data/' + source);`);
  try {
    // A parked answer let go: refused; the feed resumed answers, and one after.
    expect((await w.ask('feed')).tag).toBe(1);
    const device = w.ask('device');
    await Bun.sleep(50);
    w.forget(['feed']);
    expect(String(await race(device))).toContain('rejected');
    expect((await race(w.resume('feed'))).value).toBe('fresh app:/data/feed2');
    expect((await race(w.ask('after'))).value).toBe('app:/data/after');
    // The feed let go between its reads, with an answer parked behind them:
    // its chain is finished, and the parked answer answers.
    expect((await w.ask('feed')).tag).toBe(1);
    const parked = w.ask('parked');
    await Bun.sleep(50);
    w.forget(['parked']);
    expect((await race(parked)).value).toBe('app:/data/parked');
    expect((await race(w.ask('last'))).value).toBe('app:/data/last');
  } finally { w.done(); }
}, 20000);

const setupDatabase = `if (source === 'setup') return storage.sqlite.open('app:/data/test.db').then(async db => {
  globalThis.db = db; globalThis.statement = await db.prepare('SELECT ?'); return 'ready';
});`;

for (const placement of ['main', 'worker']) {
  for (const [name, operation, key] of [
    ['file clone', "storage.fs.writeFile('app:/data/device', () => {})", 'k'],
    ['database clone', "db.query('SELECT ?', [() => {}])", 'k'],
    ['statement clone', 'statement.query([() => {}])', 'k'],
    ['missing store', "storage.fs.writeFile('app:/data/device', 'bytes')", null],
  ]) test(`${placement}: ${name} failure recovers in the answer that reserved it`, async () => {
    const realm = await moduleRealm(`${setupDatabase}
  if (source === 'barrier') return 'ready';
  if (source === 'feed') {
    const file = storage.fs.readdir('app:/data');
    return fetch('https://api.castle.test/feed').then(r => r.text()).then(async text => text + JSON.stringify(await file));
  }
  return ${operation}.then(() => 'unexpected success', error => {
    const owner = store.get('recovery-owner');
    return storage.fs.readdir('app:/data').then(files => ({ owner, files, refused: !!error.code }));
  });`, { placement, key });
    try {
      if (name.includes('database') || name.includes('statement')) expect((await race(realm.ask('setup'))).value).toBe('ready');
      expect((await race(realm.ask('feed'))).tag).toBe(1);
      const device = race(realm.ask('device'));
      // This turn follows device's parked turn, so the failed call is queued
      // before feed resumes, without a timer deciding when it was accepted.
      expect((await race(realm.ask('barrier'))).value).toBe('ready');
      const feed = await race(realm.resume('feed'));
      expect(feed.value).toBe('fresh[]');
      // The store read observes the same host context.owner that reserves
      // the recovery's storage cell. It must belong to device, never feed.
      expect(feed.reads).toEqual([]);
      const recovered = await device;
      expect(recovered.reads).toEqual(['recovery-owner']);
      expect(recovered.value).toEqual({ owner: 'device', files: [], refused: true });
    } finally { realm.done(); }
  });

  test(`${placement}: file and SQLite arguments are snapshotted when issued`, async () => {
    const realm = await moduleRealm(`${setupDatabase}
  const bytes = new Uint8Array([1, 2]);
  if (source === 'file') {
    const written = storage.fs.writeFile('app:/data/snapshot', bytes);
    bytes[0] = 9;
    return written.then(() => storage.fs.readFile('app:/data/snapshot')).then(value => Array.from(value));
  }
  const pending = source === 'database' ? db.query('SELECT ?', [bytes]) : statement.query([bytes]);
  bytes[0] = 9;
  return pending.then(value => Array.from(value[0]));`, { placement });
    try {
      expect((await race(realm.ask('setup'))).value).toBe('ready');
      for (const source of ['file', 'database', 'statement']) expect((await race(realm.ask(source))).value).toEqual([1, 2]);
    } finally { realm.done(); }
  });
}
