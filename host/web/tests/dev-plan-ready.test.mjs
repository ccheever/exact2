// A dev loop says 'plan ready' only once its server answers, the JS loop's
// and the wasm loop's alike: at least seven tests, the gate scripts and the
// agent take the line as the cue to connect. The JS loop said it at its
// first build, before it listened; on a loaded machine the gap grew, and
// the agent navigated to a refused connection and waited out its bound on
// Chrome's error page (dev-scheduler.test.mjs, 2026-10-29). Each loop here
// waits before it listens (the test-only EXACT_DEV_LISTEN_DELAY_MS), and
// the test connects the moment it reads the line. A loop whose port is
// taken says so and never says 'plan ready'.
import { afterEach, expect, test } from 'bun:test';
import { createServer } from 'node:net';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned, stopAll } from '../../../scripts/test-children.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
afterEach(() => stopAll());
const free = () => new Promise(ok => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => ok(port)); }); });

// The JS loop (Caltrain) and the wasm loop's own compiler (theme-fixture, no app.ts). The wasm loop's
// wait outlasts its first plan, so the plan comes while it is not listening yet.
const LOOPS = [
  { name: 'the JS dev loop', args: ['--app', 'caltrain'], delay: 3000 },
  { name: 'the wasm dev loop', args: ['--app', 'theme-fixture', '--wasm'], delay: 20000 },
];

// The loop on `port`, its output heard as it is said; `onReady` runs in the same turn as the line is read.
function devLoop(loop, port, onReady = () => {}) {
  const dist = mkdtempSync(join(tmpdir(), 'exact-plan-ready-dist-'));
  const env = { ...process.env, EXACT_WEB_DIST: dist, EXACT_TEST_TIMING: '1', EXACT_DEV_LISTEN_DELAY_MS: String(loop.delay) };
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), ...loop.args, '--port', String(port)], { cwd: ROOT, env, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '', ready = false;
  const exited = new Promise(ok => dev.once('exit', code => ok(code)));
  const hear = d => { said += d; if (!ready && /^plan ready/m.test(said)) { ready = true; onReady(); } };
  dev.stdout.on('data', hear);
  dev.stderr.on('data', hear);
  return { dev, exited, said: () => said, ready: () => ready, done: () => rmSync(dist, { recursive: true, force: true }) };
}

for (const loop of LOOPS) {
  test(`${loop.name} says 'plan ready' only once its server answers`, async () => {
    const port = await free();
    let answer = null;
    const run = devLoop(loop, port, () => { answer = fetch(`http://127.0.0.1:${port}/`, { headers: { accept: 'text/html' } }).then(r => r.status, e => e.code ?? e.message); });
    try {
      for (const end = Date.now() + 600_000; !run.ready();) {
        if (Date.now() > end || run.dev.exitCode !== null) throw new Error(`no 'plan ready': ${run.said().slice(-4000)}`);
        await Bun.sleep(50);
      }
      expect(await answer).toBe(200);
      expect(run.said()).toContain(`dev: TEST delay ${loop.delay} ms before listening (EXACT_TEST_TIMING=1)`);
    } finally { await run.dev.stop(15_000); run.done(); }
  }, 620_000);

  test(`${loop.name} whose port is taken says so and never says 'plan ready'`, async () => {
    const port = await free();
    const holder = createServer();
    await new Promise(ok => holder.listen(port, '127.0.0.1', ok));
    const run = devLoop(loop, port);
    try {
      const code = await Promise.race([run.exited, Bun.sleep(600_000).then(() => 'still running')]);
      expect(code).not.toBe(0);
      expect(run.said()).toContain(`cannot listen on 127.0.0.1:${port}: EADDRINUSE`);
      expect(run.said()).not.toMatch(/^plan ready/m);
    } finally { await run.dev.stop(15_000); run.done(); await new Promise(ok => holder.close(ok)); }
  }, 620_000);
}
