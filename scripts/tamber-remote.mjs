// The Tamber wire's remote doors (LLP 1121 D13, LLP 1122 D12; stage 10 B).
// Off unless asked: `--remote <host:port>` opens a listener that speaks TLS
// only, with a certificate made for this run in memory (its SHA-256
// fingerprint printed, for the client to pin); `--trust-tunnel <port>` opens
// a plain listener on loopback for a tunnel the developer runs (stage 10 B2).
// On either, only the wire's paths (and the design tool's own files) are served, every request is told it is remote (never
// `access.local`, so the local token's routes stay closed), its peer is its
// socket's, no Cookie reaches the wire, and a page of another origin is
// refused. The main listener and its gate are untouched.
import { generateKeyPairSync, randomBytes, sign, X509Certificate } from 'node:crypto';
import { createServer as createPlain } from 'node:http';
import { createServer as createTLS } from 'node:tls';
import { isIP } from 'node:net';
import { networkInterfaces } from 'node:os';
import { serveToolFiles, TOOL_DIST } from './tamber-tool-files.mjs';

const DAY = 86_400_000;

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
 * `{remote: {host, port} | null, tunnel: port | null}`. A malformed value
 * throws, or with `exit` is said and exits 2. */
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
  return { remote, tunnel: t === null ? null : port(t, '--trust-tunnel') };
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
 * `127.0.0.1`), and its name without brackets, or null when it names none. */
function hostOf(req, scheme) {
  try {
    const url = new URL(`${scheme}://${String(req.headers.host ?? '')}/`);
    return url.host && url.pathname === '/' && !url.username && !url.password ? { host: url.host, name: url.hostname.replace(/^\[|\]$/g, '') } : null;
  } catch { return null; }
}
/** An origin-form target, on the admitted host, routed exactly as written:
 * no `//` anywhere, no backslash, no dot segment, no encoded dot, slash or
 * backslash, so the path checked is the path every handler reads (the
 * lead's 05:05Z note from C's round 1). Null when it is not one. */
function targetOf(req, scheme, host) {
  const target = req.url;
  if (!target.startsWith('/') || target.includes('//') || target.includes('\\') || /%(2e|2f|5c)/i.test(target)) return null;
  try {
    const url = new URL(target, `${scheme}://${host}`);
    return url.host === host && url.pathname + url.search === target ? url : null;
  } catch { return null; }
}

export function remoteHandler(wire, { tunnel, scheme, admits, toolDir = TOOL_DIST }) {
  return async (req, res) => {
    res.setHeader('cache-control', 'no-store');
    res.setHeader('x-content-type-options', 'nosniff');
    res.setHeader('referrer-policy', 'no-referrer');
    // A cookie is never a credential here, whoever set it (a canvas page can
    // set one this port receives): none reaches the wire, in any view of the
    // headers (`headersDistinct` is built from the raw ones when first read).
    delete req.headers.cookie;
    void req.headersDistinct; delete req.headersDistinct?.cookie;
    const raw = req.rawHeaders;
    for (let i = raw.length - 2; i >= 0; i -= 2) if (String(raw[i]).toLowerCase() === 'cookie') raw.splice(i, 2);
    const at = hostOf(req, scheme);
    if (!at || !admits(at.name)) return refuse(res, 404, 'not found');
    if (req.headers.origin !== undefined) {
      let same = false;
      try { const o = new URL(req.headers.origin); same = o.protocol === `${scheme}:` && o.host === at.host; } catch { /* refused */ }
      if (!same) return refuse(res, 403, 'another origin');
    }
    const url = targetOf(req, scheme, at.host);
    if (!url) return refuse(res, 404, 'not found');
    if (url.pathname === '/__tamber/tool' || url.pathname.startsWith('/__tamber/tool/')) {
      // The tool's own files send their origin, and only that, as a referrer
      // (the lead's 08:22Z ruling: one place, no per-form policy in the tool);
      // everything else here sends none.
      res.setHeader('referrer-policy', 'strict-origin');
      return serveToolFiles(req, res, url, toolDir, { 'content-security-policy': "frame-ancestors 'none'" });
    }
    if (!WIRE.has(url.pathname)) return refuse(res, 404, 'not found');
    const access = { allowed: true, local: false, remote: true, tunnel, peer: req.socket?.remoteAddress ?? null };
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
export async function openRemote(wire, { remote, tunnel = null }, { say = console.log, warn = console.error, interfaces, toolDir, timeoutMs = 10_000 } = {}) {
  if (!remote && tunnel == null) return null;
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
  let certificate = null, origin = null, names = [];
  if (remote) {
    names = remoteNames(remote.host, interfaces);
    certificate = selfSignedCertificate(names);
    const server = secure(certificate, remoteHandler(wire, { tunnel: false, scheme: 'https', admits: n => names.includes(n), toolDir }), timeoutMs);
    const port = await bind(server, remote.port, remote.host, '--remote');
    const at = n => `https://${n.includes(':') ? `[${n}]` : n}:${port}`;
    const shown = ['0.0.0.0', '::'].includes(remote.host) ? names.filter(n => !LOOPBACK.has(n)) : [names.at(-1)];
    origin = at(shown[0] ?? '127.0.0.1');
    say(`  remote (TLS only), bound to ${remote.host.includes(':') ? `[${remote.host}]` : remote.host}:${port}: ${shown.map(at).join(', ') || origin}`);
    say(`  certificate SHA-256 ${certificate.fingerprint}`);
    say('  a remote client pairs with a code and checks this fingerprint; nothing else is served there');
  }
  if (tunnel != null) {
    // On both loopbacks: a tunnel aimed at `localhost` may reach either,
    // and another process must not hold the one this does not.
    const handler = remoteHandler(wire, { tunnel: true, scheme: 'http', admits: n => LOOPBACK.has(n), toolDir });
    const port = await bind(guard(createPlain(handler), timeoutMs), tunnel, '127.0.0.1', '--trust-tunnel');
    const six = guard(createPlain(handler), timeoutMs);
    try { await listen(six, port, '::1'); servers.push(six); } catch (e) {
      // (No IPv6 loopback on this machine: nothing to hold. Anything else fails.)
      if (!['EADDRNOTAVAIL', 'EAFNOSUPPORT'].includes(e.code)) { await close(); throw Object.assign(new Error(`--trust-tunnel [::1]:${port}: ${e.code ?? e.message}`), { code: e.code }); }
    }
    warn(`TRUSTED TUNNEL ON: http://127.0.0.1:${port} takes a tunnel's requests (--trust-tunnel). Only paired tokens work through it, never this machine's local token; point only a tunnel you run at it.`);
  }
  return {
    info: () => certificate && { origin, fingerprint: certificate.fingerprint },
    async addPort({ host, port, handler }) {
      if (!certificate) throw new Error('addPort needs the TLS listener (--remote)');
      const server = secure(certificate, (req, res) => {
        // As the design server's door: one of the certificate's names, an origin-form target.
        const at = hostOf(req, 'https'), url = at && names.includes(at.name) ? targetOf(req, 'https', at.host) : null;
        if (!url) return refuse(res, 404, 'not found');
        if (req.headers.origin !== undefined) {
          let same = false;
          try { const o = new URL(req.headers.origin); same = o.protocol === 'https:' && o.host === at.host; } catch { /* refused */ }
          if (!same) return refuse(res, 403, 'another origin');
        }
        return handler(req, res, url, { allowed: true, local: false, remote: true, tunnel: false, peer: req.socket?.remoteAddress ?? null, port: 'canvas' });
      }, timeoutMs);
      return bind(server, port, host, 'canvas port');
    },
    close,
  };
}
