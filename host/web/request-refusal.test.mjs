import { expect, test } from 'bun:test';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { request } from './http-body.js';
import { deferredFulfill, refusal } from './navigation.js';
import { admitsNetwork, coversPath, grantError, scopedGrantSet } from './grant-admission.js';
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
  const origin = Bun.serve({ port: 0, fetch(req) {
    if (new URL(req.url).pathname === '/redirect') return new Response(null, { status: 307, headers: { location: `${destination.url}private` } });
    return new Response('ok');
  } });
  const set = normalized(`net.fetch ${origin.url.origin}`);
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
      expect(redirected.failed ?? redirected.kind).toBe(1);
    }
    expect(destinationHits).toBe(0);
  } finally { origin.stop(true); destination.stop(true); }
});

test('the wasm early request uses the same set and handles a redirect rejection immediately', async () => {
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
    const response = claim(request.url, { method: 'GET', headers: [], redirect: 'error', cache: 'default' });
    await expect(response).rejects.toBeTruthy();
    expect(destinationHits).toBe(0);
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
});
