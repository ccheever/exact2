import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { request } from './http-body.js';
import { deferredFulfill, refusal } from './navigation.js';
const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');

test('native requests admit the source scope before calling the module and bound UTF-8 replies', async () => {
  const results = [];
  let calls = 0, reply = 'é'.repeat(600_000), rejects = false;
  const loadPageNative = async () => ({ later: async () => { calls++; if (rejects) throw Error(reply); return reply; } });
  const apply = async op => { const r = await request(op, { grants: ['fs.read app:/data'], granted: () => false, loadPageNative }); results.push(r); };
  const op = { op: 'request', ticket: 1, method: 'POST', url: 'exact-native:', headers: {}, body: btoa('{}'), maxResponseBytes: 1024 * 1024 };
  await apply({ ...op, scope: 'fs.write app:/data' }, 2);
  expect(calls).toBe(0);
  expect(results.pop()?.kind).toBe(2);
  for (const fail of [false, true]) {
    rejects = fail;
    await apply({ ...op, scope: 'fs.read app:/data' }, 2);
    expect(results.pop()?.kind).toBe(2);
  }
  rejects = false; reply = 'é'.repeat(512 * 1024);
  await apply(op, 2);
  expect(results.pop()?.body.length).toBe(1024 * 1024);
});
test('admission refusal delivers Refused only after the enclosing batch, with its incarnation', async () => {
  const delivered = [], inflight = new Set(), defer = deferredFulfill((...args) => delivered.push(args), inflight);
  defer(...refusal({ op: 'refuse', ticket: 17, message: 'only HTTP may opt into independent transport' }, 4));
  expect(delivered.length).toBe(0);
  expect(inflight.size).toBe(1); // a settle waits for it
  await Promise.resolve();
  expect(delivered.length).toBe(1);
  expect(delivered[0].slice(0, 5)).toEqual([4, 17, 2, 0, '']);
  expect(new TextDecoder().decode(delivered[0][5])).toBe('only HTTP may opt into independent transport');
});

test('ordinary and early fetch refuse redirects before an ungranted POST destination is reached', async () => {
  const moduleSource = readFileSync(new URL('./module-glue.js', import.meta.url), 'utf8');
  const earlySource = moduleSource.slice(moduleSource.indexOf('const early ='), moduleSource.indexOf('export async function baked'));
  let hits = 0;
  const destination = Bun.serve({ port: 0, fetch() { hits++; return new Response('secret'); } });
  const origin = Bun.serve({ port: 0, fetch(req) {
    if (new URL(req.url).pathname === '/direct') return new Response('ok');
    return new Response(null, { status: 307, headers: { Location: `${destination.url}private` } });
  } });
  const controllers = new Set(), results = [];
  const granted = url => new URL(url).origin === origin.url.origin;
  const apply = async (op, incarnation) => {
    const r = await request(op, { grants: [`net.fetch ${origin.url.origin}`], granted, controllers, moduleLoader: null });
    results.push([incarnation, op.ticket, r.kind, r.status, r.headers, r.body]);
  };
  try {
    await apply({ op: 'request', ticket: 1, method: 'POST', url: `${origin.url}redirect`, headers: {}, body: btoa('private body') }, 1);
    expect(results[0][2]).toBe(1);
    expect(hits).toBe(0);
    await apply({ op: 'request', ticket: 2, method: 'GET', url: `${origin.url}direct`, headers: {} }, 1);
    expect(results[1][2]).toBe(0);
    expect(new TextDecoder().decode(results[1][5])).toBe('ok');
    const early = Function(`${earlySource.replace('export function claim', 'function claim')}; return { fetchEarly, claim };`)();
    const url = `${origin.url}redirect`;
    early.fetchEarly({ method: 'GET', url, headers: {} }, `net.fetch ${origin.url.origin}`);
    const promise = early.claim(url, { method: 'GET', headers: {}, redirect: 'error', cache: 'default' });
    expect(promise).not.toBeNull();
    await expect(promise).rejects.toThrow();
    expect(hits).toBe(0);
    // Grants that did not parse admit no early fetch, whatever the module's own lines say.
    expect(early.fetchEarly({ method: 'GET', url: `${origin.url}direct`, headers: {} }, `net.fetch ${origin.url.origin}`, 'the app\'s grants did not parse: line 2')).toBeNull();
    expect(early.claim(`${origin.url}direct`, { method: 'GET', headers: {}, redirect: 'error', cache: 'default' })).toBeNull();
  } finally { origin.stop(true); destination.stop(true); }
});

// Crew's set (`secret.keep crewHost`, the port report of 2026-09-24, F1) does not parse. The Rust host's one
// parse hands the page no lines and the reason (`grants-unparsed`, src/batch_tests.rs), so `granted()` admits
// nothing and each refusal names the line, as a native host's does.
test('grants that do not parse admit no fetch, and the refusal says why', async () => {
  const why = "the app's grants did not parse: line 2: `crewHost` is not a secret name ([a-z0-9._-]{1,64})";
  let hits = 0;
  const origin = Bun.serve({ port: 0, fetch() { hits++; return new Response('ok'); } });
  const taking = source.slice(source.indexOf('grants = op.lines;'), source.indexOf('if (grants.some('));
  const page = Function(`let grants = [], unparsed = ""; const console = { warn() {} };
    ${source.slice(source.indexOf('function grantAdmits'), source.indexOf('function surfaceGranted'))}
    return { take(op) { ${taking} }, host: () => ({ grants, granted, unparsed, controllers: new Set(), moduleLoader: null }) };`)();
  const get = { op: 'request', ticket: 1, method: 'GET', url: `${origin.url}session`, headers: {} };
  const text = r => new TextDecoder().decode(r.body);
  try {
    page.take({ op: 'grants', lines: [], error: why });
    const refused = await request(get, page.host());
    expect([refused.kind, text(refused), hits]).toEqual([2, `refused by grant: ${get.url}: ${why}`, 0]);
    // The same origin in a set that parses is admitted: the refusal above was the set's.
    page.take({ op: 'grants', lines: [`net.fetch ${origin.url.origin}`, 'secret.keep crew.host'] });
    const admitted = await request(get, page.host());
    expect([admitted.kind, text(admitted), hits]).toEqual([0, 'ok', 1]);
  } finally { origin.stop(true); }
});

// LLP 1054.000 R5: glue.js and module-glue.js apply ibex2's rule (vendor/ibex2
// patch 1): an origin matched whole, or `scheme://*.domain`, every host
// strictly under one domain of two labels or more.
test('a subdomain grant admits hosts under its domain and nothing else, in both copies', () => {
  const lift = (text, from, to) => Function(`${text.slice(text.indexOf(from), text.indexOf(to))}; return grantAdmits;`)();
  const moduleSource = readFileSync(new URL('./module-glue.js', import.meta.url), 'utf8');
  const copies = [lift(source, 'function grantAdmits', 'function granted('), lift(moduleSource, 'function grantAdmits', 'function fetchEarly')];
  const cases = [
    ['https://*.host.bsky.network', 'https://morel.us-east.host.bsky.network/xrpc/x', true],
    ['https://*.host.bsky.network', 'https://A.Host.Bsky.Network/x', true],
    ['https://*.host.bsky.network', 'https://host.bsky.network/x', false],
    ['https://*.host.bsky.network', 'https://evilhost.bsky.network/x', false],
    ['https://*.host.bsky.network', 'https://a.host.bsky.network.evil.com/x', false],
    ['https://*.host.bsky.network', 'http://a.host.bsky.network/x', false],
    ['https://*.host.bsky.network', 'https://a.host.bsky.network:8443/x', false],
    ['https://*.com', 'https://a.com/', false],
    ['https://a.*.example.com', 'https://a.b.example.com/', false],
    ['https://*.127.0.0.1', 'https://1.127.0.0.1/', false],
    ['https://bsky.social', 'https://bsky.social/x', true],
    ['https://bsky.social', 'https://x.bsky.social/x', false],
    ['https://*.example.com:8443', 'https://a.example.com:8443/', true],
    ['custom://EXAMPLE.com:90', 'custom://example.COM:90', true],
    ['custom://*.127.0.0.1:90', 'custom://sub.127.0.0.1:90', true],
    ['https://*.example.com?', 'https://a.example.com/', false],
  ];
  for (const admits of copies) for (const [grant, url, want] of cases) expect([grant, url, admits(grant, url)]).toEqual([grant, url, want]);
});

test('JS and native grant grammar share valid URL normalization and whole-set refusals', async () => {
  const { parseGrants, installGrants } = await import('./http-body.js');
  const cases = JSON.parse(readFileSync(new URL('./tests/fixtures/grants.json', import.meta.url)));
  for (const item of cases) {
    const set = parseGrants(item.spec);
    expect(!set.error, item.spec).toBe(item.ok);
    for (const [url, allowed] of item.fetch ?? []) expect(set.permits(url), `${item.spec}: ${url}`).toBe(allowed);
    if (!item.ok) { expect(set.lines).toEqual([]); expect(set.secrets.size).toBe(0); expect(set.permits('https://ok.test')).toBe(false); }
  }
  const data = {}, ts = installGrants(data, 'typescript', 'secret.keep ts.token\nnet.fetch https://ts.test');
  const rust = installGrants(data, 'rust', 'secret.keep rust.token\nnet.fetch https://rust.test');
  expect(ts.secret('rust.token')).toBe(false); expect(rust.secret('ts.token')).toBe(false);
  expect(ts.permits('https://rust.test')).toBe(false); expect(rust.permits('https://ts.test')).toBe(false);
  installGrants(data, 'rust', 'secret.keep camelCase');
  expect(ts.secret('ts.token')).toBe(false); expect(ts.permits('https://ts.test')).toBe(false);
  expect(ts.error).toContain('line 1');
});

test('the JS source fetch binding refuses before I/O, retains valid access and refuses redirects', async () => {
  const { install, fetch: sourceFetch, appGlobal } = await import('../web-js/ts-fetch.js');
  let hits = 0;
  const server = Bun.serve({ port: 0, fetch(req) { hits++; return req.url.endsWith('/redirect') ? new Response(null, { status: 307, headers: { location: 'https://outside.test/' } }) : new Response('ok'); } });
  try {
    const valid = `net.fetch ${server.url.origin}\nsecret.keep valid`;
    install({}, `${valid}\nsecret.keep camelCase`);
    await expect(sourceFetch(server.url)).rejects.toThrow('line 3');
    const { fetch: alias } = appGlobal;
    for (const f of [appGlobal.fetch, appGlobal['fetch'], appGlobal.self.fetch, appGlobal.window.fetch, alias]) await expect(f(server.url)).rejects.toThrow('line 3');
    expect(hits).toBe(0);
    install({}, valid);
    expect(await (await sourceFetch(new Request(`${server.url}yes`))).text()).toBe('ok');
    await expect(sourceFetch(`${server.url}redirect`)).rejects.toThrow();
    await expect(sourceFetch('https://outside.test/')).rejects.toThrow('refused by grant');
    expect(hits).toBe(2);
  } finally { server.stop(true); }
});

test('the wasm URL import agrees with whole-set URL grammar', async () => {
  const { grantOrigins } = await import('./navigation.js');
  const { parseGrants } = await import('./http-body.js');
  const memory = new WebAssembly.Memory({ initial: 1 }), adapter = grantOrigins(() => memory);
  for (const target of ['https://EXAMPLE.com:443/path', 'https://bücher.example', 'http://127.1', 'https://[0:0::1]', 'custom://EXAMPLE.com:90', 'https://*.example.com/', 'custom://*.127.0.0.1:90', 'https://*.example.com?', 'https://*.example.com#', 'https://*.127.1', 'https://*.com', 'https://*.example.com/path']) {
    const wildcard = target.includes('://*.'), normalized = target.replace('://*.', '://'), bytes = new TextEncoder().encode(normalized);
    new Uint8Array(memory.buffer).set(bytes);
    const n = adapter.origin(0, bytes.length, Number(wildcard), 0, 0);
    expect(n > 0).toBe(!parseGrants('net.fetch ' + target).error);
    if (n > 0) {
      expect(adapter.origin(0, bytes.length, Number(wildcard), 1024, n)).toBe(n);
      const u = new URL(normalized);
      expect(new TextDecoder().decode(new Uint8Array(memory.buffer, 1024, n)).split('\0').slice(0, 2)).toEqual([u.protocol.slice(0, -1), u.hostname.toLowerCase()]);
    }
  }
});
