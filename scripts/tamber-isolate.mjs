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
import { constants, existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { open, realpath } from 'node:fs/promises';
import { join, relative, resolve, sep } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
/** About 1 GiB resident (KiB), 10 s, a 2 MiB answer, a 4 MiB design; four queued, each waiting up to 30 s. */
export const LIMITS = { rssKb: 1 << 20, ms: 10_000, outBytes: 2 << 20, designBytes: 4 << 20, waiting: 4, waitMs: 30_000 };

/** The compiler as `tamber serve` runs it, the binary its cargo run built:
 * one process, the pid watched (never cargo, whose builds the watch would
 * not see); null before it is built. */
function contract(args) {
  const cmd = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug', 'contract'), argv = args;
  if (!existsSync(cmd)) return null;
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

// The one planner's permit: the next starts once the last has exited (an
// answer goes back as soon as it is known; a killed planner still holds
// the permit until the kernel has reaped it).
let last = Promise.resolve(), queued = 0;
const busy = { status: 503, error: { message: 'imports are busy: try again shortly' } };

/** `request` (import-plan's) planned alone against the design at `design`
 * (a file of the app at `appDir`, read when its turn comes): `{ok}` or
 * `{error}` as the serve answers, or a status and error when the watchdog
 * stopped it, the queue is full, or it waited past `waitMs`. */
export function planAlone(request, design, limits = {}, appDir = null) {
  const l = { ...LIMITS, ...limits };
  if (queued > l.waiting) return Promise.resolve(busy);
  queued++;
  let answer, expired = false;
  const reply = new Promise(ok => { answer = ok; });
  // Each waiting entry expires on its own clock, whatever is ahead of it.
  const wait = setTimeout(() => { expired = true; answer(busy); }, l.waitMs);
  const turn = last.then(async () => {
    clearTimeout(wait);
    if (expired) return;
    const t = await designText(design, appDir, l.designBytes);
    if (!('text' in t)) return answer(t);
    const [planned, exited] = planOnce(request, design, t.text, l);
    planned.then(answer);
    await exited;
  });
  last = turn.catch(() => {}).finally(() => { queued--; });
  return reply;
}

/** The design's text: a regular file inside the app, at most `max` bytes,
 * opened without following a link or waiting on a FIFO, read within 2 s;
 * null when it is not there; an error otherwise. */
async function designText(path, appDir, max) {
  try {
    const real = await realpath(path);
    if (appDir && relative(await realpath(appDir), real).split(sep)[0] === '..') return { status: 400, error: { message: 'the design is not a file of the app' } };
    const file = await open(real, constants.O_RDONLY | constants.O_NONBLOCK | constants.O_NOFOLLOW);
    try {
      const st = await file.stat();
      if (!st.isFile()) return { status: 400, error: { message: 'the design is not a regular file' } };
      const buf = Buffer.alloc(max + 1);
      const read = await Promise.race([file.read(buf, 0, max + 1, 0), new Promise(ok => setTimeout(() => ok(null), 2000))]);
      if (!read) return { status: 503, error: { message: 'the design took too long to read' } };
      if (read.bytesRead > max) return { status: 413, error: { message: `the design is over ${max} bytes` } };
      return { text: buf.subarray(0, read.bytesRead).toString('utf8') };
    } finally { await file.close(); }
  } catch (e) {
    return e?.code === 'ENOENT' ? { text: null } : { status: 500, error: { message: `the design can't be read: ${e?.code ?? e?.message}` } };
  }
}

/** The plan's answer, and when its planner has exited. */
function planOnce(request, design, text, { rssKb, ms, outBytes, spawnPlan = contract, probe = residentKb }) {
  const child = spawnPlan(['tamber', 'import-plan', design]);
  if (!child) return [Promise.resolve({ status: 503, error: { message: 'the compiler is not built yet: try again once the design server is up' } }), Promise.resolve()];
  const exited = new Promise(ok => child.on('close', ok));
  return [new Promise(done => {
    let out = '', bytes = 0, err = '', settled = false, probing = false;
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
      child.stdin.destroy();
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
      bytes += Buffer.byteLength(d);
      if (bytes > outBytes) return stop(costly(`its review is over ${outBytes} bytes`));
      out += d;
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
  }), exited];
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
