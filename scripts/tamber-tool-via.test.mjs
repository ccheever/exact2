// `tamber tool --via` while the dev loop starts (the design tool's third hands-on pass, #9; theme2's 21:06 ruling): no
// answer yet (refused: the loop on its first build; or its hello held: its design server starting) is waited for,
// saying why once per reason, up to the cap (EXACT_TAMBER_VIA_WAIT_MS under the test switch); an answer that is
// another app's fails at once. The loop here is a stub answering the token and hello as a design server does. The
// tool asks again every 50 ms (EXACT_TAMBER_VIA_EVERY_MS), and the test of a held hello gives it 500 ms
// (EXACT_TAMBER_HELLO_MS); the slow lane (EXACT_SLOW_TESTS=1) keeps the real 2 s and 120 s.
import { expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned } from './test-children.mjs';
import { ROOT, SLOW, TOOL_ORIGIN, lines, stop, toolTurns } from './design-tool-harness.mjs';
import { REAL } from './tamber-timing.mjs';

toolTurns();

const freePort = () => new Promise(done => { const s = createServer().listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => done(port)); }); });
const appDir = () => { const dir = mkdtempSync(join(tmpdir(), 'exact-tool-via-')); writeFileSync(join(dir, 'app.tamber'), '{}\n'); return dir; };
// A dev loop's design server, as far as the tool's probe goes.
const loop = (port, app) => Bun.serve({ port, hostname: '127.0.0.1', idleTimeout: 0, async fetch(req) {
  const path = new URL(req.url).pathname;
  if (path === '/__tamber/token') return Response.json({ token: 't' });
  if (path === '/__tamber' && req.method === 'POST') return Response.json({ ok: { app, epoch: 'e1' } });
  return new Response('not found', { status: 404 });
} });
const HELLO_MS = SLOW ? REAL.hello : 500, EVERY_MS = SLOW ? REAL.viaEvery : 50;
// The tool through `target`, with --via's wait cut to `waitMs` and its hello's to `helloMs` (test timing only)
const tool = (app, target, { waitMs = 600_000, helloMs = REAL.hello } = {}) => spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', target, '--no-open'], {
  cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'],
  env: { ...process.env, EXACT_TAMBER_TEST_TIMING: '1', EXACT_TAMBER_VIA_WAIT_MS: String(waitMs), EXACT_TAMBER_VIA_EVERY_MS: String(EVERY_MS), EXACT_TAMBER_HELLO_MS: String(helloMs) },
});
const errOf = child => { let said = ''; child.stderr.on('data', d => { said += d; }); return () => said; };
const exited = child => new Promise(done => child.once('exit', (code, signal) => done({ code, signal })));

test('a loop not listening yet is waited for, said once, then the tool goes through it', async () => {
  const app = appDir(), port = await freePort(), target = `http://127.0.0.1:${port}`;
  const child = tool(app, target), said = errOf(child);
  let server = null;
  try {
    // Allow for the tool's own build, which may be cold
    for (const end = Date.now() + 600_000; !said().includes('waiting for the dev loop') && child.exitCode === null && Date.now() < end;) await Bun.sleep(100);
    expect(said()).toContain(`tamber tool: waiting for the dev loop at ${target}: not listening yet (its first build can take minutes); ctrl-c to stop`);
    // Wait for three more asks, then check the reason is still said once
    const asked = () => said().split('asking the dev loop').length - 1, first = asked();
    for (const end = Date.now() + 60_000; asked() < first + 3 && child.exitCode === null && Date.now() < end;) await Bun.sleep(50);
    expect(asked()).toBeGreaterThanOrEqual(first + 3);
    expect(said().split('waiting for the dev loop').length - 1).toBe(1);
    server = loop(port, app);
    await lines(child).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 60_000);
  } finally {
    await stop(child); server?.stop(true); rmSync(app, { recursive: true, force: true });
  }
}, 700_000);

test('a loop whose design server is starting (its hello held) is waited for, then gone through', async () => {
  const app = appDir(), port = await freePort(), target = `http://127.0.0.1:${port}`;
  // Hold the first hello until the tool gives up on it (its request aborts), then answer the next
  let asked = 0;
  // Turn off the idle timeout: Bun's default 10 s would reset a held hello
  const server = Bun.serve({ port, hostname: '127.0.0.1', idleTimeout: 0, async fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/__tamber/token') return Response.json({ token: 't' });
    if (path === '/__tamber' && req.method === 'POST') { if (asked++ === 0 && !req.signal.aborted) await new Promise(gone => req.signal.addEventListener('abort', gone, { once: true })); return Response.json({ ok: { app, epoch: 'e1' } }); }
    return new Response('not found', { status: 404 });
  } });
  const child = tool(app, target, { helloMs: HELLO_MS }), said = errOf(child);
  try {
    // Allow for the tool's own build, which may be cold
    await lines(child).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000 + HELLO_MS);
    expect(said()).toContain(`tamber tool: waiting for the dev loop at ${target}: its design server is starting; ctrl-c to stop`);
    expect(said()).not.toContain('not listening yet');
  } finally {
    await stop(child); server.stop(true); rmSync(app, { recursive: true, force: true });
  }
}, 700_000 + HELLO_MS);

test('a loop that answers as another app fails at once, with no wait', async () => {
  const app = appDir(), other = appDir(), port = await freePort(), target = `http://127.0.0.1:${port}`;
  const server = loop(port, other);
  const child = tool(app, target), said = errOf(child);
  try {
    const { code } = await exited(child);
    expect(code, said()).toBe(1);
    expect(said()).toContain(`tamber tool: ${target} serves the design of ${other}, not ${app}'s`);
    expect(said()).not.toContain('waiting for the dev loop');
  } finally {
    server.stop(true); rmSync(app, { recursive: true, force: true }); rmSync(other, { recursive: true, force: true });
  }
}, 700_000);

test('past the cap, it says how long it waited and exits 1, nothing listening on the tool\'s port', async () => {
  const app = appDir(), port = await freePort(), target = `http://127.0.0.1:${port}`;
  const child = tool(app, target, { waitMs: 1500 }), said = errOf(child);
  try {
    const { code } = await exited(child);
    expect(code, said()).toBe(1);
    expect(said()).toMatch(new RegExp(`tamber tool: ${target.replace(/\./g, '\\.')} does not answer after \\d+ s, not `));
    expect(await fetch(`${TOOL_ORIGIN}/`).then(() => 'answered', () => 'nothing')).toBe('nothing');
  } finally {
    rmSync(app, { recursive: true, force: true });
  }
}, 700_000);
