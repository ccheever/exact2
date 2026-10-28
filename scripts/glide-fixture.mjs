// `clock settle` waits for a scroller gliding on its own after an input (a fling's glide), 4 s at most, as a native
// host waits for a scroll view a finger left moving: the app and the drive host/web-js/glide-settle.test.mjs (the JS
// target) and host/web/glide-settle.test.mjs (wasm) run. One list, its `scroll=` handler keeping the scroll top
// it was told (`y`) and counting the events.
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const CONTRACT = `component GlideSettle
  state y = 0
  state events = 0
  action scrolled(x: number, top: number)
    y = top
    events = events + 1
  view
    column height="100%" min-height=0
      list testId="feed" scroll=scrolled flex=1 min-height=0
        for i in range(60) key=i
          text \`row \${i}\` height=60
`;

/** The app in a fresh directory under `root` (target/). */
export function glideApp(root) {
  const dir = mkdtempSync(resolve(root, 'target/glide-settle-test-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Glide Settle', app: { id: 'test.glidesettle', name: 'Glide Settle' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), CONTRACT);
    writeFileSync(resolve(dir, 'app.ts'), `export const appId = 'test.glidesettle';
export const grants = '';
export function answer(name: string): any { throw new Error('no source ' + name); }
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
}

// `bun scripts/glide-fixture.mjs native <dir>`: the app as an `exact new` app at `dir` (outside the repo), for the
// wasm build host/web/glide-settle.test.mjs runs against.
if (import.meta.main) {
  const [verb, dir] = process.argv.slice(2);
  if (verb !== 'native' || !dir) { console.error('usage: bun scripts/glide-fixture.mjs native <dir>'); process.exit(2); }
  const { spawnSync } = await import('node:child_process');
  const { cpSync } = await import('node:fs');
  const root = resolve(import.meta.dir, '..');
  const made = spawnSync(process.execPath, ['scripts/exact.mjs', 'new', resolve(dir), '--name', 'Glide Settle', '--bundle-id', 'test.glidesettle'], { cwd: root, stdio: 'inherit' });
  if (made.status) process.exit(made.status);
  const app = glideApp(root);
  for (const f of ['app.contract', 'app.ts']) cpSync(resolve(app, f), resolve(dir, f));
  rmSync(app, { recursive: true, force: true });
  rmSync(resolve(dir, 'app.test.contract'), { force: true });
  console.log(`${dir}: build its wasm dist with (cd ${dir} && EXACT_APP_DIR=${resolve(dir)} EXACT_WEB_DIST=<dist> bun ${root}/host/web/build.mjs <its name>-web --wasm)`);
}
