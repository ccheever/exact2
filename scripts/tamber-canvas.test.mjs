// The canvas gateway (LLP 1121 stage 10 C): entered by a single-use grant,
// held by a server-side session, an allow list by scope, nothing of the
// loop's loopback origin or a client's credentials crossing it, and revoke
// or expiry ending every proxied stream at once. A fake pairing (A's
// interface) and a fake dev loop on loopback; plain HTTP (B owns TLS).
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { randomBytes } from 'node:crypto';
import { createServer, request as httpRequest } from 'node:http';
import { connect } from 'node:net';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { readFileSync } from 'node:fs';
import { BODY_LIMIT, CANVAS_COOKIE, CHECKPOINT_LIMIT, MINT_RATE, PLACEHOLDER, canonical, canvasGateway, needs, rewriter } from './tamber-canvas.mjs';
import { tamberServer } from './tamber-server.mjs';

const ORIGIN = 'https://canvas.test:9443';
const TOOL = 'https://canvas.test:9442';
const ORDER = ['read', 'propose', 'write'];

/** A's run pairing, by hand, in its shapes (scripts/tamber-pairing.mjs on
 * x2/s10a): `lookup`, and `onEnd` whose `fn(reason)` is called once, or at
 * once for a client already ended. */
function fakePairing() {
  const tokens = new Map(), ends = new Map(), ended = new Set();
  const finish = reason => client => {
    for (const [t, r] of tokens) if (r.client === client) tokens.delete(t);
    ended.add(client);
    const fns = ends.get(client) ?? new Set();
    ends.delete(client);
    for (const fn of fns) fn(reason);
  };
  return {
    add(client, scope, expires = Date.now() + 3_600_000) { const t = randomBytes(32).toString('hex'); tokens.set(t, { client, scope, expires }); ended.delete(client); return t; },
    revoke: finish('revoked'),
    expire: finish('expired'),
    listeners: client => ends.get(client)?.size ?? 0,
    lookup: token => tokens.get(token) ?? null,
    onEnd(client, fn) {
      if (ended.has(client)) { fn('revoked'); return () => {}; }
      if (!ends.has(client)) ends.set(client, new Set());
      ends.get(client).add(fn);
      return () => ends.get(client)?.delete(fn);
    },
  };
}
/** A's module exports, in their shapes: a token bucket per key (a burst of
 * `rate`, refilled at `rate` per `per` ms), and `allows(scope, needs, local)`. */
const limiter = ({ rate, per, clock = Date.now }) => {
  const buckets = new Map();
  const level = key => { const now = clock(), b = buckets.get(key) ?? { tokens: rate, at: now }; b.tokens = Math.min(rate, b.tokens + (now - b.at) * rate / per); b.at = now; buckets.set(key, b); return b; };
  return { take: key => { const b = level(key); if (b.tokens < 1) return false; b.tokens -= 1; return true; }, wait: key => { const b = level(key); return b.tokens >= 1 ? 0 : Math.ceil((1 - b.tokens) * per / rate); } };
};
const allowed = [];
const allows = (scope, need, local = false) => {
  allowed.push(local);
  if (need === 'local') return local;
  return ORDER.indexOf(scope) >= 0 && ORDER.indexOf(scope) >= ORDER.indexOf(need);
};

/** A dev loop stand-in on loopback: it records each request, answers with
 * its own loopback URLs everywhere, and holds `/__dev/page` (SSE) and
 * `/__dev/commands?wait=` open until the client goes. */
let loop, loopBase, port;
const seen = [], closed = [], pressed = new Set();
/** Loopback addresses in each of their forms, a line each, and the lines a
 * canvas client sees for them (the loop's own as the canvas origin, any
 * other as the placeholder, a lookalike untouched). */
const ADDRESS_CASES = p => [
  [`http://127.0.0.1:${p}/a`, 'https://canvas.test:9443/a'],
  ['http://localhost:7000/b', `http://${PLACEHOLDER}/b`],
  [`http://%6cocalhost:${p}/c`, 'https://canvas.test:9443/c'],
  ['http://localhost/d', `http://${PLACEHOLDER}/d`],
  [`https://127.0.0.2:${p}/e`, 'https://canvas.test:9443/e'],
  [`src="//[::1]:${p}/f"`, 'src="//canvas.test:9443/f"'],
  [`ws://localhost:${p}/g`, 'wss://canvas.test:9443/g'],
  [`http:\\/\\/127.0.0.1:${p}\\/h`, 'https:\\/\\/canvas.test:9443\\/h'],
  ['u=http%3A%2F%2F127.0.0.1%3A7000%2Fi', `u=http%3A%2F%2F${PLACEHOLDER}%2Fi`],
  [`u=http%3A%2F%2Flocalhost%3A${p}%2Fj`, 'u=https%3A%2F%2Fcanvas.test%3A9443%2Fj'],
  [`u=http%3A%2F%2F%256cocalhost%3A${p}%2Fj2`, 'u=https%3A%2F%2Fcanvas.test%3A9443%2Fj2'],
  [`"host": "localhost:${p}"`, '"host": "canvas.test:9443"'],
  ['"other": "127.0.0.1:7001"', `"other": "${PLACEHOLDER}"`],
  [`http://0x7f.1:${p}/k`, 'https://canvas.test:9443/k'],
  ['http://2130706433:7002/l', `http://${PLACEHOLDER}/l`],
  ['http://[::ffff:127.0.0.1]:7003/m', `http://${PLACEHOLDER}/m`],
  ['https://example.test/localhost:9000.png', 'https://example.test/localhost:9000.png'],
  ['notlocalhost:9000 and see localhost:9000.', `notlocalhost:9000 and see ${PLACEHOLDER}.`],
  ['https://example.test/x and http://192.168.1.20:8765/', 'https://example.test/x and http://192.168.1.20:8765/'],
];
const addresses = p => ADDRESS_CASES(p).map(([from]) => from).join('\n');
beforeAll(async () => {
  loop = createServer((req, res) => {
    const chunks = [];
    req.on('data', c => chunks.push(c));
    req.on('end', () => {
      const url = new URL(req.url, 'http://x');
      seen.push({ method: req.method, url: req.url, headers: req.headers, body: Buffer.concat(chunks).toString() });
      const me = `http://127.0.0.1:${port}`;
      res.on('close', () => closed.push(url.pathname));
      if (url.pathname === '/') {
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'set-cookie': ['loop=1; Path=/', 'other=2'], 'content-security-policy': `default-src 'self' ${me}; frame-ancestors 'self' http://127.0.0.1:7740`, 'access-control-allow-origin': '*', link: `<http://localhost:${port}/app.js>; rel=preload` });
        res.end(`<!doctype html><a href="${me}/x">x</a><a href="http://[::1]:${port}/y">y</a><a href="exact://open?url=${encodeURIComponent(me + '/')}">open</a>`);
      } else if (url.pathname === '/moved') {
        res.writeHead(302, { location: `http://localhost:${port}/next` }); res.end();
      } else if (url.pathname === '/app.js') {
        res.writeHead(200, { 'content-type': 'text/javascript' }); res.end('export {};');
      } else if (url.pathname === '/__dev/hello') {
        res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify({ hello: true, page: `${me}/`, url: `http://localhost:${port}/exact.json`, host: `127.0.0.1:${port}` }));
      } else if (url.pathname === '/__dev/commands' && Number(url.searchParams.get('wait')) > 0) {
        // Held: answered only if the client stays a minute.
        const t = setTimeout(() => { res.writeHead(200, { 'content-type': 'application/json' }); res.end('{}'); }, 60_000);
        res.on('close', () => clearTimeout(t));
      } else if (url.pathname === '/__dev/page') {
        res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store' });
        res.write(`data: ${JSON.stringify({ reload: 1, page: `${me}/` })}\n\n`);
      } else if (url.pathname === '/__dev/open') {
        res.writeHead(200, { 'content-type': 'text/html' }); res.end(`<a href="exact://x?u=${encodeURIComponent(me + '/')}">${me}/</a>`);
      } else if (url.pathname === '/addresses.html' || url.pathname === '/addresses.json') {
        res.writeHead(200, { 'content-type': url.pathname.endsWith('json') ? 'application/json' : 'text/html', link: `<http://localhost:7000/x>; rel=preload, <http://%6cocalhost:${port}/y>; rel=preload`, 'x-addresses': addresses(port).split('\n')[0] });
        res.end(addresses(port));
      } else if (url.pathname === '/__dev/generation/stream') {
        res.writeHead(200, { 'content-type': 'text/event-stream' });
        for (const line of addresses(port).split('\n')) res.write(`data: ${line}\n\n`);
        res.end();
      } else if (url.pathname === '/big.bin') {
        // Endless, as fast as the reader takes it; `pressed` once a write waits.
        res.writeHead(200, { 'content-type': 'application/octet-stream' });
        const chunk = Buffer.alloc(64 * 1024, 120);
        const pump = () => { while (!res.destroyed && res.write(chunk)); if (!res.destroyed) { pressed.add('/big.bin'); res.once('drain', pump); } };
        pump();
      } else if (url.pathname === '/__tamber/token') {
        res.writeHead(200, { 'content-type': 'application/json' }); res.end('{"token":"the-loop-token"}');
      } else {
        res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify({ path: url.pathname }));
      }
    });
  });
  await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
  port = loop.address().port;
  loopBase = `http://127.0.0.1:${port}`;
});
afterAll(() => new Promise(ok => { loop.closeAllConnections?.(); loop.close(ok); }));

/** A gateway on loopback in front of the fake loop. */
async function gateway(opts = {}) {
  const pairing = opts.pairing ?? fakePairing();
  const gw = canvasGateway({ loop: loopBase, pairing, origin: ORIGIN, toolOrigin: TOOL, limiter, allows, ...opts });
  const server = createServer((req, res) => { gw.handle(req, res, new URL(req.url, 'http://x')); });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const base = `http://127.0.0.1:${server.address().port}`;
  const at = (path, init = {}) => fetch(`${base}${path}`, { redirect: 'manual', ...init });
  const stop = () => new Promise(ok => { gw.close(); server.closeAllConnections?.(); server.close(ok); });
  return { gw, pairing, at, base, stop };
}

/** A client's way in: a grant minted, POSTed to its entry, the session. */
async function enter(g, client, scope = 'read') {
  const token = g.pairing.add(client, scope);
  const { ok } = g.gw.mint(token);
  const entry = await g.at(new URL(ok.url).pathname, { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded', origin: TOOL }, body: `grant=${ok.grant}` });
  const cookie = entry.headers.get('set-cookie')?.split(';')[0];
  return { token, cookie, entry, ok };
}

const loopbackOf = p => new RegExp(`(?:127\\.0\\.0\\.1|localhost|\\[::1\\]|%5B%3A%3A1%5D)(?::|%3A)${p}(?!\\d)`, 'i');

test('the route table: read, write, and what is never passed', () => {
  for (const p of ['/', '/index.html', '/app.js', '/__dev', '/__dev/page', '/__dev/hello', '/__dev/commands', '/__dev/generation/1/exact.json', '/__dev/map/abc', '/__dev/inspect.js', '/dev-inspect.json', '/exact.json', '/modules/a.js', '/__dev/design']) expect(needs('GET', p)).toBe('read');
  for (const p of ['/__dev/reloaded', '/__dev/painted', '/__dev/gpu', '/__dev/report']) expect(needs('POST', p)).toBe('read');
  for (const p of ['/__dev/design', '/__dev/checkpoint', '/__dev/open']) expect(needs('POST', p)).toBe('write');
  for (const [m, p] of [['GET', '/__tamber'], ['POST', '/__tamber'], ['GET', '/__tamber/token'], ['GET', '/__tamber/tool/'], ['POST', '/__tamber/events'], ['GET', '/__dev/install/ios'], ['POST', '/__exact/trace'], ['GET', '/__exact/trace'], ['GET', '/.exact/install/index.html'], ['POST', '/'], ['PUT', '/x'], ['DELETE', '/'], ['GET', '/canvas/c/'], ['PUT', '/__dev/gpu'], ['GET', '/__TAMBER/token'], ['GET', '/__Tamber'], ['GET', '/.EXACT/install/index.html'], ['GET', '/.Exact/install.json'], ['GET', '/__DEV/open'], ['POST', '/__Dev/open'], ['GET', '/__dev/OPEN']]) expect(needs(m, p)).toBeNull();
});

test('mint, enter, cookie, 303, and the page proxied with nothing of the loop or the client crossing', async () => {
  const g = await gateway();
  try {
    const { cookie, entry, ok } = await enter(g, 'phone#1');
    expect(ok.url).toBe(`${ORIGIN}/canvas/phone%231/`);
    expect(ok.grant).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(entry.status).toBe(303);
    expect(entry.headers.get('location')).toBe('/');
    expect(entry.headers.get('set-cookie')).toMatch(new RegExp(`^${CANVAS_COOKIE}=[A-Za-z0-9_-]{43}; Secure; HttpOnly; SameSite=Strict; Path=/$`));
    expect(entry.headers.get('set-cookie')).not.toMatch(/domain/i);

    seen.length = 0;
    const page = await g.at('/', { headers: { cookie: `a=b; ${cookie}`, 'x-exact-tamber-token': 'nope', forwarded: 'for=203.0.113.9', 'x-forwarded-for': '203.0.113.9', 'x-forwarded-host': 'evil.test', 'x-forwarded-proto': 'http', connection: 'x-secret', 'x-secret': 's', 'keep-alive': 'timeout=5' } });
    expect(page.status).toBe(200);
    const html = await page.text();
    const up = seen.at(-1).headers;
    expect(up.cookie).toBeUndefined();
    expect(up['x-exact-tamber-token']).toBeUndefined();
    expect(up['x-forwarded-for'] ?? up['x-forwarded-host'] ?? up['x-forwarded-proto'] ?? up['x-secret']).toBeUndefined();
    // The client's `forwarded` never; the gateway's own marker, so the dev gate calls it not local.
    expect(up.forwarded).toBe('for="_exact-canvas"');
    expect(up.host).toBe(`127.0.0.1:${port}`);
    // Downstream: no Set-Cookie, no CORS, no-store, the canvas origin for the loop's, frame-ancestors the tool's.
    expect(page.headers.get('set-cookie')).toBeNull();
    expect(page.headers.get('access-control-allow-origin')).toBeNull();
    expect(page.headers.get('cache-control')).toBe('no-store');
    const csp = page.headers.get('content-security-policy');
    expect(csp).toContain(`default-src 'self' ${ORIGIN}`);
    expect(csp).toContain(`frame-ancestors 'self' ${TOOL}`);
    expect(csp).not.toContain('127.0.0.1');
    expect(page.headers.get('link')).toBe(`<${ORIGIN}/app.js>; rel=preload`);
    expect(html).toContain(`href="${ORIGIN}/x"`);
    expect(html).toContain(`href="${ORIGIN}/y"`);
    expect(html).toContain(encodeURIComponent(`${ORIGIN}/`));

    // Every passed answer: no loopback form of the loop's origin in a header or a body.
    const leak = loopbackOf(port);
    for (const [path, init] of [['/moved'], ['/__dev/hello'], ['/app.js'], ['/__dev/design'], ['/exact.json']]) {
      const r = await g.at(path, { headers: { cookie }, ...init });
      expect(r.status).not.toBe(404);
      for (const [k, v] of r.headers) expect(`${k}: ${v}`).not.toMatch(leak);
      expect(await r.text()).not.toMatch(leak);
    }
    expect((await g.at('/moved', { headers: { cookie } })).headers.get('location')).toBe(`${ORIGIN}/next`);
    expect(await (await g.at('/__dev/hello', { headers: { cookie } })).json()).toEqual({ hello: true, page: `${ORIGIN}/`, url: `${ORIGIN}/exact.json`, host: 'canvas.test:9443' });
  } finally { await g.stop(); }
});

test('the query string is kept: /__dev/commands?nonce=&ack=&wait=', async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'q');
    seen.length = 0;
    const r = await g.at('/__dev/commands?nonce=n1&ack=3&wait=0', { headers: { cookie } });
    expect(r.status).toBe(200);
    expect(seen.at(-1).url).toBe('/__dev/commands?nonce=n1&ack=3&wait=0');
    // An allowed POST: Origin is the loop's upstream, the body passed.
    const b = await g.at('/__dev/reloaded?seq=2&boot=5', { method: 'POST', headers: { cookie, origin: ORIGIN }, body: 'x' });
    expect(b.status).toBe(200);
    expect(seen.at(-1)).toMatchObject({ method: 'POST', url: '/__dev/reloaded?seq=2&boot=5', body: 'x' });
    expect(seen.at(-1).headers.origin).toBe(loopBase);
  } finally { await g.stop(); }
});

test('a grant reused, older than 30 s, wrong, for another client, or in a query string is refused', async () => {
  let clock = 1_000_000;
  const g = await gateway({ now: () => clock, limiter: () => ({ take: () => true, wait: () => 0 }) });
  const post = (path, body, type = 'application/x-www-form-urlencoded') => g.at(path, { method: 'POST', headers: { 'content-type': type, origin: TOOL }, body });
  try {
    const token = g.pairing.add('a', 'read');
    // Reused.
    let { ok } = g.gw.mint(token);
    expect((await post('/canvas/a/', `grant=${ok.grant}`)).status).toBe(303);
    expect((await post('/canvas/a/', `grant=${ok.grant}`)).status).toBe(404);
    // JSON works once too.
    ({ ok } = g.gw.mint(token));
    expect((await post('/canvas/a/', JSON.stringify({ grant: ok.grant }), 'application/json')).status).toBe(303);
    // Older than 30 s.
    ({ ok } = g.gw.mint(token));
    clock += 30_001;
    expect((await post('/canvas/a/', `grant=${ok.grant}`)).status).toBe(404);
    // Wrong.
    expect((await post('/canvas/a/', `grant=${randomBytes(32).toString('base64url')}`)).status).toBe(404);
    // Another client's entry: refused, and burned.
    g.pairing.add('b', 'read');
    ({ ok } = g.gw.mint(token));
    expect((await post('/canvas/b/', `grant=${ok.grant}`)).status).toBe(404);
    expect((await post('/canvas/a/', `grant=${ok.grant}`)).status).toBe(404);
    // In a query string (with or without a body), or by GET.
    ({ ok } = g.gw.mint(token));
    const r = await post(`/canvas/a/?grant=${ok.grant}`, '');
    expect(r.status).toBe(404);
    expect(r.headers.get('set-cookie')).toBeNull();
    expect((await post(`/canvas/a/?grant=${ok.grant}`, `grant=${ok.grant}`)).status).toBe(404);
    expect((await g.at(`/canvas/a/?grant=${ok.grant}`)).status).toBe(404);
    // An entry Origin that is neither the canvas's nor the tool's: another, none, `null`, malformed. Each burns nothing it didn't read.
    for (const origin of ['https://evil.test', undefined, 'null', 'not a url', `${TOOL}/`, 'https://canvas.test:9442.evil.test']) {
      ({ ok } = g.gw.mint(token));
      const r = await g.at('/canvas/a/', { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded', ...(origin === undefined ? {} : { origin }) }, body: `grant=${ok.grant}` });
      expect(r.status).toBe(404);
      expect(r.headers.get('set-cookie')).toBeNull();
    }
    // The canvas's own Origin is the other one admitted.
    ({ ok } = g.gw.mint(token));
    expect((await g.at('/canvas/a/', { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded', origin: ORIGIN }, body: `grant=${ok.grant}` })).status).toBe(303);
  } finally { await g.stop(); }
});

test('another client\'s session, an unknown one, a page-set cookie of that name, or none is refused; a real one beside a tossed one still works', async () => {
  const g = await gateway();
  try {
    const a = await enter(g, 'a', 'read');
    const b = await enter(g, 'b', 'read');
    expect((await g.at('/', { headers: { cookie: a.cookie } })).status).toBe(200);
    // None, unknown, page-set (any value, a session-shaped one too), the token itself as the cookie.
    for (const cookie of [undefined, `${CANVAS_COOKIE}=${randomBytes(32).toString('base64url')}`, `${CANVAS_COOKIE}=page-set`, `exact-canvas=${a.cookie.split('=')[1]}`, `${CANVAS_COOKIE}=${a.token}`, `token=${a.token}`]) {
      const r = await g.at('/', { headers: cookie ? { cookie } : {} });
      expect(r.status).toBe(404);
      expect(await r.json()).toEqual({ message: 'Not found.' });
    }
    // Cookie tossing: a page-set value first, the real one after: still the real session.
    expect((await g.at('/', { headers: { cookie: `${CANVAS_COOKIE}=tossed; ${a.cookie}` } })).status).toBe(200);
    // Another client's session reads as that client, never as `a`: revoking `a` leaves `b`'s answering and `a`'s not.
    g.pairing.revoke('a');
    expect((await g.at('/', { headers: { cookie: a.cookie } })).status).toBe(404);
    expect((await g.at('/', { headers: { cookie: b.cookie } })).status).toBe(200);
    // `a`'s token with `b`'s entry is no way in either (the grant is bound to the client that minted it).
    const { ok } = g.gw.mint(g.pairing.add('a2', 'read'));
    expect((await g.at('/canvas/b/', { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded', origin: TOOL }, body: `grant=${ok.grant}` })).status).toBe(404);
  } finally { await g.stop(); }
});

test('/__tamber and /__tamber/token through the gateway are not-found, and never reach the loop', async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'w', 'write');
    seen.length = 0;
    for (const [path, method] of [['/__tamber/token', 'GET'], ['/__tamber', 'POST'], ['/__tamber', 'GET'], ['/__tamber/subscribe', 'POST'], ['/__tamber/tool/', 'GET'], ['/__dev/install/ios', 'GET'], ['/.exact/install/index.html', 'GET']]) {
      const r = await g.at(path, { method, headers: { cookie, origin: ORIGIN } });
      expect(r.status).toBe(404);
      expect(await r.text()).not.toContain('the-loop-token');
    }
    expect(seen).toEqual([]);
  } finally { await g.stop(); }
});

test('a cross-origin POST is refused; a read client is refused the write routes, a write client passes', async () => {
  const g = await gateway();
  try {
    const r = await enter(g, 'r', 'read');
    const p = await enter(g, 'p', 'propose');
    const w = await enter(g, 'w', 'write');
    for (const origin of ['https://evil.test', TOOL, loopBase, undefined]) {
      const a = await g.at('/__dev/report', { method: 'POST', headers: { cookie: r.cookie, ...(origin ? { origin } : {}) }, body: '{}' });
      expect(a.status).toBe(404);
    }
    expect((await g.at('/__dev/report', { method: 'POST', headers: { cookie: r.cookie, origin: ORIGIN }, body: '{}' })).status).toBe(200);
    for (const path of ['/__dev/design', '/__dev/checkpoint?id=1-2-3-4&seq=1', '/__dev/open']) {
      for (const who of [r, p]) {
        const refused = await g.at(path, { method: 'POST', headers: { cookie: who.cookie, origin: ORIGIN }, body: '{}' });
        expect(refused.status).toBe(403);
        expect((await refused.json()).needs).toBe('write');
      }
      const passed = await g.at(path, { method: 'POST', headers: { cookie: w.cookie, origin: ORIGIN }, body: '{}' });
      expect(passed.status).toBe(200);
      // A write POST's Origin must be the canvas's too.
      expect((await g.at(path, { method: 'POST', headers: { cookie: w.cookie, origin: 'https://evil.test' }, body: '{}' })).status).toBe(404);
    }
    // `/__dev/open`'s page (write) comes back on the canvas origin.
    const open = await (await g.at('/__dev/open', { headers: { cookie: w.cookie } })).text();
    expect(open).not.toMatch(loopbackOf(port));
    expect(open).toContain(`${ORIGIN}/`);
    expect((await g.at('/__dev/open', { headers: { cookie: r.cookie } })).status).toBe(403);
  } finally { await g.stop(); }
});

for (const how of ['revoke', 'expire']) test(`${how}: an open SSE stream and a held long poll close at once, upstream too, and the next request is refused`, async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'c', 'read');
    const other = await enter(g, 'd', 'read');
    closed.length = 0;
    const sse = await g.at('/__dev/page', { headers: { cookie } });
    expect(sse.headers.get('content-type')).toContain('text/event-stream');
    const reader = sse.body.getReader();
    const first = new TextDecoder().decode((await reader.read()).value);
    expect(first).toContain(`"page":"${ORIGIN}/"`);
    const held = g.at('/__dev/commands?nonce=n&ack=0&wait=25', { headers: { cookie } }).then(r => r.status, () => 'cut');
    const otherSse = await g.at('/__dev/page', { headers: { cookie: other.cookie } });
    const otherReader = otherSse.body.getReader();
    await otherReader.read();
    while (g.gw.live < 3) await new Promise(ok => setTimeout(ok, 5));
    const t0 = performance.now();
    g.pairing[how]('c');
    const ended = await Promise.race([reader.read().then(r => (r.done ? 'done' : 'more'), () => 'cut'), new Promise(ok => setTimeout(() => ok('open'), 2000))]);
    expect(['done', 'cut']).toContain(ended);
    expect(['cut', 404]).toContain(await Promise.race([held, new Promise(ok => setTimeout(() => ok('open'), 2000))]));
    expect(performance.now() - t0).toBeLessThan(1000);
    // The fake loop saw both upstream requests end.
    while (!(closed.includes('/__dev/page') && closed.includes('/__dev/commands')) && performance.now() - t0 < 2000) await new Promise(ok => setTimeout(ok, 5));
    expect(closed).toContain('/__dev/page');
    expect(closed).toContain('/__dev/commands');
    expect(g.gw.live).toBe(1);
    expect((await g.at('/', { headers: { cookie } })).status).toBe(404);
    // The other client's stream is untouched.
    expect((await g.at('/', { headers: { cookie: other.cookie } })).status).toBe(200);
    otherReader.cancel();
  } finally { await g.stop(); }
});

test('a token whose `expires` has passed is refused at once, before any onEnd', async () => {
  let clock = 1_000_000;
  const pairing = fakePairing();
  const g = await gateway({ pairing, now: () => clock });
  try {
    const token = pairing.add('e', 'read', clock + 60_000);
    const { ok } = g.gw.mint(token);
    const entry = await g.at('/canvas/e/', { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded', origin: TOOL }, body: `grant=${ok.grant}` });
    const cookie = entry.headers.get('set-cookie').split(';')[0];
    expect((await g.at('/', { headers: { cookie } })).status).toBe(200);
    clock += 60_000;
    expect((await g.at('/', { headers: { cookie } })).status).toBe(404);
    expect(g.gw.mint(token).status).toBe(401);
  } finally { await g.stop(); }
});

test('mint: rate-limited per client, refused to an unknown token, url never loopback; close() unsubscribes', async () => {
  const pairing = fakePairing();
  let clock = 1_000_000;
  const g = await gateway({ pairing, now: () => clock });
  try {
    expect(g.gw.mint('nope').status).toBe(401);
    expect(g.gw.mint(undefined).status).toBe(401);
    const token = pairing.add('m', 'read');
    for (let i = 0; i < MINT_RATE.rate; i++) {
      const { ok } = g.gw.mint(token);
      expect(ok.url.startsWith(`${ORIGIN}/canvas/`)).toBe(true);
      expect(ok.url).not.toMatch(/127\.0\.0\.1|localhost|\[::1\]/);
    }
    const over = g.gw.mint(token);
    expect(over.status).toBe(429);
    expect(over.ok).toBeUndefined();
    // The bucket refills: one more grant per `per / rate` ms.
    clock += MINT_RATE.per / MINT_RATE.rate;
    expect(g.gw.mint(token).ok).toBeTruthy();
    expect(g.gw.mint(token).status).toBe(429);
    // Another client has its own bucket.
    expect(g.gw.mint(pairing.add('n', 'read')).ok).toBeTruthy();
    expect(pairing.listeners('m')).toBe(1);
    g.gw.close();
    expect(pairing.listeners('m')).toBe(0);
    expect(pairing.listeners('n')).toBe(0);
    // Every scope check was a remote one.
    expect(allowed.length).toBeGreaterThan(0);
    expect(allowed.every(local => local === false)).toBe(true);
    // A's shapes are required.
    expect(() => canvasGateway({ loop: loopBase, pairing, origin: ORIGIN, allows })).toThrow(TypeError);
  } finally { await g.stop(); }
});

test('the server\'s `canvas` method (the seam until A lands) answers with the gateway\'s mint for the request\'s token', async () => {
  const fakeServe = () => {
    const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
    child.kill = () => { child.exitCode = 0; child.emit('exit', 0); };
    let buf = '';
    child.stdin.on('data', chunk => {
      buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
      for (const line of lines) child.stdout.write(`${JSON.stringify({ id: JSON.parse(line).id, ok: { files: {} } })}\n`);
    });
    return child;
  };
  let given;
  const canvas = { mint: token => { given = token; return { ok: { url: `${ORIGIN}/canvas/x/`, grant: 'g' } }; } };
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, canvas });
  const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': tamber.token }, socket: { remoteAddress: '127.0.0.1' } });
  let status = 0, text = '';
  const res = { writeHead(s) { status = s; }, write(t) { text += t; }, end(t) { text += t ?? ''; }, destroyed: false, writableEnded: false };
  queueMicrotask(() => req.end('{"method":"canvas"}'));
  await tamber.handle(req, res, new URL('/__tamber', 'http://x'), { allowed: true, local: true });
  expect(status).toBe(200);
  expect(JSON.parse(text)).toEqual({ ok: { url: `${ORIGIN}/canvas/x/`, grant: 'g' } });
  expect(given).toBe(tamber.token);
  await tamber.close();
});

test('a write client\'s checkpoint takes the loop\'s own cap; every other body 1 MiB', async () => {
  // The mirrored cap is the JS loop's.
  const [, expr] = /const CHECKPOINT_BYTES = ([\d\s*]+)[,;]/.exec(readFileSync(new URL('../host/web-js/dev.mjs', import.meta.url), 'utf8'));
  expect(expr.split('*').reduce((n, f) => n * Number(f), 1)).toBe(CHECKPOINT_LIMIT);
  const g = await gateway();
  try {
    const w = await enter(g, 'w', 'write');
    const post = (path, size) => g.at(path, { method: 'POST', headers: { cookie: w.cookie, origin: ORIGIN }, body: Buffer.alloc(size, 97) });
    seen.length = 0;
    const between = await post('/__dev/checkpoint?id=1-2-3-4&seq=1', 2 * BODY_LIMIT);
    expect(between.status).toBe(200);
    expect(seen.at(-1).body.length).toBe(2 * BODY_LIMIT);
    expect((await post('/__dev/checkpoint?id=1-2-3-4&seq=1', CHECKPOINT_LIMIT)).status).toBe(200);
    seen.length = 0;
    expect((await post('/__dev/checkpoint?id=1-2-3-4&seq=1', CHECKPOINT_LIMIT + 1).catch(() => ({ status: 'cut' }))).status).toBe(413);
    expect((await post('/__dev/design', BODY_LIMIT + 1).catch(() => ({ status: 'cut' }))).status).toBe(413);
    expect((await post('/__dev/report', BODY_LIMIT + 1).catch(() => ({ status: 'cut' }))).status).toBe(413);
    expect(seen).toEqual([]);
  } finally { await g.stop(); }
});

test('onEnd: the reason goes to the log only, both reasons end the same, and a client ended before the gateway watched it is ended at once', async () => {
  const said = [];
  const pairing = fakePairing();
  const g = await gateway({ pairing, log: line => said.push(line) });
  try {
    const a = await enter(g, 'remote:a#1');
    const b = await enter(g, 'remote:b#1');
    pairing.revoke('remote:a#1');
    pairing.expire('remote:b#1');
    expect(said).toEqual(['canvas: remote:a#1 revoked: its proxied streams end', 'canvas: remote:b#1 expired: its proxied streams end']);
    for (const line of said) for (const secret of [a.token, b.token, a.cookie.split('=')[1], b.cookie.split('=')[1], a.ok.grant, b.ok.grant]) expect(line).not.toContain(secret);
    for (const who of [a, b]) expect((await g.at('/', { headers: { cookie: who.cookie } })).status).toBe(404);
    // A's onEnd for an ended client calls back at once: nothing stays watched.
    const c = canvasGateway({ loop: loopBase, pairing: { ...pairing, lookup: () => ({ client: 'remote:a#1', scope: 'read', expires: Infinity }) }, origin: ORIGIN, limiter, allows, log: line => said.push(line) });
    expect(c.mint('t').ok).toBeTruthy();
    expect(said.at(-1)).toBe('canvas: remote:a#1 revoked: its proxied streams end');
    c.close();
  } finally { await g.stop(); }
});

/** A request sent exactly as written (no client parses its target), and
 * the status it got (0 when the connection was cut). */
function rawRequest(base, method, target, headers = {}) {
  return new Promise(ok => {
    const { hostname, port: p } = new URL(base);
    const socket = connect(Number(p), hostname);
    let text = '';
    socket.on('data', d => { text += d; });
    socket.on('error', () => {});
    socket.on('close', () => ok({ status: Number(/^HTTP\/1\.1 (\d+)/.exec(text)?.[1] ?? 0), text }));
    socket.write(`${method} ${target} HTTP/1.1\r\nHost: canvas.test:9443\r\n${Object.entries(headers).map(([k, v]) => `${k}: ${v}\r\n`).join('')}Connection: close\r\n\r\n`);
  });
}

test('canonical: a target the loop could read as another path is refused; the path forwarded is the one checked', () => {
  expect(canonical('/app.js?x=%2F&y=..')).toEqual({ path: '/app.js', query: '?x=%2F&y=..' });
  expect(canonical('/canvas/remote%3Aa%231/')).toEqual({ path: '/canvas/remote%3Aa%231/', query: '' });
  expect(canonical('/a%20b/%C3%A9')).toEqual({ path: '/a%20b/%C3%A9', query: '' });
  for (const bad of ['//a//b/__dev/open', '//127.0.0.1/__tamber/token', '/%2eexact/install/', '/%2Eexact/install/index.html', '/a/../__tamber/token', '/./__dev/open', '/x/%2e%2e/__tamber', '/x/..', '/x/.',
    '/__dev%2fopen', '/__dev%2Fopen', '/__dev%5copen', '/__dev\\open', '/x%00', '/x\u0000', '/%5F%5Ftamber/token', '/%5f_dev/open', '/_%5Fdev/open', '/%61pp.js', '/a//b', '/a/', '/%zz', '/%2', 'http://127.0.0.1/__tamber/token', '*', '', 'x', '/a#b', '/a b', '/a?b c', undefined]) {
    if (bad === '/a/') { expect(canonical(bad)).toEqual({ path: '/a/', query: '' }); continue; }
    expect(canonical(bad)).toBeNull();
  }
});

test('through the gateway: dot segments, empty segments, encoded separators and prefixes, case and absolute targets reach nothing', async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'r', 'read');
    seen.length = 0;
    for (const target of ['//a//b/__dev/open', '//127.0.0.1/__tamber/token', '/%2eexact/install/index.html', '/a/../__tamber/token', '/./__dev/open', '/x/%2e%2e/__tamber', '/__dev%2fopen', '/__dev%5copen', '/__dev\\open', '/x%00',
      '/%5F%5Ftamber/token', '/__TAMBER/token', '/.EXACT/install/index.html', '/__Dev/open', '/a//b', '/%zz', 'http://127.0.0.1/__tamber/token', `http://127.0.0.1:${port}/__dev/open`]) {
      const r = await rawRequest(g.base, 'GET', target, { cookie });
      expect([404, 400]).toContain(r.status);
      expect(r.text).not.toContain('the-loop-token');
    }
    expect(seen).toEqual([]);
    // The query is the client's own, kept as it came; the path is the one checked.
    const r = await rawRequest(g.base, 'GET', '/app.js?x=%2F..%2F__tamber&y=//z', { cookie });
    expect(r.status).toBe(200);
    expect(seen.map(s => s.url)).toEqual(['/app.js?x=%2F..%2F__tamber&y=//z']);
  } finally { await g.stop(); }
});

test('rewriter: the loop\'s own origin in every form as the canvas\'s, any other loopback authority as the placeholder, lookalikes untouched', () => {
  const rewrite = rewriter(loopBase, ORIGIN);
  for (const [from, to] of ADDRESS_CASES(port)) expect(rewrite(from)).toBe(to);
  // One pass: a canvas origin on loopback (a test listener's) is not rewritten again.
  expect(rewriter(loopBase, 'https://127.0.0.1:9443')(`http://localhost:${port}/a 127.0.0.1:${port}`)).toBe('https://127.0.0.1:9443/a 127.0.0.1:9443');
  // A loop on the default port: an authority with no port is it.
  expect(rewriter('http://127.0.0.1', ORIGIN)('http://localhost/x http://localhost:80/y http://localhost:8080/z')).toBe(`${ORIGIN}/x ${ORIGIN}/y http://${PLACEHOLDER}/z`);
  // A loopback toolOrigin is refused.
  for (const toolOrigin of ['http://127.0.0.1:7740', 'http://localhost:7740', 'http://[::1]:7740']) expect(() => canvasGateway({ loop: loopBase, pairing: fakePairing(), origin: ORIGIN, toolOrigin, limiter, allows })).toThrow(TypeError);
});

test('every loopback address is rewritten in headers, HTML, JSON and SSE', async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'r', 'read');
    const want = ADDRESS_CASES(port).map(([, to]) => to).join('\n');
    for (const path of ['/addresses.html', '/addresses.json']) {
      const r = await g.at(path, { headers: { cookie } });
      expect(await r.text()).toBe(want);
      expect(r.headers.get('link')).toBe(`<http://${PLACEHOLDER}/x>; rel=preload, <${ORIGIN}/y>; rel=preload`);
      expect(r.headers.get('x-addresses')).toBe(ADDRESS_CASES(port)[0][1]);
    }
    const sse = await (await g.at('/__dev/generation/stream', { headers: { cookie } })).text();
    expect(sse).toBe(ADDRESS_CASES(port).map(([, to]) => `data: ${to}\n\n`).join(''));
  } finally { await g.stop(); }
});

test('revoke closes a stalled upload and a download whose client never reads, at once, upstream too', async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'u', 'read');
    const { hostname, port: p } = new URL(g.base);
    // An upload that stalls: 10 of a promised 100,000 bytes.
    const upload = new Promise(ok => {
      const r = httpRequest({ host: hostname, port: p, method: 'POST', path: '/__dev/report', headers: { cookie, origin: ORIGIN, 'content-length': '100000' } });
      r.on('error', () => ok('cut'));
      r.on('close', () => ok('closed'));
      r.on('response', res => { res.resume(); res.on('close', () => ok('answered')); });
      r.write('0123456789');
    });
    // A download the client never reads: the gateway's writes back up.
    closed.length = 0; pressed.clear();
    // A raw socket paused at its first bytes reads nothing more from the kernel.
    const reading = connect(Number(p), hostname);
    const download = new Promise(ok => { reading.on('error', () => ok('cut')); reading.on('close', () => ok('closed')); });
    reading.once('data', () => reading.pause());
    reading.write(`GET /big.bin HTTP/1.1\r\nHost: canvas.test:9443\r\nCookie: ${cookie}\r\n\r\n`);
    while (g.gw.live < 2) await new Promise(ok => setTimeout(ok, 5));
    // Held there: the client reads nothing, so the gateway's writes back up.
    await new Promise(ok => setTimeout(ok, 200));
    expect(g.gw.live).toBe(2);
    const t0 = performance.now();
    g.pairing.revoke('u');
    const timeout = new Promise(ok => setTimeout(() => ok('open'), 2000));
    expect(['cut', 'closed', 'answered']).toContain(await Promise.race([upload, timeout]));
    // The gateway let both go at once, and the loop's download ended (its fetch aborted).
    expect(g.gw.live).toBe(0);
    while (!closed.includes('/big.bin') && performance.now() - t0 < 2000) await new Promise(ok => setTimeout(ok, 5));
    expect(closed).toContain('/big.bin');
    expect(performance.now() - t0).toBeLessThan(1000);
    // The connection is cut: the client, reading at last, finds what was in flight and then its end.
    reading.resume();
    expect(['cut', 'closed']).toContain(await Promise.race([download, timeout]));
  } finally { await g.stop(); }
});

test('a session evicted past the cap takes its streams with it; a client has at most 32 requests in flight', async () => {
  const g = await gateway({ limiter: () => ({ take: () => true, wait: () => 0 }) });
  try {
    const first = await enter(g, 's', 'read');
    const token = first.token;
    const sse = await g.at('/__dev/page', { headers: { cookie: first.cookie } });
    const reader = sse.body.getReader();
    await reader.read();
    // Sixteen more sessions: the first is the oldest past the cap of 16.
    let last;
    for (let i = 0; i < 16; i++) {
      const { ok } = g.gw.mint(token);
      const entry = await g.at(new URL(ok.url).pathname, { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded', origin: TOOL }, body: `grant=${ok.grant}` });
      last = entry.headers.get('set-cookie').split(';')[0];
    }
    const ended = await Promise.race([reader.read().then(r => (r.done ? 'done' : 'more'), () => 'cut'), new Promise(ok => setTimeout(() => ok('open'), 2000))]);
    expect(['done', 'cut']).toContain(ended);
    expect((await g.at('/', { headers: { cookie: first.cookie } })).status).toBe(404);
    // At most 32 at once: the 33rd is 503 until one ends.
    const polls = Array.from({ length: 32 }, () => g.at('/__dev/commands?nonce=n&ack=0&wait=25', { headers: { cookie: last } }).then(r => r.status, () => 'cut'));
    while (g.gw.live < 32) await new Promise(ok => setTimeout(ok, 5));
    const over = await g.at('/__dev/hello', { headers: { cookie: last } });
    expect(over.status).toBe(503);
    g.pairing.revoke('s');
    for (const status of await Promise.all(polls)) expect(['cut', 404]).toContain(status);
    expect(g.gw.live).toBe(0);
  } finally { await g.stop(); }
});

test('the JS loop\'s page reloads afresh when its checkpoint is refused (a canvas below write), and its script still parses', () => {
  const s = readFileSync(new URL('../host/web-js/dev.mjs', import.meta.url), 'utf8');
  const head = 'const client = (n, revision) => `';
  const i = s.indexOf(head) + head.length - 1, j = s.indexOf('</script>`;', i) + '</script>`'.length;
  const names = [...new Set([...s.slice(i, j).matchAll(/\$\{([^}]*)\}/g)].flatMap(m => m[1].match(/\b[A-Z_]{3,}\b/g) ?? []))].filter(n => n !== 'JSON');
  const html = new Function('n', 'revision', ...names, `return ${s.slice(i, j)};`)(3, 'r', ...names.map(() => []));
  const code = html.replace(/^<script>/, '').replace(/<\/script>$/, '');
  expect(() => new Function(code)).not.toThrow();
  // The 403 is checked before the failure that keeps the old page.
  const refused = code.indexOf('if(saved.status===403){'), failed = code.indexOf("if(!saved.ok)throw new Error('checkpoint handoff answered '");
  expect(refused).toBeGreaterThan(0);
  expect(refused).toBeLessThan(failed);
  expect(code.slice(refused, failed)).toContain('location.reload();return}');
});
