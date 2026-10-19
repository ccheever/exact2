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
//   both scopes. Allowed POSTs also need `Origin` to be the canvas's own;
//   the entry needs an explicit Origin, the canvas's or the tool's.
// - The raw request target is checked (`canonical`): one the loop could
//   read as another path (an empty or dot segment, an encoded separator,
//   dot or unreserved character, a backslash or control character) is
//   not-found, and the path forwarded is exactly the path authorized.
// - The allow list (`needs`): `read` for the page and its assets (GET/HEAD
//   outside `/__…` and `/.exact`, in any case), the dev streams and reads, GET
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
// - Downstream: Set-Cookie, CORS and hop-by-hop headers dropped. Loopback
//   addresses (a URL or a bare host:port whose host WHATWG URL parses as
//   loopback, at any port) in headers and text bodies (HTML, JS, CSS) are
//   rewritten, and JSON bodies (and SSE's JSON data lines) are parsed and
//   each string rewritten: the loop's own to `origin`, any other to
//   `loopback.invalid`, no other host ever touched. Escaped spellings in
//   text are a known limit (functional, not a boundary: theme2). Range is
//   not passed on, so a rewritten body is never a slice. `frame-ancestors`
//   names the tool's remote origin (`toolOrigin`, never a loopback one)
//   instead of any loopback source; `cache-control: no-store`; bodies
//   streamed with backpressure.
// - Revoke and expiry (A's `onEnd`): each of the client's proxied requests
//   (streams, long polls, uploads, downloads a client stopped reading) is
//   ended at once (its upstream fetch aborted, its request and response
//   destroyed), and its sessions and grants deleted. A session evicted past
//   the cap ends its own; a client has at most 32 in flight.
import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';
import { allows as pairingAllows, limiter as pairingLimiter } from './tamber-pairing.mjs';

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
// Per client: live sessions, unexpired grants, and requests proxied at once
// (the page's two streams, a long poll and its assets fit many times over).
const ENTRY_LIMIT = 4096, SESSIONS_PER_CLIENT = 16, GRANTS_PER_CLIENT = 8, LIVE_PER_CLIENT = 32;

const READS = new Set(['/__dev', '/__dev/page', '/__dev/hello', '/__dev/commands', '/__dev/inspect.js', '/__dev/design', '/dev-inspect.json', '/exact.json']);
const READ_PREFIXES = ['/__dev/generation/', '/__dev/map/', '/modules/'];
const BEACONS = new Set(['/__dev/reloaded', '/__dev/painted', '/__dev/gpu', '/__dev/report']);
const WRITES = new Set(['/__dev/design', '/__dev/checkpoint', '/__dev/open']);
const ENTRY = /^\/canvas\/([^/]+)\/$/;
const HOP = new Set(['connection', 'keep-alive', 'proxy-authenticate', 'proxy-authorization', 'proxy-connection', 'te', 'trailer', 'transfer-encoding', 'upgrade']);
const DROP_UP = new Set(['host', 'cookie', 'origin', 'referer', 'x-exact-tamber-token', 'forwarded', 'x-real-ip', 'content-length', 'accept-encoding', 'range', 'if-range']);
const DROP_DOWN = new Set(['set-cookie', 'set-cookie2', 'content-length', 'content-encoding', 'cache-control', 'alt-svc', 'strict-transport-security']);

/** The request target as the gateway authorizes and forwards it,
 * `{path, query}`, or null for any target the loop could read as another
 * path: not origin-form, an empty segment (a leading `//` too), a `.` or
 * `..` segment, a backslash, NUL or other control character, an encoded
 * `/`, `\`, NUL or `.`, any percent-encoded unreserved character (which
 * decodes to the same path) and a malformed escape. The path forwarded is
 * exactly the path checked. */
export function canonical(target) {
  if (typeof target !== 'string' || !target.startsWith('/') || target.includes('#')) return null;
  const q = target.indexOf('?');
  const path = q < 0 ? target : target.slice(0, q), query = q < 0 ? '' : target.slice(q);
  if (path.includes('//') || /[\\\x00-\x20\x7f]/.test(path) || /[\x00-\x20\x7f]/.test(query)) return null;
  if (/%(?![0-9a-f]{2})/i.test(path) || /%(?:00|2e|2f|5c|2d|5f|7e|3[0-9]|4[1-9a-f]|5[0-9a]|6[1-9a-f]|7[0-9a])/i.test(path)) return null;
  if (path.split('/').some(seg => seg === '.' || seg === '..')) return null;
  return { path, query };
}

/** The scope a canonical path needs, or null when the gateway never passes
 * it. The reserved prefixes compare without case. */
export function needs(method, path) {
  const lower = path.toLowerCase();
  if (lower === '/__tamber' || lower.startsWith('/__tamber/')) return null;
  if (method === 'POST') return BEACONS.has(path) ? 'read' : WRITES.has(path) ? 'write' : null;
  if (method !== 'GET' && method !== 'HEAD') return null;
  // `/__dev/open` is the loop's opening page; theme2 ruled it `write`.
  if (path === '/__dev/open') return 'write';
  if (READS.has(path) || READ_PREFIXES.some(p => path.startsWith(p))) return 'read';
  if (lower.startsWith('/__') || lower.startsWith('/.exact') || ENTRY.test(path)) return null;
  return 'read';
}

const sha = value => createHash('sha256').update(value).digest();
const same = (a, b) => a.length === b.length && timingSafeEqual(a, b);

/** Where an address that is no loop of ours points instead: never routed. */
export const PLACEHOLDER = 'loopback.invalid';
/** Whether a host, as WHATWG URL parses it, is this machine's (one
 * trailing dot dropped). */
const isLoopback = parsed => {
  const host = parsed.endsWith('.') ? parsed.slice(0, -1) : parsed;
  return host === 'localhost' || host.endsWith('.localhost') || /^127\.\d+\.\d+\.\d+$/.test(host) || host === '0.0.0.0'
    || host === '[::1]' || host === '[::]' || /^\[::ffff:7f[0-9a-f]{2}:[0-9a-f]{1,4}\]$/.test(host);
};
/** An authority's port when WHATWG URL parses its host as loopback, else
 * null. Nothing is decoded first: `127.0.0.1%2F@evil.test` is evil.test's. */
function loopbackPort(authority, scheme = 'http:') {
  let u;
  try { u = new URL(`${scheme}//${authority}/`); } catch { return null; }
  if (!isLoopback(u.hostname)) return null;
  return u.port || (scheme === 'https:' || scheme === 'wss:' ? '443' : '80');
}

// Candidates, each decided by WHATWG URL: a web URL or a scheme-relative
// one (its authority up to the first delimiter, whitespace, quote or
// bracket of the text around it), and a bare `host:port` standing alone
// (not in a path, not inside a longer name). Escaped spellings (HTML or
// JSON escapes in text, percent-encoded URLs) are a known limit (§10).
const ADDRESSES = new RegExp(
  String.raw`(?<![\w+.\-:\/\\%])(?<scheme>(?:https?|wss?):)?\/\/(?<auth>[^\s"'<>\x60\/?#\\(){}|^,;]+)`
  + String.raw`|(?<![\w.\-\/\\%@:\[])(?<bare>\[[0-9a-f:.]+\]|[a-z0-9](?:[a-z0-9.\-]*[a-z0-9])?\.?):(?<port>\d{1,5})(?![\w\-]|\.\w)`, 'gi');

/** The rewriting of loopback addresses for a canvas client: the loop's own
 * (any loopback host at its port) as `origin`; any other loopback address
 * as PLACEHOLDER; no other host ever touched. `text` for headers, HTML and
 * other text; `json` parses a JSON body and rewrites each string in it (its
 * bytes unchanged when nothing is); `sse` a stream's lines, a `data:` line
 * that is JSON as JSON. */
export function rewriter(loop, origin) {
  const from = new URL(loop), to = new URL(origin);
  const port = from.port || '80';
  const text = t => t.replace(ADDRESSES, (m, ...args) => {
    const g = args.at(-1);
    if (g.auth !== undefined) {
      const scheme = (g.scheme ?? '').toLowerCase();
      const p = loopbackPort(g.auth, scheme || 'http:');
      if (p == null) return m;
      if (p !== port) return `${scheme}//${PLACEHOLDER}`;
      return `${!scheme ? '' : scheme.startsWith('ws') ? 'wss:' : to.protocol}//${to.host}`;
    }
    const p = loopbackPort(`${g.bare}:${g.port}`);
    return p == null ? m : p === port ? to.host : PLACEHOLDER;
  });
  const json = body => {
    let value;
    try { value = JSON.parse(body); } catch { return text(body); }
    let changed = false;
    const walk = v => {
      if (typeof v === 'string') { const w = text(v); if (w !== v) changed = true; return w; }
      if (Array.isArray(v)) return v.map(walk);
      if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [walk(k), walk(x)]));
      return v;
    };
    const out = walk(value);
    return changed ? JSON.stringify(out) : body;
  };
  const sse = lines => lines.split('\n').map(line => {
    const at = /^data: ?/.exec(line);
    return at ? at[0] + json(line.slice(at[0].length)) : text(line);
  }).join('\n');
  return { text, json, sse };
}

/** A CSP's `frame-ancestors`: the loop's origin as the canvas's, any other
 * loopback source (the local tool's) gone, the tool's remote origin added. */
function frameAncestors(policy, loopPort, origin, toolOrigin) {
  const loopbackSource = s => {
    const bare = s.replace(/^[a-z][a-z0-9+.-]*:\/\//i, '');
    const scheme = /^(https?|wss?):\/\//i.exec(s)?.[1];
    return /^\w|^\[/.test(bare) ? loopbackPort(bare.replace(/\/.*$/, ''), scheme ? `${scheme.toLowerCase()}:` : undefined) : null;
  };
  return policy.split(',').map(one => one.split(';').map(d => {
    const words = d.trim().split(/\s+/);
    if (words[0]?.toLowerCase() !== 'frame-ancestors') return d;
    const kept = words.slice(1).flatMap(s => { const p = loopbackSource(s); return p == null ? [s] : p === loopPort ? [origin] : []; });
    if (toolOrigin && !kept.includes(toolOrigin)) kept.push(toolOrigin);
    return ` frame-ancestors ${kept.length ? kept.join(' ') : "'none'"}`;
  }).join(';')).join(',');
}

/** Each value of the cookie `name` in a Cookie header, in order (page code
 * may have set more than one). */
function cookies(header, name) {
  return [header ?? []].flat().join(';').split(';').map(p => p.trim()).filter(p => p.startsWith(`${name}=`)).map(p => p.slice(name.length + 1)).slice(0, 4);
}

/** A request's body up to `limit`: `{body}`, `{over}` (the caller answers
 * 413 and closes), or `{gone}` (the client went, or `signal` aborted: a
 * stalled upload ends with its revoke). */
const readBody = (req, limit, signal) => new Promise(done => {
  const chunks = [];
  let size = 0, settled = false;
  const finish = v => {
    if (settled) return;
    settled = true;
    req.off('data', data); req.off('end', end); req.off('error', gone); req.off('close', gone);
    signal?.removeEventListener('abort', gone);
    done(v);
  };
  const data = chunk => { size += chunk.length; if (size > limit) { req.pause(); finish({ over: true }); } else chunks.push(chunk); };
  const end = () => finish({ body: Buffer.concat(chunks) });
  const gone = () => finish({ gone: true });
  if (signal?.aborted || req.destroyed) return gone();
  req.on('data', data); req.on('end', end); req.on('error', gone); req.on('close', gone);
  signal?.addEventListener('abort', gone, { once: true });
});

/** `write` with backpressure: resolved once `res` drains or closes, or
 * `signal` aborts (a client that never reads ends with its revoke). */
const drained = (res, signal) => new Promise(ok => {
  const done = () => { res.off('drain', done); res.off('close', done); signal.removeEventListener('abort', done); ok(); };
  res.on('drain', done); res.on('close', done); signal.addEventListener('abort', done, { once: true });
});
const put = async (res, chunk, signal) => {
  if (signal.aborted) throw new Error('ended');
  if (!res.write(chunk)) await drained(res, signal);
  if (signal.aborted) throw new Error('ended');
};

/** The gateway for the dev loop at `loop` (its loopback origin), served on
 * `origin` (the canvas's public origin), framed by `toolOrigin`. Stage 10
 * A's interface (scripts/tamber-pairing.mjs): `pairing` is the run's, as
 * `tamberServer(...).pairing` returns it. Each request acts as `lookup`'s
 * client for the session's token, and `onEnd` (at once for a client already
 * ended) ends its streams. `limiter` and `allows` are that module's.
 * `log` hears each client's end and its reason (`'revoked'` or
 * `'expired'`), never a secret. A server makes it with
 * `tamberServer({canvas: pairing => canvasGateway({loop, pairing, origin,
 * toolOrigin})})`, and answers `canvas` with its `mint`. */
export function canvasGateway({ loop, pairing, origin, toolOrigin = null, now = Date.now, limiter = pairingLimiter, allows = pairingAllows, log = () => {} }) {
  if (typeof limiter !== 'function' || typeof allows !== 'function') throw new TypeError('canvasGateway takes A\'s `limiter` and `allows`');
  // One bucket per client, for grant minting.
  const mints = limiter({ ...MINT_RATE, clock: now });
  // Every canvas request is remote: never the local token's `local`.
  const may = (scope, need) => allows(scope, need, false);
  const loopOrigin = new URL(loop).origin, loopHost = new URL(loop).host, loopPort = new URL(loop).port || '80';
  origin = new URL(origin).origin;
  if (toolOrigin) {
    toolOrigin = new URL(toolOrigin).origin;
    // The remote tool's origin: a loopback one would frame the canvas from this machine's other services.
    if (isLoopback(new URL(toolOrigin).hostname)) throw new TypeError('canvasGateway: `toolOrigin` is the tool\'s remote origin, never a loopback one');
  }
  const rewrite = rewriter(loopOrigin, origin);
  // Keyed by SHA-256 of the secret, and the hash compared timing-safe.
  const grants = new Map(); // hash hex → {hash, client, token, expires}
  const sessions = new Map(); // hash hex → {hash, client, token, grant}
  const live = new Map(); // client → Set<{session, gone, req, res}>, until each response closes
  const ends = new Map(); // client → unsubscribe
  let closed = false;

  const valid = token => {
    const rec = token ? pairing.lookup(token) : null;
    return rec && !(rec.expires != null && rec.expires <= now()) ? rec : null;
  };
  /** A proxied request ended at once: its upstream fetch aborted, its
   * upload and its response destroyed whatever they wait on. */
  const kill = ({ gone, req, res }) => {
    gone.abort();
    // Not yet answered: not-found, the same as any refused request, then the
    // connection closed; answered in part: the connection cut.
    if (!res.headersSent && !res.writableEnded && !res.destroyed) { res.on('finish', () => { req.destroy(); res.socket?.destroy(); }); notFound(res, true); return; }
    req.destroy(); res.destroy(); res.socket?.destroy();
  };
  const dropSession = key => {
    const s = sessions.get(key);
    if (!s) return;
    sessions.delete(key);
    for (const flight of live.get(s.client) ?? []) if (flight.session === key) kill(flight);
  };
  // Revoked or expired: the same end.
  const end = (client, reason = 'closed') => {
    if (reason !== 'closed') log(`canvas: ${client} ${reason}: its proxied streams end`);
    ends.get(client)?.();
    ends.delete(client);
    for (const flight of live.get(client) ?? []) kill(flight);
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
  const notFound = (res, close = false) => { res.writeHead(404, { 'content-type': 'application/json', 'cache-control': 'no-store', ...(close ? { connection: 'close' } : {}) }); res.end('{"message":"Not found."}'); };
  const json = (res, status, body, headers = {}) => { res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store', ...headers }); res.end(JSON.stringify(body)); };
  /** Answered, then the connection closed: a body not read to its end. */
  const refuseBody = (req, res, answer) => { res.on('finish', () => req.destroy?.()); res.setHeader('connection', 'close'); answer(); };

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

  async function enter(req, res, target, client) {
    // POST only, the grant only in the body: any query is refused.
    if (req.method !== 'POST' || target.query) return notFound(res);
    // An explicit Origin, the canvas's or the tool's: none, `null` or another is refused.
    const given = req.headers.origin;
    if (typeof given !== 'string' || (given !== origin && given !== toolOrigin)) return notFound(res);
    const read = await readBody(req, ENTRY_LIMIT);
    if (!read.body) { if (read.over) refuseBody(req, res, () => notFound(res)); else if (!res.destroyed) notFound(res); return; }
    const text = read.body.toString('utf8'), type = String(req.headers['content-type'] ?? '');
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
    // The oldest past the cap goes, and its streams with it.
    for (const [k] of mine.slice(0, Math.max(0, mine.length - SESSIONS_PER_CLIENT + 1))) dropSession(k);
    const sessionHash = sha(session);
    sessions.set(sessionHash.toString('hex'), { hash: sessionHash, client, token: held.token, grant: key });
    watch(client);
    res.writeHead(303, { location: '/', 'cache-control': 'no-store', 'set-cookie': `${CANVAS_COOKIE}=${session}; Secure; HttpOnly; SameSite=Strict; Path=/` });
    res.end();
  }

  /** The session a request's cookie names, with its token still good. */
  function sessionOf(req) {
    for (const value of cookies(req.headers.cookie, CANVAS_COOKIE)) {
      if (!/^[A-Za-z0-9_-]{43}$/.test(value)) continue;
      const hash = sha(value), key = hash.toString('hex');
      const s = sessions.get(key);
      if (!s || !same(s.hash, hash)) continue;
      const rec = valid(s.token);
      if (rec && rec.client === s.client) return { ...s, key, scope: rec.scope };
    }
    return null;
  }

  /** The canvas port's every request. `req.url`, the raw target, is what
   * is checked and forwarded (`url`, a parse of it, is not trusted). */
  async function handle(req, res, url) {
    const target = closed ? null : canonical(req.url ?? (url && url.pathname + url.search));
    if (!target) { notFound(res); return true; }
    const entry = ENTRY.exec(target.path);
    if (entry) {
      let client = null;
      try { client = decodeURIComponent(entry[1]); } catch { /* malformed */ }
      if (client == null) notFound(res); else await enter(req, res, target, client);
      return true;
    }
    const session = sessionOf(req);
    const need = needs(req.method, target.path);
    if (!session || !need) { notFound(res); return true; }
    if (!may(session.scope, need)) { json(res, 403, { message: `${req.method} ${target.path} needs ${need}`, scope: session.scope, needs: need }); return true; }
    if (req.method === 'POST' && req.headers.origin !== origin) { notFound(res); return true; }
    const held = live.get(session.client) ?? new Set();
    if (held.size >= LIVE_PER_CLIENT) { json(res, 503, { message: 'too many open canvas requests' }, { 'retry-after': '1' }); return true; }
    live.set(session.client, held);
    const flight = { session: session.key, gone: new AbortController(), req, res };
    held.add(flight);
    // Tracked until the response closes (after its last byte is flushed, or cut).
    res.on('close', () => {
      flight.gone.abort();
      held.delete(flight);
      if (!held.size && live.get(session.client) === held) live.delete(session.client);
    });
    await proxy(req, res, target, flight.gone.signal);
    return true;
  }

  async function proxy(req, res, target, signal) {
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
      // but never calls it local, so the loop hands this request no local
      // credential (its install token, its tamber token, a native client's
      // admission links on `/__dev/open`), whatever path the allow list
      // someday passes. The client's own forwarding headers are dropped above.
      headers.forwarded = 'for="_exact-canvas"';
      if (req.method === 'POST') headers.origin = loopOrigin;
      let body;
      if (req.method !== 'GET' && req.method !== 'HEAD') {
        const limit = target.path === '/__dev/checkpoint' ? CHECKPOINT_LIMIT : BODY_LIMIT;
        const read = await readBody(req, limit, signal);
        if (read.over) { refuseBody(req, res, () => json(res, 413, { error: { message: `a request is at most ${limit === BODY_LIMIT ? '1' : limit >> 20} MiB` } })); return; }
        if (read.gone) throw new Error('ended');
        body = read.body;
      }
      if (signal.aborted) throw new Error('ended');
      const answer = await fetch(`${loopOrigin}${target.path}${target.query}`, { method: req.method, headers, body, redirect: 'manual', signal });
      const out = {};
      const dropNamed = new Set(String(answer.headers.get('connection') ?? '').toLowerCase().split(',').map(s => s.trim()));
      for (const [k, v] of answer.headers) {
        const key = k.toLowerCase();
        if (HOP.has(key) || dropNamed.has(key) || DROP_DOWN.has(key) || key.startsWith('access-control-')) continue;
        out[key] = key === 'content-security-policy' || key === 'content-security-policy-report-only' ? rewrite.text(frameAncestors(v, loopPort, origin, toolOrigin)) : rewrite.text(v);
      }
      out['cache-control'] = 'no-store';
      const type = String(out['content-type'] ?? '');
      const stream = /^text\/event-stream\b/i.test(type);
      const isJSON = /^application\/(?:[\w.+-]+\+)?json\b/i.test(type);
      // Text the page reads (HTML, JS, CSS, plain), whole; bytes (wasm, images) streamed as they are.
      const text = isJSON || /^(?:text\/|application\/(?:x-)?javascript\b)/i.test(type);
      if (req.method === 'HEAD' || !answer.body) { res.writeHead(answer.status, out); res.end(); return; }
      if (text && !stream) {
        // Whole, so a URL split over two chunks is rewritten.
        const whole = Buffer.from(await answer.arrayBuffer()).toString('utf8');
        const bytes = Buffer.from(isJSON ? rewrite.json(whole) : rewrite.text(whole));
        if (signal.aborted) throw new Error('ended');
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
          await put(res, rewrite.sse(rest.slice(0, at + 1)), signal);
          rest = rest.slice(at + 1);
        }
        if (rest) await put(res, rewrite.sse(rest), signal);
      } else for await (const chunk of answer.body) await put(res, chunk, signal);
      res.end();
    } catch {
      // Cut part way (revoked, or either side gone): the answer begun cannot become another.
      if (res.headersSent) res.destroy();
      else if (!res.destroyed) {
        if (signal.aborted) notFound(res);
        else json(res, 502, { error: { message: 'the dev loop did not answer' } });
      }
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
