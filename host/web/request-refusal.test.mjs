import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { request } from './http-body.js';
const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
const defer = source.match(/function deferFulfill\(\.\.\.args\) \{[\s\S]*?\n\}/)[0];
const branch = source.match(/case "refuse": \{[^\n]*\}/)[0];

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
  const delivered = [];
  const apply = Function('safelyFulfill', 'encoder', 'inflight', `${defer}; return (op,incarnation)=>{switch(op.op){${branch}}};`)(
    (...args) => delivered.push(args), new TextEncoder(), new Set());
  apply({ op: 'refuse', ticket: 17, message: 'only HTTP may opt into independent transport' }, 4);
  expect(delivered.length).toBe(0);
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
  } finally { origin.stop(true); destination.stop(true); }
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
  ];
  for (const admits of copies) for (const [grant, url, want] of cases) expect([grant, url, admits(grant, url)]).toEqual([grant, url, want]);
});
