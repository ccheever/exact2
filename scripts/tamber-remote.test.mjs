// Stage 10 B: the wire's remote doors (scripts/tamber-remote.mjs). Every
// listener here is on loopback and closed after.
import { expect, test } from 'bun:test';
import { X509Certificate } from 'node:crypto';
import { request as plainRequest } from 'node:http';
import { request as secureRequest } from 'node:https';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { EventEmitter } from 'node:events';
import { connect } from 'node:net';
import { PassThrough } from 'node:stream';
import { derInteger, openRemote, remoteArgs, selfSignedCertificate } from './tamber-remote.mjs';
import { tamberServer } from './tamber-server.mjs';
import { replaceBuild } from '../host/web/serve.mjs';

/** A wire that records what it was asked and answers its own paths. */
function recordingWire() {
  const seen = [];
  return {
    seen,
    async handle(req, res, url, access) {
      seen.push({ path: url.pathname, access, cookie: req.headers.cookie, token: req.headers['x-exact-tamber-token'] });
      seen.at(-1).anyCookie = req.rawHeaders.some(h => /^cookie$/i.test(h)) || req.headersDistinct?.cookie !== undefined;
      if (url.pathname === '/__tamber/token') return false;
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ ok: url.pathname }));
      return true;
    },
  };
}

/** What a plaintext connection gets back, and whether it closed. */
function rawTCP(port, text) {
  return new Promise(ok => {
    const chunks = [], socket = connect(port, '127.0.0.1', () => socket.write(text));
    const done = closed => ok({ bytes: Buffer.concat(chunks), closed });
    socket.on('data', d => chunks.push(d));
    socket.on('close', () => done(true));
    socket.on('error', () => {});
    setTimeout(() => { socket.destroy(); done(false); }, 3000);
  });
}
/** A request written by hand over TLS (headers as given), its answer's text. */
async function rawTLS(port, text) {
  const { connect: tlsConnect } = await import('node:tls');
  return new Promise(ok => {
    let out = '';
    const socket = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => socket.write(text));
    socket.on('data', d => (out += d));
    socket.on('close', () => ok(out));
    socket.on('error', () => ok(out));
  });
}

function send(secure, port, { method = 'GET', path = '/', headers = {}, body } = {}) {
  return new Promise(ok => {
    const req = (secure ? secureRequest : plainRequest)({ host: '127.0.0.1', port, method, path, headers: { host: `127.0.0.1:${port}`, ...headers }, rejectUnauthorized: false }, res => {
      const fingerprint = res.socket.getPeerCertificate?.()?.fingerprint256;
      let text = '';
      res.on('data', d => (text += d));
      res.on('end', () => ok({ status: res.statusCode, headers: res.headers, text, fingerprint }));
    });
    req.on('error', e => ok({ error: e.code ?? e.message }));
    req.end(body);
  });
}

test('the certificate is made for this run, parses, verifies and names its hosts', () => {
  const a = selfSignedCertificate(['localhost', '127.0.0.1', '192.168.1.20']);
  const b = selfSignedCertificate(['localhost']);
  const x = new X509Certificate(a.cert);
  expect(x.verify(x.publicKey)).toBe(true);
  expect(x.subjectAltName).toBe('DNS:localhost, IP Address:127.0.0.1, IP Address:192.168.1.20');
  expect(x.fingerprint256).toBe(a.fingerprint);
  expect(a.fingerprint).not.toBe(b.fingerprint);
  expect(new Date(x.validTo) - Date.now()).toBeGreaterThan(6 * 86_400_000);
  expect(a.key).toContain('PRIVATE KEY');
});

test('the DER integers are minimal, and every certificate made parses', () => {
  expect(derInteger(Buffer.from([0, 0, 5]))).toEqual(Buffer.from([2, 1, 5]));
  expect(derInteger(Buffer.from([0x80]))).toEqual(Buffer.from([2, 2, 0, 0x80]));
  expect(derInteger(Buffer.from([0, 0x80]))).toEqual(Buffer.from([2, 2, 0, 0x80]));
  expect(derInteger(Buffer.from([0]))).toEqual(Buffer.from([2, 1, 0]));
  for (let i = 0; i < 300; i++) new X509Certificate(selfSignedCertificate(['localhost']).cert);
});

test('an IPv6 host is in the certificate', () => {
  const x = new X509Certificate(selfSignedCertificate(['localhost', '::1', 'fe80::1:2']).cert);
  expect(x.checkIP('::1')).toBe('::1');
  expect(x.checkIP('fe80::1:2')).toBe('fe80::1:2');
});

test('the flags: host:port and a tunnel port, refused when malformed', () => {
  expect(remoteArgs([])).toEqual({ remote: null, tunnel: null, plugin: false, hosts: [], relay: null });
  expect(remoteArgs(['--remote', '0.0.0.0:7443', '--trust-tunnel', '7444', '--allow-plugin-origin', '--allow-host', 'Tamber.Example.com'])).toEqual({ remote: { host: '0.0.0.0', port: 7443 }, tunnel: 7444, plugin: true, hosts: ['tamber.example.com'], relay: null });
  expect(() => remoteArgs(['--trust-tunnel', '70000'])).toThrow(/port/);
  expect(() => remoteArgs(['--remote', 'https://x:1'])).toThrow(/host:port/);
  expect(() => remoteArgs(['--remote', 'x:70000'])).toThrow(/port/);
});

test('nothing opens unless asked', async () => {
  expect(await openRemote(recordingWire(), { remote: null, tunnel: null })).toBe(null);
});

test('the remote listener speaks TLS only and serves only the wire, as a remote request', async () => {
  const wire = recordingWire(), said = [];
  const remote = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: l => said.push(l) });
  try {
    const { origin, fingerprint } = remote.info();
    const port = Number(new URL(origin).port);
    expect(said.join('\n')).toContain(fingerprint);
    // Plain HTTP, the pairing endpoint included: no protocol bytes back, the
    // connection closed, the wire never asked.
    const plain = await rawTCP(port, `POST /__tamber/pair HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Length: 12\r\n\r\n{"code":"x"}`);
    expect(plain.closed).toBe(true);
    expect(plain.bytes.length).toBe(0);
    expect(wire.seen.length).toBe(0);
    // TLS: the wire's paths, told remote, the socket's peer, no cookie.
    const paired = await send(true, port, { method: 'POST', path: '/__tamber/pair', headers: { cookie: 'x-exact-tamber-token=abc', 'x-forwarded-for': '203.0.113.9' }, body: '{}' });
    expect(paired.status).toBe(200);
    expect(paired.fingerprint).toBe(fingerprint);
    expect(paired.headers['cache-control']).toBe('no-store');
    expect(wire.seen.at(-1)).toEqual({ path: '/__tamber/pair', access: { allowed: true, local: false, remote: true, tunnel: false, door: 'tls', peer: '127.0.0.1' }, cookie: undefined, token: undefined, anyCookie: false });
    // Two Cookie headers, in two cases: none in any view of the headers.
    await rawTLS(port, 'POST /__tamber HTTP/1.1\r\nHost: 127.0.0.1:' + port + '\r\nCookie: a=1\r\ncOOkie: b=2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n');
    expect(wire.seen.at(-1)).toMatchObject({ path: '/__tamber', cookie: undefined, anyCookie: false });
    for (const path of ['/__tamber/subscribe', '/__tamber/events', '/__tamber']) expect((await send(true, port, { path })).status).toBe(200);
    // Everything else is not there: the token, the tool, the app, the dev endpoints, the installer.
    const before = wire.seen.length;
    for (const path of ['/', '/index.html', '/__dev', '/__dev/page', '/install', '/app.wasm', '//__tamber', '//foreign/__tamber', '///foreign/__tamber', '/\\foreign/__tamber', '/__tamber/../__dev', '/x/../__tamber', '/__tamber/./events', '/__tamber//events', '/__tamber/%2e%2e/__dev', '/%5f_tamber', '/__tamber%2fevents', '/__tamber/events#x'])
      expect((await send(true, port, { path })).status).toBe(404);
    expect(wire.seen.length).toBe(before);
    expect((await send(true, port, { path: '/__tamber/token' })).status).toBe(404);
    // Another origin's page (the canvas's, a rebound name) is refused; a name not its own is not there.
    expect((await send(true, port, { method: 'POST', path: '/__tamber', headers: { origin: `https://127.0.0.1:${port + 1}` } })).status).toBe(403);
    expect((await send(true, port, { method: 'POST', path: '/__tamber', headers: { origin: `http://127.0.0.1:${port}` } })).status).toBe(403);
    expect((await send(true, port, { method: 'POST', path: '/__tamber', headers: { origin: `https://127.0.0.1:${port}` } })).status).toBe(200);
    expect((await send(true, port, { path: '/__tamber', headers: { host: `evil.example:${port}` } })).status).toBe(404);
  } finally { await remote.close(); }
});

test('another TLS port shares the certificate and hands its requests over as they come', async () => {
  const remote = await openRemote(recordingWire(), { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  try {
    let got;
    const port = await remote.addPort({ host: '127.0.0.1', port: 0, handler: (req, res, url, access) => { got = { path: url.pathname, cookie: req.headers.cookie, access }; res.end('canvas'); } });
    const r = await send(true, port, { path: '/page', headers: { cookie: '__Host-c=1' } });
    expect(r.text).toBe('canvas');
    expect(r.fingerprint).toBe(remote.info().fingerprint);
    expect(got).toEqual({ path: '/page', cookie: '__Host-c=1', access: { allowed: true, local: false, remote: true, tunnel: false, door: 'tls', peer: '127.0.0.1', port: 'canvas' } });
    got = null;
    expect((await send(true, port, { path: '/page', headers: { host: `evil.example:${port}` } })).status).toBe(404);
    expect((await send(true, port, { path: '//evil.example/page' })).status).toBe(404);
    expect(got).toBe(null);
  } finally { await remote.close(); }
});

test('the remote port serves the design tool\'s own files, static GETs only, and frames them nowhere', async () => {
  const tool = mkdtempSync(join(tmpdir(), 'exact-remote-tool-'));
  writeFileSync(join(tool, 'index.html'), '<base href="/"><p>tool</p>');
  writeFileSync(join(tool, 'app.js'), 'export {}');
  mkdirSync(join(tool, '.gen'));
  writeFileSync(join(tool, '.gen', 'secret'), 'no');
  const remote = await openRemote(recordingWire(), { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {}, warn: () => {}, toolDir: tool });
  try {
    const port = Number(new URL(remote.info().origin).port);
    const page = await send(true, port, { path: '/__tamber/tool/' });
    expect(page.status).toBe(200);
    expect(page.text).toContain('<base href="/__tamber/tool/">');
    expect(page.headers['content-security-policy']).toBe("frame-ancestors 'none'");
    expect(page.headers['referrer-policy']).toBe('strict-origin');
    expect((await send(true, port, { path: '/__tamber/tool/app.js' })).headers['referrer-policy']).toBe('strict-origin');
    expect((await send(true, port, { method: 'POST', path: '/__tamber' })).headers['referrer-policy']).toBe('no-referrer');
    expect((await send(true, port, { path: '/__tamber/tool/app.js' })).status).toBe(200);
    expect((await send(true, port, { path: '/__tamber/tool/.gen/secret' })).status).toBe(404);
    expect((await send(true, port, { path: '/__tamber/tool/../../etc/passwd' })).status).toBe(404);
    expect((await send(true, port, { method: 'POST', path: '/__tamber/tool/' })).status).toBe(404);
  } finally { await remote.close(); rmSync(tool, { recursive: true, force: true }); }
});

test('the TLS door and the tunnel serve a tool page\'s files from the build it booted, under the same rules', async () => {
  const root = mkdtempSync(join(tmpdir(), 'exact-remote-tool-gen-')), tool = join(root, 'tool');
  const build = (k) => { const dir = join(root, `stage-${k}`); mkdirSync(dir); writeFileSync(join(dir, 'index.html'), `<base href="/"><p>tool ${k}</p>`);
    writeFileSync(join(dir, 'app.js'), `tool ${k}`); writeFileSync(join(dir, '.exact-build.json'), JSON.stringify({ exactBuild: 1, target: 'js', files: [{ name: 'app.js', sha256: String(k) }] })); return dir; };
  replaceBuild(build(1), tool);
  const warned = [];
  const remote = await openRemote(recordingWire(), { remote: { host: '127.0.0.1', port: 0 }, tunnel: 0 }, { say: () => {}, warn: (l) => warned.push(l), toolDir: tool });
  try {
    const tls = Number(new URL(remote.info().origin).port), tunnel = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    for (const [secure, port] of [[true, tls], [false, tunnel]]) {
      const page = await send(secure, port, { path: '/__tamber/tool/' });
      const base = /<base href="([^"]+)">/.exec(page.text)[1];
      expect(base).toMatch(/^\/__tamber\/tool\/\.exact\/gen\/[0-9a-f]{32}\/$/);
      expect(page.headers['set-cookie']).toBeUndefined();
      expect((await send(secure, port, { path: base + 'app.js' })).text).toContain('tool');
      // The same rules: no dot segment, no way out, no encoded slash, an id's form only; a document under it is not served.
      for (const path of [`${base}../../app.js`, `${base}%2e%2e/app.js`, `${base}a%2fb`, '/__tamber/tool/.exact/gen/xyz/app.js', `${base}index.html`, `${base}.exact-build.json`])
        expect([path, (await send(secure, port, { path })).status]).toEqual([path, 404]);
    }
    // Another build put in place: a page booted on the first still gets the first's files; a new page boots the second.
    const first = /<base href="([^"]+)">/.exec((await send(true, tls, { path: '/__tamber/tool/' })).text)[1];
    replaceBuild(build(2), tool);
    expect((await send(true, tls, { path: first + 'app.js' })).text).toBe('tool 1');
    expect((await send(false, tunnel, { path: first + 'app.js' })).text).toBe('tool 1');
    expect((await send(true, tls, { path: '/__tamber/tool/' })).text).toContain('tool 2');
  } finally { await remote.close(); rmSync(root, { recursive: true, force: true }); }
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

test('the real wire on the remote door: the local token works nowhere there, nor the token route', async () => {
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const remote = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  try {
    const port = Number(new URL(remote.info().origin).port);
    // The local token, as a local client gets it.
    let token = null;
    const res = { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } };
    await wire.handle({ method: 'GET', headers: {} }, res, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    expect(typeof token).toBe('string');
    for (const path of ['/__tamber', '/__tamber/subscribe', '/__tamber/events']) {
      const r = await send(true, port, { method: 'POST', path, headers: { 'x-exact-tamber-token': token, 'content-type': 'application/json' }, body: '{"method":"hello"}' });
      expect(r.status).toBe(404);
      expect(r.text).not.toContain(token);
    }
    expect((await send(true, port, { path: '/__tamber/token' })).status).toBe(404);
  } finally { await remote.close(); await wire.close(); }
});

// The events' GET long poll (tamber/1.17, LLP 1122 stage 14 D) through the TLS door: a paired token of the POST read's
// scope gets it, with its cursor; the local token, another origin and plaintext do not (theme2's condition (a)).
test('the events GET long poll through the TLS door: a paired read token gets it, the local token, another origin and plaintext do not', async () => {
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const doors = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  try {
    const port = Number(new URL(doors.info().origin).port);
    let token = null, text = '';
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'read', name: 'laptop' })));
    await wire.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
    const pairing = JSON.parse((await send(true, port, { method: 'POST', path: '/__tamber/pair', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code: JSON.parse(text).ok.code, name: 'laptop' }) })).text).ok, paired = pairing.token;
    const first = await send(true, port, { path: '/__tamber/events?client=laptop', headers: { 'x-exact-tamber-token': paired } });
    expect(first.status).toBe(200);
    const body = JSON.parse(first.text);
    expect(body.cursor).toBe(`${body.seq}.${body.held}`);
    expect(first.headers['retry-after']).toBeUndefined();
    // A slash in a query value is a value (a client's name), not a path: only the path is held to the door's rules.
    expect((await send(true, port, { path: '/__tamber/events?client=agent%2Fcodex', headers: { 'x-exact-tamber-token': paired } })).status).toBe(200);
    expect((await send(true, port, { path: '/__tamber%2Fevents?client=laptop', headers: { 'x-exact-tamber-token': paired } })).status).toBe(404);
    // A raw backslash is refused anywhere in the target, a query value included; one encoded in a query value is the
    // value's (rt1's note on the door's query rule).
    expect((await send(true, port, { path: '/__tamber/events?client=agent%5Ccodex', headers: { 'x-exact-tamber-token': paired } })).status).toBe(200);
    expect((await send(true, port, { path: '/__tamber/events?client=agent\\codex', headers: { 'x-exact-tamber-token': paired } })).status).toBe(404);
    // The local token is no token here; another page's origin is refused; plaintext is nothing.
    expect((await send(true, port, { path: '/__tamber/events', headers: { 'x-exact-tamber-token': token } })).status).toBe(404);
    expect((await send(true, port, { path: '/__tamber/events', headers: { 'x-exact-tamber-token': paired, origin: 'https://evil.example' } })).status).toBe(403);
    // (Plain HTTP from another machine, a --lan bind's peer: no door.)
    const lan = { status: 0, writeHead(s) { this.status = s; }, setHeader() {}, end() {} };
    await wire.handle(Object.assign(new PassThrough(), { method: 'GET', headers: { 'x-exact-tamber-token': paired }, socket: { remoteAddress: '192.168.1.20' } }), lan, new URL('http://192.168.1.2/__tamber/events'), { allowed: true, local: false });
    expect(lan.status).toBe(404);
    // Revoked: told so (401), as the POST form is, not a token unknown (404).
    const out = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => out.end(JSON.stringify({ method: 'revoke', client: pairing.client })));
    await wire.handle(out, { writeHead() {}, setHeader() {}, write() {}, end() {} }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
    expect((await send(true, port, { path: '/__tamber/events?client=laptop', headers: { 'x-exact-tamber-token': paired } })).status).toBe(401);
  } finally { await doors.close(); await wire.close(); }
}, 30_000);

test('edit-held through the TLS door: a paired propose client edits its own held proposal, not another\'s; a write client edits another\'s, which stays its client\'s', async () => {
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const doors = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  try {
    const port = Number(new URL(doors.info().origin).port);
    let token = null;
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    const local = async body => {
      let text = '';
      const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
      queueMicrotask(() => req.end(JSON.stringify(body)));
      await wire.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
      return JSON.parse(text);
    };
    const pair = async (scope, name) => {
      const code = (await local({ method: 'pair-code', scope, name })).ok.code;
      return JSON.parse((await send(true, port, { method: 'POST', path: '/__tamber/pair', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code, name }) })).text).ok;
    };
    const remote = async (paired, body) => {
      const r = await send(true, port, { method: 'POST', path: '/__tamber', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': paired.token }, body: JSON.stringify(body) });
      return { status: r.status, ...JSON.parse(r.text) };
    };
    const proposer = await pair('propose', 'laptop'), writer = await pair('write', 'desk');
    const agents = (await local({ method: 'propose', hold: true, client: 'agent:codex', ops: [{ op: 'set-token', name: 'ink', value: '#010101' }] })).ok.proposal;
    const own = (await remote(proposer, { method: 'propose', hold: true, client: proposer.client, ops: [{ op: 'set-token', name: 'ink', value: '#020202' }] })).ok.proposal;
    const edit = (paired, proposal) => remote(paired, { method: 'edit-held', client: paired.client, proposal, index: 0, value: '#0a0a0a' });
    const mine = await edit(proposer, own);
    expect([mine.status, mine.ok?.client, mine.ok?.replaces]).toEqual([200, proposer.client, own]);
    const theirs = await edit(proposer, agents);
    expect([theirs.status, theirs.error?.needs]).toEqual([403, 'write']);
    // (rt1's ask) A forged body client, the owner's name, is pinned to the paired one: still another's, still 403.
    const forged = await remote(proposer, { method: 'edit-held', client: 'agent:codex', proposal: agents, index: 0, value: '#0b0b0b' });
    expect([forged.status, forged.error?.needs]).toEqual([403, 'write']);
    const written = await edit(writer, agents);
    expect([written.status, written.ok?.client]).toEqual([200, 'agent:codex']);
  } finally { await doors.close(); await wire.close(); }
}, 30_000);

test('choose through the TLS door (tamber/1.18): a paired propose client chooses on its own held proposal, not another\'s, a forged name pinned; a write client chooses on another\'s, named as the chooser', async () => {
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const doors = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  try {
    const port = Number(new URL(doors.info().origin).port);
    let token = null;
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    const local = async body => {
      let text = '';
      const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
      queueMicrotask(() => req.end(JSON.stringify(body)));
      await wire.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
      return JSON.parse(text);
    };
    const pair = async (scope, name) => {
      const code = (await local({ method: 'pair-code', scope, name })).ok.code;
      return JSON.parse((await send(true, port, { method: 'POST', path: '/__tamber/pair', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code, name }) })).text).ok;
    };
    const remote = async (paired, body) => {
      const r = await send(true, port, { method: 'POST', path: '/__tamber', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': paired.token }, body: JSON.stringify(body) });
      return { status: r.status, ...JSON.parse(r.text) };
    };
    const proposer = await pair('propose', 'laptop'), writer = await pair('write', 'desk');
    const agents = (await local({ method: 'propose', hold: true, client: 'agent:codex', ops: [{ op: 'set-token', name: 'ink', value: '#010101' }] })).ok.proposal;
    const own = (await remote(proposer, { method: 'propose', hold: true, client: proposer.client, ops: [{ op: 'set-token', name: 'ink', value: '#020202' }] })).ok.proposal;
    const choose = (paired, proposal, client = paired.client) => remote(paired, { method: 'choose', client, proposal, items: [0] });
    const mine = await choose(proposer, own);
    expect([mine.status, mine.ok?.choiceBy]).toEqual([200, proposer.client]);
    const theirs = await choose(proposer, agents);
    expect([theirs.status, theirs.error?.needs]).toEqual([403, 'write']);
    const forged = await choose(proposer, agents, 'agent:codex');
    expect([forged.status, forged.error?.needs]).toEqual([403, 'write']);
    const written = await choose(writer, agents);
    expect([written.status, written.ok?.choiceBy]).toEqual([200, writer.client]);
  } finally { await doors.close(); await wire.close(); }
}, 30_000);

// The ghost builds' follow-up (review2's condition 1): under Bun a request on a socket handed to the HTTP server, as
// every TLS door request is, never fires req 'close'. A paired client's subscription there still ends when the client
// goes, on res 'close': it is present while the stream is open and not after.
test('a subscription through the TLS door ends when its client leaves', async () => {
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const doors = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  let stream;
  try {
    const port = Number(new URL(doors.info().origin).port);
    let token = null, text = '';
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'write', name: 'laptop' })));
    await wire.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
    const paired = JSON.parse((await send(true, port, { method: 'POST', path: '/__tamber/pair', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code: JSON.parse(text).ok.code, name: 'laptop' }) })).text).ok.token;
    const headers = { 'x-exact-tamber-token': paired, 'content-type': 'application/json' };
    // A pin is a present client's: 409 when it is not.
    const pin = async () => (await send(true, port, { method: 'POST', path: '/__tamber', headers, body: JSON.stringify({ method: 'pin', build: 'nowhere', scheme: 'dark' }) })).status;
    expect(await pin()).toBe(409);
    const first = await new Promise((ok, fail) => {
      // (its first event whole, however the bytes come)
      stream = secureRequest({ host: '127.0.0.1', port, method: 'POST', path: '/__tamber/subscribe', headers: { host: `127.0.0.1:${port}`, ...headers }, rejectUnauthorized: false }, res => { let got = ''; res.on('data', d => { got += d; if (got.includes('\n\n')) ok(got); }); });
      stream.on('error', fail);
      stream.end('{}');
    });
    expect(first).toContain('"event":"subscribed"');
    expect(await pin()).not.toBe(409);
    stream.destroy();
    const end = Date.now() + 5_000;
    while ((await pin()) !== 409) {
      if (Date.now() > end) throw new Error('the subscription outlived its client');
      await new Promise(ok => setTimeout(ok, 100));
    }
  } finally { stream?.destroy(); await doors.close(); await wire.close(); }
}, 30_000);

test('a connection slow to send its request, silent, trickling its head, or idle after an answer is closed; an answer that takes long is not', async () => {
  // (A wire reads the request's body before it answers, as the real one does.)
  const wire = { async handle(req, res) { for await (const _ of req); await new Promise(ok => setTimeout(ok, 600)); res.end('{}'); return true; } };
  const remote = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {}, timeoutMs: 300 });
  try {
    const port = Number(new URL(remote.info().origin).port);
    // Headers that never end: cut.
    const { connect: tlsConnect } = await import('node:tls');
    const slow = await new Promise(ok => {
      const at = Date.now(), socket = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => socket.write(`GET /__tamber HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\n`));
      socket.on('close', () => ok(Date.now() - at));
      socket.on('error', () => {});
      setTimeout(() => { socket.destroy(); ok(Infinity); }, 3000);
    });
    expect(slow).toBeLessThan(2000);
    // A head trickled a byte at a time, each well inside the timeout: cut at the deadline.
    const trickled = await new Promise(ok => {
      const at = Date.now(), socket = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => {
        const text = `GET /__tamber HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nX-Pad: ${'a'.repeat(100)}`;
        let i = 0;
        const drip = setInterval(() => { if (socket.destroyed || i >= text.length) return clearInterval(drip); socket.write(text[i++]); }, 50);
      });
      socket.on('close', () => ok(Date.now() - at));
      socket.on('error', () => {});
      setTimeout(() => { socket.destroy(); ok(Infinity); }, 4000);
    });
    expect(trickled).toBeLessThan(2000);
    // A connection that never starts its handshake: cut too.
    const t0 = Date.now(), silent = await rawTCP(port, '');
    expect(silent.closed).toBe(true);
    expect(Date.now() - t0).toBeLessThan(2000);

    // Answered, then idle on a kept-alive connection: closed at the deadline.
    const idle = await new Promise(ok => {
      let answeredAt = 0;
      const socket = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => socket.write(`POST /__tamber HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Length: 0\r\n\r\n`));
      socket.on('data', () => { answeredAt ||= Date.now(); });
      socket.on('close', () => ok(answeredAt ? Date.now() - answeredAt : -1));
      socket.on('error', () => {});
      setTimeout(() => { socket.destroy(); ok(Infinity); }, 4000);
    });
    expect(idle).toBeGreaterThan(0);
    expect(idle).toBeLessThan(2000);
    // A request whose answer takes twice the timeout is answered.
    expect((await send(true, port, { method: 'POST', path: '/__tamber' })).status).toBe(200);
  } finally { await remote.close(); }
});

test('mixed-case names and IPv6 hosts: lowercased, bracketed', () => {
  expect(remoteArgs(['--remote', 'MyBox.local:7443']).remote).toEqual({ host: 'mybox.local', port: 7443 });
  expect(remoteArgs(['--remote', '[::1]:0']).remote).toEqual({ host: '::1', port: 0 });
  expect(remoteArgs(['--remote', '[0:0:0:0:0:0:0:0]:7443']).remote).toEqual({ host: '::', port: 7443 });
});

test('a trusted tunnel is a loopback port of its own, said loudly, its requests remote and tunnelled', async () => {
  const wire = recordingWire(), warned = [];
  const doors = await openRemote(wire, { remote: null, tunnel: 0 }, { say: () => {}, warn: l => warned.push(l) });
  try {
    expect(doors.info()).toBe(null);
    const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    expect(warned.join('\n')).toBe(`TRUSTED TUNNEL ON: http://127.0.0.1:${port} takes a tunnel's requests (--trust-tunnel). Only paired tokens work through it, never this machine's local token; point only a tunnel you run at it.`);
    // On IPv6's loopback too, as remote and tunnelled.
    const six = await new Promise(ok => { const r = plainRequest({ host: '::1', port, method: 'POST', path: '/__tamber', headers: { host: `[::1]:${port}` } }, res => { res.resume(); res.on('end', () => ok(res.statusCode)); }); r.on('error', e => ok(e.code)); r.end(); });
    expect(six).toBe(200);
    expect(wire.seen.at(-1).access).toMatchObject({ remote: true, tunnel: true, local: false });
    // A page of another origin on this machine, a simple text/plain POST: refused, the wire not asked.
    const asked = wire.seen.length;
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'http://localhost:3000', 'content-type': 'text/plain' }, body: '{}' })).status).toBe(403);
    expect((await send(false, port, { path: '/__tamber', headers: { host: `127.0.0.1.nip.io:${port}` } })).status).toBe(404);
    expect(wire.seen.length).toBe(asked);
    // Two Cookie headers on the plain socket: none reaches the wire.
    await new Promise(ok => { const c = connect(port, '127.0.0.1', () => c.write(`POST /__tamber HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nCookie: a=1\r\nCOOKIE: b=2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n`)); c.on('data', () => {}); c.on('close', ok); c.on('error', ok); });
    expect(wire.seen.at(-1)).toMatchObject({ cookie: undefined, anyCookie: false });
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { cookie: 'a=b' } })).status).toBe(200);
    expect(wire.seen.at(-1)).toMatchObject({ access: { allowed: true, local: false, remote: true, tunnel: true, peer: '127.0.0.1' }, cookie: undefined, anyCookie: false });
    expect((await send(false, port, { path: '/' })).status).toBe(404);
    expect((await send(false, port, { path: '/__tamber/token' })).status).toBe(404);
    // A tunnel's own name for this machine, at the port it forwards from.
    expect((await send(false, port, { path: '/__tamber', headers: { host: 'localhost:9000' } })).status).toBe(200);
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { host: 'localhost:9000', origin: 'http://localhost:9001' } })).status).toBe(403);
    expect((await send(false, port, { path: '/__tamber', headers: { host: 'example.com' } })).status).toBe(404);
  } finally { await doors.close(); }
});

test('the real wire through the tunnel: the local token works not there either', async () => {
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const warned = [];
  const doors = await openRemote(wire, { remote: null, tunnel: 0 }, { say: () => {}, warn: l => warned.push(l) });
  try {
    const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    let token = null;
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    for (const path of ['/__tamber', '/__tamber/subscribe', '/__tamber/events'])
      for (const headers of [{}, { origin: `http://127.0.0.1:${port}`, 'content-type': 'text/plain' }])
      { const r = await send(false, port, { method: 'POST', path, headers: { 'x-exact-tamber-token': token, ...headers }, body: '{"method":"hello"}' }); expect(r.status).toBe(404); expect(r.text).not.toContain(token); }
    // Its port taken: refused, not another.
    await expect(openRemote(wire, { remote: null, tunnel: port }, { say: () => {}, warn: () => {} })).rejects.toThrow();
  } finally { await doors.close(); await wire.close(); }
});

test('keep-alive requests leave one timer listener on their socket, not one each', async () => {
  const counts = [];
  const wire = { async handle(req, res) { counts.push(req.socket.listenerCount('timeout')); res.end('{}'); return true; } };
  const remote = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  try {
    const port = Number(new URL(remote.info().origin).port);
    const { request, Agent } = await import('node:https');
    const agent = new Agent({ keepAlive: true, maxSockets: 1 });
    for (let i = 0; i < 20; i++) await new Promise(ok => request({ host: '127.0.0.1', port, method: 'POST', path: '/__tamber', agent, rejectUnauthorized: false, headers: { host: `127.0.0.1:${port}` } }, r => { r.resume(); r.on('end', ok); }).end());
    agent.destroy();
    expect(counts.length).toBe(20);
    expect(Math.max(...counts)).toBe(counts[0]);
  } finally { await remote.close(); }
});

test('a 65th connection is refused while 64 sit before their handshake', async () => {
  const remote = await openRemote(recordingWire(), { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {}, timeoutMs: 5000 });
  const held = [];
  try {
    const port = Number(new URL(remote.info().origin).port);
    for (let i = 0; i < 64; i++) held.push(await new Promise(ok => { const s = connect(port, '127.0.0.1', () => ok(s)); s.on('error', () => {}); }));
    await new Promise(ok => setTimeout(ok, 100));
    const extra = await new Promise(ok => { const s = connect(port, '127.0.0.1'); s.on('error', () => {}); s.on('close', () => ok('closed')); setTimeout(() => { s.destroy(); ok('open'); }, 1000); });
    expect(extra).toBe('closed');
  } finally { for (const s of held) s.destroy(); await remote.close(); }
});

test('an answer sent before its body ends still leaves a deadline; a pipelined slow answer is not cut by the first one\'s', async () => {
  const wire = { async handle(req, res, url) { for await (const _ of req); if (url.pathname === '/__tamber/events') await new Promise(ok => setTimeout(ok, 700)); res.end(url.pathname); return true; } };
  const remote = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {}, timeoutMs: 300 });
  try {
    const port = Number(new URL(remote.info().origin).port);
    const { connect: tlsConnect } = await import('node:tls');
    // A 404 before the body: the body comes, then the connection idles and is closed.
    const early = await new Promise(ok => {
      let answered = 0;
      const socket = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => socket.write(`POST /nowhere HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Length: 5\r\n\r\n`));
      socket.on('data', () => { if (!answered) { answered = Date.now(); socket.write('abcde'); } });
      socket.on('close', () => ok(answered ? Date.now() - answered : -1));
      socket.on('error', () => {});
      setTimeout(() => { socket.destroy(); ok(Infinity); }, 4000);
    });
    expect(early).toBeGreaterThan(0);
    expect(early).toBeLessThan(2000);
  } finally { await remote.close(); }
  // Two requests in one write, the second's answer longer than the deadline:
  // both answered. (On the plain tunnel: Bun 1.4.2 closes a TLS connection
  // that pipelines, which no browser does.)
  const warned = [];
  const doors = await openRemote(wire, { remote: null, tunnel: 0 }, { warn: l => warned.push(l), timeoutMs: 300 });
  try {
    const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    const both = await new Promise(ok => {
      let text = '';
      const head = path => `POST ${path} HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Length: 0\r\n\r\n`;
      const socket = connect(port, '127.0.0.1', () => socket.write(head('/__tamber') + head('/__tamber/events')));
      socket.on('data', d => { text += d; if (text.includes('/__tamber/events')) { socket.destroy(); ok(text); } });
      socket.on('close', () => ok(text));
      socket.on('error', () => {});
      setTimeout(() => { socket.destroy(); ok(text); }, 4000);
    });
    expect(both).toContain('/__tamber/events');
  } finally { await doors.close(); }
});

test('an answer sent before its body is read, even with no body, ends with the connection; 64 such do not keep the door full', async () => {
  const wire = { async handle(req, res) { res.end('early'); return true; } };
  const remote = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {}, timeoutMs: 300 });
  try {
    const port = Number(new URL(remote.info().origin).port);
    const { connect: tlsConnect } = await import('node:tls');
    const open = () => new Promise(ok => {
      const socket = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => socket.write(`POST /__tamber HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Length: 0\r\n\r\n`));
      socket.on('error', () => {});
      socket.on('data', () => {});
      ok(socket);
    });
    const sockets = await Promise.all(Array.from({ length: 64 }, open));
    await new Promise(ok => setTimeout(ok, 1200));
    expect(sockets.filter(s => !s.destroyed && !s.closed).length).toBe(0);
    expect((await send(true, port, { method: 'POST', path: '/__tamber' })).text).toBe('early');
  } finally { await remote.close(); }
});

test('a head sent late gains no time for its body', async () => {
  const wire = { async handle(req, res) { for await (const _ of req); res.end('{}'); return true; } };
  const remote = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {}, timeoutMs: 400 });
  try {
    const port = Number(new URL(remote.info().origin).port);
    const { connect: tlsConnect } = await import('node:tls');
    const at = Date.now();
    const closed = await new Promise(ok => {
      const socket = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => setTimeout(() => socket.write(`POST /__tamber HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Length: 5\r\n\r\n`), 250));
      socket.on('data', () => {});
      socket.on('close', () => ok(Date.now() - at));
      socket.on('error', () => {});
      setTimeout(() => { socket.destroy(); ok(Infinity); }, 3000);
    });
    // Cut at the connection's first deadline (400 ms from accept), not 400 ms after the head.
    expect(closed).toBeLessThan(600);
  } finally { await remote.close(); }
});

test('pairing end to end: the code names the doors and the fingerprint; the other machine pairs over TLS and its token works there', async () => {
  let doors = null;
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, remote: () => doors?.list() ?? [] });
  const warned = [];
  doors = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 }, tunnel: 0 }, { say: () => {}, warn: l => warned.push(l) });
  try {
    const { origin, fingerprint } = doors.info();
    const port = Number(new URL(origin).port), tunnelPort = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    // This machine: the local token, then a code.
    const local = async (body) => {
      let status = 0, text = '';
      const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
      const res = { writeHead(s) { status = s; }, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } };
      queueMicrotask(() => req.end(JSON.stringify(body)));
      await wire.handle(req, res, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
      return { status, body: JSON.parse(text) };
    };
    let token = null;
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    const minted = await local({ method: 'pair-code', scope: 'propose', name: 'laptop' });
    expect(minted.status).toBe(200);
    expect(minted.body.ok.remote).toEqual([{ origin, fingerprint }, { origin: `http://127.0.0.1:${tunnelPort}`, tunnel: true }]);
    // The other machine, over TLS: on the connection it pairs on, it checks the
    // certificate is the one named before a byte of the code is sent.
    const { connect: tlsConnect } = await import('node:tls');
    const body = JSON.stringify({ code: minted.body.ok.code, name: 'laptop' });
    const pairOn = expected => new Promise(ok => {
      let text = '';
      const s = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => {
        if (s.getPeerCertificate().fingerprint256 !== expected) { s.destroy(); return ok(null); }
        s.write(`POST /__tamber/pair HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`);
      });
      s.on('data', d => (text += d));
      s.on('close', () => ok(text));
      s.on('error', () => ok(text));
    });
    // A pin that is not this certificate's: nothing is sent, and the code still pairs after.
    expect(await pairOn(fingerprint.replace(/^../, fingerprint.startsWith('00') ? '11' : '00'))).toBe(null);
    const answer = await pairOn(fingerprint);
    expect(answer).toStartWith('HTTP/1.1 200');
    const answered = answer.slice(answer.indexOf('\r\n\r\n') + 4); // (its one JSON body, chunked or not)
    const { token: paired_token, scope } = JSON.parse(answered.slice(answered.indexOf('{'), answered.lastIndexOf('}') + 1)).ok;
    expect(scope).toBe("propose");
    const hello = await send(true, port, { method: 'POST', path: '/__tamber', headers: { 'x-exact-tamber-token': paired_token, 'content-type': 'application/json' }, body: '{"method":"hello"}' });
    expect(hello.status).toBe(200);
    expect(JSON.parse(hello.text).ok.scope).toBe("propose");
    // Its scope holds there: no write, no local method.
    const write = await send(true, port, { method: 'POST', path: '/__tamber', headers: { 'x-exact-tamber-token': paired_token }, body: '{"method":"transaction","ops":[]}' });
    expect([write.status, JSON.parse(write.text).error?.needs]).toEqual([403, 'write']);
    const mint = await send(true, port, { method: 'POST', path: '/__tamber', headers: { 'x-exact-tamber-token': paired_token }, body: '{"method":"pair-code","scope":"read"}' });
    expect([mint.status, JSON.parse(mint.text).error?.needs]).toEqual([403, 'local']);
    // The local token is still no one there.
    const localThere = await send(true, port, { method: 'POST', path: '/__tamber', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' });
    expect(localThere.status).toBe(404);
    expect(localThere.text).not.toContain(token);
  } finally { await doors.close(); await wire.close(); }
});

test('with no door open, pair-code names none', async () => {
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  try {
    let token = null, text = '';
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'read' })));
    await wire.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
    const ok = JSON.parse(text).ok;
    expect(typeof ok.code).toBe('string');
    expect('remote' in ok).toBe(false);
  } finally { await wire.close(); }
});

test('--allow-plugin-origin: the tunnel answers a sandboxed page (Origin: null) on the wire\'s paths, with CORS; off, it does not', async () => {
  for (const plugin of [false, true]) {
    const wire = recordingWire(), warned = [];
    const doors = await openRemote(wire, { remote: null, tunnel: 0, plugin }, { say: () => {}, warn: l => warned.push(l) });
    try {
      const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
      const pre = await send(false, port, { method: 'OPTIONS', path: '/__tamber', headers: { origin: 'null', 'access-control-request-method': 'POST', 'access-control-request-headers': 'x-exact-tamber-token' } });
      const post = await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null' } });
      const tool = await send(false, port, { path: '/__tamber/tool/', headers: { origin: 'null' } });
      if (!plugin) {
        expect([pre.status, post.status]).toEqual([403, 403]);
        continue;
      }
      expect(pre.status).toBe(204);
      expect([pre.headers['access-control-allow-origin'], pre.headers['access-control-allow-headers'], pre.headers['access-control-allow-methods']]).toEqual(['null', 'content-type, x-exact-tamber-token', 'POST']);
      expect([post.status, post.headers['access-control-allow-origin']]).toEqual([200, 'null']);
      expect(wire.seen.at(-1).access).toMatchObject({ remote: true, tunnel: true, local: false });
      expect(tool.status).toBe(403); // the wire's paths only
    } finally { await doors.close(); }
  }
});

test('a tunnel\'s public name (--allow-host) is admitted, its https origin too; another name is not', async () => {
  const warned = [];
  const doors = await openRemote(recordingWire(), { remote: null, tunnel: 0, hosts: ['tamber.example.com'] }, { say: () => {}, warn: l => warned.push(l) });
  try {
    const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { host: 'tamber.example.com', origin: 'https://tamber.example.com' } })).status).toBe(200);
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { host: 'tamber.example.com', origin: 'https://evil.example.com' } })).status).toBe(403);
    expect((await send(false, port, { path: '/__tamber', headers: { host: 'other.example.com' } })).status).toBe(404);
    // A loopback name is not a public one: its origin is http only.
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { host: `127.0.0.1:${port}`, origin: `https://127.0.0.1:${port}` } })).status).toBe(403);
  } finally { await doors.close(); }
});

test('the main loopback port answers a sandboxed page\'s wire request as a remote one; the local token is no one there', async () => {
  const { createServer } = await import('node:http');
  const { pluginOnLoopback } = await import('./tamber-remote.mjs');
  const { developmentGate } = await import('./install-page.mjs');
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port, gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const reached = [];
  server.on('request', async (req, res) => {
    if (await pluginOnLoopback(wire, req, res)) return reached.push(req.url);
    const access = gate.check(req);
    if (!access.allowed) { res.writeHead(421); res.end(); return; }
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), access))) { res.writeHead(404); res.end(); }
  });
  try {
    const token = (await (await fetch(`http://127.0.0.1:${port}/__tamber/token`)).json()).token;
    const pre = await send(false, port, { method: 'OPTIONS', path: '/__tamber', headers: { origin: 'null' } });
    expect([pre.status, pre.headers['access-control-allow-origin']]).toEqual([204, 'null']);
    // The local token from a sandboxed page: no one.
    const local = await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', 'x-exact-tamber-token': token }, body: '{"method":"hello"}' });
    expect(local.status).toBe(404);
    // A paired token from it: works, at its scope.
    const minted = await fetch(`http://127.0.0.1:${port}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"pair-code","scope":"read"}' }).then(r => r.json());
    const paired = await send(false, port, { method: 'POST', path: '/__tamber/pair', headers: { origin: 'null' }, body: JSON.stringify({ code: minted.ok.code }) });
    expect([paired.status, paired.headers['access-control-allow-origin']]).toEqual([200, 'null']);
    const hello = await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', 'x-exact-tamber-token': JSON.parse(paired.text).ok.token }, body: '{"method":"hello"}' });
    expect([hello.status, JSON.parse(hello.text).ok.scope]).toEqual([200, 'read']);
    // At its scope only: a read token proposes nothing, and a local method is no one's from here.
    const read = JSON.parse(paired.text).ok.token;
    const call = async body => send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', 'x-exact-tamber-token': read }, body: JSON.stringify(body) });
    expect((await call({ method: 'propose', client: 'figma', hold: true, ops: [] })).status).toBe(403);
    expect((await call({ method: 'pair-code', scope: 'read' })).status).toBe(403);
    expect((await call({ method: 'transaction', client: 'figma', ops: [] })).status).toBe(403);
    // The event stream path takes it too, with CORS.
    const events = await send(false, port, { method: 'POST', path: '/__tamber/events', headers: { origin: 'null', 'x-exact-tamber-token': read }, body: '{"after":0,"wait":false}' });
    expect([events.status, events.headers['access-control-allow-origin']]).toEqual([200, 'null']);
    // Wrong codes: refused, then limited, the wait readable through CORS.
    let limited;
    for (let i = 0; i < 8 && !limited; i++) {
      const r = await send(false, port, { method: 'POST', path: '/__tamber/pair', headers: { origin: 'null' }, body: '{"code":"AAAA-AAAA-AAAA"}' });
      if (r.status === 429) limited = r;
    }
    expect([limited?.headers['access-control-expose-headers'], typeof limited?.headers['retry-after']]).toEqual(['retry-after', 'string']);
    // Not the loop's otherwise: a rebound name at this very port goes to its gate.
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', host: `evil.example:${port}` } })).status).toBe(421);
    // The token route is the gate's as ever, and carries no CORS: a sandboxed page cannot read it.
    const tokenRoute = await send(false, port, { path: '/__tamber/token', headers: { origin: 'null' } });
    expect(tokenRoute.headers['access-control-allow-origin']).toBeUndefined();
    expect(reached.every(u => ['/__tamber', '/__tamber/pair', '/__tamber/events'].includes(u))).toBe(true);
  } finally { server.close(); await wire.close(); }
});

test('round 1 of E1: a loopback --allow-host stays http; a default port written or not; Retry-After readable; the TLS door ignores the plugin flag', async () => {
  const warned = [];
  const doors = await openRemote(recordingWire(), { remote: { host: '127.0.0.1', port: 0 }, tunnel: 0, plugin: true, hosts: ['localhost', 'tamber.example.com:443'] }, { say: () => {}, warn: l => warned.push(l) });
  try {
    const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]), tls = Number(new URL(doors.info().origin).port);
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { host: 'localhost:9000', origin: 'https://localhost:9000' } })).status).toBe(403);
    for (const host of ['tamber.example.com', 'tamber.example.com:443'])
      expect([host, (await send(false, port, { method: 'POST', path: '/__tamber', headers: { host, origin: 'https://tamber.example.com' } })).status]).toEqual([host, 200]);
    expect((await send(false, port, { path: '/__tamber', headers: { host: 'tamber.example.com:8443' } })).status).toBe(404);
    const pre = await send(false, port, { method: 'OPTIONS', path: '/__tamber/pair', headers: { origin: 'null' } });
    expect(pre.headers['access-control-expose-headers']).toBe('retry-after');
    // A sandboxed page's preflight for another path: refused.
    expect((await send(false, port, { method: 'OPTIONS', path: '/__tamber/tool/', headers: { origin: 'null' } })).status).toBe(403);
    // The TLS door: no sandboxed page, the flag or not.
    expect((await send(true, tls, { method: 'POST', path: '/__tamber', headers: { origin: 'null' } })).status).toBe(403);
  } finally { await doors.close(); }
});

test('round 1 of E1: a public name takes https only; the loopback hook takes this port and no forwarded request', async () => {
  const warned = [];
  const doors = await openRemote(recordingWire(), { remote: null, tunnel: 0, hosts: ['tamber.example.com'] }, { say: () => {}, warn: l => warned.push(l) });
  try {
    const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { host: 'tamber.example.com', origin: 'http://tamber.example.com' } })).status).toBe(403);
  } finally { await doors.close(); }
  const { createServer } = await import('node:http');
  const { pluginOnLoopback } = await import('./tamber-remote.mjs');
  const wire = recordingWire(), server = createServer(async (req, res) => { if (!(await pluginOnLoopback(wire, req, res))) { res.writeHead(421); res.end(); } });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  try {
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', cookie: 'a=b' } })).status).toBe(200);
    expect(wire.seen.at(-1)).toMatchObject({ cookie: undefined, anyCookie: false, access: { remote: true, plugin: true, local: false } });
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', host: `127.0.0.1:${port + 1}` } })).status).toBe(421);
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', 'x-forwarded-for': '203.0.113.9' } })).status).toBe(421);
  } finally { server.close(); }
});

test('round 2 of E1: each side\'s left-out port is its own scheme\'s default; a bare --allow-host is 443; any x-forwarded-* is a proxy', async () => {
  const warned = [];
  const doors = await openRemote(recordingWire(), { remote: null, tunnel: 0, plugin: true, hosts: ['tamber.example.com', 'alt.example.com:8443'] }, { say: () => {}, warn: l => warned.push(l) });
  try {
    const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    const status = async (host, origin) => (await send(false, port, { method: 'POST', path: '/__tamber', headers: { host, ...(origin ? { origin } : {}) } })).status;
    expect(await status('tamber.example.com', 'https://tamber.example.com:8443')).toBe(403);
    expect(await status('tamber.example.com:443', 'https://tamber.example.com')).toBe(200);
    expect(await status('tamber.example.com:8443')).toBe(404);
    expect(await status('tamber.example.com:80', 'https://tamber.example.com')).toBe(404);
    expect(await status('alt.example.com:8443', 'https://alt.example.com:8443')).toBe(200);
    expect(await status('alt.example.com')).toBe(404);
    // A loopback Host with no port is the door's default, never the origin's port.
    expect(await status('127.0.0.1', 'http://127.0.0.1:9999')).toBe(403);
    const pre = await send(false, port, { method: 'OPTIONS', path: '/__tamber', headers: { host: 'tamber.example.com', origin: 'null' } });
    expect([pre.status, pre.headers.vary, pre.headers['access-control-max-age'], pre.headers['access-control-allow-credentials']]).toEqual([204, 'origin', '600', undefined]);
  } finally { await doors.close(); }
  const { createServer } = await import('node:http');
  const { pluginOnLoopback } = await import('./tamber-remote.mjs');
  const wire = recordingWire(), server = createServer(async (req, res) => { if (!(await pluginOnLoopback(wire, req, res))) { res.writeHead(421); res.end(); } });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  try {
    for (const h of ['x-forwarded-proto', 'x-forwarded-port', 'forwarded'])
      expect([h, (await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', [h]: 'https' } })).status]).toEqual([h, 421]);
  } finally { server.close(); }
});

test('round 3 of E1: the real wire through the tunnel to a sandboxed page: the local token is no one; [::1] at its port is its own origin', async () => {
  const wire = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const warned = [];
  const doors = await openRemote(wire, { remote: null, tunnel: 0, plugin: true, hosts: [] }, { say: () => {}, warn: l => warned.push(l) });
  try {
    const port = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
    let token = null;
    const res = { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } };
    await wire.handle({ method: 'GET', headers: {} }, res, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    const local = await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', 'x-exact-tamber-token': token }, body: '{"method":"hello"}' });
    expect([local.status, local.headers['access-control-allow-origin']]).toEqual([404, 'null']);
    expect(local.text).not.toContain(token);
    const six = await send(false, port, { method: 'POST', path: '/__tamber', headers: { host: `[::1]:${port}`, origin: `http://[::1]:${port}` }, body: '{"method":"hello"}' });
    expect(six.status).not.toBe(403);
  } finally { await doors.close(); await wire.close(); }
});

// tamber/1.15 (round 1's rework): what a client reads of this
// machine's files. Every `file` is named by one rule (from the app, from its
// package, or `<outside>/…`); a client that is not local (paired, through B's
// door or a tunnel) gets machine-written messages without this machine's
// directories and `hello.app` as the directory's name; authored text comes
// back byte for byte to every client. Through the real design server
// (`contract tamber serve --stdio`), with the app under the home directory
// and a package linked from outside both, and a build registered with a
// source map naming that package's real path.
async function pairedApp(dir) {
  let doors = null;
  const { readFileSync: read, realpathSync } = await import('node:fs');
  const { createHash } = await import('node:crypto');
  const linkedFile = realpathSync(join(dir, 'node_modules/@acme/linked/index.contract'));
  const P1 = '1'.repeat(64);
  const map = { digest: P1, sources: [{ file: linkedFile, sha256: createHash('sha256').update(read(linkedFile)).digest('hex') }], nodes: [{ file: linkedFile, line: 5, col: 5, end_col: 11, component: 'LinkedCard', chain: [], bindings: [{ row: 'padding_top', origin: 'own' }] }] };
  const wire = tamberServer({ appDir: dir, log: () => {}, remote: () => doors?.list() ?? [], sourceMap: () => Buffer.from(JSON.stringify(map)) });
  const warned = [];
  doors = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 }, tunnel: 0 }, { say: () => {}, warn: l => warned.push(l) });
  await wire.ready;
  const { origin, fingerprint } = doors.info();
  const port = Number(new URL(origin).port), tunnelPort = Number(/127\.0\.0\.1:(\d+)/.exec(warned.join('\n'))[1]);
  let token = null;
  await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
  const local = async (body, path = '/__tamber') => {
    let text = '';
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
    const res = { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } };
    queueMicrotask(() => req.end(JSON.stringify(body)));
    await wire.handle(req, res, new URL(`http://127.0.0.1${path}`), { allowed: true, local: true });
    return text;
  };
  const { connect: tlsConnect } = await import('node:tls');
  const pair = async scope => {
    const minted = JSON.parse(await local({ method: 'pair-code', scope, name: scope }));
    const body = JSON.stringify({ code: minted.ok.code, name: scope });
    const answer = await new Promise(ok => {
      let text = '';
      const s = tlsConnect({ host: '127.0.0.1', port, rejectUnauthorized: false }, () => {
        expect(s.getPeerCertificate().fingerprint256).toBe(fingerprint);
        s.write(`POST /__tamber/pair HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`);
      });
      s.on('data', d => (text += d));
      s.on('close', () => ok(text));
      s.on('error', () => ok(text));
    });
    const answered = answer.slice(answer.indexOf('\r\n\r\n') + 4);
    return JSON.parse(answered.slice(answered.indexOf('{'), answered.lastIndexOf('}') + 1)).ok.token;
  };
  const remote = (paired, body, path = '/__tamber', via = 'door') => via === 'door'
    ? send(true, port, { method: 'POST', path, headers: { 'x-exact-tamber-token': paired, 'content-type': 'application/json' }, body: JSON.stringify(body) })
    : send(false, tunnelPort, { method: 'POST', path, headers: { 'x-exact-tamber-token': paired, 'content-type': 'application/json' }, body: JSON.stringify(body) });
  return { wire, doors, local, pair, remote, P1, linkedFile };
}

/** An app under the repo's target/ (inside the home directory) whose design
 * holds its own path in an authored value, a git history whose commit names
 * it too, and `@acme/linked` linked from a directory outside the app and
 * the home directory. */
async function machineApp() {
  const { execFileSync } = await import('node:child_process');
  const root = new URL('..', import.meta.url).pathname;
  mkdirSync(join(root, 'target'), { recursive: true });
  const dir = mkdtempSync(join(root, 'target', 'tamber-paths-'));
  const outside = mkdtempSync(join(tmpdir(), 'tamber-linked-'));
  writeFileSync(join(outside, 'package.json'), '{"name":"@acme/linked","version":"1.0.0","exports":{".":"./index.contract"}}');
  writeFileSync(join(outside, 'index.contract'), 'component LinkedCard\n  params\n    inset: length = "4px"\n  view\n    column padding="param.inset"\n      text "x"\n');
  mkdirSync(join(dir, 'node_modules/@acme'), { recursive: true });
  const { symlinkSync } = await import('node:fs');
  symlinkSync(outside, join(dir, 'node_modules/@acme/linked'));
  writeFileSync(join(dir, 'app.contract'), 'use LinkedCard from "@acme/linked"\ncomponent App\n  view\n    column background-color="ink"\n      LinkedCard()\n');
  const authored = `${dir}/assets/icon.svg`;
  writeFileSync(join(dir, 'app.tamber'), `tamber t\n\ntheme\n  ink = #111111  help="${authored}"\n`);
  const git = (...a) => execFileSync('git', ['-C', dir, ...a], { stdio: 'pipe' });
  git('init', '-q'); git('add', 'app.tamber', 'app.contract');
  git('-c', 'user.name=t', '-c', 'user.email=t@example.com', 'commit', '-q', '-m', `design at ${dir}`);
  return { dir, outside, authored };
}

test('no answer to a remote client names this machine\'s directories; authored text comes back whole', async () => {
  const { homedir } = await import('node:os');
  const { realpathSync } = await import('node:fs');
  const { basename } = await import('node:path');
  const { dir, outside, authored } = await machineApp();
  const app = await pairedApp(dir);
  const machine = [...new Set([dir, realpathSync(dir), homedir(), realpathSync(homedir()), outside, realpathSync(outside)])];
  try {
    // The local tool's hello: the app's path, as before.
    expect(JSON.parse(await app.local({ method: 'hello' })).ok.app).toBe(dir);
    const writer = await app.pair('write');
    // A build on the dev loop's channel, its map naming the package's real path.
    const frames = [], b = app.wire.channel.open(f => frames.push(f));
    app.wire.channel.report(JSON.stringify({ nonce: b.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'], plan: 'g', planDigest: app.P1 }));
    app.wire.channel.report(JSON.stringify({ nonce: b.nonce, kind: 'status', scheme: 'light', revision: 0, planDigest: app.P1 }));
    await app.remote(writer, { client: 'laptop', wait: false }, '/__tamber/events');
    app.wire.channel.inspect('laptop', null, { on: true });
    const selected = { node: 0, site: 0, instance: [], planDigest: app.P1, tuple: {}, rows: [] };
    expect(app.wire.channel.report(JSON.stringify({ nonce: b.nonce, kind: 'selected', selected, inspect: frames.filter(f => f.kind === 'inspect').at(-1).seq, plan: 'g', revision: 0 })).status).toBe(204);
    const files = JSON.parse(await app.local({ method: 'hello' })).ok.files;
    const asked = [
      [writer, { method: 'hello', client: 'laptop' }],
      [writer, { method: 'describe' }],
      [writer, { method: 'resolve', token: 'LinkedCard.inset' }],
      [writer, { method: 'explain', token: 'LinkedCard.inset' }],
      [writer, { method: 'uses', token: 'LinkedCard.inset' }],
      [writer, { method: 'uses', token: 'ink' }],
      [writer, { method: 'propose', ops: [{ op: 'set-row', file: 'node_modules/@acme/linked/index.contract', at: { line: 5, col: 5 }, sha256: '0'.repeat(64), row: 'padding_top', value: '8px' }] }],
      [writer, { method: 'propose', ops: [{ op: 'set-token', name: 'ink', value: 'nowhere.at.all' }] }],
      [writer, { method: 'transaction', client: 'laptop', files, ops: [{ op: 'set-token', name: 'ink', value: 'nowhere.at.all' }] }],
      [writer, { method: 'inspect', client: 'laptop', refresh: true }],
    ];
    const replies = [];
    for (const [who, request] of asked) replies.push([request, await app.remote(who, request)]);
    const body = i => JSON.parse(replies[i][1].text);
    expect(body(0).ok.app).toBe(basename(dir));
    expect(body(0).ok.protocol).toBe('tamber/1.18');
    const sites = body(4).ok.sites;
    expect(sites.length).toBeGreaterThan(0);
    const linkedName = sites[0].file;
    expect(['@acme/linked/index.contract', 'node_modules/@acme/linked/index.contract']).toContain(linkedName);
    expect(body(3).ok.param.file).toBe(linkedName);
    expect(body(5).ok.sites.every(s => s.file === 'app.contract')).toBe(true);
    // The selection in the long poll: the package's name for its file.
    expect(body(9).ok).toBeDefined();
    const polled = await app.remote(writer, { client: 'laptop', after: 0, wait: false }, '/__tamber/events');
    replies.push([{ path: '/__tamber/events' }, polled]);
    expect(JSON.parse(polled.text).events.find(e => e.event === 'selection').source.file).toBe('@acme/linked/index.contract');
    // The tool outlines `uses`'s site: it reaches the build's map, whose file is the real path.
    const outlined = await app.remote(writer, { method: 'outline', client: 'laptop', at: sites });
    replies.push([{ method: 'outline' }, outlined]);
    expect(JSON.parse(outlined.text).ok.builds).toEqual(['web-1']);
    expect(frames.filter(f => f.kind === 'outline').at(-1)).toEqual({ kind: 'outline', sites: [0], plan: app.P1 });
    replies.push([{ method: 'hello', via: 'tunnel' }, await app.remote(writer, { method: 'hello' }, '/__tamber', 'tunnel')]);
    expect(JSON.parse(replies.at(-1)[1].text).ok.app).toBe(basename(dir));
    for (const [request, r] of replies) {
      expect([request, r.status !== undefined]).toEqual([request, true]);
      // (describe's metadata holds the authored path: held to it below.)
      if (request.method === 'describe') continue;
      for (const m of machine) expect([request, m, r.text.includes(m)]).toEqual([request, m, false]);
    }
    // Authored text: the token's value and the history's commit message, byte for byte, to both.
    for (const request of [{ method: 'describe' }, { method: 'history' }]) {
      const mine = JSON.parse(await app.local(request)).ok, theirs = JSON.parse((await app.remote(writer, request)).text).ok;
      const pick = request.method === 'describe' ? o => o.meta.tokens.ink : o => o.commits[0].message;
      if (request.method === 'describe') expect(pick(mine)).toEqual([['help', authored]]);
      else expect(pick(mine)).toStartWith(`design at ${dir}`);
      expect(pick(theirs)).toEqual(pick(mine));
    }
  } finally {
    await app.doors.close();
    await app.wire.close();
    rmSync(dir, { recursive: true, force: true });
    rmSync(outside, { recursive: true, force: true });
  }
}, 600_000);

test('a design server that is down says so to a remote client without this machine\'s directories', async () => {
  const { homedir } = await import('node:os');
  const { realpathSync } = await import('node:fs');
  const dir = mkdtempSync(join(new URL('..', import.meta.url).pathname, 'target', 'tamber-down-'));
  // And a package's file outside the app and the home directory, as a stderr line might name it.
  const outside = mkdtempSync(join(tmpdir(), 'tamber-down-pkg-'));
  writeFileSync(join(outside, 'package.json'), '{"name":"@acme/far"}');
  writeFileSync(join(outside, 'index.contract'), 'x');
  // A design server that stops at once, saying the app's path as the real one does.
  const dying = () => {
    const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
    child.kill = () => {};
    setTimeout(() => { child.stderr.write(`contract tamber serve: another tamber server writes ${realpathSync(dir)} (it read ${realpathSync(outside)}/index.contract:2:1; no ${realpathSync(outside)}/dist/theme.tamber)\n`); child.exitCode = 1; child.emit('exit', 1); }, 20);
    return child;
  };
  let doors = null;
  const wire = tamberServer({ appDir: dir, spawnServe: dying, log: () => {}, remote: () => doors?.list() ?? [] });
  doors = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  try {
    for (let i = 0; i < 100 && !wire.down; i++) await new Promise(ok => setTimeout(ok, 20));
    expect(wire.down).toContain(realpathSync(dir));
    let token = null;
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    let text = '';
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'read', name: 'laptop' })));
    await wire.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
    const code = JSON.parse(text).ok.code;
    const port = Number(new URL(doors.info().origin).port);
    const paired = JSON.parse((await send(true, port, { method: 'POST', path: '/__tamber/pair', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code, name: 'laptop' }) })).text).ok.token;
    const headers = { 'x-exact-tamber-token': paired, 'content-type': 'application/json' };
    const subscribe = await send(true, port, { method: 'POST', path: '/__tamber/subscribe', headers, body: '{"client":"laptop"}' });
    // A long poll from now: nothing after its `after` but the server's end.
    const poll = await send(true, port, { method: 'POST', path: '/__tamber/events', headers, body: '{"wait":false}' });
    // One from the start: the `down` event, its message without the paths too.
    const all = await send(true, port, { method: 'POST', path: '/__tamber/events', headers, body: '{"after":0,"wait":false}' });
    expect(JSON.parse(all.text).events.find(e => e.event === 'down').message).toBe('contract tamber serve: another tamber server writes <app> (it read @acme/far/index.contract:2:1; no @acme/far/dist/theme.tamber)');
    const machine = new Set([dir, realpathSync(dir), homedir(), realpathSync(homedir()), outside, realpathSync(outside)]);
    for (const m of machine) expect(all.text.includes(m)).toBe(false);
    for (const r of [subscribe, poll]) {
      expect(r.status).toBe(503);
      expect(JSON.parse(r.text).error.message).toContain('<app>');
      for (const m of machine) expect(r.text.includes(m)).toBe(false);
      expect(r.text).toContain('@acme/far/index.contract:2:1');
    }
  } finally {
    await doors.close();
    await wire.close();
    rmSync(dir, { recursive: true, force: true });
    rmSync(outside, { recursive: true, force: true });
  }
});

// Round 2 (Astra 1 = Grok 1): a compile refusal names a linked package's file
// by its path; to a remote client that package, outside the app and the home
// directory, is named by its package's name.
test('a linked package outside the app whose design (under a missing dist/) is missing is named by its package to a remote client', async () => {
  const { homedir } = await import('node:os');
  const { realpathSync } = await import('node:fs');
  const { dir, outside } = await machineApp();
  // Its manifest names a design under a `dist/` that was never built: refused with that design's path.
  writeFileSync(join(outside, 'package.json'), '{"name":"@acme/linked","version":"1.0.0","exports":{".":"./index.contract"},"tamber":"dist/theme.tamber"}');
  const app = await pairedApp(dir);
  try {
    const writer = await app.pair('write');
    const machine = [...new Set([dir, realpathSync(dir), homedir(), realpathSync(homedir()), outside, realpathSync(outside)])];
    const local = JSON.parse(await app.local({ method: 'uses', token: 'ink' }));
    expect(JSON.stringify(local)).toContain(realpathSync(outside));
    let refused = 0;
    for (const request of [{ method: 'uses', token: 'ink' }, { method: 'explain', token: 'LinkedCard.inset' }, { method: 'describe' }]) {
      const r = await app.remote(writer, request);
      const answer = JSON.parse(r.text);
      // (An answer that is no refusal holds the authored path, which goes as written.)
      if (!answer.error) continue;
      refused += 1;
      if (request.method === 'uses') expect(r.text).toContain('@acme/linked/dist/theme.tamber');
      for (const m of machine) expect([request.method, m, r.text.includes(m)]).toEqual([request.method, m, false]);
    }
    expect(refused).toBeGreaterThan(0);
    // The local tool reads the path itself.
  } finally {
    await app.doors.close();
    await app.wire.close();
    rmSync(dir, { recursive: true, force: true });
    rmSync(outside, { recursive: true, force: true });
  }
}, 600_000);

// The path fix's last finding (theme2: refuse it): a package linked from
// outside the app and the home directory whose `package.json` has no `name`
// is a compile refusal, `contract-package-unnamed`, at the app's `use`,
// naming the package as the `use` does (`uses` says it). No answer to a remote client carries
// any of its path: not its directories, nor its place from the app
// (`../../…`) as an identity.
test('an unnamed package linked from far outside the app is refused, and no remote answer carries its path', async () => {
  const { homedir } = await import('node:os');
  const { realpathSync } = await import('node:fs');
  const { basename } = await import('node:path');
  const { dir, outside, authored } = await machineApp();
  writeFileSync(join(outside, 'package.json'), '{"version":"1.0.0","exports":{".":"./index.contract"}}');
  const app = await pairedApp(dir);
  try {
    const writer = await app.pair('write');
    const machine = [...new Set([dir, realpathSync(dir), homedir(), realpathSync(homedir()), outside, realpathSync(outside), basename(outside), '../'])];
    let refused = 0;
    for (const request of [{ method: 'hello' }, { method: 'describe' }, { method: 'explain', token: 'LinkedCard.inset' }, { method: 'resolve', token: 'LinkedCard.inset' }, { method: 'uses', token: 'ink' }]) {
      const r = await app.remote(writer, request);
      // (The design's authored `help` holds the app's path, and goes as written.)
      const text = r.text.replaceAll(JSON.stringify(authored).slice(1, -1), '');
      if (text.includes('the package `@acme/linked` is outside the app and its `package.json` has no `name`')) refused += 1;
      for (const m of machine) expect([request.method, m, text.includes(m)]).toEqual([request.method, m, false]);
    }
    expect(refused).toBeGreaterThan(0);
    // `uses` says why the app does not build, to the local tool too.
    expect(await app.local({ method: 'uses', token: 'ink' })).toContain('needs a name in its package.json');
  } finally {
    await app.doors.close();
    await app.wire.close();
    rmSync(dir, { recursive: true, force: true });
    rmSync(outside, { recursive: true, force: true });
  }
}, 600_000);

// tamber/1.17's batched resolve (the third pass's #5 B) through the TLS door, against the real design server: a paired
// read client's batch whose answers pass 1 MiB (a stream's reply cap, not a fetch's) is read whole (theme2's condition).
test('a batched resolve past 1 MiB through the TLS door: a paired read client reads it whole', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-remote-batch-'));
  // 180 families near the value bound (about 3.9 KB each, twice that in an answer): about 1.6 MB of answers.
  const stack = n => Array.from({ length: 550 }, (_, i) => `f${n}x${i}`).join(', ').slice(0, 3900).replace(/, [^,]*$/, '');
  const faces = Array.from({ length: 180 }, (_, n) => `face${n}`);
  writeFileSync(join(app, 'app.json'), '{"app": {"id": "dev.test.batch", "name": "Batch"}, "host": {"web": {}}}');
  writeFileSync(join(app, 'app.tamber'), `tamber t\n\ntheme\n  ink = #111111\n${faces.map((f, n) => `  ${f}: family = ${stack(n)}\n`).join('')}`);
  writeFileSync(join(app, 'app.contract'), `component App\n  view\n    column background-color="ink"\n${faces.map(f => `      text "x" font-family="${f}"\n`).join('')}`);
  const wire = tamberServer({ appDir: app, log: () => {} });
  const doors = await openRemote(wire, { remote: { host: '127.0.0.1', port: 0 } }, { say: () => {} });
  try {
    const port = Number(new URL(doors.info().origin).port);
    let token = null, text = '';
    await wire.handle({ method: 'GET', headers: {} }, { writeHead() {}, setHeader() {}, end(t) { token = JSON.parse(t).token; } }, new URL('http://127.0.0.1/__tamber/token'), { allowed: true, local: true });
    const req = Object.assign(new PassThrough(), { method: 'POST', headers: { 'x-exact-tamber-token': token }, socket: { remoteAddress: '127.0.0.1' } });
    queueMicrotask(() => req.end(JSON.stringify({ method: 'pair-code', scope: 'read', name: 'laptop' })));
    await wire.handle(req, { writeHead() {}, setHeader() {}, write(t) { text += t; }, end(t) { text += t ?? ''; } }, new URL('http://127.0.0.1/__tamber'), { allowed: true, local: true });
    const paired = JSON.parse((await send(true, port, { method: 'POST', path: '/__tamber/pair', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ code: JSON.parse(text).ok.code, name: 'laptop' }) })).text).ok.token;
    const tokens = ['ink', ...faces];
    const r = await send(true, port, { method: 'POST', path: '/__tamber', headers: { 'x-exact-tamber-token': paired, 'content-type': 'application/json' }, body: JSON.stringify({ method: 'resolve', tokens }) });
    expect(r.status, r.text?.slice(0, 300)).toBe(200);
    expect(r.text.length).toBeGreaterThan(1 << 20);
    const answers = JSON.parse(r.text).ok.answers;
    expect(answers.length).toBe(tokens.length);
    expect(answers.every(a => !a.error)).toBe(true);
    expect(answers.at(-1).tuples[0].family).toContain('f179x0');
  } finally { await doors.close(); await wire.close(); rmSync(app, { recursive: true, force: true }); }
}, 600_000);
