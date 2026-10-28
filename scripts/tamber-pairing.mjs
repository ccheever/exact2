// Scopes on the design server's wire (LLP 1121 D13 "Authentication",
// LLP 1122 D6 and D12, stage 10 A; tamber/1.13).
//
// Every request names a method, and every method has one scope here: the
// dispatcher (scripts/tamber-server.mjs) checks it before any handler runs,
// and a method with no entry is refused (fail closed). A new method, or a
// gateway's (stage 10 C's `canvas`), adds its entry here.
//
// - `read`: what reads the design, its history and the running builds'
//   reports, and the change stream.
// - `propose`: what only offers a change for someone else to accept (a held
//   proposal: `propose` with `hold`, `import`, `revert`; `drop`, `edit-held` and `choose` of one's own).
// - `write`: what writes the design, or changes what the developer's screens
//   show (`pin`, `preview`, `outline`, `hold`: the lead's ruling 5), or drives
//   the running builds (a scrub).
// - `local`: the local token only, never a paired client's, whatever its
//   scope (stage 10 A2's `clients`, `revoke`, `pair-code`).
//
// `inspect` is `read` (ruling 5): a client below `write` may read the
// selection each build last reported (`refresh`); turning inspect mode on or
// off, which takes the developer's builds' input, is `write`, as is asking
// the builds again (their answer outlines on the developer's screens).

import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';

/** The scopes a token may have, lowest first. */
export const SCOPES = ['read', 'propose', 'write'];

/** Each method's scope; the change stream's two paths by their names. */
export const METHOD_SCOPE = Object.freeze({
  hello: 'read',
  describe: 'read',
  resolve: 'read',
  explain: 'read',
  uses: 'read',
  history: 'read',
  export: 'read',
  inspect: 'read',
  subscribe: 'read',
  events: 'read',
  propose: 'propose',
  import: 'propose',
  revert: 'propose',
  drop: 'propose',
  'edit-held': 'propose',
  choose: 'propose',
  transaction: 'write',
  undo: 'write',
  commit: 'write',
  accept: 'write',
  pin: 'write',
  navigate: 'write',
  preview: 'write',
  outline: 'write',
  hold: 'write',
  scrub: 'write',
  'scrub-release': 'write',
  'scrub-cancel': 'write',
  clients: 'local',
  revoke: 'local',
  'pair-code': 'local',
  // Stage 10 C's gateway grant (scripts/tamber-canvas.mjs): seeing the canvas is reading.
  canvas: 'read',
});

/** Whether a client of `scope` may do what `needs`: `local` needs the local
 * token (`local` true), whatever its scope. */
export function allows(scope, needs, local = false) {
  if (needs === 'local') return local;
  const have = SCOPES.indexOf(scope), want = SCOPES.indexOf(needs);
  return have >= 0 && want >= 0 && have >= want;
}

/** What `request` needs: its method's entry (an own property only: no
 * inherited name is a method), `drop`, `edit-held` or `choose` of another client's held proposal
 * `write` (theme2's ruling (a)), and `inspect` other than `refresh` `write`
 * (ruling 5). `owner(id)` is a held proposal's client. Null for a method
 * with no entry. */
export function needs(request, owner = () => null) {
  if (!Object.hasOwn(METHOD_SCOPE, String(request.method))) return null;
  const base = METHOD_SCOPE[request.method];
  if (request.method === 'inspect' && request.refresh !== true) return 'write';
  if (request.method === 'drop' || request.method === 'edit-held' || request.method === 'choose') {
    const by = owner(request.proposal);
    return by !== null && by !== request.client ? 'write' : 'propose';
  }
  return base;
}

/** A refusal for a client of `scope` asking what `need`s. */
export function refusal(method, scope, need) {
  const why = need === 'local' ? 'the local tool\'s alone' : `\`${need}\` scope`;
  return { status: 403, error: { message: `${method} needs ${why}; this client has \`${scope}\``, scope, needs: need } };
}

// Pairing (stage 10 A2; LLP 1121 D13, the lead's rulings 2–4 and 7).
//
// A local client mints a code (`pair-code {scope, name?}`, or
// `bun exact.mjs tamber pair`); a remote client presents it once at
// `POST /__tamber/pair` and gets its own token at that scope. Codes are 50
// CSPRNG bits (10 Crockford base32 characters), valid two minutes, single
// use, at most 8 live. A failed attempt cannot be told from a guess at any
// one code, so it counts against every live code, and a code with 5 is
// burned. Attempts are limited per peer (5 a minute). Tokens are 32 CSPRNG
// bytes, in memory only (a restart revokes them all), kept by their SHA-256
// so no lookup compares a secret byte by byte, and live 12 hours unless
// `--pair-ttl` says otherwise. No token or code is ever logged, put in an
// event or read from a cookie: a token travels only in the
// `x-exact-tamber-token` header. A paired client's name is `remote:<name>#n`,
// a name no local client may take, so neither can act as the other.
//
// The interface stage 10 C (the canvas gateway) and B (the transport) build
// on, from `pairing(...)`:
// - `lookup(token) → {client, scope, expires} | null`: a paired token's
//   client; null for an unknown, revoked or expired one (past its deadline,
//   whether or not its end has run yet). Look it up on every
//   request and stream, and act as that `client`, never one a request names.
//   It refreshes `lastSeen` (listed by `clients`), not the wire's presence,
//   which owns pins, scrubs, inspect, outlines and holds.
// - `onEnd(client, fn) → unsubscribe`: `fn(reason)` once, when the client is
//   revoked (`'revoked'`) or its token expires (`'expired'`); at once, in the
//   same call, for a client that has already ended or is past its deadline
//   (with the reason, for the newest 1024 ended). `live(client)` and
//   `expire()` are there for the same reason: a deadline is a fact at once,
//   its end comes from the timer or the next `expire`. The wire ends the
//   client's subscriptions, long polls, scrub and pins this way; the gateway
//   ends its proxied streams. A request that authenticated before an end and
//   is still reading its body is refused when it has (401); one already
//   dispatched, a write sent to the design server, completes as a local
//   edit in flight would.
// - `limiter({rate, per})`, exported below: the per-key token bucket the
//   pair endpoint uses, for the gateway's grant minting.
// - `allows(scope, needs)` and METHOD_SCOPE (above): the gateway's `canvas`
//   entry goes in the table, and its routes check scope as the dispatcher
//   does.
// - From B, per request, the wire reads `access.remote` (the request came
//   on the remote listener), `access.tunnel` (on loopback through a tunnel
//   the developer trusted, `--trust-tunnel`) and `access.peer`, the rate
//   limit's key: the connection's own address (never a header's), canonical
//   (an IPv4-mapped IPv6 address as its IPv4). A remote or tunnelled request
//   is never local (no token page, no tool), reaches only `POST
//   /__tamber/pair` without a paired token, and the local token is refused
//   there (ruling 3).


const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
export const CODE_MS = 2 * 60_000, CODE_TRIES = 5, CODES_LIVE = 8;
export const PAIR_TTL_MS = 12 * 3600_000;

/** `--pair-ttl`'s value: a number and `s`, `m`, `h` or `d` (`12h`, `90m`),
 * as ms; null for anything else, or under a minute, or over 30 days. */
export function parseDuration(text) {
  const m = /^(\d+(?:\.\d+)?)([smhd])$/.exec(String(text ?? '').trim());
  if (!m) return null;
  const ms = Number(m[1]) * { s: 1e3, m: 6e4, h: 36e5, d: 864e5 }[m[2]];
  return ms >= 6e4 && ms <= 30 * 864e5 ? Math.round(ms) : null;
}

/** `--pair-ttl <duration>` from a command's arguments, as ms (12 h unless
 * said); a bad one is said and the process exits 2. */
export function pairTtlOf(argv, exit = code => process.exit(code)) {
  const i = argv.indexOf('--pair-ttl');
  if (i < 0) return PAIR_TTL_MS;
  const ms = parseDuration(argv[i + 1]);
  if (ms == null) { console.error(`--pair-ttl ${argv[i + 1]}: a duration from 1m to 30d, as 90m, 12h or 2d`); return exit(2); }
  return ms;
}

/** A token bucket per key: a burst of `rate`, refilled at `rate` per `per`
 * ms. `take(key)` is true while the key has a token left; `wait(key)` the ms
 * until it has one. A bucket refilled whole is forgotten, swept as keys
 * come, and at most `keys` are kept (the stalest go first), so a stream of
 * new peers costs bounded memory. `first(key)` is true once per emptying,
 * for logging a limit hit once rather than on every refused try. */
export function limiter({ rate = 5, per = 60_000, clock = Date.now, keys = 4096 } = {}) {
  const buckets = new Map();
  const refill = (b, now) => { b.tokens = Math.min(rate, b.tokens + (now - b.at) * rate / per); b.at = now; };
  const sweep = now => {
    for (const [k, b] of buckets) { refill(b, now); if (b.tokens >= rate) buckets.delete(k); }
    while (buckets.size > keys) buckets.delete(buckets.keys().next().value);
  };
  let swept = clock();
  const level = key => {
    const now = clock();
    if (now - swept > per || buckets.size > keys) { sweep(now); swept = now; }
    const b = buckets.get(key) ?? { tokens: rate, at: now, told: false };
    refill(b, now);
    buckets.delete(key); buckets.set(key, b);
    return b;
  };
  return {
    take(key) {
      const b = level(key);
      if (b.tokens < 1) return false;
      b.tokens -= 1; b.told = false;
      if (b.tokens >= rate) buckets.delete(key);
      return true;
    },
    wait(key) { const b = level(key); return b.tokens >= 1 ? 0 : Math.ceil((1 - b.tokens) * per / rate); },
    first(key) { const b = level(key); if (b.told) return false; b.told = true; return true; },
  };
}

const sha = t => createHash('sha256').update(t).digest('hex');
const code50 = () => {
  // 50 bits: 56 random bits, the low 6 shifted off.
  const b = randomBytes(7); let n = 0n;
  for (const x of b) n = (n << 8n) | BigInt(x);
  n >>= 6n;
  let s = '';
  for (let i = 0; i < 10; i++) { s = CROCKFORD[Number(n & 31n)] + s; n >>= 5n; }
  return `${s.slice(0, 5)}-${s.slice(5)}`;
};
/** A code as typed: Crockford's reading (case, `I`/`L` as 1, `O` as 0),
 * hyphens and spaces dropped. */
const normal = c => String(c ?? '').toUpperCase().replace(/[\s-]/g, '').replace(/[IL]/g, '1').replace(/O/g, '0');

/** The run's pairing: codes, paired tokens, their ends. `say(what)` is told
 * of each burn and rate-limit hit (no code in it), for the log and the
 * tool. */
export function pairing({ ttlMs = PAIR_TTL_MS, clock = Date.now, say = () => {} } = {}) {
  const codes = [], tokens = new Map(), clients = new Map(), ends = new Map(), names = new Map(), gone = new Map();
  const attempts = limiter({ rate: 5, per: 60_000, clock });
  // A relay's session is one peer for every client it hands down (stage
  // 10c, theme2's 6): 10 a minute across them, its own bucket, so a flood
  // through the relay never limits the direct doors or the local tool.
  const relayed = limiter({ rate: 10, per: 60_000, clock });
  const live = () => { const now = clock(); for (let i = codes.length - 1; i >= 0; i--) if (codes[i].expires <= now || codes[i].burned) codes.splice(i, 1); };
  const end = (client, reason) => {
    const c = clients.get(client);
    if (!c) return false;
    clients.delete(client); tokens.delete(c.hash); clearTimeout(c.timer);
    // Why it ended, for an `onEnd` that comes after (the newest 1024 kept).
    gone.set(client, reason);
    if (gone.size > 1024) gone.delete(gone.keys().next().value);
    const fns = ends.get(client) ?? new Set(); ends.delete(client);
    for (const fn of fns) { try { fn(reason); } catch {} }
    return true;
  };
  return {
    /** A code for a local client to show: `{code, scope, expires}`. */
    mint({ scope, name } = {}) {
      if (!SCOPES.includes(scope)) return { status: 400, error: { message: `\`scope\` is one of ${SCOPES.join(', ')}` } };
      if (name !== undefined && (typeof name !== 'string' || !/^[\w .-]{1,40}$/.test(name))) return { status: 400, error: { message: '`name` is up to 40 letters, digits, spaces, dots, dashes or underscores' } };
      live();
      if (codes.length >= CODES_LIVE) { codes.shift(); say({ what: 'replaced', why: `at most ${CODES_LIVE} codes are live: the oldest is burned` }); }
      const code = code50(), expires = clock() + CODE_MS;
      codes.push({ code: normal(code), scope, name: name ?? null, expires, fails: 0, burned: false });
      return { ok: { code, scope, expires: new Date(expires).toISOString() } };
    },
    /** `POST /__tamber/pair {code, name?}` from `peer`: the token, once. */
    /** One attempt from `peer` taken, or the 429 for it: the endpoint asks
     * before it reads a body. */
    admit(peer) {
      const relay = String(peer).startsWith('relay:'), bucket = relay ? relayed : attempts;
      if (bucket.take(peer)) return null;
      const retry = Math.ceil(bucket.wait(peer) / 1000);
      if (bucket.first(peer)) say({ what: 'limited', peer, ...(relay ? { relay: true, retry } : {}) });
      return { status: 429, retry, error: { message: relay ? `pairing is limited through the relay: try again in ${retry} s` : 'too many pairing attempts from here: wait and try again' } };
    },
    pair({ code, name } = {}, peer = 'unknown', { admitted = false } = {}) {
      if (!admitted) { const limited = this.admit(peer); if (limited) return limited; }
      if (typeof code !== 'string' || code.length > 32) return { status: 400, error: { message: '`code` is the pairing code, a string' } };
      if (name !== undefined && (typeof name !== 'string' || name.length > 40)) return { status: 400, error: { message: '`name` is a string of up to 40 characters' } };
      live();
      const given = Buffer.from(normal(code));
      // Every live code is compared, each in constant time, whichever matches.
      let hit = null;
      for (const c of codes) { const mine = Buffer.from(c.code); if (given.length === mine.length && timingSafeEqual(given, mine) && !hit) hit = c; }
      if (!hit) {
        for (const c of codes) if (++c.fails >= CODE_TRIES) { c.burned = true; say({ what: 'burned', peer, why: `${CODE_TRIES} failed attempts, the last from this peer` }); }
        live();
        return { status: 403, error: { message: 'no such pairing code: it may have expired, been used, or been burned by failed attempts' } };
      }
      hit.burned = true; live();
      const base = (typeof name === 'string' && /^[\w .-]{1,40}$/.test(name) ? name : hit.name) ?? 'remote';
      const n = (names.get(base) ?? 0) + 1; names.set(base, n);
      const client = `remote:${base}#${n}`, token = randomBytes(32).toString('hex'), hash = sha(token);
      const now = clock(), expires = now + ttlMs;
      const entry = { client, name: base, scope: hit.scope, peer, created: now, expires, lastSeen: now, hash };
      // Expiry ends the client as revoke does, at its time and at any lookup
      // after; a lifetime past a timer's reach (24.8 days) is armed again.
      const arm = () => {
        const left = entry.expires - clock();
        if (left <= 0) return end(client, 'expired');
        entry.timer = setTimeout(arm, Math.min(left, 2 ** 31 - 1));
        entry.timer.unref?.();
      };
      arm();
      tokens.set(hash, client); clients.set(client, entry);
      return { ok: { token, client, scope: hit.scope, expires: new Date(expires).toISOString() } };
    },
    lookup(token) {
      if (typeof token !== 'string' || !token) return null;
      const client = tokens.get(sha(token));
      const c = client && clients.get(client);
      // Past its deadline it is no one, now; its end (and the callbacks) is
      // the timer's or `expire`'s, never a read's.
      if (!c || c.expires <= clock()) return null;
      c.lastSeen = clock();
      return { client, scope: c.scope, expires: c.expires };
    },
    onEnd(client, fn) {
      const c = clients.get(client);
      if (!c || c.expires <= clock()) { fn(c ? 'expired' : gone.get(client) ?? 'revoked'); return () => {}; }
      const set = ends.get(client) ?? new Set(); set.add(fn); ends.set(client, set);
      return () => set.delete(fn);
    },
    revoke(client) { return end(client, 'revoked'); },
    /** The paired clients, never a token. */
    list() {
      const iso = t => new Date(t).toISOString();
      return [...clients.values()].map(c => ({ client: c.client, name: c.name, scope: c.scope, peer: c.peer, created: iso(c.created), expires: iso(c.expires), lastSeen: iso(c.lastSeen) }));
    },
    /** Whether `client` is a paired client of this run, its token not
     * revoked (its expiry not checked: `live`). */
    has(client) { return clients.has(client); },
    /** Whether `client` may still act: paired, and before its deadline.
     * Reads nothing else and ends nothing: a deadline its timer hasn't
     * reached yet is ended by `expire`. */
    live(client) {
      const c = clients.get(client);
      return !!c && c.expires > clock();
    },
    /** Ends every client past its deadline (each callback once): the wire
     * calls it as each request comes and on its presence sweep, so no read
     * waits on a timer and no end happens inside another event's send. */
    expire() { const now = clock(); for (const [client, c] of [...clients]) if (c.expires <= now) end(client, 'expired'); },
    close() { for (const client of [...clients.keys()]) end(client, 'revoked'); },
  };
}

/** `bun exact.mjs tamber pair <url> --scope read|propose|write [--name n]`
 * (stage 10 A2, the lead's ruling 7): a pairing code from the local server,
 * which a remote client presents once at `POST /__tamber/pair`. It takes the
 * local token as `op` does, so it works from this machine only. */
export async function pairCode(argv) {
  const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const base = argv.find((a, i) => !a.startsWith('-') && !['--scope', '--name'].includes(argv[i - 1]));
  if (!base || !at('--scope')) { console.error('usage: bun exact.mjs tamber pair <server url> --scope read|propose|write [--name <name>]'); return 2; }
  const origin = new URL(base).origin;
  const got = await fetch(`${origin}/__tamber/token`, { redirect: 'error' });
  if (!got.ok) { console.error(`tamber pair: ${origin} gave no token (${got.status}); it answers this machine's loopback only`); return 1; }
  const { token } = await got.json();
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', redirect: 'error', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: JSON.stringify({ method: 'pair-code', scope: at('--scope'), ...(at('--name') ? { name: at('--name') } : {}) }) });
  const reply = await res.json();
  if (!reply.ok) { console.error(`tamber pair: ${reply.error?.message ?? res.status}`); return 1; }
  console.log(`${reply.ok.code}  (${reply.ok.scope}; until ${reply.ok.expires}, once)`);
  // Where the other machine pairs, and the certificate it checks (the lead's ruling 2).
  const doors = reply.ok.remote ?? [];
  for (const door of doors) console.log(door.tunnel
    ? `  ${doors.some(d => !d.tunnel) ? 'or ' : ''}through the trusted tunnel to ${door.origin}: open /__tamber/tool/ at the other machine's end of it`
    : `  on the other machine open ${door.origin}/__tamber/tool/ and check its certificate's SHA-256 is ${door.fingerprint} before entering the code`);
  return 0;
}
