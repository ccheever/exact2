import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { createServer, request } from 'node:http';
import { createServer as createSocketServer } from 'node:net';
import { alsoOnIPv6Loopback, freeOnIPv6Loopback } from '../serve.mjs';

const listening = (server, port, host) => new Promise((ok, fail) => { server.once('error', fail); server.listen(port, host, () => ok(server.address().port)); });
const get = (host, port) => new Promise((ok, fail) => request({ host, port, path: '/', headers: { host: `localhost:${port}` } }, (res) => { let body = ''; res.on('data', (d) => { body += d; }); res.on('end', () => ok(body)); }).on('error', fail).end());

test('a dev loop answers on [::1] too, and refuses a port another server holds there', async () => {
  // Its connections there are the HTTP server's own.
  const server = createServer((req, res) => res.end(`seen ${req.socket.remoteAddress}`));
  const port = await listening(server, 0, '127.0.0.1');
  assert.equal(await freeOnIPv6Loopback(port), true);
  const six = await alsoOnIPv6Loopback(server, port);
  assert.equal(await get('127.0.0.1', port), 'seen 127.0.0.1');
  // No IPv6 here: the loop serves IPv4 alone, which is fine.
  if (six) assert.equal(await get('::1', port), 'seen ::1');
  await new Promise((ok) => server.close(ok));
  if (!six) return;
  // Another server on [::1] at that port: a localhost URL would reach it.
  const other = createSocketServer();
  const held = await listening(other, 0, '::1');
  assert.equal(await freeOnIPv6Loopback(held), false);
  const second = createServer();
  await listening(second, held, '127.0.0.1');
  await assert.rejects(alsoOnIPv6Loopback(second, held), { code: 'EADDRINUSE' });
  await new Promise((ok) => second.close(ok));
  await new Promise((ok) => other.close(ok));
});
