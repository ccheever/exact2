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
  const i = argv.indexOf('--relay-name'), names = i < 0 ? null : { tool: argv[i + 1] ?? '', canvas: argv[i + 2] ?? '' };
  if (names && ![names.tool, names.canvas].every(n => /^[a-z0-9.-]+$/i.test(n) && n.includes('.'))) throw new Error('--relay-name takes two names of your own domain, the tool\'s and the canvas\'s');
  let token = env.EXACT_RELAY_TOKEN ?? null;
  if (!token) {
    const file = join(home, '.exact', 'relay', account, 'token');
    if (!existsSync(file)) throw new Error(`--relay: no token for ${account} (EXACT_RELAY_TOKEN, or ${file})`);
    if (statSync(file).mode & 0o077) throw new Error(`--relay: ${file} may be read by others: chmod 600 it`);
    token = readFileSync(file, 'utf8').trim();
  }
  if (!/^[\x21-\x7e]{16,512}$/.test(token)) throw new Error('--relay: the token is not one the relay gave (16 to 512 printable characters)');
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

/** The connector: `POOL` idle connections to the relay at `host:port`,
 * each client the relay hands down given to `client(sni, address, stream)`;
 * `ready` is the names the relay serves this account's session by (`{tool,
 * canvas}`), or rejects when the relay refuses. A refusal stops it (a bad or
 * revoked token is never retried); a connection that fails is tried again,
 * waiting up to `BACKOFF_MS`. `close()` ends every connection, idle or
 * carrying a client. */
export function connectRelay({ host, port, account, token, names = null }, { client, warn = console.error, connect = connectTLS, pool = POOL, backoffMs = BACKOFF_MS } = {}) {
  const live = new Set();
  let closed = false, idle = 0, wait = 250, timer = null, served = null;
  let ready, refused;
  const named = new Promise((ok, fail) => { ready = ok; refused = fail; });
  named.catch(() => {});
  const stop = why => {
    if (closed) return;
    closed = true;
    clearTimeout(timer);
    for (const c of live) c.destroy();
    refused(new Error(why));
  };
  const again = () => {
    if (closed || timer) return;
    timer = setTimeout(() => { timer = null; fill(); }, wait);
    wait = Math.min(wait * 2, backoffMs);
  };
  const fill = () => { while (!closed && idle < pool) dial(); };
  const dial = () => {
    idle++;
    let handed = false, buf = Buffer.alloc(0), greeted = false;
    // (WebPKI, the relay's own name: a connection whose certificate fails is refused by `connect`, never sent the token)
    const conn = connect({ host, port, servername: host, minVersion: 'TLSv1.2' });
    live.add(conn);
    // An idle one gone (the relay restarted, the network moved) is dialled again after a wait, never in a tight loop.
    const down = () => {
      live.delete(conn);
      if (!handed) { idle--; again(); }
    };
    conn.once('close', down);
    conn.on('error', e => { if (!greeted) warn(`tamber: the relay ${host}:${port} can't be reached: ${e.code ?? e.message}`); });
    conn.once('secureConnect', () => {
      conn.write(`EXACT-RELAY/1 ${account}\r\nauthorization: Bearer ${token}\r\n${names ? `names: ${names.tool} ${names.canvas}\r\n` : ''}\r\n`);
    });
    const onData = chunk => {
      buf = Buffer.concat([buf, chunk]);
      for (let nl = buf.indexOf(10); nl >= 0 && !handed; nl = buf.indexOf(10)) {
        const line = buf.subarray(0, nl).toString('latin1').replace(/\r$/, '');
        buf = buf.subarray(nl + 1);
        const [word, a = '', b = ''] = line.split(' ');
        if (!greeted) {
          if (word === 'REFUSED') { warn(`tamber: the relay refused this server: ${line.slice(8).slice(0, 200)}`); return stop(`the relay refused: ${line.slice(8, 208)}`); }
          if (word !== 'READY' || !a || !b) { conn.destroy(); return; }
          greeted = true;
          wait = 250;
          if (!served) { served = { tool: a, canvas: b }; ready(served); }
          continue;
        }
        if (word !== 'CLIENT' || !a) { conn.destroy(); return; }
        // Handed down: no longer idle; another dialled in its place.
        handed = true;
        idle--;
        conn.off('data', onData);
        conn.pause();
        fill();
        client(a.toLowerCase(), b, streamOf(conn, buf));
        return;
      }
      if (buf.length > LINE) conn.destroy();
    };
    conn.on('data', onData);
  };
  fill();
  return { ready: named, close: () => stop('closed'), get idle() { return idle; } };
}
