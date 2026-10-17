#!/usr/bin/env bun
// `bun exact.mjs verify [web|ios]` (LLP 1116.004 D7, 1116.005 D7): one command a builder can stop on, where test,
// drives, check and screenshots were three to five turns. It builds when the build is stale (web: the dist's
// record against its sources, as the driver checks it; iOS: agent-build.mjs `buildFirst`, the plan fast path
// included), runs app.test.contract (agent-test.mjs `runTests`), launches the app on a fresh scratch store at a
// phone width and a desktop width (iOS: the simulator's own size, with UIKit's bars), and runs `check` light and
// dark there. Each test's end screen is checked the same way inside the test's own session (the web at both
// widths, resized to each), so its secondary screens and populated states are in the report. Then the app
// relaunches once on the store a passing test left, on real time (at most REAL_MS) and under the zone, locale and seed
// the test launched with and the date it had reached (its launch date plus its `clock` steps, as a `reload` relaunches,
// LLP 1116.007 D3.4, LLP 1116.009 D1), and its first screen is compared with a fresh launch's under the same (the
// driver's, for an app whose server holds the tests' writes): what the test stored should come back. On the web each test's end screen is also
// reloaded where it stands (the test's `reload` step: the page loads again on its URL and storage, at the date it had reached) and
// its texts compared before and after (LLP 1116.010 D6; `judgeReload` says what it forgives): a screen that reloads into another state
// ('Card 2 of 3' → '0 / 0') is a note, not a problem, since a game or a draft in progress lives in memory by design (LLP 1116.011 D5);
// it is a finding only when a saved value came back different (`savedChanges`: a `persist` state, a field showing what the app kept).
// A native host's cold launch is `/` (LLP 1038 §7), so iOS has no such reload to compare. A refusal
// the host journaled during the run (`logs`: 'swipeContent … is refused: …') is shown first in the report (LLP 1116.010 D5). One capture of each screen is kept in
// .exact/verify/ for milestones (four of the first screen: each width and scheme). It prints one short report
// ending "verify: clean" or "verify: N problems" (`--json`: one object). Exit 1 when the build, a test, a launch
// or the relaunch fails; check's findings, the relaunch's, those at the largest text size and the compiler's warnings
// (`contract build`'s, whether or not this run built) are counted in the last line but never fail it, since check reads the screen
// by heuristics and some of either only advise. The web's tests and its two launches run at once; a simulator
// takes one drive at a time, and runs the tests with UIKit's bars (`--chrome platform`) so their end screens
// are what a person sees. An app with a server of its own (the `rest` pattern's) has it run by verify, where the
// app calls it, on an empty scratch database kept through the run and removed after (LLP 1116.006 D3, `appServer`); a server
// entry that does not read DB is refused before the build, in one line naming it and the fix (LLP 1116.008 D1).
// Each of those screens is read again at the largest text size (iOS: the simulator's; the web: iOS's body as the root font
// size, at the phone's width), and what spills there (past the screen's edge, squeezed to no width, overlapping) is reported
// with a remedy that keeps the text's size, counted in the last line and never failing the run (LLP 1116.019 D2, LLP
// 1116.020 D4, `largestText`).
//
//   bun scripts/verify.mjs [web|ios] --app <name> [--sim <udid|name>] [--shots <dir>] [--json]
import { spawn, spawnSync } from 'node:child_process';
import { closeSync, copyFileSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, rmdirSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { guardFlags } from './help.mjs';
import { holderOf, takePort } from './port-lock.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
/** The web's two widths: a phone's, and a desktop window's. */
export const WEB_SIZES = [[390, 844], [1280, 900]];
const SHOWN = 6, WARNINGS = 4; // the findings and build warnings shown; the count says the rest
const REAL_MS = 4000, REAL_STEP = 250; // the relaunch's bound on real time for its first loads, in steps
const FIRST = 'first screen', RELAUNCH = 'relaunch';
let ACCEPT = []; // app.json `verify.accept`: findings the app's design accepts, set by main
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
  // Its port in turn with other runs (scripts/port-lock.mjs), then refused
  // if something that took no turn (a server run by hand) answers there.
  const give = await takePort(server.port, { label: `${name}: ${server.entry} in ${dir}` });
  try { return await started(dir, server, name, forward, give); } catch (e) { give(); throw e; }
}

async function started(dir, server, name, forward, give) {
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
    try { clear(); } finally { give(); }
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

/** The texts a screen shows, in tree order: each active node's text and field value, and a switch or checkbox as
 * `Label: on|off` (what it holds is what a person sees). A live region (`role` alert, status, log) is a message
 * the screen says once, not state, so it is left out; so are the values of the fields named in `leave` (by `testId`). */
export function screenTexts(nodes, leave = new Set()) {
  const out = [];
  let live = null;
  for (const n of nodes ?? []) {
    if (live != null) { if (n.depth > live) continue; live = null; }
    if (n.inactive) continue;
    const p = n.props ?? {};
    if (['alert', 'status', 'log'].includes(p.accessibilityRole)) { live = n.depth; continue; }
    if (typeof p.text === 'string' && p.text.trim()) out.push(p.text.trim());
    if (typeof p.value === 'string' && p.value.trim() && !leave.has(p.testId)) out.push(p.value.trim());
    const label = p.accessibilityLabel ?? p.testId;
    if (typeof p.checked === 'boolean' && label) out.push(`${label}: ${p.checked ? 'on' : 'off'}`);
  }
  return out;
}

/** Every string and number the app holds after a reload, in its state and resources (what its storage gave back), as
 * text: `30` and `"30"` both give "30". */
export function storedTexts(state) {
  const out = new Set();
  const walk = (v, depth) => {
    if (depth > 8 || v == null) return;
    if (typeof v === 'string') out.add(v.trim());
    else if (typeof v === 'number' || typeof v === 'boolean') out.add(String(v));
    else if (typeof v === 'object') for (const x of Object.values(v)) walk(x, depth + 1);
  };
  walk(state?.slots, 0);
  walk(state?.resources, 0);
  return out;
}

/** What a reload did to the fields the test typed into: `drafts`, which a reload is not expected to bring back (a calculator's
 * amount, a form the app refused with its error), so their values are left out of the comparison; and `saved`, the fields that
 * showed a value the app kept and now show another. A field is compared like any text only when the app kept what it showed and
 * the reloaded field shows something it did not keep: focus-timer's settings reloaded with 25 and 5 over the saved 30 and 10.
 * What the app kept is what its state and resources hold both before and after the reload (`storedTexts` of each); a field that
 * shows what is kept after the reload (the water tracker's refused 0, then its saved goal) is the app doing as it should. */
function typedFields(before, after, touched, states) {
  const was = storedTexts(states?.before), now = storedTexts(states?.after);
  const kept = new Set([...was].filter((t) => now.has(t))), has = (t) => t !== '' && (kept.has(t) || (Number.isFinite(Number(t)) && kept.has(String(Number(t)))));
  const value = (n) => String(n.props?.value ?? '').trim(), field = (n, id) => n.type === 'TextInput' && !n.inactive && n.props?.testId === id;
  const ids = (before ?? []).filter((n) => n.type === 'TextInput' && !n.inactive && (touched ?? []).includes(n.props?.testId)).map((n) => n.props.testId);
  const drafts = new Set(), saved = [];
  for (const id of ids) {
    const there = (before ?? []).find((n) => field(n, id)), here = (after ?? []).find((n) => field(n, id));
    if (!(has(value(there)) && (!here || !has(value(here))))) drafts.add(id);
    else if (here) saved.push({ kind: 'field', id, was: value(there), now: value(here) });
  }
  return { drafts, saved };
}

/** The fields to leave out of a reload's comparison: the drafts (`typedFields`). */
export const draftFields = (before, after, touched, states) => typedFields(before, after, touched, states).drafts;

/** The root states the app says `persist` on (LLP 1116 D5), by name, read from app.contract: only the root's may, and the root is
 * its first component. A trailing comment is not read. */
export function persistedStates(dir) {
  const names = [];
  for (const line of (readIn(dir, 'app.contract') ?? '').split('\n')) {
    const m = line.replace(/\s\/\/.*$/, '').match(/^\s*state\s+([A-Za-z_][\w-]*)\s*=.*\spersist\s*$/);
    if (m) names.push(m[1]);
  }
  return names;
}

/** What a reload did to a saved value, from the screen's fields (`typedFields`) and the persisted states (`persistedStates`):
 * the `persist` states whose value came back different, and the fields that showed a kept value and now show another. These
 * are the only evidence that something saved was lost; a game's or a draft's state is in memory and is not (LLP 1116.011 D5).
 * `[{ kind: 'state', name, was, now } | { kind: 'field', id, was, now }]`. */
export function savedChanges({ persisted = [], states, nodes, touched }) {
  const was = states?.before?.slots ?? {}, now = states?.after?.slots ?? {};
  const slots = persisted.filter((name) => name in was && JSON.stringify(was[name]) !== JSON.stringify(now[name]))
    .map((name) => ({ kind: 'state', name, was: was[name], now: now[name] }));
  return [...slots, ...typedFields(nodes?.before, nodes?.after, touched, states).saved];
}

/** A reload that only takes this many texts away, and puts none in their place, took a message with it (a toast, a
 * banner, a validation error, an open dialog's words), not state. */
const GONE = 3;
/** A clock face: `24:49`, `1:02:03`, `00:12.5`. A running timer shows another one after a reload, and a pinned clock
 * does not make it the same: the elapsed time lived in memory (LLP 1116.010 D6). */
const CLOCK_FACE = /\d{1,3}:\d{2}(?::\d{2})?(?:[.,]\d+)?/g;
const faceless = (text) => text.replace(CLOCK_FACE, '§');

/** What a reload changed on a screen, from the texts shown before and after (`screenTexts`) and the URLs; null when
 * nothing a person would notice. Forgiven, with the guard that says so: a clock face (its digits are masked); a live
 * region (`screenTexts` leaves it out); texts that only went (at most GONE, nothing in their place: a toast, a validation
 * error, an open dialog). What is left is another state: `{ route, now?, lost, gained }`, the texts before and after. */
export function judgeReload(before, after) {
  const counts = (texts) => { const m = new Map(); for (const t of texts) m.set(faceless(t), (m.get(faceless(t)) ?? 0) + 1); return m; };
  const without = (texts, other) => { const m = counts(other); return texts.filter((t) => { const k = faceless(t), n = m.get(k) ?? 0; if (n) m.set(k, n - 1); return !n; }); };
  const lost = without(before.texts, after.texts), gained = without(after.texts, before.texts);
  if ((!lost.length && !gained.length) || (!gained.length && lost.length <= GONE)) return null;
  return { route: before.url ?? '/', ...(after.url != null && after.url !== before.url ? { now: after.url } : {}), lost, gained };
}

const texts3 = (texts) => `${texts.slice(0, 3).map((t) => quoted(t, 28)).join(', ')}${texts.length > 3 ? `, … (${texts.length - 3} more)` : ''}`;
const saying = (v) => quoted(typeof v === 'string' ? v : JSON.stringify(v) ?? 'none', 24);
/** The finding's line (LLP 1116.011 D5): the route and the saved values that came back different, and the repair. */
export function reloadLine(f, test) {
  const what = f.saved.map((c) => (c.kind === 'state' ? `the saved state \`${c.name}\` was ${saying(c.was)} and is ${saying(c.now)}` : `the field ${quoted(c.id)} showed ${saying(c.was)}, which the app holds, and shows ${saying(c.now)}`));
  return `  reload   ${f.route}${f.now ? `, landing on ${f.now},` : ''} a saved value came back different: ${what.join('; ')}; a reload must bring back what was saved (${quoted(test, 36)})`;
}

/** The note's line (LLP 1116.010 D6, 1116.011 D5): the route, and the texts that changed. Not counted. */
export function reloadNote(n, test) {
  const what = n.lost.length && n.gained.length ? `${texts3(n.lost)} → ${texts3(n.gained)}` : n.lost.length ? `loses ${texts3(n.lost)}` : `gains ${texts3(n.gained)}`;
  return `  note     ${n.route} reloads into another state${n.now ? `, landing on ${n.now}` : ''}: ${what} (${quoted(test, 36)})`;
}

/** What the notes say once: the reload check cannot tell a game in progress from a lost value, so it asks. */
const NOTE_ADVICE = '  a saved value must survive a reload; a game or a draft in progress may not';

/** A line of the journal or the host's output that says the host refused what the app asked (LLP 1116.010 D5), with the
 * journal's `t=N` and the host's `app: exact: ` taken off. A refusal the app's own rule makes when a person tries (a
 * dismissal its `closedby` refuses, a Back its control disables) and a connection a `fail fetch` refused are the app's design. */
export function refusalsOf({ lines = [], host = [] } = {}) {
  const found = [];
  for (const raw of [...lines, ...host]) {
    const line = String(raw).replace(/^t=\d+(?:\.\d+)?\s+/, '').replace(/^(?:app|console\.\w+|chrome|warning|error): /, '').replace(/^exact: /, '').trim();
    if (/\brefus(?:ed|es|al)\b/i.test(line) && !/\b(?:dismissal|back|back gesture|drag) refused\b|connection[ _]refused/i.test(line) && !found.includes(line)) found.push(line);
  }
  return found;
}

/** A session's refusals so far (`refusalsOf` its `logs`). */
const refusalsIn = async (s) => refusalsOf(await s.logs().catch(() => ({})));

/** The end screen reloaded where it stands (the test's `reload` step, run on the session; the web only), and what it
 * showed before and after compared (`judgeReload`): `finding` when a saved value came back different (`savedChanges`),
 * else `note` for any other change, which is the app's state in memory and no problem (LLP 1116.011 D5). `{ ran: false }` when
 * there is nothing to compare: a native host, a failed test, or one that ended on a draft it typed and did not submit (a page
 * keeps none). `persisted` names the app's `persist` states. */
async function reloadEnd(s, t, persisted = []) {
  if (!t.restart || t.failures.length) return { ran: false };
  const read = async () => ({ state: await s.state().catch(() => ({})), nodes: (await s.tree().catch(() => ({ nodes: [] }))).nodes ?? [] });
  const before = await read();
  if (before.nodes.some((n) => n.type === 'TextInput' && !n.inactive && (t.typed ?? []).includes(n.props?.testId))) return { ran: false };
  try {
    await t.restart();
    await s.clock('settle');
    const after = await read();
    const states = { before: before.state, after: after.state };
    const leave = draftFields(before.nodes, after.nodes, t.touched, states);
    const shown = ({ state, nodes }) => ({ url: state.navigation?.url ?? null, texts: screenTexts(nodes, leave) });
    const changed = judgeReload(shown(before), shown(after));
    const saved = savedChanges({ persisted, states, nodes: { before: before.nodes, after: after.nodes }, touched: t.touched });
    return saved.length
      ? { ran: true, finding: { route: changed?.route ?? shown(before).url ?? '/', ...(changed?.now ? { now: changed.now } : {}), lost: changed?.lost ?? [], gained: changed?.gained ?? [], saved }, note: null }
      : { ran: true, finding: null, note: changed };
  } catch (e) { return { ran: true, error: e.message }; }
}

/** `check both` on the session at each of `sizes` (null: as it is; else resized to it first), a run per size and scheme
 * tagged with its `screen`; a resource that failed is a line of each. `shot(viewport, scheme)` names a capture to keep. */
async function checkHere(s, { screen, sizes = [null], shot, state }) {
  const { checkOp } = await import('./agent-check.mjs');
  const failed = Object.entries(state?.failed ?? {}).map(([name, why]) => `  failed   resource ${name}: ${why}; the screen shows its placeholder or last value`);
  const runs = [];
  for (const size of sizes) {
    if (size) { await s.resize(...size); await s.clock('settle'); }
    const r = await checkOp(s, ['both'], { shots: shot, accept: ACCEPT });
    runs.push(...groups(r.text).map((g, i) => {
      const run = r.check[i] ?? {}, vp = run.viewport ?? { w: size?.[0], h: size?.[1] }, kept = shot?.(vp, run.asked ?? run.scheme);
      return { screen, size: [vp.w, vp.h], scheme: run.scheme, asked: run.asked, ...(kept ? { shot: kept } : {}), findings: run.findings ?? [], accepted: run.accepted ?? [], lines: [...failed, ...g.lines], hidden: g.hidden, advice: g.advice };
    }));
  }
  return runs;
}

// LLP 1116.019 D2: each screen verify visits is read again at the largest text size, and what spills is reported, counted
// and never failing the run (LLP 1116.020 D4: uncounted, half of the builders who saw them stopped at "verify: clean"): the
// layout is the builder's (LLP 1115.000 D12.1), and a failure would invite `overflow: hidden`. iOS sets the
// simulator's own size, which UIKit's controls and Exact's text both follow; the web, where a builder iterates, sets the root
// font size to iOS's body at that size on the phone's width. Only the kinds of finding a larger text makes are kept, each with
// a remedy that keeps the text's size: a smaller font is the platform tell this pass exists to prevent.
export const LARGEST = 'accessibility-extra-extra-extra-large', LARGEST_PX = 53;
const LARGE_KINDS = new Set(['overflow', 'clipped', 'overlap', 'squeezed']), PAGES = 6;
const WRAP = 'wrap the row: flex-wrap="wrap" with a flex-basis in rem on its items (docs/recipes/responsive-columns.md)';
/** The remedy for a finding at the largest size: never a smaller or fixed font. */
export function largeRemedy(f) {
  if (f.kind === 'clipped') return f.detail?.edge ? 'let the page scroll there (a scroll with flex=1 min-height=0)'
    : f.detail?.need ? 'let its box grow with its text: no px width or height on it (a rem size, or none)' : `give the box that cuts it room (no px width or height on it), or ${WRAP}`;
  if (f.kind === 'squeezed') return `let it keep its words (no min-width: 0 or px width beside it), or ${WRAP}`;
  return WRAP;
}
/** LLP 1116.020 D4: each font size the app's own Contract fixes (no-tells.mjs `fixedSizesIn`), which no spill shows (a fixed
 * size never grows to spill): `{ lines, accepted }`, a line each, those app.json's `verify.accept` takes (`fixed-type`, the
 * node's testId) apart. Exact's parts that are fixed on purpose are the library's, not here. */
export async function fixedType(dir, host, accept = ACCEPT) {
  const { contractFiles, fixedSizesIn } = await import('./no-tells.mjs'), unit = host === 'ios' ? 'pt' : 'px', lines = [], accepted = [];
  for (const file of contractFiles(dir).filter((f) => !f.endsWith('.test.contract'))) {
    for (const f of fixedSizesIn(readFileSync(file, 'utf8'))) {
      const rem = Math.round(f.size / 16 * 4) / 4;
      const said = `  fixed    ${relative(dir, file)}:${f.line} ${f.tag} font-size=${f.value}: stays ${f.size} ${unit} as Dynamic Type grows; a role (\`font="-exact-title1"\`) or a \`rem\` size ("${rem}rem") scales`;
      const takes = f.testId != null && accept.find((e) => e.finding === 'fixed-type' && (/^[^*]+\*$/.test(e.node) ? f.testId.startsWith(e.node.slice(0, -1)) : e.node === f.testId));
      if (takes) accepted.push({ line: said, accept: takes }); else lines.push(said);
    }
  }
  return { lines, accepted };
}
const contentSize = (udid, size) => spawnSync('xcrun', ['simctl', 'ui', udid, 'content_size', ...(size ? [size] : [])], { encoding: 'utf8' });

/** The screen `s` shows, again at the largest text size: its findings of LARGE_KINDS as check words them with their remedy,
 * the app's `verify.accept` taken out, `{ screen, size, rootFontSize, lines, ms }`; or `{ screen, error }`. The simulator's
 * own size is put back after, and when the process ends first. Null on iOS with no simulator to set. */
async function largestText(s, screen) {
  const web = s.host === 'web', udid = s.carrier?.udid, was = web ? null : udid && contentSize(udid).stdout?.trim();
  if (!web && !was) return null;
  const started = Date.now(), body = async () => (await s.prefer({})).page?.['root-font-size'];
  const before = web ? null : await body();
  const restore = () => { if (!web && was !== LARGEST) contentSize(udid, was); }, forget = web ? () => {} : whenEnded(restore);
  try {
    if (web) {
      // Each session it reads closes after it, so the page's own size is not put back.
      await s.resize(...WEB_SIZES[0]);
      await s.prefer({ 'root-font-size': LARGEST_PX });
    } else if (was !== LARGEST) {
      const set = contentSize(udid, LARGEST);
      if (set.status !== 0) return { screen, error: `simctl ui content_size: ${firstLine(set.stderr)}` };
      // The app hears the change as a notification of its own: until then its body size is the one it had.
      for (const end = Date.now() + 3000; (await body()) === before && Date.now() < end;) await new Promise((r) => setTimeout(r, 50));
    }
    await s.clock('settle');
    const [{ judge, line, accepts }, tree] = await Promise.all([import('./agent-check.mjs'), s.op({ op: 'tree' })]);
    const nodes = new Map((tree.nodes ?? []).map((n) => [n.id, n])), looked = new Set(), found = new Map();
    let obs;
    // A screen at a time, as check reads one: the page is scrolled to the first node with a testId that no reading has
    // held yet, at most PAGES times (a list's rows off the screen have no cells to read).
    for (let page = 0; page < PAGES; page++) {
      obs = await s.op({ op: 'check' });
      for (const it of obs.items ?? []) looked.add(it.id).add(it.of);
      for (const f of judge(obs, null, { host: web ? 'web' : 'ios' })) if (LARGE_KINDS.has(f.kind) && !accepts(ACCEPT, f, nodes) && !found.has(`${f.kind} ${f.id}`)) found.set(`${f.kind} ${f.id}`, f);
      let moved = false;
      for (const n of tree.nodes ?? []) {
        if (n.inactive || n.props?.testId == null || looked.has(n.id)) continue;
        looked.add(n.id);
        if ((await s.carrier.reveal(n.id).catch(() => null))?.scrolled) { moved = true; break; }
      }
      if (!moved) break;
    }
    return { screen, size: [obs.viewport.w, obs.viewport.h], rootFontSize: web ? LARGEST_PX : await body(), lines: [...found.values()].map((f) => line({ ...f, hint: largeRemedy(f) }, nodes)), ms: Date.now() - started };
  } catch (e) { return { screen, error: e.message }; }
  finally { restore(); forget(); }
}

/** runTests' `atEnd`: a test's last screen once its transitions end, checked in both schemes (the web: at both widths,
 * one capture kept at the width nearer the test's own; iOS: the simulator's), and what the relaunch chooses by. */
const testEnd = (host, shots, persisted) => async (s, t) => {
  await s.clock('settle');
  const state = await s.state().catch(() => ({}));
  const own = (t.size ?? [420, 900])[0], near = // the test's own width, else the driver's window (agent.mjs VIEWPORT)
    WEB_SIZES.reduce((a, b) => (Math.abs(b[0] - own) < Math.abs(a[0] - own) ? b : a));
  const path = resolve(shots, shotName(host, t.index, t.name));
  const shot = (vp, scheme) => (scheme === 'light' && (host !== 'web' || vp.w === near[0]) ? path : undefined);
  const facts = await seen(s, state);
  const runs = await checkHere(s, { screen: t.name, sizes: host === 'web' ? WEB_SIZES : [null], shot, state });
  const refusals = await refusalsIn(s);
  const reloaded = await reloadEnd(s, t, persisted);
  const large = await largestText(s, t.name);
  return { keep: true, runs, shot: path, resources: facts.resources, ops: facts.ops, reload: t.reload, launch: t.launch, reached: t.reached, refusals, reloaded, large };
};

/** The authored tests: `{ passed, failed, results }` (each with its end screen's `end`, and the store it left), `{ none }`
 * without a file, or `{ error }` when they do not compile. */
async function tests(host, app, shots) {
  const persisted = persistedStates(app.dir);
  const file = resolve(app.dir, 'app.test.contract');
  if (!existsSync(file)) return { none: true };
  const { runTests } = await import('./agent-test.mjs');
  try {
    const r = await runTests({ host, file, app: app.name, atEnd: testEnd(host, shots, persisted), ...(host === 'ios' ? { chrome: 'platform' } : {}) });
    return { passed: r.passed, failed: r.failed, results: r.results.map(({ name, failures, end, store, shots }) => ({ name, failures, ...(shots ? { shots } : {}), ...(end ? { end } : {}), ...(store ? { store } : {}) })) };
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
    Object.defineProperty(runs, 'refusals', { value: await refusalsIn(s) });
    if (host === 'ios' || size === WEB_SIZES[0]) Object.defineProperty(runs, 'large', { value: await largestText(s, FIRST) });
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

/** The facts a relaunch on the store `test` left is made under: its launch facts, the date being the one it had reached
 * (its launch date plus its `clock` steps), as a `reload` relaunches (LLP 1116.009 D1): a phone restarted at 9:05 wakes at 9:05. */
export const relaunchFacts = (test) => ({ ...test.end?.launch, ...(test.end?.reached != null ? { epoch: test.end.reached } : {}) });

/** Whether the test ended at a date other than the one it launched at: its relaunch's first screen may say another day,
 * so a fresh launch to compare with is made at that date too. */
export const movedClock = (test) => test.end?.reached != null && test.end.launch?.epoch != null && test.end.reached !== test.end.launch.epoch;

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

/** One launch on the store `test` left, under the facts the test ended with (its zone, locale and seed, and the date it had
 * reached, as a `reload` relaunches), at the phone's width (iOS: the simulator's, with UIKit's bars), not emptied:
 * its first loads on real time, in REAL_STEP steps until nothing is in flight or REAL_MS pass, then its transitions
 * end; then `check both` (the web: at both widths, one capture kept). */
async function relaunch(host, app, test, fresh, shots) {
  const [{ open }, { driveStore, removeBrowserProfile }] = await Promise.all([import('./agent.mjs'), import('./agent-launch.mjs')]);
  const s = await open({ host, app: app.name, storage: test.store, ...launchOf(relaunchFacts(test)), ...(host === 'web' ? { size: WEB_SIZES[0] } : { chrome: 'platform' }) });
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
    const refusals = await refusalsIn(s), large = await largestText(s, RELAUNCH);
    return { test: test.name, ms, shot: path, runs, refusals, large, ...judgeRelaunch(test, fresh, after, { busy, failed: state.failed }) };
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
export function report({ host, built, compiler = [], server, tested, checks, relaunched, refused = [], reloads = [], large = [], fixed = { lines: [], accepted: [] }, shots, copied, own = [], accept = [], ms, cwd = process.cwd() }) {
  const out = [];
  let failedTests = 0, findings = 0, broken = 0;
  // What the host refused while the app ran (LLP 1116.010 D5): first, since each is a call or a wiring the app got wrong.
  if (refused.length) {
    const shownAt = (where) => where.length > 2 ? `${where.slice(0, 2).join(', ')} and ${where.length - 2} more` : where.join(', ');
    out.push(`refused at run time: ${refused.length} (the host's journal, 'logs'; each is a call or a wiring the app got wrong)`,
      ...refused.slice(0, SHOWN).map((r) => `  ${r.line.length > 260 ? `${r.line.slice(0, 259)}…` : r.line} (${shownAt(r.where)})`));
    if (refused.length > SHOWN) out.push(`  … and ${refused.length - SHOWN} more (verify --json lists them all)`);
    findings += refused.length;
  }
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
  // Font sizes the app's Contract fixes (LLP 1116.020 D4): findings, as check's are.
  if (fixed.lines.length) {
    out.push(`fixed type: ${plural(fixed.lines.length, 'font size')} the reader's text size does not move`, ...fixed.lines.slice(0, SHOWN));
    if (fixed.lines.length > SHOWN) out.push(`  … and ${fixed.lines.length - SHOWN} more (verify --json lists them all)`);
    findings += fixed.lines.length;
  }
  // What app.json's `verify.accept` names as the app's design: said once each, not counted; an entry that took nothing is named.
  const accepted = [...distinctFindings(runs.map((r) => ({ ...r, lines: (r.accepted ?? []).map((f) => f.line) }))), ...fixed.accepted.map((f) => f.line)];
  const took = (e, f) => f.accept?.finding === e.finding && f.accept?.node === e.node;
  const idle = accept.filter((e) => !runs.some((r) => (r.accepted ?? []).some((f) => took(e, f))) && !fixed.accepted.some((f) => took(e, f)));
  if (accepted.length || idle.length) {
    out.push(`accepted (app.json verify.accept, not counted): ${plural(accepted.length, 'finding')}`, ...accepted.slice(0, SHOWN));
    if (accepted.length > SHOWN) out.push(`  … and ${accepted.length - SHOWN} more (verify --json lists them all)`);
    for (const e of idle) out.push(`  accepts nothing: ${e.finding} ${e.node} (no such finding in this run)`);
  }
  // Each test's end screen reloaded where it stands (LLP 1116.010 D6): a saved value that came back different is a finding, one a
  // route and change, the first test that ended there named; any other change is a note under its own heading, which counts
  // for nothing (LLP 1116.011 D5: a game or a draft in progress lives in memory).
  const ran = reloads.filter((r) => r.ran);
  if (ran.length) {
    const lines = [], notes = [], keys = new Set();
    const once = (key) => { if (keys.has(key)) return false; keys.add(key); return true; };
    for (const r of ran) {
      if (r.error) { if (once(`error ${r.test}`)) lines.push(`  reload   ${quoted(r.test, 36)}: could not reload the end screen: ${firstLine(r.error)}`); }
      else if (r.finding) { if (once(JSON.stringify(['finding', r.finding.route, r.finding.saved]))) lines.push(reloadLine(r.finding, r.test)); }
      else if (r.note && once(JSON.stringify(['note', r.note.route, r.note.lost, r.note.gained]))) notes.push(reloadNote(r.note, r.test));
    }
    out.push(`reload ${plural(ran.length, 'test end')}: ${lines.length ? plural(lines.length, 'finding') : notes.length ? 'no findings' : 'the same screen after each'}`, ...lines.slice(0, SHOWN));
    if (lines.length > SHOWN) out.push(`  … and ${lines.length - SHOWN} more (verify --json lists them all)`);
    findings += lines.length;
    if (notes.length) {
      out.push(`reload note: ${plural(notes.length, 'screen')} ${notes.length === 1 ? 'reloads' : 'reload'} into another state (not counted)`, ...notes.slice(0, SHOWN));
      if (notes.length > SHOWN) out.push(`  … and ${notes.length - SHOWN} more (verify --json lists them all)`);
      out.push(NOTE_ADVICE);
    }
  }
  // The same screens at the largest text size (LLP 1116.019 D2): each finding once, said where unless everywhere; counted
  // apart from check's, so the last line says where they came from (LLP 1116.020 D4).
  let largeCount = 0;
  if (large.length) {
    const read = large.filter((l) => !l.error), shown = distinctFindings(read.map((l) => ({ ...l, scheme: 'light' })));
    const root = [...new Set(read.map((l) => l.rootFontSize).filter(Boolean))].join(', ');
    const size = host === 'ios' ? `${LARGEST}${root ? `, body ${root} pt` : ''}` : `root font size ${root || LARGEST_PX} px at ${WEB_SIZES[0].join('x')}, iOS's body at its largest`;
    out.push(`largest text size (${size}), ${plural(read.length, 'screen')}: ${shown.length ? `${plural(shown.length, 'finding')} (give the text room; a smaller or fixed font is the tell)` : 'nothing spills'}`, ...shown.slice(0, SHOWN));
    largeCount = shown.length;
    if (shown.length > SHOWN) out.push(`  … and ${shown.length - SHOWN} more (verify --json lists them all)`);
    for (const l of large.filter((l) => l.error)) out.push(`  could not read ${screenName(l.screen)}: ${firstLine(l.error)}`);
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
    const more = [ends.length && `test ends: ${ends.map(name).join(' ')}`, again.length && `relaunch: ${again.map(name).join(' ')}`, own.length && `its own: ${own.join(' ')}`].filter(Boolean);
    if (more.length) out.push(`  ${more.join('; ')}`);
  }
  // How a person opens what ran (LLP 1116.008 D2): the dev loop serves the web app, the simulator build launches on iOS.
  if (!failedTests && !broken) out.push(host === 'ios' ? 'open it: bun exact.mjs ios --run (launches what verify checked)' : 'open it: bun exact.mjs web (serves what verify checked)');
  const problems = failedTests + findings + largeCount + broken;
  const why = [failedTests && plural(failedTests, 'failed test'), broken && plural(broken, 'failure', 'failures') + ' to compile or drive', findings && plural(findings, 'finding'), largeCount && `${largeCount} at the largest text size`].filter(Boolean);
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
  ACCEPT = app.manifest.verify?.accept ?? [];
  const say = (lines) => console.log(lines.join('\n'));
  // An app with a server of its own: verify runs it where the app calls it, on an empty database. One already
  // answering there (the builder's dev server) is refused before the build: never stopped, never written to; so is
  // an entry that does not read DB (LLP 1116.008 D1), which could only be run on its own database.
  const server = appServer(app.dir), owned = server && !server.why;
  const refuse = (line) => { if (json) console.log(JSON.stringify({ host, server: { error: line }, problems: 1, exit: 1 })); else console.error(`verify: ${line}`); return 1; };
  if (server?.refusal) return refuse(server.refusal);
  // (A test run that holds the port's turn is waited for when the server starts.)
  if (owned && await answering(server.url) && !(await holderOf(server.port))) return refuse(heldLine(server));

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
        // A test that moved the clock relaunches at the date it reached, and its first screen may say another day than a fresh
        // launch's at the driver's: that comparison is made under the reached date too, unless the server's database is the tests'.
        const pinned = pinnedLaunch(chosen, launchFacts({}));
        const against = running ? (pinned ? null : fresh) : pinned || movedClock(chosen) ? await freshUnder(host, app, relaunchFacts(chosen), host === 'web' ? `verify.${tag}.fresh` : 'verify') : fresh;
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
  // The screenshots a test took itself (its `screenshot` steps) come too, named by the test as the ends are.
  const own = [];
  if (copyTo) {
    mkdirSync(resolve(copyTo), { recursive: true });
    for (const p of kept) copyFileSync(p, resolve(copyTo, basename(p)));
    results.forEach((t, i) => { for (const p of (t.shots ?? []).filter((p) => existsSync(p))) { own.push(`${host}-t${i + 1}-${basename(p)}`); copyFileSync(p, resolve(copyTo, own.at(-1))); } });
  }
  // Refusals the host journaled, once each with where it was seen, and the end screens reloaded.
  const refused = new Map();
  const refusedAt = (where, lines) => { for (const line of lines ?? []) { if (!refused.has(line)) refused.set(line, []); if (!refused.get(line).includes(where)) refused.get(line).push(where); } };
  firsts.forEach((runs) => refusedAt(FIRST, runs.refusals));
  results.forEach((t) => refusedAt(`test ${quoted(t.name, 28)}`, t.end?.refusals));
  refusedAt(RELAUNCH, relaunched?.refusals);
  const refusedList = [...refused].map(([line, where]) => ({ line, where }));
  const reloads = results.filter((t) => t.end?.reloaded).map((t) => ({ test: t.name, ...t.end.reloaded }));
  const large = [...firsts.map((runs) => runs.large), ...results.map((t) => t.end?.large), relaunched?.large].filter(Boolean);
  const fixed = await fixedType(app.dir, host);
  const r = report({ host, built, compiler, server: serverSaid, tested, checks, relaunched, refused: refusedList, reloads, large, fixed, shots: kept, copied: copyTo && resolve(copyTo), own, accept: ACCEPT, ms: Date.now() - started });
  if (json) {
    const testsOut = tested?.results ? { ...tested, results: results.map(({ name, failures, end }) => ({ name, failures, ...(end?.shot && existsSync(end.shot) ? { shot: end.shot } : {}), ...(end?.error ? { endError: end.error } : {}) })) } : tested;
    const relaunchOut = relaunched && (({ runs, large, ...rest }) => rest)(relaunched);
    console.log(JSON.stringify({ host, built: built && { ms: built.ms, said: built.said ?? [], warnings: built.warnings }, compiler, ...(serverSaid ? { server: serverSaid } : {}), refused: refusedList, tests: testsOut, checks, reloads, large, fixed, relaunch: relaunchOut, screenshots: kept, problems: r.problems, exit: r.exit, ms: Date.now() - started }));
  } else say(r.lines);
  return r.exit;
}

if (import.meta.main) main(process.argv.slice(2)).then((code) => process.exit(code), (e) => { console.error(e.message); process.exit(1); });
