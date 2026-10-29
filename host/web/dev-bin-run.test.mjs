// The dev loop's bake driver (`devBinRun`, host/web/dev.mjs): a run with nothing changed rebuilds nothing, and a new
// asset root is seen. Without what the bake watches named, the build script kept a missing root (`deck/`) as a missing
// input wherever the app's own directory holds the build output, as a game's and an app made outside this repository
// do (bake/src/receipt/watch.rs `optional_tree`), and Cargo reran it, and rebuilt and relinked the app's web crate, on
// every run: about 60 s each restart of a game with data (QUEUE: the `-dev` bin relinked in release). So the driver
// is built here into the app's own `target/`, an app with neither `deck/` nor `gpu/shaders/`; the first build is a
// cold one, a few minutes. One run at a time (a kernel lock in that target), and only a `deck/` this run made is
// removed.
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { developmentBuildEnv, devBinRun, resolveApp } from '../../scripts/app.mjs';
import { takeLock } from './build-lock.mjs';

const APP = 'sparkline';
// What marks a `deck/` as this test's own: one left by a run that was killed is removed by the next.
const MARK = '.dev-bin-run-test';

/** One build of the driver as the dev loop runs it (its `run` made a `build`): the units, of any package, that
 * Cargo did not find fresh. */
function rebuilt(app, extra = {}) {
  const run = devBinRun(app, `${APP}-dev`, { ...developmentBuildEnv(), CARGO_TARGET_DIR: resolve(app.dir, 'target'), ...extra });
  const args = run.args.map(a => a === 'run' ? 'build' : a).filter(a => a !== '-q');
  const env = { ...run.env };
  for (const [k, v] of Object.entries(extra)) if (v === undefined) delete env[k];
  const r = spawnSync('cargo', [...args, '--message-format', 'json'], { cwd: run.cwd, env, encoding: 'utf8', maxBuffer: 256 << 20 });
  expect({ status: r.status, stderr: r.status ? r.stderr.slice(-2000) : '' }).toEqual({ status: 0, stderr: '' });
  return r.stdout.split('\n').filter(Boolean).map(l => { try { return JSON.parse(l); } catch { return null; } })
    .filter(m => m && m.reason === 'compiler-artifact' && m.fresh === false)
    .map(m => m.target.name);
}

test('the dev bin rebuilds nothing when nothing changed, and sees a new asset root', async () => {
  const app = resolveApp(APP), deck = resolve(app.dir, 'deck');
  // One run at a time: the kernel's lock on a file in that target (build-lock.mjs `takeLock`), released when its holder
  // exits, killed or not. `takeLock` waits 30 s a call; a cold run takes longer, so wait on.
  mkdirSync(resolve(app.dir, 'target'), { recursive: true }); // a cold tree has none yet
  let release;
  for (const end = Date.now() + 1_500_000; ;) {
    try { release = takeLock(resolve(app.dir, 'target', MARK + '.flock')); break; } catch (e) { if (!/has held it/.test(e.message) || Date.now() > end) throw e; }
  }
  let made = false;
  try {
    if (existsSync(resolve(deck, MARK))) rmSync(deck, { recursive: true, force: true });
    expect(existsSync(deck)).toBe(false);
    rebuilt(app); // whatever is stale from before
    expect(rebuilt(app)).toEqual([]);
    // Unnamed, as the dev loop ran it before: the missing `deck/` reruns the build script every time.
    const unnamed = { EXACT_ASSET_ROOTS: undefined, EXACT_BACKEND_SELECTION: undefined };
    rebuilt(app, unnamed);
    expect(rebuilt(app, unnamed)).toContain(`${APP}_web`);
    rebuilt(app);
    mkdirSync(deck); made = true;
    writeFileSync(resolve(deck, MARK), '');
    expect(rebuilt(app)).toContain(`${APP}_web`); // the new root is named, and watched
    expect(rebuilt(app)).toEqual([]);
    rmSync(deck, { recursive: true, force: true }); made = false;
    rebuilt(app);
    expect(rebuilt(app)).toEqual([]);
  } finally {
    try { if (made) rmSync(deck, { recursive: true, force: true }); } finally { release(); }
  }
}, 1_800_000);
