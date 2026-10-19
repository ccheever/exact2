// A hosted relay's connector (LLP 1121 §5 stage 10c R1; rt1's plan,
// ~/x2theme/rt1-s10c-plan.md, theme2's 20:45 approval). Behind a NAT the
// design server can't be reached, so it dials out: it holds a few idle TLS
// connections to the relay's control endpoint, and the relay hands each a
// client's TCP stream, unopened. The client's TLS ends here, in the same
// listener `--remote` uses (scripts/tamber-remote.mjs), so the relay never
// sees a token, a design or a canvas; it reads the ClientHello's SNI only.
//
// The connector's own protocol, outside tamber/1 (`EXACT-RELAY/1`), on a
// connection whose relay certificate verified by WebPKI (no flag turns that
// off: theme2's condition 4):
//   connector: `EXACT-RELAY/1 <account>\r\nauthorization: Bearer <token>\r\n`
//              (`names: <tool> <canvas>\r\n` with names of its own), `\r\n`
//   relay:     `READY <tool> <canvas>\n`, then idle; or `REFUSED <why>\n`
//   relay:     `CLIENT <sni> <address>\n` and the client's bytes, both ways
// The token is never in a URL, never logged, never sent to a client; the
// address is the client's as the relay reports it, shown, never trusted.

import { existsSync, readFileSync, statSync } from 'node:fs';
import { isIP } from 'node:net';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { Duplex } from 'node:stream';
import { connect as connectTLS } from 'node:tls';

/** Idle connections held for clients to come down, and the longest wait
 * between tries when the relay can't be reached. */
export const POOL = 4, BACKOFF_MS = 30_000;
const LINE = 1024; // (a header line longer than this is no relay's)

/** `--relay host[:port]` and `--relay-account <label>`, with the
 * certificate for the relay's names (`--relay-cert`, `--relay-key`, PEM
 * files: the developer's own until R2 gets one), and `--relay-name <tool>
 * <canvas>`'s names of the developer's own domain; the account's token from
 * `EXACT_RELAY_TOKEN`, else `~/.exact/relay/<account>/token`, which only its
 * owner may read. Null without `--relay`. */
export function relayArgs(argv, { env = process.env, home = homedir() } = {}) {
  const value = flag => { const i = argv.indexOf(flag); return i < 0 ? null : argv[i + 1] ?? ''; };
  const r = value('--relay');
  if (r === null) return null;
  const m = /^([a-z0-9.-]+)(?::(\d{1,5}))?$/i.exec(r);
  if (!m || Number(m[2] ?? 443) > 65535) throw new Error(`--relay takes the relay's host[:port] (relay.example.com), not ${JSON.stringify(r)}`);
  const account = value('--relay-account');
  if (!account || !/^[a-z0-9][a-z0-9-]{0,62}$/.test(account)) throw new Error('--relay needs --relay-account <label>, the account the relay gave you (lowercase letters, digits and -)');
  const cert = value('--relay-cert'), key = value('--relay-key');
  if (!cert || !key) throw new Error('--relay needs --relay-cert and --relay-key, PEM files for the relay\'s names (until the certificate is got for you)');
  const i = argv.indexOf('--relay-name'), names = i < 0 ? null : { tool: (argv[i + 1] ?? '').toLowerCase(), canvas: (argv[i + 2] ?? '').toLowerCase() };
  if (names && ![names.tool, names.canvas].every(n => /^[a-z0-9.-]+$/i.test(n) && n.includes('.'))) throw new Error('--relay-name takes two names of your own domain, the tool\'s and the canvas\'s');
  let token = env.EXACT_RELAY_TOKEN ?? null;
  if (!token) {
    const file = join(home, '.exact', 'relay', account, 'token');
    if (!existsSync(file)) throw new Error(`--relay: no token for ${account} (EXACT_RELAY_TOKEN, or ${file})`);
    if (statSync(file).mode & 0o077) throw new Error(`--relay: ${file} may be read by others: chmod 600 it`);
    token = readFileSync(file, 'utf8').trim();
  }
  if (!/^[\x21-\x7e]{16,512}$/.test(token)) throw new Error('--relay: the token is not one the relay gave (16 to 512 printable characters)');
  // (the key only its owner may read, as the token)
  if (statSync(key).mode & 0o077) throw new Error(`--relay-key ${key} may be read by others: chmod 600 it`);
  return { host: m[1].toLowerCase(), port: Number(m[2] ?? 443), account, token, names, cert: readFileSync(cert, 'utf8'), key: readFileSync(key, 'utf8') };
}

/** A relayed client's stream as a Duplex of its own: the bytes after the
 * header first, then the relay connection's, each way, read as the TLS
 * listener reads them (paused while it is behind). */
function streamOf(conn, rest) {
  const d = new Duplex({
    read() { conn.resume(); },
    write(chunk, _e, done) { conn.write(chunk, done); },
    final(done) { conn.end(); done(); },
    destroy(err, done) { conn.destroy(); done(err); },
  });
  if (rest.length) d.push(rest);
  conn.on('data', c => { if (!d.push(c)) conn.pause(); });
  conn.on('end', () => d.push(null));
  conn.on('close', () => d.destroy());
  conn.on('error', () => {});
  return d;
}

/** A host name as the relay may name a session's (lowercase, dotted, at
 * most 253 characters), and an address as it may report one. */
const NAME = /^(?=.{4,253}$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/;
/** Dials a second at most, and a burst: whatever a relay does, the token
 * goes out no faster (a dial a client is far below it). */
const DIALS = { rate: 20, burst: 40 };
/** Streams handed down at once, at most (as the TLS door's 64 connections). */
export const STREAMS = 64;
/** A relay's words, shown: printable, at most 120 characters, the token never. */
// (the token's letters and digits anywhere in it, however cased or broken up: the whole word withheld)
const bare = text => text.toLowerCase().replace(/[^a-z0-9]/g, '');
const carries = (text, token) => bare(token).length >= 8 && bare(text).includes(bare(token));
const shown = (text, token) => (carries(text, token) ? '[withheld: it carries the token]' : text.replace(/[^\x20-\x7e]/g, '?').slice(0, 120));

/** The connector: `POOL` idle connections to the relay at `host:port`,
 * each client the relay hands down given to `client(sni, address, stream,
 * settled)`, which answers whether it takes it and later calls
 * `settled(true)` once the client's own TLS has named the session (or
 * `settled(false)` when it failed first); `ready` is the names the relay
 * serves this account's session by (`{tool, canvas}`; `named(names)` is told
 * them first, at once), or rejects when the relay refuses. A refusal, at any
 * time, stops it (a bad or revoked token is never retried) and cuts every
 * stream. Only a client whose TLS settled lets the next connection be
 * dialled at once: a connection that fails, a client not taken or one that
 * never settles is followed by a wait doubling to `BACKOFF_MS`, so no relay
 * can make it dial in a loop. At most `STREAMS` clients are carried at once
 * (one more is not taken), and the idle connections are dialled again as
 * any closes. Each connection has `greetMs` to verify the relay and be
 * greeted, and as long for any line it starts. `close()` ends every
 * connection, idle or carrying a client. */
export function connectRelay({ host, port, account, token, names = null }, { client, named: told = () => true, ended = () => {}, warn = console.error, connect = connectTLS, pool = POOL, backoffMs = BACKOFF_MS, greetMs = 10_000, clock = () => performance.now() } = {}) {
  const live = new Set();
  let closed = false, idle = 0, carried = 0, wait = 250, timer = null, budget = null, served = null;
  // (a relay's word that carries the token is never shown, logged or an origin)
  const clean = word => !carries(word, token);
  // The dial budget, on a monotonic clock (a wall clock set back stalls nothing).
  let tokens = DIALS.burst, at = clock();
  const mayDial = () => { const now = clock(); tokens = Math.min(DIALS.burst, tokens + Math.max(0, now - at) * DIALS.rate / 1000); at = now; if (tokens < 1) return false; tokens -= 1; return true; };
  let ready, refused;
  const named = new Promise((ok, fail) => { ready = ok; refused = fail; });
  named.catch(() => {});
  const stop = why => {
    if (closed) return;
    closed = true;
    clearTimeout(timer);
    clearTimeout(budget);
    for (const c of live) c.destroy();
    refused(new Error(why));
    ended(why);
  };
  const again = () => {
    if (closed || timer) return;
    timer = setTimeout(() => { timer = null; fill(); }, wait);
    wait = Math.min(wait * 2, backoffMs);
  };
  // (dialled only while the pool is short, no failure's wait is running and
  // the dial budget has one; the budget's wait is its own, so a failure
  // still doubles the backoff behind it)
  const fill = () => {
    while (!closed && !timer && !budget && idle < pool) {
      if (!mayDial()) { budget = setTimeout(() => { budget = null; fill(); }, Math.ceil(1000 / DIALS.rate)); return; }
      dial();
    }
  };
  const dial = () => {
    idle++;
    let handed = false, buf = Buffer.alloc(0), greeted = false, once = false;
    // A carried client's TLS settled or failed, once: settled, the next is
    // dialled at once; failed (or closed unsettled), after the wait.
    const settled = ok => { if (once || closed) return; once = true; if (ok) { wait = 250; fill(); } else again(); };
    // WebPKI, the relay's own name, whatever the environment says
    // (NODE_TLS_REJECT_UNAUTHORIZED included): the token goes only to a
    // relay whose certificate verified.
    const conn = connect({ host, port, servername: host, minVersion: 'TLSv1.2', rejectUnauthorized: true });
    live.add(conn);
    // Until greeted, and from a line's first byte until its end: `greetMs`
    // (never put off by the line's later bytes).
    let late = setTimeout(() => conn.destroy(), greetMs);
    const due = () => { if (!buf.length) { clearTimeout(late); late = null; } else if (!late) late = setTimeout(() => conn.destroy(), greetMs); };
    conn.once('close', () => {
      clearTimeout(late);
      live.delete(conn);
      // An idle one gone (the relay restarted, the network moved): after a
      // wait. A carried one gone: its place in the pool's count is free.
      if (!handed) { idle--; again(); } else { carried--; settled(false); fill(); }
    });
    conn.on('error', e => { if (!greeted) warn(`tamber: the relay ${host}:${port} can't be reached: ${e.code ?? 'error'}`); });
    conn.once('secureConnect', () => {
      if (closed) return conn.destroy();
      if (!conn.authorized) { warn(`tamber: the relay ${host}:${port}'s certificate did not verify: ${conn.authorizationError ?? 'unknown'}`); return conn.destroy(); }
      conn.write(`EXACT-RELAY/1 ${account}\r\nauthorization: Bearer ${token}\r\n${names ? `names: ${names.tool} ${names.canvas}\r\n` : ''}\r\n`);
    });
    const onData = chunk => {
      buf = Buffer.concat([buf, chunk]);
      for (let nl = buf.indexOf(10); !handed; nl = buf.indexOf(10)) {
        // (a line past its length, ended or not, is no relay's)
        if ((nl < 0 ? buf.length : nl) > LINE) return conn.destroy();
        if (nl < 0) return greeted && due();
        const line = buf.subarray(0, nl).toString('latin1').replace(/\r$/, '');
        buf = buf.subarray(nl + 1);
        const [word, a = '', b = ''] = line.split(' ');
        if (word === 'REFUSED') { const why = shown(line.slice(8), token); warn(`tamber: the relay refused this server: ${why}`); return stop(`the relay refused: ${why}`); }
        if (!greeted) {
          // Its names: well formed, and the developer's own when given.
          if (word !== 'READY' || !NAME.test(a) || !NAME.test(b) || a === b || !clean(a) || !clean(b) || (names && (a !== names.tool || b !== names.canvas))) return conn.destroy();
          greeted = true;
          clearTimeout(late);
          late = null;
          due();
          // (names the server can't serve, its certificate not covering them: stopped, never shown)
          if (!served) { served = { tool: a, canvas: b }; if (!told(served)) { warn(`tamber: the relay names a session (${shown(a, token)}) this server's certificate for it does not cover`); return stop('the relay\'s names are not the certificate\'s'); } ready(served); }
          continue;
        }
        if (word !== 'CLIENT' || !NAME.test(a) || !clean(a)) return conn.destroy();
        // Handed down: no longer idle, and no deadline of the connector's
        // (the TLS listener's own run from here). The next connection is
        // dialled at once only when this one's TLS settles; otherwise after
        // the wait.
        handed = true;
        idle--;
        carried++;
        clearTimeout(late);
        conn.off('data', onData);
        conn.pause();
        if (carried > STREAMS || !client(a, isIP(b) && clean(b) ? b : 'unknown', streamOf(conn, buf), settled)) { conn.destroy(); settled(false); }
        return;
      }
    };
    conn.on('data', onData);
  };
  fill();
  return { ready: named, close: () => stop('closed'), get closed() { return closed; }, get idle() { return idle; }, get live() { return live.size; }, get carried() { return carried; } };
}
