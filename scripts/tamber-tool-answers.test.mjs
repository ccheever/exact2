// `tamber tool --via`'s probe of a dev loop (scripts/tamber-server.mjs
// `toolAnswers`): a token probe that times out (2 s) is tried again, so a dev
// loop under load that answers late is still reached; nothing listening is
// said at once, and a loop that never answers is refused after the tries
// (QUEUE "`tamber tool --via` gives a dev loop 2 s to answer").
import { expect, test } from 'bun:test';
import { createServer } from 'node:net';
import { toolAnswers } from './tamber-server.mjs';

const free = () => new Promise(ok => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => ok(port)); }); });

// A stand-in design server: its token answered after `late(n)` ms on the n-th ask, its hello at once.
const loop = late => {
  let asked = 0;
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/__tamber/token') { await Bun.sleep(late(++asked)); return Response.json({ token: 't' }); }
    if (path === '/__tamber') return Response.json({ ok: { app: '/apps/x', epoch: 'e1' } });
    return new Response('', { status: 404 });
  } });
  return { server, origin: `http://127.0.0.1:${server.port}`, asked: () => asked };
};

test('a dev loop that answers its first token probe late is reached on a later try; one probe alone is not', async () => {
  const slow = loop(n => (n === 1 ? 3000 : 0));
  try {
    expect(await toolAnswers(slow.origin, 5)).toEqual({ app: '/apps/x', epoch: 'e1' });
    expect(slow.asked()).toBe(2);
  } finally { slow.server.stop(true); }
  const once = loop(n => (n === 1 ? 3000 : 0));
  try { expect(await toolAnswers(once.origin)).toBe(null); } finally { once.server.stop(true); }
}, 30_000);

test('nothing listening is said at once, tries or not; a loop that never answers is refused after them', async () => {
  const port = await free(), t0 = Date.now();
  expect(await toolAnswers(`http://127.0.0.1:${port}`, 5)).toBe(null);
  expect(Date.now() - t0).toBeLessThan(1000);
  const mute = loop(() => 60_000), t1 = Date.now();
  try {
    expect(await toolAnswers(mute.origin, 3)).toBe(null);
    expect([mute.asked(), Date.now() - t1 >= 5900]).toEqual([3, true]);
  } finally { mute.server.stop(true); }
}, 60_000);
