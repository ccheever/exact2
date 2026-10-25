// The JS build's GPU cache, `<target>/web-js-gpu/<app name>` (host/web-js/build.mjs): kept between builds so a warm
// rebuild skips the GPU build, and shared by every loop and build of the app on that target (two checkouts on one
// CARGO_TARGET_DIR, two runs at once). So it is never built in place: a stale cache is built in a stage of its own and
// renamed over the cache under its lock, and a build copies the cache under the same lock, so none copies a tree
// another is removing or rewriting.
import { cpSync, existsSync, mkdirSync, readdirSync, renameSync, rmSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { withLock } from '../web/serve.mjs';

let stages = 0;
const alive = (pid) => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };

/** Copy the GPU cache at `cache` into `out`, first building it when `fresh(cache)` says it is stale: `build(stage)`
 * fills an empty stage beside the cache (and throws if it can't). `beforeRename(stage)` runs once the stage is built,
 * before the lock is taken (a test's hold). Whether the cache was built. */
export function copyGpuCache(cache, out, { fresh, build, beforeRename = () => {} }) {
  const lock = `${cache}.lock`, copy = () => cpSync(cache, out, { recursive: true });
  mkdirSync(dirname(cache), { recursive: true });
  const previous = `${cache}.previous`;
  // A stage a killed build left (its pid gone) is removed here, by the next build of the app, warm or not.
  const stageName = new RegExp(`^${basename(cache).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\.stage-(\\d+)-\\d+$`);
  for (const name of readdirSync(dirname(cache))) {
    const pid = Number(stageName.exec(name)?.[1] ?? 0);
    if (pid > 0 && pid !== process.pid && !alive(pid)) rmSync(resolve(dirname(cache), name), { recursive: true, force: true });
  }
  if (withLock(lock, () => {
    // A build killed between its renames left the cache it replaced at `.previous`: back in place first. One killed
    // after both left it beside its successor: gone.
    if (!existsSync(cache) && existsSync(previous)) renameSync(previous, cache);
    else rmSync(previous, { recursive: true, force: true });
    return fresh(cache) && (copy(), true);
  })) return false;
  const stage = `${cache}.stage-${process.pid}-${++stages}`;
  rmSync(stage, { recursive: true, force: true });
  try {
    build(stage);
    beforeRename(stage);
    withLock(lock, () => {
      // Two renames, under the lock every reader takes too, so none sees the gap between them; the cache replaced
      // waits at `.previous` until its successor is in place, and goes back if that rename fails.
      // With no cache (a build killed between its renames since this one looked), `.previous` is the last good one.
      if (existsSync(cache)) { rmSync(previous, { recursive: true, force: true }); renameSync(cache, previous); }
      try { renameSync(stage, cache); }
      catch (error) { if (existsSync(previous)) renameSync(previous, cache); throw error; }
      rmSync(previous, { recursive: true, force: true });
      copy();
    });
  } finally { rmSync(stage, { recursive: true, force: true }); }
  return true;
}
