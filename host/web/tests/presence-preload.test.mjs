// The wasm host preloads presence-glue.js where the plan names a presence row (glue.js after boot, navigation.js
// `preload`, `exact_presence`), so a view whose bound layout transition declares nothing at boot slides from where it
// was when it first gains one as it moves, as the JS target's does (host/web-js/layout-transition.test.mjs). Before,
// the module came with the first batch that carried the row, after that batch had moved the view. It needs a wasm
// Caltrain dist (EXACT_WEB_DIST), as accessibility-tree.test.mjs does; the plan is swapped in.
import { test, expect } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { open, assertWebDistApp } from '../../../scripts/agent.mjs';
import { chromium, refuseStale, webChanges } from '../../../scripts/agent-launch.mjs';
import { resolveApp } from '../../../scripts/app.mjs';
import { jsTargetBuild } from '../serve.mjs';
import { need, notApplicable } from '../../../scripts/coverage.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const app = resolveApp('caltrain');
const wasmDist = resolve(process.env.EXACT_WEB_DIST ?? resolve(ROOT, 'host/web/dist'));

async function prerequisite() {
  const { unavailable } = chromium();
  if (unavailable) return ['chrome', unavailable];
  try {
    await assertWebDistApp(wasmDist, app);
    if (jsTargetBuild(wasmDist)) return ['fixture', `${wasmDist} is a JS-target build; this test reads the wasm target (bun host/web/build.mjs caltrain-web --wasm)`];
    refuseStale('web', resolve(wasmDist, '.exact-build.json'), webChanges(wasmDist, app).all, 'bun host/web/build.mjs caltrain-web --wasm');
  } catch (error) { return ['fixture', error.message]; }
  return null;
}
const missing = process.env.EXACT_GLUE_FAST === '1' ? null : await prerequisite();
const check = process.env.EXACT_GLUE_FAST === '1' ? notApplicable('the async glue step does not build apps; the web-ax-test step runs this test')
  : need(...(missing ?? ['chrome', null]));

// Calm at boot: the row says `none`, so the boot batch carries no presence row.
const SOURCE = `component App
  state calm = true
  state shown = true
  action both
    calm = ! calm
    shown = ! shown
  view
    column
      button press=both testId="both"
        text "Both"
      if shown
        text "First" testId="first" height=40
      text "Second" testId="second" height=40 -exact-layout-transition=(calm ? "none" : "2000ms linear")
`;

check('a view that first gains its bound layout transition as it moves slides from where it was', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-presence-preload-'));
  try {
    writeFileSync(resolve(dir, 'app.contract'), SOURCE);
    const plan = resolve(dir, 'app.plan');
    const c = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(dir, 'app.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
    expect(c.status, c.stderr).toBe(0);
    const s = await open({ host: 'web', plan, webDist: wasmDist });
    try {
      // The view with the row, read from the painted surface (presence's observation).
      const second = async () => (await s.state()).presence?.map(p => p.y) ?? [];
      await s.tap('both'); // calm off and First gone: Second gains the curve as it moves up 40
      const [at] = await second();
      await s.clock('+1000');
      const [mid] = await second();
      await s.clock('settle');
      const [end] = await second();
      expect(end).toBeLessThan(at - 30); // it moved up
      expect(at - end).toBeCloseTo(40, 0); // and shows where it was at the tap
      expect(mid).toBeGreaterThan(end + 10); // halfway through the 2 s curve, still on its way
      expect(mid).toBeLessThan(at - 10);
    } finally { await s.close(); }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 120_000);
