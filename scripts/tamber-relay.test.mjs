// The relay connector (LLP 1121 §5 stage 10c R1) against a relay stand-in:
// the design server dials out, the relay hands a client's stream down
// unopened, and the client's TLS ends in the remote door's listener, routed
// as that door's. The relay sees ciphertext only; one whose certificate
// fails WebPKI is never sent the token; a refusal stops the connector.
import { afterEach, expect, test } from 'bun:test';
import { createHash, X509Certificate } from 'node:crypto';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { connect as connectTCP, createServer as createTCP } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { connect as connectTLS, createServer as createTLS } from 'node:tls';
import { pairing } from './tamber-pairing.mjs';
import { connectRelay, POOL, relayArgs } from './tamber-relay.mjs';
import { openRemote, remoteHandler, selfSignedCertificate } from './tamber-remote.mjs';
import { fakeAcme } from './tamber-acme-stub.mjs';
import { relayTxt } from './tamber-relay.mjs';
import { tamberServer } from './tamber-server.mjs';
import { openRemoteCanvas } from './tamber-canvas.mjs';
import { createServer as createHTTP } from 'node:http';

const TOKEN = 'relay-token-0123456789abcdef';
const TOOL = 's1-tool.acct.relay.test', CANVAS = 's1-canvas.acct.relay.test';
const relayCert = selfSignedCertificate(['relay.test']), devCert = selfSignedCertificate([TOOL, CANVAS]);
// The session certificate's key pin, as pair-code shows it (*1.15*).
const devSpki = createHash('sha256').update(new X509Certificate(devCert.cert).publicKey.export({ type: 'spki', format: 'der' })).digest('base64');
const closing = [];
afterEach(async () => { for (const c of closing.splice(0)) await c(); });

/** A relay stand-in: its control port (TLS, its own certificate) takes
 * connectors, its public port clients, each handed to an idle connector
 * with `CLIENT <sni> <address>` (the SNI as the client sent it, read by the
 * test from its own record), every byte between them kept. */
async function fakeRelay({ refuse = null, greet = `READY ${TOOL} ${CANVAS}\n`, label = null, address = '203.0.113.9', drop = false, token = TOKEN } = {}) {
  const idle = [], ungreeted = [], seen = { hellos: [], bytes: [], dials: 0, greets: 0, carried: [], txt: new Map() };
  const control = createTLS({ cert: relayCert.cert, key: relayCert.key }, conn => {
    seen.dials++;
    let head = '';
    const onData = d => {
      head += d.toString('latin1');
      if (!head.includes('\r\n\r\n')) return;
      conn.off('data', onData);
      seen.hellos.push(head);
      if (refuse) { conn.end(`REFUSED ${refuse}\n`); return; }
      if (!head.startsWith('EXACT-RELAY/1 acct\r\n') || !head.includes(`authorization: Bearer ${token}\r\n`)) { conn.end('REFUSED no such account\n'); return; }
      // A TXT record for ACME (R2): only under the account's own names (R3's rule), kept in `seen.txt`.
      const txt = /\r\ntxt: (\S+) (\S+)\r\n/.exec(head), clear = /\r\ntxt-clear: (\S+) (\S+)\r\n/.exec(head);
      if (txt || clear) {
        const record = (txt ?? clear)[1];
        if (!record.endsWith('.acct.relay.test')) { conn.end('REFUSED not your account\'s name\n'); return; }
        // (a clear takes down only its own value: another order's stays)
        if (txt) seen.txt.set(record, [...(seen.txt.get(record) ?? []), txt[2]]);
        else { const left = (seen.txt.get(record) ?? []).filter(v => v !== clear[2]); if (left.length) seen.txt.set(record, left); else seen.txt.delete(record); }
        conn.end('TXT-OK\n');
        return;
      }
      if (greet === null) { ungreeted.push(conn); return; } // (a relay that greets nothing, until a client comes)
      // (a function of the order greeted in; it may answer later, with a promise)
      const g = typeof greet === 'function' ? greet(++seen.greets) : greet;
      if (typeof g === 'string') conn.write(g); else g.then(x => conn.write(x));
      // (a relay that hands each connection the tool's name and drops it, at once, every time)
      if (drop) { conn.write(`CLIENT ${TOOL} 203.0.113.9\n`); setImmediate(() => conn.destroy()); return; }
      idle.push(conn);
    };
    conn.on('data', onData);
    conn.on('error', () => {});
  });
  const pub = createTCP(client => {
    client.once('data', first => {
      // (an ungreeted connection is greeted and handed the client in one write)
      const fresh = !idle.length && ungreeted.shift(), conn = fresh || idle.shift();
      if (!conn) return client.destroy();
      seen.carried.push(conn);
      // (the SNI is the stand-in's to read; the test says which it sent)
      conn.write(Buffer.concat([Buffer.from(`${fresh ? `READY ${TOOL} ${CANVAS}\n` : ''}CLIENT ${label ?? sniOf(first)} ${address}\n`), first]));
      seen.bytes.push(first);
      client.on('data', d => { seen.bytes.push(d); conn.write(d); });
      conn.on('data', d => { seen.bytes.push(d); client.write(d); });
      client.on('close', () => conn.destroy());
      // (a reset, as a relay cutting a stream would: Bun's plain destroy here left the client's socket open)
      for (const e of ['close', 'end', 'error']) conn.on(e, () => setImmediate(() => client.resetAndDestroy()));
      client.on('error', () => {});
    });
  });
  await new Promise(ok => control.listen(0, '127.0.0.1', ok));
  await new Promise(ok => pub.listen(0, '127.0.0.1', ok));
  closing.push(() => { for (const c of idle) c.destroy(); control.close(); pub.close(); });
  return { control: control.address().port, pub: pub.address().port, seen, idle, refuseIdle: why => { for (const c of idle) c.write(`REFUSED ${why}\n`); } };
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
function ask(port, sni, { host = sni, path = '/__tamber', headers = '', body = '{}' } = {}) {
  return new Promise((ok, fail) => {
    const s = connectTLS({ host: '127.0.0.1', port, servername: sni, ca: devCert.cert }, () => {
      s.write(`POST ${path} HTTP/1.1\r\nHost: ${host}\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(body)}\r\n${headers}Connection: close\r\n\r\n${body}`);
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
  await until(() => relay.idle.length === POOL && doors.relay() !== null);
  expect(doors.list()).toEqual([{ origin: `https://${TOOL}`, fingerprint: devCert.fingerprint, spki: devSpki, relay: true }]);
  expect(said.some(l => l.includes('same-site'))).toBe(true);
  const secret = 'paired-token-that-the-relay-must-not-see';
  const reply = await ask(relay.pub, TOOL, { headers: `x-exact-tamber-token: ${secret}\r\n` });
  expect(reply).toContain('{"ok":true}');
  expect(w.got).toEqual([{ path: '/__tamber', access: { allowed: true, local: false, remote: true, tunnel: false, door: 'relay', peer: `relay:${TOOL}`, reported: '203.0.113.9' }, token: secret }]);
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
    chmodSync(files, 0o600);
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
    chmodSync(files, 0o644);
    expect(() => relayArgs(base, { env: { EXACT_RELAY_TOKEN: TOKEN }, home })).toThrow('--relay-key');
    chmodSync(files, 0o600);
    expect(() => relayArgs([...base, '--relay-name', 'tool.example.com'], { env: { EXACT_RELAY_TOKEN: TOKEN }, home })).toThrow('two names');
    expect(relayArgs([...base, '--relay-name', 'tool.example.com', 'canvas.example.com'], { env: { EXACT_RELAY_TOKEN: TOKEN }, home }).names).toEqual({ tool: 'tool.example.com', canvas: 'canvas.example.com' });
    // R2: no certificate of its own, one got by ACME (Let's Encrypt unless --relay-acme), kept under the account's directory.
    const env = { env: { EXACT_RELAY_TOKEN: TOKEN }, home }, bare = ['--relay', 'relay.test', '--relay-account', 'acct'];
    expect(relayArgs(bare, env)).toMatchObject({ acme: 'https://acme-v02.api.letsencrypt.org/directory', hook: null, dir: join(home, '.exact', 'relay', 'acct') });
    expect(relayArgs([...bare, '--relay-acme', 'https://acme.test/dir'], env).acme).toBe('https://acme.test/dir');
    expect(() => relayArgs([...bare, '--relay-acme', 'http://acme.test/dir'], env)).toThrow("ACME directory's https URL");
    expect(() => relayArgs([...bare, '--relay-cert', files], env)).toThrow('go together');
    // Names of your own: your DNS command, or your certificate; one parent for both.
    expect(() => relayArgs([...bare, '--relay-name', 'tool.example.com', 'canvas.example.com'], env)).toThrow('needs --relay-dns-hook');
    expect(relayArgs([...bare, '--relay-name', 'tool.example.com', 'canvas.example.com', '--relay-dns-hook', 'true'], env)).toMatchObject({ hook: 'true' });
    expect(() => relayArgs([...bare, '--relay-dns-hook', 'true'], env)).toThrow('--relay-dns-hook is for --relay-name');
    expect(() => relayArgs([...bare, '--relay-name', 'tool.example.com', 'canvas.other.org', '--relay-dns-hook', 'true'], env)).toThrow('one parent domain');
    expect(relayArgs([...bare, '--relay-name', 'tool.example.com', 'canvas.example.com', '--relay-dns-hook', 'true', '--require-caa'], env).requireCaa).toBe(true);
    expect(() => relayArgs([...bare, '--require-caa'], env)).toThrow("--require-caa is for --relay-dns-hook's names");
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

test('round 1: the token never goes to an unverified relay, even with NODE_TLS_REJECT_UNAUTHORIZED=0; a refusal never shows it', async () => {
  const relay = await fakeRelay(), warned = [];
  const was = process.env.NODE_TLS_REJECT_UNAUTHORIZED;
  process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';
  try {
    const c = connectRelay(relayOf(relay.control), { client: () => true, connect: toRelay(relay.control, false), warn: l => warned.push(l), pool: 1, backoffMs: 50 });
    await until(() => warned.length > 0);
    c.close();
    expect(relay.seen.hellos).toEqual([]);
  } finally { if (was === undefined) delete process.env.NODE_TLS_REJECT_UNAUTHORIZED; else process.env.NODE_TLS_REJECT_UNAUTHORIZED = was; }
  // A relay that answers with the token and a terminal escape: shown redacted, printable.
  const echo = await fakeRelay({ refuse: `bad token ${TOKEN}\x1b[2J` });
  const c = connectRelay(relayOf(echo.control), { client: () => true, connect: toRelay(echo.control), warn: l => warned.push(l), pool: 1 });
  const why = await c.ready.catch(e => e.message);
  expect(why).toBe('the relay refused: [withheld: it carries the token]');
  // Broken up by a character, cased otherwise: still withheld.
  const split = await fakeRelay({ refuse: `bad ${TOKEN.slice(0, 9)}-${TOKEN.slice(9).toUpperCase()}` });
  const c2 = connectRelay(relayOf(split.control), { client: () => true, connect: toRelay(split.control), warn: l => warned.push(l), pool: 1 });
  expect(await c2.ready.catch(e => e.message)).toBe('the relay refused: [withheld: it carries the token]');
  // A refusal without it: shown, printable.
  const plain = await fakeRelay({ refuse: 'no such account\x1b[2J' });
  const c3 = connectRelay(relayOf(plain.control), { client: () => true, connect: toRelay(plain.control), warn: l => warned.push(l), pool: 1 });
  expect(await c3.ready.catch(e => e.message)).toBe('the relay refused: no such account?[2J');
  expect(warned.some(l => l.includes(TOKEN) || l.includes('\x1b'))).toBe(false);
});

test('round 1: a relay handing down strangers costs it a doubling wait, never a dial each; a silent or overlong greeting is cut', async () => {
  const strangers = await fakeRelay({ label: 'other.acct.relay.test' });
  const doors = await openRemote(wire(), { relay: relayOf(strangers.control) }, { connect: toRelay(strangers.control), say: () => {}, warn: () => {} });
  closing.push(() => doors.close());
  await until(() => strangers.idle.length === POOL);
  const before = strangers.seen.dials;
  for (let i = 0; i < 20; i++) await ask(strangers.pub, TOOL).catch(() => '');
  await new Promise(ok => setTimeout(ok, 400));
  // (the first wait is 250 ms, then 500: a dial or two more in all, not twenty)
  expect(strangers.seen.dials - before).toBeLessThanOrEqual(POOL);
  // Greeted with nothing: cut at its deadline. Greeted past the line's length: cut, never ready.
  const silent = await fakeRelay({ greet: null });
  const quiet = connectRelay(relayOf(silent.control), { client: () => true, connect: toRelay(silent.control), warn: () => {}, pool: 1, greetMs: 100, backoffMs: 10_000 });
  await until(() => silent.seen.dials === 1);
  await until(() => quiet.live === 0, 2000);
  quiet.close();
  // (well-formed names, the line padded past its length: the cap, not the names, refuses it)
  const long = await fakeRelay({ greet: `READY ${TOOL} ${CANVAS} ${'x'.repeat(2000)}\n` });
  const over = connectRelay(relayOf(long.control), { client: () => true, connect: toRelay(long.control), warn: () => {}, pool: 1, backoffMs: 10_000 });
  await until(() => long.seen.hellos.length === 1);
  await until(() => over.live === 0, 2000);
  let readied = false;
  over.ready.then(() => { readied = true; }, () => {});
  await new Promise(ok => setTimeout(ok, 50));
  expect(readied).toBe(false);
  over.close();
});

test('round 1: the relay\'s word for a name is checked against the client\'s; its address shown only when it is one; a refusal after the greeting cuts every stream', async () => {
  // A relay that labels a canvas-name connection as the tool's: closed unanswered.
  const liar = await fakeRelay({ label: TOOL, address: '1.2.3.4\rINJECTED' }), w = wire();
  const doors = await openRemote(w, { relay: relayOf(liar.control) }, { connect: toRelay(liar.control), say: () => {}, warn: () => {} });
  closing.push(() => doors.close());
  await until(() => liar.idle.length === POOL);
  expect(await ask(liar.pub, CANVAS).catch(() => '')).toBe('');
  expect(w.got).toEqual([]);
  // Its own name, the address with a CR in it: answered, the address 'unknown'.
  expect(await ask(liar.pub, TOOL)).toContain('{"ok":true}');
  expect(w.got.map(g => g.access.reported)).toEqual(['unknown']);
  // Revoked at the relay after the greeting (told on an idle connection only): the connector
  // stops, and itself cuts the stream it carries.
  const held = new Promise(ok => { const s = connectTLS({ host: '127.0.0.1', port: liar.pub, servername: TOOL, ca: devCert.cert }, () => { s.write(`GET /__tamber/events HTTP/1.1\r\nHost: ${TOOL}\r\n\r\n`); }); s.on('close', ok); s.on('end', ok); s.on('error', ok); });
  await until(() => liar.seen.carried.length === 3 && liar.idle.length === POOL);
  const dials = liar.seen.dials;
  liar.refuseIdle('this token was revoked');
  await held;
  await new Promise(ok => setTimeout(ok, 400));
  expect(liar.seen.dials).toBe(dials);
});

test('round 1: names greeted with the first client in one write are known before it is routed', async () => {
  const quick = await fakeRelay({ greet: null }), w = wire();
  const doors = await openRemote(w, { relay: relayOf(quick.control) }, { connect: toRelay(quick.control), say: () => {}, warn: () => {} });
  closing.push(() => doors.close());
  await until(() => quick.seen.hellos.length >= 1);
  // The stand-in greets a connection and hands it a client in the same write: answered.
  expect(await ask(quick.pub, TOOL)).toContain('{"ok":true}');
  expect(w.got.length).toBe(1);
});

/** A stand-in for `contract tamber serve` that answers anything. */
function fakeServe() {
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
  child.kill = () => { child.exitCode = 0; child.emit('exit', 0); };
  let buf = '';
  child.stdin.on('data', chunk => {
    buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
    for (const line of lines) child.stdout.write(`${JSON.stringify({ id: JSON.parse(line).id, ok: { files: {} } })}\n`);
  });
  return child;
}

test('round 1: the real wire through the relay: a code names the relay\'s session, a client pairs through it and its token works; the local token is no one there; pairing is limited per session', async () => {
  const relay = await fakeRelay();
  let doors = null;
  const logged = [];
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: l => logged.push(l), remote: () => doors?.list() ?? [] });
  doors = await openRemote(w, { relay: relayOf(relay.control) }, { connect: toRelay(relay.control), say: () => {}, warn: () => {} });
  closing.push(async () => { await doors.close(); await w.close(); });
  await until(() => relay.idle.length === POOL);
  let token = null;
  await w.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
  let text = '';
  const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
  queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'propose', name: 'laptop' })));
  await w.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
  const minted = JSON.parse(text).ok;
  expect(minted.remote).toEqual([{ origin: `https://${TOOL}`, fingerprint: devCert.fingerprint, spki: devSpki, relay: true }]);
  // Through the relay, as the other machine would: pair, then hello with the token it got.
  const call = async (path, body, headers = '') => {
    const out = await ask(relay.pub, TOOL, { path, headers: `${headers}`, body });
    return { status: Number(out.split(' ')[1]), body: out.slice(out.indexOf('\r\n\r\n') + 4) };
  };
  const json = b => JSON.parse(b.slice(b.indexOf('{'), b.lastIndexOf('}') + 1));
  const paired = await call('/__tamber/pair', JSON.stringify({ code: minted.code, name: 'laptop' }));
  expect(paired.status).toBe(200);
  const { token: pt, scope } = json(paired.body).ok;
  expect(scope).toBe('propose');
  expect(logged.some(l => l.includes(`from relay:${TOOL} (203.0.113.9, as the relay reports it)`))).toBe(true);
  expect((await call('/__tamber', '{"method":"hello"}', `x-exact-tamber-token: ${pt}\r\n`)).status).toBe(200);
  // The local token: no one through the relay.
  const local = await call('/__tamber', '{"method":"hello"}', `x-exact-tamber-token: ${token}\r\n`);
  expect([local.status, local.body.includes(token)]).toEqual([404, false]); // (as on the TLS door)
  // The session's pairing bucket: 10 a minute (one taken above), then 429 with the relay's words.
  const tries = [];
  for (let i = 0; i < 10; i++) tries.push(await call('/__tamber/pair', '{"code":"AAAAA-AAAAA"}'));
  expect(tries.map(t => t.status)).toEqual([...Array(9).fill(403), 429]);
  expect(json(tries[9].body).error.message).toMatch(/^pairing is limited through the relay: try again in \d+ s$/);
  expect(logged.some(l => /^tamber: relayed pairing limited \(10 a minute\): try again in \d+ s$/.test(l))).toBe(true);
  // A paired client's long poll, held through the relay: open until the relay revokes the account
  // (told on an idle connection), then cut by the connector itself.
  // Revoked as a paired client (the local `revoke`): its pending long poll through the relay told 401.
  const poll = await longPoll(relay.pub, pt);
  await new Promise(ok => setTimeout(ok, 300));
  expect([poll.open, poll.text]).toEqual([true, '']); // (pending: nothing answered yet)
  const rv = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
  queueMicrotask(() => rv.end(JSON.stringify({ method: 'revoke', client: json(paired.body).ok.client })));
  await w.handle(rv, { writeHead() {}, setHeader() {}, write() {}, end() {} }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
  await until(() => poll.text.startsWith('HTTP/1.1 401'), 3000);
});

test('round 3: a pending long poll through the relay is cut by the connector when the relay revokes the account (told on an idle connection)', async () => {
  const relay = await fakeRelay();
  let doors = null;
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemote(w, { relay: relayOf(relay.control) }, { connect: toRelay(relay.control), say: () => {}, warn: () => {} });
  closing.push(async () => { await doors.close(); await w.close(); });
  await until(() => relay.idle.length === POOL);
  const pt = await pairThrough(w, relay.pub);
  const poll = await longPoll(relay.pub, pt);
  await new Promise(ok => setTimeout(ok, 300));
  expect([poll.open, poll.text]).toEqual([true, '']);
  relay.refuseIdle('this account was revoked');
  await poll.closed;
  expect(poll.text).toBe(''); // (cut, never answered)
});

test('round 3: closing the server ends a relayed client\'s long poll', async () => {
  const relay = await fakeRelay();
  let doors = null;
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemote(w, { relay: relayOf(relay.control) }, { connect: toRelay(relay.control), say: () => {}, warn: () => {} });
  closing.push(async () => { await w.close(); });
  await until(() => relay.idle.length === POOL);
  let token = null, text = '';
  await w.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
  const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
  queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'read' })));
  await w.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
  const out = await ask(relay.pub, TOOL, { path: '/__tamber/pair', body: JSON.stringify({ code: JSON.parse(text).ok.code }) });
  const pt = JSON.parse(out.slice(out.indexOf('{'), out.lastIndexOf('}') + 1)).ok.token;
  const poll = await longPoll(relay.pub, pt);
  await new Promise(ok => setTimeout(ok, 300));
  expect([poll.open, poll.text]).toEqual([true, '']);
  await doors.close();
  await poll.closed;
});

test("the events GET long poll through the relay: a paired read token gets an answer and its cursor (tamber/1.17)", async () => {
  const relay = await fakeRelay();
  let doors = null;
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemote(w, { relay: relayOf(relay.control) }, { connect: toRelay(relay.control), say: () => {}, warn: () => {} });
  closing.push(async () => { await doors.close(); await w.close(); });
  await until(() => relay.idle.length === POOL);
  const pt = await pairThrough(w, relay.pub);
  const out = await new Promise((ok, fail) => {
    const s = connectTLS({ host: '127.0.0.1', port: relay.pub, servername: TOOL, ca: devCert.cert }, () => s.write(`GET /__tamber/events?client=relayed HTTP/1.1\r\nHost: ${TOOL}\r\nx-exact-tamber-token: ${pt}\r\nConnection: close\r\n\r\n`));
    let got = '';
    s.on('data', d => { got += d; });
    for (const e of ['end', 'close']) s.on(e, () => ok(got));
    s.on('error', e => (got ? ok(got) : fail(e)));
  });
  expect(out.startsWith('HTTP/1.1 200')).toBe(true);
  expect(out.toLowerCase()).not.toContain('retry-after');
  const body = JSON.parse(out.slice(out.indexOf('{'), out.lastIndexOf('}') + 1));
  expect(body.cursor).toBe(`${body.seq}.${body.held}`);
});

/** A paired client's long poll through the relay, pending: its first ask for where to start, then a wait after it. */
async function longPoll(port, tok) {
  const first = await ask(port, TOOL, { path: '/__tamber/events', headers: `x-exact-tamber-token: ${tok}\r\n`, body: '{"wait":false}' });
  const { seq, epoch } = JSON.parse(first.slice(first.indexOf('{'), first.lastIndexOf('}') + 1));
  const body = JSON.stringify({ after: seq, epoch, wait: true });
  const p = { open: true, text: '' };
  const s = connectTLS({ host: '127.0.0.1', port, servername: TOOL, ca: devCert.cert }, () => s.write(`POST /__tamber/events HTTP/1.1\r\nHost: ${TOOL}\r\nContent-Type: application/json\r\nContent-Length: ${body.length}\r\nx-exact-tamber-token: ${tok}\r\n\r\n${body}`));
  s.on('data', d => { p.text += d; });
  p.closed = new Promise(ok => { for (const e of ['close', 'end', 'error']) s.on(e, () => { p.open = false; ok(); }); });
  return p;
}
/** Paired through the relay with a code minted locally: its token. */
async function pairThrough(w, port) {
  let token = null, text = '';
  await w.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
  const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
  queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'read' })));
  await w.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
  const out = await ask(port, TOOL, { path: '/__tamber/pair', body: JSON.stringify({ code: JSON.parse(text).ok.code }) });
  return JSON.parse(out.slice(out.indexOf('{'), out.lastIndexOf('}') + 1)).ok.token;
}

test('round 2: a relay that hands the tool a client and drops it costs a doubling wait; a line started after the greeting has a deadline', async () => {
  // Accepted and dropped before its TLS settles, every time: a round of the pool a wait, not a loop.
  const dropper = await fakeRelay({ drop: true });
  const c = connectRelay(relayOf(dropper.control), { client: () => true, connect: toRelay(dropper.control), warn: () => {}, backoffMs: 10_000 });
  closing.push(() => c.close());
  await new Promise(ok => setTimeout(ok, 900));
  // (the first pool, then one after 250 ms and one after 500 ms more: three rounds at most)
  expect(dropper.seen.dials).toBeLessThanOrEqual(3 * POOL);
  // A line started after the greeting and never finished: cut at its deadline.
  const partial = await fakeRelay({ greet: `READY ${TOOL} ${CANVAS}\nCLI` });
  const p = connectRelay(relayOf(partial.control), { client: () => true, connect: toRelay(partial.control), warn: () => {}, pool: 1, greetMs: 150, backoffMs: 10_000 });
  closing.push(() => p.close());
  await until(() => partial.seen.hellos.length === 1);
  await until(() => p.live === 0, 2000);
});

test('round 2: at most 64 clients carried; as they close the pool is dialled again, so a burst never leaves the door down', async () => {
  const busy = await fakeRelay(), held = [];
  const c = connectRelay(relayOf(busy.control), { client: (sni, address, stream, settled) => { held.push(stream); stream.on('error', () => {}); settled(true); return true; }, connect: toRelay(busy.control), warn: () => {}, backoffMs: 50 });
  closing.push(() => c.close());
  await until(() => busy.idle.length === POOL);
  // 70 clients, each handed down an idle connection as one comes (the stand-in names the tool).
  const clients = [];
  for (let i = 0; i < 70; i++) {
    await until(() => busy.idle.length > 0, 3000).catch(() => {});
    if (!busy.idle.length) break;
    clients.push(await new Promise(ok => { const x = connectTCP(busy.pub, '127.0.0.1', () => { x.write(Buffer.from(TOOL)); ok(x); }); x.on('error', () => {}); }));
    await until(() => busy.seen.carried.length === i + 1, 3000);
  }
  // (64 carried; the 65th and on handed down but not taken, each a wait)
  await until(() => c.carried === 64, 3000);
  expect(clients.length).toBeGreaterThan(64);
  // Every client gone: the pool whole again.
  for (const s of clients) s.destroy();
  for (const s of held) s.destroy();
  await until(() => c.carried === 0 && busy.idle.length === POOL, 5000);
});

test('round 3: dials are budgeted whatever the relay does; a relay word carrying the token, or names the certificate does not cover, are never shown', async () => {
  // A relay that hands the tool's name and drops it, its TLS "settled" each time: still no faster than the budget.
  const looper = await fakeRelay({ drop: true });
  const c = connectRelay(relayOf(looper.control), { client: (sni, address, stream, settled) => { settled(true); return true; }, connect: toRelay(looper.control), warn: () => {} });
  closing.push(() => c.close());
  await new Promise(ok => setTimeout(ok, 1000));
  // (a burst of 40 and 20 a second after it, never a loop)
  expect(looper.seen.dials).toBeLessThanOrEqual(POOL + 40 + 20 + 4);
  // The token in a greeted name: refused, never ready.
  const label = 'relaytoken0123456789abcdef'; // (a token that can be a host name's label)
  const hexed = await fakeRelay({ greet: `READY ${label}.acct.relay.test ${CANVAS}\n`, token: label });
  const h = connectRelay({ ...relayOf(hexed.control), token: label }, { client: () => true, connect: toRelay(hexed.control), warn: () => {}, pool: 1, backoffMs: 10_000 });
  await until(() => hexed.seen.hellos.length >= 1); // (greeted by the relay that knows it, not refused)
  await until(() => h.live === 0, 2000);
  let readied = false;
  h.ready.then(() => { readied = true; }, () => {});
  await new Promise(ok => setTimeout(ok, 50));
  expect(readied).toBe(false);
  h.close();
  // Names the certificate for them does not cover: stopped, the session never shown.
  const stranger = await fakeRelay({ greet: `READY s9-tool.other.relay.test s9-canvas.other.relay.test\n` }), warned = [];
  const doors = await openRemote(wire(), { relay: relayOf(stranger.control) }, { connect: toRelay(stranger.control), say: () => {}, warn: l => warned.push(l) });
  closing.push(() => doors.close());
  await until(() => warned.some(l => l.includes('does not cover')));
  expect(doors.list()).toEqual([]);
});

test('round 3: a line trickled a byte at a time is cut at its deadline from its first byte; an address carrying the token is shown as unknown', async () => {
  const trickle = await fakeRelay();
  const c = connectRelay(relayOf(trickle.control), { client: () => true, connect: toRelay(trickle.control), warn: () => {}, pool: 1, greetMs: 300, backoffMs: 10_000 });
  closing.push(() => c.close());
  await until(() => trickle.idle.length === 1);
  const conn = trickle.idle[0], t0 = Date.now();
  const drip = setInterval(() => { if (!conn.destroyed) conn.write('C'); }, 100);
  await until(() => conn.destroyed, 3000);
  clearInterval(drip);
  expect(Date.now() - t0).toBeLessThan(1000);
  // An address that is the token (hex, as an address might look): unknown.
  const hex = 'abcdef0123456789abcdef', w = wire();
  const sly = await fakeRelay({ address: hex, token: hex });
  const doors = await openRemote(w, { relay: { ...relayOf(sly.control), token: hex } }, { connect: toRelay(sly.control), say: () => {}, warn: () => {} });
  closing.push(() => doors.close());
  await until(() => sly.idle.length === POOL);
  expect(await ask(sly.pub, TOOL)).toContain('{"ok":true}');
  expect(w.got.map(g => g.access.reported)).toEqual(['unknown']);
});

test('theme2\'s R1 condition: no direct door\'s peer can begin relay: (a socket\'s address, an address or none)', async () => {
  const w = wire();
  const handler = remoteHandler(w, { tunnel: false, scheme: 'https', admits: () => true });
  for (const remoteAddress of ['relay:s1-tool.acct.relay.test', '203.0.113.5']) {
    const req = Object.assign(new PassThrough(), { method: 'POST', url: '/__tamber', headers: { host: 'x.test' }, rawHeaders: ['Host', 'x.test'], socket: { remoteAddress } });
    const res = { setHeader() {}, writeHead() {}, end() {}, headersSent: false };
    queueMicrotask(() => req.end('{}'));
    await handler(req, res);
  }
  expect(w.got.map(g => g.access.peer)).toEqual([null, '203.0.113.5']);
});

/** One HTTPS request over a TLS connection to `port` naming `sni`, as raw HTTP/1.1: {status, headers, text}. */
function https1(port, sni, { method = 'GET', path = '/', host = sni, headers = {}, body = '', ca = devCert.cert } = {}) {
  return new Promise((ok, fail) => {
    const s = connectTLS({ host: '127.0.0.1', port, ...(sni ? { servername: sni } : {}), ...(ca ? { ca } : { rejectUnauthorized: false }) }, () => {
      const h = Object.entries({ host, 'content-length': String(Buffer.byteLength(body)), connection: 'close', ...headers }).map(([k, v]) => `${k}: ${v}\r\n`).join('');
      s.write(`${method} ${path} HTTP/1.1\r\n${h}\r\n${body}`);
    });
    let raw = '';
    s.on('data', d => { raw += d; });
    const done = () => {
      if (!raw) return ok({ status: 0, headers: {}, text: '' });
      const [head, ...rest] = raw.split('\r\n\r\n');
      const lines = head.split('\r\n'), headers = {};
      for (const l of lines.slice(1)) { const i = l.indexOf(':'); const k = l.slice(0, i).toLowerCase(), v = l.slice(i + 1).trim(); headers[k] = k === 'set-cookie' ? [...(headers[k] ?? []), v] : v; }
      ok({ status: Number(lines[0].split(' ')[1]), headers, text: rest.join('\r\n\r\n') });
    };
    s.on('end', done); s.on('close', done); s.on('error', e => (raw ? done() : fail(e)));
  });
}

test('R1b: the canvas through the relay: its own gateway and origin pair, minted only through the relay\'s door, Host bound to the SNI, a revoke ending it on every gateway', async () => {
  const relay = await fakeRelay();
  // (the loop's /__dev/commands holds, as a long poll does)
  const upstream = new Set();
  const loop = createHTTP((req, res) => { if (req.url.startsWith('/__dev/commands')) { upstream.add(req); req.on('close', () => upstream.delete(req)); res.writeHead(200, { 'content-type': 'text/event-stream' }); res.write(': open\n\n'); return; } if (req.url === '/') { res.setHeader('content-type', 'text/html'); res.setHeader('content-security-policy', "frame-ancestors 'self' http://127.0.0.1:7740"); res.end(`<a href="http://127.0.0.1:${loop.address().port}/x">x</a>`); } else res.end('{}'); });
  await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
  const loopBase = `http://127.0.0.1:${loop.address().port}`;
  let doors = null;
  const said = [];
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemoteCanvas(w, { remote: { host: '127.0.0.1', port: 0 }, tunnel: null, relay: relayOf(relay.control) }, loopBase, { connect: toRelay(relay.control), say: l => said.push(l), warn: () => {} });
  closing.push(async () => { await doors.close(); await w.close(); loop.close(); });
  await until(() => relay.idle.length === POOL && said.some(l => l.includes('canvas through the relay')));
  // Paired through the relay, as in R1.
  let token = null, text = '';
  await w.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
  const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
  queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'read', name: 'far' })));
  await w.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
  const json = t => JSON.parse(t.slice(t.indexOf('{'), t.lastIndexOf('}') + 1));
  const paired = json((await https1(relay.pub, TOOL, { method: 'POST', path: '/__tamber/pair', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code: JSON.parse(text).ok.code, name: 'far' }) })).text).ok;
  const mint = async () => json((await https1(relay.pub, TOOL, { method: 'POST', path: '/__tamber', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': paired.token }, body: '{"method":"canvas"}' })).text).ok;
  // Minted through the relay's door: the relay's canvas origin.
  const minted = await mint();
  expect(minted.url).toBe(`https://${CANVAS}/canvas/${encodeURIComponent(paired.client)}/`);
  const path = new URL(minted.url).pathname;
  // Entered on the canvas's own origin; the tool's origin refused at the door.
  expect((await https1(relay.pub, CANVAS, { method: 'POST', path, headers: { origin: `https://${TOOL}`, 'content-type': 'application/json' }, body: JSON.stringify({ grant: minted.grant }) })).status).toBe(403);
  const entry = await https1(relay.pub, CANVAS, { method: 'POST', path, headers: { origin: `https://${CANVAS}`, 'content-type': 'application/json' }, body: JSON.stringify({ grant: minted.grant }) });
  expect(entry.status).toBe(204);
  const cookie = entry.headers['set-cookie'][0].split(';')[0];
  const page = await https1(relay.pub, CANVAS, { headers: { cookie } });
  expect(page.status).toBe(200);
  expect(page.text).toContain(`href="https://${CANVAS}/x"`);
  expect(page.headers['content-security-policy']).toContain(`frame-ancestors 'self' https://${TOOL}`);
  // No other origin may include its resources: same-origin only, even a same-site one sending the cookie.
  expect(page.headers['cross-origin-resource-policy']).toBe('same-origin');
  expect((await https1(relay.pub, CANVAS, { headers: { cookie, 'sec-fetch-site': 'same-site', 'sec-fetch-mode': 'no-cors', 'sec-fetch-dest': 'style' } })).status).toBe(404);
  expect((await https1(relay.pub, CANVAS, { headers: { cookie, 'sec-fetch-site': 'same-origin', 'sec-fetch-mode': 'no-cors' } })).status).toBe(200);
  expect((await https1(relay.pub, CANVAS, { headers: { cookie, 'sec-fetch-site': 'same-site', 'sec-fetch-mode': 'navigate' } })).status).toBe(200);
  // A form POST navigation from another origin passes the metadata check as a navigation, and dies on the Origin check.
  expect((await https1(relay.pub, CANVAS, { method: 'POST', path: '/__dev/report', headers: { cookie, origin: 'https://s9-canvas.other.relay.test', 'sec-fetch-site': 'same-site', 'sec-fetch-mode': 'navigate', 'content-type': 'application/x-www-form-urlencoded' }, body: 'a=1' })).status).toBe(403);
  // Host bound to the SNI, both ways: no fronting from one name into the other's server.
  expect((await https1(relay.pub, CANVAS, { host: TOOL, headers: { cookie } })).status).toBe(404);
  // (a page the canvas gateway would answer 200, asked through the tool's SNI with the canvas's Host)
  expect((await https1(relay.pub, TOOL, { host: CANVAS, headers: { cookie } })).status).toBe(404);
  // A grant from the relay's pair never opens the TLS door's canvas, nor one of its pair the relay's:
  // answered as a grant that is no grant there (an unknown one, asked on this gateway).
  const notEntered = (await https1(relay.pub, CANVAS, { method: 'POST', path, headers: { origin: `https://${CANVAS}`, 'content-type': 'application/json' }, body: JSON.stringify({ grant: 'x'.repeat(43) }) })).status;
  expect(notEntered).not.toBe(204);
  const tls = new URL(w.canvas.origin);
  const second = await mint();
  expect((await https1(Number(tls.port), null, { method: 'POST', path: new URL(second.url).pathname, host: tls.host, headers: { origin: tls.origin, 'content-type': 'application/json' }, body: JSON.stringify({ grant: second.grant }), ca: null })).status).toBe(notEntered);
  // Minted on the TLS door: its own canvas; that grant never opens the relay's.
  const door = new URL(doors.info().origin);
  const onTls = json((await https1(Number(door.port), null, { method: 'POST', path: '/__tamber', host: door.host, headers: { 'content-type': 'application/json', 'x-exact-tamber-token': paired.token }, body: '{"method":"canvas"}', ca: null })).text).ok;
  expect(onTls.url.startsWith(`${tls.origin}/canvas/`)).toBe(true);
  expect((await https1(relay.pub, CANVAS, { method: 'POST', path: new URL(onTls.url).pathname, headers: { origin: `https://${CANVAS}`, 'content-type': 'application/json' }, body: JSON.stringify({ grant: onTls.grant }) })).status).toBe(notEntered);
  const tlsEntry = await https1(Number(tls.port), null, { method: 'POST', path: new URL(onTls.url).pathname, host: tls.host, headers: { origin: tls.origin, 'content-type': 'application/json' }, body: JSON.stringify({ grant: onTls.grant }), ca: null });
  expect(tlsEntry.status).toBe(204);
  const tlsCookie = tlsEntry.headers['set-cookie'][0].split(';')[0];
  // Each session cookie opens its own gateway only: exchanged, refused; at home, answered.
  expect((await https1(relay.pub, CANVAS, { headers: { cookie: tlsCookie } })).status).toBe(404);
  expect((await https1(Number(tls.port), null, { host: tls.host, headers: { cookie }, ca: null })).status).toBe(404);
  expect((await https1(relay.pub, CANVAS, { headers: { cookie } })).status).toBe(200);
  expect((await https1(Number(tls.port), null, { host: tls.host, headers: { cookie: tlsCookie }, ca: null })).status).toBe(200);
  // A live stream on each canvas, open before the revoke.
  const live = (port, sni, host, c, ca) => {
    const p = { open: true, text: '' };
    const sock = connectTLS({ host: '127.0.0.1', port, ...(sni ? { servername: sni } : {}), ...(ca ? { ca } : { rejectUnauthorized: false }) }, () => sock.write(`GET /__dev/commands HTTP/1.1\r\nhost: ${host}\r\ncookie: ${c}\r\n\r\n`));
    sock.on('data', d => { p.text += d; });
    p.closed = new Promise(ok => { for (const e of ['close', 'end', 'error']) sock.on(e, () => { p.open = false; ok(); }); });
    return p;
  };
  const viaRelay = live(relay.pub, CANVAS, CANVAS, cookie, devCert.cert), viaTls = live(Number(tls.port), null, tls.host, tlsCookie, null);
  await until(() => viaRelay.text.includes(': open') && viaTls.text.includes(': open'), 3000);
  expect(upstream.size).toBe(2);
  // Revoked: both live streams cut, and both canvases refuse the next request.
  const rv = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
  queueMicrotask(() => rv.end(JSON.stringify({ method: 'revoke', client: paired.client })));
  await w.handle(rv, { writeHead() {}, setHeader() {}, write() {}, end() {} }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
  await Promise.all([viaRelay.closed, viaTls.closed]);
  await until(() => upstream.size === 0, 3000); // (and the loop's two requests let go of)
  expect((await https1(relay.pub, CANVAS, { headers: { cookie } })).status).toBe(404);
  expect((await https1(Number(tls.port), null, { host: tls.host, headers: { cookie: tlsCookie }, ca: null })).status).toBe(404);
});

// The ghost builds' follow-up (theme2's per-door test): a canvas through the relay stops being a build when it goes.
// The connector hands its TLS socket to the canvas's HTTP server, whose req 'close' never fires under Bun; the gateway
// lets go of the loop's request on res 'close'.
test('a canvas stream through the relay ends upstream when its client leaves', async () => {
  const relay = await fakeRelay();
  const upstream = new Set();
  const loop = createHTTP((req, res) => { if (req.url.startsWith('/__dev/page')) { upstream.add(req); req.on('close', () => upstream.delete(req)); res.writeHead(200, { 'content-type': 'text/event-stream' }); res.write(': open\n\n'); return; } res.end('{}'); });
  await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
  let doors = null;
  const said = [];
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemoteCanvas(w, { remote: { host: '127.0.0.1', port: 0 }, tunnel: null, relay: relayOf(relay.control) }, `http://127.0.0.1:${loop.address().port}`, { connect: toRelay(relay.control), say: l => said.push(l), warn: () => {} });
  closing.push(async () => { await doors.close(); await w.close(); loop.close(); });
  await until(() => relay.idle.length === POOL && said.some(l => l.includes('canvas through the relay')));
  let token = null, text = '';
  await w.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
  const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
  queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'read', name: 'far' })));
  await w.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
  const json = t => JSON.parse(t.slice(t.indexOf('{'), t.lastIndexOf('}') + 1));
  const paired = json((await https1(relay.pub, TOOL, { method: 'POST', path: '/__tamber/pair', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code: JSON.parse(text).ok.code, name: 'far' }) })).text).ok;
  const minted = json((await https1(relay.pub, TOOL, { method: 'POST', path: '/__tamber', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': paired.token }, body: '{"method":"canvas"}' })).text).ok;
  const entry = await https1(relay.pub, CANVAS, { method: 'POST', path: new URL(minted.url).pathname, headers: { origin: `https://${CANVAS}`, 'content-type': 'application/json' }, body: JSON.stringify({ grant: minted.grant }) });
  const cookie = entry.headers['set-cookie'][0].split(';')[0];
  let got = '';
  const sock = connectTLS({ host: '127.0.0.1', port: relay.pub, servername: CANVAS, ca: devCert.cert }, () => sock.write(`GET /__dev/page HTTP/1.1\r\nhost: ${CANVAS}\r\ncookie: ${cookie}\r\n\r\n`));
  sock.on('data', d => { got += d; });
  sock.on('error', () => {});
  await until(() => got.includes(': open'), 3000);
  expect(upstream.size).toBe(1);
  sock.destroy();
  await until(() => upstream.size === 0, 5000);
}, 30_000);

test('R1b: one mint limit a client across both doors\' gateways', async () => {
  const relay = await fakeRelay();
  const loop = createHTTP((req, res) => res.end('{}'));
  await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
  let doors = null;
  const said = [];
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemoteCanvas(w, { remote: { host: '127.0.0.1', port: 0 }, tunnel: null, relay: relayOf(relay.control) }, `http://127.0.0.1:${loop.address().port}`, { connect: toRelay(relay.control), say: l => said.push(l), warn: () => {} });
  closing.push(async () => { await doors.close(); await w.close(); loop.close(); });
  await until(() => relay.idle.length === POOL && said.some(l => l.includes('canvas through the relay')));
  const pt = await pairThrough(w, relay.pub);
  const door = new URL(doors.info().origin);
  const viaRelay = async () => (await https1(relay.pub, TOOL, { method: 'POST', path: '/__tamber', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': pt }, body: '{"method":"canvas"}' })).status;
  const viaTls = async () => (await https1(Number(door.port), null, { method: 'POST', path: '/__tamber', host: door.host, headers: { 'content-type': 'application/json', 'x-exact-tamber-token': pt }, body: '{"method":"canvas"}', ca: null })).status;
  const got = [];
  for (let i = 0; i < 3; i++) got.push(await viaRelay(), await viaTls());
  got.push(await viaRelay(), await viaTls());
  // (six a minute in all: three on each door, then 429 on both)
  expect(got).toEqual([200, 200, 200, 200, 200, 200, 429, 429]);
});

test('R1b: one session a run: a connection greeted with another session\'s names is not taken', async () => {
  // (the second greeting held until the first is read: the session-1 names are surely first)
  let first;
  const read = new Promise(ok => { first = ok; });
  const shifty = await fakeRelay({ greet: n => (n === 1 ? `READY ${TOOL} ${CANVAS}\n` : read.then(() => 'READY s2-tool.acct.relay.test s2-canvas.acct.relay.test\n')) });
  const c = connectRelay(relayOf(shifty.control), { client: () => true, connect: toRelay(shifty.control), warn: () => {}, pool: 2, backoffMs: 10_000 });
  closing.push(() => c.close());
  expect(await c.ready).toEqual({ tool: TOOL, canvas: CANVAS });
  first();
  await until(() => shifty.seen.dials >= 2);
  // (the second, greeted as another session, cut: only the first stays)
  await until(() => c.live === 1, 2000);
});

test('R1b round 2: a canvas client handed down in the same write as the greeting is routed to the relay canvas', async () => {
  const relay = await fakeRelay({ greet: null });
  const loop = createHTTP((req, res) => res.end('{}'));
  await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
  let doors = null;
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemoteCanvas(w, { remote: null, tunnel: null, relay: relayOf(relay.control) }, `http://127.0.0.1:${loop.address().port}`, { connect: toRelay(relay.control), say: () => {}, warn: () => {} });
  closing.push(async () => { await doors.close(); await w.close(); loop.close(); });
  await until(() => relay.seen.hellos.length >= 1);
  // (no session: the canvas answers its entry page, a 200, only once its gateway is in place)
  const entry = await https1(relay.pub, CANVAS, { path: '/canvas/x/' });
  expect(entry.status).toBe(200);
});

test('R1b round 3: every canvas answer is framed by its tool only, an SVG\'s too; the relay canvas is in place while the TLS canvas binds', async () => {
  const relay = await fakeRelay();
  const loop = createHTTP((req, res) => { if (req.url === '/x.svg') { res.setHeader('content-type', 'image/svg+xml'); res.end('<svg xmlns="http://www.w3.org/2000/svg"/>'); } else if (req.url === '/y.svg') { res.setHeader('content-type', 'image/svg+xml'); res.setHeader('content-security-policy', "default-src 'none'"); res.end('<svg xmlns="http://www.w3.org/2000/svg"/>'); } else res.end('{}'); });
  await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
  let doors = null;
  const said = [];
  const w = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemoteCanvas(w, { remote: { host: '127.0.0.1', port: 0 }, tunnel: null, relay: relayOf(relay.control) }, `http://127.0.0.1:${loop.address().port}`, { connect: toRelay(relay.control), say: l => said.push(l), warn: () => {} });
  closing.push(async () => { await doors.close(); await w.close(); loop.close(); });
  // (registered before the TLS canvas's port was awaited)
  await until(() => relay.idle.length === POOL && said.some(l => l.includes('canvas through the relay')));
  const pt = await pairThrough(w, relay.pub);
  const json = t => JSON.parse(t.slice(t.indexOf('{'), t.lastIndexOf('}') + 1));
  const minted = json((await https1(relay.pub, TOOL, { method: 'POST', path: '/__tamber', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': pt }, body: '{"method":"canvas"}' })).text).ok;
  const entry = await https1(relay.pub, CANVAS, { method: 'POST', path: new URL(minted.url).pathname, headers: { origin: `https://${CANVAS}`, 'content-type': 'application/json' }, body: JSON.stringify({ grant: minted.grant }) });
  const cookie = entry.headers['set-cookie'][0].split(';')[0];
  const svg = await https1(relay.pub, CANVAS, { path: '/x.svg', headers: { cookie } });
  expect([svg.status, svg.headers['content-security-policy']]).toEqual([200, `frame-ancestors 'self' https://${TOOL}`]);
  const kept = await https1(relay.pub, CANVAS, { path: '/y.svg', headers: { cookie } });
  expect(kept.headers['content-security-policy']).toBe(`default-src 'none'; frame-ancestors 'self' https://${TOOL}`);
});

test('R2: with no certificate of its own, the session\'s wildcard is got by ACME, its record put up by the relay and taken down, and clients served on it', async () => {
  const relay = await fakeRelay();
  const acme = fakeAcme({ dns: relay.seen.txt });
  const home = mkdtempSync(join(tmpdir(), 'exact-relay-acme-'));
  closing.push(() => rmSync(home, { recursive: true, force: true }));
  const said = [], warned = [], w = wire();
  const args = { host: 'relay.test', port: relay.control, account: 'acct', token: TOKEN, names: null, acme: acme.directory, hook: null, dir: join(home, '.exact', 'relay', 'acct') };
  const doors = await openRemote(w, { relay: args }, { connect: toRelay(relay.control), say: l => said.push(l), warn: l => warned.push(l), acme: { fetch: acme.fetch, wait: async () => {} } });
  closing.push(() => doors.close());
  await until(() => doors.relay() !== null, 10_000);
  // Taken down once issued; the certificate kept for the account.
  expect([...relay.seen.txt.keys()]).toEqual([]);
  expect(acme.orders.map(o => o.names)).toEqual([['*.acct.relay.test']]);
  const kept = new X509Certificate(readFileSync(join(args.dir, 'cert.pem'), 'utf8'));
  expect(doors.relay()).toEqual({ origin: `https://${TOOL}`, fingerprint: kept.fingerprint256, spki: createHash('sha256').update(kept.publicKey.export({ type: 'spki', format: 'der' })).digest('base64'), relay: true });
  expect(said.some(l => l.includes('key pin'))).toBe(true);
  // A client trusting the ACME CA (as WebPKI would a real one) is served on it.
  const reply = await new Promise((ok, fail) => {
    const s = connectTLS({ host: '127.0.0.1', port: relay.pub, servername: TOOL, ca: acme.caPem }, () => s.write(`POST /__tamber HTTP/1.1\r\nHost: ${TOOL}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}`));
    let out = ''; s.on('data', d => { out += d; }); s.on('end', () => ok(out)); s.on('error', fail);
  });
  expect(reply).toContain('{"ok":true}');
  expect(warned).toEqual([]);
});

test('R2: the relay refuses a TXT record outside the account\'s names; names of the developer\'s own go through their DNS command, never the relay', async () => {
  const relay = await fakeRelay();
  await expect(relayTxt({ host: 'relay.test', port: relay.control, account: 'acct', token: TOKEN }, '_acme-challenge.other.relay.test', 'A'.repeat(43), { connect: toRelay(relay.control) })).rejects.toThrow("the relay refused the TXT record: not your account's name");
  // A record or value of no ACME shape (a newline a server slipped in) never leaves: refused before the token goes.
  const hellos = relay.seen.hellos.length;
  await expect(relayTxt({ host: 'relay.test', port: relay.control, account: 'acct', token: TOKEN }, '_acme-challenge.acct.relay.test\r\ntxt: x y', 'A'.repeat(43), { connect: toRelay(relay.control) })).rejects.toThrow('no ACME TXT record of that shape');
  // An untrusted relay is never sent the token for a record either.
  await expect(relayTxt({ host: 'relay.test', port: relay.control, account: 'acct', token: TOKEN }, '_acme-challenge.acct.relay.test', 'A'.repeat(43), { connect: toRelay(relay.control, false) })).rejects.toThrow();
  expect(relay.seen.hellos.length).toBe(hellos);
  // BYO: the hook writes the record to a file the ACME stand-in reads as DNS.
  const home = mkdtempSync(join(tmpdir(), 'exact-relay-byo-'));
  closing.push(() => rmSync(home, { recursive: true, force: true }));
  const zone = join(home, 'zone');
  writeFileSync(zone, '');
  const dns = record => readFileSync(zone, 'utf8').split('\n').filter(l => l.startsWith(`${record} `)).map(l => l.split(' ')[1]);
  const acme = fakeAcme({ dns });
  const byo = { tool: 'tool.example.com', canvas: 'canvas.example.com' };
  const relayBYO = await fakeRelay({ greet: `READY ${byo.tool} ${byo.canvas}\n` });
  const hook = `if [ "$EXACT_ACME_ACTION" = publish ]; then echo "$EXACT_ACME_RECORD $EXACT_ACME_VALUE" >> ${zone}; fi`;
  const args = { host: 'relay.test', port: relayBYO.control, account: 'acct', token: TOKEN, names: byo, acme: acme.directory, hook, dir: join(home, '.exact', 'relay', 'acct') };
  const warned = [], said = [];
  const doors = await openRemote(wire(), { relay: args }, { connect: toRelay(relayBYO.control), say: l => said.push(l), warn: l => warned.push(l), acme: { fetch: acme.fetch, wait: async () => {} }, acmeOptions: { resolveCaa: async () => [], resolveCname: async () => [] } });
  closing.push(() => doors.close());
  await until(() => doors.relay() !== null, 10_000);
  expect(acme.orders.map(o => o.names)).toEqual([['*.example.com']]);
  // No CAA record pinning this account: said plainly, and shown beside the fingerprint (*1.15*).
  expect(doors.relay().caa).toBe(false);
  expect(warned.some(l => l.startsWith('tamber: not zero-trust: the relay could obtain a certificate for *.example.com'))).toBe(true);
  // The records to set: `issue` for the hosts and `issuewild` for the wildcard (neither covers the other), with the
  // CA's CAA name from its directory and this account; the hosts pointed by A/AAAA, never a CNAME.
  const rule = '"acme.test; validationmethods=dns-01; accounturi=https://acme.test/acct/1"';
  expect(said.find(l => l.includes('CAA records'))).toBe(`  for names of your own, set these CAA records on example.com, so only this account, by DNS-01, can get a certificate for them: 0 issue ${rule} and 0 issuewild ${rule}; and point tool.example.com and canvas.example.com at the relay by A/AAAA records, never a CNAME (a CA reads a CNAME's target's CAA, which the relay writes)`);
  expect([...relayBYO.seen.txt.keys()]).toEqual([]); // (nothing of the developer's domain through the relay)
  expect(readFileSync(zone, 'utf8')).toContain('_acme-challenge.example.com ');
});

test('R2 round 1: the DNS hook gets the record and its value to clear as to publish, never the relay\'s token, and a timeout stops all it started', async () => {
  const { dnsHook } = await import('./tamber-relay.mjs');
  const home = mkdtempSync(join(tmpdir(), 'exact-relay-hook-'));
  closing.push(() => rmSync(home, { recursive: true, force: true }));
  const log = join(home, 'log');
  const was = process.env.EXACT_RELAY_TOKEN;
  process.env.EXACT_RELAY_TOKEN = TOKEN;
  try {
    const hook = `echo "$EXACT_ACME_ACTION $EXACT_ACME_RECORD $EXACT_ACME_VALUE \${EXACT_RELAY_TOKEN:-none}" >> ${log}`;
    await dnsHook(hook, '_acme-challenge.example.com', 'B'.repeat(43));
    await dnsHook(hook, '_acme-challenge.example.com', 'B'.repeat(43), { clear: true });
    expect(readFileSync(log, 'utf8')).toBe(`publish _acme-challenge.example.com ${'B'.repeat(43)} none\nclear _acme-challenge.example.com ${'B'.repeat(43)} none\n`);
  } finally { if (was === undefined) delete process.env.EXACT_RELAY_TOKEN; else process.env.EXACT_RELAY_TOKEN = was; }
  // A record of no ACME shape: never run.
  await expect(dnsHook(`touch ${join(home, 'ran')}`, '_acme-challenge.example.com; touch x', 'B'.repeat(43))).rejects.toThrow('no ACME TXT record of that shape');
  // A hook that outlives its time: its whole group stopped (the sleep it started too).
  const marker = join(home, 'late');
  await expect(dnsHook(`(sleep 1; touch ${marker}) & sleep 5`, '_acme-challenge.example.com', 'B'.repeat(43), { ms: 200 })).rejects.toThrow('took more than');
  await new Promise(ok => setTimeout(ok, 1500));
  expect(() => readFileSync(marker)).toThrow();
});

test('R2 (theme2 09:57): a CAA record pinning this account and DNS-01 makes it zero-trust (caa: true); --require-caa without one refuses before any order', async () => {
  const home = mkdtempSync(join(tmpdir(), 'exact-relay-caa-'));
  closing.push(() => rmSync(home, { recursive: true, force: true }));
  const zone = join(home, 'zone');
  writeFileSync(zone, '');
  const dns = record => readFileSync(zone, 'utf8').split('\n').filter(l => l.startsWith(`${record} `)).map(l => l.split(' ')[1]);
  const byo = { tool: 'tool.example.com', canvas: 'canvas.example.com' };
  const hook = `if [ "$EXACT_ACME_ACTION" = publish ]; then echo "$EXACT_ACME_RECORD $EXACT_ACME_VALUE" >> ${zone}; fi`;
  const run = async (resolveCaa, requireCaa, tag, resolveCname = async () => []) => {
    const acme = fakeAcme({ dns });
    const relay = await fakeRelay({ greet: `READY ${byo.tool} ${byo.canvas}\n` });
    const warned = [];
    const args = { host: 'relay.test', port: relay.control, account: 'acct', token: TOKEN, names: byo, acme: acme.directory, hook, requireCaa, dir: join(home, tag) };
    const doors = await openRemote(wire(), { relay: args }, { connect: toRelay(relay.control), say: () => {}, warn: l => warned.push(l), acme: { fetch: acme.fetch, wait: async () => {} }, acmeOptions: { resolveCaa, resolveCname, caaHonouring: ['acme.test'], caaMs: 300 } });
    closing.push(() => doors.close());
    return { acme, doors, warned };
  };
  // Pinned, the hosts by `issue` and the wildcard by `issuewild` (the account the stand-in gives first is /acct/1):
  const rule = 'acme.test; validationmethods=dns-01; accounturi=https://acme.test/acct/1';
  const pinned = async d => (d === 'example.com' ? [{ critical: 0, issue: rule }, { critical: 0, issuewild: rule }] : []);
  const ok = await run(pinned, false, 'a');
  await until(() => ok.doors.relay() !== null, 10_000);
  expect(ok.doors.relay().caa).toBe(true);
  expect(ok.warned).toEqual([]);
  // `issuewild` alone (round 2): a host's certificate is still open to the relay (RFC 8659 §4.3).
  const wildOnly = await run(async d => (d === 'example.com' ? [{ critical: 0, issuewild: rule }] : []), false, 'c');
  await until(() => wildOnly.doors.relay() !== null, 10_000);
  expect(wildOnly.doors.relay().caa).toBe(false);
  expect(wildOnly.warned).toEqual(['tamber: not zero-trust: the relay could obtain a certificate for tool.example.com until the CAA records above govern it']);
  // Pinned, but the tool's name a CNAME (the relay writes its target's CAA): not pinned.
  const aliased = await run(pinned, false, 'd', async d => (d === 'tool.example.com' ? ['edge.relay.test'] : []));
  await until(() => aliased.doors.relay() !== null, 10_000);
  expect(aliased.doors.relay().caa).toBe(false);
  expect(aliased.warned).toEqual(['tamber: not zero-trust: the relay could obtain a certificate for tool.example.com (tool.example.com is a CNAME, to edge.relay.test) until the CAA records above govern it']);
  // Required and missing: refused before any order, the relay door closed.
  const refused = await run(async () => [], true, 'b');
  await until(() => refused.warned.some(l => l.includes('--require-caa')), 10_000);
  expect(refused.acme.orders.length).toBe(0);
  expect(refused.doors.relay()).toBeNull();
  // Pinned at a CA not known to honour accounturi (RFC 8657 §5.2): not pinned.
  const unknownCa = await run(async d => (d === 'example.com' ? [{ issue: rule.replace('acme.test', 'other-ca.test') }, { issuewild: rule.replace('acme.test', 'other-ca.test') }] : []), false, 'e');
  await until(() => unknownCa.doors.relay() !== null, 10_000);
  expect(unknownCa.doors.relay().caa).toBe(false);
  // A resolver that never answers (theme2's 14:20): not pinned within the deadline, and refused under --require-caa.
  const hung = await run(() => new Promise(() => {}), false, 'f');
  await until(() => hung.doors.relay() !== null, 10_000);
  expect(hung.doors.relay().caa).toBe(false);
  expect(hung.warned).toEqual(['tamber: not zero-trust: the relay could obtain a certificate for *.example.com (its CAA lookups did not answer in time) until the CAA records above govern it']);
  const hungRequired = await run(() => new Promise(() => {}), true, 'g');
  await until(() => hungRequired.warned.some(l => l.includes('--require-caa')), 10_000);
  expect(hungRequired.acme.orders.length).toBe(0);
  // A kept certificate whose account URL is not kept (round 3): with --require-caa, refused, never served unchecked.
  const { unlinkSync } = await import('node:fs');
  unlinkSync(join(home, 'a', 'account-uri'));
  const lost = await run(pinned, true, 'a');
  await until(() => lost.warned.some(l => l.includes('--require-caa')), 10_000);
  expect(lost.warned[0]).toContain('the ACME account\'s URL is not kept');
  expect(lost.acme.orders.length).toBe(0);
  expect(lost.doors.relay()).toBeNull();
});

test('R2 (theme2 09:57): the CAA that governs a name is the nearest set up the tree; every rule must pin, `;` forbids', async () => {
  const { caaPinned } = await import('./tamber-acme.mjs');
  const uri = 'https://acme.test/acct/1', ok = `letsencrypt.org; validationmethods=dns-01; accounturi=${uri}`;
  const zoneOf = recs => async d => recs[d] ?? [];
  // (no CNAME anywhere, unless said)
  const zone = recs => zoneOf(recs), none = async () => [];
  expect(await caaPinned('*.a.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issuewild: ok }] }) })).toEqual({ pinned: true, at: 'example.com' });
  // The nearest set wins: a.example.com's own, unpinned, though example.com is pinned.
  expect((await caaPinned('*.a.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'a.example.com': [{ issue: 'letsencrypt.org' }], 'example.com': [{ issuewild: ok }] }) })).pinned).toBe(false);
  // Every rule must pin: one open issuer is a way in.
  expect((await caaPinned('*.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issuewild: ok }, { issuewild: 'pki.goog' }] }) })).pinned).toBe(false);
  // Another account, or another method, is not this one.
  expect((await caaPinned('*.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issuewild: ok.replace('acct/1', 'acct/2') }] }) })).pinned).toBe(false);
  expect((await caaPinned('*.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issuewild: ok.replace('dns-01', 'tls-alpn-01') }] }) })).pinned).toBe(false);
  // No issuewild: the issue rules govern a wildcard; `;` forbids, which pins nothing open.
  expect((await caaPinned('*.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issue: ok }, { issue: ';' }] }) })).pinned).toBe(true);
  // Read as Boulder reads them (round 3): a tag in another case, or said twice, pins nothing.
  expect((await caaPinned('*.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issuewild: `letsencrypt.org; AccountURI=${uri}; ValidationMethods=dns-01` }] }) })).pinned).toBe(false);
  expect((await caaPinned('*.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issuewild: `${ok}; accounturi=https://acme.test/acct/2` }] }) })).pinned).toBe(false);
  // At a CA not known to honour the parameters: nothing pinned.
  expect((await caaPinned('*.example.com', uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issuewild: ok.replace('letsencrypt.org', 'pki.goog') }] }) })).pinned).toBe(false);
  // A CNAME on the way up: never pinned, whatever its target says.
  expect(await caaPinned('*.a.example.com', uri, { resolveCaa: zone({ 'example.com': [{ issuewild: ok }] }), resolveCname: async d => (d === 'a.example.com' ? ['x.relay.test'] : []) })).toEqual({ pinned: false, at: 'a.example.com', cname: 'x.relay.test' });
  // A wildcard's `issuewild` covers no host: the host is governed by `issue` alone.
  const { caaGoverns } = await import('./tamber-acme.mjs');
  expect(await caaGoverns(['*.example.com', 'tool.example.com'], uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issuewild: ok }] }) })).toEqual({ pinned: false, unpinned: { name: 'tool.example.com', pinned: false, at: 'example.com' } });
  expect((await caaGoverns(['*.example.com', 'tool.example.com'], uri, { resolveCname: none, resolveCaa: zone({ 'example.com': [{ issue: ok }, { issuewild: ok }] }) })).pinned).toBe(true);
  // None at all, up to the top: not pinned.
  expect(await caaPinned('*.example.com', uri, { resolveCname: none, resolveCaa: zone({}) })).toEqual({ pinned: false, at: null });
});

test('R2 round 2: a TXT answer is one line, read once: what the relay sends after it is never read, and the connection is gone; a line past its length fails at once', async () => {
  const closed = [];
  const chatty = createTLS({ cert: relayCert.cert, key: relayCert.key }, conn => {
    conn.on('error', () => {});
    conn.once('data', () => {
      conn.write('TXT-OK\n');
      // (a relay that goes on sending, as much as it likes)
      const flood = setInterval(() => { if (!conn.destroyed) conn.write('x'.repeat(64 * 1024)); }, 5);
      conn.on('close', () => { clearInterval(flood); closed.push('ok'); });
    });
  });
  await new Promise(ok => chatty.listen(0, '127.0.0.1', ok));
  closing.push(() => chatty.close());
  const port = chatty.address().port;
  await relayTxt({ host: 'relay.test', port, account: 'acct', token: TOKEN }, '_acme-challenge.acct.relay.test', 'A'.repeat(43), { connect: toRelay(port) });
  await until(() => closed.length === 1, 5000);
  const endless = createTLS({ cert: relayCert.cert, key: relayCert.key }, conn => { conn.on('error', () => {}); conn.once('data', () => conn.write('y'.repeat(4096))); });
  await new Promise(ok => endless.listen(0, '127.0.0.1', ok));
  closing.push(() => endless.close());
  const at = Date.now();
  await expect(relayTxt({ host: 'relay.test', port: endless.address().port, account: 'acct', token: TOKEN }, '_acme-challenge.acct.relay.test', 'A'.repeat(43), { connect: toRelay(endless.address().port) })).rejects.toThrow('with no line');
  expect(Date.now() - at).toBeLessThan(5000);
});

test('R2 round 2: by ACME on the relay\'s names, a session under another domain (the account\'s label, a domain the relay chose) is refused before any order', async () => {
  const relay = await fakeRelay({ greet: 'READY s1-tool.acct.evil.test s1-canvas.acct.evil.test\n' });
  const acme = fakeAcme({ dns: relay.seen.txt });
  const home = mkdtempSync(join(tmpdir(), 'exact-relay-suffix-'));
  closing.push(() => rmSync(home, { recursive: true, force: true }));
  const warned = [];
  const args = { host: 'relay.test', port: relay.control, account: 'acct', token: TOKEN, names: null, acme: acme.directory, hook: null, dir: join(home, 'acct') };
  const doors = await openRemote(wire(), { relay: args }, { connect: toRelay(relay.control), say: () => {}, warn: l => warned.push(l), acme: { fetch: acme.fetch, wait: async () => {} } });
  closing.push(() => doors.close());
  await until(() => warned.some(l => l.includes('does not cover')), 10_000);
  expect(acme.orders.length).toBe(0);
  expect(relay.seen.txt.size).toBe(0);
  expect(doors.relay()).toBeNull();
});

test('R2 round 2: closed while its certificate is got, a run stops there: no record put up after, nothing kept, its lock let go; another run on the account adopts the kept certificate with no order', async () => {
  const home = mkdtempSync(join(tmpdir(), 'exact-relay-cancel-'));
  closing.push(() => rmSync(home, { recursive: true, force: true }));
  const zone = join(home, 'zone'), log = join(home, 'hook-log');
  writeFileSync(zone, '');
  const dns = record => readFileSync(zone, 'utf8').split('\n').filter(l => l.startsWith(`${record} `)).map(l => l.split(' ')[1]);
  const byo = { tool: 'tool.example.com', canvas: 'canvas.example.com' };
  // (a slow DNS: each publish takes a second)
  const hook = `echo "$EXACT_ACME_ACTION" >> ${log}; if [ "$EXACT_ACME_ACTION" = publish ]; then sleep 1; echo "$EXACT_ACME_RECORD $EXACT_ACME_VALUE" >> ${zone}; fi`;
  const acme = fakeAcme({ dns });
  const dir = join(home, 'acct');
  const open = async () => {
    const relay = await fakeRelay({ greet: `READY ${byo.tool} ${byo.canvas}\n` });
    const args = { host: 'relay.test', port: relay.control, account: 'acct', token: TOKEN, names: byo, acme: acme.directory, hook, dir };
    const doors = await openRemote(wire(), { relay: args }, { connect: toRelay(relay.control), say: () => {}, warn: () => {}, acme: { fetch: acme.fetch, wait: async () => {} }, acmeOptions: { resolveCaa: async () => [], resolveCname: async () => [] } });
    closing.push(() => doors.close());
    return doors;
  };
  const first = await open();
  await until(() => { try { return readFileSync(log, 'utf8').includes('publish'); } catch { return false; } }, 10_000);
  await first.close();
  // (close resolved only once the record was taken down: the publish under way stopped, nothing more asked of the
  // server, nothing kept, the lock gone)
  expect(readFileSync(log, 'utf8')).toBe('publish\nclear\n');
  expect(acme.orders[0].status).toBe('pending');
  const { existsSync } = await import('node:fs');
  expect([existsSync(join(dir, 'cert.pem')), existsSync(join(dir, 'lock'))]).toEqual([false, false]);
  // Two runs on one account in turn: the second adopts the first's certificate, by its fingerprint, with no order.
  const a = await open();
  await until(() => a.relay() !== null, 15_000);
  const orders = acme.orders.length;
  const b = await open();
  await until(() => b.relay() !== null, 15_000);
  expect(acme.orders.length).toBe(orders);
  expect(b.relay().fingerprint).toBe(a.relay().fingerprint);
}, 60_000);
