import { expect, test } from 'bun:test';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { Worker } from 'node:worker_threads';
import { Cdp } from '../../scripts/agent.mjs';
import { boundedHttpBody, request } from './http-body.js';
import { deferredFulfill, refusal } from './navigation.js';
import { admitsNetwork, coversPath, createGrantSet, grantError, sameGrantDeclaration, scopedGrantSet } from './grant-admission.js';
import { createRequestExecutor, createSecretFacade, fetchWith } from '../web-js/admission.js';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const sets = new Map();
function normalized(spec) {
  if (sets.has(spec)) return sets.get(spec);
  const scratch = mkdtempSync(resolve(tmpdir(), 'exact-grants-'));
  const input = resolve(scratch, 'grants.txt');
  writeFileSync(input, spec);
  const target = resolve(process.env.CARGO_TARGET_DIR || resolve(ROOT, 'target'), `debug/exact-web-js${process.platform === 'win32' ? '.exe' : ''}`);
  const result = existsSync(target)
    ? spawnSync(target, ['normalize-grants', input], { cwd: ROOT, encoding: 'utf8' })
    : spawnSync('cargo', ['run', '-q', '-p', 'exact-web-js', '--', 'normalize-grants', input], { cwd: ROOT, encoding: 'utf8' });
  rmSync(scratch, { recursive: true });
  if (result.status !== 0) throw new Error(result.stderr || 'grant normalizer failed');
  const set = JSON.parse(result.stdout);
  sets.set(spec, set);
  return set;
}
const text = result => new TextDecoder().decode(result.body);

test('native requests resolve their real source scope before calling the module', async () => {
  const grants = normalized('fs.read app:/data');
  let calls = 0;
  const host = { grantSet: grants, controllers: new Set(), loadPageNative: async () => ({
    later: async () => { calls++; return 'é'.repeat(512 * 1024); },
  }) };
  const op = { op: 'request', ticket: 1, method: 'POST', url: 'exact-native:', headers: {}, body: btoa('{}'), maxResponseBytes: 1024 * 1024 };
  const exceeded = await request({ ...op, scope: 'fs.write app:/data' }, host);
  expect([exceeded.kind, text(exceeded), calls]).toEqual([2, "source scope exceeds the app's admitted grants", 0]);
  const admitted = await request({ ...op, scope: 'fs.read app:/data' }, host);
  expect([admitted.kind, admitted.body.length, calls]).toEqual([0, 1024 * 1024, 1]);
});

test('admission refusal delivers Refused only after the enclosing batch, with its incarnation', async () => {
  const delivered = [], inflight = new Set(), defer = deferredFulfill((...args) => delivered.push(args), inflight);
  defer(...refusal({ op: 'refuse', ticket: 17, message: 'only HTTP may opt into independent transport' }, 4));
  expect(delivered.length).toBe(0);
  expect(inflight.size).toBe(1);
  await Promise.resolve();
  expect(delivered.length).toBe(1);
  expect(delivered[0].slice(0, 5)).toEqual([4, 17, 2, 0, '']);
  expect(new TextDecoder().decode(delivered[0][5])).toBe('only HTTP may opt into independent transport');
});

test('wasm and JS request executors refuse outside origins and redirects, and admit a granted origin', async () => {
  let destinationHits = 0;
  const destination = Bun.serve({ port: 0, fetch() { destinationHits++; return new Response('secret'); } });
  const grantedDestination = Bun.serve({ port: 0, fetch() { return new Response('granted'); } });
  const origin = Bun.serve({ port: 0, fetch(req) {
    if (new URL(req.url).pathname === '/redirect') return new Response(null, { status: 307, headers: { location: `${destination.url}private` } });
    if (new URL(req.url).pathname === '/granted-redirect') return new Response(null, { status: 307, headers: { location: `${grantedDestination.url}public` } });
    return new Response('ok');
  } });
  const set = normalized(`net.fetch ${origin.url.origin}\nnet.fetch ${grantedDestination.url.origin}`);
  const wasm = op => request(op, { grantSet: set, controllers: new Set(), moduleLoader: null });
  const js = createRequestExecutor('test.app', set, response => response.arrayBuffer());
  try {
    for (const run of [wasm, js]) {
      const refused = await run({ method: 'GET', url: `${destination.url}outside`, headers: [] });
      expect(refused.failed ?? refused.kind).toBe(2);
      expect(refused.message ?? text(refused)).toBe("outside the app's grants (net.fetch)");
      const admitted = await run({ method: 'GET', url: `${origin.url}ok`, headers: [] });
      expect(admitted.failed ?? admitted.kind ?? 0).toBe(0);
      expect(new TextDecoder().decode(admitted.body)).toBe('ok');
      const redirected = await run({ method: 'GET', url: `${origin.url}redirect`, headers: [] });
      expect(redirected.failed ?? redirected.kind).toBe(2);
      // It names where the redirect led (podcast F5), as the native executor does.
      expect(redirected.message ?? text(redirected)).toBe(`outside the app's grants (net.fetch): redirected to ${destination.url.origin}`);
      const grantedRedirect = await run({ method: 'GET', url: `${origin.url}granted-redirect`, headers: [] });
      expect(grantedRedirect.failed ?? grantedRedirect.kind ?? 0).toBe(0);
      expect(new TextDecoder().decode(grantedRedirect.body)).toBe('granted');
    }
    expect(destinationHits).toBe(2);
  } finally { origin.stop(true); destination.stop(true); grantedDestination.stop(true); }
});

// LLP 1109 D3: a response over its size limit is the host refusing it (`failure(x)`'s `refused`) on every web
// executor, as on Apple's: plain HTTP on the wasm host, a stream's one answer, a Rust source's request on the JS
// target, and a TypeScript source's `fetch` there, which takes `exactIndependentHttp.maxResponseBytes` as native does.
test('a response over its size limit is Refused on every web executor', async () => {
  // `/open` sends its 4096 bytes and never ends: the refusal must not wait for its end.
  const origin = Bun.serve({ port: 0, fetch(req) {
    if (new URL(req.url).pathname !== '/open') return new Response('x'.repeat(4096));
    return new Response(new ReadableStream({ start(c) { c.enqueue(new TextEncoder().encode('x'.repeat(4096))); } }));
  } });
  const set = normalized(`net.fetch ${origin.url.origin}`);
  const url = `${origin.url}big`;
  try {
    for (const op of [
      { method: 'GET', url, headers: [], nativeHttp: 'independent', maxResponseBytes: 64 },
      { method: 'GET', url, headers: [], stream: true, maxResponseBytes: 64 },
    ]) {
      const over = await request(op, { grantSet: set, controllers: new Set() });
      expect([over.kind, text(over)]).toEqual([2, 'HTTP response exceeds limit']);
    }
    const within = await request({ method: 'GET', url, headers: [], nativeHttp: 'independent', maxResponseBytes: 4096 }, { grantSet: set, controllers: new Set() });
    expect([within.kind, within.body.length]).toEqual([0, 4096]);
    const rust = createRequestExecutor('test.app', set, boundedHttpBody);
    for (const at of [url, `${origin.url}open`]) {
      expect(await rust({ method: 'GET', url: at, headers: [], maxResponseBytes: 64 })).toEqual({ failed: 2, message: 'HTTP response exceeds limit' });
      const refused = await fetchWith(set, at, { exactIndependentHttp: { maxResponseBytes: 64 } }).catch(e => e);
      expect([refused.name, refused.kind, refused.message]).toEqual(['FetchError', 'Refused', 'HTTP response exceeds limit']);
    }
    const exact = await fetchWith(set, url, { exactIndependentHttp: { maxResponseBytes: 4096 } });
    expect((await exact.text()).length).toBe(4096);
    expect((await (await fetchWith(set, url)).text()).length).toBe(4096);
    for (const bad of [0, 1.5, 67108865, '64', null]) {
      const e = await fetchWith(set, url, { exactIndependentHttp: bad === null ? null : { maxResponseBytes: bad } }).catch(e => e);
      expect([e.name, e.message]).toEqual(['TypeError', 'exactIndependentHttp.maxResponseBytes must be an integer from 1 to 67108864']);
    }
  } finally { origin.stop(true); }
});

test('a redirect rejected before the browser exposes a Response is the declared Network deviation', async () => {
  const origin = Bun.serve({ port: 0, fetch() {
    return new Response(null, { status: 307, headers: { location: 'http://127.0.0.1:1/unreachable' } });
  } });
  const set = normalized(`net.fetch ${origin.url.origin}`);
  const runs = [
    op => request(op, { grantSet: set, controllers: new Set() }),
    createRequestExecutor('test.app', set, response => response.arrayBuffer()),
  ];
  try {
    for (const run of runs) {
      const result = await run({ method: 'GET', url: origin.url.href, headers: [] });
      expect(result.failed ?? result.kind).toBe(1);
      expect(result.message ?? text(result)).not.toContain("outside the app's grants");
    }
  } finally { origin.stop(true); }
});

test('the wasm early request is claimed and its ungranted final redirect is Refused', async () => {
  globalThis.exact ??= {};
  const { claim, fetchEarly } = await import(`./module-glue.js?early=${Date.now()}`);
  let destinationHits = 0;
  const destination = Bun.serve({ port: 0, fetch() { destinationHits++; return new Response('secret'); } });
  const origin = Bun.serve({ port: 0, fetch(req) {
    return new URL(req.url).pathname === '/redirect'
      ? new Response(null, { status: 307, headers: { location: `${destination.url}private` } })
      : new Response('ok');
  } });
  const set = normalized(`net.fetch ${origin.url.origin}`);
  try {
    const request = { method: 'GET', url: `${origin.url}redirect`, headers: [] };
    expect(fetchEarly(request, set)).toBeFunction();
    const response = await (await import('./http-body.js')).request(request, { grantSet: set, controllers: new Set(), moduleLoader: { claim } });
    expect([response.kind, text(response)]).toEqual([2, `outside the app's grants (net.fetch): redirected to ${destination.url.origin}`]);
    expect(destinationHits).toBe(1);
    expect(fetchEarly({ ...request, url: `${destination.url}outside` }, set)).toBeNull();
  } finally { origin.stop(true); destination.stop(true); }
});

test('malformed parents refuse unscoped work but preserve native child-scope semantics and exact-only lines', async () => {
  let hits = 0;
  const origin = Bun.serve({ port: 0, fetch() { hits++; return new Response('ok'); } });
  const spec = `net.fetch ${origin.url.origin}\nauth.session ${origin.url.origin}\nsecret.keep camelCase`;
  const set = normalized(spec), why = "the app's grants did not parse: line 3: `camelCase` is not a secret name ([a-z0-9._-]{1,64})";
  const run = scope => request({ method: 'GET', url: origin.url.href, headers: [], scope }, { grantSet: set, controllers: new Set() });
  try {
    const refused = await run(null);
    expect([refused.kind, text(refused), hits]).toEqual([2, why, 0]);
    const childSource = `net.fetch ${origin.url.origin}\nauth.session ${origin.url.origin}`;
    const child = scopedGrantSet(set, childSource);
    expect(grantError(child)).toBeNull();
    expect(await run(childSource).then(r => [r.kind, text(r), hits])).toEqual([0, 'ok', 1]);
    expect(grantError(scopedGrantSet(set, 'secret.keep camelCase'))).toContain('line 1: `camelCase`');
    const fabricated = { version: 1, entries: [[1, 'net.fetch https://*.com', ['fetch-subdomains', 'https', 'com', 443], null]], error: null };
    expect([grantError(fabricated), admitsNetwork(fabricated, 'https://evil.com')]).toEqual(['the grant set was not validated', false]);
    const body = JSON.stringify(fabricated), encoder = new TextEncoder(); let seal = 0xcbf29ce484222325n;
    for (const byte of encoder.encode(body)) { seal ^= BigInt(byte); seal = BigInt.asUintN(64, seal * 0x100000001b3n); }
    fabricated.seal = seal.toString(16).padStart(16, '0');
    expect([grantError(fabricated), admitsNetwork(fabricated, 'https://evil.com')]).toEqual(['the grant set was not validated', false]);
    const substituted = { version: 1, entries: [[1, '# no I/O', ['fetch', 'https', 'evil.example', 443], null]], error: null };
    const substitutedBody = JSON.stringify(substituted); seal = 0xcbf29ce484222325n;
    for (const byte of encoder.encode(substitutedBody)) { seal ^= BigInt(byte); seal = BigInt.asUintN(64, seal * 0x100000001b3n); }
    substituted.seal = seal.toString(16).padStart(16, '0');
    expect([grantError(substituted), admitsNetwork(substituted, 'https://evil.example')]).toEqual(['the grant set was not validated', false]);
  } finally { origin.stop(true); }
});

test('a narrower child cannot use another origin held by its parent', async () => {
  const a = Bun.serve({ port: 0, fetch() { return new Response('a'); } });
  const b = Bun.serve({ port: 0, fetch() { return new Response('b'); } });
  const set = normalized(`net.fetch ${a.url.origin}\nnet.fetch ${b.url.origin}`);
  try {
    const result = await request({ method: 'GET', url: b.url.href, headers: [], scope: `net.fetch ${a.url.origin}` }, { grantSet: set, controllers: new Set() });
    expect([result.kind, text(result)]).toEqual([2, "outside the app's grants (net.fetch)"]);
  } finally { a.stop(true); b.stop(true); }
});

test('TypeScript fetch exposes FetchError kinds while a late host module keeps browser fetch', async () => {
  const browserFetch = globalThis.fetch;
  const origin = Bun.serve({ port: 0, fetch() { return new Response('ok'); } });
  const set = normalized(`net.fetch ${origin.url.origin}`);
  const locationDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'location');
  const headersDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'Headers'), NativeHeaders = globalThis.Headers;
  Object.defineProperty(globalThis, 'location', { configurable: true, value: { href: origin.url.href, origin: origin.url.origin } });
  Object.defineProperty(globalThis, 'Headers', { configurable: true, writable: true, value: class extends NativeHeaders {
    constructor(value) { if (value && Object.keys(value).some(name => name.includes(' '))) throw new TypeError('Invalid header name'); super(value); }
  } });
  try {
    await expect(fetchWith(set, 'https://outside.test/')).rejects.toMatchObject({ name: 'FetchError', kind: 'Refused', message: "outside the app's grants (net.fetch)" });
    await expect(fetchWith(set, 'http://[')).rejects.toMatchObject({ name: 'FetchError', kind: 'Network' });
    await expect(fetchWith(set, '/relative')).rejects.toMatchObject({ name: 'FetchError', kind: 'Network' });
    await expect(fetchWith(set, origin.url, { headers: { 'Bad Name': 'x' } })).rejects.toMatchObject({ name: 'FetchError', kind: 'Network' });
    await expect(fetchWith(set, '/assets/runtime.wasm', { headers: { 'Bad Name': 'x' } })).rejects.toMatchObject({ name: 'FetchError', kind: 'Network' });
    expect(await (await fetchWith(set, origin.url)).text()).toBe('ok');
    const scratch = mkdtempSync(resolve(tmpdir(), 'exact-late-host-')), module = resolve(scratch, 'late.mjs');
    writeFileSync(module, 'export default globalThis.fetch;\n');
    const lateHost = await import(pathToFileURL(module).href);
    rmSync(scratch, { recursive: true });
    expect(globalThis.fetch).toBe(browserFetch);
    expect(lateHost.default).toBe(browserFetch);
    origin.stop(true);
    await expect(fetchWith(set, origin.url)).rejects.toMatchObject({ name: 'FetchError', kind: 'Network' });
  } finally {
    origin.stop(true);
    if (locationDescriptor) Object.defineProperty(globalThis, 'location', locationDescriptor); else delete globalThis.location;
    if (headersDescriptor) Object.defineProperty(globalThis, 'Headers', headersDescriptor); else delete globalThis.Headers;
  }
});

test('a bodyless same-origin assets GET is host I/O on the JS target', async () => {
  const origin = Bun.serve({ port: 0, fetch(req) { return new Response(new URL(req.url).pathname); } });
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'location');
  Object.defineProperty(globalThis, 'location', { configurable: true, value: { href: origin.url.href, origin: origin.url.origin } });
  try {
    expect(await (await fetchWith(normalized(''), '/assets/runtime.wasm')).text()).toBe('/assets/runtime.wasm');
    expect(await (await fetchWith(normalized(''), '/assets/runtime.wasm', null)).text()).toBe('/assets/runtime.wasm');
    await expect(fetchWith(normalized(''), '/assets/runtime.wasm', { method: 'POST' })).rejects.toMatchObject({ kind: 'Network' });
  } finally {
    origin.stop(true);
    if (descriptor) Object.defineProperty(globalThis, 'location', descriptor); else delete globalThis.location;
  }
});

test('the TypeScript secret facade has native Store null, reservation, read and parse behavior', async () => {
  const backing = new Map([['token', 'kept'], ['dpop', 'handle']]);
  const set = normalized('secret.keep token\nsecret.keep dpop\nsecret.keep exact.kept.foo');
  const keys = () => Promise.resolve({ get: value => value === 'handle' ? 'pair' : null, put() {} });
  const store = createSecretFacade(backing, set, keys);
  expect(store.get('missing')).toBeNull();
  expect(store.read).toBe(true);
  store.read = false;
  expect(await store.key('dpop')).toBe('pair');
  expect(store.read).toBe(true);
  expect(() => store.set('exact.kept.foo', 'x')).toThrow('not granted');
  store.read = false;
  expect(store.get('exact.kept.foo')).toBeNull();
  expect(store.read).toBe(false);
  expect(await store.key('exact.kept.foo')).toBeNull();
  expect(store.read).toBe(false);
  const malformed = createSecretFacade(new Map(), normalized('secret.keep camelCase'), keys);
  expect(() => malformed.set('token', 'x')).toThrow('line 1');
});

test('filesystem admission is component-based and refuses traversal', () => {
  const set = normalized('fs.read app:/data\nfs.write app:/data/out');
  expect(coversPath(set, 'fs.read', 'app:/data/note.txt')).toBe(true);
  expect(coversPath(set, 'fs.read', 'app:/database/note.txt')).toBe(false);
  expect(coversPath(set, 'fs.read', 'app:/data/../secret')).toBe(false);
  expect(coversPath(set, 'fs.write', 'app:/data/other')).toBe(false);
});

test('a refused sqlite.open names the file it wanted and the grant lines that admit it', async () => {
  const { authorize } = await import('./storage-fs.js');
  const set = normalized('sqlite.open app:/data/garden');
  expect(() => authorize(set, 'sqlite.open', 'app:/data/garden-amy.sqlite')).toThrow(
    'denied: sqlite.open app:/data/garden-amy.sqlite: no grant covers it; grant `sqlite.open app:/data/garden-amy.sqlite`, or `sqlite.open app:/data` for every file there');
  expect(authorize(normalized('sqlite.open app:/data'), 'sqlite.open', 'app:/data/garden-amy.sqlite')).toBe('app:/data/garden-amy.sqlite');
});

// LLP 1069.010 D1, files F2: a folder the person chose is one reach on the
// web, for a TypeScript source (`files`) and a Rust request (`run`), with
// the codes and refusals ibex2's `run_document` gives both on Hermes.
test('a chosen folder is the same storage for a TypeScript source and a Rust request', async () => {
  globalThis.exact = {};
  try {
    await import(`./documents-glue.js?documents=${Date.now()}`);
    let chosen;
    const host = globalThis.exact.documents.install({ dispatch: (_, kind, payload) => { chosen = [kind, payload]; }, log() {}, openFile: () => false });
    host.answer({ node: 1 }, 'showDirectoryPicker', [{ name: 'chosen', files: [{ path: 'a.txt', bytes: btoa('A') }, { path: 'sub/b.txt', bytes: btoa('B') }] }]);
    expect(chosen[0]).toBe(1);
    const doc = chosen[1], both = normalized('fs.read doc:/\nfs.write doc:/'), read = normalized('fs.read doc:/');
    expect(doc).toMatch(/^doc:\/\d+\/chosen$/);
    const fs = globalThis.exact.documents.files(both), text = (b) => new TextDecoder().decode(b);
    expect(await fs.readdir(doc)).toEqual(['a.txt', 'sub']);
    expect(await fs.readdir(doc.replace(/\/chosen$/, ''))).toEqual(['chosen']);
    expect(text(await fs.readFile(`${doc}/sub/b.txt`))).toBe('B');
    expect(await fs.stat(`${doc}/sub`)).toMatchObject({ size: 0, isDirectory: true, isFile: false });
    expect(await fs.stat(`${doc}/a.txt`)).toMatchObject({ size: 1, isFile: true });
    await fs.writeFile(`${doc}/new.txt`, new TextEncoder().encode('hi'));
    await fs.appendFile(`${doc}/new.txt`, new TextEncoder().encode('!'));
    expect(text(await fs.readFile(`${doc}/new.txt`))).toBe('hi!');
    await fs.mkdir(`${doc}/x/y`);
    expect(await fs.readdir(`${doc}/x`)).toEqual(['y']);
    await fs.rm(`${doc}/new.txt`);
    const code = (p) => p.then(() => 'ok', (e) => `${e.kind} ${e.code}`);
    expect(await Promise.all([
      code(fs.readFile(`${doc}/absent`)), code(fs.readFile(`${doc}/../escape`)), code(fs.readFile(`${doc}/sub`)),
      code(fs.readdir(`${doc}/a.txt`)), code(fs.rm(`${doc}/sub`)), code(fs.rm(doc)), code(fs.rename(`${doc}/a.txt`, `${doc}/c.txt`)),
      code(fs.readFile('doc:/999999/a.txt')), code(globalThis.exact.documents.files(read).writeFile(`${doc}/a.txt`, new Uint8Array([1]))),
      // Review B6: writing the chosen folder itself, or a folder in it, is EISDIR, as on Hermes.
      code(fs.writeFile(doc, new Uint8Array([1]))), code(fs.appendFile(doc, new Uint8Array([1]))), code(fs.writeFile(`${doc}/sub`, new Uint8Array([1]))),
    ])).toEqual(['Unavailable ENOENT', 'Unavailable denied', 'Unavailable EISDIR', 'Unavailable ENOTDIR', 'Unavailable ENOTEMPTY',
      'Unavailable failed', 'Unavailable failed', 'Unavailable failed', 'Unavailable denied', 'Unavailable EISDIR', 'Unavailable EISDIR', 'Unavailable EISDIR']);
    // A Rust source's storage request: the same operation, its bytes as base64.
    expect(await globalThis.exact.documents.run('fs.readFile', { path: `${doc}/a.txt` }, null, read)).toEqual({ base64: btoa('A') });
    await expect(globalThis.exact.documents.run('fs.writeFile', { path: `${doc}/a.txt` }, new Uint8Array([1]), read)).rejects.toThrow(/^denied: fs.writeFile .*needs `fs.write doc:\/`/);
  } finally { delete globalThis.exact; }
});

test('quoted source scopes keep their original lines and native tuples are inert in browsers', () => {
  const native = 'fs.read "C:\\\\Users\\\\With Space"';
  const app = 'fs.read "app:/data/with space"';
  const set = normalized(`${native}\n${app}`);
  expect(grantError(set)).toBeNull();
  expect(coversPath(set, 'fs.read', 'app:/data/with space/file')).toBe(true);
  expect(coversPath(set, 'fs.read', 'C:/Users/With Space/file')).toBe(false);
  expect(grantError(scopedGrantSet(set, native))).toBeNull();
  expect(grantError(scopedGrantSet(set, 'fs.read "C:/Users/With Space"'))).toContain('source scope');
  expect(grantError(scopedGrantSet(set, 'fs.read "app:/data/with\\u0020space"'))).toContain('source scope');
  expect(coversPath(scopedGrantSet(set, ''), 'fs.read', 'app:/data/with space/file')).toBe(false);
});

test('sealed native tuples still require valid source grammar and exact decoded components', () => {
  const seal = entries => {
    const set = { version: 1, entries, error: null };
    let hash = 0xcbf29ce484222325n;
    for (const byte of new TextEncoder().encode(JSON.stringify(set))) {
      hash ^= BigInt(byte); hash = BigInt.asUintN(64, hash * 0x100000001b3n);
    }
    return { ...set, seal: hash.toString(16).padStart(16, '0') };
  };
  for (const [source, component] of [
    ['fs.read "C:/safe" fs.write C:/', 'safe'],
    ['fs.read "C:/safe"', 'outside'],
    ['fs.read "C:/CON.txt"', 'CON.txt'],
    ['fs.read "C:/bad\\u007f"', 'bad\u007f'],
    ['fs.read "C:/bad\\uD800"', 'bad\ud800'],
    ['fs.read C:/bad\ud800', 'bad\ud800'],
    ['fs.read C:/bad\udc00', 'bad\udc00'],
  ]) expect(grantError(seal([[1, source, ['fs-read', 'win:C', component], null]]))).toBe('the grant set was not validated');
  const boundary = '💾'.repeat(127) + 'x';
  expect(grantError(normalized(`fs.read ${JSON.stringify(`C:/${boundary}`)}`))).toBeNull();
  expect(grantError(normalized(`fs.read ${JSON.stringify(`C:/${boundary}x`)}`))).not.toBeNull();
});

test('the production file command refuses a source outside the admitted fs.read prefix', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-files-admission-'));
  writeFileSync(resolve(dir, 'files.js'), readFileSync(resolve(ROOT, 'host/web-js/files.js'), 'utf8')
    .replace("from './rt.js'", "from './rt-stub.js'").replace("from './navigation.js'", "from './navigation-stub.js'"));
  writeFileSync(resolve(dir, 'rt-stub.js'), `export const journal=[],clock={now:0,agent:true},inflight={n:0},Hosts={},OnHooks={},Views=new Map(),data={appId:'test.files'};export const nextTicket=()=>1,viewId=()=>1;\n`);
  writeFileSync(resolve(dir, 'navigation-stub.js'), `export const reportPlace=()=> ['en','UTC','1'].join(String.fromCharCode(0));\n`);
  cpSync(resolve(ROOT, 'host/web-js/pointer.js'), resolve(dir, 'pointer.js'));
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(ROOT, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'").replaceAll("'../web/faults.js'", "'./faults.js'"));
  for (const name of ['grant-admission.js', 'faults.js', 'navigation.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
  const documentDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'document');
  Object.defineProperty(globalThis, 'document', { configurable: true, value: { getElementById: () => ({ localName: 'button', isConnected: true, getAttribute: () => null, dispatchEvent() {} }) } });
  globalThis.exact = {};
  try {
    const admission = await import(pathToFileURL(resolve(dir, 'admission.js')).href);
    admission.setAppGrantSet(normalized('fs.read app:/elsewhere'));
    const runtime = await import(pathToFileURL(resolve(dir, 'rt-stub.js')).href);
    await import(pathToFileURL(resolve(dir, 'files.js')).href);
    runtime.Hosts.saveFile('export', 'app:/data/notes.json', 'notes.json');
    expect(runtime.journal.at(-1)).toContain('outside the app\'s fs.read grants');
  } finally {
    if (documentDescriptor) Object.defineProperty(globalThis, 'document', documentDescriptor); else delete globalThis.document;
    delete globalThis.exact;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the web matcher consumes the Rust grammar corpus', () => {
  const cases = JSON.parse(readFileSync(new URL('./tests/fixtures/grants.json', import.meta.url)));
  for (const item of cases) {
    const set = normalized(item.spec);
    expect(!grantError(set), item.spec).toBe(item.ok);
    for (const [url, admitted] of item.fetch ?? []) expect(admitsNetwork(set, url, 'fetch'), `${item.spec}: ${url}`).toBe(admitted);
    for (const [cap, path, admitted] of item.fs ?? []) expect(coversPath(set, cap, path), `${item.spec}: ${path}`).toBe(admitted);
    for (const [cap, path] of item.nativeFs ?? []) expect(coversPath(set, cap, path), `${item.spec}: inert ${path}`).toBe(false);
  }
  expect(grantError(normalized('net.fetch\fhttps://api.example'))).toBeNull();
  expect(grantError(normalized('net.fetch\u2028https://api.example\n# paragraph\u2029separator'))).toBeNull();
  const portZero = normalized('net.fetch http://example.test:0');
  expect([grantError(portZero), admitsNetwork(portZero, 'http://example.test:0/x', 'fetch')]).toEqual([null, true]);
}, 15_000);

test('network grants are selected by the requested operation, not ws URL spelling', async () => {
  const fetchSet = normalized('net.fetch wss://socket.example');
  const socketSet = normalized('net.websocket wss://socket.example');
  expect([admitsNetwork(fetchSet, 'wss://socket.example/path', 'fetch'), admitsNetwork(fetchSet, 'wss://socket.example/path', 'websocket')]).toEqual([true, false]);
  expect([admitsNetwork(socketSet, 'wss://socket.example/path', 'fetch'), admitsNetwork(socketSet, 'wss://socket.example/path', 'websocket')]).toEqual([false, true]);
  await expect(fetchWith(fetchSet, 'wss://socket.example/path')).rejects.toMatchObject({ kind: 'Network' });
  await expect(fetchWith(socketSet, 'wss://socket.example/path')).rejects.toMatchObject({ kind: 'Refused', message: "outside the app's grants (net.fetch)" });
  const plain = await request({ method: 'GET', url: 'wss://socket.example/path', headers: [] }, { grantSet: fetchSet, controllers: new Set() });
  expect(plain.kind).toBe(1);
  const refused = await request({ method: 'GET', url: 'wss://socket.example/path', headers: [] }, { grantSet: socketSet, controllers: new Set() });
  expect([refused.kind, text(refused)]).toEqual([2, "outside the app's grants (net.fetch)"]);
});

test('parser-produced grant sets are deeply immutable and module declarations compare normalized forms', () => {
  const value = structuredClone(normalized('  net.fetch https://api.example\n\nsecret.keep token'));
  Object.freeze(value);
  const set = createGrantSet(value);
  expect([Object.isFrozen(set), Object.isFrozen(set.entries), Object.isFrozen(set.entries[0]), Object.isFrozen(set.entries[0][2])]).toEqual([true, true, true, true]);
  expect(() => set.entries.push([3, 'net.fetch https://evil.example', ['fetch', 'https', 'evil.example', 443], null])).toThrow();
  expect(sameGrantDeclaration(set, '\n net.fetch https://api.example\n  secret.keep token  \n')).toBe(true);
  expect(sameGrantDeclaration(set, '\u0085net.fetch https://api.example\n\u0085secret.keep token\u0085')).toBe(true);
  expect(sameGrantDeclaration(set, '\ufeffnet.fetch https://api.example\nsecret.keep token')).toBe(false);
  expect(sameGrantDeclaration(set, 'net.fetch https://evil.example\nsecret.keep token')).toBe(false);
  const wrongSeal = structuredClone(normalized('net.fetch https://api.example'));
  wrongSeal.seal = '0000000000000000';
  expect(grantError(wrongSeal)).toBe('the grant set was not validated');
});

test('a clean JS dist imports every lazy storage and document entry with its complete graph', async () => {
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-grants-dist-'));
  const built = spawnSync(process.execPath, ['host/web-js/build.mjs', 'fieldnotes', '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
  try {
    expect(built.status, built.stderr || built.stdout).toBe(0);
    globalThis.exact ??= {};
    for (const name of ['grant-admission.js', 'storage-fs.js', 'storage-sqlite.js', 'storage-request.js', 'picker-glue.js', 'documents-glue.js']) {
      await import(`${pathToFileURL(resolve(dist, name)).href}?built=${Date.now()}-${name}`);
    }
    for (const name of ['storage-worker.js', 'sqlite3.mjs', 'sqlite3.wasm']) expect(existsSync(resolve(dist, name)), name).toBe(true);
  } finally { rmSync(dist, { recursive: true, force: true }); }
}, 60_000);

test('a typescript.sources mount still builds after it moves', () => {
  // Bun's runtime transpiler cache keys a module by its text and keeps the
  // imports the mount resolver gave it: the same file at a new place must
  // not resolve back into the old one.
  const root = mkdtempSync(resolve(tmpdir(), 'exact ts mount # ')), dir = resolve(root, 'app'), dist = resolve(root, 'dist');
  const nonce = `${process.pid}-${Date.now()}`;
  mkdirSync(resolve(root, 'first'), { recursive: true }); mkdirSync(dir);
  // Past the size Bun's cache starts at (it skips small files).
  writeFileSync(resolve(root, 'first/word.ts'), `// ${nonce}\n${'// padding\n'.repeat(8000)}import { suffix } from './suffix.ts';\nexport const word = (text: string) => text + suffix;\n`);
  writeFileSync(resolve(root, 'first/suffix.ts'), `export const suffix = '!';\n`);
  const manifest = mount => JSON.stringify({ name: 'Mount probe', app: { id: 'test.mount-probe', name: 'Mount probe' }, host: { web: {} }, typescript: { sources: { lib: mount } } });
  writeFileSync(resolve(dir, 'app.contract'), `shape Result\n  value: string\ncomponent Probe\n  resource result = probe() as shape Result\n  view\n    text result.value testId="result"\n`);
  writeFileSync(resolve(dir, 'app.ts'), `import type { Answer, Sources } from './app.contract.d.ts';\nimport { word } from './lib/word.ts';\nexport const appId='test.mount-probe',grants='';\nconst sources: Sources = { probe: () => ({ value: word('hi') }) };\nexport const answer: Answer = (source, args, store, storage, native) => sources[source](args as never, store, storage, native) as never;\n`);
  const env = { ...process.env, EXACT_APP_DIR: dir };
  delete env.BUN_RUNTIME_TRANSPILER_CACHE_PATH;
  const build = () => spawnSync(process.execPath, ['host/web-js/build.mjs', 'mount-probe', '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8', env });
  try {
    writeFileSync(resolve(dir, 'app.json'), manifest('../first'));
    const first = build();
    expect(first.status, first.stderr || first.stdout).toBe(0);
    renameSync(resolve(root, 'first'), resolve(root, 'second'));
    writeFileSync(resolve(dir, 'app.json'), manifest('../second'));
    const second = build();
    expect(second.status, second.stderr || second.stdout).toBe(0);
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 60_000);

test('a built TypeScript source refuses fetch and storage without grants', async () => {
  if (!process.env.CHROME || !existsSync(process.env.CHROME)) return;
  let destinationHits = 0;
  const destination = Bun.serve({ port: 0, fetch() { destinationHits++; return new Response('raw browser fetch', { headers: { 'access-control-allow-origin': '*' } }); } });
  const dir = mkdtempSync(resolve(tmpdir(), 'exact ts fetch # ')), dist = resolve(dir, 'dist'), profile = resolve(dir, 'chrome');
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Grant probe', app: { id: 'test.grant-probe', name: 'Grant probe' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `shape Result\n  value: string\ncomponent Probe\n  resource result = probe() as shape Result\n  view\n    text result.value testId="result"\n`);
  // The app dependency must be rewritten; copied host modules must retain browser
  // globals. Rewriting both would make the generated appGlobal self-referential.
  writeFileSync(resolve(dir, 'fetch-request.ts'), `export async function attempt(_args:any,_store:any,storage:any){const results=[];try{await fetch(${JSON.stringify(destination.url.href)});results.push('raw browser fetch');}catch(error:any){results.push(error.name+':'+error.kind);}for(const call of [()=>storage.fs.readFile('app:/data/file'),()=>storage.sqlite.open('app:/data/test.db')]){try{await call();results.push('unexpected storage');}catch(error:any){results.push(error.kind+':'+error.code);}}return {value:results.join('|')};}\n`);
  writeFileSync(resolve(dir, 'app.ts'), `import type { Answer, Sources } from './app.contract.d.ts';\nimport { attempt } from './fetch-request.ts';\nexport const appId='test.grant-probe',grants='';\nconst sources: Sources = { probe: attempt };\nexport const answer: Answer = (source, args, store, storage, native) => sources[source](args as never, store, storage, native) as never;\n`);
  const built = spawnSync(process.execPath, ['host/web-js/build.mjs', 'grant-probe', '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir } });
  let page, child, cdp, exited;
  try {
    expect(built.status, built.stderr || built.stdout).toBe(0);
    page = Bun.serve({ port: 0, async fetch(request) {
      const name = new URL(request.url).pathname === '/' ? 'index.html' : decodeURIComponent(new URL(request.url).pathname.slice(1));
      const file = resolve(dist, name);
      if (!file.startsWith(dist + sep) || !existsSync(file)) return new Response('not found', { status: 404 });
      return new Response(Bun.file(file));
    } });
    child = spawn(process.env.CHROME, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--no-first-run', '--disable-background-networking', `--user-data-dir=${profile}`, 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
    cdp = new Cdp(child.stdio[3], child.stdio[4]);
    exited = new Promise(resolveExit => child.on('exit', () => { cdp.fail('browser closed'); resolveExit(); }));
    const { targetInfos } = await cdp.send('Target.getTargets');
    const target = targetInfos.find(info => info.type === 'page') ?? await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: target.targetId, flatten: true });
    const call = (method, params = {}) => cdp.send(method, params, sessionId);
    await call('Page.enable');
    await call('Page.navigate', { url: page.url.href });
    const result = await call('Runtime.evaluate', { expression: `(async()=>{for(let i=0;i<120;i++){await new Promise(r=>requestAnimationFrame(r));const value=document.querySelector('[data-testid="result"]')?.textContent;if(value)return value;}return document.body.innerText;})()`, returnByValue: true, awaitPromise: true });
    expect(result.exceptionDetails).toBeUndefined();
    expect(result.result.value).toBe('FetchError:Refused|Unavailable:denied|Unavailable:denied');
    expect(existsSync(resolve(dist, 'storage-fs.js'))).toBe(false);
    expect(existsSync(resolve(dist, 'storage-sqlite.js'))).toBe(false);
    expect(destinationHits).toBe(0);
  } finally {
    if (child?.pid) { try { if (process.platform === 'win32') child.kill(); else process.kill(-child.pid, 'SIGKILL'); } catch {} await exited; }
    page?.stop(true); destination.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 60_000);

// @ref LLP 1108 D6 R2 — the page's `exactBodyFrom` read counts against the
// request's deadline and nothing is sent after it; a WebSocket takes no body.
test('a body read from a file is bounded by the deadline, and a socket refuses one', async () => {
  const grants = normalized('fs.read app:/data\nnet.fetch https://x.test\nnet.websocket wss://x.test');
  let sent = 0;
  const fetchBefore = globalThis.fetch;
  globalThis.fetch = async () => { sent++; return new Response('sent'); };
  try {
    const host = { grantSet: grants, controllers: new Set(), bodyFile: () => new Promise(r => setTimeout(() => r(new Blob(['late'])), 100)) };
    const op = { op: 'request', ticket: 1, method: 'POST', url: 'https://x.test/up', headers: [], body: '', bodyFrom: 'app:/data/p.jpg', cache: 'default', timeoutMs: 10 };
    const late = await request(op, host);
    expect([late.kind, text(late)]).toEqual([10, 'the request timed out after 10 ms']);
    // A read that returns in time but holds the event loop past the deadline: no timer fires first, the clock decides.
    const blocking = { ...host, bodyFile: async () => { const end = performance.now() + 30; while (performance.now() < end); return new Blob(['held']); } };
    const held = await request({ ...op, timeoutMs: 5 }, blocking);
    expect([held.kind, text(held)]).toEqual([10, 'the request timed out after 5 ms']);
    // An already-aborted request reads nothing.
    let reads = 0;
    const aborted = new AbortController(); aborted.abort();
    const gone = await request({ ...op, timeoutMs: undefined }, { ...host, controller: aborted, bodyFile: async () => { reads++; return new Blob(['x']); } });
    expect([gone.kind, reads]).toEqual([4, 0]);
    const socket = await request({ ...op, url: 'wss://x.test/feed', stream: true, timeoutMs: undefined }, host);
    expect([socket.kind, text(socket)]).toEqual([2, 'exactBodyFrom: a WebSocket sends no body']);
    const traversal = await request({ ...op, bodyFrom: 'app:/data/../tmp/p.jpg', timeoutMs: undefined }, host);
    expect(text(traversal)).toContain('no . or .. segment');
    await new Promise(r => setTimeout(r, 150));
    expect(sent).toBe(0);
    // The JS target carries the deadline's instant into fetchWith, which checks it after its grant work.
    const lateJs = await fetchWith(normalized('net.fetch https://x.test'), 'https://x.test/up', { method: 'POST', exactTimeout: 1000 }, { at: performance.now() - 1, ms: 5 }).catch(e => e);
    expect([lateJs.kind, lateJs.message]).toEqual(['Timeout', 'the request timed out after 5 ms']);
  } finally { globalThis.fetch = fetchBefore; }
});

// @ref LLP 1108 D6 R2 — an `exactBodyFrom` fetch on the JS target whose signal
// is already aborted rejects with its reason, reads nothing, and leaves no
// rejection unhandled (Astra, post-landing).
test('a pre-aborted exactBodyFrom fetch rejects once, reads nothing, and leaves nothing unhandled', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-fetch-abort-'));
  writeFileSync(resolve(dir, 'ts-fetch.js'), readFileSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), 'utf8'));
  writeFileSync(resolve(dir, 'admission.js'), 'export class FetchError extends Error { constructor(kind, message) { super(message); this.kind = kind; } } export const coversPath = () => { globalThis.bodyReads = (globalThis.bodyReads ?? 0) + 1; return true; }; export const fetchWith = async () => { globalThis.sentBodies = (globalThis.sentBodies ?? 0) + 1; return new Response(); }; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n');
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = {};\n');
  const unhandled = [];
  const listen = reason => unhandled.push(reason);
  process.on('unhandledRejection', listen);
  try {
    const { fetch: appFetch } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    const aborted = new AbortController(); aborted.abort(new Error('gone'));
    const error = await appFetch('https://x.test/up', { method: 'POST', exactBodyFrom: 'app:/data/p.jpg', signal: aborted.signal, exactTimeout: 1000 }).catch(e => e);
    await new Promise(r => setTimeout(r, 20));
    expect([error.message, globalThis.bodyReads ?? 0, globalThis.sentBodies ?? 0, unhandled.length]).toEqual(['gone', 0, 0, 0]);
  } finally { process.off('unhandledRejection', listen); rmSync(dir, { recursive: true, force: true }); }
});

// @ref LLP 1108 D6 R2 — `exactBodyFrom` on the JS target: the app file is the
// request's body, read from the page's store as a Blob (a picked entry is its
// own File), with no Content-Type but the author's; a missing file and a path
// outside `fs.read` reject the fetch before anything is sent, and fetch's own
// refusals are TypeErrors.
test('exactBodyFrom sends an app file from the page\'s store, a picked File too', async () => {
  if (!process.env.CHROME || !existsSync(process.env.CHROME)) return;
  const received = [];
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type', 'access-control-allow-methods': 'POST' };
  const destination = Bun.serve({ port: 0, async fetch(request) {
    if (request.method === 'OPTIONS') return new Response(null, { headers: cors });
    const body = new Uint8Array(await request.arrayBuffer());
    received.push({ type: request.headers.get('content-type'), sha: new Bun.CryptoHasher('sha256').update(body).digest('hex'), length: body.length });
    return new Response('got ' + body.length, { headers: cors });
  } });
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-body-from-')), dist = resolve(dir, 'dist'), profile = resolve(dir, 'chrome');
  const bytes = 'new Uint8Array(300000).map((_, i) => (Math.imul(i, 2654435761) >>> 13) & 255)';
  const expected = new Bun.CryptoHasher('sha256').update(new Uint8Array(300000).map((_, i) => (Math.imul(i, 2654435761) >>> 13) & 255)).digest('hex');
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Upload probe', app: { id: 'test.upload-probe', name: 'Upload probe' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `shape Result\n  value: string\ncomponent Probe\n  resource result = upload() as shape Result\n  view\n    text result.value testId="result"\n`);
  writeFileSync(resolve(dir, 'app.ts'), `import type { Answer, Sources } from './app.contract.d.ts';
export const appId = 'test.upload-probe', grants = 'fs.read app:/data\\nfs.write app:/data\\nnet.fetch ${destination.url.origin}';
const url = ${JSON.stringify(destination.url.href + 'upload')};
const sources: Sources = { upload: async (_args, _store, storage) => {
  await storage!.fs.atomicWriteFile('app:/data/photo.bin', ${bytes});
  const out: string[] = [];
  const steps: [string, any][] = [
    ['typed', { method: 'POST', headers: { 'content-type': 'image/jpeg' }, exactBodyFrom: 'app:/data/photo.bin' }],
    ['picked', { method: 'POST', exactBodyFrom: 'app:/data/picked.bin' }],
    ['denied', { method: 'POST', exactBodyFrom: 'app:/tmp/photo.bin' }],
    ['get', { exactBodyFrom: 'app:/data/photo.bin' }],
    ['both', { method: 'POST', body: 'x', exactBodyFrom: 'app:/data/photo.bin' }],
  ];
  for (const [name, init] of steps) {
    try { const r: any = await fetch(url, init); out.push(name + ' ' + r.status + ' ' + await r.text()); }
    catch (e: any) { out.push(name + ' ' + e.name + ':' + (e.kind ?? '') + ' ' + e.message); }
  }
  return { value: out.join('|') };
} };
export const answer: Answer = (source, args, store, storage, native) => sources[source](args as never, store, storage, native) as never;
`);
  const built = spawnSync(process.execPath, ['host/web-js/build.mjs', 'upload-probe', '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir } });
  let page, child, cdp, exited;
  try {
    expect(built.status, built.stderr || built.stdout).toBe(0);
    page = Bun.serve({ port: 0, async fetch(request) {
      const name = new URL(request.url).pathname === '/' ? 'index.html' : decodeURIComponent(new URL(request.url).pathname.slice(1));
      const file = resolve(dist, name);
      if (!file.startsWith(dist + sep) || !existsSync(file)) return new Response('not found', { status: 404 });
      return new Response(Bun.file(file));
    } });
    child = spawn(process.env.CHROME, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--no-first-run', '--disable-background-networking', `--user-data-dir=${profile}`, 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
    cdp = new Cdp(child.stdio[3], child.stdio[4]);
    exited = new Promise(resolveExit => child.on('exit', () => { cdp.fail('browser closed'); resolveExit(); }));
    const { targetInfos } = await cdp.send('Target.getTargets');
    const target = targetInfos.find(info => info.type === 'page') ?? await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: target.targetId, flatten: true });
    const call = (method, params = {}) => cdp.send(method, params, sessionId);
    await call('Page.enable');
    const result = async () => (await call('Runtime.evaluate', { expression: `(async()=>{for(let i=0;i<240;i++){await new Promise(r=>requestAnimationFrame(r));const value=document.querySelector('[data-testid="result"]')?.textContent;if(value)return value;}return document.body.innerText;})()`, returnByValue: true, awaitPromise: true })).result.value;
    await call('Page.navigate', { url: page.url.href });
    const first = (await result()).split('|');
    expect(first[0]).toBe('typed 200 got 300000');
    expect(first[1]).toStartWith('picked FetchError:Refused exactBodyFrom app:/data/picked.bin: no such file');
    expect(first[2]).toStartWith('denied FetchError:Refused exactBodyFrom app:/tmp/photo.bin: denied: fs.read app:/tmp/photo.bin');
    expect(first.slice(3)).toEqual(['get TypeError: fetch: a GET request cannot have a body', 'both TypeError: fetch: a request has one body: body or exactBodyFrom']);
    expect(received).toEqual([{ type: 'image/jpeg', sha: expected, length: 300000 }]);
    // A picked file's entry, the browser's own File with its type (LLP 1069.002 D5): sent untyped, as its bytes.
    const put = await call('Runtime.evaluate', { expression: `(async()=>{const {createFileStore}=await import('/storage-fs.js');const s=createFileStore('test.upload-probe');await s.putBlob('app:/data/picked.bin',new File([${bytes}],'picked.jpg',{type:'image/jpeg'}));s.close();return 'put';})()`, returnByValue: true, awaitPromise: true });
    expect(put.result.value).toBe('put');
    await call('Page.reload');
    const second = (await result()).split('|');
    expect(second.slice(0, 2)).toEqual(['typed 200 got 300000', 'picked 200 got 300000']);
    expect(received.slice(1)).toEqual([{ type: 'image/jpeg', sha: expected, length: 300000 }, { type: null, sha: expected, length: 300000 }]);
  } finally {
    if (child?.pid) { try { if (process.platform === 'win32') child.kill(); else process.kill(-child.pid, 'SIGKILL'); } catch {} await exited; }
    page?.stop(true); destination.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 60_000);

// @ref LLP 1069.002 D7 — an `image` shows a file the app keeps in `app:/data`
// on the JS target, bound or literal, and again after the page reloads
// (recipes F9, gallery F7): the web host's file store, as an object URL.
test('an app:/data image shows from the page\'s store, after a reload too', async () => {
  if (!process.env.CHROME || !existsSync(process.env.CHROME)) return;
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-app-image-')), dist = resolve(dir, 'dist'), profile = resolve(dir, 'chrome');
  const png = [137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,6,0,0,0,31,21,196,137,0,0,0,13,73,68,65,84,120,218,99,252,207,192,80,15,0,4,133,1,128,132,169,140,33,0,0,0,0,73,69,78,68,174,66,96,130];
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Image probe', app: { id: 'test.image-probe', name: 'Image probe' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `shape Photo\n  path: string\n  how: string\ncomponent Probe\n  resource photo = photo() as shape Photo\n  view\n    column\n      text photo.how testId="how"\n      image photo.path width=4 height=4 testId="bound"\n      when photo.how != ""\n        image "app:/data/p.png" width=4 height=4 testId="literal"\n`);
  writeFileSync(resolve(dir, 'app.ts'), `import type { Answer, Sources } from './app.contract.d.ts';\nexport const appId = 'test.image-probe', grants = 'fs.read app:/data\\nfs.write app:/data';\nconst P = 'app:/data/p.png';\nconst sources: Sources = { photo: async (_args, _store, storage) => {\n  try { await storage!.fs.stat(P); return { path: P, how: 'found' }; } catch {}\n  await storage!.fs.atomicWriteFile(P, new Uint8Array([${png}]));\n  return { path: P, how: 'written' };\n} };\nexport const answer: Answer = (source, args, store, storage, native) => sources[source](args as never, store, storage, native) as never;\n`);
  const built = spawnSync(process.execPath, ['host/web-js/build.mjs', 'image-probe', '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir } });
  let page, child, cdp, exited;
  try {
    expect(built.status, built.stderr || built.stdout).toBe(0);
    page = Bun.serve({ port: 0, async fetch(request) {
      const name = new URL(request.url).pathname === '/' ? 'index.html' : decodeURIComponent(new URL(request.url).pathname.slice(1));
      const file = resolve(dist, name);
      if (!file.startsWith(dist + sep) || !existsSync(file)) return new Response('not found', { status: 404 });
      return new Response(Bun.file(file));
    } });
    child = spawn(process.env.CHROME, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--no-first-run', '--disable-background-networking', `--user-data-dir=${profile}`, 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
    cdp = new Cdp(child.stdio[3], child.stdio[4]);
    exited = new Promise(resolveExit => child.on('exit', () => { cdp.fail('browser closed'); resolveExit(); }));
    const { targetInfos } = await cdp.send('Target.getTargets');
    const target = targetInfos.find(info => info.type === 'page') ?? await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: target.targetId, flatten: true });
    const call = (method, params = {}) => cdp.send(method, params, sessionId);
    await call('Page.enable');
    // Each image's `src` (an object URL, never `app:`) once both have decoded, and what the source did.
    const shown = async () => (await call('Runtime.evaluate', { expression: `(async()=>{const img=id=>document.querySelector('[data-testid="'+id+'"]');for(let i=0;i<240;i++){await new Promise(r=>requestAnimationFrame(r));const how=img('how')?.textContent,a=img('bound'),b=img('literal');if(how&&a?.complete&&a.naturalWidth&&b?.complete&&b.naturalWidth)return [how,a.src.slice(0,5),b.src.slice(0,5),a.naturalWidth];}return document.body.innerHTML;})()`, returnByValue: true, awaitPromise: true })).result.value;
    await call('Page.navigate', { url: page.url.href });
    expect(await shown()).toEqual(['written', 'blob:', 'blob:', 1]);
    await call('Page.reload');
    expect(await shown()).toEqual(['found', 'blob:', 'blob:', 1]);
  } finally {
    if (child?.pid) { try { if (process.platform === 'win32') child.kill(); else process.kill(-child.pid, 'SIGKILL'); } catch {} await exited; }
    page?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 60_000);

test("glue.js's grants arm keeps auth lines from a malformed I/O declaration and freezes the set", async () => {
  const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
  const arm = source.match(/case "grants": \{[^]*?break; \} case "auth":/)[0].replace(/ case "auth":$/, '');
  const run = new Function('op', 'createGrantSet', 'rawGrantText', 'grantError', `
    let grantSet = null, grants = [], unparsed = "", authHost = null;
    const loads = [], afterNativePaint = () => Promise.resolve();
    const loadAfterPaint = file => { loads.push(file); return Promise.resolve({ file }); };
    switch (op.op) { ${arm} }
    return Promise.resolve(authHost).then(() => ({ grantSet, grants, unparsed, loads }));
  `);
  const set = normalized('auth.session https://login.example\nsecret.keep camelCase');
  const state = await run({ op: 'grants', set }, createGrantSet, set => set.entries.map(entry => entry[1]).join('\n'), grantError);
  expect(state.unparsed).toContain('line 2');
  expect(state.grants).toContain('auth.session https://login.example');
  expect(state.loads).toEqual(['./auth-glue.js']);
  expect([Object.isFrozen(state.grantSet), Object.isFrozen(state.grantSet.entries)]).toEqual([true, true]);
});

test("glue.js surface admission uses a valid raw child line even when another I/O line is malformed", () => {
  const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
  const declaration = source.match(/function surfaceGranted\(op\) \{[^]*?\n\}/)[0];
  const admitted = new Function('grants', 'op', `${declaration};return surfaceGranted(op);`);
  const grants = ['secret.keep camelCase', 'surface.read world'];
  expect(admitted(grants, { mode: 'capture', name: 'world', scope: 'surface.read world' })).toBe(true);
  expect(admitted(grants, { mode: 'capture', name: 'world', scope: null })).toBe(true);
  expect(admitted(grants, { mode: 'capture', name: 'world', scope: 'surface.read elsewhere' })).toBe(false);
});

test('rust-data decodes an ABI child scope before the production executor refuses its parent-granted origin', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-rust-data-'));
  const grants = 'net.fetch https://api.example\nnet.fetch https://outside.example\nsecret.keep token', set = normalized(grants);
  for (const name of ['rust-data.js', 'admission.js']) {
    const source = readFileSync(resolve(ROOT, 'host/web-js', name), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'").replaceAll("'../web/faults.js'", "'./faults.js'");
    writeFileSync(resolve(dir, name), source);
  }
  for (const name of ['http-body.js', 'grant-admission.js', 'faults.js', 'navigation.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
  writeFileSync(resolve(dir, 'admission-data.js'), `import {createGrantSet} from './admission.js';export const rustGrantSet=createGrantSet(${JSON.stringify(set)}),tsGrantSet=createGrantSet(${JSON.stringify(normalized(''))});\n`);
  const memory = new WebAssembly.Memory({ initial: 1 }), out = 32768;
  let output = new Uint8Array();
  const response = fill => {
    const bytes = [], u8 = value => bytes.push(value), u32 = value => bytes.push(value & 255, value >>> 8 & 255, value >>> 16 & 255, value >>> 24 & 255);
    const string = value => { const encoded = new TextEncoder().encode(value); u32(encoded.length); bytes.push(...encoded); };
    u32(3); fill({ u8, u32, string }); output = Uint8Array.from(bytes); new Uint8Array(memory.buffer, out, output.length).set(output);
  };
  const exports = {
    memory, exact_logic_abi: () => 3, exact_logic_create: () => 1, exact_logic_alloc: () => 0, exact_logic_dealloc() {},
    exact_logic_output: () => out, exact_logic_output_len: () => output.length,
    exact_logic_call(_session, pointer) {
      const code = new Uint8Array(memory.buffer)[pointer + 4];
      if (code === 0) response(w => { w.u8(0); w.string('test.rust'); w.string('  net.fetch https://api.example\n net.fetch https://outside.example\n\n secret.keep token  '); });
      else if (code === 1 || code === 2) response(w => { w.u8(0); w.u8(0); w.u8(3); });
      else if (code === 3) response(w => {
        w.u8(2); w.u8(0); w.u32(0); w.u8(1); w.u8(1); w.string('net.fetch https://api.example'); w.string('GET');
        w.string('https://outside.example/private'); w.u32(0); w.u32(0);
      });
      else throw new Error(`unexpected logic call ${code}`);
      return 0;
    },
  };
  const instantiate = WebAssembly.instantiate;
  WebAssembly.instantiate = async () => ({ instance: { exports } });
  try {
    const module = await import(`${pathToFileURL(resolve(dir, 'rust-data.js')).href}?abi=${Date.now()}`);
    const data = { q: [] };
    await module.install(data, { probe: '' }, async () => new ArrayBuffer(0));
    const answer = data.answer('probe', [], { map: new Map(), set() {} });
    expect(answer.req).toMatchObject({ method: 'GET', url: 'https://outside.example/private', scope: 'net.fetch https://api.example' });
    expect(await data.fetch(answer.req)).toEqual({ failed: 2, message: "outside the app's grants (net.fetch)" });
    expect(data.grantSet).toBeUndefined();
  } finally {
    WebAssembly.instantiate = instantiate;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('ts-data installs the native Store facade and a later gpu-glue shader uses browser fetch', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-gpu-'));
  const set = normalized('secret.keep token');
  const app = resolve(dir, 'source.js');
  writeFileSync(app, `export const appId='test.ts';export const grants='secret.keep token';export function answer(name,args,store){return name==='kept'?String(store.get('exact.kept.answer')):store.get('token')}\n`);
  writeFileSync(resolve(dir, 'ts-data.js'), readFileSync(resolve(ROOT, 'host/web-js/ts-data.js'), 'utf8')
    .replace('__APP_TS__', pathToFileURL(app).href).replace('__AUTH_IMPORT__', '').replace('__AUTH_INSTALL__', '')
    .replace("from './rt.js'", "from './rt-stub.js'"));
  writeFileSync(resolve(dir, 'rt-stub.js'), `export const clock={agent:false,now:0},journal=[],Resources=[],inflight={n:0};export const checkpoint=()=>({kept:null});export const commit=f=>f();export const R=()=>{};export const painted=()=>Promise.resolve();\n`);
  writeFileSync(resolve(dir, 'names.js'), `export const sourceTypes={read:[[],'s'],kept:[[],'s']};\n`);
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(ROOT, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'").replaceAll("'../web/faults.js'", "'./faults.js'"));
  writeFileSync(resolve(dir, 'admission-data.js'), `import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(${JSON.stringify(set)});\n`);
  for (const name of ['grant-admission.js', 'faults.js', 'navigation.js', 'gpu-glue.js', 'gpu-assets.js', 'pace.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
  cpSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), resolve(dir, 'ts-fetch.js')); cpSync(resolve(ROOT, 'host/web-js/shape.js'), resolve(dir, 'shape.js'));
  writeFileSync(resolve(dir, 'gpu.js'), `export default async()=>{};export const gpu_load=async()=>{},gpu_shader_names=()=> '["shader"]',gpu_shaders_clear=()=>{},gpu_shader=()=>true,gpu_unload=()=>{},gpu_child_view=()=>{};\n`);
  const descriptors = Object.fromEntries(['fetch', 'document', 'window', 'requestAnimationFrame', 'cancelAnimationFrame', 'devicePixelRatio'].map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]));
  const shaderFetches = [], browserFetch = async input => { shaderFetches.push(String(input)); return new Response('shader'); };
  const document = { baseURI: pathToFileURL(resolve(dir, 'index.html')).href, hidden: false,
    head: { append() {} }, createElement: () => ({ style: {}, dataset: {}, set textContent(_) {} }), addEventListener() {} };
  Object.defineProperties(globalThis, {
    fetch: { configurable: true, writable: true, value: browserFetch }, document: { configurable: true, value: document },
    window: { configurable: true, value: { addEventListener() {} } }, requestAnimationFrame: { configurable: true, value: () => 1 },
    cancelAnimationFrame: { configurable: true, value() {} }, devicePixelRatio: { configurable: true, value: 1 },
  });
  globalThis.exact = { devAssets: null, root: { dataset: {} }, views: new Map(), pendingSurfaces: [], generation: 0 };
  try {
    const ts = await import(`${pathToFileURL(resolve(dir, 'ts-data.js')).href}?ts=${Date.now()}`), data = { q: [] };
    ts.install(data);
    expect(data.answer('read', [], new Map([['token', 'value']]))).toEqual({ v: 'value', store: true });
    expect(data.answer('kept', [], new Map([['exact.kept.answer', 'private']]))).toEqual({ v: 'null', store: false });
    expect(data.grantSet).toBeUndefined();
    await import(`${pathToFileURL(resolve(dir, 'gpu-glue.js')).href}?late=${Date.now()}`);
    expect(shaderFetches.some(url => url.endsWith('/shaders/shader.wgsl'))).toBe(true);
    expect(globalThis.fetch).toBe(browserFetch);
  } finally {
    for (const [name, descriptor] of Object.entries(descriptors)) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor); else delete globalThis[name];
    }
    delete globalThis.exact;
    rmSync(dir, { recursive: true, force: true });
  }
});

// The JS target checks a TypeScript answer as Hermes does (js/value's
// `decode_tree`), with Hermes's message (ledger F11).
test('ts-data refuses an answer outside its shape as Hermes does', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-shape-'));
  const app = resolve(dir, 'source.js');
  writeFileSync(app, `export const appId='test.shape';export const grants='';
const day = t => ({ id: 'd', transactions: [t] });
const answers = {
  spread: { days: [day({ id: 'a', amount: 1, cents: 100 })], note: null },
  missing: { days: [day({ id: 'a' })], note: null },
  absent: { days: [day({ id: 'a', amount: 1 })], note: undefined },
  nan: { days: [day({ id: 'a', amount: NaN })], note: null },
  kind: { days: 'none', note: null },
  dropped: { days: [day({ id: 'a', amount: 1, later: undefined, f() {} })], note: 'n' },
};
import { fetch } from './ts-fetch.js';
export function answer(name, args, store, storage) {
  // A socket the grants do not name: its end, mapped (LLP 1016.000).
  if (name === 'stream') return fetch('ws://127.0.0.1:9/feed', { exactStream: e => e.type + ' ' + e.kind + ': ' + e.message });
  if (name === 'read') return storage.fs.readFile('app:/data/x').then(() => 'read', e => e.kind + ' ' + e.code + ' ' + e.message);
  return name === 'later' ? Promise.resolve(answers.spread) : answers[args[0]];
}
`);
  writeFileSync(resolve(dir, 'ts-data.js'), readFileSync(resolve(ROOT, 'host/web-js/ts-data.js'), 'utf8')
    .replace('__APP_TS__', pathToFileURL(app).href).replace('__AUTH_IMPORT__', '').replace('__AUTH_INSTALL__', '')
    .replace("from './rt.js'", "from './rt-stub.js'"));
  writeFileSync(resolve(dir, 'rt-stub.js'), `export const clock={agent:false,now:0},journal=[],Resources=[],inflight={n:0};export const checkpoint=()=>({kept:null});export const commit=f=>f();export const R=()=>{};export const painted=()=>Promise.resolve();\n`);
  const ledger = '{"days":["[",{"id":"s","transactions":["[",{"id":"s","amount":"n"}]}],"note":["?","s"]}';
  writeFileSync(resolve(dir, 'names.js'), `export const sourceTypes={ledger:[["s"],${ledger}],later:[[],${ledger}],read:[[],"s"],stream:[[],"s"]};\n`);
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(ROOT, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'").replaceAll("'../web/faults.js'", "'./faults.js'"));
  writeFileSync(resolve(dir, 'admission-data.js'), `import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(${JSON.stringify(normalized('fs.read app:/data'))});\n`);
  for (const name of ['grant-admission.js', 'faults.js', 'navigation.js', 'storage-environment.js', 'http-body.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
  for (const name of ['ts-fetch.js', 'ts-stream.js', 'shape.js']) cpSync(resolve(ROOT, 'host/web-js', name), resolve(dir, name));
  try {
    const ts = await import(`${pathToFileURL(resolve(dir, 'ts-data.js')).href}?shape=${Date.now()}`), data = { q: [] };
    ts.install(data);
    const refusal = which => { try { data.answer('ledger', [which], new Map()); return null; } catch (e) { return [e.kind, e.message]; } };
    const outside = '`ledger` answered outside its shape: ';
    expect(refusal('spread')).toEqual(['Unavailable', outside + 'field `days`: field `transactions`: field `cents` is not in the shape']);
    expect(refusal('missing')).toEqual(['Unavailable', outside + 'field `days`: field `transactions`: field `amount` is missing']);
    expect(refusal('absent')).toEqual(['Unavailable', outside + 'field `note` is missing']);
    expect(refusal('nan')).toEqual(['Unavailable', outside + 'field `days`: field `transactions`: field `amount`: expected a number, got null']);
    expect(refusal('kind')).toEqual(['Unavailable', outside + 'field `days`: expected an array, got a string']);
    // What `JSON.stringify` leaves out is not there for Hermes either.
    expect(data.answer('ledger', ['dropped'], new Map()).v).toEqual([[['d', [['a', 1]]]], 'n']);
    await expect(data.answer('later', [], new Map()).promise).rejects.toThrow('`later` answered outside its shape: field `days`: field `transactions`: field `cents` is not in the shape');
    // A storage refusal's code is Hermes's (kanban F28): a drive with no scratch store.
    globalThis.location = { href: 'http://localhost/?agent' };
    expect(await data.answer('read', [], new Map()).promise).toBe('Unavailable agent storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)');
    // A stream is the answer's, opened by the runtime, admitted as the wasm host admits it: a socket by
    // `net.websocket` (x2apps dash diary: refused as `net.fetch`). Outside an answer it says where to start one.
    const stream = data.answer('stream', [], new Map());
    expect(await stream.stream(() => {}, new AbortController())).toEqual({ v: "error Refused: outside the app's grants (net.websocket)" });
    const { fetch } = await import(pathToFileURL(resolve(dir, 'ts-fetch.js')).href);
    await expect(fetch('wss://a.example', { exactStream: e => e })).rejects.toThrow('before the answer\'s first await');
  } finally {
    delete globalThis.location;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the web build retains every ambient diagnostic with its source location', () => {
  // Diagnostic paths can resemble warnings or Cargo's progress lines.
  for (const file of ['logic clock.ts', 'warning clock.ts', 'Compiling clock.ts']) {
    const dir = mkdtempSync(resolve(tmpdir(), 'exact web diagnostics-'));
    try {
      writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Diagnostics', app: { id: 'test.exact.diagnostics', name: 'Diagnostics' }, rust: false }));
      writeFileSync(resolve(dir, 'app.contract'), 'component App\n  resource message = message() as shape string\n  view\n    text message\n');
      writeFileSync(resolve(dir, 'app.ts'), `import { prefix } from './${file}';\nexport const appId = 'test.exact.diagnostics';\nexport const grants = '';\nexport const answer: import('./app.contract.d.ts').Answer = () => prefix;\n`);
      writeFileSync(resolve(dir, file), [
        "export const prefix = 'hello';",
        'export const a = () => Date.now();',
        'export const b = () => globalThis.setTimeout(() => {}, 1);',
        "export const c = () => Date['now']();",
        'export const d = () => Math[`random`]();',
        '',
      ].join('\n'));
      const built = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), 'diagnostics', '--render', 'none'], {
        cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir, EXACT_WEB_DIST: resolve(dir, 'dist') },
      });
      expect(built.status, built.stderr).toBe(1);
      for (const [line, api] of [[2, 'Date.now()'], [3, 'setTimeout()'], [4, 'Date.now()'], [5, 'Math.random()']]) {
        expect(built.stderr, built.stderr).toContain(`${file}:${line}:24: ${api} is unavailable in data sources`);
      }
      expect(built.stderr).toContain('the web build (the JS target) failed');
    } finally { rmSync(dir, { recursive: true, force: true }); }
  }
}, 60_000);

// LLP 1027.000 D3 on the JS target: the app's modules get guarded bindings
// in place of the page's clock, Math.random and timers, injected as the web
// build injects them, and Hermes's fixture of aliases refuses with Hermes's words.
test("the JS target refuses a data module's clock, randomness and timers as Hermes does", async () => {
  const { transformSync } = await import('rolldown/utils');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-inputs-'));
  const build = readFileSync(resolve(ROOT, 'host/web-js/build.mjs'), 'utf8');
  const bound = JSON.parse(/const bound = (\[[^\]]*\]);/.exec(build)[1].replaceAll("'", '"').replace(/\s+/g, ''));
  const guards = resolve(dir, 'ts-fetch.js');
  cpSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), guards);
  writeFileSync(resolve(dir, 'admission.js'), 'export const fetchWith = () => Promise.reject(new Error("no fetch here")); export class FetchError extends Error {} export const coversPath = () => false; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n');
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = null;\n');
  const fixture = resolve(ROOT, 'js/tests/fixtures/inputs.ts');
  const app = transformSync(fixture, readFileSync(fixture, 'utf8'), { inject: { ...Object.fromEntries(bound.map(name => [name, [guards, name]])),
    ...Object.fromEntries(['globalThis', 'window', 'self'].map(name => [name, [guards, 'appGlobal']])) } });
  expect(app.errors).toEqual([]);
  writeFileSync(resolve(dir, 'app.js'), app.code);
  const page = { Date: globalThis.Date, random: Math.random };
  try {
    await import(`${pathToFileURL(resolve(dir, 'app.js')).href}?inputs=${Date.now()}`);
    const { answer } = globalThis.exact;
    const forms = { now: 'Date.now()', new: 'new Date()', call: 'Date()', 'call-with-arg': 'Date()', random: 'Math.random()',
      'alias-now': 'Date.now()', 'alias-random': 'Math.random()', 'alias-date': 'new Date()', 'prototype-constructor': 'new Date()',
      'computed-now': 'Date.now()', 'computed-random': 'Math.random()', 'bound-now': 'Date.now()', 'bound-new': 'new Date()', reflect: 'new Date()',
      'intl-format': 'Intl.DateTimeFormat.format()', 'intl-format-undefined': 'Intl.DateTimeFormat.format()',
      'intl-parts': 'Intl.DateTimeFormat.formatToParts()', 'intl-parts-undefined': 'Intl.DateTimeFormat.formatToParts()',
      'intl-format-alias': 'Intl.DateTimeFormat.format()', 'intl-format-alias-undefined': 'Intl.DateTimeFormat.format()',
      'intl-parts-alias': 'Intl.DateTimeFormat.formatToParts()', 'intl-parts-alias-undefined': 'Intl.DateTimeFormat.formatToParts()',
      'intl-format-getter': 'Intl.DateTimeFormat.format()', 'intl-format-computed': 'Intl.DateTimeFormat.format()',
      'intl-parts-prototype': 'Intl.DateTimeFormat.formatToParts()', timeout: 'setTimeout()', interval: 'setInterval()',
      'computed-timeout': 'setTimeout()', frame: 'requestAnimationFrame()', performance: 'performance.now()' };
    for (const [form, api] of Object.entries(forms)) {
      const atInit = answer('atInit', [form]);
      expect(atInit.startsWith(api) && atInit.includes('as an argument'), `${form} at initialization: ${atInit}`).toBe(true);
      expect(() => answer('ambient', [form]), form).toThrow(api);
    }
    // Explicit inputs keep the language's behavior.
    expect(answer('explicit', [86_400_000, 7])).toBe('1970-01-02T00:00:00.000Z/' + ((Math.imul(7, 1664525) + 1013904223) >>> 0));
    expect(answer('utc', [])).toBe('2024-02-29T12:34:56.789Z/12/1709210096789/true');
    expect(answer('intl', [0])).toBe('1970/1970/1970/1970');
    // The page's own are untouched.
    expect([globalThis.Date, Math.random, typeof Date.now()]).toEqual([page.Date, page.random, 'number']);
  } finally {
    delete globalThis.exact;
    rmSync(dir, { recursive: true, force: true });
  }
});

// LLP 1016.000 D3 on the JS target: a data module's own socket, request or
// event source refuses with the wasm target's words (module-glue.js), in every
// spelling, so no frame leaves past the grants (#126).
test("the JS target refuses a data module's own WebSocket, XMLHttpRequest and EventSource as the wasm target does", async () => {
  const { transformSync } = await import('rolldown/utils');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-io-'));
  const build = readFileSync(resolve(ROOT, 'host/web-js/build.mjs'), 'utf8');
  const bound = JSON.parse(/const bound = (\[[^\]]*\]);/.exec(build)[1].replaceAll("'", '"').replace(/\s+/g, ''));
  const guards = resolve(dir, 'ts-fetch.js');
  cpSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), guards);
  writeFileSync(resolve(dir, 'admission.js'), 'export const fetchWith = () => Promise.reject(new Error("no fetch here")); export class FetchError extends Error {} export const coversPath = () => false; export const overlaying = { on: false }; export const inOverlay = api => new Error(api);\n');
  writeFileSync(resolve(dir, 'admission-data.js'), 'export const tsGrantSet = null;\n');
  const source = resolve(dir, 'source.js');
  writeFileSync(source, `const io = { WebSocket, XMLHttpRequest, EventSource };
const forms = {
  bare: name => ({ WebSocket: () => new WebSocket('ws://127.0.0.1:9/x'), XMLHttpRequest: () => new XMLHttpRequest(), EventSource: () => new EventSource('http://127.0.0.1:9/x') })[name](),
  global: name => new globalThis[name]('ws://127.0.0.1:9/x'), window: name => new window[name]('ws://127.0.0.1:9/x'),
  self: name => new self[name]('ws://127.0.0.1:9/x'), alias: name => new io[name]('ws://127.0.0.1:9/x'),
  call: name => globalThis[name]('ws://127.0.0.1:9/x'), reflect: name => Reflect.construct(globalThis[name], ['ws://127.0.0.1:9/x']),
};
globalThis.exact = { answer: (form, name) => { try { forms[form](name); return 'opened'; } catch (e) { return e.message; } } };
`);
  const app = transformSync(source, readFileSync(source, 'utf8'), { inject: { ...Object.fromEntries(bound.map(name => [name, [guards, name]])),
    ...Object.fromEntries(['globalThis', 'window', 'self'].map(name => [name, [guards, 'appGlobal']])) } });
  expect(app.errors).toEqual([]);
  writeFileSync(resolve(dir, 'app.js'), app.code);
  const page = globalThis.WebSocket;
  try {
    await import(`${pathToFileURL(resolve(dir, 'app.js')).href}?io=${Date.now()}`);
    const { answer } = globalThis.exact;
    for (const name of ['WebSocket', 'XMLHttpRequest', 'EventSource'])
      for (const form of ['bare', 'global', 'window', 'self', 'alias', 'call', 'reflect'])
        expect(answer(form, name), `${form} ${name}`).toBe(`${name} is unavailable in data sources`);
    // The page's own, which the runtime's stream opens, is untouched.
    expect(globalThis.WebSocket).toBe(page);
  } finally {
    delete globalThis.exact;
    rmSync(dir, { recursive: true, force: true });
  }
});

test('module-glue prepare accepts normalized formatting and exact-only grants', async () => {
  const set = createGrantSet(structuredClone(normalized('net.fetch https://api.example\nauth.session https://login.example')));
  const formatted = ' net.fetch https://api.example\n\n  auth.session https://login.example  ';
  const win = { Error, Promise, structuredClone, crypto: { subtle: {} }, indexedDB: null, addEventListener() {},
    exact: { abi: 1, appId: 'test.module', grants: formatted, answer() {} }, __exact_install_storage() {},
    document: { createElement: () => ({}), head: { append() {} } } };
  const frame = { hidden: false, contentWindow: win, setAttribute() {}, remove() {} };
  const descriptors = Object.fromEntries(['fetch', 'document', 'location'].map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]));
  Object.defineProperties(globalThis, {
    fetch: { configurable: true, writable: true, value: async () => { const response = new Response(''); Object.defineProperty(response, 'url', { value: 'http://127.0.0.1/module-prelude.js' }); return response; } },
    document: { configurable: true, value: { createElement: () => frame, body: { append() {} } } },
    location: { configurable: true, value: { href: 'http://127.0.0.1/module', origin: 'http://127.0.0.1' } },
  });
  globalThis.exact = { moduleDigest: async () => 'a'.repeat(64) };
  const script = new TextEncoder().encode('module');
  const receipt = new TextEncoder().encode(JSON.stringify({ version: 1, abi: 1, appId: 'test.module', grants: formatted,
    web: { file: 'app.js', bytes: script.length, sha256: 'a'.repeat(64) }, module: { sha256: 'b'.repeat(64) } }));
  try {
    const { call, prepare } = await import(`./module-glue.js?prepare=${Date.now()}`);
    const realm = await prepare({ receipt, script }, { appId: 'test.module', grants: 'net.fetch https://api.example\nauth.session https://login.example', grantSet: set, placement: 'main' });
    expect(realm.meta.grants).toBe(formatted);
    expect(call({ op: 'activate', id: realm.id, appId: 'test.module', grants: 'net.fetch https://api.example\nauth.session https://login.example', revision: 'b'.repeat(64) })).toEqual({ ok: true });
    realm.dispose();
  } finally {
    for (const [name, descriptor] of Object.entries(descriptors)) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor); else delete globalThis[name];
    }
    delete globalThis.exact;
  }
});

test('ts-data always supplies storage, refusing ungranted operations without loading adapters', async () => {
  const methods = ['readFile', 'writeFile', 'atomicWriteFile', 'appendFile', 'readdir', 'mkdir', 'rm', 'stat', 'rename', 'copyFile', 'realpath'];
  for (const spec of ['', 'net.fetch https://api.example']) for (const query of ['', '?agent=1', '?agent=1&storage=test']) {
    const locationDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'location');
    Object.defineProperty(globalThis, 'location', { configurable: true, value: { href: `http://localhost/${query}` } });
    const dir = mkdtempSync(resolve(tmpdir(), 'exact-no-storage-'));
    writeFileSync(resolve(dir, 'source.js'), `export const appId='test.no-storage';
export function answer(name, args, store, storage) {
  if (name === 'work') return storage.work(Promise.resolve('done'));
  if (name === 'document') return storage.fs.readFile('doc:/picked/file').catch(error => error.kind + ':' + error.code);
  if (name === 'directories') return JSON.stringify(storage.fs.directories);
  return (name === 'open' ? storage.sqlite.open('app:/data/test.db')
    : storage.fs[name]('app:/data/file', new Uint8Array([1])))
    .then(() => 'unexpected success', error => error.kind + ':' + error.code);
}`);
    writeFileSync(resolve(dir, 'ts-data.js'), readFileSync(resolve(ROOT, 'host/web-js/ts-data.js'), 'utf8')
      .replace('__APP_TS__', './source.js').replace('__AUTH_IMPORT__', '').replace('__AUTH_INSTALL__', ''));
    writeFileSync(resolve(dir, 'rt.js'), 'export const clock={now:0},journal=[],Resources=[],inflight={n:0};export const checkpoint=()=>({kept:null});export const commit=f=>f();export const R=()=>{};export const painted=()=>Promise.resolve();');
    writeFileSync(resolve(dir, 'names.js'), `export const sourceTypes=${JSON.stringify(Object.fromEntries([...methods, 'open', 'work', 'directories', 'document'].map(n => [n, [[], 's']])))};`);
    writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(ROOT, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'").replaceAll("'../web/faults.js'", "'./faults.js'"));
    writeFileSync(resolve(dir, 'admission-data.js'), `import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(${JSON.stringify(normalized(spec))});`);
    for (const name of ['grant-admission.js', 'faults.js', 'navigation.js', 'http-body.js', 'storage-environment.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
    cpSync(resolve(ROOT, 'host/web-js/ts-fetch.js'), resolve(dir, 'ts-fetch.js')); cpSync(resolve(ROOT, 'host/web-js/shape.js'), resolve(dir, 'shape.js'));
    // No storage adapters are installed: a grant refusal must not need them.
    try {
      const ts = await import(pathToFileURL(resolve(dir, 'ts-data.js')).href), data = { q: [] };
      ts.install(data);
      for (const name of [...methods, 'open']) expect(await data.answer(name, [], new Map()).promise, name).toBe(query === '?agent=1' ? 'Unavailable:agent' : 'Unavailable:denied');
      expect(await data.answer('document', [], new Map()).promise).toBe('Unavailable:denied');
      expect(await data.answer('work', [], new Map()).promise).toBe('done');
      expect(JSON.parse(data.answer('directories', [], new Map()).v)).toEqual({ data: 'app:/data', cache: 'app:/cache', temporary: 'app:/tmp' });
    } finally {
      if (locationDescriptor) Object.defineProperty(globalThis, 'location', locationDescriptor); else delete globalThis.location;
      rmSync(dir, { recursive: true, force: true });
    }
  }
});

// The page realm's storage hands an answer's operations to the background
// before it retires the answer's owner (LLP 1097 D7): a write still in flight
// when its answer replied resolves the app's promise through its re-pointed
// cell, and a completion already queued for the answer moves with it.
test('storage re-homes an answer\'s writes to the background before retiring it', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-rehome-'));
  try {
    cpSync(resolve(ROOT, 'host/web/storage.js'), resolve(dir, 'storage.js'));
    writeFileSync(resolve(dir, 'storage-environment.js'), "export const directories = {}; export const agentStorageRefusal = 'no store'; export const storageKey = () => 'k';");
    // Each write lands when the test says.
    writeFileSync(resolve(dir, 'storage-fs.js'), 'export const createFileSystem = () => ({ writeFile: () => new Promise(ok => globalThis.landWrite.push(ok)) });');
    globalThis.landWrite = [];
    const { createStorage } = await import(pathToFileURL(resolve(dir, 'storage.js')).href);
    const answer = {}, background = {};
    let owner = answer;
    const storage = createStorage({ Error, Promise, structuredClone }, { appId: 'test', grantSet: null }, () => owner, 'k');
    const results = [];
    const first = storage.capability.fs.writeFile('app:/data/a', 'one').then(() => results.push('first'));
    const second = storage.capability.fs.writeFile('app:/data/b', 'two').then(() => results.push('second'));
    for (let i = 0; i < 20 && globalThis.landWrite.length < 2; i++) await new Promise(r => setTimeout(r, 1));
    globalThis.landWrite.shift()(); // the first lands while the answer is current: queued for it
    await new Promise(r => setTimeout(r, 1));
    // The answer replies: its owner's storage becomes the background's.
    storage.rehome(answer, background);
    storage.retire(answer);
    owner = null;
    globalThis.landWrite.shift()(); // the second lands after its answer is gone
    await storage.deliver(background);
    await storage.deliver(background);
    await Promise.all([first, second]);
    expect(results).toEqual(['first', 'second']);
  } finally {
    delete globalThis.landWrite;
    rmSync(dir, { recursive: true, force: true });
  }
});

// A read queued behind a write the answer did not await is issued later, in
// the background round that delivers the write (LLP 1097 D7). The cell has
// to belong to the answer that asked, which is current when the read is
// accepted. Attributing it when the adapter runs attributes it to the
// background, and the parked answer's deliver waits forever.
test('a read queued behind a background write stays the answer\'s', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-queued-owner-'));
  const workerPath = resolve(dir, 'worker.mjs');
  writeFileSync(workerPath, `
import { readFileSync } from 'node:fs';
import * as api from './storage.js';
const realSetTimeout = setTimeout;
self.land = [];
const checkpoint = () => new Promise(resolve => {
  const channel = new MessageChannel();
  channel.port1.onmessage = () => { channel.port1.close(); channel.port2.close(); resolve(); };
  channel.port2.postMessage(null);
});
const wait = ms => new Promise(resolve => realSetTimeout(resolve, ms));
async function until(pred) {
  for (let i = 0; i < 50 && !pred(); i++) await wait(10);
}
let context = null;
const backgroundOwner = {};
self.__exact_host = op => {
  if (!context) throw new Error('host call outside an answer');
  if (op === 5) return;
  if (op === 13) return '1';
};
const storage = api.createStorage(self, { appId: 'test', grantSet: null }, () => context.owner, 'k');
if (typeof api.bindAnswerStorage === 'function') api.bindAnswerStorage(self, storage, () => context.owner);
eval(readFileSync(${JSON.stringify(resolve(ROOT, 'js/src/prelude.js'))}, 'utf8'));
self.__exact_storage = storage.capability;
self.__exact_install_storage();
self.__exact_main_thread();
self.exact = { abi: 1, appId: 'test', answer(name, _args, _store, cap) {
  if (name === 'write') { cap.fs.writeFile('app:/data/note', 'x'); return 'wrote'; }
  return cap.fs.readFile('app:/data/note');
}};
try {
  const writeOwner = {};
  context = { owner: writeOwner };
  const started = JSON.parse(self.__exact_call('write', '[]'));
  const wrote = JSON.parse(self.__exact_settle(String(started.call)));
  if (wrote.tag !== 0) throw new Error('write did not reply: ' + JSON.stringify(wrote));
  storage.rehome(writeOwner, backgroundOwner);
  storage.retire(writeOwner);
  context = null;
  await until(() => self.land.length >= 1);

  const readOwner = {};
  context = { owner: readOwner };
  const read = JSON.parse(self.__exact_call('read', '[]'));
  await checkpoint();
  const parked = JSON.parse(self.__exact_settle(String(read.call)));
  if (!parked.waiting) throw new Error('read did not park: ' + JSON.stringify(parked));
  context = null;

  context = { owner: backgroundOwner };
  self.__exact_enter_background();
  self.land.shift()();
  await checkpoint();
  if (!storage.deliverNow(backgroundOwner)) throw new Error('background write was not waiting');
  await checkpoint();
  await until(() => self.land.length >= 1);
  const pending = storage.deliver(readOwner);
  await checkpoint();
  let verdict = 'not-issued';
  if (self.land.length) {
    self.land.shift()();
    await checkpoint();
    verdict = await Promise.race([pending.then(() => 'answer', () => 'rejected'), wait(250).then(() => 'stuck')]);
  }
  postMessage(verdict);
} catch (error) {
  postMessage('error: ' + (error && error.stack || error));
}
`);
  writeFileSync(resolve(dir, 'storage-environment.js'), "export const directories = {}; export const agentStorageRefusal = 'no store'; export const storageKey = () => 'k';");
  writeFileSync(resolve(dir, 'storage-fs.js'), `export const createFileSystem = () => ({
  writeFile: () => new Promise(ok => globalThis.land.push(() => ok(''))),
  readFile: () => new Promise(ok => globalThis.land.push(() => ok('contents'))),
});`);
  cpSync(resolve(ROOT, 'host/web/storage.js'), resolve(dir, 'storage.js'));
  const worker = new Worker(pathToFileURL(workerPath), { type: 'module' });
  try {
    const verdict = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('worker hung')), 5000);
      worker.on('message', value => { clearTimeout(timer); resolve(value); });
      worker.on('error', error => { clearTimeout(timer); reject(error); });
    });
    expect(verdict).toBe('answer');
  } finally {
    await worker.terminate();
    rmSync(dir, { recursive: true, force: true });
  }
});

// A realm worker with the real prelude and a filesystem that lands when the
// test says. `body` posts one verdict.
async function realmVerdict(body) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-letgo-'));
  const prelude = JSON.stringify(resolve(ROOT, 'js/src/prelude.js'));
  writeFileSync(resolve(dir, 'worker.mjs'), `
import { readFileSync } from 'node:fs';
import * as api from './storage.js';
const realSetTimeout = setTimeout;
const checkpoint = () => new Promise(resolve => {
  const channel = new MessageChannel();
  channel.port1.onmessage = () => { channel.port1.close(); channel.port2.close(); resolve(); };
  channel.port2.postMessage(null);
});
const wait = ms => new Promise(resolve => realSetTimeout(resolve, ms));
async function until(pred) {
  for (let i = 0; i < 50 && !pred(); i++) await wait(10);
}
self.land = [];
self.writes = 0;
let context = null;
self.__exact_host = op => {
  if (!context) throw new Error('host call outside an answer');
  if (op === 5) return;
  if (op === 13) return '1';
};
const storage = api.createStorage(self, { appId: 'test', grantSet: null }, () => context.owner, 'k');
if (typeof api.bindAnswerStorage === 'function') api.bindAnswerStorage(self, storage, () => context.owner);
eval(readFileSync(${prelude}, 'utf8'));
self.__exact_storage = storage.capability;
self.__exact_install_storage();
self.__exact_main_thread();
const scratch = owner => ({ owner });
(async () => {
${body}
})();
`);
  writeFileSync(resolve(dir, 'storage-environment.js'), "export const directories = {}; export const agentStorageRefusal = 'no store'; export const storageKey = () => 'k';");
  writeFileSync(resolve(dir, 'storage-fs.js'), `export const createFileSystem = () => ({
  writeFile: () => new Promise(ok => { globalThis.writes++; globalThis.land.push(() => ok('')); }),
  readFile: () => new Promise(ok => globalThis.land.push(() => ok('contents'))),
});`);
  cpSync(resolve(ROOT, 'host/web/storage.js'), resolve(dir, 'storage.js'));
  const worker = new Worker(pathToFileURL(resolve(dir, 'worker.mjs')), { type: 'module' });
  try {
    return await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('worker hung')), 5000);
      worker.on('message', value => { clearTimeout(timer); resolve(value); });
      worker.on('error', error => { clearTimeout(timer); reject(error); });
    });
  } finally {
    await worker.terminate();
    rmSync(dir, { recursive: true, force: true });
  }
}

// A send between its own storage steps is superseded. The step in flight has
// to finish and the step chained on it has to run; retiring the owner drops
// the step and the module's later storage never starts (LLP 1097, ios7).
test('a superseded send between storage steps still writes on the wasm realm', async () => {
  const verdict = await realmVerdict(`
self.exact = { abi: 1, appId: 'test', answer(name, _a, _s, cap) {
  if (name === 'pair') return cap.fs.writeFile('app:/data/a', '1').then(() => cap.fs.writeFile('app:/data/b', '2')).then(() => 'pair');
  return cap.fs.readFile('app:/data/b');
}};
try {
  const owner = {};
  context = scratch(owner);
  const started = JSON.parse(self.__exact_call('pair', '[]'));
  await until(() => self.land.length >= 1);
  const owed = new Map();
  let pumping = false;
  const pump = async () => { while (pumping) { if (self.land.length) self.land.shift()(); await wait(1); } };
  if (typeof api.finishLetGo === 'function') {
    if (self.__exact_forget(String(started.call)) !== 'storage') throw new Error('forget did not keep the chain');
    owed.set(started.call, owner);
    pumping = true; pump();
    await api.finishLetGo(storage, owed, {
      letGo: () => self.__exact_let_go('', ''),
      disposed: () => false,
      deliver: async (at) => {
        const prev = context; context = scratch(at);
        try { await storage.deliver(at); await checkpoint(); }
        finally { context = prev; }
      },
    });
    pumping = false;
  } else {
    storage.retire(owner);
    self.__exact_forget(String(started.call));
    pumping = true; const running = pump();
    await wait(80); pumping = false; await running;
  }
  let verdict = 'dropped';
  if (self.writes >= 2) {
    context = scratch({});
    const read = JSON.parse(self.__exact_call('read', '[]'));
    await until(() => self.land.length >= 1);
    const pending = storage.deliver(context.owner);
    self.land.shift()();
    await checkpoint();
    const settled = JSON.parse(self.__exact_settle(String(read.call)));
    const delivered = await Promise.race([pending.then(() => 'answer', () => 'rejected'), wait(250).then(() => 'stuck')]);
    verdict = settled.tag === 0 && delivered === 'answer' ? 'answer' : 'stuck:' + JSON.stringify(settled) + ':' + delivered;
  }
  postMessage(verdict + ':' + self.writes);
} catch (error) {
  postMessage('error: ' + (error && error.stack || error));
}
`);
  expect(verdict).toBe('answer:2');
});

// Retire can still race a completion already in flight. That completion has
// to reject the promise the prelude is waiting on, so the head clears and
// the next operation is issued.
test('retiring an in-flight storage owner settles the prelude head', async () => {
  const verdict = await realmVerdict(`
self.exact = { abi: 1, appId: 'test', answer(name, _a, _s, cap) {
  if (name === 'lose') return cap.fs.writeFile('app:/data/a', '1');
  return cap.fs.readFile('app:/data/a');
}};
try {
  const owner = {};
  context = scratch(owner);
  const started = JSON.parse(self.__exact_call('lose', '[]'));
  await until(() => self.land.length >= 1);
  storage.retire(owner);
  self.land.shift()();
  await checkpoint();
  const lost = JSON.parse(self.__exact_settle(String(started.call)));
  if (lost.tag !== 2) { postMessage('stuck:' + JSON.stringify(lost)); return; }
  context = scratch({});
  const read = JSON.parse(self.__exact_call('read', '[]'));
  await checkpoint();
  await until(() => self.land.length >= 1);
  if (!self.land.length) { postMessage('not-issued'); return; }
  const pending = storage.deliver(context.owner);
  self.land.shift()();
  await checkpoint();
  const delivered = await Promise.race([pending.then(() => 'answer', () => 'rejected'), wait(250).then(() => 'stuck')]);
  const settled = JSON.parse(self.__exact_settle(String(read.call)));
  postMessage(delivered === 'answer' && settled.tag === 0 ? 'advanced' : 'stuck:' + delivered + ':' + JSON.stringify(settled));
} catch (error) {
  postMessage('error: ' + (error && error.stack || error));
}
`);
  expect(verdict).toBe('advanced');
});

test('a request deadline cancels a server that never answers, on the wasm and TypeScript paths', async () => {
  const origin = Bun.serve({ port: 0, idleTimeout: 0, fetch: () => new Promise(() => {}) });
  const set = normalized(`net.fetch ${origin.url.origin}`);
  try {
    const started = Date.now();
    const timed = await request({ method: 'GET', url: origin.url.href, headers: [], timeoutMs: 150 }, { grantSet: set, controllers: new Set() });
    expect([timed.kind, text(timed)]).toEqual([10, 'the request timed out after 150 ms']);
    const refused = await request({ method: 'GET', url: origin.url.href, headers: [], timeoutMs: 0 }, { grantSet: set, controllers: new Set() });
    expect(refused.kind).toBe(2);
    const error = await fetchWith(set, origin.url.href, { exactTimeout: 150 }).catch(e => e);
    expect([error.name, error.kind, error.message]).toEqual(['FetchError', 'Timeout', 'the request timed out after 150 ms']);
    expect(Date.now() - started).toBeLessThan(5000);
    await expect(fetchWith(set, origin.url.href, { exactTimeout: 1.5 })).rejects.toThrow('exactTimeout must be an integer number of milliseconds from 1 to 3600000');
  } finally { origin.stop(true); }
});

test('the web build\'s deadline covers a stalled body, and keeps the caller\'s own abort', async () => {
  // Headers at once, then a body that never ends.
  const origin = Bun.serve({ port: 0, idleTimeout: 0, fetch: () => new Response(new ReadableStream({ start(c) { c.enqueue(new TextEncoder().encode('partial')); } })) });
  const set = normalized(`net.fetch ${origin.url.origin}`);
  try {
    const stalled = await fetchWith(set, origin.url.href, { exactTimeout: 150 }).catch(e => e);
    expect([stalled.name, stalled.kind, stalled.message]).toEqual(['FetchError', 'Timeout', 'the request timed out after 150 ms']);
    // A bodyless status, and the response's own URL, come through a deadline.
    const empty = Bun.serve({ port: 0, fetch: () => new Response(null, { status: 204 }) });
    try {
      const done = await fetchWith(normalized(`net.fetch ${empty.url.origin}`), empty.url.href, { exactTimeout: 2000 });
      expect([done.status, done.url, await done.text()]).toEqual([204, empty.url.href, '']);
    } finally { empty.stop(true); }
    // A body that arrived in time stays readable after the deadline.
    const quick = Bun.serve({ port: 0, fetch: () => new Response('in time') });
    try {
      const arrived = await fetchWith(normalized(`net.fetch ${quick.url.origin}`), quick.url.href, { exactTimeout: 100 });
      await new Promise(r => setTimeout(r, 250));
      expect(await arrived.text()).toBe('in time');
    } finally { quick.stop(true); }
    const aborted = new AbortController(); aborted.abort();
    const own = await fetchWith(set, new Request(origin.url.href, { signal: aborted.signal }), { exactTimeout: 5000 }).catch(e => e);
    // The caller's own abort is the native executor's `Aborted` (513450bda, LLP 1109 D3), not a lost connection.
    expect([own.name, own.kind]).toEqual(['FetchError', 'Aborted']);
    const kept = await fetchWith(set, new Request(origin.url.href, { signal: aborted.signal }), { signal: undefined, exactTimeout: 5000 }).catch(e => e);
    expect(kept.kind).toBe('Aborted');
  } finally { origin.stop(true); }
});
