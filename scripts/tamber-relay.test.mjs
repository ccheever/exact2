// The relay connector (LLP 1121 §5 stage 10c R1) against a relay stand-in:
// the design server dials out, the relay hands a client's stream down
// unopened, and the client's TLS ends in the remote door's listener, routed
// as that door's. The relay sees ciphertext only; one whose certificate
// fails WebPKI is never sent the token; a refusal stops the connector.
import { afterEach, expect, test } from 'bun:test';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer as createTCP } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { connect as connectTLS, createServer as createTLS } from 'node:tls';
import { pairing } from './tamber-pairing.mjs';
import { connectRelay, POOL, relayArgs } from './tamber-relay.mjs';
import { openRemote, selfSignedCertificate } from './tamber-remote.mjs';

const TOKEN = 'relay-token-0123456789abcdef';
const TOOL = 's1-tool.acct.relay.test', CANVAS = 's1-canvas.acct.relay.test';
const relayCert = selfSignedCertificate(['relay.test']), devCert = selfSignedCertificate([TOOL, CANVAS]);
const closing = [];
afterEach(async () => { for (const c of closing.splice(0)) await c(); });

/** A relay stand-in: its control port (TLS, its own certificate) takes
 * connectors, its public port clients, each handed to an idle connector
 * with `CLIENT <sni> <address>` (the SNI as the client sent it, read by the
 * test from its own record), every byte between them kept. */
async function fakeRelay({ refuse = null } = {}) {
  const idle = [], seen = { hellos: [], bytes: [], dials: 0 };
  const control = createTLS({ cert: relayCert.cert, key: relayCert.key }, conn => {
    seen.dials++;
    let head = '';
    const onData = d => {
      head += d.toString('latin1');
      if (!head.includes('\r\n\r\n')) return;
      conn.off('data', onData);
      seen.hellos.push(head);
      if (refuse) { conn.end(`REFUSED ${refuse}\n`); return; }
      if (!head.startsWith('EXACT-RELAY/1 acct\r\n') || !head.includes(`authorization: Bearer ${TOKEN}\r\n`)) { conn.end('REFUSED no such account\n'); return; }
      conn.write(`READY ${TOOL} ${CANVAS}\n`);
      idle.push(conn);
    };
    conn.on('data', onData);
    conn.on('error', () => {});
  });
  const pub = createTCP(client => {
    client.once('data', first => {
      const conn = idle.shift();
      if (!conn) return client.destroy();
      // (the SNI is the stand-in's to read; the test says which it sent)
      conn.write(`CLIENT ${client.sni ?? sniOf(first)} 203.0.113.9\n`);
      conn.write(first);
      seen.bytes.push(first);
      client.on('data', d => { seen.bytes.push(d); conn.write(d); });
      conn.on('data', d => { seen.bytes.push(d); client.write(d); });
      client.on('close', () => conn.destroy());
      conn.on('close', () => client.destroy());
      client.on('error', () => {});
    });
  });
  await new Promise(ok => control.listen(0, '127.0.0.1', ok));
  await new Promise(ok => pub.listen(0, '127.0.0.1', ok));
  closing.push(() => { for (const c of idle) c.destroy(); control.close(); pub.close(); });
  return { control: control.address().port, pub: pub.address().port, seen, idle };
}
/** The SNI host name in a ClientHello, or '' (the relay's one read). */
function sniOf(buf) {
  const s = buf.toString('latin1');
  for (const name of [TOOL, CANVAS, 'other.acct.relay.test']) if (s.includes(name)) return name;
  return '';
}
/** `connect` aimed at the stand-in, which a real relay's name would reach;
 * `trust` its certificate, as WebPKI would a real one. */
const toRelay = (port, trust = true) => opts => connectTLS({ ...opts, host: '127.0.0.1', port, ...(trust ? { ca: relayCert.cert } : {}) });
/** A client's HTTPS request through the relay's public port to `sni`. */
function ask(port, sni, { host = sni, path = '/__tamber', headers = '' } = {}) {
  return new Promise((ok, fail) => {
    const s = connectTLS({ host: '127.0.0.1', port, servername: sni, ca: devCert.cert }, () => {
      s.write(`POST ${path} HTTP/1.1\r\nHost: ${host}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n${headers}Connection: close\r\n\r\n{}`);
    });
    let out = '';
    s.on('data', d => { out += d; });
    s.on('end', () => ok(out));
    s.on('close', () => ok(out));
    s.on('error', e => (out ? ok(out) : fail(e)));
  });
}
/** The design wire, stood in for: each request's access, answered. */
const wire = () => {
  const got = [];
  return { got, handle: async (req, res, url, access) => { got.push({ path: url.pathname, access, token: req.headers['x-exact-tamber-token'] }); res.end('{"ok":true}'); return true; } };
};
const relayOf = port => ({ host: 'relay.test', port, account: 'acct', token: TOKEN, names: null, cert: devCert.cert, key: devCert.key });
const until = async (f, ms = 5000) => { const t = Date.now(); while (!f()) { if (Date.now() - t > ms) throw new Error('timed out'); await new Promise(ok => setTimeout(ok, 20)); } };

test('a relayed client reaches the wire as a remote request, its peer the relay\'s session; the relay carries ciphertext only', async () => {
  const relay = await fakeRelay(), w = wire(), said = [];
  const doors = await openRemote(w, { relay: relayOf(relay.control) }, { connect: toRelay(relay.control), say: l => said.push(l), warn: () => {} });
  closing.push(() => doors.close());
  await until(() => relay.idle.length === POOL);
  expect(doors.list()).toEqual([{ origin: `https://${TOOL}`, fingerprint: devCert.fingerprint, relay: true }]);
  expect(said.some(l => l.includes('same-site'))).toBe(true);
  const secret = 'paired-token-that-the-relay-must-not-see';
  const reply = await ask(relay.pub, TOOL, { headers: `x-exact-tamber-token: ${secret}\r\n` });
  expect(reply).toContain('{"ok":true}');
  expect(w.got).toEqual([{ path: '/__tamber', access: { allowed: true, local: false, remote: true, tunnel: false, peer: `relay:${TOOL}`, reported: '203.0.113.9' }, token: secret }]);
  // What passed through the relay: no token, no request line, no answer.
  const all = Buffer.concat(relay.seen.bytes).toString('latin1');
  expect([all.includes(secret), all.includes('POST /__tamber'), all.includes('{"ok":true}')]).toEqual([false, false, false]);
  // The connection handed down is replaced: the pool is whole again.
  await until(() => relay.idle.length === POOL);
});

test('a relayed client is routed as the TLS door\'s: another Host is refused, another name never reaches the listener', async () => {
  const relay = await fakeRelay(), w = wire();
  const doors = await openRemote(w, { relay: relayOf(relay.control) }, { connect: toRelay(relay.control), say: () => {}, warn: () => {} });
  closing.push(() => doors.close());
  await until(() => relay.idle.length === POOL);
  expect(await ask(relay.pub, TOOL, { host: 'other.acct.relay.test' })).toContain('404');
  expect(await ask(relay.pub, TOOL, { path: '/app.html' })).toContain('404');
  // (the canvas's name, until the canvas comes through the relay, and a stranger's: closed unanswered)
  expect(await ask(relay.pub, CANVAS).catch(() => '')).toBe('');
  expect(w.got).toEqual([]);
});

test('a relay whose certificate fails is never sent the token; a refusal stops the connector, never retried', async () => {
  const relay = await fakeRelay(), warned = [];
  const untrusted = connectRelay(relayOf(relay.control), { client: () => {}, connect: toRelay(relay.control, false), warn: l => warned.push(l), pool: 1, backoffMs: 50 });
  await until(() => warned.length > 0);
  untrusted.close();
  expect(relay.seen.hellos).toEqual([]);
  // Refused: one dial a connection, then nothing more.
  const refusing = await fakeRelay({ refuse: 'this token was revoked' });
  const stopped = connectRelay(relayOf(refusing.control), { client: () => {}, connect: toRelay(refusing.control), warn: l => warned.push(l), pool: 1, backoffMs: 50 });
  await expect(stopped.ready).rejects.toThrow('the relay refused: this token was revoked');
  await new Promise(ok => setTimeout(ok, 300));
  expect(refusing.seen.dials).toBe(1);
  expect(warned.some(l => l.includes(TOKEN))).toBe(false);
});

test('the token is read from the environment or a file only its owner may read; the names of your own domain are two', () => {
  const home = mkdtempSync(join(tmpdir(), 'exact-relay-home-'));
  try {
    const files = join(home, 'pem');
    writeFileSync(files, devCert.cert);
    const base = ['--relay', 'relay.test', '--relay-account', 'acct', '--relay-cert', files, '--relay-key', files];
    expect(relayArgs(['--remote', '0.0.0.0:0'])).toBeNull();
    expect(relayArgs(base, { env: { EXACT_RELAY_TOKEN: TOKEN }, home }).token).toBe(TOKEN);
    expect(() => relayArgs(base, { env: {}, home })).toThrow('no token for acct');
    const dir = join(home, '.exact', 'relay', 'acct');
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, 'token'), `${TOKEN}\n`);
    chmodSync(join(dir, 'token'), 0o644);
    expect(() => relayArgs(base, { env: {}, home })).toThrow('chmod 600');
    chmodSync(join(dir, 'token'), 0o600);
    expect(relayArgs(base, { env: {}, home })).toMatchObject({ host: 'relay.test', port: 443, account: 'acct', token: TOKEN, names: null });
    expect(() => relayArgs(['--relay', 'relay.test'], { env: {}, home })).toThrow('--relay-account');
    expect(() => relayArgs([...base, '--relay-name', 'tool.example.com'], { env: { EXACT_RELAY_TOKEN: TOKEN }, home })).toThrow('two names');
    expect(relayArgs([...base, '--relay-name', 'tool.example.com', 'canvas.example.com'], { env: { EXACT_RELAY_TOKEN: TOKEN }, home }).names).toEqual({ tool: 'tool.example.com', canvas: 'canvas.example.com' });
  } finally { rmSync(home, { recursive: true, force: true }); }
});

test('a relay session pairs at most 10 times a minute, its own bucket: the direct doors keep theirs', () => {
  let now = 0;
  const pairs = pairing({ clock: () => now });
  const tries = peer => Array.from({ length: 12 }, () => pairs.admit(peer)?.status ?? 200);
  expect(tries(`relay:${TOOL}`)).toEqual([...Array(10).fill(200), 429, 429]);
  expect(pairs.admit(`relay:${TOOL}`).error.message).toMatch(/^pairing is limited through the relay: try again in \d+ s$/);
  expect(tries('203.0.113.7').slice(0, 6)).toEqual([200, 200, 200, 200, 200, 429]);
  now += 60_000;
  expect(pairs.admit(`relay:${TOOL}`)).toBeNull();
});
