// @ref LLP 1116.003 D8 — a screenshot shows a transition where it ends, the app's clock unmoved, unless filmed.
import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { decodePng } from '../../scripts/png.mjs';
import { serveBuildTree } from './serve.mjs';

// A page with a transition on `swatch` (and the views `check` judges, LLP 1116.003 D8).
const BROKEN = `component CheckFixture
  state on = false
  action toggle
    on = not on
  view
    column testId="root" gap=8 padding=16 accent-color="light-dark(#6652c5, #b5a4ff)"
      text testId="fits" "Fits"
      row testId="card" padding=12 border-radius=12 background-color="-exact-secondary-grouped-background"
        text testId="faint" color="#bbbbbb" "Faint text in a card"
      button testId="go" press=toggle buttonStyle="bordered-prominent"
        text "Go"
      text testId="clip" width=60 height=20 overflow="hidden" "A long label that will not fit"
      text testId="ellipsis" width=60 white-space="nowrap" overflow="hidden" text-overflow="ellipsis" "A long label with an ellipsis"
      column testId="stage" position="relative" height=60
        button testId="hidden" press=toggle
          text "Under"
        column testId="cover" position="absolute" top=0 left=0 width=300 height=60 background-color="-exact-secondary-background"
      row gap=0
        column testId="a" press=toggle width=100 height=44 background-color="-exact-secondary-background"
        column testId="b" press=toggle width=100 height=44 margin-left=-20 background-color="-exact-tertiary-fill"
      row gap=4
        column testId="tiny" press=toggle width=16 height=16 background-color="-exact-secondary-background"
        column testId="tiny2" press=toggle width=16 height=16 background-color="-exact-secondary-background"
      row testId="wide" width=600 height=20 background-color="-exact-secondary-background"
      column testId="swatch" press=toggle width=100 height=100 background-color=(on ? "#ff0000" : "#0000ff") transition="background-color 400ms linear"
`;
const built = (() => {
  const ready = new Map();
  return (name, source) => {
    if (!ready.has(name)) ready.set(name, (async () => {
      const dir = mkdtempSync(join(tmpdir(), `exact-check-${name}-`)), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
      writeFileSync(contract, source);
      const compile = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { encoding: 'utf8' });
      if (compile.status !== 0) throw new Error(compile.stderr);
      const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: new URL('../../', import.meta.url).pathname, encoding: 'utf8' });
      if (build.status !== 0) throw new Error(build.stderr);
      const server = createServer((request, response) => serveBuildTree(dist, request, response));
      await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
      server.unref();
      return `http://127.0.0.1:${server.address().port}/`;
    })());
    return ready.get(name);
  };
})();

test('a screenshot shows a transition where it ends, the clock unmoved; a film\'s frames are the clock\'s', async () => {
  const session = await open({ host: 'web', url: await built('broken', BROKEN) });
  try {
    const dir = mkdtempSync(join(tmpdir(), 'exact-check-shot-'));
    const swatch = (await session.layout('swatch')).node.space.viewport;
    const at = (file) => { const png = decodePng(readFileSync(file)), x = Math.floor(swatch.x + swatch.w / 2), y = Math.floor(swatch.y + swatch.h / 2), i = (y * png.width + x) * 4; return [...png.data.slice(i, i + 3)]; };
    await session.tap('swatch');
    const clock = session.now;
    await session.screenshot(join(dir, 'after.png'));
    const [r, , b] = at(join(dir, 'after.png'));
    expect(r).toBeGreaterThan(240); // red, where the 400 ms transition ends
    expect(b).toBeLessThan(15);
    expect((await session.op({ op: 'tags' })).clock).toBe(clock); // the app's clock did not move for the picture
    await session.clock('settle'); // red now on the clock too
    await session.tap('swatch'); // back toward blue: a film's first frame is where the clock stands, its start
    const film = await session.screenshot(join(dir, 'film.png'), { over: 400, every: 200 });
    const frame = (i) => at(join(film.dir, `${String(i).padStart(3, '0')}-${film.at[i]}ms.png`));
    expect(frame(0)[0]).toBeGreaterThan(240); // still red at the clock it starts on
    expect(frame(1)[0]).toBeLessThan(200); // half way
    expect(frame(2)[2]).toBeGreaterThan(240); // blue where it ends
  } finally { await session.close(); }
}, 900000);
