// The canvas gateway (LLP 1121 stage 10 C): entered by a single-use grant,
// held by a server-side session, an allow list by scope, nothing of the
// loop's loopback origin or a client's credentials crossing it, and revoke
// or expiry ending every proxied stream at once. A fake pairing (A's
// interface) and a fake dev loop on loopback; plain HTTP (B owns TLS).
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { createHash, randomBytes } from 'node:crypto';
import { createServer, request as httpRequest } from 'node:http';
import { connect } from 'node:net';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { readFileSync } from 'node:fs';
import { request as httpsRequest } from 'node:https';
import { BODY_LIMIT, CANVAS_COOKIE, ENTRY_SCRIPT, ENTRY_SCRIPT_HASH, CHECKPOINT_LIMIT, MINT_RATE, PLACEHOLDER, canonical, canvasGateway, needs, openRemoteCanvas, rewriter } from './tamber-canvas.mjs';
import { tamberServer } from './tamber-server.mjs';
import { developmentGate } from './install-page.mjs';

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
/** Loopback addresses, a line each, and the lines a canvas client sees for
 * them: the loop's own as the canvas origin, any other loopback address as
 * the placeholder, any other host byte for byte, and the known limits
 * (escaped spellings in text) as they came. */
const ADDRESS_CASES = p => [
  [`http://127.0.0.1:${p}/a`, 'https://canvas.test:9443/a'],
  ['http://localhost:7000/b', `http://${PLACEHOLDER}/b`],
  [`http://%6cocalhost:${p}/c`, 'https://canvas.test:9443/c'],
  ['http://localhost/d', `http://${PLACEHOLDER}/d`],
  [`https://127.0.0.2:${p}/e`, 'https://canvas.test:9443/e'],
  [`src="//[::1]:${p}/f"`, 'src="//canvas.test:9443/f"'],
  [`ws://localhost:${p}/g`, 'wss://canvas.test:9443/g'],
  [`"host": "localhost:${p}"`, '"host": "canvas.test:9443"'],
  ['"other": "127.0.0.1:7001"', `"other": "${PLACEHOLDER}"`],
  [`http://0x7f.1:${p}/k`, 'https://canvas.test:9443/k'],
  ['http://2130706433:7002/l', `http://${PLACEHOLDER}/l`],
  ['http://[::ffff:127.0.0.1]:7003/m', `http://${PLACEHOLDER}/m`],
  [`http://localhost.:${p}/n`, 'https://canvas.test:9443/n'],
  [`at 127.1:${p} and foo.localhost:7004 now`, `at canvas.test:9443 and ${PLACEHOLDER} now`],
  [`(see http://user@localhost:${p}/o)`, '(see https://canvas.test:9443/o)'],
  // Never another host: evil.test is this URL's host, whatever its userinfo holds.
  ['http://127.0.0.1%2F@evil.test/a', 'http://127.0.0.1%2F@evil.test/a'],
  ['http://127.0.0.1;@evil.test/a http://127.0.0.1,x@evil.test/b http://a(127.0.0.1)@evil.test/c', 'http://127.0.0.1;@evil.test/a http://127.0.0.1,x@evil.test/b http://a(127.0.0.1)@evil.test/c'],
  // Userinfo punctuation in a loopback URL: the whole authority goes.
  [`http://user;name@localhost:${p}/u1 http://u,(x)@127.0.0.1:7005/u2`, `https://canvas.test:9443/u1 http://${PLACEHOLDER}/u2`],
  // A list's or a sentence's punctuation after an authority that doesn't parse with it stays in the text.
  [`default-src 'self' http://127.0.0.1:${p}; img-src http://localhost:7006, (at http://localhost:${p})`, `default-src 'self' https://canvas.test:9443; img-src http://${PLACEHOLDER}, (at https://canvas.test:9443)`],
  ['https://example.test/localhost:9000.png', 'https://example.test/localhost:9000.png'],
  ['notlocalhost:9000 and see localhost:9000.', `notlocalhost:9000 and see ${PLACEHOLDER}.`],
  ['https://example.test/x and http://192.168.1.20:8765/ and http://127.0.0.1.example.test/', 'https://example.test/x and http://192.168.1.20:8765/ and http://127.0.0.1.example.test/'],
  // Known limits (§10): escaped spellings in text pass as they came.
  [`u=http%3A%2F%2Flocalhost%3A${p}%2Fj`, `u=http%3A%2F%2Flocalhost%3A${p}%2Fj`],
  [`http:\\/\\/127.0.0.1:${p}\\/h`, `http:\\/\\/127.0.0.1:${p}\\/h`],
  [`http://localhos&#116;:${p}/q`, `http://localhos&#116;:${p}/q`],
];
/** A JSON body: the cases as strings (one under a key that is a loopback
 * URL), and one written with a \u escape, which parsing decodes. */
const addressJSON = p => `{"cases":${JSON.stringify(ADDRESS_CASES(p).map(([from]) => from))},${JSON.stringify(`http://localhost:${p}/key`)}:1,"escaped":"http://localhos\\u0074:${p}/x"}`;
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
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'set-cookie': ['loop=1; Path=/', 'other=2'], 'content-security-policy': `default-src 'self' ${me}; frame-ancestors 'self' http://127.0.0.1:7740 https://evil.test`, 'access-control-allow-origin': '*', link: `<http://localhost:${port}/app.js>; rel=preload` });
        res.end(`<!doctype html><a href="${me}/x">x</a><a href="http://[::1]:${port}/y">y</a><p>open ${me}/ here</p>`);
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
        res.writeHead(200, { 'content-type': 'text/html' }); res.end(`<p>build with --url ${me}/</p><a id="browser" href="${me}/">${me}/</a>`);
      } else if (url.pathname === '/addresses.html' || url.pathname === '/addresses.json') {
        res.writeHead(200, { 'content-type': url.pathname.endsWith('json') ? 'application/vnd.exact.envelope+json' : 'text/html', link: `<http://localhost:7000/x>; rel=preload, <http://%6cocalhost:${port}/y>; rel=preload`, 'x-addresses': addresses(port).split('\n')[0], 'x-evil': 'http://127.0.0.1%2F@evil.test/a', 'x-userinfo': `http://127.0.0.1;@evil.test/a http://user;name@localhost:${port}/u http://u,(x)@127.0.0.1:7005/v` });
        res.end(url.pathname.endsWith('json') ? addressJSON(port) : addresses(port));
      } else if (url.pathname === '/plain.json') {
        // No loopback in it: its bytes, spacing and escapes, pass as they came.
        res.writeHead(200, { 'content-type': 'application/json' }); res.end('{ "a" : "https://example.test/\\u0041",\n "n": 1.50 }');
      } else if (url.pathname === '/app.css' || url.pathname === '/app2.js') {
        res.writeHead(200, { 'content-type': url.pathname.endsWith('css') ? 'text/css' : 'text/javascript' }); res.end(addresses(port));
      } else if (url.pathname === '/__dev/generation/stream') {
        res.writeHead(200, { 'content-type': 'text/event-stream' });
        for (const line of addresses(port).split('\n')) res.write(`data: ${line}\n\n`);
        res.write(`data: {"page":"http://localhos\\u0074:${port}/","evil":"http://127.0.0.1%2F@evil.test/a"}\n\n`);
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
  await listening(loop);
  port = loop.address().port;
  loopBase = `http://127.0.0.1:${port}`;
});
afterAll(() => new Promise(ok => { loop.closeAllConnections?.(); loop.close(ok); }));

/** `server` on a port the system picks on 127.0.0.1, bound once and never
 * rebound (no close-then-reuse race); a refused bind says so. */
const listening = server => new Promise((ok, fail) => {
  server.once('error', e => fail(new Error(`the test could not bind 127.0.0.1:0 (${e.code}); this sandbox may not allow listening`)));
  server.listen(0, '127.0.0.1', ok);
});

/** A gateway on loopback in front of the fake loop. */
async function gateway(opts = {}) {
  const pairing = opts.pairing ?? fakePairing();
  const gw = canvasGateway({ loop: loopBase, pairing, origin: ORIGIN, toolOrigin: TOOL, limiter, allows, ...opts });
  // The door a request came through: B's TLS canvas door (`remote`) unless a test says otherwise.
  let door = { allowed: true, local: false, remote: true, port: 'canvas' };
  const server = createServer((req, res) => { gw.handle(req, res, new URL(req.url, 'http://x'), door); });
  await listening(server);
  const base = `http://127.0.0.1:${server.address().port}`;
  const at = (path, init = {}) => fetch(`${base}${path}`, { redirect: 'manual', ...init });
  const stop = () => new Promise(ok => { gw.close(); server.closeAllConnections?.(); server.close(ok); });
  return { gw, pairing, at, base, stop, through: d => { door = d; } };
}

/** The entry page's POST of a grant (its JSON body, its own Origin; null: none). */
const enterInit = (grant, origin = ORIGIN) => ({ method: 'POST', headers: { 'content-type': 'application/json', ...(origin === null ? {} : { origin }) }, body: JSON.stringify({ grant }) });

/** A client's way in: a grant minted, POSTed to its entry, the session. */
async function enter(g, client, scope = 'read') {
  const token = g.pairing.add(client, scope);
  const { ok } = g.gw.mint(token);
  const entry = await g.at(new URL(ok.url).pathname, enterInit(ok.grant));
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

test('mint, enter, cookie, 204, and the page proxied with nothing of the loop or the client crossing', async () => {
  const g = await gateway();
  try {
    const { cookie, entry, ok } = await enter(g, 'phone#1');
    expect(ok.url).toBe(`${ORIGIN}/canvas/phone%231/`);
    expect(ok.grant).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(entry.status).toBe(204);
    expect(entry.headers.get('location')).toBeNull();
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
    // 'self' kept (the canvas's own origin, apps frame their own routes), the tool's added, nothing else: no foreign origin frames a canvas page.
    expect(/frame-ancestors ([^;]*)/.exec(csp)[1].trim()).toBe(`'self' ${TOOL}`);
    expect(csp).not.toContain('evil.test');
    expect(csp).not.toContain('127.0.0.1');
    expect(page.headers.get('link')).toBe(`<${ORIGIN}/app.js>; rel=preload`);
    expect(html).toContain(`href="${ORIGIN}/x"`);
    expect(html).toContain(`href="${ORIGIN}/y"`);
    expect(html).toContain(`<p>open ${ORIGIN}/ here</p>`);

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

test('a grant only in the entry POST\'s JSON body: reused, older than 30 s, wrong, another client\'s, in a query or a path, a form, or from another Origin (the tool\'s too) is refused', async () => {
  let clock = 1_000_000;
  const g = await gateway({ now: () => clock, limiter: () => ({ take: () => true, wait: () => 0 }) });
  const post = (path, grant, origin) => g.at(path, enterInit(grant, origin));
  const refused = async r => { expect(r.status).toBe(404); expect(r.headers.get('set-cookie')).toBeNull(); };
  try {
    const token = g.pairing.add('a', 'read');
    // Once, then replayed.
    let { ok } = g.gw.mint(token);
    expect((await post('/canvas/a/', ok.grant)).status).toBe(204);
    await refused(await post('/canvas/a/', ok.grant));
    // Older than 30 s.
    ({ ok } = g.gw.mint(token));
    clock += 30_001;
    await refused(await post('/canvas/a/', ok.grant));
    // Wrong.
    await refused(await post('/canvas/a/', randomBytes(32).toString('base64url')));
    // Another client's entry: refused, and burned.
    g.pairing.add('b', 'read');
    ({ ok } = g.gw.mint(token));
    await refused(await post('/canvas/b/', ok.grant));
    await refused(await post('/canvas/a/', ok.grant));
    // In a query string (with or without a body), in a path, by GET, or as a form: never read, and not burned.
    ({ ok } = g.gw.mint(token));
    await refused(await g.at(`/canvas/a/?grant=${ok.grant}`, { method: 'POST', headers: { origin: ORIGIN } }));
    const noBody = { method: 'POST', headers: { 'content-type': 'application/json', origin: ORIGIN }, body: '{}' };
    await refused(await g.at(`/canvas/a/?g=${ok.grant}`, enterInit(ok.grant)));
    await refused(await g.at(`/canvas/a/${ok.grant}`, noBody));
    await refused(await g.at(`/canvas/a/${ok.grant}/`, noBody));
    await refused(await g.at(`/canvas/a/#g=${ok.grant}`.replace('#', '%23'), noBody));
    await refused(await g.at(`/canvas/a/?g=${ok.grant}`));
    await refused(await g.at('/canvas/a/', { method: 'POST', headers: { 'content-type': 'application/x-www-form-urlencoded', origin: ORIGIN }, body: `grant=${ok.grant}` }));
    // Cross-origin: another, the tool's, none, `null`, malformed, a lookalike.
    for (const origin of ['https://evil.test', TOOL, null, 'null', 'not a url', `${ORIGIN}/`, 'https://canvas.test:9443.evil.test']) await refused(await post('/canvas/a/', ok.grant, origin));
    // None of those read it: it still enters once, from the canvas's own page.
    expect((await post('/canvas/a/', ok.grant)).status).toBe(204);
  } finally { await g.stop(); }
});

test('the entry page: static, no-store, no referrer, framed by the tool alone, its one script allowed by hash; the fragment leaves the address before any request', async () => {
  const g = await gateway();
  try {
    const r = await g.at('/canvas/remote%3Aa%231/');
    expect(r.status).toBe(200);
    expect(r.headers.get('cache-control')).toBe('no-store');
    expect(r.headers.get('referrer-policy')).toBe('no-referrer');
    expect(r.headers.get('set-cookie')).toBeNull();
    const csp = r.headers.get('content-security-policy');
    expect(csp).toBe(`default-src 'none'; script-src '${ENTRY_SCRIPT_HASH}'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors ${TOOL}`);
    const html = await r.text();
    const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map(m => m[1]);
    expect(scripts).toEqual([ENTRY_SCRIPT]);
    expect(ENTRY_SCRIPT_HASH).toBe(`sha256-${createHash('sha256').update(scripts[0]).digest('base64')}`);
    // The remote tool's origin, for its one message, and no other address.
    expect(html).toContain(`<html lang="en" data-tool="${TOOL}">`);
    expect(html.replace(` data-tool="${TOOL}"`, '')).not.toMatch(/https?:\/\/|src=|href=/);
    // The same page for any client (it holds nothing); a query or more path is not it.
    expect(await (await g.at('/canvas/x/')).text()).toBe(html);
    expect((await g.at('/canvas/x/?a=1')).status).toBe(404);
    expect((await g.at('/canvas/x/y')).status).toBe(404);

    // Its script in a stand-in page: what it calls, in order.
    const run = async (hash, answer, tool = TOOL) => {
      const calls = [];
      const location = { hash, pathname: '/canvas/remote%3Aa%231/', replace: to => calls.push(['replace', to]) };
      const history = { replaceState: (s, t, to) => { calls.push(['replaceState', to]); location.hash = ''; } };
      const fetch = async (to, init) => { calls.push(['fetch', to, init.method, init.body, init.referrerPolicy, init.credentials, location.hash]); if (answer === 'throw') throw new TypeError('failed'); return { ok: answer === 204, status: answer }; };
      const m = { textContent: '' };
      const document = { getElementById: id => (id === 'm' ? m : null), documentElement: { dataset: tool ? { tool } : {} } };
      const parent = { postMessage: (message, target) => calls.push(['postMessage', message, target]) };
      await new Function('location', 'history', 'fetch', 'document', 'parent', `return ${ENTRY_SCRIPT}`)(location, history, fetch, document, parent);
      return { calls, said: m.textContent };
    };
    const grant = 'G'.repeat(43);
    const good = await run(`#g=${grant}`, 204);
    expect(good.calls).toEqual([['replaceState', '/canvas/remote%3Aa%231/'], ['fetch', '/canvas/remote%3Aa%231/', 'POST', JSON.stringify({ grant }), 'same-origin', 'same-origin', ''], ['postMessage', { type: 'exact-canvas-entered' }, TOOL], ['replace', '/']]);
    // No remote tool (no `data-tool`): no message.
    expect((await run(`#g=${grant}`, 204, null)).calls.map(c => c[0])).toEqual(['replaceState', 'fetch', 'replace']);
    expect(good.said).toBe('');
    for (const answer of [404, 'throw']) {
      const bad = await run(`#g=${grant}`, answer);
      expect(bad.calls.map(c => c[0])).toEqual(['replaceState', 'fetch']);
      expect(bad.said).toBe('This canvas link has expired; ask for a new one.');
    }
    // No grant: the address cleaned, the message, and no request at all.
    const none = await run('', 204);
    expect(none.calls).toEqual([['replaceState', '/canvas/remote%3Aa%231/']]);
    expect(none.said).toBe('This canvas link has expired; ask for a new one.');
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
    expect((await g.at('/canvas/b/', enterInit(ok.grant))).status).toBe(404);
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
    const entry = await g.at('/canvas/e/', enterInit(ok.grant));
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
    // A's own limiter and allows by default; anything but functions is refused.
    canvasGateway({ loop: loopBase, pairing, origin: ORIGIN }).close();
    expect(() => canvasGateway({ loop: loopBase, pairing, origin: ORIGIN, limiter: 'no' })).toThrow(TypeError);
  } finally { await g.stop(); }
});

/** A stand-in for `contract tamber serve`: every request answered. */
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

/** A real design server's wire (stage 10 A's pairing and dispatcher) with
 * the gateway it makes, each on its own loopback port: `post` a request as
 * a local or a remote (`peer`) client, `pair` a client at a scope. */
async function wired(canvas = pairing => canvasGateway({ loop: loopBase, pairing, origin: ORIGIN, toolOrigin: TOOL })) {
  // `'default'`: the server's own gateway, dormant until its port opens.
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, ...(canvas === 'default' ? {} : { canvas }) });
  const wire = createServer(), port = createServer();
  await listening(wire); await listening(port);
  const wireBase = `http://127.0.0.1:${wire.address().port}`;
  const gate = developmentGate([{ origin: wireBase }], wire.address().port);
  let remote = null, peers = 0;
  wire.on('request', async (req, res) => { if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), remote ?? gate.check(req)))) { res.writeHead(404); res.end(); } });
  if (tamber.canvas) port.on('request', (req, res) => { tamber.canvas.handle(req, res, new URL(req.url, 'http://x'), { allowed: true, local: false, remote: true, port: 'canvas' }); });
  const local = (await (await fetch(`${wireBase}/__tamber/token`)).json()).token;
  await tamber.ready;
  const post = async (request, token = local, path = '/__tamber', peer = null, door = true) => {
    // `peer` through B's door, or (`door` false) plain HTTP from off this machine.
    remote = peer ? (door ? { local: false, remote: true, peer } : { allowed: true, local: false, peer }) : null;
    try {
      const res = await fetch(`${wireBase}${path}`, { method: 'POST', headers: { 'content-type': 'application/json', ...(token ? { 'x-exact-tamber-token': token } : {}) }, body: JSON.stringify(request) });
      const text = await res.text();
      let body = {};
      try { body = JSON.parse(text); } catch {}
      return { status: res.status, ...body };
    } finally { remote = null; }
  };
  const pair = async (scope, name) => {
    const { code } = (await post({ method: 'pair-code', scope, name })).ok;
    return (await post({ code }, null, '/__tamber/pair', `203.0.113.${++peers}`)).ok;
  };
  const at = (path, init = {}) => fetch(`http://127.0.0.1:${port.address().port}${path}`, { redirect: 'manual', ...init });
  const stop = async () => { await tamber.close(); for (const s of [wire, port]) { s.closeAllConnections?.(); s.close(); } };
  return { tamber, post, pair, at, local, stop };
}

test('with stage 10 A: `canvas` through the real dispatcher (read), enter, proxy, and A\'s revoke ending the stream and the next request', async () => {
  const w = await wired();
  try {
    const p = await w.pair('read', 'phone');
    expect(p.client).toBe('remote:phone#1');
    // Minted over the wire as a remote request, the paired token in its header only.
    const minted = await w.post({ method: 'canvas' }, p.token, '/__tamber', '198.51.100.9');
    expect(minted.status).toBe(200);
    expect(minted.ok.url).toBe(`${ORIGIN}/canvas/remote%3Aphone%231/`);
    const entry = await w.at(new URL(minted.ok.url).pathname, enterInit(minted.ok.grant));
    expect(entry.status).toBe(204);
    const cookie = entry.headers.get('set-cookie').split(';')[0];
    expect((await w.at('/', { headers: { cookie } })).status).toBe(200);
    const sse = await w.at('/__dev/page', { headers: { cookie } });
    const reader = sse.body.getReader();
    expect(new TextDecoder().decode((await reader.read()).value)).toContain(`"page":"${ORIGIN}/"`);
    expect(w.tamber.canvas.live).toBe(1);
    // A's revoke, by the local tool.
    closed.length = 0;
    const t0 = performance.now();
    expect((await w.post({ method: 'revoke', client: p.client })).ok).toEqual({ revoked: p.client });
    const ended = await Promise.race([reader.read().then(r => (r.done ? 'done' : 'more'), () => 'cut'), new Promise(ok => setTimeout(() => ok('open'), 2000))]);
    expect(['done', 'cut']).toContain(ended);
    expect(performance.now() - t0).toBeLessThan(1000);
    while (!closed.includes('/__dev/page') && performance.now() - t0 < 2000) await new Promise(ok => setTimeout(ok, 5));
    expect(closed).toContain('/__dev/page');
    expect(w.tamber.canvas.live).toBe(0);
    expect((await w.at('/', { headers: { cookie } })).status).toBe(404);
    // Its token is told it was revoked; no grant for it.
    expect((await w.post({ method: 'canvas' }, p.token, '/__tamber', '198.51.100.9')).status).toBe(401);
  } finally { await w.stop(); }
});

test('a plain LAN peer\'s `canvas` call is not found, the grant minted only through the door', async () => {
  const w = await wired();
  try {
    const p = await w.pair('read', 'lan');
    const plain = await w.post({ method: 'canvas' }, p.token, '/__tamber', '192.0.2.41', false);
    expect([plain.status, plain.ok]).toEqual([404, undefined]);
    expect(w.tamber.canvas.live).toBe(0);
    expect((await w.post({ method: 'canvas' }, p.token, '/__tamber', '192.0.2.41')).status).toBe(200);
  } finally { await w.stop(); }
});

test('with stage 10 A: a propose client mints (canvas is read) but is refused the write routes; the local token, no token and no gateway as the table says', async () => {
  const w = await wired();
  try {
    const p = await w.pair('propose', 'tablet');
    const minted = await w.post({ method: 'canvas' }, p.token, '/__tamber', '198.51.100.10');
    expect(minted.status).toBe(200);
    const entry = await w.at(new URL(minted.ok.url).pathname, enterInit(minted.ok.grant));
    const cookie = entry.headers.get('set-cookie').split(';')[0];
    expect((await w.at('/', { headers: { cookie } })).status).toBe(200);
    for (const path of ['/__dev/design', '/__dev/checkpoint?id=1-2-3-4&seq=1', '/__dev/open']) {
      const refused = await w.at(path, { method: 'POST', headers: { cookie, origin: ORIGIN }, body: '{}' });
      expect(refused.status).toBe(403);
      expect(await refused.json()).toMatchObject({ scope: 'propose', needs: 'write' });
    }
    // A write client passes the same route.
    const wr = await w.pair('write', 'desk');
    const m2 = (await w.post({ method: 'canvas' }, wr.token, '/__tamber', '198.51.100.11')).ok;
    const c2 = (await w.at(new URL(m2.url).pathname, enterInit(m2.grant))).headers.get('set-cookie').split(';')[0];
    expect((await w.at('/__dev/design', { method: 'POST', headers: { cookie: c2, origin: ORIGIN }, body: '{}' })).status).toBe(200);
    // The local token passes the table (read) and is told the canvas is a paired client's.
    const local = await w.post({ method: 'canvas' });
    expect([local.status, local.error.message]).toEqual([400, '`canvas` is a paired client\'s: the local tool frames the dev loop itself']);
    // No token, or an unknown one: the wire's not-found, before any method.
    expect((await w.post({ method: 'canvas' }, null)).status).toBe(404);
    expect((await w.post({ method: 'canvas' }, randomBytes(32).toString('hex'), '/__tamber', '198.51.100.12')).status).toBe(404);
    // The local token through B's listener is no one.
    expect((await w.post({ method: 'canvas' }, w.local, '/__tamber', '198.51.100.13')).status).toBe(404);
  } finally { await w.stop(); }
  // A server with no gateway answers so.
  const bare = await wired(null);
  try {
    const p = await bare.pair('read', 'x');
    const none = await bare.post({ method: 'canvas' }, p.token, '/__tamber', '198.51.100.14');
    expect([none.status, none.error.message]).toEqual([404, 'this server has no canvas gateway']);
  } finally { await bare.stop(); }
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
  for (const [from, to] of ADDRESS_CASES(port)) expect(rewrite.text(from)).toBe(to);
  // One pass: a canvas origin on loopback (a test listener's) is not rewritten again.
  expect(rewriter(loopBase, 'https://127.0.0.1:9443').text(`http://localhost:${port}/a 127.0.0.1:${port}`)).toBe('https://127.0.0.1:9443/a 127.0.0.1:9443');
  // A loop on the default port: an authority with no port is it.
  expect(rewriter('http://127.0.0.1', ORIGIN).text('http://localhost/x http://localhost:80/y http://localhost:8080/z')).toBe(`${ORIGIN}/x ${ORIGIN}/y http://${PLACEHOLDER}/z`);
  // JSON is parsed: an escaped spelling is decoded, keys too; no loopback, its bytes as they came; not JSON, as text.
  expect(JSON.parse(rewrite.json(addressJSON(port)))).toEqual({ cases: ADDRESS_CASES(port).map(([from, to]) => (from.startsWith('http:\\/') || from.startsWith('u=http%3A') || from.includes('&#116;') ? from : to)), [`${ORIGIN}/key`]: 1, escaped: `${ORIGIN}/x` });
  for (const body of ['{ "a" : "https://example.test/\\u0041",\n "n": 1.50 }', '[1, 2]', '"x"', 'null']) expect(rewrite.json(body)).toBe(body);
  expect(rewrite.json(`not json http://localhost:${port}/`)).toBe(`not json ${ORIGIN}/`);
  // SSE: a JSON data line as JSON, any other line as text.
  expect(rewrite.sse(`data: {"u":"http://localhos\\u0074:${port}/"}\nid: http://localhost:7000/\n\n`)).toBe(`data: {"u":"${ORIGIN}/"}\nid: http://${PLACEHOLDER}/\n\n`);
  // A plain toolOrigin (the local tool's, any other) is refused: the remote tool is B's https door.
  for (const toolOrigin of ['http://127.0.0.1:7740', 'http://localhost:7740', 'http://[::1]:7740', 'http://canvas.test:9442']) expect(() => canvasGateway({ loop: loopBase, pairing: fakePairing(), origin: ORIGIN, toolOrigin, limiter, allows })).toThrow(TypeError);
});

test('loopback addresses are rewritten in headers, HTML, JS, CSS, JSON and SSE; no other host is touched', async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'r', 'read');
    const want = ADDRESS_CASES(port).map(([, to]) => to).join('\n');
    for (const path of ['/addresses.html', '/app.css', '/app2.js']) expect(await (await g.at(path, { headers: { cookie } })).text()).toBe(want);
    const html = await g.at('/addresses.html', { headers: { cookie } });
    expect(html.headers.get('link')).toBe(`<http://${PLACEHOLDER}/x>; rel=preload, <${ORIGIN}/y>; rel=preload`);
    expect(html.headers.get('x-addresses')).toBe(ADDRESS_CASES(port)[0][1]);
    expect(html.headers.get('x-evil')).toBe('http://127.0.0.1%2F@evil.test/a');
    expect(html.headers.get('x-userinfo')).toBe(`http://127.0.0.1;@evil.test/a ${ORIGIN}/u http://${PLACEHOLDER}/v`);
    // JSON (an envelope's type too) by parsing: the escaped spelling caught, the evil.test URL untouched.
    const json = await (await g.at('/addresses.json', { headers: { cookie } })).json();
    expect(json.escaped).toBe(`${ORIGIN}/x`);
    expect(json[`${ORIGIN}/key`]).toBe(1);
    expect(json.cases).toContain('http://127.0.0.1%2F@evil.test/a');
    expect(json.cases).toContain('http://127.0.0.1;@evil.test/a http://127.0.0.1,x@evil.test/b http://a(127.0.0.1)@evil.test/c');
    expect(json.cases).toContain(`${ORIGIN}/u1 http://${PLACEHOLDER}/u2`);
    expect(json.cases).toContain('https://canvas.test:9443/n');
    expect(await (await g.at('/plain.json', { headers: { cookie } })).text()).toBe('{ "a" : "https://example.test/\\u0041",\n "n": 1.50 }');
    const sse = await (await g.at('/__dev/generation/stream', { headers: { cookie } })).text();
    expect(sse).toBe(ADDRESS_CASES(port).map(([, to]) => `data: ${to}\n\n`).join('') + `data: {"page":"${ORIGIN}/","evil":"http://127.0.0.1%2F@evil.test/a"}\n\n`);
  } finally { await g.stop(); }
});

test('Range and If-Range are not passed on, so a rewritten body is never a slice', async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'r', 'read');
    seen.length = 0;
    const r = await g.at('/addresses.html', { headers: { cookie, range: 'bytes=0-9', 'if-range': '"x"' } });
    expect(r.status).toBe(200);
    expect(seen.at(-1).headers.range).toBeUndefined();
    expect(seen.at(-1).headers['if-range']).toBeUndefined();
    expect(await r.text()).toBe(ADDRESS_CASES(port).map(([, to]) => to).join('\n'));
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
      const entry = await g.at(new URL(ok.url).pathname, enterInit(ok.grant));
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
  // (Its gallery page's options at their defaults: the app's own page.)
  const head = "const client = (n, revision, { base = '/__dev', stream = '/__dev/page', page = null } = {}) => `";
  const i = s.indexOf(head) + head.length - 1, j = s.indexOf('</script>`;', i) + '</script>`'.length;
  const names = [...new Set([...s.slice(i, j).matchAll(/\$\{([^}]*)\}/g)].flatMap(m => m[1].match(/\b[A-Z_]{3,}\b/g) ?? []))].filter(n => n !== 'JSON');
  const html = new Function('n', 'revision', 'base', 'stream', 'page', ...names, `return ${s.slice(i, j)};`)(3, 'r', '/__dev', '/__dev/page', null, ...names.map(() => []));
  const code = html.replace(/^<script>/, '').replace(/<\/script>$/, '');
  expect(() => new Function(code)).not.toThrow();
  // The 403 is checked before the failure that keeps the old page.
  const refused = code.indexOf('if(saved.status===403){'), failed = code.indexOf("if(!saved.ok)throw new Error('checkpoint handoff answered '");
  expect(refused).toBeGreaterThan(0);
  expect(refused).toBeLessThan(failed);
  expect(code.slice(refused, failed)).toContain('location.reload();return}');
});

test("an https loop on its default port is the loop's own origin, not another loopback (round 3, Grok)", () => {
  const r = rewriter('https://127.0.0.1', 'https://canvas.test:9443');
  expect(r.text('see https://127.0.0.1/x and https://127.0.0.1:443/y')).toBe('see https://canvas.test:9443/x and https://canvas.test:9443/y');
  expect(r.text('http://127.0.0.1/z')).toBe('http://loopback.invalid/z');
});

test('a compact CSP directive after a loopback source keeps its boundary (round 3 re-check, Astra)', () => {
  const r = rewriter('http://127.0.0.1:9000', 'https://canvas.test:9443');
  expect(r.text("default-src http://127.0.0.1:9000;img-src 'self'")).toBe("default-src https://canvas.test:9443;img-src 'self'");
  expect(r.text("default-src http://127.0.0.1:7005;img-src 'self'")).toBe("default-src http://loopback.invalid;img-src 'self'");
  expect(r.text('a http://localhost:9000,http://127.0.0.1:7005 b')).toBe('a https://canvas.test:9443,http://loopback.invalid b');
  expect(r.text('http://127.0.0.1;@evil.test/a')).toBe('http://127.0.0.1;@evil.test/a');
});

test('the cut of an unparsed authority never lands in its userinfo (round 3 re-check 2, Astra)', () => {
  const r = rewriter('http://127.0.0.1:9000', 'https://canvas.test:9443');
  expect(r.text('(http://127.0.0.1;@evil.test:9000)')).toBe('(http://127.0.0.1;@evil.test:9000)');
  expect(r.text('(http://u,v@evil.test:9000),')).toBe('(http://u,v@evil.test:9000),');
  expect(r.text('(http://user;name@localhost:9000)')).toBe('(https://canvas.test:9443)');
  expect(r.text('(http://user;name@127.0.0.1:7005);')).toBe('(http://loopback.invalid);');
  expect(r.text("default-src http://127.0.0.1:9000;img-src 'self'")).toBe("default-src https://canvas.test:9443;img-src 'self'");
  // Re-check 3: the longest prefix that parses, so a host WHATWG reads whole stays.
  expect(r.text('(http://user;name@localhost;evil.test:9000)')).toBe('(http://user;name@localhost;evil.test:9000)');
  expect(r.text('a http://localhost:9000,http://127.0.0.1:7005 b')).toBe('a https://canvas.test:9443,http://loopback.invalid b');
});

/** A request over TLS to B's per-run certificate, pinned by its SHA-256
 * fingerprint (no CA vouches for it): the answer, or a throw when the
 * certificate is another. */
const pinned = (url, fingerprint, { method = 'GET', headers = {}, body } = {}) => new Promise((ok, fail) => {
  const u = new URL(url);
  const r = httpsRequest({ host: u.hostname, port: u.port, path: u.pathname + u.search, method, headers, rejectUnauthorized: false }, res => {
    if (res.socket.getPeerCertificate().fingerprint256 !== fingerprint) { res.destroy(); return fail(new Error('another certificate')); }
    let text = '';
    res.on('data', c => { text += c; });
    res.on('end', () => ok({ status: res.statusCode, headers: res.headers, text }));
  });
  r.on('error', fail);
  r.end(body);
});

test('the canvas port (C2): on B\'s doors with the same certificate, the entry posted from the canvas\'s own origin, the gateway served at the shown name; none without --remote', async () => {
  const w = await wired('default');
  const said = [];
  let doors = null;
  try {
    // The server's own default gateway: dormant until its port is open.
    const p = await w.pair('read', 'far');
    const before = await w.post({ method: 'canvas' }, p.token, '/__tamber', '198.51.100.20');
    expect([before.status, before.error.message]).toEqual([404, 'this server has no canvas port (--remote)']);
    doors = await openRemoteCanvas(w.tamber, { remote: { host: '127.0.0.1', port: 0 }, tunnel: null }, loopBase, { say: line => said.push(line), warn: () => {} });
    const { origin: tool, fingerprint } = doors.info();
    const canvas = w.tamber.canvas.origin;
    expect(canvas).toMatch(/^https:\/\/127\.0\.0\.1:\d+$/);
    expect(canvas).not.toBe(tool);
    expect(said.at(-1)).toContain(canvas);
    // Minted over B's TLS wire door, the token in its header.
    const minted = JSON.parse((await pinned(`${tool}/__tamber`, fingerprint, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': p.token }, body: '{"method":"canvas"}' })).text).ok;
    expect(minted.url).toBe(`${canvas}/canvas/remote%3Afar%231/`);
    // The tool's page posts the grant: its Origin admitted by the port, the cookie set.
    // The tool's page can't post it (its Origin is refused at the port); the entry page on the canvas's own origin does.
    expect((await pinned(minted.url, fingerprint, { method: 'POST', headers: { origin: tool, 'content-type': 'application/json' }, body: JSON.stringify({ grant: minted.grant }) })).status).toBe(403);
    const entryPage = await pinned(minted.url, fingerprint);
    expect(entryPage.headers['content-security-policy']).toContain(`frame-ancestors ${tool}`);
    const entry = await pinned(minted.url, fingerprint, { method: 'POST', headers: { origin: canvas, 'content-type': 'application/json' }, body: JSON.stringify({ grant: minted.grant }) });
    expect(entry.status).toBe(204);
    const cookie = entry.headers['set-cookie'][0].split(';')[0];
    const page = await pinned(`${canvas}/`, fingerprint, { headers: { cookie } });
    expect(page.status).toBe(200);
    expect(page.text).toContain(`href="${canvas}/x"`);
    // Framed by the remote tool even when B is on loopback: its origin survives the rewriting of loopback addresses.
    expect(page.headers['content-security-policy']).toContain(`frame-ancestors 'self' ${tool}`);
    expect(page.headers['content-security-policy']).not.toContain('7740');
    // Another origin, the tool's too, is refused by the port itself.
    expect((await pinned(`${canvas}/__dev/report`, fingerprint, { method: 'POST', headers: { cookie, origin: 'https://evil.test' }, body: '{}' })).status).toBe(403);
    expect((await pinned(`${canvas}/__dev/report`, fingerprint, { method: 'POST', headers: { cookie, origin: tool }, body: '{}' })).status).toBe(403);
    expect((await pinned(`${canvas}/__dev/report`, fingerprint, { method: 'POST', headers: { cookie, origin: canvas }, body: '{}' })).status).toBe(200);
    // A's revoke: the canvas port refuses the next request, as the wire door does the token.
    await w.post({ method: 'revoke', client: p.client });
    expect((await pinned(`${canvas}/`, fingerprint, { headers: { cookie } })).status).toBe(404);
    expect((await pinned(`${tool}/__tamber`, fingerprint, { method: 'POST', headers: { 'x-exact-tamber-token': p.token }, body: '{"method":"hello"}' })).status).toBe(401);
  } finally { await doors?.close(); await w.stop(); }
  // No --remote: no doors, no port; a tunnel alone: no TLS, no canvas port.
  const v = await wired('default');
  try {
    expect(await openRemoteCanvas(v.tamber, { remote: null, tunnel: null }, loopBase)).toBeNull();
    const tunnel = await openRemoteCanvas(v.tamber, { remote: null, tunnel: 0 }, loopBase, { say: () => {}, warn: () => {} });
    expect(tunnel.info()).toBeNull();
    expect(v.tamber.canvas.origin).toBeNull();
    await tunnel.close();
  } finally { await v.stop(); }
});

// The ghost builds' follow-up (theme2's per-door test): a canvas framed through the TLS door's canvas port stops being
// a build when it goes. Its requests are handed to the port's HTTP server, whose req 'close' never fires under Bun;
// the gateway ends the loop's page stream upstream on res 'close'.
test('a page stream through the canvas port on the TLS door ends upstream when its client leaves', async () => {
  const w = await wired('default');
  let doors = null, stream = null;
  try {
    const p = await w.pair('read', 'far');
    doors = await openRemoteCanvas(w.tamber, { remote: { host: '127.0.0.1', port: 0 }, tunnel: null }, loopBase, { say: () => {}, warn: () => {} });
    const { origin: tool, fingerprint } = doors.info();
    const canvas = w.tamber.canvas.origin;
    const minted = JSON.parse((await pinned(`${tool}/__tamber`, fingerprint, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': p.token }, body: '{"method":"canvas"}' })).text).ok;
    const entry = await pinned(minted.url, fingerprint, { method: 'POST', headers: { origin: canvas, 'content-type': 'application/json' }, body: JSON.stringify({ grant: minted.grant }) });
    const cookie = entry.headers['set-cookie'][0].split(';')[0];
    closed.length = 0;
    const u = new URL(canvas);
    const first = await new Promise((ok, fail) => {
      stream = httpsRequest({ host: u.hostname, port: u.port, path: '/__dev/page', headers: { cookie }, rejectUnauthorized: false }, res => { let got = ''; res.on('data', d => { got += d; if (got.includes('\n\n')) ok(got); }); });
      stream.on('error', fail);
      stream.end();
    });
    expect(first).toContain('"reload":1');
    expect(closed).not.toContain('/__dev/page');
    stream.destroy();
    const t0 = performance.now();
    while (!closed.includes('/__dev/page') && performance.now() - t0 < 5000) await new Promise(ok => setTimeout(ok, 5));
    expect(closed).toContain('/__dev/page');
  } finally { stream?.destroy(); await doors?.close(); await w.stop(); }
}, 30_000);

test('plain HTTP from a LAN peer (a --lan bind): no session, grant or entry page is honoured by the gateway, and `canvas` is not minted for it', async () => {
  const g = await gateway();
  try {
    const { cookie } = await enter(g, 'lan', 'read');
    expect((await g.at('/', { headers: { cookie } })).status).toBe(200);
    const token = g.pairing.add('lan2', 'read');
    const { ok } = g.gw.mint(token);
    for (const door of [{ allowed: true, local: false }, { allowed: true, local: false, peer: '192.168.1.30' }, undefined]) {
      g.through(door);
      for (const r of [await g.at('/', { headers: { cookie } }), await g.at('/canvas/lan2/'), await g.at('/canvas/lan2/', enterInit(ok.grant))]) {
        expect(r.status).toBe(404);
        expect(r.headers.get('set-cookie')).toBeNull();
      }
    }
    // None of those read the grant: through the TLS door it still enters, once.
    g.through({ allowed: true, local: false, remote: true, port: 'canvas' });
    expect((await g.at('/canvas/lan2/', enterInit(ok.grant))).status).toBe(204);
  } finally { await g.stop(); }
  const w = await wired();
  try {
    const p = await w.pair('read', 'lan');
    const plain = await w.post({ method: 'canvas' }, p.token, '/__tamber', '192.168.1.30', false);
    expect(plain.status).toBe(404);
    expect(plain.ok).toBeUndefined();
    // Through B's door it is.
    expect((await w.post({ method: 'canvas' }, p.token, '/__tamber', '192.168.1.30')).status).toBe(200);
  } finally { await w.stop(); }
});

test('the tool origin on the entry page is a plain https origin, written as an attribute value (C2 re-check, Astra)', async () => {
  const pairing = fakePairing();
  expect(() => canvasGateway({ loop: loopBase, pairing, origin: ORIGIN, toolOrigin: 'https://tool".test', limiter, allows })).toThrow(/plain https origin/);
  expect(() => canvasGateway({ loop: loopBase, pairing, origin: ORIGIN, toolOrigin: 'https://a&b.test', limiter, allows })).toThrow(/plain https origin/);
  const g = await gateway();
  try {
    const html = await (await g.at('/canvas/a/')).text();
    expect(/<html lang="en" data-tool="([^"]*)">/.exec(html)?.[1]).toBe(TOOL);
  } finally { await g.stop(); }
});
