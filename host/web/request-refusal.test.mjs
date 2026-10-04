import { expect, test } from 'bun:test';
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { Cdp } from '../../scripts/agent.mjs';
import { request } from './http-body.js';
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
  const target = resolve(process.env.CARGO_TARGET_DIR || resolve(ROOT, 'target'), 'debug/exact-web-js');
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
      expect(redirected.message ?? text(redirected)).toBe("outside the app's grants (net.fetch)");
      const grantedRedirect = await run({ method: 'GET', url: `${origin.url}granted-redirect`, headers: [] });
      expect(grantedRedirect.failed ?? grantedRedirect.kind ?? 0).toBe(0);
      expect(new TextDecoder().decode(grantedRedirect.body)).toBe('granted');
    }
    expect(destinationHits).toBe(2);
  } finally { origin.stop(true); destination.stop(true); grantedDestination.stop(true); }
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
    expect([response.kind, text(response)]).toEqual([2, "outside the app's grants (net.fetch)"]);
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
    ])).toEqual(['Unavailable ENOENT', 'Unavailable denied', 'Unavailable EISDIR', 'Unavailable ENOTDIR', 'Unavailable ENOTEMPTY',
      'Unavailable failed', 'Unavailable failed', 'Unavailable failed', 'Unavailable denied']);
    // A Rust source's storage request: the same operation, its bytes as base64.
    expect(await globalThis.exact.documents.run('fs.readFile', { path: `${doc}/a.txt` }, null, read)).toEqual({ base64: btoa('A') });
    await expect(globalThis.exact.documents.run('fs.writeFile', { path: `${doc}/a.txt` }, new Uint8Array([1]), read)).rejects.toThrow(/^denied: fs.writeFile .*needs `fs.write doc:\/`/);
  } finally { delete globalThis.exact; }
});

test('the production file command refuses a source outside the admitted fs.read prefix', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-files-admission-'));
  writeFileSync(resolve(dir, 'files.js'), readFileSync(resolve(ROOT, 'host/web-js/files.js'), 'utf8')
    .replace("from './rt.js'", "from './rt-stub.js'").replace("from './navigation.js'", "from './navigation-stub.js'"));
  writeFileSync(resolve(dir, 'rt-stub.js'), `export const journal=[],clock={now:0,agent:true},inflight={n:0},Hosts={},OnHooks={},Views=new Map(),data={appId:'test.files'};export const nextTicket=()=>1,viewId=()=>1;\n`);
  writeFileSync(resolve(dir, 'navigation-stub.js'), `export const reportPlace=()=> ['en','UTC','1'].join(String.fromCharCode(0));\n`);
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(ROOT, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'"));
  for (const name of ['grant-admission.js', 'navigation.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
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
  }
  expect(grantError(normalized('net.fetch\fhttps://api.example'))).toBeNull();
  expect(grantError(normalized('net.fetch\u2028https://api.example\n# paragraph\u2029separator'))).toBeNull();
  const portZero = normalized('net.fetch http://example.test:0');
  expect([grantError(portZero), admitsNetwork(portZero, 'http://example.test:0/x', 'fetch')]).toEqual([null, true]);
});

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
    for (const name of ['navigation.js', 'storage-worker.js', 'sqlite3.mjs', 'sqlite3.wasm']) expect(existsSync(resolve(dist, name)), name).toBe(true);
  } finally { rmSync(dist, { recursive: true, force: true }); }
}, 60_000);

test('a built TypeScript source reaches fetch through the app grant binding', async () => {
  if (!process.env.CHROME || !existsSync(process.env.CHROME)) return;
  let destinationHits = 0;
  const destination = Bun.serve({ port: 0, fetch() { destinationHits++; return new Response('raw browser fetch', { headers: { 'access-control-allow-origin': '*' } }); } });
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-ts-fetch-build-')), dist = resolve(dir, 'dist'), profile = resolve(dir, 'chrome');
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Grant probe', app: { id: 'test.grant-probe', name: 'Grant probe' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `shape Result\n  value: string\ncomponent Probe\n  resource result = probe() as shape Result\n  view\n    text result.value testId="result"\n`);
  writeFileSync(resolve(dir, 'app.ts'), `export const appId='test.grant-probe',grants='';export async function answer(source){if(source!=='probe')return null;try{await fetch(${JSON.stringify(destination.url.href)});return {value:'raw browser fetch'};}catch(error){return {value:error.name+':'+error.kind};}}\n`);
  const built = spawnSync(process.execPath, ['host/web-js/build.mjs', 'grant-probe', '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: dir } });
  let page, child, cdp, exited;
  try {
    expect(built.status, built.stderr || built.stdout).toBe(0);
    page = Bun.serve({ port: 0, async fetch(request) {
      const name = new URL(request.url).pathname === '/' ? 'index.html' : decodeURIComponent(new URL(request.url).pathname.slice(1));
      const file = resolve(dist, name);
      if (!file.startsWith(dist + '/') || !existsSync(file)) return new Response('not found', { status: 404 });
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
    expect(result.result.value).toBe('FetchError:Refused');
    expect(destinationHits).toBe(0);
  } finally {
    if (child?.pid) { try { process.kill(-child.pid, 'SIGKILL'); } catch {} await exited; }
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
      if (!file.startsWith(dist + '/') || !existsSync(file)) return new Response('not found', { status: 404 });
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
    if (child?.pid) { try { process.kill(-child.pid, 'SIGKILL'); } catch {} await exited; }
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
    const source = readFileSync(resolve(ROOT, 'host/web-js', name), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'");
    writeFileSync(resolve(dir, name), source);
  }
  for (const name of ['http-body.js', 'grant-admission.js', 'navigation.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
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
  writeFileSync(resolve(dir, 'rt-stub.js'), `export const clock={agent:false,now:0},journal=[],Resources=[];export const checkpoint=()=>({kept:null});export const commit=f=>f();export const R=()=>{};export const painted=()=>Promise.resolve();\n`);
  writeFileSync(resolve(dir, 'names.js'), `export const sourceTypes={read:[[],'s'],kept:[[],'s']};\n`);
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(ROOT, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'"));
  writeFileSync(resolve(dir, 'admission-data.js'), `import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(${JSON.stringify(set)});\n`);
  for (const name of ['grant-admission.js', 'navigation.js', 'gpu-glue.js', 'gpu-assets.js', 'pace.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
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
export function answer(name, args, store, storage) {
  if (name === 'read') return storage.fs.readFile('app:/data/x').then(() => 'read', e => e.kind + ' ' + e.code + ' ' + e.message);
  return name === 'later' ? Promise.resolve(answers.spread) : answers[args[0]];
}
`);
  writeFileSync(resolve(dir, 'ts-data.js'), readFileSync(resolve(ROOT, 'host/web-js/ts-data.js'), 'utf8')
    .replace('__APP_TS__', pathToFileURL(app).href).replace('__AUTH_IMPORT__', '').replace('__AUTH_INSTALL__', '')
    .replace("from './rt.js'", "from './rt-stub.js'"));
  writeFileSync(resolve(dir, 'rt-stub.js'), `export const clock={agent:false,now:0},journal=[],Resources=[];export const checkpoint=()=>({kept:null});export const commit=f=>f();export const R=()=>{};export const painted=()=>Promise.resolve();\n`);
  const ledger = '{"days":["[",{"id":"s","transactions":["[",{"id":"s","amount":"n"}]}],"note":["?","s"]}';
  writeFileSync(resolve(dir, 'names.js'), `export const sourceTypes={ledger:[["s"],${ledger}],later:[[],${ledger}],read:[[],"s"]};\n`);
  writeFileSync(resolve(dir, 'admission.js'), readFileSync(resolve(ROOT, 'host/web-js/admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'"));
  writeFileSync(resolve(dir, 'admission-data.js'), `import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(${JSON.stringify(normalized('fs.read app:/data'))});\n`);
  for (const name of ['grant-admission.js', 'navigation.js', 'storage-environment.js']) cpSync(resolve(ROOT, 'host/web', name), resolve(dir, name));
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
  } finally {
    delete globalThis.location;
    rmSync(dir, { recursive: true, force: true });
  }
});

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
  writeFileSync(resolve(dir, 'admission.js'), 'export const fetchWith = () => Promise.reject(new Error("no fetch here"));\n');
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
