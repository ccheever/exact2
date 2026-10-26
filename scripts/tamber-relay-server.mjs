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
import { createServer as createTCP, isIP } from 'node:net';
import { basename, dirname, join, resolve } from 'node:path';
import { createServer as createTLS } from 'node:tls';
import { dnsHook, LINE, NAME, RECORD } from './tamber-relay.mjs';

/** An account's label, as the connector checks one. */
export const LABEL = /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/;
/** A TLS record's body, at most (RFC 8446 §5.1). */
const RECORD_MAX = 16_384;
/** What a session's limits are when the config says none. */
export const LIMITS = { txtInFlight: 2, pendingPerIp: 32, pendingTotal: 4096, perIpPerSession: 30, streamsPerSession: 64, idlePerSession: 8, sessionsPerAccount: 4, txtPerAccount: 10, greetingsPerIp: 10, waitMs: 2000, graceMs: 60_000, helloMs: 10_000 };
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
  // (`buf` read by index only, and no index past the first record: a test holds it to that)
  if (buf.length < 5) return { state: 'more' };
  if (buf[0] !== 0x16 || buf[1] !== 0x03) return { state: 'bad' };
  const length = (buf[3] << 8) | buf[4];
  if (length > RECORD_MAX || length < 4) return { state: 'bad' };
  if (buf.length < 5 + length) return { state: 'more' };
  const rec = Buffer.alloc(length), read = 5 + length;
  for (let i = 0; i < length; i++) rec[i] = buf[5 + i];
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
 * label it holds (`<label>`) and each it revokes (`revoked <label>`), for
 * good. Refused (thrown) when a label is malformed; is an account and
 * revoked, now or ever (`labels.seen` says so though `revoked` no longer
 * does); or was seen and is in neither (a label is never given out again);
 * when a token hash is not 64 hex digits; or when a parent of the
 * account's own (`names`, for `--relay-name`) is not a name, is under
 * `domain`, or is another account's or under or over one. */
export function loadAccounts(path, domain) {
  const file = JSON.parse(readFileSync(path, 'utf8'));
  const accounts = new Map(), revoked = new Set(file.revoked ?? []);
  const seenPath = join(dirname(path), 'labels.seen');
  const lines = existsSync(seenPath) ? readFileSync(seenPath, 'utf8').split('\n').filter(Boolean) : [];
  const seen = new Set(lines.map(l => l.replace(/^revoked /, ''))), gone = new Set(lines.filter(l => l.startsWith('revoked ')).map(l => l.slice(8)));
  for (const label of revoked) if (!LABEL.test(label)) throw new Error(`revoked: ${JSON.stringify(label).slice(0, 70)} is no label`);
  const parents = [];
  for (const [label, a] of Object.entries(file.accounts ?? {})) {
    if (!LABEL.test(label)) throw new Error(`${JSON.stringify(label).slice(0, 70)} is no label (lowercase letters, digits and -, a letter or digit at each end)`);
    if (!NAME.test(`${'0'.repeat(12)}-canvas.${label}.${domain}`)) throw new Error(`${label}: its sessions' names would be too long under ${domain}`);
    if (revoked.has(label) || gone.has(label)) throw new Error(`${label} is revoked: a label is never given out again`);
    if (!/^[0-9a-f]{64}$/.test(a?.token_sha256 ?? '')) throw new Error(`${label}: token_sha256 is not 64 hex digits`);
    const names = a.names ?? [];
    for (const p of names) {
      if (!NAME.test(p)) throw new Error(`${label}: ${JSON.stringify(p).slice(0, 70)} is no name`);
      if (p === domain || p.endsWith(`.${domain}`) || domain.endsWith(`.${p}`)) throw new Error(`${label}: ${p} is the relay's own domain, under it or over it`);
      const clash = parents.find(q => q.label !== label && (q.p === p || q.p.endsWith(`.${p}`) || p.endsWith(`.${q.p}`)));
      if (clash) throw new Error(`${p} (${label}) and ${clash.p} (${clash.label}): one account's names may not be, or be under or over, another's`);
      parents.push({ p, label });
    }
    accounts.set(label, { hash: Buffer.from(a.token_sha256, 'hex'), names });
  }
  for (const label of seen) if (!accounts.has(label) && !revoked.has(label) && !gone.has(label)) throw new Error(`${label} was an account and is neither one nor revoked: a label is never given out again`);
  const fresh = [...[...accounts.keys(), ...revoked].filter(l => !seen.has(l)), ...[...revoked].filter(l => !gone.has(l)).map(l => `revoked ${l}`)];
  if (fresh.length) appendFileSync(seenPath, fresh.map(l => `${l}\n`).join(''), { mode: 0o600 });
  return { accounts, revoked: new Set([...revoked, ...gone]) };
}

/** Whether `token` is `account`'s (in constant time, by its hash). */
// (a missing account is compared against nothing it could match, so every refusal costs one hash and one compare)
const NONE = Buffer.alloc(32);
const holds = (account, token) => timingSafeEqual(sha256(String(token)), account?.hash ?? NONE) && !!account;

/** Keys a minute's count holds at most: past it, whatever is new is
 * refused until the minute's old keys are swept (fails closed). */
const KEYS = 100_000;
/** A count of events in the last minute, by key; a key with none in the
 * minute dropped (swept as it grows, and `sweep()`). */
function perMinute(limit, now) {
  const seen = new Map();
  let swept = -Infinity;
  const sweep = () => { const t = now(); swept = t; for (const [k, list] of seen) if (!list.some(x => t - x < MINUTE)) seen.delete(k); };
  return {
    take(key) {
      const t = now(), list = (seen.get(key) ?? []).filter(x => t - x < MINUTE);
      // (full: swept at most once a second, and refused while still full)
      if (!seen.has(key) && seen.size >= KEYS) { if (t - swept >= 1000) sweep(); if (seen.size >= KEYS) return false; }
      if (list.length >= limit) { seen.set(key, list); return false; }
      list.push(t);
      seen.set(key, list);
      return true;
    },
    peek(key) { const t = now(); return (seen.get(key) ?? []).filter(x => t - x < MINUTE).length; },
    sweep,
    get size() { return seen.size; },
  };
}

/** An address as a log shows it: an IPv4's first three parts, an IPv6's first four groups. */
/** An address as the limits count it: an IPv4 (mapped or not) whole, an IPv6 by its /64 (one host's
 * rotating addresses one key). */
export const keyOf = address => {
  const m = masked(address);
  return m.endsWith('.x') ? String(address).replace(/^::ffff:/i, '') : m;
};
export const masked = address => {
  const ip = String(address).replace(/^::ffff:(?=\d+\.\d+\.\d+\.\d+$)/i, '');
  if (/^\d+\.\d+\.\d+\.\d+$/.test(ip)) return ip.replace(/\.\d+$/, '.x');
  if (!isIP(ip)) return 'unknown';
  // (an IPv6 address expanded first, `::` and all: its first four groups, the /64)
  const [head, tail = ''] = ip.split('::');
  const a = head ? head.split(':') : [], b = tail ? tail.split(':') : [];
  const groups = ip.includes('::') ? [...a, ...Array(8 - a.length - b.length).fill('0'), ...b] : a;
  return `${groups.slice(0, 4).map(g => g.toLowerCase().replace(/^0+(?=.)/, '')).join(':')}::/64`;
};

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
  for (const [k, v] of Object.entries(limits)) if (!(k in LIMITS) || !Number.isSafeInteger(v) || v <= 0) throw new Error(`limits.${k}: a whole number above 0 (one of ${Object.keys(LIMITS).join(', ')})`);
  // Its own certificate, for its own name, valid now and its key's: else no start (theme2's 14:34 (5)).
  const own = new X509Certificate(config.tls?.cert ?? '');
  const sans = (own.subjectAltName ?? '').split(', ').filter(Boolean);
  if (!own.checkHost(domain, { partialWildcards: false }) || sans.length !== 1 || sans[0] !== `DNS:${domain}`) throw new Error(`tls.cert does not name ${domain} alone (the relay's own name, nothing more)`);
  if (Date.parse(own.validFrom) > now() || Date.parse(own.validTo) <= now()) throw new Error(`tls.cert is not valid now (${own.validFrom} to ${own.validTo})`);
  if (!own.checkPrivateKey(createPrivateKey(config.tls.key))) throw new Error('tls.key is not tls.cert\'s key');
  // (its handshake within helloMs too: the first record's deadline runs on through it)
  const control = createTLS({ cert: config.tls.cert, key: config.tls.key, minVersion: 'TLSv1.2', handshakeTimeout: limits.helloMs }, conn => greet(conn));
  control.on('tlsClientError', (_e, s) => s?.destroy());
  const accountsPath = resolve(config.accounts);
  let { accounts, revoked } = loadAccounts(accountsPath, domain);
  /** Each label's pending DNS work, stopped when the label is cut. */
  const stops = new Map(), inFlight = new Map();
  const stopOf = label => { let c = stops.get(label); if (!c || c.signal.aborted) { c = new AbortController(); stops.set(label, c); } return c.signal; };
  const dns = config.dns?.command
    ? { publish: (r, v, signal) => dnsHook(config.dns.command, r, v, { signal }), clear: (r, v, signal) => dnsHook(config.dns.command, r, v, { clear: true, signal }) }
    : config.dns;
  /** Each live session, by its key (`<label> <run>`) and by each of its names. */
  const sessions = new Map(), byName = new Map();
  /** Each label's control connections (idle, carrying, or asking for a TXT record), cut together. */
  const conns = new Map();
  const clientsPerIp = perMinute(limits.perIpPerSession, now), txts = perMinute(limits.txtPerAccount, now), refusals = perMinute(limits.greetingsPerIp, now);
  const sweeper = setInterval(() => { for (const m of [clientsPerIp, txts, refusals]) m.sweep(); }, MINUTE);
  sweeper.unref?.();
  const tcp = createTCP({ pauseOnConnect: false }, socket => route(socket));
  const live = new Set(), quietOf = new WeakMap();
  /** Connections not yet carried or authenticated, counted, by address. */
  const pending = new Set(), pendingPerIp = new Map(), controlRaw = new Map();
  const hold = (socket, ip) => {
    pending.add(socket);
    pendingPerIp.set(ip, (pendingPerIp.get(ip) ?? 0) + 1);
    socket.once('close', () => settle(socket));
    socket.pendingIp = ip;
  };
  const settle = socket => {
    if (!socket || !pending.delete(socket)) return;
    const n = (pendingPerIp.get(socket.pendingIp) ?? 1) - 1;
    if (n > 0) pendingPerIp.set(socket.pendingIp, n); else pendingPerIp.delete(socket.pendingIp);
  };
  /** A refused or answered connection: ended, and destroyed a moment after whether or not its peer closes. */
  const finish = (conn, text) => { conn.end(text); setTimeout(() => conn.destroy(), 1000).unref?.(); };

  /** A client's first record read (never more), then down to its session, to the control endpoint, or closed. */
  function route(socket) {
    live.add(socket);
    socket.once('close', () => live.delete(socket));
    socket.on('error', () => socket.destroy());
    // (not yet a session's or an authenticated connector's: at most `pendingPerIp` from one address, `pendingTotal` in all)
    const ip = keyOf(socket.remoteAddress ?? 'unknown');
    if (pending.size >= limits.pendingTotal || (pendingPerIp.get(ip) ?? 0) >= limits.pendingPerIp) return socket.destroy();
    hold(socket, ip);
    // The chunks as they came, kept to be handed on whole; the parser given only the first record's bytes (the
    // header's five, then as many as it says), copied out of them, never a byte past it.
    const chunks = [];
    let total = 0;
    const late = setTimeout(() => socket.destroy(), limits.helloMs);
    socket.once('close', () => clearTimeout(late));
    const onData = d => {
      chunks.push(d);
      total += d.length;
      const need = total < 5 ? 5 : 5 + ((Buffer.concat(chunks, 5)[3] << 8) | Buffer.concat(chunks, 5)[4]);
      const h = helloName(Buffer.concat(chunks, Math.min(total, need)));
      if (h.state === 'more') { if (total > 5 + RECORD_MAX) socket.destroy(); return; }
      const buf = Buffer.concat(chunks);
      clearTimeout(late);
      socket.off('data', onData);
      socket.pause();
      if (h.state !== 'ok' || !h.sni) return socket.destroy();
      if (h.sni === domain) {
        // (the control endpoint: its TLS ends here; the bytes read so far given back first)
        socket.unshift(buf);
        // (its TLS socket found again, by its peer, when it authenticates: pending until then)
        const peer = `${socket.remoteAddress} ${socket.remotePort}`;
        controlRaw.set(peer, socket);
        socket.once('close', () => controlRaw.delete(peer));
        control.emit('connection', socket);
        return;
      }
      const session = byName.get(h.sni);
      if (!session) return socket.destroy();
      settle(socket);
      hand(session, h.sni, socket, buf);
    };
    socket.on('data', onData);
  }

  /** `socket` (its first bytes `first`) down an idle connection of `session`, or closed. Carried and waiting
   * together at most `streamsPerSession`. */
  function hand(session, name, socket, first) {
    const ip = socket.remoteAddress ?? 'unknown';
    if (!clientsPerIp.take(`${session.key} ${keyOf(ip)}`)) { log(`relay: ${session.label} ${session.id}: ${masked(ip)} over ${limits.perIpPerSession} a minute`); return socket.destroy(); }
    if (session.carried.size + session.waiting.size >= limits.streamsPerSession) return socket.destroy();
    const give = conn => {
      conn.off('data', quietOf.get(conn));
      session.carried.add(conn);
      conn.once('close', () => { session.carried.delete(conn); socket.destroy(); ended(session); });
      socket.once('close', () => conn.destroy());
      conn.write(Buffer.concat([Buffer.from(`CLIENT ${name} ${ip.replace(/^::ffff:/i, '')}\n`), first]));
      socket.pipe(conn);
      conn.pipe(socket);
      socket.resume();
    };
    const conn = session.idle.shift();
    if (conn) return give(conn);
    // (none idle: a moment for the connector to dial one, then closed)
    const wait = { give, socket, timer: setTimeout(() => socket.destroy(), limits.waitMs) };
    session.waiting.add(wait);
    socket.once('close', () => { clearTimeout(wait.timer); session.waiting.delete(wait); ended(session); });
  }

  /** A connector's or a TXT request's greeting, on the control endpoint. */
  function greet(conn) {
    const ip = keyOf(conn.remoteAddress ?? 'unknown');
    conn.on('error', () => conn.destroy());
    let head = '';
    const late = setTimeout(() => conn.destroy(), limits.helloMs);
    conn.once('close', () => clearTimeout(late));
    // (a refusal it can't count, its counts full, is answered nothing)
    const refuse = why => { if (!refusals.take(ip)) return conn.destroy(); log(`relay: refused ${masked(ip)}: ${why}`); finish(conn, `REFUSED ${why}\n`); };
    const onData = d => {
      head += d.toString('latin1');
      const end = head.indexOf('\r\n\r\n');
      if ((end < 0 ? head.length : end) > 4 * LINE) return conn.destroy();
      if (end < 0) return;
      clearTimeout(late);
      conn.off('data', onData);
      if (head.length > end + 4) return conn.destroy(); // (nothing of a connector's after its greeting)
      // (after its refusals pass the limit, an address is answered nothing for a minute)
      if (refusals.peek(ip) >= limits.greetingsPerIp) return conn.destroy();
      const lines = head.slice(0, end).split('\r\n');
      const [magic, label] = lines[0].split(' ');
      const field = name => { const l = lines.slice(1).filter(x => x.toLowerCase().startsWith(`${name}: `)); return l.length === 1 ? l[0].slice(name.length + 2) : l.length ? undefined : null; };
      const bearer = field('authorization');
      const account = magic === 'EXACT-RELAY/1' && LABEL.test(label ?? '') ? accounts.get(label) : undefined;
      const said = typeof bearer === 'string' && bearer.startsWith('Bearer ') ? bearer.slice(7) : '';
      if (!holds(account, said)) return refuse('no such account');
      // (refusals of an authenticated connection, its own account's limits, never count to its address's silence)
      const answer = why => { log(`relay: ${label}: refused: ${why}`); finish(conn, `REFUSED ${why}\n`); };
      // (every authenticated connection the account's, cut with it; no longer pending)
      settle(controlRaw.get(`${conn.remoteAddress} ${conn.remotePort}`));
      const mine = conns.get(label) ?? new Set();
      conns.set(label, mine);
      mine.add(conn);
      conn.once('close', () => mine.delete(conn));
      const txt = field('txt'), clear = field('txt-clear');
      if (txt !== null || clear !== null) return record(conn, label, txt ?? clear, clear !== null && txt === null, answer);
      const run = field('run'), names = field('names');
      if (typeof run !== 'string' || !/^[0-9a-f]{32}$/.test(run)) return answer('a connector says its run');
      if (names === undefined || (names !== null && names.split(' ').length !== 2)) return answer('one names line, of two names');
      connector(conn, label, run, names, answer);
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
    // (at most `txtInFlight` an account at once: its DNS work never piles up)
    if ((inFlight.get(label) ?? 0) >= limits.txtInFlight) return refuse(`more than ${limits.txtInFlight} TXT requests at once`);
    inFlight.set(label, (inFlight.get(label) ?? 0) + 1);
    // (stopped when the account is cut: no record put up for a revoked account)
    const signal = stopOf(label);
    const did = clearing ? 'taken down' : 'put up';
    try {
      await (clearing ? dns.clear(record, value, signal) : dns.publish(record, value, signal));
      finish(conn, 'TXT-OK\n');
    } catch (e) { log(`relay: ${label}: the TXT record could not be ${did}: ${String(e?.message ?? e).slice(0, 120)}`); finish(conn, `REFUSED the record could not be ${did}\n`); }
    finally { const n = inFlight.get(label) - 1; if (n > 0) inFlight.set(label, n); else inFlight.delete(label); }
  }

  /** A run's id: of its label and run id, so a run's names are its own whatever the relay remembers (after a
   * restart or a lapse it gets them back, and no other run can claim them). */
  const idOf = (label, run) => createHash('sha256').update(`${label} ${run}`).digest('hex').slice(0, 12);
  /** The names a greeting says, for `label`'s `run`: under a parent the account holds, or the relay's own form
   * for this run's id. Null when neither. */
  function namesFor(label, run, tool, canvas) {
    if (!NAME.test(tool ?? '') || !NAME.test(canvas ?? '') || tool === canvas) return null;
    const parentOf = n => n.slice(n.indexOf('.') + 1);
    if (parentOf(tool) !== parentOf(canvas)) return null;
    if (accounts.get(label).names.includes(parentOf(tool))) return { tool, canvas, id: tool.split('.')[0] };
    const id = idOf(label, run);
    return tool === `${id}-tool.${label}.${domain}` && canvas === `${id}-canvas.${label}.${domain}` ? { tool, canvas, id } : null;
  }

  /** A connector's connection: the session of its run, made the first time (its names the relay's, or the
   * account's own, or the relay's from before for this label). */
  function connector(conn, label, run, names, refuse) {
    const key = `${label} ${run}`;
    let session = sessions.get(key);
    if (!session) {
      let said;
      if (names) {
        said = namesFor(label, run, ...names.split(' '));
        if (!said) return refuse('names not of your account');
      } else {
        const id = idOf(label, run);
        said = { tool: `${id}-tool.${label}.${domain}`, canvas: `${id}-canvas.${label}.${domain}`, id };
      }
      // (names a session of this account holds with no connector connection left, in its grace: a restarted
      // server, a new run, takes them over, the clients waiting on the old one closed (they dial again); a live
      // session's are refused, and another account's never reach here)
      const lapsed = [said.tool, said.canvas].map(n => byName.get(n)).filter(h => h && h.label === label && !h.idle.length && !h.carried.size);
      if ([said.tool, said.canvas].some(n => byName.has(n) && !lapsed.includes(byName.get(n)))) return refuse('names in use by another session');
      // (the cap counted after the session it replaces is gone)
      if ([...sessions.values()].filter(x => x.label === label && !lapsed.includes(x)).length >= limits.sessionsPerAccount) return refuse(`more than ${limits.sessionsPerAccount} sessions on this account`);
      for (const held of new Set(lapsed)) drop(held, 'taken over by a new run');
      session = { key, label, run, ...said, idle: [], carried: new Set(), waiting: new Set(), grace: null };
      sessions.set(key, session);
      byName.set(session.tool, session);
      byName.set(session.canvas, session);
      log(`relay: ${label} ${session.id}: session up`);
    } else if (names && names !== `${session.tool} ${session.canvas}`) return refuse('names not this run\'s');
    // (a run's idle connections, at most `idlePerSession`: more are closed, said nothing, and the connector waits)
    if (session.idle.length >= limits.idlePerSession) return conn.destroy();
    clearTimeout(session.grace);
    session.grace = null;
    conn.once('close', () => { const i = session.idle.indexOf(conn); if (i >= 0) session.idle.splice(i, 1); ended(session); });
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
    if (session.idle.length || session.carried.size || session.waiting.size || session.grace || !sessions.has(session.key)) return;
    session.grace = setTimeout(() => drop(session, 'down'), limits.graceMs);
    session.grace.unref?.();
  }
  /** A session gone: its names free, its clients (waiting or carried) and connections cut. */
  function drop(session, why) {
    clearTimeout(session.grace);
    for (const w of session.waiting) { clearTimeout(w.timer); w.socket.destroy(); }
    for (const c of [...session.idle, ...session.carried]) c.destroy();
    session.waiting.clear();
    if (sessions.get(session.key) !== session) return;
    sessions.delete(session.key);
    byName.delete(session.tool);
    byName.delete(session.canvas);
    log(`relay: ${session.label} ${session.id}: session ${why}`);
  }

  /** The accounts read again. A label gone, revoked or its token changed is cut: every connection (idle,
   * carrying or asking for a TXT record), every client waiting, its DNS work; a session whose names are no
   * longer its account's (a parent moved to another account) is dropped. */
  function reload() {
    let next;
    try { next = loadAccounts(accountsPath, domain); } catch (e) { log(`relay: ${config.accounts} not taken (the accounts before it kept): ${e.message}`); return false; }
    const cut = new Set();
    for (const [label, a] of accounts) {
      const b = next.accounts.get(label);
      if (b && b.hash.equals(a.hash)) continue;
      cut.add(label);
      log(`relay: ${label}: ${b ? 'its token changed' : next.revoked.has(label) ? 'revoked' : 'removed'}: every connection cut`);
    }
    ({ accounts, revoked } = next);
    for (const label of cut) {
      stops.get(label)?.abort();
      for (const c of conns.get(label) ?? []) c.destroy();
      for (const s of [...sessions.values()]) if (s.label === label) drop(s, 'cut');
    }
    for (const s of [...sessions.values()]) if (!namesFor(s.label, s.run, s.tool, s.canvas)) drop(s, 'cut: its names are no longer its account\'s');
    return true;
  }
  let watcher = null;

  return {
    async listen() {
      const [, host, port] = /^(.*):(\d+)$/.exec(config.listen ?? '127.0.0.1:0') ?? [];
      await new Promise((ok, fail) => { tcp.once('error', fail); tcp.listen(Number(port), host.replace(/^\[|\]$/g, ''), ok); });
      // (the directory watched, not the file: a file replaced by a rename, as `accounts` writes it, is seen every time)
      const file = basename(accountsPath);
      try { watcher = watch(dirname(accountsPath), (_e, name) => { if (!name || name === file) reload(); }); } catch { /* reload() on SIGHUP alone */ }
      return tcp.address().port;
    },
    get port() { return tcp.address()?.port; },
    get revoked() { return revoked; },
    sessions,
    reload,
    async close() {
      watcher?.close();
      clearInterval(sweeper);
      for (const s of live) s.destroy();
      for (const set of conns.values()) for (const c of set) c.destroy();
      for (const s of [...sessions.values()]) drop(s, 'closed');
      for (const c of stops.values()) c.abort();
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
  ['limits.streamsPerSession', `clients carried or waiting at once for one session (${LIMITS.streamsPerSession})`],
  ['limits.pendingPerIp, limits.pendingTotal', `connections not yet a session's or an authenticated connector's, from one address (${LIMITS.pendingPerIp}) and in all (${LIMITS.pendingTotal})`],
  ['limits.idlePerSession', `a session's idle connector connections at once (${LIMITS.idlePerSession})`],
  ['limits.sessionsPerAccount', `sessions one account holds at once (${LIMITS.sessionsPerAccount})`],
  ['limits.txtPerAccount', `TXT requests a minute from one account (${LIMITS.txtPerAccount})`],
  ['limits.txtInFlight', `TXT requests one account has under way at once (${LIMITS.txtInFlight})`],
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
    const { writeFileSync, renameSync, unlinkSync } = await import('node:fs');
    const config = JSON.parse(readFileSync(configPath, 'utf8'));
    const path = resolve(dirname(configPath), config.accounts ?? 'accounts.json');
    // (one change at a time: read, checked and renamed in under a lock, so two can't undo each other)
    const lock = `${path}.lock`;
    for (let tries = 0; ; tries++) {
      try { writeFileSync(lock, String(process.pid), { flag: 'wx', mode: 0o600 }); break; } catch (e) {
        if (e.code !== 'EEXIST' || tries > 100) { console.error(`${lock} is held (another accounts change, or one that died: remove it)`); process.exit(1); }
        await new Promise(ok => setTimeout(ok, 50));
      }
    }
    process.on('exit', () => { try { unlinkSync(lock); } catch { /* gone */ } });
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
    const tmp = `${path}.${process.pid}.tmp`;
    writeFileSync(tmp, `${JSON.stringify(file, null, 2)}\n`, { mode: 0o600 });
    try { loadAccounts(tmp, String(config.domain).toLowerCase()); } catch (e) { unlinkSync(tmp); console.error(e.message); process.exit(1); }
    renameSync(tmp, path);
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
