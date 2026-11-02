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
      queueMicrotask, URL, Headers, DOMException, crypto, indexedDB: null, MessageChannel, Uint8Array, ArrayBuffer, addEventListener() {} });
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
async function moduleRealm(script, { placement = 'worker', key = 'k', top = '', secrets = false, stored = [] } = {}) {
  const declaration = 'net.fetch https://api.castle.test' + (secrets ? '\nsecret.keep token' : '');
  const normalized = secrets ? { version: 1, entries: [...GRANTS.entries, [2, 'secret.keep token', ['secret-keep', 'token'], null]], error: null, seal: '7c439325539248a1' } : GRANTS;
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-worker-storage-')), captured = {};
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
    const bytes = new TextEncoder().encode(`exact = { abi: 1, appId: 'test.storage', grants: ${JSON.stringify(declaration)}, answer(source, args, store, storage) {
  const read = path => storage.fs.readFile(path).then(bytes => new TextDecoder().decode(bytes));
  ${script}
} };
${top}`);
    const sha = new Bun.CryptoHasher('sha256').update(bytes).digest('hex');
    globalThis.fetch = async url => { const r = new Response(String(url).endsWith('module-prelude.js') ? prelude : String(url).endsWith('module-binary.js') ? binary : 'never'); Object.defineProperty(r, 'url', { value: 'http://127.0.0.1/x' }); return r; };
    globalThis.location = { href: 'http://127.0.0.1/', origin: 'http://127.0.0.1' };
    globalThis.exact = {};
    if (placement === 'main') {
      const win = vm.createContext({ console: { log() {}, info() {}, warn() {}, error() {}, debug() {} }, TextEncoder, TextDecoder, structuredClone, setTimeout, clearTimeout,
        queueMicrotask, URL, Headers, DOMException, crypto, indexedDB: null, MessageChannel, Uint8Array, ArrayBuffer, AbortController, AbortSignal, addEventListener() {} });
      win.window = win;
      for (const name of ['Promise', 'Error', 'Object', 'Array', 'JSON', 'Symbol']) win[name] = vm.runInContext(name, win);
      win.document = { createElement: () => ({}), head: { append: el => vm.runInContext(el.textContent, win) } };
      const frame = { contentWindow: win, setAttribute() {}, remove() {} };
      Object.defineProperty(globalThis, 'document', { configurable: true, value: { createElement: () => frame, body: { append() {} } } });
      // The realm's door, kept as module-glue installs it (the prelude takes and deletes it at load).
      Object.defineProperty(win, '__exact_host', { configurable: true, set(door) {
        captured.door = door; Object.defineProperty(win, '__exact_host', { configurable: true, writable: true, value: door });
      } });
    }
    const { createGrantSet } = await import(resolve(dir, 'grant-admission.js'));
    const glue = await import(resolve(dir, 'module-glue.js'));
    const grants = declaration;
    const receipt = new TextEncoder().encode(JSON.stringify({ version: 1, abi: 1, appId: 'test.storage', grants, web: { file: 'app.js', bytes: bytes.length, sha256: sha }, module: { sha256: 'b'.repeat(64) } }));
    realm = await glue.prepare({ receipt, script: bytes }, { appId: 'test.storage', grants, grantSet: createGrantSet(normalized), placement });
    const request = (op, source, extra = {}) => ({ op, id: realm.id, target: source, source, args: [], store: [['recovery-owner', source], ...stored], grants: secrets ? ['token'] : [], ...extra });
    return {
      done, door: () => captured.door, aborted: () => glue.call({ op: 'aborted', id: realm.id }).tickets,
      ask: source => glue.run(glue.call(request('answer', source)).continuation),
      resume: (source, response = { status: 200, headers: [], body: 'fresh' }) => glue.run(glue.call(request('resume', source, { outcome: { response } })).continuation),
      background: () => glue.call({ op: 'background', id: realm.id }),
      drain: () => glue.run(glue.call({ op: 'background-round', id: realm.id }).token),
      forget: inFlight => glue.call({ op: 'forget', id: realm.id, inFlight: inFlight.map(s => request('answer', s)) }),
      overlay: source => glue.call({ op: 'overlay', id: realm.id, source, args: [], answer: [], writes: [] }),
    };
  } catch (error) { done(); throw error; }
}
const race = p => Promise.race([p.then(v => v, e => `rejected: ${e.message}`), new Promise(r => setTimeout(() => r('STILL WAITING'), 1500))]);

test('background storage counters survive a turn after an answer abandons its read', async () => {
  const w = await moduleRealm(`if (source === 'abandoned') {
    void read('app:/data/abandoned'); throw new Error('abandoned');
  }
  return read('app:/data/current');`, { placement: 'main' });
  try {
    expect((await w.ask('abandoned')).tag).toBe(2);
    expect(w.background().head).toBe(true);
    const current = w.ask('current');
    const drained = await w.drain();
    expect(drained.writes).toBe(0);
    expect(drained.failed).toBe(0);
    expect(w.background().head).toBe(false);
    expect((await current).value).toBe('app:/data/current');
  } finally { w.done(); }
}, 20000);

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

// `scheduler.yield()` (LLP 1141.001 D10): the realm resolves an answer's
// yields in a task of their own, and the answer goes on to its value.
for (const placement of ['main', 'worker']) test(`${placement}: an answer that yields goes on to its value`, async () => {
  const realm = await moduleRealm(`return (async () => { let n = 0; for (let i = 0; i < 5; i++) { await scheduler.yield(); n++; } return 'yielded ' + n; })();`, { placement });
  try {
    expect((await race(realm.ask('walk'))).value).toBe('yielded 5');
  } finally { realm.done(); }
});

// The prelude asks at load whether the host has a local listener (LLP 1133.005), outside any answer: the page's
// realm answers that ask, no (''), the one call newly let through; what it refused outside an answer it still
// refuses (a journal line, 13, and the clock, 21, were already answered there).
test('outside an answer the realm newly answers only the load-time listen ask', async () => {
  const w = await moduleRealm(`return 'loaded';`, { placement: 'main' });
  try {
    expect((await race(w.ask('any'))).value).toBe('loaded');
    const door = w.door();
    expect(door(12, 'listen', '')).toBe('');
    expect(() => door(12, 'callback', '')).toThrow('host call outside an answer');
    expect(() => door(2, 'name', '')).toThrow('host call outside an answer');
    expect(() => door(1, '1', '{}')).toThrow('host call outside an answer');
    // An abort made outside a turn (a background round, a mapper) waits in the realm until the runner asks,
    // and wakes the host to ask, since no turn's batch follows it.
    let wakes = 0;
    globalThis.exact.abortWake = () => wakes++;
    expect(door(23, '9', '')).toBeUndefined();
    expect([wakes, w.aborted(), w.aborted()]).toEqual([1, [9], []]);
  } finally { w.done(); }
}, 20000);

// A fetch the app aborts (the abort runtime gap): another answer's turn aborts an awaited POST, and that
// turn's result names its ticket (door 23) for the runner to answer aborted and the page to cancel; the
// awaiting answer, resumed, has the rejection. A fetch aborted before it is awaited is withdrawn (16).
for (const placement of ['main', 'worker']) {
  test(`an aborted fetch is reported with the turn that aborted it and withdrawn when unsent (${placement})`, async () => {
    const w = await moduleRealm(`if (source === 'held') { globalThis.held = new AbortController();
    return fetch('https://api.castle.test/held', { method: 'POST', body: 'x', signal: globalThis.held.signal }).then(() => 'fetched', e => 'aborted: ' + e.message); }
  if (source === 'halt') { globalThis.held.abort(new Error('by the app')); return 'halted'; }
  if (source === 'pair') { const c = new AbortController(), first = fetch('https://api.castle.test/first'), second = fetch('https://api.castle.test/second', { signal: c.signal });
    c.abort(); return second.then(() => 'sent', e => e.name).then(async why => (await (await first).text()) + ' ' + why); }
  return read('app:/data/' + source);`, { placement });
    try {
      const held = await race(w.ask('held'));
      expect([held.tag, held.request?.method]).toEqual([1, 'POST']);
      let wakes = 0;
      globalThis.exact.abortWake = () => wakes++;
      const halt = await race(w.ask('halt'));
      // The realm keeps the abort until the runner asks, the worker's posted to the page as it was made. Each
      // wakes the host, since a let-go's turn (its storage delivery included) has no batch to follow it.
      expect([halt.value, halt.aborted, w.aborted(), w.aborted()]).toEqual(['halted', undefined, [held.ticket], []]);
      expect(wakes).toBe(1);
      expect((await race(w.resume('held'))).value).toBe('aborted: by the app');
      const pair = await race(w.ask('pair'));
      expect([pair.tag, pair.request?.url, w.aborted()]).toEqual([1, 'https://api.castle.test/first', []]);
      expect((await race(w.resume('pair'))).value).toBe('fresh AbortError');
    } finally { w.done(); }
  }, 20000);
}

// A getter on `exact.overlay` runs inside the overlay's guard (prelude.js `__exact_overlay`, LLP 1054): on the
// page realm an answer's call stays current between its tasks, so before the first read moved under the guard a
// getter could `store.set` through a store it kept into that outstanding answer. Both reads refuse it now.
test('a writing getter on exact.overlay is refused on both of the overlay\'s reads (main)', async () => {
  const w = await moduleRealm(`if (source === 'arm') { globalThis.kept = store; return fetch('https://api.castle.test/feed').then(r => r.text()); }
  return globalThis.tries.join('\\n');`, { placement: 'main', top: `globalThis.tries = [];
Object.defineProperty(exact, 'overlay', { get() {
  if (globalThis.kept) { try { globalThis.kept.set('x', 'y'); globalThis.tries.push('ran'); } catch (e) { globalThis.tries.push('throws ' + e.message); } }
  return (source, args, answer) => answer.concat(['overlaid']);
} });` });
  try {
    expect((await w.ask('arm')).tag).toBe(1); // its fetch out: the answer is outstanding, its store kept
    expect(w.overlay('items')).toMatchObject({ tag: 0, value: ['overlaid'] });
    const tries = (await race(w.ask('tries'))).value.split('\n');
    expect(tries).toHaveLength(2);
    for (const line of tries) expect(line).toStartWith('throws store.set() is unavailable in an overlay');
  } finally { w.done(); }
}, 20000);

function secretEnvironment(kept) {
  const saved = Object.fromEntries(['navigator', 'localStorage'].map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]));
  const disk = new Map([['exact.secret.token', kept]]), queues = new Map();
  let writesFail = false, requested = 0; const barriers = [];
  const locks = { request(name, options, work) {
    if (typeof options === 'function') { work = options; options = {}; }
    requested++; for (const [count, done] of barriers) if (requested >= count) done();
    let granted = false, abort;
    const cancelled = new Promise((_, reject) => { abort = () => { if (!granted) reject(new Error('aborted')); }; options.signal?.addEventListener('abort', abort, { once: true }); });
    const run = (queues.get(name) ?? Promise.resolve()).then(() => {
      if (options.signal?.aborted) throw new Error('aborted');
      granted = true; return work();
    }).finally(() => options.signal?.removeEventListener('abort', abort));
    queues.set(name, run.catch(() => {})); return Promise.race([run, cancelled]);
  } };
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { locks } });
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: key => disk.get(key) ?? null, setItem: (key, value) => { if (writesFail) throw new Error('quota'); disk.set(key, value); }, removeItem: key => disk.delete(key) } });
  return { disk, requested(count) { return requested >= count ? Promise.resolve() : new Promise(done => barriers.push([count, done])); }, failWrites() { writesFail = true; }, restore() {
    for (const [name, descriptor] of Object.entries(saved)) { if (descriptor) Object.defineProperty(globalThis, name, descriptor); else delete globalThis[name]; }
  } };
}

for (const placement of ['main', 'worker']) test(`${placement}: module read flags remain boolean across secret waits`, async () => {
  const environment = secretEnvironment('old');
  const w = await moduleRealm(`if (source === 'plain') return source;
    if (source === 'entropy') crypto.randomUUID();
    if (source === 'external') store.has('token');
    return store.exclusive('token', () => {
      if (source === 'after') { crypto.randomUUID(); store.has('token'); }
      return source;
    });`, { placement, secrets: true });
  try {
    for (const [source, entropy, externalRead] of [
      ['plain', false, false], ['locked', false, false], ['entropy', true, false],
      ['external', false, true], ['after', true, true], ['plain', false, false],
    ]) {
      const result = await w.ask(source);
      expect(result.value).toBe(source);
      expect(result.entropy).toBe(entropy);
      expect(result.externalRead).toBe(externalRead);
    }
  } finally { w.done(); environment.restore(); }
}, 20000);

// These are the real OAuth package, browser prelude, module turns and worker protocol. Only the
// provider's HTTP replies and the browser's lock/storage backends are supplied by the test.
for (const pair of [['main', 'main'], ['main', 'worker'], ['worker', 'worker'], ['main'], ['worker']]) {
  test(`rotating OAuth session is spent once across ${pair.join('/')} realms`, async () => {
    const kept = JSON.stringify({ accessToken: 'old', refreshToken: 'rotate-0', tokenEndpoint: 'https://api.castle.test/token', scopes: [] });
    const environment = secretEnvironment(kept), { disk } = environment;
    const code = new Bun.Transpiler({ loader: 'ts' }).transformSync(readFileSync(resolve(ROOT, 'packages/oauth/index.ts'), 'utf8')).replace(/^export /gm, '');
    const top = `${code}
const provider = oauth({clientId:'fixture',scopes:[],key:'token',endpoints:{authorization:'https://api.castle.test/authorize',token:'https://api.castle.test/token'},apis:['https://api.castle.test']});`;
    const modules = [];
    try {
      for (const placement of pair) modules.push(await moduleRealm(`return provider.fetch(store, 'https://api.castle.test/me').then(r => r.status);`, { placement, secrets: true, stored: [['token', kept]], top }));
      const first = modules[0], second = modules[1] ?? first;
      expect((await first.ask('a')).request.method).toBe('GET');
      expect((await second.ask('b')).request.method).toBe('GET');
      const refreshing = await first.resume('a', { status: 401, headers: [], body: '' });
      expect(refreshing.request.method).toBe('POST');
      expect(refreshing.request.body).toContain('refresh_token=rotate-0');
      const waiting = second.resume('b', { status: 401, headers: [], body: '' });
      const response = { status: 200, headers: [], body: JSON.stringify({ access_token: 'fresh', refresh_token: 'rotate-1', token_type: 'Bearer', expires_in: 3600 }) };
      const refreshed = await first.resume('a', response);
      expect(refreshed.request.method).toBe('GET');
      expect(refreshed.request.headers).toContainEqual(['authorization', 'Bearer fresh']);
      expect((await first.resume('a')).value).toBe(200);
      const reused = await race(waiting);
      expect(reused.request.method).toBe('GET');
      expect(reused.request.headers).toContainEqual(['authorization', 'Bearer fresh']);
      expect((await second.resume('b')).value).toBe(200);
      expect(JSON.parse(disk.get('exact.secret.token')).refreshToken).toBe('rotate-1');
    } finally {
      for (const module of modules.reverse()) module.done();
      environment.restore();
    }
  }, 20000);
}

for (const placement of ['main', 'worker']) test(`${placement}: secret locks retire and failed writes cannot retain a spent token`, async () => {
  const environment = secretEnvironment('old');
  const w = await moduleRealm(`return store.exclusive(source === 'denied' ? 'other' : 'token', async () => {
    if (source === 'hold') await fetch('https://api.castle.test/wait');
    if (source === 'write') store.set('token', 'rotated');
    return store.get('token');
  });`, { placement, secrets: true, stored: [['token', 'stale']] });
  try {
    expect((await w.ask('hold')).tag).toBe(1);
    const waiter = w.ask('waiter');
    const cancelled = race(waiter);
    await environment.requested(2);
    w.forget(['hold']);
    expect(await cancelled).toContain('let go');
    // The cancelled waiter did not end the holder; its continuation still owns the key.
    expect((await w.resume('hold')).value).toBe('old');
    expect((await w.ask('read')).value).toBe('old');
    expect((await w.ask('denied')).message).toContain('not granted');
    environment.failWrites();
    const failed = await w.ask('write').catch(error => error);
    expect(failed.message).toContain('quota');
    expect(environment.disk.has('exact.secret.token')).toBe(false);
    expect((await w.ask('read')).value).toBe(null);
    expect((await w.ask('hold')).tag).toBe(1);
    w.forget([]);
    expect((await race(w.ask('read'))).value).toBe(null);
  } finally { w.done(); environment.restore(); }
}, 20000);


for (const placement of ['main', 'worker']) test(`${placement}: abandoning a sent secret update cannot replay its credential`, async () => {
  const environment = secretEnvironment('old');
  const w = await moduleRealm(`return store.exclusive('token', async () => {
    if (source === 'refresh') await fetch('https://api.castle.test/token', {method:'POST', body:'old'});
    return store.get('token');
  });`, { placement, secrets: true, stored: [['token', 'stale']] });
  try {
    expect((await w.ask('refresh')).request.method).toBe('POST');
    expect(environment.disk.get('exact.exclusive.exact.secret.token')).toBe('1');
    w.forget([]);
    expect((await w.ask('read')).value).toBeNull();
    expect(environment.disk.has('exact.secret.token')).toBe(false);
    // A page that died without cleanup leaves the same marker for its successor.
    environment.disk.set('exact.secret.token', 'spent-before-page-exit');
    environment.disk.set('exact.exclusive.exact.secret.token', '1');
    expect((await w.ask('successor')).value).toBeNull();
    expect(environment.disk.has('exact.exclusive.exact.secret.token')).toBe(false);
  } finally { w.done(); environment.restore(); }
}, 20000);
