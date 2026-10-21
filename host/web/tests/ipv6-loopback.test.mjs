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

test("a stream over [::1] ends when its client leaves, as over 127.0.0.1; the [::1] server closes with the loop", async () => {
  // A page's event stream (the dev loop's /__dev/page): ended on the request's `close`, which a socket handed to the
  // HTTP server lost under Bun, so a page the design tool framed at localhost stayed connected for ever.
  const ended = { four: 0, six: 0 }, opened = { four: 0, six: 0 };
  const server = createServer((req, res) => {
    const which = req.socket.remoteAddress === '::1' ? 'six' : 'four';
    opened[which]++;
    res.writeHead(200, { 'content-type': 'text/event-stream' });
    res.write('data: hi\n\n');
    req.on('close', () => { ended[which]++; });
  });
  const port = await listening(server, 0, '127.0.0.1');
  const six = await alsoOnIPv6Loopback(server, port);
  const leave = (host) => new Promise((ok) => { const r = request({ host, port, path: '/', headers: { host: `localhost:${port}` } }, (res) => res.once('data', () => { r.destroy(); ok(); })); r.on('error', () => {}); r.end(); });
  const until = async (pass) => { for (let i = 0; i < 100 && !pass(); i++) await new Promise((ok) => setTimeout(ok, 20)); };
  await leave('127.0.0.1');
  await until(() => ended.four === 1);
  if (six) { await leave('::1'); await until(() => ended.six === 1); }
  assert.deepEqual({ opened, ended }, six ? { opened: { four: 1, six: 1 }, ended: { four: 1, six: 1 } } : { opened: { four: 1, six: 0 }, ended: { four: 1, six: 0 } });
  // A stream still open over [::1] when the loop stops: the loop's close ends it too, and [::1] stops listening.
  if (six) {
    await new Promise((ok) => { const r = request({ host: '::1', port, path: '/', headers: { host: `localhost:${port}` } }, (res) => res.once('data', ok)); r.on('error', () => {}); r.end(); });
    server.closeAllConnections?.();
    await new Promise((ok) => server.close(ok));
    await until(() => ended.six === 2);
    assert.equal(ended.six, 2);
    assert.equal(await freeOnIPv6Loopback(port), true);
  } else await new Promise((ok) => server.close(ok));
});
