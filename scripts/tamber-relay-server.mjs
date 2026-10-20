// The hosted relay (LLP 1121 §5 stage 10c R3; rt1's plan,
// ~/x2theme/rt1-s10c-r3-plan.md, theme2's 14:34 approval): a design server
// behind a NAT dials out to it (the connector, scripts/tamber-relay.mjs), and
// it hands each client's TCP stream down unopened. One port, the ClientHello's
// name deciding:
//
// - **The relay's own name** (`domain`): TLS ends here, on the relay's only
//   certificate. This is the control endpoint, where connectors greet
//   (EXACT-RELAY/1) and ask for ACME TXT records.
// - **A live session's name** (`<id>-tool.<label>.<domain>`, its canvas twin,
//   or names under a parent of the account's own): the bytes go down an idle
//   connector connection of that session as they came, after one line,
//   `CLIENT <name> <address>`, both ways. The relay holds no key for them and
//   reads nothing past the first record (the ClientHello's).
// - **Anything else**: closed, nothing said in clear.
//
// Accounts are a file of SHA-256 token hashes (`accounts.json`): reloaded on
// change, a revoked or changed account cut at once, every stream it carries
// with it; a label is never given out again (its `revoked` list and the
// append-only `labels.seen` beside it). A connector's TXT records are only its
// own account's (`_acme-challenge.<label>.<domain>`). Run as
// `bun scripts/tamber-relay-server.mjs --config relay.json`; `--help` names
// every key.

import { createHash, createPrivateKey, randomBytes, timingSafeEqual, X509Certificate } from 'node:crypto';
import { appendFileSync, existsSync, readFileSync, watch } from 'node:fs';
import { createServer as createTCP } from 'node:net';
import { dirname, join, resolve } from 'node:path';
import { createServer as createTLS } from 'node:tls';
import { dnsHook, LINE, NAME, RECORD } from './tamber-relay.mjs';

/** An account's label, as the connector checks one. */
export const LABEL = /^[a-z0-9][a-z0-9-]{0,62}$/;
/** A TLS record's body, at most (RFC 8446 §5.1). */
const RECORD_MAX = 16_384;
/** What a session's limits are when the config says none. */
export const LIMITS = { perIpPerSession: 30, streamsPerSession: 64, sessionsPerAccount: 4, txtPerAccount: 10, greetingsPerIp: 10, waitMs: 2000, graceMs: 60_000, helloMs: 10_000 };
const MINUTE = 60_000;

/** The server name a client's first TLS record asks for. `buf` is what has
 * come so far: `{state: 'more'}` until the first record is whole, then
 * `{state: 'ok', sni, read}` (`sni` lower case, or null when the hello has
 * none) or `{state: 'bad'}`: not a handshake record, a record past 16 KiB,
 * not a ClientHello, a ClientHello longer than its record (fragmented over
 * records: a stated limit, which no mainstream browser sends), or one whose
 * fields run past their bounds. `read` is the bytes looked at: never more
 * than that first record. */
export function helloName(buf) {
  if (buf.length < 5) return { state: 'more' };
  if (buf[0] !== 0x16 || buf[1] !== 0x03) return { state: 'bad' };
  const length = buf.readUInt16BE(3);
  if (length > RECORD_MAX || length < 4) return { state: 'bad' };
  if (buf.length < 5 + length) return { state: 'more' };
  const rec = buf.subarray(5, 5 + length), read = 5 + length;
  if (rec[0] !== 0x01) return { state: 'bad' };
  const hello = (rec[1] << 16) | (rec[2] << 8) | rec[3];
  if (hello + 4 > rec.length) return { state: 'bad' };
  const body = rec.subarray(4, 4 + hello);
  let at = 2 + 32;
  const skip = n => { at += n; return at <= body.length; };
  if (!skip(0) || at + 1 > body.length) return { state: 'bad' };
  if (!skip(1 + body[at])) return { state: 'bad' };
  if (at + 2 > body.length || !skip(2 + body.readUInt16BE(at))) return { state: 'bad' };
  if (at + 1 > body.length || !skip(1 + body[at])) return { state: 'bad' };
  if (at === body.length) return { state: 'ok', sni: null, read };
  if (at + 2 > body.length) return { state: 'bad' };
  const end = at + 2 + body.readUInt16BE(at);
  if (end !== body.length) return { state: 'bad' };
  at += 2;
  let sni = null, seen = false;
  while (at < end) {
    if (at + 4 > end) return { state: 'bad' };
    const type = body.readUInt16BE(at), size = body.readUInt16BE(at + 2);
    const data = body.subarray(at + 4, at + 4 + size);
    if (at + 4 + size > end) return { state: 'bad' };
    at += 4 + size;
    if (type !== 0) continue;
    // (one server_name extension, its one host_name entry: RFC 6066 §3)
    if (seen) return { state: 'bad' };
    seen = true;
    if (data.length < 5 || data.readUInt16BE(0) !== data.length - 2 || data[2] !== 0) return { state: 'bad' };
    const n = data.readUInt16BE(3);
    if (5 + n !== data.length) return { state: 'bad' };
    sni = data.subarray(5).toString('latin1').toLowerCase();
    if (!NAME.test(sni)) return { state: 'bad' };
  }
  return { state: 'ok', sni, read };
}

const sha256 = text => createHash('sha256').update(text).digest();

/** The accounts at `path`: `{accounts: {<label>: {token_sha256, names?}},
 * revoked: [<label>]}`, with `labels.seen` beside it, appended with each
 * label it holds. Refused (thrown) when a label is malformed, revoked and
 * an account too, or seen before and in neither (a label is never given
 * out again); when a token hash is not 64 hex digits; or when a parent of
 * the account's own (`names`, for `--relay-name`) is not a name, is under
 * `domain`, or is another account's or under or over one. */
export function loadAccounts(path, domain) {
  const file = JSON.parse(readFileSync(path, 'utf8'));
  const accounts = new Map(), revoked = new Set(file.revoked ?? []);
  const seenPath = join(dirname(path), 'labels.seen');
  const seen = new Set(existsSync(seenPath) ? readFileSync(seenPath, 'utf8').split('\n').filter(Boolean) : []);
  for (const label of revoked) if (!LABEL.test(label)) throw new Error(`revoked: ${JSON.stringify(label).slice(0, 70)} is no label`);
  const parents = [];
  for (const [label, a] of Object.entries(file.accounts ?? {})) {
    if (!LABEL.test(label)) throw new Error(`${JSON.stringify(label).slice(0, 70)} is no label (lowercase letters, digits and -)`);
    if (revoked.has(label)) throw new Error(`${label} is revoked: a label is never given out again`);
    if (!/^[0-9a-f]{64}$/.test(a?.token_sha256 ?? '')) throw new Error(`${label}: token_sha256 is not 64 hex digits`);
    const names = a.names ?? [];
    for (const p of names) {
      if (!NAME.test(p)) throw new Error(`${label}: ${JSON.stringify(p).slice(0, 70)} is no name`);
      if (p === domain || p.endsWith(`.${domain}`)) throw new Error(`${label}: ${p} is under the relay's own domain`);
      const clash = parents.find(q => q.label !== label && (q.p === p || q.p.endsWith(`.${p}`) || p.endsWith(`.${q.p}`)));
      if (clash) throw new Error(`${p} (${label}) and ${clash.p} (${clash.label}): one account's names may not be, or be under or over, another's`);
      parents.push({ p, label });
    }
    accounts.set(label, { hash: Buffer.from(a.token_sha256, 'hex'), names });
  }
  for (const label of seen) if (!accounts.has(label) && !revoked.has(label)) throw new Error(`${label} was an account and is neither one nor revoked: a label is never given out again`);
  const fresh = [...accounts.keys(), ...revoked].filter(l => !seen.has(l));
  if (fresh.length) appendFileSync(seenPath, fresh.map(l => `${l}\n`).join(''), { mode: 0o600 });
  return { accounts, revoked };
}

/** Whether `token` is `account`'s (in constant time, by its hash). */
const holds = (account, token) => !!account && timingSafeEqual(sha256(token), account.hash);

/** A count of events in the last minute, by key. */
function perMinute(limit, now) {
  const seen = new Map();
  return {
    take(key) {
      const t = now(), list = (seen.get(key) ?? []).filter(x => t - x < MINUTE);
      if (list.length >= limit) { seen.set(key, list); return false; }
      list.push(t);
      seen.set(key, list);
      return true;
    },
    peek(key) { const t = now(); return (seen.get(key) ?? []).filter(x => t - x < MINUTE).length; },
  };
}

/** An address as a log shows it: an IPv4's first three parts, an IPv6's first four groups. */
const masked = ip => (/^\d+\.\d+\.\d+\.\d+$/.test(ip) ? ip.replace(/\.\d+$/, '.x') : String(ip).split(':').slice(0, 4).join(':') + '::');

/** The relay: `config` as the file says it (`domain`, `listen`, `tls`
 * `{cert, key}` (PEM text), `accounts` (a path), `dns`, `limits`). `dns`
 * puts a TXT record up or takes one value down: `{command}` (run as the
 * connector's `--relay-dns-hook` is) or, in process, `{publish(record,
 * value), clear(record, value)}`. `log(line)` gets one line a session
 * start, end and refusal, never a token or a client's bytes. Throws at
 * once when its certificate doesn't cover `domain`, isn't valid now, or
 * isn't its key's. `{listen(), close(), reload(), port, sessions}`. */
export function relayServer(config, { log = line => console.log(line), now = () => Date.now() } = {}) {
  const domain = String(config.domain ?? '').toLowerCase();
  if (!NAME.test(domain)) throw new Error('domain: the relay\'s own name (lowercase, dotted)');
  const limits = { ...LIMITS, ...(config.limits ?? {}) };
  // Its own certificate, for its own name only, valid now and its key's: else no start (theme2's 14:34 (5)).
  const own = new X509Certificate(config.tls?.cert ?? '');
  if (!own.checkHost(domain, { partialWildcards: false })) throw new Error(`tls.cert does not name ${domain}`);
  if (Date.parse(own.validFrom) > now() || Date.parse(own.validTo) <= now()) throw new Error(`tls.cert is not valid now (${own.validFrom} to ${own.validTo})`);
  if (!own.checkPrivateKey(createPrivateKey(config.tls.key))) throw new Error('tls.key is not tls.cert\'s key');
  const control = createTLS({ cert: config.tls.cert, key: config.tls.key, minVersion: 'TLSv1.2' }, conn => greet(conn));
  control.on('tlsClientError', (_e, s) => s?.destroy());
  const accountsPath = resolve(config.accounts);
  let { accounts, revoked } = loadAccounts(accountsPath, domain);
  const dns = config.dns?.command ? { publish: (r, v) => dnsHook(config.dns.command, r, v), clear: (r, v) => dnsHook(config.dns.command, r, v, { clear: true }) } : config.dns;
  /** Each live session, by its key (`<label> <run>`) and by each of its names. */
  const sessions = new Map(), byName = new Map();
  /** Each label's connector connections (idle or carrying), cut together on revocation. */
  const conns = new Map();
  const clientsPerIp = perMinute(limits.perIpPerSession, now), txts = perMinute(limits.txtPerAccount, now), refusals = perMinute(limits.greetingsPerIp, now);
  const tcp = createTCP({ pauseOnConnect: false }, socket => route(socket));
  const live = new Set(), quietOf = new WeakMap();

  /** A client's first record read (never more), then down to its session, to the control endpoint, or closed. */
  function route(socket) {
    live.add(socket);
    socket.once('close', () => live.delete(socket));
    socket.on('error', () => socket.destroy());
    let buf = Buffer.alloc(0);
    const late = setTimeout(() => socket.destroy(), limits.helloMs);
    const onData = d => {
      buf = Buffer.concat([buf, d]);
      const h = helloName(buf);
      if (h.state === 'more') { if (buf.length > 5 + RECORD_MAX) socket.destroy(); return; }
      clearTimeout(late);
      socket.off('data', onData);
      socket.pause();
      if (h.state !== 'ok' || !h.sni) return socket.destroy();
      if (h.sni === domain) {
        // (the control endpoint: its TLS ends here; the bytes read so far given back first)
        socket.unshift(buf);
        control.emit('connection', socket);
        return;
      }
      const session = byName.get(h.sni);
      if (!session) return socket.destroy();
      hand(session, h.sni, socket, buf);
    };
    socket.on('data', onData);
  }

  /** `socket` (its first bytes `first`) down an idle connection of `session`, or closed. */
  function hand(session, name, socket, first) {
    const ip = socket.remoteAddress ?? 'unknown';
    if (!clientsPerIp.take(`${session.key} ${ip}`)) { log(`relay: ${session.label} ${session.id}: ${masked(ip)} over ${limits.perIpPerSession} a minute`); return socket.destroy(); }
    if (session.carried.size >= limits.streamsPerSession) return socket.destroy();
    const give = conn => {
      conn.off('data', quietOf.get(conn));
      session.carried.add(conn);
      conn.once('close', () => { session.carried.delete(conn); socket.destroy(); ended(session); });
      socket.once('close', () => conn.destroy());
      conn.write(`CLIENT ${name} ${ip.replace(/^::ffff:/, '')}\n`);
      conn.write(first);
      socket.pipe(conn);
      conn.pipe(socket);
      socket.resume();
    };
    const conn = session.idle.shift();
    if (conn) return give(conn);
    // (none idle: a moment for the connector to dial one, then closed)
    const wait = { give, timer: setTimeout(() => { session.waiting.delete(wait); socket.destroy(); }, limits.waitMs) };
    session.waiting.add(wait);
    socket.once('close', () => { clearTimeout(wait.timer); session.waiting.delete(wait); });
  }

  /** A connector's or a TXT request's greeting, on the control endpoint. */
  function greet(conn) {
    const ip = conn.remoteAddress ?? 'unknown';
    conn.on('error', () => conn.destroy());
    let head = '';
    const late = setTimeout(() => conn.destroy(), limits.helloMs);
    const refuse = (why, quiet = false) => { refusals.take(ip); if (!quiet) log(`relay: refused ${masked(ip)}: ${why}`); conn.end(`REFUSED ${why}\n`); };
    const onData = d => {
      head += d.toString('latin1');
      const end = head.indexOf('\r\n\r\n');
      if (end < 0) { if (head.length > 4 * LINE) conn.destroy(); return; }
      clearTimeout(late);
      conn.off('data', onData);
      if (head.length > end + 4) return conn.destroy(); // (nothing of a connector's after its greeting)
      // (after its refusals pass the limit, an address is answered nothing for a minute)
      if (refusals.peek(ip) >= limits.greetingsPerIp) return conn.destroy();
      const lines = head.slice(0, end).split('\r\n');
      const [magic, label] = lines[0].split(' ');
      const field = name => { const l = lines.slice(1).filter(x => x.toLowerCase().startsWith(`${name}: `)); return l.length === 1 ? l[0].slice(name.length + 2) : l.length ? undefined : null; };
      const bearer = field('authorization');
      const account = magic === 'EXACT-RELAY/1' && LABEL.test(label ?? '') ? accounts.get(label) : null;
      if (!account || typeof bearer !== 'string' || !bearer.startsWith('Bearer ') || !holds(account, bearer.slice(7))) return refuse('no such account');
      const txt = field('txt'), clear = field('txt-clear');
      if (txt !== null || clear !== null) return record(conn, label, txt ?? clear, clear !== null && txt === null, refuse);
      const run = field('run'), names = field('names');
      if (typeof run !== 'string' || !/^[0-9a-f]{32}$/.test(run)) return refuse('a connector says its run');
      if (names === undefined) return refuse('one names line');
      connector(conn, label, run, names, refuse);
    };
    conn.on('data', onData);
  }

  /** A TXT record put up or one value taken down, for the account's own name only. */
  async function record(conn, label, said, clearing, refuse) {
    const [record, value, more] = String(said ?? '').split(' ');
    // (every request counted, refused or not: a connector can't hammer the DNS provider or the CA's limits)
    if (!txts.take(label)) return refuse(`more than ${limits.txtPerAccount} TXT requests a minute`);
    if (more !== undefined || !RECORD.test(record ?? '') || !/^[A-Za-z0-9_-]{43}$/.test(value ?? '')) return refuse('no ACME TXT record of that shape');
    if (record !== `_acme-challenge.${label}.${domain}`) return refuse('not your account\'s name');
    try {
      await (clearing ? dns.clear(record, value) : dns.publish(record, value));
      conn.end('TXT-OK\n');
    } catch (e) { log(`relay: ${label}: the TXT record could not be ${clearing ? 'taken down' : 'put up'}: ${String(e?.message ?? e).slice(0, 120)}`); conn.end('REFUSED the record could not be put up\n'); }
  }

  /** A connector's connection: the session of its run, made the first time (its names the relay's, or of the account's own parents). */
  function connector(conn, label, run, names, refuse) {
    const key = `${label} ${run}`;
    let session = sessions.get(key);
    if (!session) {
      if ([...sessions.values()].filter(s => s.label === label).length >= limits.sessionsPerAccount) return refuse(`more than ${limits.sessionsPerAccount} sessions on this account`);
      let tool, canvas;
      if (names) {
        [tool, canvas] = names.split(' ');
        const parentOf = n => n.slice(n.indexOf('.') + 1);
        const ours = n => NAME.test(n ?? '') && accounts.get(label).names.includes(parentOf(n));
        if (!ours(tool) || !ours(canvas) || tool === canvas || parentOf(tool) !== parentOf(canvas)) return refuse('names not of your account');
        if (byName.has(tool) || byName.has(canvas)) return refuse('names in use by another session');
      } else {
        const id = randomBytes(8).toString('hex').slice(0, 12);
        [tool, canvas] = [`${id}-tool.${label}.${domain}`, `${id}-canvas.${label}.${domain}`];
      }
      session = { key, label, id: tool.split('.')[0].replace(/-tool$/, ''), tool, canvas, idle: [], carried: new Set(), waiting: new Set(), grace: null };
      sessions.set(key, session);
      byName.set(tool, session);
      byName.set(canvas, session);
      log(`relay: ${label} ${session.id}: session up`);
    } else if (names && names !== `${session.tool} ${session.canvas}`) return refuse('names not this run\'s');
    clearTimeout(session.grace);
    session.grace = null;
    const mine = conns.get(label) ?? new Set();
    conns.set(label, mine);
    mine.add(conn);
    conn.once('close', () => { mine.delete(conn); const i = session.idle.indexOf(conn); if (i >= 0) session.idle.splice(i, 1); ended(session); });
    conn.write(`READY ${session.tool} ${session.canvas}\n`);
    // (an idle connection says nothing: one that does is no connector's)
    const quiet = () => conn.destroy();
    conn.on('data', quiet);
    quietOf.set(conn, quiet);
    const waiter = session.waiting.values().next().value;
    if (waiter) { session.waiting.delete(waiter); clearTimeout(waiter.timer); waiter.give(conn); }
    else session.idle.push(conn);
  }

  /** A session with no connection left: after `graceMs`, gone (its names free again). */
  function ended(session) {
    if (session.idle.length || session.carried.size || session.grace || !sessions.has(session.key)) return;
    session.grace = setTimeout(() => {
      if (session.idle.length || session.carried.size) return;
      sessions.delete(session.key);
      byName.delete(session.tool);
      byName.delete(session.canvas);
      log(`relay: ${session.label} ${session.id}: session down`);
    }, limits.graceMs);
    session.grace.unref?.();
  }

  /** The accounts read again: a label gone, revoked or its token changed is cut, every connection and stream. */
  function reload() {
    let next;
    try { next = loadAccounts(accountsPath, domain); } catch (e) { log(`relay: ${config.accounts} not taken (the accounts before it kept): ${e.message}`); return false; }
    for (const [label, a] of accounts) {
      const b = next.accounts.get(label);
      if (b && b.hash.equals(a.hash)) continue;
      for (const c of conns.get(label) ?? []) c.destroy();
      for (const s of sessions.values()) if (s.label === label) { for (const c of s.carried) c.destroy(); for (const w of s.waiting) clearTimeout(w.timer); }
      log(`relay: ${label}: ${b ? 'its token changed' : next.revoked.has(label) ? 'revoked' : 'removed'}: every connection cut`);
    }
    ({ accounts, revoked } = next);
    return true;
  }
  let watcher = null;

  return {
    async listen() {
      const [, host, port] = /^(.*):(\d+)$/.exec(config.listen ?? '127.0.0.1:0') ?? [];
      await new Promise((ok, fail) => { tcp.once('error', fail); tcp.listen(Number(port), host.replace(/^\[|\]$/g, ''), ok); });
      try { watcher = watch(accountsPath, () => reload()); } catch { /* reload() on SIGHUP alone */ }
      return tcp.address().port;
    },
    get port() { return tcp.address()?.port; },
    get revoked() { return revoked; },
    sessions,
    reload,
    async close() {
      watcher?.close();
      for (const s of live) s.destroy();
      for (const set of conns.values()) for (const c of set) c.destroy();
      for (const s of sessions.values()) clearTimeout(s.grace);
      await new Promise(ok => tcp.close(() => ok()));
    },
  };
}

/** Every key the config file takes, as `--help` says them. */
export const CONFIG_HELP = [
  ['domain', 'the relay\'s own name; sessions are <id>-tool.<label>.<domain> and <id>-canvas.<label>.<domain>'],
  ['listen', 'host:port to take clients and connectors on, one port (443 deployed)'],
  ['tls.cert, tls.key', 'PEM files of the relay\'s own certificate and key, for domain only (checked at start: names domain, valid now, the key\'s)'],
  ['accounts', 'the accounts file: {"accounts": {"<label>": {"token_sha256": "<hex>", "names": ["<parent of the account\'s own>"]}}, "revoked": ["<label>"]}; labels.seen is kept beside it'],
  ['dns.command', 'a command that puts an ACME TXT record up or takes one value down (EXACT_ACME_ACTION publish|clear, EXACT_ACME_RECORD, EXACT_ACME_VALUE)'],
  ['limits.perIpPerSession', `client connections a minute from one address to one session (${LIMITS.perIpPerSession})`],
  ['limits.streamsPerSession', `clients carried at once for one session (${LIMITS.streamsPerSession})`],
  ['limits.sessionsPerAccount', `sessions one account holds at once (${LIMITS.sessionsPerAccount})`],
  ['limits.txtPerAccount', `TXT requests a minute from one account (${LIMITS.txtPerAccount})`],
  ['limits.greetingsPerIp', `refused greetings a minute from one address before it is answered nothing (${LIMITS.greetingsPerIp})`],
  ['limits.waitMs, limits.graceMs, limits.helloMs', `a client's wait for an idle connection (${LIMITS.waitMs}), a session's life after its last connection (${LIMITS.graceMs}), a hello's or greeting's time (${LIMITS.helloMs})`],
];

const usage = () => [
  'bun scripts/tamber-relay-server.mjs --config <relay.json>   run the relay',
  'bun scripts/tamber-relay-server.mjs accounts add <label> --config <relay.json>      a new account; its token printed once',
  'bun scripts/tamber-relay-server.mjs accounts revoke <label> --config <relay.json>   revoked for good (never given out again)',
  '',
  'config keys:',
  ...CONFIG_HELP.map(([k, v]) => `  ${k.padEnd(44)} ${v}`),
  '',
  'A ClientHello split over more than one TLS record is closed (no mainstream browser sends one).',
].join('\n');

/** The config file at `path`, its files read (relative to it). */
export function readConfig(path) {
  const at = p => resolve(dirname(path), p);
  const c = JSON.parse(readFileSync(path, 'utf8'));
  return { ...c, tls: { cert: readFileSync(at(c.tls?.cert ?? ''), 'utf8'), key: readFileSync(at(c.tls?.key ?? ''), 'utf8') }, accounts: at(c.accounts ?? 'accounts.json') };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const value = flag => { const i = argv.indexOf(flag); return i >= 0 ? argv[i + 1] : null; };
  if (argv.includes('--help') || !value('--config')) { console.log(usage()); process.exit(argv.includes('--help') ? 0 : 2); }
  const configPath = resolve(value('--config'));
  if (argv[0] === 'accounts') {
    const { writeFileSync, renameSync } = await import('node:fs');
    const config = JSON.parse(readFileSync(configPath, 'utf8'));
    const path = resolve(dirname(configPath), config.accounts ?? 'accounts.json');
    const file = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : { accounts: {}, revoked: [] };
    const label = argv[2];
    if (!LABEL.test(label ?? '')) { console.error('a label: lowercase letters, digits and -'); process.exit(2); }
    let token = null;
    if (argv[1] === 'add') {
      if (file.accounts?.[label] || file.revoked?.includes(label)) { console.error(`${label} is taken, or was: a label is never given out again`); process.exit(1); }
      token = randomBytes(32).toString('base64url');
      file.accounts = { ...file.accounts, [label]: { token_sha256: sha256(token).toString('hex') } };
    } else if (argv[1] === 'revoke') {
      if (!file.accounts?.[label]) { console.error(`${label} is no account`); process.exit(1); }
      delete file.accounts[label];
      file.revoked = [...(file.revoked ?? []), label];
    } else { console.log(usage()); process.exit(2); }
    writeFileSync(`${path}.tmp`, `${JSON.stringify(file, null, 2)}\n`, { mode: 0o600 });
    loadAccounts(`${path}.tmp`, String(config.domain));
    renameSync(`${path}.tmp`, path);
    // (printed once, here, to the operator who asked: never logged, never kept but as its hash)
    if (token) console.log(`${label}: its connector token (EXACT_RELAY_TOKEN), shown once: ${token}`);
    else console.log(`${label}: revoked; a running relay cuts its connections when it reads the file again`);
    process.exit(0);
  }
  const relay = relayServer(readConfig(configPath));
  const port = await relay.listen();
  process.on('SIGHUP', () => relay.reload());
  console.log(`relay: listening on ${port}`);
}
