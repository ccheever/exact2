#!/usr/bin/env bun
// `bun exact.mjs verify [web|ios]` (LLP 1116.004 D7, 1116.005 D7): one command a builder can stop on, where test,
// drives, check and screenshots were three to five turns. It builds when the build is stale (web: the dist's
// record against its sources, as the driver checks it; iOS: agent-build.mjs `buildFirst`, the plan fast path
// included), runs app.test.contract (agent-test.mjs `runTests`), launches the app on a fresh scratch store at a
// phone width and a desktop width (iOS: the simulator's own size, with UIKit's bars), and runs `check` light and
// dark there. Each test's end screen is checked the same way inside the test's own session (the web at both
// widths, resized to each), so its secondary screens and populated states are in the report. Then the app
// relaunches once on the store a passing test left, on real time (at most REAL_MS) and under the date, zone,
// locale and seed the test launched with (LLP 1116.007 D3.4), and its first screen is compared with a fresh
// launch's under the same: what the test stored should come back. One capture of each screen is kept in
// .exact/verify/ for milestones (four of the first screen: each width and scheme). It prints one short report
// ending "verify: clean" or "verify: N problems" (`--json`: one object). Exit 1 when the build, a test, a launch
// or the relaunch fails; check's findings, the relaunch's and the compiler's warnings (`contract build`'s,
// whether or not this run built) are counted in the last line but never fail it, since check reads the screen
// by heuristics and some of either only advise. The web's tests and its two launches run at once; a simulator
// takes one drive at a time, and runs the tests with UIKit's bars (`--chrome platform`) so their end screens
// are what a person sees. An app with a server of its own (the `rest` pattern's) has it run by verify, where the
// app calls it, on an empty scratch database kept through the run and removed after (LLP 1116.006 D3, `appServer`); a server
// entry that does not read DB is refused before the build, in one line naming it and the fix (LLP 1116.008 D1).
//
//   bun scripts/verify.mjs [web|ios] --app <name> [--sim <udid|name>] [--shots <dir>] [--json]
import { spawn, spawnSync } from 'node:child_process';
import { closeSync, copyFileSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, rmdirSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { guardFlags } from './help.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
/** The web's two widths: a phone's, and a desktop window's. */
export const WEB_SIZES = [[390, 844], [1280, 900]];
const SHOWN = 6, WARNINGS = 4; // the findings and build warnings shown; the count says the rest
const REAL_MS = 4000, REAL_STEP = 250; // the relaunch's bound on real time for its first loads, in steps
const FIRST = 'first screen', RELAUNCH = 'relaunch';
const secs = (ms) => `${(ms / 1000).toFixed(1)} s`;
const firstLine = (text) => String(text ?? '').split('\n').find((l) => l.trim())?.trim() ?? '';
const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

// The app's own server (LLP 1116.006 D3). A local origin the data module calls, with any path under it.
const LOCAL = /\bhttps?:\/\/(?:127\.0\.0\.1|localhost|\[::1\]):\d{2,5}(?:\/[\w\-./]*)?/;
const readIn = (dir, path) => { try { return readFileSync(resolve(dir, path), 'utf8'); } catch { return null; } };

/** The modules of the app's own that `path` imports (`from './x'`, `import './x'`, `import('./x')`, `require('./x')`),
 * as paths in `dir` that exist: `./x` is x, x.ts, x.js, x.mjs or x/index.ts. */
function importsOf(dir, path) {
  const found = [], base = resolve(dir, path, '..');
  for (const [, spec] of (readIn(dir, path) ?? '').matchAll(/(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\(\s*)['"](\.{1,2}\/[^'"]+)['"]/g)) {
    const at = resolve(base, spec);
    const file = [at, `${at}.ts`, `${at}.js`, `${at}.mjs`, `${at}/index.ts`].find((p) => readIn(dir, p) != null);
    if (file && !relative(dir, file).startsWith('..')) found.push(relative(dir, file));
  }
  return found;
}

/** The server URL the data module calls: the first local one (`http://127.0.0.1:8787`, or localhost) in app.ts, else in
 * a module of the app's own that it imports (`import { API } from './api'`); null when it calls none. */
export function serverUrl(dir) {
  const at = (text) => text?.match(LOCAL)?.[0].replace(/\/$/, '') ?? null;
  return at(readIn(dir, 'app.ts')) ?? importsOf(dir, 'app.ts').map((p) => at(readIn(dir, p))).find(Boolean) ?? null;
}

/** Whether `entry`, or a module of the app's own it imports (a root shim that only imports the backend), reads the
 * database's path from `DB`: `process.env.DB`, `Bun.env.DB`, `env["DB"]`, `const { DB } = process.env`. */
const READS_DB = /\benv\??\.DB\b|\benv\[\s*['"]DB['"]\s*\]|\{[^}]*\bDB\b[^}]*\}\s*=\s*[\w.]*\benv\b/;
function readsDB(dir, entry) {
  const seen = new Set([entry]);
  for (const path of seen) {
    if (READS_DB.test(readIn(dir, path) ?? '')) return true;
    for (const next of importsOf(dir, path)) if (seen.size < 40) seen.add(next);
  }
  return false;
}

/** The app's own server, the `rest` pattern's shape: an entry Bun runs (app.json's `commands.server`, as
 * `["bun", "server/index.ts"]`, else server.ts, server/index.ts or server/server.ts) at the URL the data module calls,
 * its database's path read from `DB`. null without one; else `{ entry, url, port, why, refusal }`: `why` says what keeps it
 * from being run when the app calls no local URL (null when nothing does), `refusal` (LLP 1116.008 D1) the one line for an
 * entry that does not read `DB`, which `test` and `verify` stop on, never running without it. */
export function appServer(dir) {
  let named = null;
  try { const argv = JSON.parse(readIn(dir, 'app.json') ?? '{}').commands?.server; if (Array.isArray(argv) && /(^|\/)bun$/.test(argv[0])) named = argv.slice(1).find((a) => !a.startsWith('-')) ?? null; } catch {}
  const entry = [named, 'server.ts', 'server/index.ts', 'server/server.ts'].find((p) => p && readIn(dir, p) != null);
  if (!entry) return null;
  const url = serverUrl(dir);
  const why = !url ? `${entry} is left alone: app.ts calls no local URL (http://127.0.0.1:<port>), so where the app reaches it is unknown` : null;
  const refusal = url && !readsDB(dir, entry)
    ? `${entry}${entry === named ? ' (app.json\'s commands.server)' : ''} does not read its database's path from DB, so it cannot be run on an empty one: write new Database(process.env.DB ?? 'data.db') in it or a module it imports, or name the entry that does in app.json: "commands": { "server": ["bun", "server/index.ts"] }`
    : null;
  return { entry, url, port: url ? Number(new URL(url).port) : null, why, refusal };
}

/** Whether anything answers HTTP at `url` (any status). */
export const answering = (url) => fetch(url, { signal: AbortSignal.timeout(1000) }).then(() => true, () => false);

/** The one line a server's URL already answering gets: who holds the port, when the system says, and what to do. A
 * server this did not start is never stopped, nor written to. */
export function heldLine(server) {
  const pid = spawnSync('lsof', ['-nP', `-iTCP:${server.port}`, '-sTCP:LISTEN', '-Fp'], { encoding: 'utf8' }).stdout?.match(/^p(\d+)/m)?.[1];
  const command = pid ? spawnSync('ps', ['-o', 'command=', '-p', pid], { encoding: 'utf8' }).stdout?.trim().slice(0, 60) : '';
  return `${server.url} already answers${pid ? ` (pid ${pid}${command ? `: ${command}` : ''})` : ''}, and the tests would write into that server's database: stop it (${pid ? `kill ${pid}` : 'Ctrl-C where it runs'}) and run this again, which starts ${server.entry} itself on an empty one`;
}

/** Runs `kill` when this process ends however it does (a signal ends it with the shell's code too); the function returned
 * takes that back. */
function whenEnded(kill) {
  const signals = { SIGINT: 130, SIGTERM: 143, SIGHUP: 129 }, onSignal = (signal) => { kill(); process.exit(signals[signal]); };
  process.on('exit', kill);
  for (const s of Object.keys(signals)) process.on(s, onSignal);
  return () => { process.off('exit', kill); for (const s of Object.keys(signals)) process.off(s, onSignal); };
}

/** Waits for `server.url` to answer, for the `child` that runs it: null once it does, else why it will not (it exited, or 10 s). */
async function untilAnswers(server, child, tail) {
  for (let i = 0; !(await answering(server.url)); i++) {
    if (child.exitCode != null || i >= 100) return `${server.entry} ${child.exitCode != null ? `exited (code ${child.exitCode})` : 'did not answer in 10 s'} before it answered at ${server.url}${tail() ? `: ${tail()}` : ''}`;
    await Bun.sleep(100);
  }
  return null;
}

/** The last error line of what a server said, else its last line. */
const lastError = (said) => { const lines = said.split('\n').map((l) => l.trim()).filter(Boolean); return lines.findLast((l) => /error/i.test(l)) ?? lines.at(-1) ?? ''; };

/** Runs the app's server (`appServer`) on an empty scratch database, `.exact/<name>/db.sqlite`, and answers once it
 * answers; refused (`heldLine`) when its URL already does. `{ stop(), exited(), tail() }`: `stop` ends it and removes
 * the database; `exited` is its exit code once it has ended on its own, else null. `forward` passes its stderr on. */
export async function startServer(dir, server, name, { forward = false } = {}) {
  if (await answering(server.url)) throw new Error(heldLine(server));
  // `owner` names this process, for a `bun exact.mjs test` it runs (an api-test's test.mjs) to find the server already here.
  const db = resolve(dir, '.exact', name, 'db.sqlite'), owner = resolve(dirname(db), 'owner');
  const clear = () => { for (const end of ['', '-wal', '-shm', '-journal']) rmSync(`${db}${end}`, { force: true }); rmSync(owner, { force: true }); };
  mkdirSync(dirname(db), { recursive: true });
  clear();
  writeFileSync(owner, `${process.pid}\n`);
  const child = Bun.spawn([process.execPath, server.entry], { cwd: dir, env: { ...process.env, DB: db, PORT: String(server.port) }, stdin: 'ignore', stdout: 'ignore', stderr: 'pipe' });
  let said = '';
  (async () => { for await (const chunk of child.stderr) { said = (said + new TextDecoder().decode(chunk)).slice(-4000); if (forward) process.stderr.write(chunk); } })().catch(() => {});
  const tail = () => lastError(said);
  // The server goes with this process however it ends; a signal still ends it.
  const off = whenEnded(() => { try { child.kill('SIGKILL'); } catch {} clear(); });
  const stop = async () => {
    off();
    if (child.exitCode == null) { child.kill('SIGTERM'); await Promise.race([child.exited, Bun.sleep(3000)]); }
    if (child.exitCode == null) { child.kill('SIGKILL'); await child.exited; }
    clear();
    try { rmdirSync(dirname(db)); } catch {} // when nothing else is in it (verify's keeps its screenshots)
  };
  const why = await untilAnswers(server, child, tail);
  if (why) { await stop(); throw new Error(why); }
  return { stop, exited: () => child.exitCode, tail };
}

const alive = (pid) => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };

/** The app's server for one `agent` drive (LLP 1116.008 D1): null when the app has none, or the URL it calls already answers
 * (the builder's own, or another drive's: used, said nothing of); else started where app.ts calls it, on a database of the
 * drive's own, `.exact/agent/<storage>/db.sqlite` kept for the `--storage` name so a second drive on it sees the first's data
 * (without one, an empty database removed after), and one line says so. `{ stop() }` ends it. Drives that overlap share the
 * server: each is a `drive-<pid>` file in `.exact/agent-server/`, the servers a drive started are `server-<pid>` (their
 * database as the file's text), and the last drive out stops them; one that did not start the server stops nothing, and the
 * builder's own is never in the list. An entry that does not read `DB` is refused (`appServer`'s `refusal`), as `test` refuses it. */
export async function driveServer(dir, storage, say = (line) => console.error(line)) {
  const server = appServer(dir);
  if (!server?.url) return null;
  if (server.refusal) throw new Error(`agent: ${server.refusal}`);
  const lease = resolve(dir, '.exact/agent-server'), mine = resolve(lease, `drive-${process.pid}`), starting = resolve(lease, 'starting');
  const db = storage ? resolve(dir, '.exact/agent', storage.replace(/[^\w.-]+/g, '-'), 'db.sqlite') : resolve(lease, 'db.sqlite');
  const files = (kind) => { try { return readdirSync(lease).filter((f) => f.startsWith(`${kind}-`)).map((f) => [resolve(lease, f), Number(f.slice(kind.length + 1))]); } catch { return []; } };
  mkdirSync(lease, { recursive: true });
  writeFileSync(mine, '');
  // This drive is out: the servers to end when it was the last one, their files removed.
  const leave = () => {
    rmSync(mine, { force: true });
    for (const [file, pid] of files('drive')) if (!alive(pid)) rmSync(file, { force: true });
    if (files('drive').length) return null;
    return files('server').map(([file, pid]) => { rmSync(file, { force: true }); return pid; })
      .filter((pid) => alive(pid) && spawnSync('ps', ['-o', 'command=', '-p', String(pid)], { encoding: 'utf8' }).stdout.includes(server.entry));
  };
  const clean = () => rmSync(lease, { recursive: true, force: true });
  const off = whenEnded(() => { const ending = leave(); for (const pid of ending ?? []) try { process.kill(pid, 'SIGKILL'); } catch {} if (ending) clean(); });
  const stop = async () => {
    off();
    const ending = leave();
    for (const pid of ending ?? []) {
      try { process.kill(pid, 'SIGTERM'); } catch {}
      for (let i = 0; i < 30 && alive(pid); i++) await Bun.sleep(100);
      if (alive(pid)) try { process.kill(pid, 'SIGKILL'); } catch {}
    }
    if (ending) clean();
  };
  // One drive starts the server at a time: another waits, then finds it answering.
  let locked = false;
  for (let i = 0; i < 150 && !locked; i++) {
    try { writeFileSync(starting, String(process.pid), { flag: 'wx' }); locked = true; }
    catch { const owner = Number(readIn(lease, 'starting')); if (owner && !alive(owner)) rmSync(starting, { force: true }); else await Bun.sleep(100); }
  }
  try {
    if (await answering(server.url)) {
      const other = files('server').map(([file]) => readIn(lease, file)).find((was) => was && was !== db);
      if (other) say(`agent: ${server.entry} at ${server.url} is another drive's, on its database (${relative(dir, other)}); this drive uses it`);
      return { stop };
    }
    mkdirSync(dirname(db), { recursive: true });
    if (!storage) for (const end of ['', '-wal', '-shm', '-journal']) rmSync(`${db}${end}`, { force: true });
    const log = openSync(resolve(lease, 'log'), 'a');
    const child = Bun.spawn([process.execPath, server.entry], { cwd: dir, env: { ...process.env, DB: db, PORT: String(server.port) }, stdin: 'ignore', stdout: 'ignore', stderr: log });
    closeSync(log);
    writeFileSync(resolve(lease, `server-${child.pid}`), db);
    const why = await untilAnswers(server, child, () => lastError(readIn(lease, 'log') ?? ''));
    if (why) { await stop(); throw new Error(`agent: ${why}`); }
    say(`agent: started ${server.entry} at ${server.url} for this drive (${storage ? `database ${relative(dir, db)}, kept for --storage ${storage}` : 'an empty database'}); stopped after`);
    return { stop };
  } finally { if (locked) rmSync(starting, { force: true }); }
}

/** `fn` with the app's server running on an empty database, as verify runs it: its exit code, or 1 when the server cannot
 * run, said in one line. `bun exact.mjs test` runs its tests so (LLP 1116.007 D2: `optional`: an app with no server, or one
 * whose URL app.ts does not say, runs them as they are, the second said once, and the tests use whatever answers there);
 * a server entry that does not read `DB` is refused whatever the verb (LLP 1116.008 D1), never run without it;
 * a server its parent process runs (an older app's api-test test.mjs, which runs `test` under `withAppServer`) is used, not started again.
 * `api-test` is the same call without `optional`: it wants a server, and says why it has none. */
export async function withAppServer(dir, fn, { verb = 'api-test', optional = false } = {}) {
  const server = appServer(dir);
  if (server && ['api-test', 'test', 'verify'].some((name) => readIn(dir, `.exact/${name}/owner`)?.trim() === String(process.ppid))) return fn(null);
  let running;
  try {
    if (server?.refusal) throw new Error(server.refusal);
    if (!server || server.why) {
      if (!optional) throw new Error(server?.why ?? 'no server: server.ts, server/index.ts, server/server.ts, or app.json\'s commands.server');
      if (server) console.error(`${verb}: ${server.why}; the tests use whatever answers there`);
    } else {
      running = await startServer(dir, server, verb, { forward: true });
      console.error(`${verb}: bun ${server.entry} at ${server.url}, on an empty database (.exact/${verb}/db.sqlite, removed after)`);
    }
  } catch (e) { console.error(`${verb}: ${e.message}`); return 1; }
  try { return await fn(running); } finally { await running?.stop(); }
}

/** Builds the web app when its dist is missing or older than its sources, as the driver would refuse it:
 * null when fresh, else `{ ms, code, warnings, output }`. */
async function webBuild(app) {
  const [{ webDist }, { builtAppMatches, jsTargetBuild }, { webChanges }] = await Promise.all([import('./app.mjs'), import('../host/web/serve.mjs'), import('./agent-launch.mjs')]);
  const dist = webDist();
  if (await builtAppMatches(dist, app) && jsTargetBuild(dist) && !webChanges(dist, app).all.length) return null;
  const started = Date.now();
  const r = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), app.name], { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20, env: { ...process.env, EXACT_APP_DIR: app.dir } });
  const output = `${r.stdout ?? ''}${r.stderr ?? ''}`;
  return { ms: Date.now() - started, code: r.status ?? 1, output, warnings: output.split('\n').filter((l) => /^warning: /.test(l)) };
}

/** The iOS build a drive needs, or its plan alone (agent-build.mjs): null when fresh, else what it said. */
async function iosBuild(app, sim) {
  const { buildFirst } = await import('./agent-build.mjs');
  const said = [], started = Date.now();
  // A native build (or the wait for one) is said as it starts too: it is minutes, and the report comes after it.
  const code = await buildFirst('ios', app, { sim, say: (line) => { said.push(line); if (/^(building|waiting) for/.test(line)) console.error(line); } });
  return said.length || code ? { ms: Date.now() - started, code, said, warnings: [] } : null;
}

/** The compiler's warnings for the app's sources, as `bun exact.mjs contract build app.contract` prints them,
 * whatever this run built (a fresh build said them when it was made): this checkout's compiler as built, or
 * through Cargo when its sources are newer. */
async function compilerWarningsOf(app) {
  const [{ fresh }, { compilerWarnings }, { cargoEnvironment }] = await Promise.all([import('../host/web-js/module.mjs'), import('./agent-launch.mjs'), import('./app.mjs')]);
  const env = cargoEnvironment(process.env), binary = resolve(ROOT, 'target/debug', process.platform === 'win32' ? 'contract.exe' : 'contract');
  delete env.RUSTUP_TOOLCHAIN; delete env.CARGO_TARGET_DIR; // as `exact contract` runs it: exact2's toolchain and target
  const [command, ...pre] = fresh(binary, resolve(ROOT, 'target/debug/contract.d')) ? [binary] : ['cargo', 'run', '-q', '--manifest-path', resolve(ROOT, 'Cargo.toml'), '-p', 'contract', '--'];
  const child = spawn(command, [...pre, 'build', resolve(app.dir, 'app.contract')], { cwd: ROOT, env, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  for (const stream of [child.stdout, child.stderr]) stream.on('data', (d) => { said += d; });
  await new Promise((done) => { child.on('close', done); child.on('error', done); });
  return compilerWarnings(said);
}

/** A test's capture, by its number and the start of its name: `web-t2-the-items-survive-a-relaunch.png`. */
export function shotName(host, index, name) {
  let slug = '';
  for (const word of String(name).toLowerCase().split(/[^a-z0-9]+/).filter(Boolean)) { if (slug && slug.length + 1 + word.length > 28) break; slug = slug ? `${slug}-${word}` : word; }
  return `${host}-t${index + 1}${slug ? `-${slug.slice(0, 28)}` : ''}.png`;
}

/** What a launch shows and holds, for the relaunch's comparison: the active screen's texts and field values, the
 * resources, and how many storage operations the data module has run. */
async function seen(s, state) {
  const texts = new Set();
  for (const n of (await s.tree().catch(() => ({ nodes: [] }))).nodes ?? []) for (const v of [n.props?.text, n.props?.value]) if (!n.inactive && typeof v === 'string' && v.trim()) texts.add(v.trim());
  const b = state.background;
  return { texts, resources: state.resources ?? {}, slots: state.slots ?? {}, ops: b ? b.done + b.failed : 0 };
}

/** `check both` on the session at each of `sizes` (null: as it is; else resized to it first), a run per size and scheme
 * tagged with its `screen`; a resource that failed is a line of each. `shot(viewport, scheme)` names a capture to keep. */
async function checkHere(s, { screen, sizes = [null], shot, state }) {
  const { checkOp } = await import('./agent-check.mjs');
  const failed = Object.entries(state?.failed ?? {}).map(([name, why]) => `  failed   resource ${name}: ${why}; the screen shows its placeholder or last value`);
  const runs = [];
  for (const size of sizes) {
    if (size) { await s.resize(...size); await s.clock('settle'); }
    const r = await checkOp(s, ['both'], { shots: shot });
    runs.push(...groups(r.text).map((g, i) => {
      const run = r.check[i] ?? {}, vp = run.viewport ?? { w: size?.[0], h: size?.[1] }, kept = shot?.(vp, run.asked ?? run.scheme);
      return { screen, size: [vp.w, vp.h], scheme: run.scheme, asked: run.asked, ...(kept ? { shot: kept } : {}), findings: run.findings ?? [], lines: [...failed, ...g.lines], hidden: g.hidden, advice: g.advice };
    }));
  }
  return runs;
}

/** runTests' `atEnd`: a test's last screen once its transitions end, checked in both schemes (the web: at both widths,
 * one capture kept at the width nearer the test's own; iOS: the simulator's), and what the relaunch chooses by. */
const testEnd = (host, shots) => async (s, t) => {
  await s.clock('settle');
  const state = await s.state().catch(() => ({}));
  const own = (t.size ?? [420, 900])[0], near = // the test's own width, else the driver's window (agent.mjs VIEWPORT)
    WEB_SIZES.reduce((a, b) => (Math.abs(b[0] - own) < Math.abs(a[0] - own) ? b : a));
  const path = resolve(shots, shotName(host, t.index, t.name));
  const shot = (vp, scheme) => (scheme === 'light' && (host !== 'web' || vp.w === near[0]) ? path : undefined);
  const facts = await seen(s, state);
  const runs = await checkHere(s, { screen: t.name, sizes: host === 'web' ? WEB_SIZES : [null], shot, state });
  return { keep: true, runs, shot: path, resources: facts.resources, ops: facts.ops, reload: t.reload, launch: t.launch };
};

/** The authored tests: `{ passed, failed, results }` (each with its end screen's `end`, and the store it left), `{ none }`
 * without a file, or `{ error }` when they do not compile. */
async function tests(host, app, shots) {
  const file = resolve(app.dir, 'app.test.contract');
  if (!existsSync(file)) return { none: true };
  const { runTests } = await import('./agent-test.mjs');
  try {
    const r = await runTests({ host, file, app: app.name, atEnd: testEnd(host, shots), ...(host === 'ios' ? { chrome: 'platform' } : {}) });
    return { passed: r.passed, failed: r.failed, results: r.results.map(({ name, failures, end, store }) => ({ name, failures, ...(end ? { end } : {}), ...(store ? { store } : {}) })) };
  } catch (e) { return { error: e.message }; }
}

/** One launch at `size` (null: the device's own) on a fresh scratch store: the app's data landed and its
 * transitions ended, then `check both`, its captures kept in `shots`. A run per scheme, `{ screen, size, scheme,
 * asked, shot, findings, lines, hidden }` (`lines`: check's own text for it, with a line for a resource that
 * failed), and at the phone's width what it shows (`seen`), for the relaunch. */
async function checkAt(host, app, size, store, shots) {
  const [{ open }, { driveStore, removeBrowserProfile }] = await Promise.all([import('./agent.mjs'), import('./agent-launch.mjs')]);
  const fresh = { EXACT_AGENT_STORAGE_FRESH: '1' };
  const s = await open({ host, app: app.name, storage: store, env: fresh, ...(size ? { size } : {}), ...(host === 'ios' ? { chrome: 'platform' } : {}) });
  try {
    await s.clock('data');
    await s.clock('settle');
    const state = await s.state().catch(() => ({}));
    const facts = !size || size === WEB_SIZES[0] ? await seen(s, state) : null;
    const runs = await checkHere(s, { screen: FIRST, shot: (vp, scheme) => resolve(shots, `${host}-${vp.w}x${vp.h}-${scheme}.png`), state });
    if (facts) Object.defineProperty(runs, 'seen', { value: facts });
    return runs;
  } finally {
    await s.close();
    if (host === 'web') await removeBrowserProfile(driveStore(app, store).profile).catch(() => {});
  }
}

const quoted = (text, n = 40) => JSON.stringify(text.length > n ? `${text.slice(0, n - 1)}…` : text);
const same = (a, b) => JSON.stringify(a ?? {}) === JSON.stringify(b ?? {});

/** The store the relaunch opens: a passing test's that wrote storage (more operations than a fresh launch's, or a
 * `reload` step), and of those the one whose resources held the most at its end; null when no test passed. */
export function relaunchStore(results, fresh) {
  const passed = (results ?? []).filter((r) => !r.failures.length && r.store && r.end && !r.end.error);
  const wrote = (r) => r.end.reload || r.end.ops > (fresh?.ops ?? 0);
  const score = (r) => [wrote(r) ? 1 : 0, JSON.stringify(r.end.resources ?? {}).length, results.indexOf(r)];
  const above = (a, b) => { for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return a[i] > b[i]; return false; };
  return passed.reduce((best, r) => (!best || above(score(r), score(best)) ? r : best), null);
}

/** What the relaunch found, from the test whose store it opened (its `end`), a fresh launch's `seen` and its own: the
 * texts its first screen shows that a fresh one does not, what it says of them, and its problems (a line each, counted
 * as findings). What the test stored is lost when the test wrote storage (more operations than a fresh launch, or a
 * `reload` step) and ended holding resources a fresh launch does not, and the relaunch holds a fresh launch's. */
export function judgeRelaunch(test, fresh, after, { busy = [], failed = {} } = {}) {
  const problems = [];
  if (busy.length) problems.push(`  loading  still in flight after ${secs(REAL_MS)} of real time: ${busy.join(', ')}; a store or the network that slow leaves the first screen empty`);
  for (const [name, why] of Object.entries(failed ?? {})) problems.push(`  failed   resource ${name}: ${why}; the screen shows its placeholder or last value`);
  if (!fresh) return { shows: [], problems, said: 'it relaunched' };
  const shows = [...after.texts].filter((t) => !fresh.texts.has(t)), wrote = test.end.reload || test.end.ops > fresh.ops;
  const held = !same(after.resources, fresh.resources), kept = held || !same(after.slots, fresh.slots);
  if (!shows.length && !busy.length && wrote && !same(test.end.resources, fresh.resources) && !held)
    problems.push('  lost     its first screen and resources are a fresh launch\'s, though the test wrote storage and ended holding data: what it stored did not come back');
  const said = shows.length ? `the first screen shows what it stored: ${shows.slice(0, 3).map((t) => quoted(t, 24)).join(', ')}`
    : kept ? 'its state holds what the test left, though the first screen\'s text is a fresh launch\'s'
    : wrote ? 'the first screen is a fresh launch\'s, as the test left it' : 'the first screen is a fresh launch\'s; that test wrote no storage';
  return { shows, problems, said };
}

/** The facts a launch is made under that a test may pin (agent-launch.mjs `launchFacts`): the date at the clock's zero, the
 * zone, the locale and the seed. */
const LAUNCH_FACTS = ['epoch', 'timeZone', 'locale', 'seed'];
const launchOf = (launch) => Object.fromEntries(LAUNCH_FACTS.filter((k) => launch?.[k] != null).map((k) => [k, launch[k]]));

/** Whether `test` launched under facts other than the driver's own, which the first screens launch under: a date, zone,
 * locale or seed its leading lines pinned. What it stored is read back under those facts, and a fresh launch to compare
 * with must be made under them too (LLP 1116.007 D3.4). */
export function pinnedLaunch(test, defaults) {
  const launch = test.end?.launch;
  return !!launch && LAUNCH_FACTS.some((k) => launch[k] != null && launch[k] !== defaults[k]);
}

/** What a fresh launch shows under `launch`'s facts on an empty scratch store, for `seen`'s comparison. */
async function freshUnder(host, app, launch, store) {
  const [{ open }, { driveStore, removeBrowserProfile }] = await Promise.all([import('./agent.mjs'), import('./agent-launch.mjs')]);
  const s = await open({ host, app: app.name, storage: store, env: { EXACT_AGENT_STORAGE_FRESH: '1' }, ...launchOf(launch), ...(host === 'web' ? { size: WEB_SIZES[0] } : { chrome: 'platform' }) });
  try {
    await s.clock('data');
    await s.clock('settle');
    return await seen(s, await s.state().catch(() => ({})));
  } finally {
    await s.close();
    if (host === 'web') await removeBrowserProfile(driveStore(app, store).profile).catch(() => {});
  }
}

/** One launch on the store `test` left, under the facts the test launched with (its date, zone, locale and seed, as a
 * `reload` relaunches), at the phone's width (iOS: the simulator's, with UIKit's bars), not emptied:
 * its first loads on real time, in REAL_STEP steps until nothing is in flight or REAL_MS pass, then its transitions
 * end; then `check both` (the web: at both widths, one capture kept). */
async function relaunch(host, app, test, fresh, shots) {
  const [{ open }, { driveStore, removeBrowserProfile }] = await Promise.all([import('./agent.mjs'), import('./agent-launch.mjs')]);
  const s = await open({ host, app: app.name, storage: test.store, ...launchOf(test.end?.launch), ...(host === 'web' ? { size: WEB_SIZES[0] } : { chrome: 'platform' }) });
  try {
    const started = performance.now(), b = (st) => (st.background?.queued ?? 0) + (st.background?.inFlight ?? 0);
    const waiting = (st) => [...(st.pending ?? []).filter((p) => !p.device).map((p) => p.name), ...(b(st) ? ['storage'] : [])];
    let state, busy;
    do { await s.clock(`+${REAL_STEP} real`); state = await s.state().catch(() => ({})); busy = waiting(state); } while (busy.length && performance.now() - started < REAL_MS);
    const ms = performance.now() - started;
    if (!busy.length) { await s.clock('settle'); state = await s.state().catch(() => state); }
    const after = await seen(s, state);
    const path = resolve(shots, `${host}-relaunch.png`);
    const runs = await checkHere(s, { screen: RELAUNCH, sizes: host === 'web' ? [null, WEB_SIZES[1]] : [null], shot: (vp, scheme) => (scheme === 'light' && (host !== 'web' || vp.w === WEB_SIZES[0][0]) ? path : undefined) });
    return { test: test.name, ms, shot: path, runs, ...judgeRelaunch(test, fresh, after, { busy, failed: state.failed }) };
  } finally {
    await s.close();
    if (host === 'web') await removeBrowserProfile(driveStore(app, test.store).profile).catch(() => {});
  }
}

/** check's text by scheme: each header's finding lines, how many its "… and N more" left out, and its advice
 * (LLP 1116.004 D1.3), which is never a finding. */
export function groups(text) {
  const out = [];
  for (const line of String(text ?? '').split('\n')) {
    if (/^check /.test(line)) { out.push({ head: line, lines: [], hidden: 0, advice: [] }); continue; }
    const g = out.at(-1), more = line.match(/^\s+… and (\d+) more/);
    if (!g || !line.trim()) continue;
    if (more) g.hidden += Number(more[1]);
    else if (/^\s+advice\s/.test(line)) g.advice.push(line);
    else g.lines.push(line);
  }
  return out;
}

const BOX = /\s*\[-?\d+,-?\d+ \d+×\d+\]/g;
const sizeOf = (r) => r.size.join('x'), schemeOf = (r) => r.asked ?? r.scheme, screenOf = (r) => r.screen ?? FIRST;
const screenName = (screen) => (screen === FIRST || screen === RELAUNCH ? screen : quoted(screen, 28));
/** The findings of every screen, launch and scheme, each once: a line found at several (its box aside, which a width
 * moves) is shown once, tagged with the screens it was on unless it was on all, and with the widths and schemes of
 * those screens it was at unless it was at all. */
export function distinctFindings(runs) {
  const seen = new Map(), screens = [...new Set(runs.map(screenOf))];
  for (const r of runs) for (const line of r.lines) {
    const key = line.replace(BOX, '').replace(/\d+-wide/g, '');
    if (!seen.has(key)) seen.set(key, { line, where: new Set() });
    seen.get(key).where.add(r);
  }
  return [...seen.values()].map(({ line, where }) => {
    const at = [...where], on = screens.filter((sc) => at.some((r) => screenOf(r) === sc)), mine = runs.filter((r) => on.includes(screenOf(r)));
    const sizes = new Set(mine.map(sizeOf)), schemes = new Set(mine.map(schemeOf)), ws = [...new Set(at.map(sizeOf))], ss = [...new Set(at.map(schemeOf))];
    const how = at.length === mine.length ? '' : ws.length === sizes.size ? ss.join(', ') : ss.length === schemes.size ? ws.join(', ') : [...new Set(at.map((r) => `${sizeOf(r)} ${schemeOf(r)}`))].join(', ');
    const names = on.length === screens.length ? [] : on.length > 3 ? [...on.slice(0, 2).map(screenName), `${on.length - 2} more`] : on.map(screenName);
    const tag = [...names, how].filter(Boolean).join(', ');
    return tag ? line.replace(/^(\s*)/, `$1(${tag} only) `) : line;
  });
}

/** The report's lines and its problem count, from what each stage gave. */
export function report({ host, built, compiler = [], server, tested, checks, relaunched, shots, copied, ms, cwd = process.cwd() }) {
  const out = [];
  let failedTests = 0, findings = 0, broken = 0;
  // The app's own server (LLP 1116.006 D3): run on an empty database, or why not; one that died mid-run is a failure.
  if (server?.died) { broken++; out.push(`server: ${server.died}`); }
  else if (server) out.push(`server: ${server.said}`);
  if (built) {
    const said = built.said?.length ? built.said.join('\n') : `built ${host}`;
    out.push(`${said} (${secs(built.ms)}${built.warnings.length ? `, ${plural(built.warnings.length, 'warning')}` : ''})`);
    for (const w of built.warnings.slice(0, WARNINGS)) out.push(`  ${w}`);
    if (built.warnings.length > WARNINGS) out.push(`  … and ${built.warnings.length - WARNINGS} more (bun exact.mjs web-build prints them all)`);
    findings += built.warnings.length;
  }
  // The compiler's warnings, each as `contract build` says it (LLP 1116.004 D7): findings, never a failure.
  if (compiler.length) {
    out.push(`compiler: ${plural(compiler.length, 'warning')}`, ...compiler.slice(0, SHOWN).map((w) => `  ${w}`));
    if (compiler.length > SHOWN) out.push(`  … and ${compiler.length - SHOWN} more (bun exact.mjs contract build app.contract prints them all)`);
    findings += compiler.length;
  }
  if (tested?.none) out.push('tests: none (no app.test.contract)');
  else if (tested?.error) {
    broken++;
    out.push(`tests: did not compile`, ...tested.error.split('\n').filter((l) => l.trim()).slice(0, 6).map((l) => `  ${l}`));
  } else if (tested) {
    failedTests = tested.failed;
    out.push(`tests: ${tested.passed} passed, ${tested.failed} failed`);
    for (const t of tested.results) for (const f of t.failures) out.push(`  FAIL "${t.name}": ${firstLine(f).replace(`${t.name}: `, '')}`);
  }
  // Every screen, launch and scheme in one group: each finding once, said where unless it was everywhere.
  for (const e of checks.filter((c) => c.error)) { broken++; out.push(`check ${e.label}: could not drive: ${firstLine(e.error)}`); }
  const runs = checks.filter((c) => !c.error);
  if (runs.length) {
    const shown = distinctFindings(runs), count = shown.length + Math.max(0, ...runs.map((r) => r.hidden));
    const kept = runs.some((r) => r.asked && r.asked !== r.scheme) ? '; the app keeps its own scheme' : '';
    const screens = [...new Set(runs.map(screenOf))], ends = screens.filter((sc) => sc !== FIRST && sc !== RELAUNCH).length;
    const where = [screens.includes(FIRST) && FIRST, ends && plural(ends, 'test end'), screens.includes(RELAUNCH) && RELAUNCH].filter(Boolean).join(', ');
    out.push(`check ${where} · ${[...new Set(runs.map(sizeOf))].join(', ')} · ${[...new Set(runs.map(schemeOf))].join(', ')}${kept}: ${count ? plural(count, 'finding') : 'no findings'}`);
    out.push(...shown.slice(0, SHOWN));
    if (count > SHOWN) out.push(`  … and ${count - Math.min(SHOWN, shown.length)} more (verify --json lists them all)`);
    out.push(...new Set(runs.flatMap((r) => r.advice ?? [])));
    findings += count;
  }
  // The relaunch on a test's store (LLP 1116.005 D7): what came back, or each problem, counted as findings.
  if (relaunched?.error) { broken++; out.push(`relaunch: could not drive: ${firstLine(relaunched.error)}`); }
  else if (relaunched) {
    out.push(`relaunch on ${quoted(relaunched.test)}'s store (${secs(relaunched.ms)} of real time): ${relaunched.problems.length ? plural(relaunched.problems.length, 'problem') : relaunched.said}`, ...relaunched.problems);
    findings += relaunched.problems.length;
  }
  if (shots.length) {
    const place = (p) => { const r = relative(cwd, p); return r.startsWith('..') ? p : r || '.'; }, name = (p) => relative(resolve(p, '..'), p);
    const first = shots.filter((p) => /^\w+-\d+x\d+-(light|dark)\.png$/.test(name(p))), ends = shots.filter((p) => /^\w+-t\d+[-.]/.test(name(p))), again = shots.filter((p) => /^\w+-relaunch\.png$/.test(name(p)));
    out.push(`screenshots (${place(resolve(shots[0], '..'))}${copied ? `, copied to ${place(copied)}` : ''}): ${first.map(name).join(' ')}`);
    const more = [ends.length && `test ends: ${ends.map(name).join(' ')}`, again.length && `relaunch: ${again.map(name).join(' ')}`].filter(Boolean);
    if (more.length) out.push(`  ${more.join('; ')}`);
  }
  // How a person opens what ran (LLP 1116.008 D2): the dev loop serves the web app, the simulator build launches on iOS.
  if (!failedTests && !broken) out.push(host === 'ios' ? 'open it: bun exact.mjs ios --run (launches what verify checked)' : 'open it: bun exact.mjs web (serves what verify checked)');
  const problems = failedTests + findings + broken;
  const why = [failedTests && plural(failedTests, 'failed test'), broken && plural(broken, 'failure', 'failures') + ' to compile or drive', findings && plural(findings, 'finding')].filter(Boolean);
  out.push(problems ? `verify: ${plural(problems, 'problem')} (${why.join(', ')}${failedTests || broken ? '' : '; the tests passed'}) in ${secs(ms)}` : `verify: clean in ${secs(ms)}`);
  return { lines: out, problems, exit: failedTests || broken ? 1 : 0 };
}

async function main(argv) {
  const args = guardFlags('verify', argv);
  const value = (flag) => args.indexOf(flag) >= 0 ? args[args.indexOf(flag) + 1] : undefined;
  const json = args.includes('--json'), sim = value('--sim'), copyTo = value('--shots');
  const flagged = new Set([args.indexOf('--app') + 1, args.indexOf('--sim') + 1, args.indexOf('--shots') + 1].filter((i) => i > 0));
  if (args.includes('--shots') && (!copyTo || copyTo.startsWith('-'))) { console.error('verify: --shots takes a directory: the screenshots are copied there, under the names in .exact/verify/'); return 2; }
  const host = args.find((a, i) => !a.startsWith('-') && !flagged.has(i)) ?? 'web';
  if (!['web', 'ios'].includes(host)) { console.error(`verify: web or ios, not ${host} (test ${host} runs the tests there)`); return 2; }
  if (sim) process.env.EXACT_SIM = sim; // as the driver names it (devices.mjs `simulator`)
  const started = Date.now();
  const { resolveApp } = await import('./app.mjs');
  const app = resolveApp(value('--app'));
  const say = (lines) => console.log(lines.join('\n'));
  // An app with a server of its own: verify runs it where the app calls it, on an empty database. One already
  // answering there (the builder's dev server) is refused before the build: never stopped, never written to; so is
  // an entry that does not read DB (LLP 1116.008 D1), which could only be run on its own database.
  const server = appServer(app.dir), owned = server && !server.why;
  const refuse = (line) => { if (json) console.log(JSON.stringify({ host, server: { error: line }, problems: 1, exit: 1 })); else console.error(`verify: ${line}`); return 1; };
  if (server?.refusal) return refuse(server.refusal);
  if (owned && await answering(server.url)) return refuse(heldLine(server));

  const built = host === 'web' ? await webBuild(app) : await iosBuild(app, sim);
  // The build said the compiler's warnings when it ran; the report's own group says them whether it ran or not.
  if (built?.said) { const { compilerWarnings } = await import('./agent-launch.mjs'); built.said = built.said.filter((line) => !compilerWarnings(line).length); }
  if (built?.code) {
    const tail = host === 'web' ? built.output.split('\n').filter((l) => l.trim()).slice(-20) : built.said;
    if (json) console.log(JSON.stringify({ host, built: { ms: built.ms, code: built.code, said: tail }, problems: 1, exit: built.code }));
    else say([...tail, `verify: the ${host} build failed (exit ${built.code}), so nothing ran`]);
    return built.code;
  }

  const shots = resolve(app.dir, '.exact', 'verify');
  mkdirSync(shots, { recursive: true });
  for (const f of readdirSync(shots)) if (f.startsWith(`${host}-`) && f.endsWith('.png')) rmSync(resolve(shots, f), { force: true });
  // The web's scratch stores are this run's own, removed after it (and any a killed run left, as a test's are).
  const tag = `r${process.pid}-${Math.random().toString(36).slice(2, 8)}`;
  if (host === 'web') {
    const [{ driveStore }, { sweepTestStores }] = await Promise.all([import('./agent-launch.mjs'), import('./agent-test.mjs')]);
    sweepTestStores(driveStore(app, 'verify').base, 'verify');
  }
  const launch = (size, n) => checkAt(host, app, size, host === 'web' ? `verify.${tag}.t${n}` : 'verify', shots)
    .catch((e) => [{ size: null, label: size ? size.join('x') : host, error: e.message }]);
  let tested, firsts, relaunched = null, results = [], running = null, serverSaid = server?.why ? { said: `${server.why}; the tests use whatever answers there` } : null;
  const compiling = compilerWarningsOf(app);
  if (owned) {
    try { running = await startServer(app.dir, server, 'verify'); }
    catch (e) { return refuse(e.message); }
    serverSaid = { said: `bun ${server.entry} at ${server.url}, on an empty database (.exact/verify/db.sqlite, removed after)` };
  }
  try {
    // An app with a server opens its first screen before the tests write to it: an empty database's.
    if (running) { firsts = host === 'web' ? await Promise.all(WEB_SIZES.map(launch)) : [await launch(null, 0)]; tested = await tests(host, app, shots); }
    else if (host === 'web') [tested, ...firsts] = await Promise.all([tests(host, app, shots), ...WEB_SIZES.map(launch)]);
    else { tested = await tests(host, app, shots); firsts = [await launch(null, 0)]; }
    results = tested?.results ?? [];
    const fresh = firsts.find((runs) => runs.seen)?.seen ?? null;
    // The relaunch, on the store the chosen test left; the others' (the web's: Chrome profiles of this run) go after it.
    const chosen = relaunchStore(results, fresh);
    if (chosen) {
      // A test that pinned its date (or zone, locale, seed) left data the driver's default date may not show, and a
      // first screen of another date differs from the relaunch's for that alone: compare with a fresh launch under the
      // test's facts. An app with a server has no fresh database after the tests: its relaunch is not compared.
      relaunched = await (async () => {
        const { launchFacts } = await import('./agent-launch.mjs');
        const against = !pinnedLaunch(chosen, launchFacts({})) ? fresh : running ? null : await freshUnder(host, app, chosen.end.launch, host === 'web' ? `verify.${tag}.fresh` : 'verify');
        return relaunch(host, app, chosen, against, shots);
      })().catch((e) => ({ test: chosen.name, error: e.message }));
    }
  } finally {
    if (host === 'web') {
      const { driveStore, removeBrowserProfile } = await import('./agent-launch.mjs');
      for (const r of results) if (r.store) await removeBrowserProfile(driveStore(app, r.store).profile).catch(() => {});
    }
    if (running) {
      const died = running.exited();
      if (died != null) serverSaid = { died: `${server.entry} exited (code ${died}) during the run${running.tail() ? `: ${running.tail()}` : ''}` };
      await running.stop();
    }
  }
  // A test's end screen that could not be checked fails the run, unless the test failed already (its FAIL line says why).
  const ends = results.flatMap((t) => (t.end?.error ? (t.failures.length ? [] : [{ size: null, label: `${quoted(t.name)} end`, error: t.end.error }]) : t.end?.runs ?? []));
  const checks = [...firsts.flat(), ...ends, ...(relaunched?.runs ?? [])];
  const compiler = await compiling;
  const kept = [...new Set([...checks.map((c) => c.shot), relaunched?.shot])].filter((p) => p && existsSync(p));
  // `--shots <dir>`: every capture of this run copied out, under its own name (LLP 1116.008 D2).
  if (copyTo) { mkdirSync(resolve(copyTo), { recursive: true }); for (const p of kept) copyFileSync(p, resolve(copyTo, basename(p))); }
  const r = report({ host, built, compiler, server: serverSaid, tested, checks, relaunched, shots: kept, copied: copyTo && resolve(copyTo), ms: Date.now() - started });
  if (json) {
    const testsOut = tested?.results ? { ...tested, results: results.map(({ name, failures, end }) => ({ name, failures, ...(end?.shot && existsSync(end.shot) ? { shot: end.shot } : {}), ...(end?.error ? { endError: end.error } : {}) })) } : tested;
    const relaunchOut = relaunched && (({ runs, ...rest }) => rest)(relaunched);
    console.log(JSON.stringify({ host, built: built && { ms: built.ms, said: built.said ?? [], warnings: built.warnings }, compiler, ...(serverSaid ? { server: serverSaid } : {}), tests: testsOut, checks, relaunch: relaunchOut, screenshots: kept, problems: r.problems, exit: r.exit, ms: Date.now() - started }));
  } else say(r.lines);
  return r.exit;
}

if (import.meta.main) main(process.argv.slice(2)).then((code) => process.exit(code), (e) => { console.error(e.message); process.exit(1); });
