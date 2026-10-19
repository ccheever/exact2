// One import planned in a process of its own (LLP 1121 §5 stage 10b; the
// lead's 12:31Z ruling on E2+E3): a client's file (a paired one's, through
// a remote door, as much as the local tool's or `bun exact.mjs tamber
// import`'s) is read by `contract tamber import-plan`, never by the shared
// `tamber serve`, so a document that costs too much to read stops only its
// own process. A watchdog here kills it past `rssKb` of resident memory,
// `ms` of wall clock or `outBytes` of answer, or when it can't watch it
// (macOS ignores RLIMIT_AS, so the parent watches; on Linux the limit is set
// too): 413 or 503, "the source is too costly to read". Imports run one at
// a time, at most `waiting` queued behind, each waiting at most `waitMs`.

import { execFile, spawn } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { open, realpath } from 'node:fs/promises';
import { join, relative, resolve, sep } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
/** About 1 GiB resident (KiB), 10 s, a 2 MiB answer, a 4 MiB design; four queued, each waiting up to 30 s. */
export const LIMITS = { rssKb: 1 << 20, ms: 10_000, outBytes: 2 << 20, designBytes: 4 << 20, waiting: 4, waitMs: 30_000 };

/** The compiler as `tamber serve` runs it: the built binary when cargo has
 * built it (the serve's own run did), else through cargo (which replaces
 * itself with the compiler, so the pid watched is the planner's). */
function contract(args) {
  const built = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug', 'contract');
  const [cmd, argv] = existsSync(built) ? [built, args] : ['cargo', ['run', '-q', '-p', 'contract', '--', ...args]];
  // (a process group of its own, so a kill stops whatever it started)
  if (process.platform !== 'linux') return spawn(cmd, argv, { cwd: ROOT, stdio: ['pipe', 'pipe', 'pipe'], detached: true });
  // Linux enforces an address-space limit, set too. Address space is not
  // resident memory (an allocator reserves ahead), so it is generous: the
  // resident watch is the bound, this the backstop.
  const kb = LIMITS.rssKb * 4;
  return spawn('sh', ['-c', `ulimit -v ${kb} && exec "$0" "$@"`, cmd, ...argv], { cwd: ROOT, stdio: ['pipe', 'pipe', 'pipe'], detached: true });
}

/** A process's resident KiB, by `ps`, or null when it can't be read. */
const residentKb = pid => new Promise(done => {
  execFile('ps', ['-o', 'rss=', '-p', String(pid)], { timeout: 1000 }, (e, out) => done(e ? null : Number(out) || 0));
});

let last = Promise.resolve(), queued = 0;
const busy = { status: 503, error: { message: 'imports are busy: try again shortly' } };

/** `request` (import-plan's) planned alone against the design at `design`
 * (a file of the app at `appDir`, read when its turn comes): `{ok}` or
 * `{error}` as the serve answers, or a status and error when the watchdog
 * stopped it or the queue is full. */
export function planAlone(request, design, limits = {}, appDir = null) {
  const l = { ...LIMITS, ...limits };
  if (queued > l.waiting) return Promise.resolve(busy);
  queued++;
  const asked = Date.now();
  const run = last.then(() => (Date.now() - asked > l.waitMs ? busy : designText(design, appDir, l.designBytes).then(t => ('text' in t ? planOnce(request, design, t.text, l) : t))));
  last = run.catch(() => {}).finally(() => { queued--; });
  return run;
}

/** The design's text: a regular file inside the app, at most `max` bytes,
 * read without blocking; null when it is not there; an error otherwise. */
async function designText(path, appDir, max) {
  try {
    const real = await realpath(path);
    if (appDir && relative(await realpath(appDir), real).split(sep)[0] === '..') return { status: 400, error: { message: 'the design is not a file of the app' } };
    const file = await open(real, 'r');
    try {
      const st = await file.stat();
      if (!st.isFile()) return { status: 400, error: { message: 'the design is not a regular file' } };
      if (st.size > max) return { status: 413, error: { message: `the design is over ${max} bytes` } };
      return { text: await file.readFile('utf8') };
    } finally { await file.close(); }
  } catch (e) {
    return e?.code === 'ENOENT' ? { text: null } : { status: 500, error: { message: `the design can't be read: ${e?.code ?? e?.message}` } };
  }
}

function planOnce(request, design, text, { rssKb, ms, outBytes, spawnPlan = contract, probe = residentKb }) {
  return new Promise(done => {
    const child = spawnPlan(['tamber', 'import-plan', design]);
    let out = '', err = '', settled = false, probing = false;
    const finish = reply => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      clearInterval(watch);
      done(reply);
    };
    const stop = reply => {
      // Its group, whatever it started (never a group that is not its own); its pipes let go of.
      if (child.pid > 1) { try { process.kill(-child.pid, 'SIGKILL'); } catch { child.kill('SIGKILL'); } }
      child.stdout.destroy();
      child.stderr.destroy();
      finish(reply);
    };
    const costly = why => ({ status: 413, error: { message: `the source is too costly to read: ${why}` } });
    const timer = setTimeout(() => stop({ status: 503, error: { message: `the source is too costly to read: reading it took more than ${ms / 1000} s` } }), ms);
    // One probe at a time; one that can't read the planner stops it (never an import unwatched).
    const watch = setInterval(() => {
      if (probing || settled) return;
      probing = true;
      probe(child.pid).then(kb => {
        probing = false;
        if (settled) return;
        if (kb === null) stop({ status: 503, error: { message: 'the import could not be watched: try again' } });
        else if (kb > rssKb) stop(costly(`reading it took more than ${Math.round(rssKb / 1024)} MiB`));
      });
    }, 50);
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', d => {
      out += d;
      if (out.length > outBytes) stop(costly(`its review is over ${outBytes} bytes`));
    });
    child.stderr.on('data', d => { err = (err + d).slice(-2000); });
    child.on('error', e => { err += e.message; });
    child.on('close', code => {
      try { finish(JSON.parse(out)); } catch {
        // Killed by the kernel's limit, or a crash: no answer, its cost the likely cause.
        finish(costly(`the planner stopped${code === null ? '' : ` (${code})`}${err ? `: ${err.trim().split('\n').at(-1)}` : ''}`));
      }
    });
    child.stdin.on('error', () => {});
    child.stdin.end(JSON.stringify({ request, design: text }));
  });
}

/** A source path's files as `import`'s `sources` (`contract tamber
 * import`'s reading): the file, or each `.json` under the directory, by its
 * path from it, at most 256. */
export function sourcesOf(path) {
  if (!statSync(path).isDirectory()) return [{ name: path.split(sep).at(-1), text: readFileSync(path, 'utf8') }];
  const out = [], walk = dir => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.isFile() && e.name.toLowerCase().endsWith('.json')) out.push({ name: relative(path, p).split(sep).join('/'), text: readFileSync(p, 'utf8') });
      if (out.length > 256) throw new Error(`${path}: more than 256 .json files`);
    }
  };
  walk(path);
  return out;
}
