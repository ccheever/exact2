// LLP 1103 on the web: the page's fault table, and the three places that
// consult it — the wasm host's `request`, the JS target's `fetchWith`, and
// `fetchEarly`, which starts no GET a fault will fail.
import { afterEach, expect, test } from 'bun:test';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { faultMatches, faultOp, faultsJson, parseFaults, setFaultLog, takeFault } from './faults.js';
import { request } from './http-body.js';
import { fetchWith } from '../web-js/admission.js';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
function normalized(spec) {
  const scratch = mkdtempSync(resolve(tmpdir(), 'exact-grants-'));
  const input = resolve(scratch, 'grants.txt');
  writeFileSync(input, spec);
  const target = resolve(process.env.CARGO_TARGET_DIR || resolve(ROOT, 'target'), 'debug/exact-web-js');
  const result = existsSync(target)
    ? spawnSync(target, ['normalize-grants', input], { cwd: ROOT, encoding: 'utf8' })
    : spawnSync('cargo', ['run', '-q', '-p', 'exact-web-js', '--', 'normalize-grants', input], { cwd: ROOT, encoding: 'utf8' });
  rmSync(scratch, { recursive: true });
  if (result.status !== 0) throw new Error(result.stderr || 'grant normalizer failed');
  return JSON.parse(result.stdout);
}
afterEach(() => { delete globalThis.__exactFaults; });

test('the longest live prefix decides, counts run out, pass keeps hits, and lines wait for a journal', () => {
  const lines = [];
  expect(faultOp({ fail: 'https://api.test/' }).faults).toHaveLength(1);
  faultOp({ fail: 'https://api.test/recipes', times: 1 });
  expect(faultMatches('https://api.test/recipes/3')).toBe(true);
  expect(faultsJson().find(f => f.times === 1).left).toBe(1); // matching does not count
  expect(takeFault('https://api.test/recipes/3')).toBe(true);
  expect(faultsJson().find(f => f.times === 1)).toMatchObject({ left: 0, hits: 1 });
  expect(takeFault('https://api.test/recipes/3')).toBe(true); // the shorter prefix, now
  expect(takeFault('https://other.test/')).toBe(false);
  setFaultLog(line => lines.push(line));
  expect(lines).toEqual(['fetch failed (driver fault): https://api.test/recipes/3', 'fetch failed (driver fault): https://api.test/recipes/3']);
  expect(faultOp({ pass: 'https://api.test/' }).faults.find(f => f.prefix === 'https://api.test/')).toMatchObject({ armed: false, hits: 1 });
  expect(takeFault('https://api.test/x')).toBe(false);
  expect(faultOp({ pass: 'https://nothing.test/' }).error).toContain('no fault was armed');
  expect(faultOp({ fail: 'x', times: 0 }).error).toContain('positive integer');
  expect(faultOp({ fail: '' }).error).toContain('prefix');
  expect(parseFaults('https://a.test/\nhttps://b.test/\t2\nhttps://c.test/\t1\t0\t1\t0')).toEqual([
    { prefix: 'https://a.test/', times: null, left: null, hits: 0, armed: true },
    { prefix: 'https://b.test/', times: 2, left: 2, hits: 0, armed: true },
    { prefix: 'https://c.test/', times: 1, left: 0, hits: 1, armed: false },
  ]);
  expect(() => parseFaults('x\t0')).toThrow('positive integer');
});

test('a faulted request on the wasm host and the JS target fails as a refused connection and never goes out', async () => {
  let hits = 0;
  const origin = Bun.serve({ port: 0, fetch() { hits++; return new Response('ok'); } });
  const set = normalized(`net.fetch ${origin.url.origin}`);
  try {
    faultOp({ fail: `${origin.url.origin}/recipes`, times: 2 });
    const r = await request({ method: 'GET', url: `${origin.url}recipes/1`, headers: [] }, { grantSet: set, controllers: new Set() });
    expect([r.kind, new TextDecoder().decode(r.body)]).toEqual([1, `fetch failed (driver fault): ${origin.url}recipes/1`]);
    const error = await fetchWith(set, `${origin.url}recipes/2`).catch(e => e);
    expect([error.name, error.kind, error.message]).toEqual(['FetchError', 'Network', `fetch failed (driver fault): ${origin.url}recipes/2`]);
    expect(hits).toBe(0);
    // Spent: the next goes out, and so does another prefix.
    expect((await fetchWith(set, `${origin.url}recipes/3`)).status).toBe(200);
    const other = await request({ method: 'GET', url: `${origin.url}users`, headers: [] }, { grantSet: set, controllers: new Set() });
    expect(other.status).toBe(200);
    expect(hits).toBe(2);
    // An ungranted fetch is still Refused, not faulted: the grant check comes first.
    faultOp({ fail: 'https://outside.test/' });
    expect((await fetchWith(set, 'https://outside.test/x').catch(e => e)).kind).toBe('Refused');
  } finally { origin.stop(true); }
});

test('fetchEarly starts no GET a fault will fail, and counts nothing', async () => {
  globalThis.exact ??= {};
  const { fetchEarly } = await import(`./module-glue.js?faults=${Date.now()}`);
  const set = normalized('net.fetch https://api.test');
  faultOp({ fail: 'https://api.test/recipes', times: 1 });
  expect(fetchEarly({ method: 'GET', url: 'https://api.test/recipes/1', headers: [] }, set)).toBeNull();
  expect(faultsJson()[0]).toMatchObject({ left: 1, hits: 0 });
});

test('a GET fetchEarly sent before its fault was armed is not failed by it', async () => {
  globalThis.exact ??= {};
  const { claim, fetchEarly } = await import(`./module-glue.js?claimed=${Date.now()}`);
  let hits = 0;
  const origin = Bun.serve({ port: 0, fetch() { hits++; return new Response('ok'); } });
  const set = normalized(`net.fetch ${origin.url.origin}`);
  try {
    const req = { method: 'GET', url: `${origin.url}recipes/1`, headers: [] };
    expect(fetchEarly(req, set)).toBeFunction();
    faultOp({ fail: `${origin.url.origin}/recipes`, times: 1 }); // armed after the GET left
    const r = await request(req, { grantSet: set, controllers: new Set(), moduleLoader: { claim } });
    expect([r.kind, r.status]).toEqual([0, 200]);
    expect(faultsJson()[0]).toMatchObject({ left: 1, hits: 0 });
    expect(hits).toBe(1);
  } finally { origin.stop(true); }
});
