// One import planned in a process of its own (LLP 1121 §5 stage 10b; the
// lead's 12:31Z ruling on E2+E3): a client's file (a paired one's, through
// a remote door, as much as the local tool's) is read by
// `contract tamber import-plan`, never by the shared `tamber serve`, so a
// document that costs too much to read stops only its own process. A
// watchdog here kills it past `rssKb` of resident memory or `ms` of wall
// clock (macOS ignores RLIMIT_AS, so the parent watches; on Linux the limit
// is set too): 413 or 503, "the source is too costly to read".

import { execFile, spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
/** About 1 GiB resident, in KiB, and 10 s. */
export const LIMITS = { rssKb: 1 << 20, ms: 10_000 };

/** The compiler as `tamber serve` runs it: the built binary when cargo has
 * built it (the serve's own run did), else through cargo. */
function contract(args) {
  const built = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug', 'contract');
  const [cmd, argv] = existsSync(built) ? [built, args] : ['cargo', ['run', '-q', '-p', 'contract', '--', ...args]];
  // (a process group of its own, so a kill stops cargo's compiler too)
  if (process.platform !== 'linux') return spawn(cmd, argv, { cwd: ROOT, stdio: ['pipe', 'pipe', 'pipe'], detached: true });
  // (Linux enforces an address-space limit: the same bound, set where it holds)
  const kb = Math.ceil(LIMITS.rssKb * 1.5);
  return spawn('sh', ['-c', `ulimit -v ${kb} && exec "$0" "$@"`, cmd, ...argv], { cwd: ROOT, stdio: ['pipe', 'pipe', 'pipe'], detached: true });
}

// One at a time: imports in parallel would multiply the bound.
let last = Promise.resolve();

/** `request` (import-plan's) planned alone against the design at `design`
 * (its file, read when its turn comes): `{ok}` or `{error}` as the serve
 * answers, or a status and error when the watchdog stopped it. */
export function planAlone(request, design, limits = {}) {
  const run = last.then(() => planOnce(request, design, limits));
  last = run.catch(() => {});
  return run;
}

function planOnce(request, design, { rssKb = LIMITS.rssKb, ms = LIMITS.ms, spawnPlan = contract } = {}) {
  let text = null;
  try { text = readFileSync(design, 'utf8'); } catch { /* no design yet */ }
  return new Promise(done => {
    const child = spawnPlan(['tamber', 'import-plan', design]);
    let out = '', err = '', stopped = null;
    const stop = why => {
      if (stopped) return;
      stopped = why;
      // Its group, whatever it started; its pipes let go of, so nothing a straggler holds keeps the answer waiting.
      try { process.kill(-child.pid, 'SIGKILL'); } catch { child.kill('SIGKILL'); }
      child.stdout.destroy();
      child.stderr.destroy();
    };
    const timer = setTimeout(() => stop('time'), ms);
    const watch = setInterval(() => {
      execFile('ps', ['-o', 'rss=', '-p', String(child.pid)], (e, rss) => { if (!e && Number(rss) > rssKb) stop('memory'); });
    }, 50);
    child.stdout.on('data', d => { out += d; });
    child.stderr.on('data', d => { err += d; });
    child.on('error', e => { err += e.message; });
    child.on('close', code => {
      clearTimeout(timer);
      clearInterval(watch);
      if (stopped === 'memory') return done({ status: 413, error: { message: `the source is too costly to read: reading it took more than ${Math.round(rssKb / 1024)} MiB` } });
      if (stopped === 'time') return done({ status: 503, error: { message: `the source is too costly to read: reading it took more than ${ms / 1000} s` } });
      try { return done(JSON.parse(out)); } catch {
        // Killed by the kernel's limit, or a crash: no answer, its cost the likely cause.
        return done({ status: 413, error: { message: `the source is too costly to read (the planner stopped${code === null ? '' : `, ${code}`}${err ? `: ${err.trim().split('\n').at(-1)}` : ''})` } });
      }
    });
    child.stdin.on('error', () => {});
    child.stdin.end(JSON.stringify({ request, design: text }));
  });
}
