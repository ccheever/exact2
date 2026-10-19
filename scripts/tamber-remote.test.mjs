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
  expect(remoteArgs([])).toEqual({ remote: null, tunnel: null, plugin: false, hosts: [] });
  expect(remoteArgs(['--remote', '0.0.0.0:7443', '--trust-tunnel', '7444', '--allow-plugin-origin', '--allow-host', 'Tamber.Example.com'])).toEqual({ remote: { host: '0.0.0.0', port: 7443 }, tunnel: 7444, plugin: true, hosts: ['tamber.example.com'] });
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
    expect(wire.seen.at(-1)).toEqual({ path: '/__tamber/pair', access: { allowed: true, local: false, remote: true, tunnel: false, peer: '127.0.0.1' }, cookie: undefined, token: undefined, anyCookie: false });
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
    expect(got).toEqual({ path: '/page', cookie: '__Host-c=1', access: { allowed: true, local: false, remote: true, tunnel: false, peer: '127.0.0.1', port: 'canvas' } });
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
    // Not the loop's otherwise: a rebound name, another path, another origin go to its gate.
    expect((await send(false, port, { method: 'POST', path: '/__tamber', headers: { origin: 'null', host: 'evil.example:1' } })).status).toBe(421);
    // The token route is the gate's as ever, and carries no CORS: a sandboxed page cannot read it.
    const tokenRoute = await send(false, port, { path: '/__tamber/token', headers: { origin: 'null' } });
    expect(tokenRoute.headers['access-control-allow-origin']).toBeUndefined();
    expect(reached.every(u => ['/__tamber', '/__tamber/pair'].includes(u))).toBe(true);
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
