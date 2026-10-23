// A request sent on to a design server or dev loop goes on a connection of its own (tamber-request.mjs `upstream`):
// `tamber tool --via`'s proxy (scripts/tamber-server.mjs `forward`) and the tool tests' proxy
// (design-tool-harness.mjs `proxied`) reused pooled ones, and one the server has just closed as idle meets a reset:
// the likely cause of "commit refuses …"'s one ECONNRESET from the proxied `tamber serve` under load (QUEUE
// 2026-10-22). The server here resets any reused connection, so the reuse is certain.
import { expect, test } from 'bun:test';
import { createServer } from 'node:http';
import { createServer as tcpServer } from 'node:net';
import { upstream } from './tamber-request.mjs';
import { forward } from './tamber-server.mjs';

/** A keep-alive server that resets a connection the moment a second request comes on it, unanswered: the race
 * a client loses when it sends on a socket the server closed as idle a moment before, made certain. */
async function closesReused() {
  let connections = 0;
  const server = tcpServer(socket => {
    connections++;
    let served = 0, buf = '';
    socket.on('error', () => {});
    socket.on('data', d => {
      buf += d;
      const head = buf.indexOf('\r\n\r\n');
      if (head < 0) return;
      if (served > 0) { socket.resetAndDestroy(); return; }
      const length = Number(/content-length: *(\d+)/i.exec(buf.slice(0, head))?.[1] ?? 0);
      if (buf.length < head + 4 + length) return;
      served++; buf = '';
      socket.write('HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 11\r\nconnection: keep-alive\r\n\r\n{"ok":true}');
    });
  });
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  return { origin: `http://127.0.0.1:${server.address().port}`, connections: () => connections, close: () => server.close() };
}

test('a request sent on goes on a connection of its own, so none meets a server that closed it as idle', async () => {
  const loop = await closesReused();
  try {
    const replies = [];
    for (let i = 0; i < 3; i++) replies.push(await (await upstream(`${loop.origin}/__tamber`, { method: 'POST', body: '{}' })).text());
    expect(replies).toEqual(Array(3).fill('{"ok":true}'));
    expect(loop.connections()).toBe(3);
  } finally { loop.close(); }
});

test("a client's `connection: keep-alive` passed on leaves no socket in the pool for a later request to meet closed", async () => {
  const loop = await closesReused();
  try {
    const sent = await upstream(`${loop.origin}/__tamber`, { method: 'POST', headers: { connection: 'keep-alive', 'content-type': 'application/json' }, body: '{}' });
    expect(await sent.text()).toBe('{"ok":true}');
    // A plain fetch after it, as the tool tests make beside their proxy: a connection of its own, not that one.
    expect(await (await fetch(`${loop.origin}/__tamber`, { method: 'POST', body: '{}' })).text()).toBe('{"ok":true}');
    expect(loop.connections()).toBe(2);
  } finally { loop.close(); }
});

test("--via's proxy sends each request on to the dev loop on a connection of its own: a second is answered, not 502", async () => {
  const loop = await closesReused();
  const proxy = createServer((req, res) => forward(req, res, new URL(req.url, 'http://x'), { local: true }, loop.origin));
  await new Promise(ok => proxy.listen(0, '127.0.0.1', ok));
  try {
    const answers = [];
    for (let i = 0; i < 3; i++) {
      const res = await fetch(`http://127.0.0.1:${proxy.address().port}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}' });
      answers.push([res.status, await res.text()]);
    }
    expect(answers).toEqual(Array(3).fill([200, '{"ok":true}']));
  } finally {
    proxy.close();
    loop.close();
  }
});
