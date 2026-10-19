// The relay connector (LLP 1121 §5 stage 10c R1) against a relay stand-in:
// the design server dials out, the relay hands a client's stream down
// unopened, and the client's TLS ends in the remote door's listener, routed
// as that door's. The relay sees ciphertext only; one whose certificate
// fails WebPKI is never sent the token; a refusal stops the connector.
import { afterEach, expect, test } from 'bun:test';
import { EventEmitter } from 'node:events';
import { PassThrough } from 'node:stream';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { connect as connectTCP, createServer as createTCP } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { connect as connectTLS, createServer as createTLS } from 'node:tls';
import { pairing } from './tamber-pairing.mjs';
import { connectRelay, POOL, relayArgs } from './tamber-relay.mjs';
import { openRemote, remoteHandler, selfSignedCertificate } from './tamber-remote.mjs';
import { tamberServer } from './tamber-server.mjs';

const TOKEN = 'relay-token-0123456789abcdef';
const TOOL = 's1-tool.acct.relay.test', CANVAS = 's1-canvas.acct.relay.test';
const relayCert = selfSignedCertificate(['relay.test']), devCert = selfSignedCertificate([TOOL, CANVAS]);
const closing = [];
afterEach(async () => { for (const c of closing.splice(0)) await c(); });

/** A relay stand-in: its control port (TLS, its own certificate) takes
 * connectors, its public port clients, each handed to an idle connector
 * with `CLIENT <sni> <address>` (the SNI as the client sent it, read by the
 * test from its own record), every byte between them kept. */
async function fakeRelay({ refuse = null, greet = `READY ${TOOL} ${CANVAS}\n`, label = null, address = '203.0.113.9', drop = false, token = TOKEN } = {}) {
  const idle = [], ungreeted = [], seen = { hellos: [], bytes: [], dials: 0, carried: [] };
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
      if (greet === null) { ungreeted.push(conn); return; } // (a relay that greets nothing, until a client comes)
      conn.write(greet);
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
  expect(minted.remote).toEqual([{ origin: `https://${TOOL}`, fingerprint: devCert.fingerprint, relay: true }]);
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
