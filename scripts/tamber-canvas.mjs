// The canvas gateway (LLP 1121 stage 10 C, LLP 1122 D12): a paired client's
// view of the app's dev URL, served by the Tamber server under its paired
// token on the canvas's own origin (B's second TLS port), never the loop's
// loopback URL handed out.
//
// - `mint(token)`, the `canvas` method's body (`read`): a single-use grant
//   (256 bits, 30 s, bound to the client) and the entry URL,
//   `<origin>/canvas/<client>/`. Rate-limited per client with A's `limiter`.
// - The entry: the tool frames `/canvas/<client>/#g=<grant>` (theme2's
//   approval of the fragment entry: the single-use 30 s grant, never the
//   token, in a fragment no server sees). GET serves a static page whose one
//   script (hashed in its CSP) takes the fragment out of its address and
//   history first, then POSTs `{grant}` as JSON, same-origin: the grant
//   compared timing-safe and burned, a session id (256 bits) bound here to
//   the client, its token and that grant, set as `__Host-exact-canvas`
//   (Secure, HttpOnly, SameSite=Strict, Path=/), 204; the page goes to `/`.
//   A grant in a query or a path is never read. The cookie only names a session: a value this map didn't
//   mint (page code can set a cookie of that name; cookies aren't
//   port-scoped) is an unknown session, and a token is never read from it.
// - Every other request: the session, then its client's token through A's
//   `lookup` (a revoked or expired token fails at once), then the route's
//   scope through `allows`. An unknown, missing, revoked or expired session
//   is not-found, the same answer for each; a scope refused is 403 naming
//   both scopes. Allowed POSTs also need `Origin` to be the canvas's own;
//   so does the entry's POST (the tool's origin no longer posts it).
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
import { openRemote } from './tamber-remote.mjs';
import { refuseBody, upstream, viaDoor } from './tamber-request.mjs';

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
const parses = (authority, scheme) => { try { new URL(`${scheme}//${authority}/`); return true; } catch { return false; } };
/** An authority's port when WHATWG URL parses its host as loopback, else
 * null. Nothing is decoded first: `127.0.0.1%2F@evil.test` is evil.test's. */
function loopbackPort(authority, scheme = 'http:') {
  let u;
  try { u = new URL(`${scheme}//${authority}/`); } catch { return null; }
  if (!isLoopback(u.hostname)) return null;
  return u.port || (scheme === 'https:' || scheme === 'wss:' ? '443' : '80');
}

// Candidates, each decided by WHATWG URL: a web URL or a scheme-relative
// one (its authority up to the first `/`, `?`, `#`, `\`, whitespace, quote,
// `<` or `>`, so userinfo punctuation such as `;@` stays in it: theme2), and a bare `host:port` standing alone
// (not in a path, not inside a longer name). Escaped spellings (HTML or
// JSON escapes in text, percent-encoded URLs) are a known limit (§10).
const ADDRESSES = new RegExp(
  String.raw`(?<![\w+.\-:\/\\%])(?<scheme>(?:https?|wss?):)?\/\/(?<auth>[^\s"'<>\x60\/?#\\]+)`
  + String.raw`|(?<![\w.\-\/\\%@:\[])(?<bare>\[[0-9a-f:.]+\]|[a-z0-9](?:[a-z0-9.\-]*[a-z0-9])?\.?):(?<port>\d{1,5})(?![\w\-]|\.\w)`, 'gi');

/** The rewriting of loopback addresses for a canvas client: the loop's own
 * (any loopback host at its port) as `origin`; any other loopback address
 * as PLACEHOLDER; no other host ever touched. `text` for headers, HTML and
 * other text; `json` parses a JSON body and rewrites each string in it (its
 * bytes unchanged when nothing is); `sse` a stream's lines, a `data:` line
 * that is JSON as JSON. */
export function rewriter(loop, origin) {
  const from = new URL(loop), to = new URL(origin);
  const port = from.port || (from.protocol === 'https:' ? '443' : '80');
  // One pass, left to right, so nothing written is read again; a match cut
  // short (below) resumes the scan where the cut was.
  const text = t => {
    let out = '', at = 0;
    const re = new RegExp(ADDRESSES.source, ADDRESSES.flags);
    for (let m; (m = re.exec(t));) {
      const g = m.groups;
      let swap = m[0], end = m.index + m[0].length;
      if (g.auth !== undefined) {
        const scheme = (g.scheme ?? '').toLowerCase();
        // The whole authority as WHATWG reads it; only one that doesn't
        // parse at all (a CSP directive's `;img-src` after it, a list's
        // `,`, a sentence's `)`) is read up to its first such mark, and the
        // scan goes on from there. A userinfo `;@` parses whole: never cut.
        let auth = g.auth;
        if (!parses(auth, scheme || 'http:')) {
          // The longest prefix that parses, cut at a `;`, `,`, `)` or `}`
          // past any userinfo (after the last `@`): `;@` stays, and so does
          // a host such as `localhost;evil.test` that WHATWG reads whole.
          const host = auth.lastIndexOf('@') + 1;
          let cut = host;
          for (let i = auth.length - 1; i >= host; i--) if (';,)}'.includes(auth[i]) && parses(auth.slice(0, i), scheme || 'http:')) { cut = i; break; }
          auth = auth.slice(0, cut);
          end = m.index + m[0].length - (g.auth.length - auth.length);
          swap = m[0].slice(0, end - m.index);
        }
        const p = loopbackPort(auth, scheme || 'http:');
        if (p != null) swap = p !== port ? `${scheme}//${PLACEHOLDER}` : `${!scheme ? '' : scheme.startsWith('ws') ? 'wss:' : to.protocol}//${to.host}`;
      } else {
        const p = loopbackPort(`${g.bare}:${g.port}`);
        if (p != null) swap = p === port ? to.host : PLACEHOLDER;
      }
      out += t.slice(at, m.index) + swap;
      at = end; re.lastIndex = Math.max(end, m.index + 1);
    }
    return out + t.slice(at);
  };
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

/** A CSP's `frame-ancestors` through the canvas: `'self'` (the canvas's
 * own origin, which can already do anything a frame of its own could, and
 * apps frame their own routes) and the tool's remote origin, nothing else:
 * the loop's own origin is 'self' here, and any other source (the local
 * tool's, a foreign one a page names) goes, so no other origin frames it. */
function frameAncestors(policy, loopPort, origin, toolOrigin) {
  const loopbackSource = s => {
    const bare = s.replace(/^[a-z][a-z0-9+.-]*:\/\//i, '');
    const scheme = /^(https?|wss?):\/\//i.exec(s)?.[1];
    return /^\w|^\[/.test(bare) ? loopbackPort(bare.replace(/\/.*$/, ''), scheme ? `${scheme.toLowerCase()}:` : undefined) : null;
  };
  return policy.split(',').map(one => one.split(';').map(d => {
    const words = d.trim().split(/\s+/);
    if (words[0]?.toLowerCase() !== 'frame-ancestors') return d;
    // (Read after the addresses are rewritten: another loopback source is the placeholder by then, and goes too.)
    const kept = words.slice(1).flatMap(s => s.toLowerCase() === "'self'" ? ["'self'"] : s === origin || loopbackSource(s) === loopPort ? [origin] : []);
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
export const readBody = (req, limit, signal) => new Promise(done => {
  const chunks = [];
  let size = 0, settled = false;
  const finish = v => {
    if (settled) return;
    settled = true;
    req.off('data', data); req.off('end', end); req.off('error', gone); req.off('close', gone);
    req.socket?.off?.('close', gone);
    signal?.removeEventListener('abort', gone);
    done(v);
  };
  const data = chunk => { size += chunk.length; if (size > limit) { req.pause(); finish({ over: true }); } else chunks.push(chunk); };
  const end = () => finish({ body: Buffer.concat(chunks) });
  const gone = () => finish({ gone: true });
  if (signal?.aborted || req.destroyed) return gone();
  req.on('data', data); req.on('end', end); req.on('error', gone); req.on('close', gone);
  // (its connection's close too: under Bun a request on the canvas port's TLS hand-off never fires its own)
  req.socket?.once?.('close', gone);
  signal?.addEventListener('abort', gone, { once: true });
});

/** `write` with backpressure: resolved once `res` drains or closes, or
 * `signal` aborts (a client that never reads ends with its revoke). */
const drained = (res, signal) => new Promise(ok => {
  if (signal.aborted || res.destroyed || res.writableEnded) return ok(); // (already over: nothing more will fire)
  const done = () => { res.off('drain', done); res.off('close', done); signal.removeEventListener('abort', done); ok(); };
  res.on('drain', done); res.on('close', done); signal.addEventListener('abort', done, { once: true });
});
const put = async (res, chunk, signal) => {
  if (signal.aborted) throw new Error('ended');
  // Behind the TLS door Bun's response drains before its socket does: past
  // 8 MiB unread on the socket, wait for it to come down to 4 MiB (a reader
  // that is reading gets there), and only a socket stuck 30 s is cut.
  const queued = () => res.socket?.writableLength || 0;
  if (queued() > 8 << 20) {
    // (30 s without any progress cuts it; every byte that leaves starts the 30 s again)
    for (let low = queued(), until = Date.now() + 30_000; queued() > 4 << 20; await new Promise(ok => setTimeout(ok, 50))) {
      if (signal.aborted || res.destroyed) throw new Error('ended');
      if (queued() < low) { low = queued(); until = Date.now() + 30_000; }
      if (Date.now() > until) { res.destroy(); throw new Error('ended'); }
    }
  }
  if (signal.aborted || res.destroyed || res.writableEnded) throw new Error('ended');
  if (!res.write(chunk)) await drained(res, signal);
  if (signal.aborted) throw new Error('ended');
};

/** The entry page's one script (theme2's approval of dtool2's fragment
 * entry): the grant from `#g=…`, the fragment taken out of the address and
 * its history entry first (`replaceState`, before any request), then POSTed
 * same-origin as JSON (its own referrer policy, so the POST carries its
 * Origin under the page's `no-referrer`), then tells the remote tool it
 * entered (`exact-canvas-entered`, to the tool's origin alone, which the page
 * carries as `data-tool`; nothing when there is none; no secret, no client),
 * then `/`. Refused: one short message, and no further request. */
export const ENTRY_SCRIPT = `(async()=>{const g=new URLSearchParams(location.hash.slice(1)).get('g');history.replaceState(null,'',location.pathname);const no=()=>{document.getElementById('m').textContent='This canvas link has expired; ask for a new one.'};if(!g)return no();let r=null;try{r=await fetch(location.pathname,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({grant:g}),credentials:'same-origin',cache:'no-store',redirect:'error',referrerPolicy:'same-origin'})}catch{}if(r&&r.ok){const t=document.documentElement.dataset.tool;if(t)parent.postMessage({type:'exact-canvas-entered'},t);location.replace('/')}else no()})()`;
export const ENTRY_SCRIPT_HASH = `sha256-${createHash('sha256').update(ENTRY_SCRIPT).digest('base64')}`;
/** The entry page, one script whatever the gateway (its hash fixed); the remote tool's origin, an
 * origin and no secret, in `data-tool` for its one message. */
const attr = v => String(v).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const entryHtml = tool => `<!doctype html><html lang="en"${tool ? ` data-tool="${attr(tool)}"` : ''}><meta charset="utf-8"><title>Canvas</title><p id="m" role="status"></p><script>${ENTRY_SCRIPT}</script></html>`;

/** The gateway for the dev loop at `loop` (its loopback origin), served on
 * `origin` (the canvas's public origin), framed by `toolOrigin`. Stage 10
 * A's interface (scripts/tamber-pairing.mjs): `pairing` is the run's, as
 * `tamberServer(...).pairing` returns it. Each request acts as `lookup`'s
 * client for the session's token, and `onEnd` (at once for a client already
 * ended) ends its streams. `limiter` and `allows` are that module's.
 * `log` hears each client's end and its reason (`'revoked'` or
 * `'expired'`), never a secret. A server makes it with
 * `tamberServer({canvas: pairing => canvasGateway({pairing})})` (its
 * default) and answers `canvas` with its `mint`. `loop`, `origin` and
 * `toolOrigin` are given at construction or once by `serve(...)`, when the
 * canvas port is bound (`openCanvas`); until then it mints nothing and
 * answers nothing. */
export function canvasGateway({ loop = null, pairing, origin = null, toolOrigin = null, now = Date.now, limiter = pairingLimiter, allows = pairingAllows, log = () => {}, mints: shared = null }) {
  if (typeof limiter !== 'function' || typeof allows !== 'function') throw new TypeError('canvasGateway takes A\'s `limiter` and `allows`');
  // One bucket per client, for grant minting: another gateway's (`mints`,
  // stage 10c R1b's relay canvas) when given, so a client's cap is one
  // across every door.
  const mints = shared ?? limiter({ ...MINT_RATE, clock: now });
  // Every canvas request is remote: never the local token's `local`.
  const may = (scope, need) => allows(scope, need, false);
  let loopOrigin, loopHost, loopPort, rewrite, served = false;
  /** Where it proxies from and is served on, once. */
  function serve(at) {
    if (served) throw new Error('canvasGateway: served already');
    const u = new URL(at.loop);
    const tool = at.toolOrigin ? new URL(at.toolOrigin).origin : null;
    // The remote tool's origin, B's TLS door: never a plain one (the local
    // tool's http://127.0.0.1:7740, or any other service on this machine).
    if (tool && new URL(tool).protocol !== 'https:') throw new TypeError('canvasGateway: `toolOrigin` is the remote tool\'s https origin (B\'s door), never a plain or local one');
    // A host name or address only (B's door names one): nothing an HTML attribute or a header could misread.
    if (tool && !/^https:\/\/(?:[a-z0-9.-]+|\[[0-9a-f:.]+\])(?::\d+)?$/i.test(tool)) throw new TypeError('canvasGateway: `toolOrigin` is a plain https origin (a host name or address and a port)');
    [loopOrigin, loopHost, loopPort] = [u.origin, u.host, u.port || (u.protocol === 'https:' ? '443' : '80')];
    origin = new URL(at.origin).origin;
    toolOrigin = tool;
    rewrite = rewriter(loopOrigin, origin);
    served = true;
  }
  if (loop && origin) serve({ loop, origin, toolOrigin });
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

  function mint(token) {
    if (closed) return { status: 503, error: { message: 'the canvas gateway is closed' } };
    if (!served) return { status: 404, error: { message: 'this server has no canvas port (--remote)' } };
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
    // The entry page (GET), which carries no secret; the grant only in a POST's body: any query is refused.
    if (target.query) return notFound(res);
    if (req.method === 'GET' || req.method === 'HEAD') { entryPage(req, res); return; }
    if (req.method !== 'POST') return notFound(res);
    // The entry page's own POST: an explicit Origin, the canvas's; none, `null` or another (the tool's too) is refused.
    if (req.headers.origin !== origin) return notFound(res);
    const read = await readBody(req, ENTRY_LIMIT);
    if (!read.body) { if (read.over) refuseBody(req, res, () => notFound(res)); else if (!res.destroyed) notFound(res); return; }
    const text = read.body.toString('utf8'), type = String(req.headers['content-type'] ?? '');
    let grant = null;
    try {
      grant = /^application\/json\b/i.test(type) ? JSON.parse(text)?.grant : null;
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
    // The page goes on to `/` itself (`location.replace`).
    res.writeHead(204, { 'cache-control': 'no-store', 'set-cookie': `${CANVAS_COOKIE}=${session}; Secure; HttpOnly; SameSite=Strict; Path=/` });
    res.end();
  }

  /** `GET /canvas/<client>/`, the entry page the tool frames with the grant
   * in its fragment (`#g=<grant>`, never sent to a server): its one inline
   * script takes the grant out of its address before any request, posts it
   * here, and goes on to `/`. Static, and framed by the tool alone. */
  function entryPage(req, res) {
    res.writeHead(200, {
      'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store', 'referrer-policy': 'no-referrer', 'x-content-type-options': 'nosniff', 'cross-origin-resource-policy': 'same-origin',
      'content-security-policy': `default-src 'none'; script-src '${ENTRY_SCRIPT_HASH}'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors ${toolOrigin ?? "'none'"}`,
    });
    res.end(req.method === 'HEAD' ? undefined : entryHtml(toolOrigin));
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
   * is checked and forwarded (`url`, a parse of it, is not trusted);
   * `access` says which door it came through (B's `addPort` gives `remote`). */
  async function handle(req, res, url, access) {
    // Only through the canvas's TLS door (B's `addPort`: `remote`), or from this machine: never plain HTTP from a LAN peer.
    const door = viaDoor(access);
    const target = closed || !served || !door ? null : canonical(req.url ?? (url && url.pathname + url.search));
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
    // A page of another origin including this one's resources (a stylesheet,
    // an image, a script), its cookie sent along because the two are
    // same-site (another relay account's canvas, stage 10c R1b's round 2):
    // refused, as Fetch metadata says it; only this origin's own requests
    // and navigations are answered. A request with no Fetch metadata (an
    // older browser, a driver) is handled as stage 10 handled it, the
    // answer's CORP covering the browsers.
    const site = req.headers['sec-fetch-site'];
    if (site !== undefined && site !== 'same-origin' && site !== 'none' && req.headers['sec-fetch-mode'] !== 'navigate') { notFound(res); return true; }
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
      const answer = await upstream(`${loopOrigin}${target.path}${target.query}`, { method: req.method, headers, body, redirect: 'manual', signal });
      const out = {};
      const dropNamed = new Set(String(answer.headers.get('connection') ?? '').toLowerCase().split(',').map(s => s.trim()));
      for (const [k, v] of answer.headers) {
        const key = k.toLowerCase();
        if (HOP.has(key) || dropNamed.has(key) || DROP_DOWN.has(key) || key.startsWith('access-control-')) continue;
        out[key] = key === 'content-security-policy' || key === 'content-security-policy-report-only' ? frameAncestors(rewrite.text(v), loopPort, origin, toolOrigin) : rewrite.text(v);
      }
      out['cache-control'] = 'no-store';
      // (and for a browser without Fetch metadata: no other origin may include it;
      // this governs no-cors subresources, never the tool's frame, and the tool
      // sends no COEP: should it ever, a framed canvas would need its own policy)
      out['cross-origin-resource-policy'] = 'same-origin';
      // Framed by the tool's origin and its own only, whatever the loop sent
      // (an SVG, an image or any document the loop answers without the
      // HTML's header is a navigation another same-site origin could frame).
      const csp = out['content-security-policy'];
      const ancestors = `frame-ancestors 'self'${toolOrigin ? ` ${toolOrigin}` : ''}`;
      if (!csp) out['content-security-policy'] = ancestors;
      else if (!/(^|[;,])\s*frame-ancestors\b/i.test(csp)) out['content-security-policy'] = `${csp}; ${ancestors}`;
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

  return { mint, handle, serve, close, mints, get origin() { return served ? origin : null; }, get live() { return [...live.values()].reduce((n, s) => n + s.size, 0); } };
}

/** The canvas port on B's remote doors (`openRemote`), for `tamber`'s
 * gateway: another TLS listener on `host`, the same certificate (B's
 * Origin check: its own only), and the gateway served at `https://<the shown name>:<its
 * port>`, proxying `loop`. The port is bound first and its handler defers to
 * the gateway, which answers nothing until `serve` (no request can come
 * before the origin is known). Null without the TLS listener or a gateway. */
export async function openCanvas(doors, tamber, { loop, host, port = 0, say = console.log }) {
  const gateway = tamber?.canvas, info = doors?.info?.();
  if (!gateway || !info) return null;
  const toolOrigin = info.origin;
  const bound = await doors.addPort({ host, port, handler: (req, res, url, access) => gateway.handle(req, res, url, access) });
  const origin = `https://${new URL(toolOrigin).hostname}:${bound}`;
  gateway.serve({ loop, origin, toolOrigin });
  say(`  canvas (TLS only, the same certificate): ${origin}, a paired client's view of this loop, entered by a grant from \`canvas\``);
  return origin;
}

/** The canvas through a hosted relay (stage 10c R1b, theme2's 22:06): a
 * gateway of its own, this same code, for the relay session's own pair
 * (`https://<canvas name>` framed by `https://<tool name>`), made once the
 * relay names the run's one session; `canvas` through the relay's door mints
 * from it, and the pairing it shares ends a client on both. Closed with the
 * doors, its grants and sessions with it. */
export function openRelayCanvas(doors, tamber, { loop, say = console.log }) {
  if (!doors?.onRelay || !tamber?.relayCanvas || !tamber.pairing) return;
  let gateway = null;
  const shut = doors.close;
  doors.close = () => { gateway?.close(); return shut(); };
  doors.onRelay(({ tool, canvas }) => {
    // (the TLS door's mint limiter, shared: one cap a client, whichever door)
    gateway = canvasGateway({ pairing: tamber.pairing, ...(tamber.canvas?.mints ? { mints: tamber.canvas.mints } : {}) });
    gateway.serve({ loop, origin: `https://${canvas}`, toolOrigin: `https://${tool}` });
    doors.relayCanvas((req, res, url, access) => gateway.handle(req, res, url, access));
    tamber.relayCanvas(gateway);
    say(`  canvas through the relay: https://${canvas}, framed by https://${tool} only`);
  });
}

/** A dev loop's remote doors (`openRemote`) and, with the TLS listener, its
 * canvas port, proxying `loop` (the loop's own loopback origin); through a
 * relay, the relay's canvas too. A canvas port that cannot bind closes the
 * doors, and the error is thrown. */
export async function openRemoteCanvas(tamber, asked, loop, options) {
  const doors = await openRemote(tamber, asked, options);
  // (the relay's first, before the TLS canvas's port is awaited: the relay may name its session meanwhile)
  openRelayCanvas(doors, tamber, { loop, ...options });
  try { await openCanvas(doors, tamber, { loop, host: asked.remote?.host, ...options }); } catch (e) { await doors?.close(); throw e; }
  return doors;
}
