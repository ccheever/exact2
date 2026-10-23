// A design server request over its 1 MiB is answered 413, not cut: the server's own wire and `tamber tool --via`'s
// proxy (scripts/tamber-server.mjs `forward`) answer first and close the connection after (tamber-request.mjs
// `refuseBody`, the canvas gateway's too), where they once cut it and wrote the answer to nobody (QUEUE "`tamber tool
// --via`'s proxy drops a 413 before it is sent").
import { afterAll, beforeAll, expect, jest, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer, request } from 'node:http';
import { connect } from 'node:net';
import { EventEmitter } from 'node:events';
import { earlyAnswer, refuseBody } from './tamber-request.mjs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { developmentGate } from './install-page.mjs';
import { forward, tamberServer } from './tamber-server.mjs';

const OVER = (1 << 20) + 1;
const REFUSED = { status: 413, body: JSON.stringify({ error: { message: 'a request is at most 1 MiB' } }), connection: 'close', closed: true };
// The local token's limit (a CLI import's sources, tamber/1.17).
const LOCAL_OVER = (16 << 20) + 1;
const LOCAL_REFUSED = { ...REFUSED, body: JSON.stringify({ error: { message: 'a request is at most 16 MiB' } }) };

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

/** Headers announcing a body over the limit, and not a byte of it: what comes back, and when the socket closed. */
const announce = (port, extra = '', length = OVER) => new Promise(done => {
  const sock = connect(port, '127.0.0.1');
  let got = '';
  const began = performance.now();
  sock.on('data', d => { got += d; });
  sock.on('close', () => done({ got, ms: performance.now() - began }));
  sock.write(`POST /__tamber HTTP/1.1\r\nhost: 127.0.0.1:${port}\r\ncontent-type: application/json\r\n${extra}content-length: ${length}\r\n\r\n`);
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

test("the design server answers the local token's body over 16 MiB 413, announced or chunked, and still answers the next request", async () => {
  const headers = { 'x-exact-tamber-token': token };
  for (const chunked of [false, true]) {
    const r = await send(`${origin}/__tamber`, LOCAL_OVER, { headers, chunked });
    expect({ chunked, ...r }).toEqual({ chunked, ...LOCAL_REFUSED });
  }
  const hello = await fetch(`${origin}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', ...headers }, body: '{"method":"hello"}' });
  expect(hello.status).toBe(200);
}, 60_000);

test("a client's transaction past 1024 operations is refused with tamber-ops-size before any is read; 1024 are not refused for their count", async () => {
  const headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  const op = { op: 'set-token', name: 'accent', value: '#123456' };
  for (const method of ['transaction', 'propose']) {
    const over = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify({ method, client: 'limit', ops: Array(1025).fill(op) }) });
    expect({ method, status: over.status, body: await over.json() }).toEqual({ method, status: 400, body: { error: { id: 'tamber-ops-size', message: 'a transaction is at most 1024 operations; this one has 1025' } } });
    const at = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify({ method, client: 'limit', ops: Array(1024).fill(op) }) });
    expect(JSON.stringify(await at.json())).not.toContain('tamber-ops-size');
  }
}, 120_000);

test("a local client that announces a body over its 16 MiB and sends none of it is answered 413 and let go", async () => {
  const { got, ms } = await announce(Number(new URL(origin).port), `x-exact-tamber-token: ${token}\r\n`, LOCAL_OVER);
  expect(got).toStartWith('HTTP/1.1 413');
  expect(got.toLowerCase()).toContain('connection: close');
  expect(ms).toBeLessThan(7000); // at once here (Bun closes it), refuseBody's 5 s at most
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
    // Announced and sent none of: refused all the same, a GET's too.
    const announced = await announce(proxy.address().port);
    expect(announced.got).toStartWith('HTTP/1.1 413');
    const get = await new Promise(done => request(at, { method: 'GET', headers: { 'content-length': OVER } }, res => { res.resume(); res.on('end', () => done(res.statusCode)); }).on('error', e => done(e.code)).end());
    expect(get).toBe(413);
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

test("refuseBody lets a request go after 5 s when its answer never finishes, and leaves no timer behind a closed socket or an ended request", () => {
  const party = () => {
    const socket = new EventEmitter(), req = Object.assign(new EventEmitter(), { socket, destroyed: 0, paused: 0, resumed: 0, pause() { this.paused++; }, resume() { this.resumed++; }, destroy() { this.destroyed++; } });
    const res = Object.assign(new EventEmitter(), { headers: {}, setHeader(k, v) { this.headers[k] = v; } });
    return { socket, req, res };
  };
  jest.useFakeTimers();
  try {
    // An answer that never goes out: paused, then let go at 5 s and not before.
    const stuck = party();
    let answered = 0;
    refuseBody(stuck.req, stuck.res, () => answered++);
    expect({ answered, paused: stuck.req.paused, resumed: stuck.req.resumed, connection: stuck.res.headers.connection }).toEqual({ answered: 1, paused: 1, resumed: 0, connection: 'close' });
    jest.advanceTimersByTime(4999);
    expect(stuck.req.destroyed).toBe(0);
    jest.advanceTimersByTime(2);
    expect(stuck.req.destroyed).toBe(1);
    // Its socket closed first: the timer goes with it.
    const gone = party();
    refuseBody(gone.req, gone.res);
    gone.socket.emit('close');
    jest.advanceTimersByTime(6000);
    expect(gone.req.destroyed).toBe(0);
    // Answered: resumed, the rest read and dropped, let go when it ends, and the timer gone with it.
    const done = party();
    refuseBody(done.req, done.res);
    done.res.emit('finish');
    expect({ resumed: done.req.resumed, destroyed: done.req.destroyed }).toEqual({ resumed: 1, destroyed: 0 });
    done.req.emit('data', Buffer.alloc(10));
    done.req.emit('end');
    expect(done.req.destroyed).toBe(1);
    jest.advanceTimersByTime(6000);
    expect(done.req.destroyed).toBe(1);
  } finally { jest.useRealTimers(); }
});

test("a remote door's body read while the server starts: over the limit refused before it is ready, a client gone answered nothing, a body or a ready server goes on", async () => {
  const never = new Promise(() => {});
  expect(await earlyAnswer(Promise.resolve(null), never)).toBe('over');
  expect(await earlyAnswer(Promise.resolve(undefined), never)).toBe('gone');
  expect(await earlyAnswer(Promise.resolve(''), never)).toBe('go'); // an empty body is a body
  expect(await earlyAnswer(never, Promise.resolve())).toBe('go'); // ready first: the body is read after
  // Ready and refused both settled: the refusal (already fulfilled) is said.
  const rejected = Promise.reject(new Error('no compiler'));
  rejected.catch(() => {});
  expect(await earlyAnswer(Promise.resolve(null), rejected)).toBe('over');
});
