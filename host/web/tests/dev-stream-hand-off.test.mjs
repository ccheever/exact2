// The wasm dev loop's page stream (host/web/dev.mjs /__dev) ends with its client when the request came on a socket
// handed to the loop's server (the ghost builds' follow-up): under Bun such a request never fires req 'close', so the
// stream's res 'close' backstop is what ends it. The hand-off is host/web-js/hand-off.preload.mjs's, a test's only.
import { afterEach, expect, test } from 'bun:test';
import { connect, createServer } from 'node:net';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned, stopAll } from '../../../scripts/test-children.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
afterEach(() => stopAll());
const free = () => new Promise(ok => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => ok(port)); }); });

test('a wasm page stream on a socket handed to the loop\'s server ends with its client, by the res close backstop alone', async () => {
  const dist = mkdtempSync(join(tmpdir(), 'exact-wasm-hand-off-dist-'));
  const port = await free(), handOff = await free();
  const dev = spawnOwned(process.execPath, ['--preload', resolve(ROOT, 'host/web-js/hand-off.preload.mjs'), resolve(ROOT, 'host/web/dev.mjs'), '--app', 'theme-fixture', '--wasm', '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist, EXACT_TEST_HAND_OFF: `${port}:${handOff}` }, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '', socket, heard = false;
  // (heard in what has come so far, so a line split across chunks still counts)
  const hear = d => { said = (said + d).slice(-8000); heard ||= /plan ready:|module generation ready/.test(said); };
  dev.stdout.on('data', hear);
  dev.stderr.on('data', hear);
  const until = async (pass, ms, what) => { const end = Date.now() + ms; while (!(await pass())) { if (Date.now() > end || dev.exitCode !== null) throw new Error(`${what}: not seen in ${said.slice(-2000)}`); await Bun.sleep(100); } };
  const base = `http://127.0.0.1:${port}`;
  const builds = async () => {
    const { token } = await (await fetch(`${base}/__tamber/token`)).json();
    const hello = await (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    return hello.ok.builds.length;
  };
  try {
    await until(() => said.includes(`127.0.0.1:${port}/`) && heard, 900_000, 'the first generation');
    // The page's stream, by the hand-off: its hello gives the nonce a build registers with.
    let got = '';
    socket = connect(handOff, '127.0.0.1', () => socket.write(`GET /__dev HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nAccept: text/event-stream\r\n\r\n`));
    socket.on('data', d => { got += d; });
    socket.on('error', () => {});
    await until(() => /"nonce":"[0-9a-f]+"/.test(got), 30_000, 'the stream\'s hello');
    const nonce = /"nonce":"([0-9a-f]+)"/.exec(got)[1];
    const registered = await fetch(`${base}/__dev/report`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ nonce, kind: 'register', id: 'handed-1', host: 'web', capabilities: [] }) });
    expect(registered.status).toBeLessThan(300);
    await until(async () => (await builds()) === 1, 10_000, 'the handed-off page\'s build');
    socket.destroy();
    await until(async () => (await builds()) === 0, 10_000, 'the handed-off page\'s build gone');
  } finally {
    socket?.destroy();
    await dev.stop(30_000);
    rmSync(dist, { recursive: true, force: true });
  }
}, 1_200_000);
