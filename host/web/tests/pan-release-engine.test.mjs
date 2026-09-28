// `panrelease` end to end on the web (LLP 1057.001 §6), nothing mocked: the
// compiled wasm's tracker (motion ops 13/14), the glue, and real touch input
// through the agent's contact phases. A flick at a known speed releases at
// about that speed; the same distance dragged slowly releases near rest, and
// so does a contact that pauses (`tap hold`) before it lifts, as a finger's
// does on UIKit. Needs a web dist that links every capability:
//   EXACT_WEB_LINK=all EXACT_WEB_DIST=/tmp/exact-all-dist bun host/web/build.mjs caltrain-web
//   EXACT_MOTION_DIST=/tmp/exact-all-dist bun test host/web/tests/pan-release-engine.test.mjs
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { open } from '../../../scripts/agent.mjs';

const ROOT = resolve(new URL('../../..', import.meta.url).pathname);
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const dist = process.env.EXACT_MOTION_DIST;
const check = existsSync(chrome) && dist && existsSync(resolve(dist, 'app.wasm')) ? test : test.skip;

const SOURCE = `component App
  state x = 0
  state vx = 0
  state vy = 0
  state releases = 0
  action moved(dx: number, dy: number) writes x
    x = x + dx
  action released(sx: number, sy: number) writes vx, vy, releases
    vx = sx
    vy = sy
    releases = releases + 1
  view
    column width="100%" height="100%"
      box testId="card" width=300 height=300 background-color="#f06" touch-action="none" pan=moved panrelease=released
`;

check('a flick releases at its speed; a slow drag and a pause before lifting near rest', async () => {
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-panrelease-'));
  const plan = resolve(tmp, 'pan.plan');
  writeFileSync(resolve(tmp, 'pan.contract'), SOURCE);
  const built = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(tmp, 'pan.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  expect(built.status, built.stderr).toBe(0);
  const s = await open({ host: 'web', plan, webDist: dist });
  try {
    const drag = async (dx, ms, hold = 0) => {
      await s.tap('card', { down: true });
      await s.pointer('move', { dx, dy: 0, ms });
      if (hold) await s.pointer('hold', { ms: hold });
      await s.pointer('up');
      return (await s.state()).slots;
    };
    // 150 px in 60 ms: 2500 px/s.
    let slots = await drag(150, 60);
    expect(slots.x).toBe(150);
    expect(slots.releases).toBe(1);
    expect(slots.vx).toBeGreaterThan(1500);
    expect(slots.vx).toBeLessThan(3500);
    expect(Math.abs(slots.vy)).toBeLessThan(1);
    // The same distance back over 1.5 s: about 100 px/s.
    slots = await drag(-150, 1500);
    expect(slots.releases).toBe(2);
    expect(slots.vx).toBeLessThan(0);
    expect(slots.vx).toBeGreaterThan(-300);
    // A flick that stops for 200 ms before lifting has no throw left.
    slots = await drag(150, 60, 200);
    expect(slots.releases).toBe(3);
    expect(Math.abs(slots.vx)).toBeLessThan(1);
  } finally {
    await s.close();
    rmSync(tmp, { recursive: true, force: true });
  }
}, 120000);
