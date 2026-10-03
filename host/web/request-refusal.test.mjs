import { expect, test } from 'bun:test';
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
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
  try {
    await expect(fetchWith(set, 'https://outside.test/')).rejects.toMatchObject({ name: 'FetchError', kind: 'Refused', message: "outside the app's grants (net.fetch)" });
    await expect(fetchWith(set, 'http://[')).rejects.toMatchObject({ name: 'FetchError', kind: 'Network' });
    expect(await (await fetchWith(set, origin.url)).text()).toBe('ok');
    const scratch = mkdtempSync(resolve(tmpdir(), 'exact-late-host-')), module = resolve(scratch, 'late.mjs');
    writeFileSync(module, 'export default globalThis.fetch;\n');
    const lateHost = await import(pathToFileURL(module).href);
    rmSync(scratch, { recursive: true });
    expect(globalThis.fetch).toBe(browserFetch);
    expect(lateHost.default).toBe(browserFetch);
    origin.stop(true);
    await expect(fetchWith(set, origin.url)).rejects.toMatchObject({ name: 'FetchError', kind: 'Network' });
  } finally { origin.stop(true); }
});

test('a bodyless same-origin assets GET is host I/O on the JS target', async () => {
  const origin = Bun.serve({ port: 0, fetch(req) { return new Response(new URL(req.url).pathname); } });
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'location');
  Object.defineProperty(globalThis, 'location', { configurable: true, value: { href: origin.url.href, origin: origin.url.origin } });
  try {
    expect(await (await fetchWith(normalized(''), '/assets/runtime.wasm')).text()).toBe('/assets/runtime.wasm');
    await expect(fetchWith(normalized(''), '/assets/runtime.wasm', { method: 'POST' })).rejects.toMatchObject({ kind: 'Refused' });
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

test('the web matcher consumes the Rust grammar corpus', () => {
  const cases = JSON.parse(readFileSync(new URL('./tests/fixtures/grants.json', import.meta.url)));
  for (const item of cases) expect(!grantError(normalized(item.spec)), item.spec).toBe(item.ok);
  expect(grantError(normalized('net.fetch\fhttps://api.example'))).toBeNull();
});

test('parser-produced grant sets are deeply immutable and module declarations compare normalized forms', () => {
  const value = structuredClone(normalized('  net.fetch https://api.example\n\nsecret.keep token'));
  Object.freeze(value);
  const set = createGrantSet(value);
  expect([Object.isFrozen(set), Object.isFrozen(set.entries), Object.isFrozen(set.entries[0]), Object.isFrozen(set.entries[0][2])]).toEqual([true, true, true, true]);
  expect(() => set.entries.push([3, 'net.fetch https://evil.example', ['fetch', 'https', 'evil.example', 443], null])).toThrow();
  expect(sameGrantDeclaration(set, '\n net.fetch https://api.example\n  secret.keep token  \n')).toBe(true);
  expect(sameGrantDeclaration(set, 'net.fetch https://evil.example\nsecret.keep token')).toBe(false);
});

test('a clean JS dist imports every lazy storage and document entry with its complete graph', async () => {
  const dist = mkdtempSync(resolve(tmpdir(), 'exact-grants-dist-'));
  const built = spawnSync(process.execPath, ['host/web-js/build.mjs', 'fieldnotes', '--out', dist, '--render', 'none'], { cwd: ROOT, encoding: 'utf8' });
  try {
    expect(built.status, built.stderr || built.stdout).toBe(0);
    globalThis.exact ??= {};
    for (const name of ['grant-admission.js', 'storage-fs.js', 'storage-sqlite.js', 'picker-glue.js', 'documents-glue.js']) {
      await import(`${pathToFileURL(resolve(dist, name)).href}?built=${Date.now()}-${name}`);
    }
    for (const name of ['navigation.js', 'storage-worker.js', 'sqlite3.mjs', 'sqlite3.wasm']) expect(existsSync(resolve(dist, name)), name).toBe(true);
  } finally { rmSync(dist, { recursive: true, force: true }); }
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

test('rust-data decodes an ABI request before the production executor refuses its origin', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-rust-data-'));
  const grants = 'net.fetch https://api.example\nsecret.keep token', set = normalized(grants);
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
      if (code === 0) response(w => { w.u8(0); w.string('test.rust'); w.string('  net.fetch https://api.example\n\n secret.keep token  '); });
      else if (code === 1 || code === 2) response(w => { w.u8(0); w.u8(0); w.u8(3); });
      else if (code === 3) response(w => {
        w.u8(2); w.u8(0); w.u32(0); w.u8(1); w.u8(0); w.string(''); w.string('GET');
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
    expect(answer.req).toMatchObject({ method: 'GET', url: 'https://outside.example/private', scope: null });
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
  writeFileSync(resolve(dir, 'rt-stub.js'), `export const clock={agent:false,now:0},journal=[],Resources=[];export const checkpoint=()=>({kept:null});export const commit=f=>f();export const R=()=>{};\n`);
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
