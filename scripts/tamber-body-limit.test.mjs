// A design server request over its 1 MiB is answered 413, not cut: the server's own wire and `tamber tool --via`'s
// proxy (scripts/tamber-server.mjs `forward`) answer first and close the connection after (tamber-request.mjs
// `refuseBody`, the canvas gateway's too), where they once cut it and wrote the answer to nobody (QUEUE "`tamber tool
// --via`'s proxy drops a 413 before it is sent").
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer, request } from 'node:http';
import { connect } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { forward, tamberServer } from './tamber-server.mjs';

const OVER = (1 << 20) + 1;
const REFUSED = { status: 413, body: JSON.stringify({ error: { message: 'a request is at most 1 MiB' } }), connection: 'close', closed: true };

/** A POST of `size` bytes: by content-length, or chunked (`chunked`); its status, body and `connection` header,
 * and whether its socket then closed (within 6 s), or `{cut}` when the connection ended with no answer. */
const send = (url, size, { headers = {}, chunked = false } = {}) => new Promise(done => {
  const req = request(url, { method: 'POST', headers: { 'content-type': 'application/json', ...headers, ...(chunked ? { 'transfer-encoding': 'chunked' } : { 'content-length': size }) } }, res => {
    const chunks = [];
    res.on('data', c => chunks.push(c));
    res.on('end', () => {
      const answer = { status: res.statusCode, body: Buffer.concat(chunks).toString('utf8'), connection: res.headers.connection };
      // A refused request's connection is closed, never left paused for the keep-alive pool.
      if (res.socket.destroyed) return done({ ...answer, closed: true });
      const t = setTimeout(() => done({ ...answer, closed: false }), 6000);
      res.socket.once('close', () => { clearTimeout(t); done({ ...answer, closed: true }); });
    });
    res.on('error', e => done({ cut: e.code ?? e.message }));
  });
  req.on('error', e => done({ cut: e.code ?? e.message }));
  // Written in pieces, as a client sends a large body; a refused one stops being read.
  const piece = Buffer.alloc(64 << 10, 97);
  let left = size;
  const more = () => { while (left > 0) { const n = Math.min(left, piece.length); left -= n; if (!req.write(piece.subarray(0, n))) return req.once('drain', more); } req.end(); };
  more();
});

let dir, server, tamber, origin, token;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-limit-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber 1\n\ntoken accent = #3366ff\n');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port, gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 250 });
  server.on('request', async (req, res) => { if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); } });
  origin = `http://127.0.0.1:${port}`;
  ({ token } = await (await fetch(`${origin}/__tamber/token`)).json());
  await tamber.ready;
}, 600_000); // a cold checkout builds the compiler first
afterAll(async () => {
  await tamber?.close();
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});

test("the design server answers a body over 1 MiB 413, announced or chunked, and still answers the next request", async () => {
  const headers = { 'x-exact-tamber-token': token };
  for (const chunked of [false, true]) {
    const r = await send(`${origin}/__tamber`, OVER, { headers, chunked });
    expect({ chunked, ...r }).toEqual({ chunked, ...REFUSED });
  }
  const hello = await fetch(`${origin}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', ...headers }, body: '{"method":"hello"}' });
  expect(hello.status).toBe(200);
}, 60_000);

test('a client that announces a body over 1 MiB and then sends nothing is answered 413 and let go (at once here; 5 s at most)', async () => {
  const sock = connect(new URL(origin).port, '127.0.0.1');
  let got = '';
  sock.on('data', d => { got += d; });
  const began = performance.now();
  const closed = new Promise(ok => sock.on('close', () => ok(performance.now() - began)));
  sock.write(`POST /__tamber HTTP/1.1\r\nhost: 127.0.0.1:${new URL(origin).port}\r\ncontent-type: application/json\r\nx-exact-tamber-token: ${token}\r\ncontent-length: ${OVER}\r\n\r\n{"method"`);
  const ms = await closed;
  expect(got).toStartWith('HTTP/1.1 413');
  expect(got.toLowerCase()).toContain('connection: close');
  expect(ms).toBeLessThan(7000);
}, 30_000);

test("--via's proxy answers a body over 1 MiB 413 and sends nothing on; a body within it goes on byte for byte", async () => {
  // The dev loop's stand-in: every connection to it counted, every body kept.
  const seen = [];
  let connections = 0;
  const loop = createServer((req, res) => {
    const chunks = [];
    req.on('data', c => chunks.push(c));
    req.on('end', () => { const bytes = Buffer.concat(chunks); seen.push(bytes); res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify({ ok: { bytes: bytes.length } })); });
  });
  loop.on('connection', () => connections++);
  await new Promise(ok => loop.listen(0, '127.0.0.1', ok));
  const proxy = createServer((req, res) => forward(req, res, new URL(req.url, 'http://x'), { local: true }, `http://127.0.0.1:${loop.address().port}`));
  await new Promise(ok => proxy.listen(0, '127.0.0.1', ok));
  const at = `http://127.0.0.1:${proxy.address().port}/__tamber`;
  try {
    for (const chunked of [false, true]) {
      expect({ chunked, ...(await send(at, OVER, { chunked })) }).toEqual({ chunked, ...REFUSED });
    }
    // Refused before anything was opened upstream, an announced length before a byte of it was read.
    expect({ connections, seen }).toEqual({ connections: 0, seen: [] });
    // Not text: bytes that are no UTF-8 reach the loop as they were sent.
    const body = Uint8Array.from([0x7b, 0xff, 0xfe, 0x00, 0xc3, 0x7d]);
    const ok = await fetch(at, { method: 'POST', headers: { 'content-type': 'application/json' }, body });
    expect(await ok.json()).toEqual({ ok: { bytes: body.length } });
    expect([...seen[0]]).toEqual([...body]);
  } finally {
    proxy.close();
    loop.close();
  }
}, 60_000);
