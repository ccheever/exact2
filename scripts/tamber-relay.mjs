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
//              `run: <id>\r\n` (one random id a run, on every connection: the
//              relay's one session for it, R3), (`names: <tool> <canvas>\r\n`
//              with names of its own, or once the relay has named the run's),
//              `\r\n`
//   relay:     `READY <tool> <canvas>\n`, then idle; or `REFUSED <why>\n`
//   relay:     `CLIENT <sni> <address>\n` and the client's bytes, both ways
// The token is never in a URL, never logged, never sent to a client; the
// address is the client's as the relay reports it, shown, never trusted.

import { randomBytes } from 'node:crypto';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { isIP } from 'node:net';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import { Duplex } from 'node:stream';
import { connect as connectTLS } from 'node:tls';

/** Idle connections held for clients to come down, and the longest wait
 * between tries when the relay can't be reached. */
export const POOL = 4, BACKOFF_MS = 30_000;
export const LINE = 1024; // (a header line longer than this is no relay's)

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
  if (!account || !/^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(account)) throw new Error('--relay needs --relay-account <label>, the account the relay gave you (lowercase letters, digits and -)');
  // The certificate: the developer's own (`--relay-cert`/`--relay-key`), or
  // got here by ACME (R2): through the relay for its names, or with names of
  // the developer's own domain, through their DNS by `--relay-dns-hook`, a
  // command run here (their credentials never pass through the relay).
  const cert = value('--relay-cert'), key = value('--relay-key');
  if (!cert !== !key) throw new Error('--relay-cert and --relay-key go together: PEM files for the relay\'s names');
  const i = argv.indexOf('--relay-name'), names = i < 0 ? null : { tool: (argv[i + 1] ?? '').toLowerCase(), canvas: (argv[i + 2] ?? '').toLowerCase() };
  if (names && (![names.tool, names.canvas].every(n => NAME.test(n)) || names.tool === names.canvas)) throw new Error('--relay-name takes two names of your own domain, the tool\'s and the canvas\'s');
  const hook = value('--relay-dns-hook');
  if (names && !cert && !hook) throw new Error('--relay-name needs --relay-dns-hook (a command that puts the ACME TXT record up in your DNS), or --relay-cert and --relay-key');
  if (hook !== null && (!names || cert)) throw new Error('--relay-dns-hook is for --relay-name\'s names of your own, got by ACME');
  const requireCaa = argv.includes('--require-caa');
  if (requireCaa && hook === null) throw new Error('--require-caa is for --relay-dns-hook\'s names of your own');
  if (names && parentOf(names.tool) !== parentOf(names.canvas)) throw new Error('--relay-name\'s two names share one parent domain (one wildcard certifies both)');
  const acme = value('--relay-acme') ?? ACME;
  if (!/^https:\/\//.test(acme)) throw new Error('--relay-acme takes an ACME directory\'s https URL');
  let token = env.EXACT_RELAY_TOKEN ?? null;
  if (!token) {
    const file = join(home, '.exact', 'relay', account, 'token');
    if (!existsSync(file)) throw new Error(`--relay: no token for ${account} (EXACT_RELAY_TOKEN, or ${file})`);
    if (statSync(file).mode & 0o077) throw new Error(`--relay: ${file} may be read by others: chmod 600 it`);
    token = readFileSync(file, 'utf8').trim();
  }
  if (!/^[\x21-\x7e]{16,512}$/.test(token)) throw new Error('--relay: the token is not one the relay gave (16 to 512 printable characters)');
  // (the key only its owner may read, as the token)
  if (key && statSync(key).mode & 0o077) throw new Error(`--relay-key ${key} may be read by others: chmod 600 it`);
  return { host: m[1].toLowerCase(), port: Number(m[2] ?? 443), account, token, names,
    ...(cert ? { cert: readFileSync(cert, 'utf8'), key: readFileSync(key, 'utf8') } : { acme, hook, requireCaa, dir: join(home, '.exact', 'relay', account) }) };
}

/** Let's Encrypt's directory, the certificate's issuer by default. */
export const ACME = 'https://acme-v02.api.letsencrypt.org/directory';

/** A host name's parent domain (`s1-tool.acct.relay.test` → `acct.relay.test`). */
export const parentOf = name => name.slice(name.indexOf('.') + 1);

/** The ACME DNS-01 record `record` = `value` put up (or taken down,
 * `clear`) by the relay, over a connection of its own: the connector's
 * handshake with a `txt:` line, the relay answering `TXT-OK` or `REFUSED
 * <why>` (only names under the connector's own account, R3's rule). The
 * relay's certificate verified by WebPKI before the token goes, as a
 * pooled connection's. */
export function relayTxt({ host, port, account, token }, record, value, { clear = false, connect = connectTLS, ms = 30_000, signal = null } = {}) {
  // (a record and a value of ACME's own shape: nothing a server gave can add a line)
  if (!RECORD.test(record) || !/^[A-Za-z0-9_-]{43}$/.test(value)) return Promise.reject(new Error('no ACME TXT record of that shape'));
  return new Promise((ok, fail) => {
    const conn = connect({ host, port, servername: host, minVersion: 'TLSv1.2', rejectUnauthorized: true });
    let buf = '';
    const done = setTimeout(() => { conn.destroy(); fail(new Error('the relay did not answer the TXT record in time')); }, ms);
    // (stopped with the run: the connection gone, whatever it had sent)
    const stop = () => { clearTimeout(done); conn.destroy(); fail(new Error('stopped')); };
    if (signal?.aborted) stop(); else signal?.addEventListener('abort', stop, { once: true });
    // (closed with no answer, cut or refused unsaid: failed then, not at the deadline)
    conn.once('close', () => { signal?.removeEventListener('abort', stop); clearTimeout(done); fail(new Error('the relay closed the connection without answering the TXT record')); });
    conn.once('secureConnect', () => {
      if (!conn.authorized) { clearTimeout(done); conn.destroy(); return fail(new Error(`the relay's certificate did not verify: ${conn.authorizationError ?? 'unknown'}`)); }
      conn.write(`EXACT-RELAY/1 ${account}\r\nauthorization: Bearer ${token}\r\n${clear ? 'txt-clear' : 'txt'}: ${record} ${value}\r\n\r\n`);
    });
    // One line answers, at most LINE bytes: then the connection is gone, nothing more read.
    const read = d => {
      buf += d.toString('latin1');
      const nl = buf.indexOf('\n');
      if (nl < 0 && buf.length <= LINE) return;
      clearTimeout(done);
      conn.off('data', read);
      conn.destroy();
      if (nl < 0 || nl > LINE) return fail(new Error('the relay answered the TXT record with no line'));
      const line = buf.slice(0, nl).replace(/\r$/, '');
      if (line === 'TXT-OK') ok();
      else fail(new Error(`the relay refused the TXT record: ${shown(line.replace(/^REFUSED ?/, ''), token)}`));
    };
    conn.on('data', read);
    conn.on('error', e => { clearTimeout(done); fail(new Error(`the relay ${host}:${port} can't be reached: ${e.code ?? 'error'}`)); });
  });
}

/** The ACME DNS-01 record put up (or taken down) by the developer's own
 * command (`--relay-dns-hook`), run here by `sh -c` with
 * `EXACT_ACME_ACTION` (`publish`|`clear`), `EXACT_ACME_RECORD` and
 * `EXACT_ACME_VALUE`; its output is never logged (it may name their DNS
 * account), only its exit. */
export function dnsHook(command, record, value, { clear = false, ms = 300_000, signal = null } = {}) {
  if (!RECORD.test(record) || !/^[A-Za-z0-9_-]{43}$/.test(value)) return Promise.reject(new Error('no ACME TXT record of that shape'));
  return new Promise((ok, fail) => {
    // (never the relay's token in its environment; its own process group, so a timeout stops all it started)
    const { EXACT_RELAY_TOKEN: _token, ...env } = process.env;
    const child = spawn('sh', ['-c', command], { stdio: 'ignore', detached: true, env: { ...env, EXACT_ACME_ACTION: clear ? 'clear' : 'publish', EXACT_ACME_RECORD: record, EXACT_ACME_VALUE: value } });
    const kill = () => { try { process.kill(-child.pid, 'SIGKILL'); } catch { child.kill('SIGKILL'); } };
    // (killed, by its time or with the run: settled only once it has exited, so a clear after never races it)
    let why = null;
    const done = setTimeout(() => { why = `--relay-dns-hook took more than ${ms / 1000} s`; kill(); }, ms);
    const stop = () => { clearTimeout(done); why = 'stopped'; kill(); };
    if (signal?.aborted) stop(); else signal?.addEventListener('abort', stop, { once: true });
    const settle = () => { clearTimeout(done); signal?.removeEventListener('abort', stop); };
    child.on('exit', code => { settle(); if (why) fail(new Error(why)); else code === 0 ? ok() : fail(new Error(`--relay-dns-hook exited ${code} for ${clear ? 'clearing' : 'publishing'} ${record}`)); });
    child.on('error', e => { settle(); fail(new Error(`--relay-dns-hook: ${e.code ?? e.message}`)); });
  });
}

/** An ACME DNS-01 record's name. */
export const RECORD = /^_acme-challenge\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/;

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
export const NAME = /^(?=.{4,253}$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/;
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
  // (this run's one id, on every connection: the relay gives all of them one session, R3)
  const run = randomBytes(16).toString('hex');
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
      // (once served, its names asked for again on every connection: a relay that restarted, or let the session
      // lapse, gives this run the same names back, or refuses; never others the run would have to drop)
      const asked = names ?? served;
      conn.write(`EXACT-RELAY/1 ${account}\r\nauthorization: Bearer ${token}\r\nrun: ${run}\r\n${asked ? `names: ${asked.tool} ${asked.canvas}\r\n` : ''}\r\n`);
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
          // (one session a run: a connection greeted with other names is not this run's, its origins and grants another's)
          if (served && (a !== served.tool || b !== served.canvas)) return conn.destroy();
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
