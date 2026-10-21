#!/usr/bin/env bun
// Clear out Cargo output no build has used for a week. Every source, feature, or path change
// compiles a unit under a new hash and Cargo never removes the old one, so a checkout's target/
// only grows (one reached 116 GB, most of it stale copies of each app's debug staticlib).
// `resolveApp` starts this, detached, at most once an hour; `bun scripts/sweep.mjs <target>` runs it now.
//
// "Used" is measured, not guessed. Cargo reads the fingerprint of every unit a build considers,
// even when it compiles nothing, and never reads one it does not build. Each sweep sets those
// files' access times back to RESET; the kernel updates an access time older than the file's
// mtime on the next read (macOS, and Linux relatime), so one read since the last sweep shows.
// A unit is `.fingerprint/<package>-<hash>` plus every `deps/` file and `build/` directory with
// that hash. Incremental caches carry rustc's own names, so they go by when they last compiled.
//
// Cargo holds `<profile>/.cargo-lock` (flock) while it builds into a profile directory. The sweep
// takes the same lock without waiting and skips a directory a build holds; under the lock it only
// renames, fingerprints first, so a unit is never half there. It deletes after letting go.
import { closeSync, existsSync, mkdirSync, openSync, readdirSync, readFileSync, renameSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const KEEP = 7 * 86_400_000, EVERY = 3_600_000;
const RESET = 86_400; // 1970-01-02, in seconds: older than any real mtime
const LEDGER = '.exact-sweep.json', LOG = '.exact-sweep.log', ASIDE = '.exact-swept';
const HASH = /-([0-9a-f]{16})(?:\.|$)/;

/** Start a sweep of `target` in the background if the last one finished over an hour ago. A
 * sweep that failed leaves its error in the log, reported on every build until one succeeds. */
export function startSweep(target, now = Date.now()) {
  // This is optional cache housekeeping. Windows has neither this flock
  // protocol nor reliable immediate access-time updates; keep its build units.
  if (process.platform === 'win32') return;
  if (!existsSync(target)) return;
  const log = resolve(target, LOG);
  if (existsSync(log) && statSync(log).size) process.stderr.write(`target sweep failed (${log}):\n${readFileSync(log, 'utf8')}`);
  const ledger = resolve(target, LEDGER);
  if (existsSync(ledger) && now - JSON.parse(readFileSync(ledger, 'utf8')).at < EVERY) return;
  const err = openSync(log, 'a');
  spawn(process.execPath, [fileURLToPath(import.meta.url), target], { detached: true, stdio: ['ignore', 'ignore', err] }).unref();
  closeSync(err);
}

async function locker() {
  const { dlopen, FFIType } = await import('bun:ffi');
  const { symbols } = dlopen(process.platform === 'darwin' ? '/usr/lib/libSystem.B.dylib' : 'libc.so.6', { flock: { args: [FFIType.i32, FFIType.i32], returns: FFIType.i32 } });
  // LOCK_EX | LOCK_NB, the same flock Cargo takes. Closing the descriptor releases it.
  return path => { const fd = openSync(path, 'a'); if (symbols.flock(fd, 6) === 0) return fd; closeSync(fd); return null; };
}

/** Without access times the sweep cannot tell a used unit from an abandoned one, so it refuses. */
function assertAccessTimes(target) {
  const probe = resolve(target, `${LEDGER}.probe`);
  writeFileSync(probe, 'x');
  utimesSync(probe, RESET, statSync(probe).mtime);
  readFileSync(probe);
  const seen = statSync(probe).atimeMs > RESET * 1000;
  rmSync(probe);
  if (!seen) throw new Error(`${target}: this filesystem does not record reads (noatime), so the sweep cannot tell which units builds use`);
}

const profiles = target => [target, ...readdirSync(target, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => resolve(target, e.name))]
  .flatMap(dir => readdirSync(dir, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => resolve(dir, e.name)))
  .filter(dir => existsSync(resolve(dir, '.fingerprint')));

/** Sweep `target` as of `now`. Returns what was moved out, per profile directory. */
export async function sweep(target, now = Date.now()) {
  if (process.platform === 'win32') throw new Error('Cargo cache sweeping is unavailable on Windows: its lock and access-time semantics have not been verified');
  const lock = await locker(), own = lock(resolve(target, `${LEDGER}.lock`));
  if (own === null) return {};
  try {
    assertAccessTimes(target);
    const path = resolve(target, LEDGER), old = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')).units : {};
    const units = {}, removed = {}, aside = resolve(target, ASIDE, String(now));
    let n = 0;
    const move = p => { mkdirSync(aside, { recursive: true }); renameSync(p, resolve(aside, String(n++))); };
    for (const dir of profiles(target)) {
      const at = relative(target, dir), fd = lock(resolve(dir, '.cargo-lock'));
      if (fd === null) { // a build holds it: keep what we knew, look again next time
        for (const [key, t] of Object.entries(old)) if (key.startsWith(at + '#')) units[key] = t;
        continue;
      }
      try {
        const stale = new Set(), fingerprints = resolve(dir, '.fingerprint');
        for (const name of readdirSync(fingerprints)) {
          const hash = HASH.exec(name)?.[1];
          if (!hash) continue;
          const unit = resolve(fingerprints, name), files = readdirSync(unit).map(f => resolve(unit, f));
          const key = `${at}#${hash}`, read = files.some(f => statSync(f).atimeMs > RESET * 1000);
          units[key] = read || !(key in old) ? now : old[key];
          if (now - units[key] > KEEP) { stale.add(hash); delete units[key]; move(unit); continue; }
          for (const f of files) utimesSync(f, RESET, statSync(f).mtime);
        }
        let count = stale.size;
        for (const sub of ['deps', 'build', 'examples']) {
          const d = resolve(dir, sub);
          if (existsSync(d)) for (const name of readdirSync(d)) if (stale.has(HASH.exec(name)?.[1])) move(resolve(d, name));
        }
        const incremental = resolve(dir, 'incremental');
        if (existsSync(incremental)) for (const name of readdirSync(incremental)) {
          const cache = resolve(incremental, name);
          const last = Math.max(statSync(cache).mtimeMs, ...readdirSync(cache).map(s => statSync(resolve(cache, s)).mtimeMs));
          if (now - last > KEEP) { move(cache); count++; }
        }
        if (count) removed[at] = count;
      } finally { closeSync(fd); }
    }
    writeFileSync(`${path}.tmp`, JSON.stringify({ at: now, units }));
    renameSync(`${path}.tmp`, path);
    rmSync(resolve(target, ASIDE), { recursive: true, force: true });
    writeFileSync(resolve(target, LOG), '');
    return removed;
  } finally { closeSync(own); }
}

if (import.meta.main) {
  const removed = await sweep(resolve(process.argv[2] ?? 'target'));
  if (process.stdout.isTTY) console.log(Object.keys(removed).length ? removed : 'nothing unused for a week');
}
