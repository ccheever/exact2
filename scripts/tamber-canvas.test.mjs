// The canvas gateway (LLP 1121 stage 10 C): entered by a single-use grant,
// held by a server-side session, an allow list by scope, nothing of the
// loop's loopback origin or a client's credentials crossing it, and revoke
// or expiry ending every proxied stream at once. A fake pairing (A's
// interface) and a fake dev loop on loopback; plain HTTP (B owns TLS).
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { readFileSync } from 'node:fs';
import { BODY_LIMIT, CANVAS_COOKIE, CHECKPOINT_LIMIT, MINT_RATE, canvasGateway, needs } from './tamber-canvas.mjs';
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
const seen = [], closed = [];
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
  for (const [m, p] of [['GET', '/__tamber'], ['POST', '/__tamber'], ['GET', '/__tamber/token'], ['GET', '/__tamber/tool/'], ['POST', '/__tamber/events'], ['GET', '/__dev/install/ios'], ['POST', '/__exact/trace'], ['GET', '/__exact/trace'], ['GET', '/.exact/install/index.html'], ['POST', '/'], ['PUT', '/x'], ['DELETE', '/'], ['GET', '/canvas/c/'], ['PUT', '/__dev/gpu']]) expect(needs(m, p)).toBeNull();
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
  const g = await gateway({ now: () => clock });
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
    // A foreign Origin on the entry.
    expect((await g.at('/canvas/a/', { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded', origin: 'https://evil.test' }, body: `grant=${ok.grant}` })).status).toBe(404);
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
