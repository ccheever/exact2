// readBody (scripts/tamber-request.mjs) when the client leaves mid-body on a
// TLS door's hand-off (dtool2's repro, review2's ghost-builds follow-up):
// under Bun, `req`'s close is lost on a socket a TLS server hands to the HTTP
// server with `emit('connection')`, so the body's read settles on its
// connection's close instead, and nothing it read is held.
import { expect, test } from 'bun:test';
import { createServer as createPlain } from 'node:http';
import { connect, createServer as createTLS } from 'node:tls';
import { readBody } from './tamber-request.mjs';
import { selfSignedCertificate } from './tamber-remote.mjs';

test('a body whose client left mid-way through a TLS door settles as gone, at once', async () => {
  const cert = selfSignedCertificate(['127.0.0.1']);
  let settled;
  const read = new Promise(ok => { settled = ok; });
  // (the door's shape: TLS accepts, HTTP parses, as tamber-remote.mjs's `secure`)
  let observed;
  const partial = new Promise(ok => { observed = ok; });
  // (the client leaves once the server has some of its body: no timing guessed)
  const http = createPlain(async req => { req.once('data', () => observed()); settled(await readBody(req, 1 << 20)); });
  const door = createTLS({ cert: cert.cert, key: cert.key }, socket => http.emit('connection', socket));
  await new Promise(ok => door.listen(0, '127.0.0.1', ok));
  try {
    const c = connect({ host: '127.0.0.1', port: door.address().port, rejectUnauthorized: false }, () => {
      c.write('POST /__tamber HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: 100000\r\n\r\n{"partial":');
      partial.then(() => c.destroy());
    });
    c.on('error', () => {});
    const got = await Promise.race([read, new Promise(ok => setTimeout(() => ok('still waiting'), 3000))]);
    expect(got).toBeUndefined();
  } finally { door.close(); }
});

test('a keep-alive connection\'s finished reads leave no close listener behind', async () => {
  const { PassThrough } = await import('node:stream');
  const { EventEmitter } = await import('node:events');
  const socket = new EventEmitter();
  for (let i = 0; i < 20; i++) {
    const req = Object.assign(new PassThrough(), { headers: {}, socket });
    const read = readBody(req, 1024);
    req.end('{}');
    expect(await read).toBe('{}');
  }
  expect(socket.listenerCount('close')).toBe(0);
});

test('the canvas port\'s own body reader settles when its connection closes, and leaves no listener behind once settled', async () => {
  const { PassThrough } = await import('node:stream');
  const { EventEmitter } = await import('node:events');
  const { readBody: canvasBody } = await import('./tamber-canvas.mjs');
  const socket = new EventEmitter();
  for (let i = 0; i < 20; i++) {
    const req = Object.assign(new PassThrough(), { socket });
    const read = canvasBody(req, 1024);
    req.end('ok');
    expect((await read).body.toString()).toBe('ok');
  }
  expect(socket.listenerCount('close')).toBe(0);
  // A body whose request's own close never comes: its connection's close settles it as gone.
  const req = Object.assign(new PassThrough(), { socket });
  const read = canvasBody(req, 1024);
  req.write('par');
  socket.emit('close');
  expect(await read).toEqual({ gone: true });
});

// `tamber tool --via`'s wait (the third pass's #9): what is "no answer yet", and the cap.
test('probeTool: refused is listening, a held hello is starting, a token or hello answered is an answer', async () => {
  const { probeTool } = await import('./tamber-request.mjs');
  const free = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch: () => new Response('') }), refusedAt = `http://127.0.0.1:${free.port}`;
  free.stop(true);
  expect(await probeTool(refusedAt)).toEqual({ wait: 'listening' });
  // The token answered, then the loop gone under its hello (a restart: the connection reset): still no answer yet.
  const half = Bun.serve({ port: 0, hostname: '127.0.0.1', idleTimeout: 0, async fetch(req) {
    if (new URL(req.url).pathname === '/__tamber/token') return Response.json({ token: 't' });
    setTimeout(() => half.stop(true), 50); await Bun.sleep(5000); return Response.json({});
  } });
  expect(await probeTool(`http://127.0.0.1:${half.port}`)).toEqual({ wait: 'listening' });
  // A hello answered with an error status, its body then cut off: an answer (no design server), never a wait.
  const erring = Bun.serve({ port: 0, hostname: '127.0.0.1', idleTimeout: 0, fetch(req) {
    if (new URL(req.url).pathname === '/__tamber/token') return Response.json({ token: 't' });
    const body = new ReadableStream({ start(c) { c.enqueue(new TextEncoder().encode('{"err')); setTimeout(() => c.error(new Error('cut')), 30); } });
    return new Response(body, { status: 404, headers: { 'content-type': 'application/json' } });
  } });
  try { expect(await probeTool(`http://127.0.0.1:${erring.port}`)).toEqual({ answer: { app: null } }); } finally { erring.stop(true); }
  // No design server (the token refused): an answer, `{app: null}`.
  const none = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch: () => new Response('', { status: 404 }) });
  try { expect(await probeTool(`http://127.0.0.1:${none.port}`)).toEqual({ answer: { app: null } }); } finally { none.stop(true); }
});

test('waitForAnswer: one line a reason, the first answer, and the cap bounding a probe in flight', async () => {
  const { waitForAnswer } = await import('./tamber-request.mjs');
  const said = [], answers = [{ wait: 'listening' }, { wait: 'listening' }, { wait: 'starting' }, { answer: { app: '/a' } }];
  expect(await waitForAnswer(async () => answers.shift(), { say: w => said.push(w), capMs: 10_000, everyMs: 5 })).toEqual({ answer: { app: '/a' } });
  expect(said).toEqual(['listening', 'starting']);
  // A probe still out at the cap (a hello held): the wait ends at the cap, its late answer not taken.
  const t0 = Date.now();
  const got = await waitForAnswer(() => new Promise(done => setTimeout(() => done({ answer: { app: '/late' } }), 600)), { say: () => {}, capMs: 100, everyMs: 5 });
  expect(got.waited).toBeGreaterThanOrEqual(90);
  expect(Date.now() - t0).toBeLessThan(500);
  // And no signal handler left behind.
  expect(process.listenerCount('SIGINT')).toBe(0);
});
