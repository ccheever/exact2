// The Tamber wire's remote doors (LLP 1121 D13, LLP 1122 D12; stage 10 B).
// Off unless asked: `--remote <host:port>` opens a listener that speaks TLS
// only, with a certificate made for this run in memory (its SHA-256
// fingerprint printed, for the client to pin); `--trust-tunnel <port>` opens
// a plain listener on loopback for a tunnel the developer runs (stage 10 B2).
// On either, only the wire's paths (and the design tool's own files) are served, every request is told it is remote (never
// `access.local`, so the local token's routes stay closed), its peer is its
// socket's, no Cookie reaches the wire, and a page of another origin is
// refused. The main listener and its gate are untouched.
import { createHash, generateKeyPairSync, randomBytes, sign, X509Certificate } from 'node:crypto';
import { createServer as createPlain } from 'node:http';
import { createSecureContext, createServer as createTLS, TLSSocket } from 'node:tls';
import { EventEmitter } from 'node:events';
import { isIP } from 'node:net';
import { networkInterfaces } from 'node:os';
import { allowHostArgs } from './install-page.mjs';
import { connectRelay, dnsHook, parentOf, relayArgs, relayTxt } from './tamber-relay.mjs';
import { CAA_HONOURING, caaGoverns, relayCertificate } from './tamber-acme.mjs';
import { serveToolFiles, TOOL_DIST } from './tamber-tool-files.mjs';

const DAY = 86_400_000;

/** A certificate held for relayed TLS: its context, fingerprint, key pin,
 * and whether it covers a session's names. */
function hold(cert, key) {
  const x = new X509Certificate(cert);
  return {
    context: createSecureContext({ cert, key, minVersion: 'TLSv1.2' }),
    fingerprint: x.fingerprint256,
    spki: createHash('sha256').update(x.publicKey.export({ type: 'spki', format: 'der' })).digest('base64'),
    covers: n => [n.tool, n.canvas].every(h => !!x.checkHost(h, { partialWildcards: false })),
    /** When it ends: no client's TLS ends on it after. */
    until: Date.parse(x.validTo),
  };
}

// DER, as much of it as a certificate needs (X.690).
const len = n => n < 0x80 ? Buffer.from([n]) : n < 0x100 ? Buffer.from([0x81, n]) : Buffer.from([0x82, n >> 8, n & 0xff]);
const tlv = (tag, ...parts) => { const body = Buffer.concat(parts); return Buffer.concat([Buffer.from([tag]), len(body.length), body]); };
const seq = (...parts) => tlv(0x30, ...parts);
/** A non-negative INTEGER from big-endian `bytes`, in DER's minimal form. */
export function derInteger(bytes) {
  let start = 0;
  while (start < bytes.length - 1 && bytes[start] === 0) start++;
  const min = bytes.subarray(start);
  return tlv(0x02, min[0] & 0x80 ? Buffer.concat([Buffer.from([0]), min]) : min);
}
const int = derInteger;
/** An IPv6 address's sixteen bytes. */
function ipv6Bytes(address) {
  const [head, tail = null] = address.split('::');
  const words = part => (part ? part.split(':') : []).flatMap(w => w.includes('.') ? (([a, b, c, d]) => [(a << 8 | b).toString(16), (c << 8 | d).toString(16)])(w.split('.').map(Number)) : [w]);
  const h = words(head), t = tail === null ? [] : words(tail);
  const all = [...h, ...Array(8 - h.length - t.length).fill('0'), ...t];
  return Buffer.from(all.flatMap(w => { const n = parseInt(w, 16); return [n >> 8, n & 0xff]; }));
}
const time = ms => { const s = new Date(ms).toISOString().replace(/[-:T]|\.\d+/g, ''); return tlv(0x17, Buffer.from(s.slice(2))); };
function oid(dotted) {
  const [a, b, ...rest] = dotted.split('.').map(Number), out = [40 * a + b];
  for (let v of rest) { const bytes = [v & 0x7f]; while ((v >>= 7)) bytes.unshift(0x80 | (v & 0x7f)); out.push(...bytes); }
  return tlv(0x06, Buffer.from(out));
}
const ECDSA_SHA256 = seq(oid('1.2.840.10045.4.3.2'));

/** A self-signed X.509 v3 certificate for `names` (DNS names and IPv4
 * addresses), an ECDSA P-256 key made now, signed SHA-256, valid from a
 * minute before `now` for `days`. Nothing touches the disk; the key is the
 * caller's to keep in memory. Seven days by default: the certificate lives
 * for one run and is pinned by its fingerprint, so its validity only has to
 * outlast a paired token's (`--pair-ttl`, 12 hours by default). */
export function selfSignedCertificate(names, { days = 7, now = Date.now() } = {}) {
  const { privateKey, publicKey } = generateKeyPairSync('ec', { namedCurve: 'P-256' });
  const subject = seq(tlv(0x31, seq(oid('2.5.4.3'), tlv(0x0c, Buffer.from('exact tamber')))));
  const alt = [...new Set(names)].map(n => isIP(n) === 4 ? tlv(0x87, Buffer.from(n.split('.').map(Number))) : isIP(n) === 6 ? tlv(0x87, ipv6Bytes(n)) : tlv(0x82, Buffer.from(n)));
  const extensions = tlv(0xa3, seq(seq(oid('2.5.29.17'), tlv(0x04, seq(...alt)))));
  // Positive, and a leading byte that is never zero: 16 bytes, minimal as it stands.
  const serial = randomBytes(16);
  serial[0] = 0x40 | (serial[0] & 0x3f);
  const tbs = seq(tlv(0xa0, int(Buffer.from([2]))), int(serial), ECDSA_SHA256, subject, seq(time(now - 60_000), time(now + days * DAY)), subject,
                  publicKey.export({ type: 'spki', format: 'der' }), extensions);
  const der = seq(tbs, ECDSA_SHA256, tlv(0x03, Buffer.from([0]), sign('sha256', tbs, privateKey)));
  const cert = `-----BEGIN CERTIFICATE-----\n${der.toString('base64').match(/.{1,64}/g).join('\n')}\n-----END CERTIFICATE-----\n`;
  return { cert, key: privateKey.export({ type: 'pkcs8', format: 'pem' }), fingerprint: new X509Certificate(cert).fingerprint256 };
}

/** `--remote <host:port>` and `--trust-tunnel <port>` in argv (each once):
 * `{remote: {host, port} | null, tunnel: port | null, plugin, hosts}`, with
 * `--allow-plugin-origin` (stage 10b) and the `--allow-host` names, which a
 * tunnel's door answers to as well (a tunnel with a real certificate of its
 * own). A malformed value throws, or with `exit` is said and exits 2. */
export function remoteArgs(argv, { exit = false } = {}) {
  if (exit) { try { return remoteArgs(argv); } catch (e) { console.error(e.message); process.exit(2); } }
  const value = flag => { const i = argv.indexOf(flag); return i < 0 ? null : argv[i + 1] ?? ''; };
  const port = (text, what) => {
    if (!/^\d{1,5}$/.test(text) || Number(text) > 65535) throw new Error(`${what} takes a port, 0–65535, not ${JSON.stringify(text)}`);
    return Number(text);
  };
  const r = value('--remote'), t = value('--trust-tunnel');
  let remote = null;
  if (r !== null) {
    const m = /^(\[[0-9a-f:.]+\]|[a-z0-9.-]+):(\d+)$/i.exec(r);
    if (!m) throw new Error(`--remote takes host:port (192.168.1.20:7443, 0.0.0.0:0), not ${JSON.stringify(r)}`);
    const host = m[1].replace(/^\[|\]$/g, '').toLowerCase();
    remote = { host: isIP(host) === 6 ? new URL(`http://[${host}]/`).hostname.slice(1, -1) : host, port: port(m[2], '--remote') };
  }
  return { remote, tunnel: t === null ? null : port(t, '--trust-tunnel'), plugin: argv.includes('--allow-plugin-origin'), hosts: allowHostArgs(argv), relay: relayArgs(argv) };
}

/** The names a remote listener on `host` answers to (its certificate's). */
export function remoteNames(host, interfaces = networkInterfaces()) {
  const names = ['localhost', '127.0.0.1'];
  const canonical = h => isIP(h) === 6 ? new URL(`http://[${h}]/`).hostname.slice(1, -1) : h.toLowerCase();
  if (host === '0.0.0.0' || host === '::') {
    for (const entries of Object.values(interfaces)) for (const e of entries ?? [])
      if (!e.internal && (e.family === 'IPv4' || host === '::' && e.family === 'IPv6' && !e.address.startsWith('fe80'))) names.push(canonical(e.address));
  } else names.push(canonical(host));
  return [...new Set(names)];
}

// The wire's own paths (its token, local only, stays out).
const WIRE = new Set(['/__tamber', '/__tamber/subscribe', '/__tamber/events', '/__tamber/pair']);
const LOOPBACK = new Set(['localhost', '127.0.0.1', '::1']);

// A sandboxed page's (a Figma plugin's) cross-origin requests, with
// `--allow-plugin-origin`: CORS for `Origin: null` on the wire's paths only.
const CORS = { 'access-control-allow-origin': 'null', 'access-control-expose-headers': 'retry-after', vary: 'origin' };
const PREFLIGHT = { ...CORS, 'access-control-allow-methods': 'POST', 'access-control-allow-headers': 'content-type, x-exact-tamber-token', 'access-control-max-age': '600' };

/** Every Cookie header out of `req`, in every view of its headers. */
function stripCookies(req) {
  delete req.headers.cookie;
  void req.headersDistinct; delete req.headersDistinct?.cookie;
  const raw = req.rawHeaders;
  for (let i = raw.length - 2; i >= 0; i -= 2) if (String(raw[i]).toLowerCase() === 'cookie') raw.splice(i, 2);
}

/** A sandboxed page's request on the main loopback port (stage 10b, with
 * `--allow-plugin-origin`): `Origin: null`, from this machine, naming a
 * loopback host, on the wire's paths. It is no local request: the wire
 * takes it as a remote one (a paired token, never the local one), its
 * Cookie removed, with CORS for `null`. True when answered; anything else
 * is the loop's, its gate first. */
export async function pluginOnLoopback(wire, req, res) {
  if (req.headers.origin !== 'null' || !['127.0.0.1', '::1', '::ffff:127.0.0.1'].includes(req.socket?.remoteAddress)) return false;
  // As the dev gate's local test has it: this port, and no proxy's mark (`forwarded`, or any `x-forwarded-*`).
  if (Object.keys(req.headers).some(h => h === 'forwarded' || h.startsWith('x-forwarded-'))) return false;
  const at = hostOf(req, 'http'), here = String(req.socket.localPort);
  const url = at && LOOPBACK.has(at.name) && (at.port || '80') === here ? targetOf(req, 'http', at.host) : null;
  if (!url || !WIRE.has(url.pathname)) return false;
  stripCookies(req);
  for (const [k, v] of Object.entries({ 'cache-control': 'no-store', 'x-content-type-options': 'nosniff', 'referrer-policy': 'no-referrer', ...CORS })) res.setHeader(k, v);
  if (req.method === 'OPTIONS') { res.writeHead(204, PREFLIGHT); res.end(); return true; }
  const access = { allowed: true, local: false, remote: true, tunnel: false, door: 'local', plugin: true, peer: req.socket.remoteAddress };
  if (!(await wire.handle(req, res, url, access)) && !res.headersSent) refuse(res, 404, 'not found');
  return true;
}

const refuse = (res, status, message) => {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify({ message }));
};

/** A request handler for a remote door. `admits(hostName)` says which
 * names its Host may carry; `scheme` is what its pages' origins say. The
 * design tool's own files are served too (no token: its code is no secret
 * and it holds no design until it pairs), so a remote tool is same-origin
 * with the wire; nothing of the app is. */
/** The request's Host in its canonical form (`new URL`'s, so `127.1` is
 * `127.0.0.1`), its name without brackets, and the port as written (none
 * when none is: `name:80` is not `name`), or null when it names none. */
function hostOf(req, scheme) {
  try {
    const raw = String(req.headers.host ?? ''), url = new URL(`${scheme}://${raw}/`), written = /:(\d+)$/.exec(raw);
    return url.host && url.pathname === '/' && !url.username && !url.password ? { host: url.host, name: url.hostname.replace(/^\[|\]$/g, ''), port: written ? String(Number(written[1])) : '' } : null;
  } catch { return null; }
}
/** An origin-form target, on the admitted host, routed exactly as written:
 * no `//` anywhere in its path, no backslash, no dot segment, no encoded
 * dot, slash or backslash, so the path checked is the path every handler
 * reads (the lead's 05:05Z note from C's round 1); its query, which no
 * route reads as a path, only as written (the events' GET: a client name
 * may hold a slash). Null when it is not one. */
function targetOf(req, scheme, host) {
  const target = req.url, path = target.split('?', 1)[0];
  if (!path.startsWith('/') || path.includes('//') || target.includes('\\') || /%(2e|2f|5c)/i.test(path)) return null;
  try {
    const url = new URL(target, `${scheme}://${host}`);
    return url.host === host && url.pathname + url.search === target ? url : null;
  } catch { return null; }
}

export function remoteHandler(wire, { tunnel, scheme, admits, toolDir = TOOL_DIST, plugin = false, publicName = () => false, peerOf = req => (isIP(req.socket?.remoteAddress ?? '') ? req.socket.remoteAddress : null), door = tunnel ? 'tunnel' : 'tls' }) {
  return async (req, res) => {
    // A connection 8 MiB behind on what it was sent asks nothing more, the
    // tool's files included: answers queue only while they are read.
    if ((req.socket?.writableLength || 0) > 8 << 20) { req.socket.destroy(); return; }
    res.setHeader('cache-control', 'no-store');
    res.setHeader('x-content-type-options', 'nosniff');
    res.setHeader('referrer-policy', 'no-referrer');
    // A cookie is never a credential here, whoever set it (a canvas page can
    // set one this port receives): none reaches the wire, in any view of the
    // headers (`headersDistinct` is built from the raw ones when first read).
    stripCookies(req);
    const at = hostOf(req, scheme);
    if (!at || !admits(at)) return refuse(res, 404, 'not found');
    // A sandboxed page (`Origin: null`, a Figma plugin) only with
    // `--allow-plugin-origin`, and only on the wire's paths, with CORS.
    const sandboxed = req.headers.origin === 'null' && plugin;
    if (!sandboxed && req.headers.origin !== undefined) {
      // A page's own origin: this door's scheme, or https through a tunnel
      // whose public name answers with a certificate of its own; the same
      // host and effective port, each side's left-out port its scheme's
      // default (a public name's Host https's), never the other side's.
      let same = false;
      try {
        // (a tunnel's public name is https only, a loopback name the door's scheme only)
        const o = new URL(req.headers.origin), want = publicName(at.name) ? 'https:' : `${scheme}:`;
        const port = (p, protocol) => p || (protocol === 'https:' ? '443' : '80');
        same = o.protocol === want && o.hostname.replace(/^\[|\]$/g, '') === at.name && port(o.port, o.protocol) === port(at.port, want);
      } catch { /* refused */ }
      if (!same) return refuse(res, 403, 'another origin');
    }
    const url = targetOf(req, scheme, at.host);
    if (!url) return refuse(res, 404, 'not found');
    if (sandboxed) {
      if (!WIRE.has(url.pathname)) return refuse(res, 403, 'another origin');
      for (const [k, v] of Object.entries(CORS)) res.setHeader(k, v);
      if (req.method === 'OPTIONS') { res.writeHead(204, PREFLIGHT); res.end(); return; }
    }
    if (url.pathname === '/__tamber/tool' || url.pathname.startsWith('/__tamber/tool/')) {
      // The tool's own files send their origin, and only that, as a referrer
      // (the lead's 08:22Z ruling: one place, no per-form policy in the tool);
      // everything else here sends none.
      res.setHeader('referrer-policy', 'strict-origin');
      return serveToolFiles(req, res, url, toolDir, { 'content-security-policy': "frame-ancestors 'none'" });
    }
    if (!WIRE.has(url.pathname)) return refuse(res, 404, 'not found');
    // (`door`: which door it came through, set by this code where it accepted the stream, never read from a request or the relay)
    const access = { allowed: true, local: false, remote: true, tunnel, door, peer: peerOf(req), ...(req.socket?.relayed ? { reported: req.socket.relayed } : {}) };
    if (!(await wire.handle(req, res, url, access)) && !res.headersSent) refuse(res, 404, 'not found');
  };
}

/** At most 64 connections, and none idle or slow for `ms`: from the moment
 * TCP accepts it, a connection has `ms` to finish its handshake and send a
 * request whole (its head and body), however it trickles the bytes, and as
 * long again after each answer for the next. Bun 1.4.2 takes `headersTimeout`,
 * `requestTimeout`, `handshakeTimeout` and an HTTPS server's
 * `maxConnections` without enforcing them before a handshake, so a count
 * and a deadline of its own do it, stopped once a request has arrived
 * whole while it is answered (a long poll, the stream). `server` accepts
 * the sockets; `http` parses their requests. */
function guard(server, ms, http = server) {
  // Per connection: its deadline, the requests whose bodies are still
  // arriving, and those whose answers have not finished. The deadline runs
  // while anything is arriving or nothing is being answered, and starts again
  // at each request and each time the connection falls idle.
  const sockets = new Map();
  const arm = (socket, at) => { clearTimeout(at.timer); at.timer = setTimeout(() => socket.destroy(), ms); };
  const stop = at => { clearTimeout(at.timer); at.timer = null; };
  server.on('connection', socket => {
    if (sockets.size >= 64) return socket.destroy();
    const at = { timer: null, incoming: 0, outstanding: 0 };
    sockets.set(socket, at);
    arm(socket, at);
    socket.once('close', () => { stop(at); sockets.delete(socket); });
  });
  http.on('request', (req, res) => {
    const socket = req.socket, at = sockets.get(socket);
    if (!at) return;
    // A deadline already running (the connection's first, or its idle one)
    // runs on through this request's body: a head sent late gains no time.
    at.incoming++; at.outstanding++;
    if (!at.timer) arm(socket, at);
    req.once('end', () => {
      at.incoming--;
      if (at.incoming === 0 && at.outstanding > 0) stop(at);
      else if (at.incoming === 0 && !socket.destroyed) arm(socket, at);
    });
    // (An answer sent before its body has arrived leaves the body's deadline running.)
    res.once('finish', () => { at.outstanding--; if (at.outstanding === 0 && at.incoming === 0 && !socket.destroyed) arm(socket, at); });
  });
  server.shut = () => new Promise(ok => { for (const s of sockets.keys()) s.destroy(); server.close(() => ok()); });
  return server;
}
/** A TLS listener on `certificate` whose requests go to `handler`: TLS
 * accepts (so the guard sees a socket before its handshake), HTTP parses.
 * (Bun 1.4.2 parses such a socket with its fallback, which closes a
 * connection that pipelines a second request behind the first: it fails
 * closed, and no browser pipelines.) */
function secure(certificate, handler, ms) {
  const http = createPlain(handler);
  return guard(createTLS({ cert: certificate.cert, key: certificate.key, minVersion: 'TLSv1.2' }, socket => http.emit('connection', socket)), ms, http);
}
const listen = (server, port, host) => new Promise((ok, fail) => { server.once('error', fail); server.listen(port, host, () => { server.off('error', fail); ok(server.address().port); }); });

/** The remote doors `remoteArgs` asked for, on `wire` (tamberServer's
 * `handle`): `{info(), addPort({host, port, handler}), close()}`, or null
 * when neither was asked for. A trusted tunnel (ruling 3) is a plain
 * listener of its own on 127.0.0.1, since a tunnel's requests arrive on
 * loopback looking local: there they are `{remote: true, tunnel: true}`,
 * so only a paired token works through it, never the local one. `info()` is `{origin, fingerprint}` while the
 * TLS listener is on. `addPort` opens another TLS listener with the same
 * certificate, its requests given to `handler(req, res, url, access)` as
 * they come (C's canvas: its cookies are its own). */
export async function openRemote(wire, { remote, tunnel = null, plugin = false, hosts = [], relay = null }, { say = console.log, warn = console.error, interfaces, toolDir, timeoutMs = 10_000, connect, acme = null, acmeOptions = {} } = {}) {
  if (!remote && tunnel == null && !relay) return null;
  const servers = [];
  const close = () => Promise.all(servers.map(s => s.shut()));
  // A listener that cannot bind closes the ones already up, and says which.
  const bind = async (server, port, host, flag) => {
    servers.push(server);
    try { return await listen(server, port, host); } catch (e) {
      await close();
      throw Object.assign(new Error(`${flag} ${host.includes(':') ? `[${host}]` : host}:${port}: ${e.code ?? e.message}`), { code: e.code });
    }
  };
  let certificate = null, origin = null, names = [], tunnelOrigin = null;
  if (remote) {
    names = remoteNames(remote.host, interfaces);
    certificate = selfSignedCertificate(names);
    const server = secure(certificate, remoteHandler(wire, { tunnel: false, scheme: 'https', admits: at => names.includes(at.name), toolDir }), timeoutMs);
    const port = await bind(server, remote.port, remote.host, '--remote');
    const at = n => `https://${n.includes(':') ? `[${n}]` : n}:${port}`;
    const shown = ['0.0.0.0', '::'].includes(remote.host) ? names.filter(n => !LOOPBACK.has(n)) : [names.at(-1)];
    origin = at(shown[0] ?? '127.0.0.1');
    say(`  remote (TLS only), bound to ${remote.host.includes(':') ? `[${remote.host}]` : remote.host}:${port}: ${shown.map(at).join(', ') || origin}`);
    say(`  certificate SHA-256 ${certificate.fingerprint}`);
    say('  a remote client opens /__tamber/tool/ there, checks this fingerprint and pairs with a code; the app and the token page are not served there');
  }
  if (tunnel != null) {
    // On both loopbacks: a tunnel aimed at `localhost` may reach either,
    // and another process must not hold the one this does not.
    // Loopback names, and a tunnel's public names (`--allow-host`, `name`, or
    // `name:port`, a bare name 443), never loopback; a Host with no port is 443 too.
    const publics = hosts.map(h => { const m = /^(.*?)(?::(\d+))?$/.exec(h); return { name: m[1], port: m[2] ? String(Number(m[2])) : '443' }; }).filter(h => !LOOPBACK.has(h.name));
    const named = at => publics.some(h => h.name === at.name && h.port === (at.port || '443'));
    const handler = remoteHandler(wire, { tunnel: true, scheme: 'http', admits: at => LOOPBACK.has(at.name) || named(at), toolDir, plugin, publicName: n => publics.some(h => h.name === n) });
    const port = await bind(guard(createPlain(handler), timeoutMs), tunnel, '127.0.0.1', '--trust-tunnel');
    const six = guard(createPlain(handler), timeoutMs);
    try { await listen(six, port, '::1'); servers.push(six); } catch (e) {
      // (No IPv6 loopback on this machine: nothing to hold. Anything else fails.)
      if (!['EADDRNOTAVAIL', 'EAFNOSUPPORT'].includes(e.code)) { await close(); throw Object.assign(new Error(`--trust-tunnel [::1]:${port}: ${e.code ?? e.message}`), { code: e.code }); }
    }
    tunnelOrigin = `http://127.0.0.1:${port}`;
    warn(`TRUSTED TUNNEL ON: http://127.0.0.1:${port} takes a tunnel's requests (--trust-tunnel). Only paired tokens work through it, never this machine's local token; point only a tunnel you run at it.`);
  }
  // The relay (stage 10c): dialled out, its clients' TLS ended here on the
  // certificate for its names, routed as the TLS door's (the names the relay
  // serves this session by, or the developer's own). Every relayed request's
  // peer is the session's (its pairing limit, `relay:<name>`); the address the
  // relay reports is shown only.
  let relayed = null, connector = null, relayCanvas = () => {}, session = null;
  // (an issuance under way when the doors close: its records taken down before close resolves)
  let settled = () => Promise.resolve();
  const waiting = [];
  if (relay) {
    // The certificate the relayed TLS ends on: the developer's own, or one
    // got here by ACME (R2) once the relay names the session, kept and got
    // again before it ends (`held` swapped whole: a stream already open
    // keeps the one it began with).
    let held = relay.cert ? hold(relay.cert, relay.key) : null;
    let names = relay.names;
    // Each relayed stream's TLS ended by a socket of its own (so the address
    // the relay reports rides on the socket every request reads), under the
    // same count and deadlines as a listener's (`guard`).
    const http = createPlain(remoteHandler(wire, { tunnel: false, scheme: 'https', admits: at => !!names && at.name === names.tool && (!at.port || at.port === '443'), toolDir, plugin, peerOf: () => `relay:${names.tool}`, door: 'relay' }));
    const front = () => Object.assign(new EventEmitter(), { close: done => done() });
    const door = guard(front(), timeoutMs, http);
    servers.push(door);
    // The canvas's name (R1b): its own HTTP server and guard, given the
    // relay's canvas gateway once `relayCanvas` names its handler; until
    // then, and for any Host that is not the canvas's name, nothing.
    let canvasHandler = null;
    const canvasHttp = createPlain((req, res) => {
      if ((req.socket?.writableLength || 0) > 8 << 20) { req.socket.destroy(); return; }
      const at = hostOf(req, 'https'), url = at && names && at.name === names.canvas && (!at.port || at.port === '443') ? targetOf(req, 'https', at.host) : null;
      if (!url || !canvasHandler) return refuse(res, 404, 'not found');
      if (req.headers.origin !== undefined) {
        let same = false;
        try { const o = new URL(req.headers.origin); same = o.protocol === 'https:' && o.host === at.host; } catch { /* refused */ }
        if (!same) return refuse(res, 403, 'another origin');
      }
      return canvasHandler(req, res, url, { allowed: true, local: false, remote: true, tunnel: false, door: 'relay', peer: `relay:${names.tool}`, ...(req.socket?.relayed ? { reported: req.socket.relayed } : {}), port: 'canvas' });
    });
    const canvasDoor = guard(front(), timeoutMs, canvasHttp);
    servers.push(canvasDoor);
    // An issuance under way stops where it is when the connector ends (its records taken down still).
    const issuing = new AbortController();
    connector = connectRelay(relay, {
      warn,
      ...(connect ? { connect } : {}),
      // The relay's names only when this server's certificate covers them (never an origin the relay alone chose).
      // (and the session told at once, so the relay canvas is in place before a client greeted in the same write is routed)
      // (by ACME, names of one parent, which one wildcard of its own certifies)
      named: served => {
        // (by ACME: two names of one parent, the account's own under the relay's domain, `<account>.<relay>`
        // whole, or the developer's own flag's, which the greeting must be; never a parent the relay alone chose,
        // another account's or a domain of its liking)
        if (held ? !held.covers(served) : parentOf(served.tool) !== parentOf(served.canvas) || (!relay.names && parentOf(served.tool) !== `${relay.account}.${relay.host}`)) return false;
        names ??= served;
        session = { ...names };
        for (const f of waiting.splice(0)) f(session);
        if (!held) certify();
        return true;
      },
      ended: () => { relayed = null; clearInterval(renewal); renewal = null; issuing.abort(); },
      client: (sni, address, stream, settled) => {
        // The SNI decides the server: the tool's, or the canvas's once it has a gateway; nothing else.
        // (none until there is a certificate to end its TLS on)
        // (and none on a certificate that has ended: a renewal that keeps failing serves nothing)
        if (held && Date.now() >= held.until) { if (!expired) warn('tamber: the relay certificate has ended and could not be got again: no client is served until it is'); expired = true; return false; }
        const to = !names || !held ? null : sni === names.tool ? [door, http] : sni === names.canvas && canvasHandler ? [canvasDoor, canvasHttp] : null;
        if (!to) return false;
        const socket = new TLSSocket(stream, { isServer: true, secureContext: held.context });
        socket.relayed = address;
        socket.on('error', () => socket.destroy());
        // Settled once its own ClientHello named what the relay said (the
        // relay's word for the name checked against the client's), its
        // requests read only then; failed, if it closes first.
        socket.once('close', () => settled(false));
        socket.once('secure', () => {
          if (socket.servername !== sni || socket.destroyed || connector.closed) return socket.destroy();
          settled(true);
          to[1].emit('connection', socket);
        });
        to[0].emit('connection', socket);
        return true;
      },
    });
    relayCanvas = handler => { canvasHandler = handler; };
    // Shown once the session is named and its certificate held: the origin,
    // the certificate's fingerprint, and its key's SPKI pin (*1.15*), which
    // stays the same across renewals with the account's one key.
    let ready = false, caa = null, expired = false;
    const announce = () => {
      if (!ready || !held || connector.closed) return;
      const first = !relayed;
      relayed = { origin: `https://${names.tool}`, fingerprint: held.fingerprint, spki: held.spki, relay: true, ...(relay.hook ? { caa: caa === true } : {}) };
      if (!first) return;
      say(`  relay ${relay.host}: ${relayed.origin}, its clients' TLS ending here (certificate SHA-256 ${held.fingerprint}, key pin ${held.spki})`);
      say('  until the relay\'s domain is a public suffix, another account\'s names there are same-site to yours: nothing here trusts that');
    };
    connector.ready.then(() => { ready = true; announce(); }, () => {});
    // By ACME: the account's wildcard, its record put up by the relay (its
    // names) or by the developer's own command (their names); checked
    // again every day and got again 30 days before it ends.
    let renewal = null;
    const certify = () => {
      const wildcard = `*.${parentOf(names.tool)}`;
      const via = connect ? { connect } : {};
      const publish = relay.hook ? (r, v, signal) => dnsHook(relay.hook, r, v, { signal }) : (r, v, signal) => relayTxt(relay, r, v, { ...via, signal });
      const unpublish = relay.hook ? (r, v) => dnsHook(relay.hook, r, v, { clear: true }) : (r, v) => relayTxt(relay, r, v, { clear: true, ...via });
      // With names of their own, the CAA records that keep the relay's operator from getting one by another
      // method (TLS-ALPN-01 or HTTP-01 at the relay's own port): `issue` for each host and `issuewild` for the
      // wildcard (RFC 8659: neither covers the other's), said with the account and CA they name, and checked at
      // start and before each renewal (theme2's 09:57): not set, said plainly and shown beside the fingerprint
      // (`caa: false`), or, with --require-caa, refused before any order.
      const parent = parentOf(names.tool);
      const caaNames = [`*.${parent}`, names.tool, names.canvas];
      const resolvers = Object.fromEntries(['resolveCaa', 'resolveCname'].filter(k => acmeOptions[k]).map(k => [k, acmeOptions[k]]));
      const honouring = acmeOptions.caaHonouring ? { honouring: acmeOptions.caaHonouring } : {};
      // (each check's answer shown at once, beside the fingerprint, whether or not a certificate came with it; with
      // --require-caa, one that no longer pins stops the door)
      const answer = (pinned, why) => {
        caa = pinned;
        if (relayed) relayed = { ...relayed, caa: pinned };
        if (!pinned && relay.requireCaa) { if (held && !connector.closed) { warn(`tamber: ${why}: the relay door is closed`); connector.close(); } throw new Error(why); }
      };
      const checkCaa = async (uri, issuer) => {
        if (!relay.hook) return;
        if (!uri) {
          warn(`tamber: not zero-trust: the ACME account's URL is not kept in ${relay.dir}, so the CAA records can't be checked: remove its cert.pem to get one again`);
          return answer(false, '--require-caa: the ACME account\'s URL is not kept, so its CAA records can\'t be checked');
        }
        if (caa === null) {
          const rule = `"${issuer ?? '<your CA\'s CAA name>'}; validationmethods=dns-01; accounturi=${uri}"`;
          say(`  for names of your own, set these CAA records on ${parent}, so only this account, by DNS-01, can get a certificate for them: 0 issue ${rule} and 0 issuewild ${rule}; and point ${names.tool} and ${names.canvas} at the relay by A/AAAA records, never a CNAME (a CA reads a CNAME's target's CAA, which the relay writes)`);
        }
        const c = await caaGoverns(caaNames, uri, { ...resolvers, ...honouring, ...(acmeOptions.caaMs ? { ms: acmeOptions.caaMs } : {}) }).catch(() => ({ pinned: false, unpinned: null }));
        const why = c.unpinned?.cname ? ` (${c.unpinned.at} is a CNAME, to ${c.unpinned.cname.replace(/[^\x21-\x7e]/g, '?').slice(0, 100)})` : c.unpinned?.timeout ? ' (its CAA lookups did not answer in time)' : '';
        if (!c.pinned) warn(`tamber: not zero-trust: the relay could obtain a certificate for ${c.unpinned?.name ?? parent}${why} until the CAA records above govern it`);
        if (!c.pinned) answer(false, `--require-caa: the CAA that governs ${c.unpinned?.name ?? parent}${why} does not pin this account and DNS-01 at a CA known to honour that (${(acmeOptions.caaHonouring ?? CAA_HONOURING).join(', ')})`);
        else answer(true);
      };
      const account = (uri, issuer) => checkCaa(uri, issuer);
      // (a certificate kept from before is checked too: no order runs to check it there)
      const got = () => relayCertificate(relay.dir, [wildcard], { directory: relay.acme, publish, unpublish, warn, account, ...(acme ?? {}) }, { ...acmeOptions, signal: issuing.signal })
        .then(async got => { if (!got.renewed) await checkCaa(got.accountUri, got.issuer); return got; });
      const get = () => { const p = got(); const quiet = p.catch(() => {}); settled = () => quiet; return p; };
      // A certificate taken only while the connector runs, for these names; a process that renewed it
      // (another app on this account) changes it too: adopted by its fingerprint, not by who renewed it.
      const take = got => {
        if (connector.closed) return false;
        const next = hold(got.cert, got.key);
        if (!next.covers(names)) { warn('tamber: the relay certificate does not name this session\'s names'); connector.close(); return false; }
        const changed = !held || held.fingerprint !== next.fingerprint;
        held = next;
        announce();
        return changed;
      };
      get().then(got => {
        if (!take(got) || connector.closed) return;
        renewal = setInterval(() => get().then(g => { if (take(g)) say(`  relay certificate got again (SHA-256 ${g.fingerprint})`); }, e => { if (!issuing.signal.aborted) warn(`tamber: the relay certificate could not be got again: ${e.message}`); }), DAY);
        renewal.unref?.();
      }).catch(e => { if (issuing.signal.aborted) return; warn(`tamber: the relay certificate could not be got: ${e.message}`); connector.close(); });
    };
  }
  const shut = close;
  return {
    info: () => certificate && { origin, fingerprint: certificate.fingerprint },
    // What a local client shows beside a pairing code (pair-code's `remote`):
    // each open door, the TLS one with its fingerprint to check.
    list: () => [...(certificate ? [{ origin, fingerprint: certificate.fingerprint }] : []), ...(tunnelOrigin ? [{ origin: tunnelOrigin, tunnel: true }] : []), ...(relayed ? [relayed] : [])],
    /** The relay's session, once it has named it: `{origin, fingerprint, relay: true}`. */
    relay: () => relayed,
    /** `f({tool, canvas})` once the relay names this run's one session (at once if it has). */
    onRelay: f => { if (session) f(session); else if (relay) waiting.push(f); },
    /** The relay canvas's gateway handler (R1b): its name's requests go to it from now on. */
    relayCanvas: handler => relayCanvas(handler),
    async addPort({ host, port, handler }) {
      if (!certificate) throw new Error('addPort needs the TLS listener (--remote)');
      const server = secure(certificate, (req, res) => {
        if ((req.socket?.writableLength || 0) > 8 << 20) { req.socket.destroy(); return; } // (8 MiB unread: cut, as the main door does)
        // As the design server's door: one of the certificate's names, an origin-form target.
        const at = hostOf(req, 'https'), url = at && names.includes(at.name) ? targetOf(req, 'https', at.host) : null;
        if (!url) return refuse(res, 404, 'not found');
        if (req.headers.origin !== undefined) {
          let same = false;
          try { const o = new URL(req.headers.origin); same = o.protocol === 'https:' && o.host === at.host; } catch { /* refused */ }
          if (!same) return refuse(res, 403, 'another origin');
        }
        return handler(req, res, url, { allowed: true, local: false, remote: true, tunnel: false, door: 'tls', peer: req.socket?.remoteAddress ?? null, port: 'canvas' });
      }, timeoutMs);
      return bind(server, port, host, 'canvas port');
    },
    close: async () => { connector?.close(); await settled(); return shut(); },
  };
}
