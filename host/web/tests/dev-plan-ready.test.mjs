// A dev loop says 'plan ready' only once its server answers, the JS loop's
// and the wasm loop's alike: at least seven tests, the gate scripts and the
// agent take the line as the cue to connect. The JS loop said it at its
// first build, before it listened; on a loaded machine the gap grew, and
// the agent navigated to a refused connection and waited out its bound on
// Chrome's error page (dev-scheduler.test.mjs, 2026-10-29). Each loop here
// waits before it listens (the test-only EXACT_DEV_LISTEN_DELAY_MS), and
// the test connects the moment it reads the line (a wasm loop's module or
// Rust generation's ready line likewise). A loop whose port is taken says
// so and never says it is ready.
import { afterEach, expect, onTestFinished, test } from 'bun:test';
import { createServer } from 'node:net';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnOwned, stopAll } from '../../../scripts/test-children.mjs';
import { checkoutPort, takePort } from '../../../scripts/port-lock.mjs';
import { listenDelay } from '../serve.mjs';
import { resolveApp } from '../../../scripts/app.mjs';
import { canonicalBytes } from '../../../scripts/deploy.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
// A private dist each, removed after the test even when its bound ends it (no `finally` runs then), with
// the wasm loop's generation cache, which is keyed by the dist (dev.mjs generationCache), so no later run takes it.
const dists = new Set();
const generationCache = (name, dist) => { const app = resolveApp(name); return resolve(ROOT, 'target/dev-generations', createHash('sha256').update(canonicalBytes({ app: app.id, path: app.dir, dist })).digest('hex')); };
afterEach(() => {
  stopAll();
  for (const [name, dist] of dists) for (const dir of [dist, generationCache(name, dist)]) rmSync(dir, { recursive: true, force: true });
  dists.clear();
});
const free = () => new Promise(ok => { const s = createServer(); s.listen(0, '127.0.0.1', () => { const { port } = s.address(); s.close(() => ok(port)); }); });

// The JS loop (Caltrain), the wasm loop's own compiler (theme-fixture, no app.ts) and its TypeScript
// producer (Canvas Gallery). A wasm loop's wait (the most the knob takes) outlasts its first generation,
// so that comes while it is not listening yet, which `GENERATED` shows: its classification, said as it
// captures one. Without it a case would prove nothing, so it fails instead. theme-fixture's design server
// takes the app's .exact/tamber.lock: this checkout's turn on the fixture first, as design-tool-inspect's.
const LOOPS = [
  { name: 'the JS dev loop', args: ['--app', 'caltrain'], delay: 3000 },
  { name: 'the wasm dev loop', args: ['--app', 'theme-fixture', '--wasm'], delay: 60000, fixture: 'theme-fixture' },
  { name: 'the wasm dev loop on a TypeScript app', args: ['--app', 'canvas-gallery', '--wasm'], delay: 60000 },
];
// Its ready lines, as the resident loop and metrics.mjs hear them.
const READY = /^(?:plan ready|module generation ready|Rust generation \w+ ready)/m;
// Whether this machine has IPv6's loopback (a loop then answers there too: a localhost URL, the design tool's canvas).
const sixLoopback = new Promise(ok => { const s = createServer(); s.once('error', () => ok(false)); s.listen(0, '::1', () => s.close(() => ok(true))); });
const GENERATED = /^  (?:web|plan\/module\/assets): /m;
const atIndex = (said, pattern) => { const m = pattern.exec(said); return m ? m.index : -1; };
async function fixtureTurn(loop) {
  if (!loop.fixture) return;
  const app = resolve(ROOT, 'apps', loop.fixture);
  onTestFinished(await takePort(1000 + checkoutPort(app), { label: `dev-plan-ready on ${app}` }));
}

// The loop on `port`, its output heard as it is said; `onReady` runs in the same turn as the line is read.
function devLoop(loop, port, onReady = () => {}) {
  const dist = mkdtempSync(join(tmpdir(), 'exact-plan-ready-dist-'));
  dists.add([loop.args[1], dist]);
  const env = { ...process.env, EXACT_WEB_DIST: dist, EXACT_TEST_TIMING: '1', EXACT_DEV_LISTEN_DELAY_MS: String(loop.delay) };
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), ...loop.args, '--port', String(port)], { cwd: ROOT, env, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '', ready = false;
  // Once its output has all been read (a child's 'close' comes after its streams end).
  const exited = new Promise(ok => dev.once('close', code => ok(code)));
  // Each stream by its own lines, so another's chunk never splits a ready line; a last line unended is one too.
  for (const stream of [dev.stdout, dev.stderr]) {
    let part = '';
    const hear = lines => { if (!ready && lines.some(line => READY.test(line))) { ready = true; onReady(); } };
    stream.on('data', d => { said += d; part += d; const lines = part.split('\n'); part = lines.pop(); hear(lines); });
    stream.on('end', () => hear([part]));
  }
  return { dev, exited, said: () => said, ready: () => ready };
}

// The wait is a test's alone: without EXACT_TEST_TIMING=1, or out of its range, a loop listens at once.
test('the listen delay is read only under EXACT_TEST_TIMING=1, from 1 to 60000 ms', async () => {
  const took = async env => { const t = performance.now(); await listenDelay(env); return performance.now() - t; };
  for (const env of [{ EXACT_DEV_LISTEN_DELAY_MS: '1500' }, { EXACT_TEST_TIMING: 'true', EXACT_DEV_LISTEN_DELAY_MS: '1500' }, { EXACT_TEST_TIMING: '1', EXACT_DEV_LISTEN_DELAY_MS: '0' }, { EXACT_TEST_TIMING: '1', EXACT_DEV_LISTEN_DELAY_MS: '60001' }, { EXACT_TEST_TIMING: '1', EXACT_DEV_LISTEN_DELAY_MS: '4e2' }])
    expect(await took(env)).toBeLessThan(500);
  expect(await took({ EXACT_TEST_TIMING: '1', EXACT_DEV_LISTEN_DELAY_MS: '1500' })).toBeGreaterThanOrEqual(1490);
});

for (const loop of LOOPS) {
  test(`${loop.name} says 'plan ready' only once its server answers`, async () => {
    await fixtureTurn(loop);
    const port = await free();
    const six = await sixLoopback;
    let answers = null;
    const ask = host => fetch(`http://${host}:${port}/`, { headers: { accept: 'text/html' } }).then(r => r.status, e => e.code ?? e.message);
    const run = devLoop(loop, port, () => { answers = Promise.all([ask('127.0.0.1'), six ? ask('[::1]') : 0]); });
    try {
      for (const end = Date.now() + 600_000; !run.ready();) {
        if (Date.now() > end || run.dev.exitCode !== null) throw new Error(`no 'plan ready': ${run.said().slice(-4000)}`);
        await Bun.sleep(50);
      }
      // [::1] answers too: a status (421, its Host not a name the loop serves), not a refused connection.
      const [four, sixAnswer] = await answers;
      expect([four, typeof sixAnswer]).toEqual([200, 'number']);
      expect(run.said()).toContain(`dev: TEST delay ${loop.delay} ms before listening (EXACT_TEST_TIMING=1)`);
      if (loop.args.includes('--wasm')) expect(atIndex(run.said(), GENERATED), 'a generation captured before the loop listened').toBeWithin(0, run.said().indexOf(`http://127.0.0.1:${port}/`));
    } finally { await run.dev.stop(15_000); }
  }, 620_000);

  test(`${loop.name} whose port is taken says so and never says 'plan ready'`, async () => {
    await fixtureTurn(loop);
    const port = await free();
    const holder = createServer();
    await new Promise(ok => holder.listen(port, '127.0.0.1', ok));
    onTestFinished(() => holder.close());
    const run = devLoop(loop, port);
    try {
      const code = await Promise.race([run.exited, Bun.sleep(600_000).then(() => { throw new Error(`the loop never exited: ${run.said().slice(-4000)}`); })]);
      expect(typeof code === 'number' && code !== 0, `exit ${code}`).toBe(true);
      const refused = run.said().indexOf(`cannot listen on 127.0.0.1:${port}: EADDRINUSE`);
      expect(refused).toBeGreaterThan(-1);
      if (loop.args.includes('--wasm')) expect(atIndex(run.said(), GENERATED), 'a generation captured before the listen failed').toBeWithin(0, refused);
      expect(run.ready(), 'a ready line').toBe(false);
    } finally { await run.dev.stop(15_000); }
  }, 620_000);
}
