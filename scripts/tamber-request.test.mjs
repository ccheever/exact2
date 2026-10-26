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
