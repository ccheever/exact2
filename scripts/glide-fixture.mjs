// `clock settle` waits for a scroller gliding on its own after an input (a fling's glide), 4 s at most, as a native
// host waits for a scroll view a finger left moving: the app and the drive host/web-js/glide-settle.test.mjs (the JS
// target) and host/web/glide-settle.test.mjs (wasm) run. One list, its `scroll=` handler keeping the scroll top
// it was told (`y`) and counting the events; two reads from a server of this machine (`glideServer`): `slow`,
// answered 4.5 s late, its `then` starting a fade (the settle round after it spends none of the 4 s a glide may
// take on the wait for the answer), and `near`, whose answer smooth-scrolls the list (a scroller a round's own work
// set moving is waited for too).
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const CONTRACT = `component GlideSettle
  state y = 0
  state events = 0
  state asks = 0
  state shown = false
  mutation late as shape number then landed
  mutation near as shape number then arrived
  action scrolled(x: number, top: number)
    y = top
    events = events + 1
  action slow
    send late = slowRead(asks)
  action jump
    send near = nearRead(asks)
  action landed
    shown = ! shown
  action arrived
    scrollIntoView("row-50", block="start", behavior="smooth")
  view
    column height="100%" min-height=0
      row
        button press=slow testId="slow"
          text "Slow"
        button press=jump testId="jump"
          text "Jump"
        text "landed" testId="landed" opacity=(shown ? 1 : 0) transition="opacity 0.3s linear"
      list testId="feed" scroll=scrolled flex=1 min-height=0
        for i in range(60) key=i
          text \`row \${i}\` id=\`row-\${i}\` height=60
`;

/** The server the app reads: `/slow` answers 4.5 s late, `/near` at once. `port`: a fixed one, for a native build. */
export function glideServer({ port = 0 } = {}) {
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*' };
  const server = Bun.serve({ hostname: '127.0.0.1', port, idleTimeout: 0, fetch(req) {
    if (req.method === 'OPTIONS') return new Response(null, { headers: cors });
    const path = new URL(req.url).pathname;
    if (path === '/slow') return new Promise(ok => setTimeout(() => ok(new Response('1', { headers: cors })), 4500));
    return new Response('1', { headers: cors });
  } });
  return { origin: `http://127.0.0.1:${server.port}`, stop: () => server.stop(true) };
}

/** The app in a fresh directory under `root` (target/), reading `origin`. */
export function glideApp(root, origin) {
  const dir = mkdtempSync(resolve(root, 'target/glide-settle-test-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Glide Settle', app: { id: 'test.glidesettle', name: 'Glide Settle' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), CONTRACT);
    writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.glidesettle';
export const grants = ${JSON.stringify(`net.fetch ${origin}`)};
const read = (path: string) => fetch(${JSON.stringify(origin)} + path).then(r => r.text()).then(Number);
export function answer(name: string): any {
  if (name === 'slowRead') return read('/slow');
  if (name === 'nearRead') return read('/near');
  throw new Error('no source ' + name);
}
`);
  } catch (e) {
    rmSync(dir, { recursive: true, force: true });
    throw e;
  }
  return dir;
}

/** The drive, on an open web session `s`: what each settle answered, how long it took, and what the page showed
 * then and a second later. */
export async function driveGlide(s) {
  const top = () => s.carrier.evaluate(`document.querySelector('[data-testid="feed"]').scrollTop`);
  const timed = async () => { const t0 = performance.now(); const r = await s.clock('settle'); return { r, ms: performance.now() - t0 }; };
  // (both rounded: a resting offset can be fractional)
  const seen = async () => ({ y: Math.round((await s.state()).slots?.y), top: Math.round(await top()) });
  const home = async () => { await s.carrier.evaluate(`document.querySelector('[data-testid="feed"]').scrollTop = 0`); await s.clock('settle'); };
  await s.clock('data');
  const out = {};
  // 1. A drag released with velocity: the glide ends before settle answers, and its last scroll is delivered.
  await s.tap('feed', { drag: { dx: 0, dy: -400 } });
  out.glide = await timed();
  out.glideAt = await seen();
  await Bun.sleep(1000);
  out.glideLater = await seen();
  // 2. A drag held at its end leaves nothing moving: settle answers at once.
  await home();
  await s.tap('feed', { drag: { dx: 0, dy: -400, hold: 300 } });
  out.held = await timed();
  out.heldAt = await seen();
  // 3. A scroller the page keeps moving on its own (an auto-scrolling list), begun from a frame's callbacks after a
  // quiet spell, so its first event comes a frame later: settle gives up at 4 s, reason `scroll`.
  await home();
  await Bun.sleep(600);
  await s.carrier.evaluate(`(() => { const e = document.querySelector('[data-testid="feed"]'); globalThis.__glideStop = false;
    const step = () => { e.scrollTop = (e.scrollTop + 2) % 2000; if (!globalThis.__glideStop) requestAnimationFrame(step); }; requestAnimationFrame(step); })()`);
  out.auto = await timed();
  await s.carrier.evaluate('globalThis.__glideStop = true');
  // 4. A read answered 4.5 s late: settle waits for it, and none of that is a glide's 4 s.
  await home();
  await s.tap('slow');
  out.slow = await timed();
  // 5. An answer whose `then` smooth-scrolls the list: settle waits for the scroll its own round started.
  await home();
  await s.tap('jump');
  out.near = await timed();
  out.nearAt = await seen();
  await Bun.sleep(1000);
  out.nearLater = await seen();
  return out;
}

/** What every target's test asserts of `driveGlide`'s answer. */
export function expectGlide(out, expect) {
  expect(out.glide.r).toMatchObject({ settled: true });
  expect(out.glideAt.top).toBeGreaterThan(400);
  expect(out.glideAt.y).toBe(out.glideAt.top);
  expect(out.glideLater).toEqual(out.glideAt);
  expect(out.held.r).toMatchObject({ settled: true });
  expect(out.held.ms).toBeLessThan(2000);
  expect(out.heldAt.y).toBe(out.heldAt.top);
  expect(out.auto.r).toMatchObject({ settled: false, reason: 'scroll' });
  expect(out.auto.ms).toBeGreaterThan(3500);
  expect(out.auto.ms).toBeLessThan(8000);
  expect(out.slow.r).toMatchObject({ settled: true });
  expect(out.slow.ms).toBeGreaterThan(4000);
  expect(out.near.r).toMatchObject({ settled: true });
  expect(out.nearAt.top).toBeGreaterThan(1000);
  expect(out.nearLater).toEqual(out.nearAt);
}

// `bun scripts/glide-fixture.mjs native <dir> [port]`: the app as an `exact new` app at `dir` (outside the repo),
// reading 127.0.0.1:<port> (47713), for the wasm build host/web/glide-settle.test.mjs runs against.
if (import.meta.main) {
  const [verb, dir, port = '47713'] = process.argv.slice(2);
  if (verb !== 'native' || !dir) { console.error('usage: bun scripts/glide-fixture.mjs native <dir> [port]'); process.exit(2); }
  const { spawnSync } = await import('node:child_process');
  const { cpSync } = await import('node:fs');
  const root = resolve(import.meta.dir, '..');
  const made = spawnSync(process.execPath, ['scripts/exact.mjs', 'new', resolve(dir), '--name', 'Glide Settle', '--bundle-id', 'test.glidesettle'], { cwd: root, stdio: 'inherit' });
  if (made.status) process.exit(made.status);
  const app = glideApp(root, `http://127.0.0.1:${port}`);
  for (const f of ['app.contract', 'app.ts']) cpSync(resolve(app, f), resolve(dir, f));
  rmSync(app, { recursive: true, force: true });
  rmSync(resolve(dir, 'app.test.contract'), { force: true });
  console.log(`${dir}: build its wasm dist with (cd ${dir} && EXACT_APP_DIR=${resolve(dir)} EXACT_WEB_DIST=<dist> bun ${root}/host/web/build.mjs <its name>-web --wasm)`);
}
