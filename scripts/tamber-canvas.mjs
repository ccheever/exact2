// The canvas gateway (LLP 1121 stage 10 C, LLP 1122 D12): a paired client's
// view of the app's dev URL, served by the Tamber server under its paired
// token on the canvas's own origin (B's second TLS port), never the loop's
// loopback URL handed out.
//
// - `mint(token)`, the `canvas` method's body (`read`): a single-use grant
//   (256 bits, 30 s, bound to the client) and the entry URL,
//   `<origin>/canvas/<client>/`. Rate-limited per client with A's `limiter`.
// - `POST /canvas/<client>/` with `grant` in the body (form or JSON, never
//   a query string): the grant compared timing-safe and burned, a session id
//   (256 bits) bound here to the client, its token and that grant, set as
//   `__Host-exact-canvas` (Secure, HttpOnly, SameSite=Strict, Path=/), and a
//   303 to `/`. The cookie only names a session: a value this map didn't
//   mint (page code can set a cookie of that name; cookies aren't
//   port-scoped) is an unknown session, and a token is never read from it.
// - Every other request: the session, then its client's token through A's
//   `lookup` (a revoked or expired token fails at once), then the route's
//   scope through `allows`. An unknown, missing, revoked or expired session
//   is not-found, the same answer for each; a scope refused is 403 naming
//   both scopes. Allowed POSTs also need `Origin` to be the canvas's own.
// - The allow list (`needs`): `read` for the page and its assets (GET/HEAD
//   outside `/__…` and `/.exact/`), the dev streams and reads, GET
//   `/__dev/design` and the page's beacons; `write` (theme2's ruling) for
//   POST `/__dev/design`, `/__dev/checkpoint` and `/__dev/open`. Never
//   `/__tamber…`: through the proxy the loop would see a local request.
// - Upstream: over loopback to the loop as itself, `redirect: 'manual'`, the
//   query kept, Host the loop's, Origin the loop's on a POST; Cookie, the
//   tamber token, forwarding and hop-by-hop headers dropped; bodies at most
//   1 MiB, a checkpoint the loop's own 16 MiB. One
//   `forwarded` marker of the gateway's own is added, so the dev gate (as
//   it is) calls the request not local: no install or tamber token can be
//   handed to it whatever path the allow list someday passes.
// - Downstream: Set-Cookie, CORS and hop-by-hop headers dropped; every
//   loopback form of the loop's origin, in headers and in HTML, JSON and SSE
//   bodies (raw and URL-encoded), rewritten to `origin`; `frame-ancestors`
//   names the tool's remote origin (`toolOrigin`) instead of any loopback
//   one; `cache-control: no-store`; bodies streamed with backpressure.
// - Revoke and expiry (A's `onEnd`): the client's live proxied responses
//   (streams, long polls, in-flight requests) are aborted with their
//   upstream fetches, and its sessions and grants deleted.
import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';

export const CANVAS_COOKIE = '__Host-exact-canvas';
export const GRANT_MS = 30_000;
// forward()'s limit in scripts/tamber-server.mjs: the server's line, less its id.
export const BODY_LIMIT = (1 << 20) - 64;
// `/__dev/checkpoint` (write only) takes the loop's own cap: host/web-js/dev.mjs's
// CHECKPOINT_BYTES, mirrored as that file is a script (importing it starts a
// loop); a test reads it there so the two can't drift.
export const CHECKPOINT_LIMIT = 16 * 1024 * 1024;
// Grants per client: a burst of 6, refilled at 6 a minute (A's token bucket).
export const MINT_RATE = { per: 60_000, rate: 6 };
const ENTRY_LIMIT = 4096, SESSIONS_PER_CLIENT = 16, GRANTS_PER_CLIENT = 8;

const READS = new Set(['/__dev', '/__dev/page', '/__dev/hello', '/__dev/commands', '/__dev/inspect.js', '/__dev/design', '/dev-inspect.json', '/exact.json']);
const READ_PREFIXES = ['/__dev/generation/', '/__dev/map/', '/modules/'];
const BEACONS = new Set(['/__dev/reloaded', '/__dev/painted', '/__dev/gpu', '/__dev/report']);
const WRITES = new Set(['/__dev/design', '/__dev/checkpoint', '/__dev/open']);
const ENTRY = /^\/canvas\/([^/]+)\/$/;
const HOP = new Set(['connection', 'keep-alive', 'proxy-authenticate', 'proxy-authorization', 'proxy-connection', 'te', 'trailer', 'transfer-encoding', 'upgrade']);
const DROP_UP = new Set(['host', 'cookie', 'origin', 'referer', 'x-exact-tamber-token', 'forwarded', 'x-real-ip', 'content-length', 'accept-encoding']);
const DROP_DOWN = new Set(['set-cookie', 'set-cookie2', 'content-length', 'content-encoding', 'cache-control', 'alt-svc', 'strict-transport-security']);

/** The scope a request needs, or null when the gateway never passes it. */
export function needs(method, path) {
  if (path === '/__tamber' || path.startsWith('/__tamber/')) return null;
  if (method === 'POST') return BEACONS.has(path) ? 'read' : WRITES.has(path) ? 'write' : null;
  if (method !== 'GET' && method !== 'HEAD') return null;
  // `/__dev/open` is the loop's opening page; theme2 ruled it `write`.
  if (path === '/__dev/open') return 'write';
  if (READS.has(path) || READ_PREFIXES.some(p => path.startsWith(p))) return 'read';
  if (path.startsWith('/__') || path.startsWith('/.exact/') || ENTRY.test(path)) return null;
  return 'read';
}

const sha = value => createHash('sha256').update(value).digest();
const same = (a, b) => a.length === b.length && timingSafeEqual(a, b);
const escape = s => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** Every loopback form of `loop`'s origin in a text, as `origin`: a URL
 * (http or https), its URL-encoded form, and the bare host and port. */
export function rewriter(loop, origin) {
  const port = new URL(loop).port;
  const to = new URL(origin);
  const hosts = ['127\\.0\\.0\\.1', 'localhost', '\\[::1\\]'];
  const encoded = ['127\\.0\\.0\\.1', 'localhost', '%5B%3A%3A1%5D'];
  const rules = [
    [new RegExp(`https?://(?:${hosts.join('|')}):${port}(?!\\d)`, 'gi'), to.origin],
    [new RegExp(`https?%3A%2F%2F(?:${encoded.join('|')})%3A${port}(?!\\d)`, 'gi'), encodeURIComponent(to.origin)],
    [new RegExp(`(?:${hosts.join('|')}):${port}(?!\\d)`, 'gi'), to.host],
    [new RegExp(`(?:${encoded.join('|')})%3A${port}(?!\\d)`, 'gi'), encodeURIComponent(to.host)],
  ];
  return text => rules.reduce((t, [from, by]) => t.replace(from, by), text);
}

const LOOPBACK_SOURCE = /^(?:https?:\/\/)?(?:127\.0\.0\.1|localhost|\[::1\])(?::\d+)?\/?$/i;
/** A CSP's `frame-ancestors`: the loop's origin as the canvas's, any other
 * loopback source (the local tool's) gone, the tool's remote origin added. */
function frameAncestors(policy, loopOrigin, origin, toolOrigin) {
  return policy.split(',').map(one => one.split(';').map(d => {
    const words = d.trim().split(/\s+/);
    if (words[0]?.toLowerCase() !== 'frame-ancestors') return d;
    const kept = words.slice(1).map(s => s.replace(/\/$/, '') === loopOrigin ? origin : s).filter(s => !LOOPBACK_SOURCE.test(s));
    if (toolOrigin && !kept.includes(toolOrigin)) kept.push(toolOrigin);
    return ` frame-ancestors ${kept.length ? kept.join(' ') : "'none'"}`;
  }).join(';')).join(',');
}

/** Each value of the cookie `name` in a Cookie header, in order (page code
 * may have set more than one). */
function cookies(header, name) {
  return [header ?? []].flat().join(';').split(';').map(p => p.trim()).filter(p => p.startsWith(`${name}=`)).map(p => p.slice(name.length + 1)).slice(0, 4);
}

async function readBody(req, limit) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    // Over: the caller answers (413, then the connection closed).
    if (size > limit) return null;
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

/** `write` with backpressure: resolved once `res` drains or closes. */
const drained = res => new Promise(ok => {
  const done = () => { res.off('drain', done); res.off('close', done); ok(); };
  res.on('drain', done); res.on('close', done);
});
const put = async (res, chunk) => { if (!res.write(chunk)) await drained(res); };

/** The gateway for the dev loop at `loop` (its loopback origin), served on
 * `origin` (the canvas's public origin), framed by `toolOrigin`. A's
 * interface (scripts/tamber-pairing.mjs): `pairing` is the run's, as
 * `tamberServer(...).pairing` returns it (`lookup`, `onEnd`); `limiter`
 * (`limiter({rate, per, clock}) → {take, wait}`) and `allows(scope, needs,
 * local)` are that module's exports. TODO(stage 10 A): default `limiter` and
 * `allows` to them once it lands. `log` hears each client's end and its
 * reason (`'revoked'` or `'expired'`), never a secret. */
export function canvasGateway({ loop, pairing, origin, toolOrigin = null, now = Date.now, limiter, allows, log = () => {} }) {
  if (typeof limiter !== 'function' || typeof allows !== 'function') throw new TypeError('canvasGateway takes A\'s `limiter` and `allows`');
  // One bucket per client, for grant minting.
  const mints = limiter({ ...MINT_RATE, clock: now });
  // Every canvas request is remote: never the local token's `local`.
  const may = (scope, need) => allows(scope, need, false);
  const loopOrigin = new URL(loop).origin, loopHost = new URL(loop).host;
  origin = new URL(origin).origin;
  if (toolOrigin) toolOrigin = new URL(toolOrigin).origin;
  const rewrite = rewriter(loopOrigin, origin);
  // Keyed by SHA-256 of the secret, so no lookup compares a secret's bytes.
  const grants = new Map(); // hash hex → {hash, client, token, expires}
  const sessions = new Map(); // hash hex → {client, token, grant}
  const live = new Map(); // client → Set<AbortController>
  const ends = new Map(); // client → unsubscribe
  let closed = false;

  const valid = token => {
    const rec = token ? pairing.lookup(token) : null;
    return rec && !(rec.expires != null && rec.expires <= now()) ? rec : null;
  };
  // Revoked or expired: the same end.
  const end = (client, reason = 'closed') => {
    if (reason !== 'closed') log(`canvas: ${client} ${reason}: its proxied streams end`);
    ends.get(client)?.();
    ends.delete(client);
    for (const gone of live.get(client) ?? []) gone.abort();
    live.delete(client);
    for (const [k, s] of sessions) if (s.client === client) sessions.delete(k);
    for (const [k, g] of grants) if (g.client === client) grants.delete(k);
  };
  const watch = client => {
    if (ends.has(client)) return;
    // A's `onEnd` calls `fn` at once for a client already ended: then
    // `end` has run (and deleted the key) before the unsubscribe is kept.
    ends.set(client, null);
    const unsubscribe = pairing.onEnd(client, reason => end(client, reason));
    if (ends.has(client)) ends.set(client, unsubscribe); else unsubscribe?.();
  };
  const notFound = res => { res.writeHead(404, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end('{"message":"Not found."}'); };
  const json = (res, status, body, headers = {}) => { res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store', ...headers }); res.end(JSON.stringify(body)); };

  function mint(token) {
    if (closed) return { status: 503, error: { message: 'the canvas gateway is closed' } };
    const rec = valid(token);
    if (!rec) return { status: 401, error: { message: 'no such client' } };
    if (!may(rec.scope, 'read')) return { status: 403, error: { message: 'canvas needs read', scope: rec.scope, needs: 'read' } };
    if (!mints.take(`canvas:${rec.client}`)) return { status: 429, error: { message: 'too many canvas grants; try again in a minute' } };
    const at = now();
    for (const [k, g] of grants) if (g.expires <= at) grants.delete(k);
    const mine = [...grants].filter(([, g]) => g.client === rec.client);
    for (const [k] of mine.slice(0, Math.max(0, mine.length - GRANTS_PER_CLIENT + 1))) grants.delete(k);
    const grant = randomBytes(32).toString('base64url');
    const hash = sha(grant);
    grants.set(hash.toString('hex'), { hash, client: rec.client, token, expires: at + GRANT_MS });
    watch(rec.client);
    return { ok: { url: `${origin}/canvas/${encodeURIComponent(rec.client)}/`, grant, expires: at + GRANT_MS } };
  }

  async function enter(req, res, url, client) {
    // POST only, the grant only in the body: any query is refused.
    if (req.method !== 'POST' || url.search) return notFound(res);
    const given = req.headers.origin;
    if (given !== undefined && given !== origin && given !== toolOrigin) return notFound(res);
    const bytes = await readBody(req, ENTRY_LIMIT);
    if (!bytes) { res.on('finish', () => req.destroy?.()); return notFound(res); }
    const text = bytes.toString('utf8'), type = String(req.headers['content-type'] ?? '');
    let grant = null;
    try {
      grant = /^application\/json\b/i.test(type) ? JSON.parse(text)?.grant : /^application\/x-www-form-urlencoded\b/i.test(type) ? new URLSearchParams(text).get('grant') : null;
    } catch { grant = null; }
    if (typeof grant !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(grant)) return notFound(res);
    const hash = sha(grant), key = hash.toString('hex');
    const held = grants.get(key);
    // Burned whatever follows: a grant is tried once.
    grants.delete(key);
    if (!held || !same(held.hash, hash) || held.client !== client || held.expires <= now()) return notFound(res);
    const rec = valid(held.token);
    if (!rec || rec.client !== client || !may(rec.scope, 'read')) return notFound(res);
    const session = randomBytes(32).toString('base64url');
    const mine = [...sessions].filter(([, s]) => s.client === client);
    for (const [k] of mine.slice(0, Math.max(0, mine.length - SESSIONS_PER_CLIENT + 1))) sessions.delete(k);
    sessions.set(sha(session).toString('hex'), { client, token: held.token, grant: key });
    watch(client);
    res.writeHead(303, { location: '/', 'cache-control': 'no-store', 'set-cookie': `${CANVAS_COOKIE}=${session}; Secure; HttpOnly; SameSite=Strict; Path=/` });
    res.end();
  }

  /** The session a request's cookie names, with its token still good. */
  function sessionOf(req) {
    for (const value of cookies(req.headers.cookie, CANVAS_COOKIE)) {
      if (!/^[A-Za-z0-9_-]{43}$/.test(value)) continue;
      const s = sessions.get(sha(value).toString('hex'));
      if (!s) continue;
      const rec = valid(s.token);
      if (rec && rec.client === s.client) return { ...s, scope: rec.scope };
    }
    return null;
  }

  async function handle(req, res, url) {
    if (closed) { notFound(res); return true; }
    const entry = ENTRY.exec(url.pathname);
    if (entry) {
      let client = null;
      try { client = decodeURIComponent(entry[1]); } catch { /* malformed */ }
      if (client == null) notFound(res); else await enter(req, res, url, client);
      return true;
    }
    const session = sessionOf(req);
    const need = needs(req.method, url.pathname);
    if (!session || !need) { notFound(res); return true; }
    if (!may(session.scope, need)) { json(res, 403, { message: `${req.method} ${url.pathname} needs ${need}`, scope: session.scope, needs: need }); return true; }
    if (req.method === 'POST' && req.headers.origin !== origin) { notFound(res); return true; }
    await proxy(req, res, url, session.client);
    return true;
  }

  async function proxy(req, res, url, client) {
    const gone = new AbortController();
    const held = live.get(client) ?? new Set();
    live.set(client, held);
    held.add(gone);
    const cut = () => gone.abort();
    res.on('close', cut);
    try {
      const named = new Set(String(req.headers.connection ?? '').toLowerCase().split(',').map(s => s.trim()).filter(Boolean));
      const headers = {};
      for (const [k, v] of Object.entries(req.headers)) {
        const key = k.toLowerCase();
        if (HOP.has(key) || named.has(key) || DROP_UP.has(key) || key.startsWith('x-forwarded-') || key.startsWith(':')) continue;
        headers[key] = [v].flat().join(', ');
      }
      headers.host = loopHost;
      // A proxy's marker, fixed (no peer address in it): the dev gate, as it
      // is (install-page.mjs), then admits the request by its loopback Host
      // but never calls it local, so the loop hands this request neither its
      // install token (installer.page) nor its tamber token, whatever path the
      // allow list someday passes. The client's own forwarding headers are
      // dropped above.
      headers.forwarded = 'for="_exact-canvas"';
      if (req.method === 'POST') headers.origin = loopOrigin;
      let body;
      if (req.method !== 'GET' && req.method !== 'HEAD') {
        const limit = url.pathname === '/__dev/checkpoint' ? CHECKPOINT_LIMIT : BODY_LIMIT;
        body = await readBody(req, limit);
        if (!body) {
          res.on('finish', () => req.destroy?.());
          json(res, 413, { error: { message: `a request is at most ${limit === BODY_LIMIT ? '1' : limit >> 20} MiB` } }, { connection: 'close' });
          return;
        }
      }
      if (gone.signal.aborted) throw new Error('ended');
      const answer = await fetch(`${loopOrigin}${url.pathname}${url.search}`, { method: req.method, headers, body, redirect: 'manual', signal: gone.signal });
      const out = {};
      const dropNamed = new Set(String(answer.headers.get('connection') ?? '').toLowerCase().split(',').map(s => s.trim()));
      for (const [k, v] of answer.headers) {
        const key = k.toLowerCase();
        if (HOP.has(key) || dropNamed.has(key) || DROP_DOWN.has(key) || key.startsWith('access-control-')) continue;
        out[key] = key === 'content-security-policy' || key === 'content-security-policy-report-only' ? rewrite(frameAncestors(v, loopOrigin, origin, toolOrigin)) : rewrite(v);
      }
      out['cache-control'] = 'no-store';
      const type = String(out['content-type'] ?? '');
      const stream = /^text\/event-stream\b/i.test(type);
      const text = /^(?:text\/html|application\/(?:[\w.+-]+\+)?json)\b/i.test(type);
      if (req.method === 'HEAD' || !answer.body) { res.writeHead(answer.status, out); res.end(); return; }
      if (text && !stream) {
        // A page or JSON whole, so a URL split over two chunks is rewritten.
        const bytes = Buffer.from(rewrite(Buffer.from(await answer.arrayBuffer()).toString('utf8')));
        res.writeHead(answer.status, out);
        res.end(bytes);
        return;
      }
      res.writeHead(answer.status, out);
      if (stream) {
        res.flushHeaders?.();
        // Rewritten a line at a time: a line is never split.
        const decoder = new TextDecoder();
        let rest = '';
        for await (const chunk of answer.body) {
          rest += decoder.decode(chunk, { stream: true });
          const at = rest.lastIndexOf('\n');
          if (at < 0) continue;
          await put(res, rewrite(rest.slice(0, at + 1)));
          rest = rest.slice(at + 1);
        }
        if (rest) await put(res, rewrite(rest));
      } else for await (const chunk of answer.body) await put(res, chunk);
      res.end();
    } catch {
      // Cut part way (revoked, or either side gone): the answer begun cannot become another.
      if (res.headersSent) res.destroy();
      else if (!res.destroyed) {
        if (gone.signal.aborted) notFound(res);
        else json(res, 502, { error: { message: 'the dev loop did not answer' } });
      }
    } finally {
      res.off('close', cut);
      held.delete(gone);
      if (!held.size && live.get(client) === held) live.delete(client);
    }
  }

  function close() {
    closed = true;
    for (const client of new Set([...ends.keys(), ...live.keys()])) end(client);
    grants.clear();
    sessions.clear();
  }

  return { mint, handle, close, get live() { return [...live.values()].reduce((n, s) => n + s.size, 0); } };
}
