#!/usr/bin/env bun
// `bun exact.mjs verify [web|ios|windows|android]` (LLP 1116.004 D7, 1116.005 D7): one command a builder can stop on, where test,
// drives, check and screenshots were three to five turns. It builds when the build is stale (web: the dist's
// record against its sources, as the driver checks it; iOS: agent-build.mjs `buildFirst`, the plan fast path
// included), runs the app's tests (app.test.contract and each other `*.test.contract` beside it, app.mjs
// `authoredTests`; agent-test.mjs `runTests`), launches the app on a fresh scratch store at a
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
// by heuristics and some of either only advise; a warning that is information (`tamber-unreachable`) is said, not
// counted. The web's tests and its two launches run at once; a simulator takes one drive at a time, and runs the
// tests with UIKit's bars (`--chrome platform`) so their end screens
// are what a person sees. An app with a server of its own (the `rest` pattern's) has it run by verify, where the
// app calls it, on an empty scratch database kept through the run and removed after (LLP 1116.006 D3, `appServer`); a server
// entry that does not read DB is refused unless app.json declares server.database: false (LLP 1116.008 D1).
// Each of those screens is read again at the largest text size (iOS: the simulator's; the web: iOS's body as the root font
// size, at the phone's width), and what spills there (past the screen's edge, squeezed to no width, overlapping) is reported
// with a remedy that keeps the text's size, counted in the last line and never failing the run (LLP 1116.019 D2, LLP
// 1116.020 D4, `largestText`); the web also runs the tests again at that size and reads screens they only pass through
// (`largeTour`). On Windows the drives are the headless host's, at app.json's window size; then the app is opened once on
// the console session's desktop and its window captured (`desktopShot`), since that, not the headless picture, is what a
// person sees there.
//
//   bun scripts/verify.mjs [web|ios|windows|android] --app <name> [--sim <udid|name>] [--shots <dir>] [--rescroll] [--json]
import { LAUNCH_MEDIA } from './agent-prefer.mjs';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, copyFileSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, realpathSync, rmdirSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, isAbsolute, relative, resolve } from 'node:path';
import { availableParallelism, loadavg, networkInterfaces } from 'node:os';
import { fileURLToPath } from 'node:url';
import { findings as feelFindings, summary as feelSummary } from '../host/apple/feel/analyze.mjs';
import { compilerWarnings, information, windowsWindow } from './app-cli.mjs';
import { guardFlags } from './help.mjs';
import { holderOf, takePort } from './port-lock.mjs';
import { backendOriginSelection, privateNetwork } from '../js/bake/src/typescript.mjs';
import { contractVerify, semanticsSummary } from './verify-semantics.mjs';

import { declaredSettings, settingValues } from '../js/bake/src/__exact_env.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
/** The web's two widths: a phone's, and a desktop window's. */
export const WEB_SIZES = [[390, 844], [1280, 900]];
const SHOWN = 6, WARNINGS = 4; // the findings and build warnings shown; the count says the rest
const REAL_MS = 4000, REAL_STEP = 250; // the relaunch's bound on real time for its first loads, in steps
const FIRST = 'first screen', RELAUNCH = 'relaunch';
// How long a run takes, said when it starts so a builder waits once rather than polling (LLP 1116.021 D3): the app's
// last run on that host, else these (a run that must build first takes that build's time too).
const ESTIMATE = { web: 30_000, ios: 60_000, windows: 60_000, android: 120_000 };
const lastRuns = (app) => { try { return JSON.parse(readFileSync(resolve(app.dir, '.exact', 'verify', 'last.json'), 'utf8')); } catch { return {}; } };
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

/** Whether `origin` is served from this machine: a loopback address, or a private-network one that names
 *  one of this machine's interfaces (a phone's development build reaches the checkout's fixture there). */
export function onThisMachine(origin) {
  return /^https?:\/\/(?:127\.0\.0\.1|localhost|\[::1\])(?::\d+)?$/.test(origin)
    || privateNetwork(origin) && Object.values(networkInterfaces()).flat().some(n => n?.address === new URL(origin).hostname.replace(/^\[|\]$/g, ''));
}

/** The server URL the data module calls: a setting app.ts reads (`import.meta.env.API_BASE_URL`, LLP 1153 D4) whose
 * value, as its sources give it now, is a local one; else the first local one (`http://127.0.0.1:8787`, or localhost) in
 * app.ts, else in a module of the app's own that it imports (`import { API } from './api'`); null when it calls none. */
export function serverUrl(dir, env = process.env) {
  const at = (text) => text?.match(LOCAL)?.[0].replace(/\/$/, '') ?? null;
  const code = readIn(dir, 'app.ts');
  const read = declaredSettings(dir).filter((name) => /^[A-Z][A-Z0-9_]*$/.test(name) && new RegExp(`\\bimport\\.meta\\.env\\.${name}\\b`).test(code ?? ''));
  const setting = Object.values(settingValues(dir, read, env)).map((s) => s && at(s.value)).find(Boolean);
  return setting ?? at(code) ?? importsOf(dir, 'app.ts').map((p) => at(readIn(dir, p))).find(Boolean) ?? null;
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
 * its database's path read from `DB` unless `server.database` is false. Arguments after the named entry are kept in
 * `args`. null without a server; else `why` says what keeps it
 * from being run when the app calls no local URL (null when nothing does), `refusal` (LLP 1116.008 D1) the one line for an
 * entry that needs a database but does not read `DB`, which `test` and `verify` stop on. */
export function appServer(dir, env = process.env) {
  let named = null, serverArgs = [], manifest = {};
  try { manifest = JSON.parse(readIn(dir, 'app.json') ?? '{}') ?? {}; } catch {}
  const argv = manifest.commands?.server, database = manifest.server?.database !== false;
  if (Array.isArray(argv) && /(^|\/)bun$/.test(argv[0])) {
    const index = argv.findIndex((a, i) => i > 0 && !a.startsWith('-'));
    if (index >= 0) { named = argv[index]; serverArgs = argv.slice(index + 1); }
  }
  const entry = [named, 'server.ts', 'server/index.ts', 'server/server.ts'].find((p) => p && readIn(dir, p) != null);
  if (!entry) return null;
  const fixture = /(^|[/\\])fixtures?[/\\]/.test(entry);
  const selected = fixture ? backendOriginSelection(dir, env) : null;
  const local = selected && onThisMachine(selected.origin);
  const url = selected ? local ? selected.origin : null : serverUrl(dir, env);
  const why = !url ? `${entry} is left alone: app.ts calls no local URL (http://127.0.0.1:<port>), so where the app reaches it is unknown` : null;
  const refusal = url && database && !readsDB(dir, entry)
    ? `${entry}${entry === named ? ' (app.json\'s commands.server)' : ''} does not read its database's path from DB, so it cannot be run on an empty one: write new Database(process.env.DB ?? 'data.db') in it or a module it imports, or name the entry that does in app.json: "commands": { "server": ["bun", "server/index.ts"] }`
    : null;
  const port = url ? Number(new URL(url).port || (new URL(url).protocol === 'https:' ? 443 : 80)) : null;
  if (selected && local) { const at = serverArgs.indexOf('--port'); if (at >= 0) serverArgs[at + 1] = String(port); }
  return { entry, args: entry === named ? serverArgs : [], database, url, port, why, refusal, ...(fixture ? { fixture: true } : {}) };
}

function serverEnv(server, db) {
  const env = { ...process.env, PORT: String(server.port) };
  if (server.database) env.DB = db;
  else delete env.DB;
  return env;
}

const serverRun = (server, name) => `bun ${[server.entry, ...server.args].join(' ')} at ${server.url}, ${server.database ? `on an empty database (.exact/${name}/db.sqlite, removed after)` : 'without a database'}`;

/** Whether anything answers HTTP at `url` (any status). */
export const answering = (url) => fetch(url, { signal: AbortSignal.timeout(1000) }).then(() => true, () => false);

/** The one line a server's URL already answering gets: who holds the port, when the system says, and what to do. A
 * server this did not start is never stopped, nor written to. */
export function heldLine(server) {
  const pid = spawnSync('lsof', ['-nP', `-iTCP:${server.port}`, '-sTCP:LISTEN', '-Fp'], { encoding: 'utf8' }).stdout?.match(/^p(\d+)/m)?.[1];
  const command = pid ? commandLine(pid).slice(0, 60) : '';
  return `${server.url} already answers${pid ? ` (pid ${pid}${command ? `: ${command}` : ''})` : ''}, and the tests would ${server.database ? "write into that server's database" : 'change that server\'s state'}: stop it (${pid ? `kill ${pid}` : 'Ctrl-C where it runs'}) and run this again, which starts ${server.entry} itself${server.database ? ' on an empty one' : ''}`;
}

/** Keep the URL appServer accepted, including its path and explicit default port, for exact record comparisons. */
export function recordedServerAddress(value) {
  if (typeof value !== 'string') return null;
  let url; try { url = new URL(value); } catch { return null; }
  const authority = /^[a-z]+:\/\/([^/]*)/i.exec(value)?.[1];
  if (!['http:', 'https:'].includes(url.protocol) || !url.hostname || !authority || authority.includes('@') || value.includes('?') || value.includes('#')) return null;
  return { url: value, port: Number(url.port || (url.protocol === 'https:' ? 443 : 80)) };
}

/** A managed server needs this directory's launch record and the recorded process still listening. */
function ownsServer(dir, server, expectedPid, child = null) {
  const listeners = process.platform === 'win32'
    ? spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `(Get-NetTCPConnection -State Listen -LocalPort ${server.port} -ErrorAction SilentlyContinue).OwningProcess`], { encoding: 'utf8' }).stdout?.match(/\d+/g) ?? []
    : [...(spawnSync('lsof', ['-nP', `-iTCP:${server.port}`, '-sTCP:LISTEN', '-Fp'], { encoding: 'utf8' }).stdout?.matchAll(/^p(\d+)/gm) ?? [])].map(m => m[1]);
  const root = realpathSync(dir), lease = resolve(dir, '.exact/agent-server');
  for (const pid of listeners) {
    if (expectedPid !== undefined && Number(pid) !== expectedPid) continue;
    let record; try { record = JSON.parse(readFileSync(resolve(lease, `server-${pid}`), 'utf8')); } catch { continue; }
    if (!record || record.directory !== root || record.origin !== server.url || !alive(Number(pid))) continue;
    const command = commandLine(pid);
    if (!command.includes(server.entry)) continue;
    // CIM exposes no cwd. A copied record cannot vouch for one; only this invocation's child can.
    if (process.platform === 'win32') {
      if (child?.pid === Number(pid) && child.exitCode == null) return true;
      continue;
    }
    const cwd = spawnSync('lsof', ['-a', '-p', pid, '-d', 'cwd', '-Fn'], { encoding: 'utf8' }).stdout?.match(/^n(.+)$/m)?.[1];
    try { if (cwd && realpathSync(cwd) === root) return true; } catch { /* The process or its directory disappeared. */ }
  }
  return false;
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

/** Runs the app's server (`appServer`) on an empty scratch database, `.exact/<name>/db.sqlite` (none with database: false), and answers once it
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
  const runDir = resolve(dir, '.exact', name), db = server.database ? resolve(runDir, 'db.sqlite') : null, owner = resolve(runDir, 'owner');
  const clear = () => { if (db) for (const end of ['', '-wal', '-shm', '-journal']) rmSync(`${db}${end}`, { force: true }); rmSync(owner, { force: true }); };
  mkdirSync(runDir, { recursive: true });
  clear();
  writeFileSync(owner, `${process.pid}\n`);
  const child = Bun.spawn([process.execPath, server.entry, ...server.args], { cwd: dir, env: serverEnv(server, db), stdin: 'ignore', stdout: 'ignore', stderr: 'pipe' });
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
    try { rmdirSync(runDir); } catch {} // when nothing else is in it (verify's keeps its screenshots)
  };
  const why = await untilAnswers(server, child, tail);
  if (why) { await stop(); throw new Error(why); }
  return { stop, exited: () => child.exitCode, tail };
}

const alive = (pid) => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
// A process's command line: `ps` has no Windows build, where CIM's Win32_Process reports it. Empty when neither answers.
const commandLine = (pid) => (process.platform === 'win32'
  ? spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `(Get-CimInstance Win32_Process -Filter 'ProcessId=${Number(pid)}').CommandLine`], { encoding: 'utf8' })
  : spawnSync('ps', ['-o', 'command=', '-p', String(pid)], { encoding: 'utf8' })).stdout?.trim() ?? '';

/** The app's server for one `agent` drive (LLP 1116.008 D1): null when the app has none, or the URL it calls already answers
 * (the builder's own, or another drive's: used, said nothing of); else started where app.ts calls it, on a database of the
 * drive's own, `.exact/agent/<storage>/db.sqlite` kept for the `--storage` name so a second drive on it sees the first's data
 * (without one, an empty database removed after; `fresh`, a drive that starts its store empty, empties it first, and is
 * refused while another drive or the builder's server is using the app server), and one line says so. `{ stop() }` ends it. Drives that overlap share the
 * server: each is a `drive-<pid>` file in `.exact/agent-server/`, the servers a drive started are `server-<pid>` (their
 * database as the file's text), and the last drive out stops them; one that did not start the server stops nothing, and the
 * builder's own is never in the list. With database: false, no database is passed, created or cleared; otherwise an entry
 * that does not read `DB` is refused (`appServer`'s `refusal`), as `test` refuses it. */
export async function driveServer(dir, storage, say = (line) => console.error(line), { fresh = false } = {}) {
  const server = appServer(dir);
  if (!server?.url) return null;
  if (server.refusal) throw new Error(`agent: ${server.refusal}`);
  const lease = resolve(dir, '.exact/agent-server'), mine = resolve(lease, `drive-${process.pid}`), starting = resolve(lease, 'starting');
  const db = !server.database ? null : storage ? resolve(dir, '.exact/agent', storage.replace(/[^\w.-]+/g, '-'), 'db.sqlite') : resolve(lease, 'db.sqlite');
  const files = (kind) => { try { return readdirSync(lease).filter((f) => f.startsWith(`${kind}-`)).map((f) => [resolve(lease, f), Number(f.slice(kind.length + 1))]); } catch { return []; } };
  let spawned = null;
  mkdirSync(lease, { recursive: true });
  writeFileSync(mine, '');
  // A refused drive may have found a copied record. Ending it must not lend that record authority to stop a process.
  const leave = () => {
    rmSync(mine, { force: true });
    for (const [file, pid] of files('drive')) if (!alive(pid)) rmSync(file, { force: true });
    if (files('drive').length) return null;
    return files('server').flatMap(([file, pid]) => {
      // Overlapping drives may choose different ports; each record names the listener to validate.
      let recordedServer = null;
      try {
        const address = recordedServerAddress(JSON.parse(readIn(lease, file))?.origin);
        if (address) recordedServer = { ...server, ...address };
      } catch { /* An unknown record grants no authority to stop a process. */ }
      const ours = spawned?.pid === pid && spawned.exitCode == null || recordedServer && ownsServer(dir, recordedServer, pid);
      if (!ours && alive(pid)) return [];
      rmSync(file, { force: true });
      return ours ? [pid] : [];
    });
  };
  const clean = () => {
    if (files('server').length) return;
    if (db) rmSync(lease, { recursive: true, force: true });
    else { rmSync(resolve(lease, 'log'), { force: true }); try { rmdirSync(lease); } catch {} }
  };
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
    // A store asked empty cannot be emptied under a server another drive (or the builder) is using: refused, and this
    // drive's lease given up, never the database deleted or kept as it was.
    const others = files('drive').filter(([file, pid]) => file !== mine && alive(pid)).map(([, pid]) => pid);
    if (fresh && (others.length || await answering(server.url))) {
      await stop();
      throw new Error(`agent: --storage ${storage} is to start empty, but ${others.length ? `another drive (pid ${others.join(', ')}) is using` : 'something already answers at'} ${server.url}, the app server${db ? ', on its database' : ''}: let it end, or name another store`);
    }
    if (await answering(server.url)) {
      if (server.fixture && !ownsServer(dir, server)) {
        await stop();
        throw new Error(`agent: ${server.url} answers, but its fixture does not belong to ${realpathSync(dir)}; use --backend http://127.0.0.1:<free-port> or PORT=<free-port> for this checkout`);
      }
      const other = db && files('server').map(([file]) => { try { return JSON.parse(readIn(lease, file)).db; } catch { return null; } }).find((was) => was && was !== db);
      if (other) say(`agent: ${server.entry} at ${server.url} is another drive's, on its database (${relative(dir, other)}); this drive uses it`);
      return { stop };
    }
    if (db) mkdirSync(dirname(db), { recursive: true });
    if (db && (!storage || fresh)) for (const end of ['', '-wal', '-shm', '-journal']) rmSync(`${db}${end}`, { force: true });
    const log = openSync(resolve(lease, 'log'), 'a');
    const child = Bun.spawn([process.execPath, server.entry, ...server.args], { cwd: dir, env: serverEnv(server, db), stdin: 'ignore', stdout: 'ignore', stderr: log });
    spawned = child;
    closeSync(log);
    writeFileSync(resolve(lease, `server-${child.pid}`), JSON.stringify({ directory: realpathSync(dir), origin: server.url, db }));
    const why = await untilAnswers(server, child, () => lastError(readIn(lease, 'log') ?? ''));
    if (why) { await stop(); throw new Error(`agent: ${why}`); }
    if (server.fixture && !ownsServer(dir, server, child.pid, child)) {
      await stop();
      throw new Error(`agent: ${server.url} answered before this checkout's fixture owned its port; choose another port with --backend or PORT`);
    }
    say(`agent: started ${server.entry} at ${server.url} for this drive (${!db ? 'no database' : storage ? `database ${relative(dir, db)}, kept for --storage ${storage}` : 'an empty database'}); stopped after`);
    return { stop };
  } finally { if (locked) rmSync(starting, { force: true }); }
}

/** This process's ancestors' pids, nearest first. */
function ancestors() {
  const out = [];
  for (let pid = process.ppid; pid > 1 && out.length < 32;) {
    out.push(String(pid));
    const parent = Number(spawnSync('ps', ['-o', 'ppid=', '-p', String(pid)], { encoding: 'utf8' }).stdout?.trim());
    if (!Number.isInteger(parent) || parent === pid) break;
    pid = parent;
  }
  return out;
}

/** `fn` with the app's server running on an empty database, as verify runs it: its exit code, or 1 when the server cannot
 * run, said in one line. `bun exact.mjs test` runs its tests so (LLP 1116.007 D2: `optional`: an app with no server, or one
 * whose URL app.ts does not say, runs them as they are, the second said once, and the tests use whatever answers there);
 * a server entry that needs a database but does not read `DB` is refused whatever the verb (LLP 1116.008 D1);
 * a server an ancestor process runs (an older app's api-test test.mjs, which runs `test` under `withAppServer`, and
 * the `agent.mjs --test` that `test` runs a level further down) is used, not started again.
 * `api-test` is the same call without `optional`: it wants a server, and says why it has none. */
export async function withAppServer(dir, fn, { verb = 'api-test', optional = false } = {}) {
  const server = appServer(dir);
  const owners = server ? ['api-test', 'test', 'verify'].map((name) => readIn(dir, `.exact/${name}/owner`)?.trim()).filter(Boolean) : [];
  if (owners.length && ancestors().some((pid) => owners.includes(pid))) return fn(null);
  let running;
  try {
    if (server?.refusal) throw new Error(server.refusal);
    if (!server || server.why) {
      if (!optional) throw new Error(server?.why ?? 'no server: server.ts, server/index.ts, server/server.ts, or app.json\'s commands.server');
      if (server) console.error(`${verb}: ${server.why}; the tests use whatever answers there`);
    } else {
      running = await startServer(dir, server, verb, { forward: true });
      console.error(`${verb}: ${serverRun(server, verb)}`);
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
async function iosBuild(app) {
  const { buildFirst } = await import('./agent-build.mjs');
  const said = [], started = Date.now();
  // A native build (or the wait for one) is said as it starts too: it is minutes, and the report comes after it.
  const code = await buildFirst('ios', app, { say: (line) => { said.push(line); if (/^(building|waiting) for/.test(line)) console.error(line); } });
  return said.length || code ? { ms: Date.now() - started, code, said, warnings: [] } : null;
}

/** The Windows build a drive needs (agent-build.mjs `windowsBuildFirst`): null when fresh, else what it said. */
async function windowsBuild(app) {
  const { windowsBuildFirst } = await import('./agent-build.mjs'), said = [], started = Date.now();
  const code = await windowsBuildFirst(app, { say: (line) => { said.push(line); if (/^building for/.test(line)) console.error(line); } });
  return said.length || code ? { ms: Date.now() - started, code, said, warnings: [] } : null;
}
/** A drive's own options on a host: iOS shows UIKit's bars; Windows opens at app.json's window size, as a person sees it. */
const hostOpen = (host, app) => host === 'ios' ? { chrome: 'platform' } : host === 'windows' ? { size: windowsWindow(app.manifest) } : {};

/** The app as it opens on the desktop of the console session, on the store of the person signed in there
 * (host/windows/build.mjs `openOnDesktop`): the picture a person sees, and a grader beside a WinUI app, with Segoe UI,
 * the system's title bar and Mica, which a headless drive does not draw. `{ shot, size, foreground }` or `{ error }`. */
async function desktopShot(app, shots) {
  const [{ openOnDesktop }, { windowsBuild }] = await Promise.all([import('../host/windows/build.mjs'), import('./agent-build.mjs')]);
  const shot = resolve(shots, 'windows-window.png'), r = await openOnDesktop(windowsBuild(app).bin, { shot }).catch((e) => ({ error: e.message }));
  return r.error ? { error: r.error } : { shot, size: r.size, foreground: r.foreground };
}

/** The compiler's warnings for the app's sources, every one (`contract build --json`: its text form says the
 * SF Symbols only the web draws blank in one line, and this report counts each), each in the line `contract build`
 * prints it as; whatever this run built (a fresh build said them when it was made): this checkout's compiler as
 * built, or through Cargo when its sources are newer. */
async function compilerWarningsOf(app) {
  const [{ fresh }, { cargoEnvironment }] = await Promise.all([import('../host/web-js/module.mjs'), import('./app.mjs')]);
  const env = cargoEnvironment(process.env), binary = resolve(ROOT, 'target/debug', process.platform === 'win32' ? 'contract.exe' : 'contract');
  delete env.RUSTUP_TOOLCHAIN; delete env.CARGO_TARGET_DIR; // as `exact contract` runs it: exact2's toolchain and target
  const [command, ...pre] = fresh(binary, resolve(ROOT, 'target/debug/contract.d')) ? [binary] : ['cargo', 'run', '-q', '--manifest-path', resolve(ROOT, 'Cargo.toml'), '-p', 'contract', '--'];
  const child = spawn(command, [...pre, 'build', '--json', resolve(app.dir, 'app.contract')], { cwd: ROOT, env, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  for (const stream of [child.stdout, child.stderr]) stream.on('data', (d) => { said += d; });
  await new Promise((done) => { child.on('close', done); child.on('error', done); });
  return warningsOfBuild(said);
}

/** The warnings in what `contract build --json` wrote: its one JSON array's entries of severity `warning`, each as
 * the line the text build prints (`file:line:col warning [id] message`, the file relative to `cwd` when under it,
 * as `compilerWarnings` gives it). Output that holds no array (Cargo failing to build the compiler) gives the
 * warning lines found in it, so a text build's are still read. */
export function warningsOfBuild(text, cwd = process.cwd()) {
  const array = String(text ?? '').split('\n').find((line) => line.startsWith('['));
  let entries;
  try { entries = JSON.parse(array); } catch { entries = null; }
  if (!Array.isArray(entries)) return compilerWarnings(text, cwd);
  const lines = new Set();
  for (const e of entries) {
    if (e?.severity !== 'warning' || typeof e.message !== 'string') continue;
    const file = typeof e.file === 'string' ? e.file : '';
    const near = file && relative(cwd, file);
    const at = file ? `${isAbsolute(file) && !near.startsWith('..') ? near : file}:` : '';
    lines.add(`${at}${e.line}:${e.col} warning [${e.id}] ${e.message}`);
  }
  return [...lines];
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
export function refusalsOf({ lines = [], host = [] } = {}, expected = []) {
  const found = [], journal = new Map(), accepted = new Map();
  for (const line of expected) accepted.set(line,(accepted.get(line) ?? 0)+1);
  const normalized = raw => String(raw).replace(/^t=\d+(?:\.\d+)?\s+/, '').replace(/^(?:app|console\.\w+|chrome|warning|error): /, '').replace(/^exact: /, '').trim();
  const collect = line => {
    if (/\brefus(?:ed|es|al)\b/i.test(line) && !/\b(?:dismissal|back|back gesture|drag) refused\b|connection[ _]refused|\(driver fault\)/i.test(line) && !found.includes(line)) found.push(line);
  };
  for (const raw of lines) {
    const line=normalized(raw), count=accepted.get(String(raw)) ?? 0;
    journal.set(line,(journal.get(line) ?? 0)+1);
    if (count) accepted.set(String(raw),count-1); else collect(line);
  }
  // A native host can also print its journal; that mirror is one event.
  for (const raw of host) {
    const line=normalized(raw), mirrors=journal.get(line) ?? 0;
    if (mirrors) journal.set(line,mirrors-1); else collect(line);
  }
  return found;
}

/** A session's refusals so far (`refusalsOf` its `logs`). */
const refusalsIn = async (s, expected = []) => refusalsOf(await s.logs().catch(() => ({})), expected);

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
  // The Windows host has no `check` observation yet: the screen's capture alone, `unchecked`, its failed resources still said.
  if (s.host === 'windows') {
    const vp = (await s.layout().catch(() => null))?.viewport ?? {}, kept = shot?.(vp, 'light');
    if (kept) await s.screenshot(kept);
    return [{ screen, size: [vp.w, vp.h], scheme: 'light', unchecked: true, ...(kept ? { shot: kept } : {}), findings: [], accepted: [], lines: failed, hidden: 0, advice: [] }];
  }
  const runs = [];
  if (s.host === 'android') return (await import('./agent-android.mjs')).androidCaptures(s, { screen, shot, failed });
  for (const size of sizes) {
    if (size) { await s.resize(...size); await s.clock('settle'); }
    const r = await checkOp(s, ['both'], { shots: shot, accept: ACCEPT });
    runs.push(...groups(r.text).map((g, i) => {
      const run = r.check[i] ?? {}, vp = run.viewport ?? { w: size?.[0], h: size?.[1] }, kept = shot?.(vp, run.asked ?? run.scheme);
      // Every finding's line, which the check's text caps (`hidden`).
      const all = run.findings?.length && run.findings.every((f) => f.line) ? run.findings.map((f) => f.line) : null;
      return { screen, size: [vp.w, vp.h], scheme: run.scheme, asked: run.asked, ...(kept ? { shot: kept } : {}), findings: run.findings ?? [], accepted: run.accepted ?? [], lines: [...failed, ...(all ?? g.lines)], hidden: all ? 0 : g.hidden, advice: g.advice };
    }));
  }
  return runs;
}


/** What verify android does not do yet (agent-android.mjs `androidCaptures`), said in its report. */
const ANDROID = 'not on Android yet: check\'s findings (each screen is captured unjudged, light and dark: look at them), the largest text size, the scrolling probe (iOS\'s), a test end\'s reload (the web\'s)';

// LLP 1116.019 D2: each screen verify visits is read again at the largest text size, and what spills is reported, counted
// and never failing the run (LLP 1116.020 D4: uncounted, half of the builders who saw them stopped at "verify: clean"): the
// layout is the builder's (LLP 1115.000 D12.1), and a failure would invite `overflow: hidden`. iOS sets the
// simulator's own size, which UIKit's controls and Exact's text both follow; the web, where a builder iterates, sets the root
// font size to iOS's body at that size on the phone's width. Only the kinds of finding a larger text makes are kept, each with
// a remedy that keeps the text's size: a smaller font is the platform tell this pass exists to prevent.
export const LARGEST = 'accessibility-extra-extra-extra-large', LARGEST_PX = 53;
const LARGE_KINDS = new Set(['overflow', 'clipped', 'overlap', 'squeezed']), PAGES = 6;
const WRAP = 'wrap the row: flex-wrap="wrap" with a flex-basis in rem on its items (docs/recipes/responsive-columns.md)';
/** The remedy for a finding at the largest size: never a smaller or fixed font. A text in a list's row (`inRow`) is cut by
 * the row, which wrapping the row does not change: at the accessibility sizes UIKit's cells wrap a label and grow taller,
 * never truncating it, so a `line-clamp` there reads as a tell (round 22's book-club and contacts took one from this
 * remedy). A grouped list's standard cell already hyphenates as its label does. Elsewhere a UILabel wraps at words and breaks
 * between syllables only a word longer than its line (iOS 27), so `hyphens="auto"` is the remedy for a text of one word;
 * on a heading or sentence it hyphenates words a UILabel keeps whole, which round 26's graders named a tell. */
export function largeRemedy(f, { inRow = false } = {}) {
  const wide = (f.detail?.need ?? f.detail?.cut ?? [0])[0] > 0;
  if (f.kind === 'clipped' && inRow && !f.detail?.edge) {
    return `${wide ? 'give the text more of the row (no px width beside it) and ' : ''}let the row grow taller with its text, as a cell does at this size (no px height on the row or what it holds; no \`line-clamp\`, which cuts what a cell shows whole)`;
  }
  const oneWord = typeof f.text === 'string' && f.text.trim() !== '' && !/\s/.test(f.text.trim());
  if (f.kind === 'clipped') return f.detail?.edge ? 'let the page scroll there (a scroll with flex=1 min-height=0)'
    : wide && f.detail?.need ? `give it room (no px width on its box), or ${WRAP}; ${oneWord ? 'a word longer than any line breaks between syllables with `hyphens="auto"`, as a UILabel breaks it' : 'not `hyphens="auto"`, which breaks words a UILabel wraps whole'}`
    : f.detail?.need ? 'let its box grow with its text: no px height on it (a rem size, or none)' : `give the box that cuts it room (no px width or height on it), or ${WRAP}`;
  if (f.kind === 'squeezed') return `let it keep its words (no min-width: 0 or px width beside it), or ${WRAP}`;
  // Overlapping cells of a row that stays one line (a week's days) wrap nowhere: UIKit's calendar caps its day numbers, which
  // grow to 31 points and no further (iOS 27), never shrinking them.
  if (f.kind === 'overlap') return `${WRAP}; for a row of equal cells that must stay one line (a week's days), cap the size its text grows to, as UIKit's calendar does: font-size="min(-exact-subheadline, 31px)"`;
  return WRAP;
}
/** LLP 1116.020 D4: each font size the app's own Contract fixes (no-tells.mjs `fixedSizesIn`), which no spill shows (a fixed
 * size never grows to spill), and LLP 1116.021 D6's two like it: a symbol sized in points beside text that grows
 * (`fixedSymbolsIn`), and text that breaks inside words (`brokenWordsIn`), the remedy builders reached for when a large
 * text spilled. `{ lines, accepted, fonts }`, a line each, those app.json's `verify.accept` takes (`fixed-type`,
 * `fixed-symbol` or `broken-words`, the node's testId) apart, and how many lines are font sizes. Exact's parts that are
 * fixed on purpose are the library's, not here. */
export async function fixedType(dir, host, accept = ACCEPT) {
  const { contractFiles, fixedSizesIn, fixedSymbolsIn, brokenWordsIn } = await import('./no-tells.mjs'), unit = host === 'ios' ? 'pt' : 'px', lines = [], accepted = [];
  let fonts = 0;
  const add = (finding, testId, said) => {
    const takes = testId != null && accept.find((e) => e.finding === finding && (/^[^*]+\*$/.test(e.node) ? testId.startsWith(e.node.slice(0, -1)) : e.node === testId));
    if (takes) accepted.push({ line: said, accept: takes }); else { lines.push(said); if (finding === 'fixed-type') fonts++; }
  };
  for (const file of contractFiles(dir).filter((f) => !f.endsWith('.test.contract'))) {
    const source = readFileSync(file, 'utf8'), at = (f) => `${relative(dir, file)}:${f.line}`;
    for (const f of fixedSizesIn(source)) {
      const rem = Math.round(f.size / 16 * 4) / 4;
      add('fixed-type', f.testId, `  fixed    ${at(f)} ${f.tag} font-size=${f.value}: stays ${f.size} ${unit} as Dynamic Type grows; a role (\`font="-exact-title1"\`) or a \`rem\` size ("${rem}rem") scales`);
    }
    for (const f of fixedSymbolsIn(source)) {
      const rem = Math.round(f.size / 16 * 4) / 4;
      add('fixed-symbol', f.testId, `  fixed    ${at(f)} image ${f.property}=${f.value}: a symbol sized in points stays small as Dynamic Type grows: leave it unsized (it follows its text), or size it with a text style (\`font="-exact-title1"\`) or a font-size in rem ("${rem}rem")`);
    }
    for (const f of brokenWordsIn(source)) add('broken-words', f.testId, `  breaks   ${at(f)} ${f.tag} overflow-wrap=${f.value}: breaks inside words (Octo/ber); wrap between words (the default) or give the row room`);
  }
  return { lines, accepted, fonts };
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
    const parents = new Map((tree.nodes ?? []).flatMap((n) => (n.children ?? []).map((c) => [c, n.id])));
    const inRow = (id) => { for (let at = parents.get(id); at != null; at = parents.get(at)) if (nodes.get(at)?.type === 'List') return true; return false; };
    return { screen, size: [obs.viewport.w, obs.viewport.h], rootFontSize: web ? LARGEST_PX : await body(), lines: [...found.values()].map((f) => line({ ...f, hint: largeRemedy(f, { inRow: inRow(f.id) || (f.of != null && inRow(f.of)) }) }, nodes)), ms: Date.now() - started };
  } catch (e) { return { screen, error: e.message }; }
  finally { restore(); forget(); }
}

/** An iOS test end whose window the test sized (`size 1100x800`) is that window scaled to fit the phone (ExactViewIOS.swift):
 * its text is drawn smaller than its points, so neither the pixels behind a label nor a control's size is what a person
 * sees there. Its contrast and target findings are left out of `runs`, and one advice line says so. */
export function scaledEnd(runs, size) {
  const measured = (k) => k === 'contrast' || k === 'target';
  for (const r of runs) {
    if (!size || Math.abs(r.size[0] - size[0]) < 0.5) continue;
    const lines = r.lines.filter((l) => !measured(l.trim().split(/\s/)[0]));
    if (lines.length === r.lines.length) continue;
    r.lines = lines;
    r.findings = r.findings.filter((f) => !measured(f.kind));
    r.advice = [...(r.advice ?? []), `  advice   the test's ${size.join('x')} window is shown scaled to fit the phone (${r.size.map(Math.round).join('x')}): contrast and target sizes are not measured there (verify web measures them at that size)`];
  }
  return runs;
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
  if (host === 'ios') scaledEnd(runs, t.size);
  const refusals = await refusalsIn(s, t.expectedRefusals);
  const reloaded = await reloadEnd(s, t, persisted);
  const large = await largestText(s, t.name);
  // A web test runs on a page origin of its own (agent-test.mjs `launch`), and what it stored is that origin's: the relaunch opens it.
  const port = host === 'web' ? s.carrier?.port : undefined;
  return { keep: true, runs, shot: path, resources: facts.resources, ops: facts.ops, reload: t.reload, launch: t.launch, reached: t.reached, refusals, reloaded, large, ...(port ? { port } : {}) };
};

/** The authored tests (app.mjs `authoredTests`), a file at a time: `{ passed, failed, results }` (each with its file,
 * its end screen's `end`, and the store it left), with `error` naming each file that does not compile; `{ none }` without
 * a file. A test is numbered by its place in `results`, so its captures' names are its own across the files, and each
 * file after the first runs under a storage name of its own, so a store one file's test left is not another's. */
async function tests(host, app, shots) {
  const [{ runTests }, { authoredTests }] = await Promise.all([import('./agent-test.mjs'), import('./app.mjs')]);
  const persisted = persistedStates(app.dir), files = authoredTests(app.dir);
  if (!files.length) return { none: true };
  const atEnd = testEnd(host, shots, persisted), out = { passed: 0, failed: 0, results: [] }, errors = [];
  for (const [i, file] of files.entries()) {
    const first = out.results.length;
    try {
      const r = await runTests({ host, file, app: app.name, atEnd: (s, t) => atEnd(s, { ...t, index: first + t.index }), ...(i ? { storage: `test${i}` } : {}), ...hostOpen(host, app) });
      out.passed += r.passed; out.failed += r.failed;
      // A failure the test did not arrange is a finding (LLP 1146 D4): `runTests` says it in a note.
      out.results.push(...r.results.map(({ name, failures, notes = [], end, store, shots }) => ({ name, file: basename(file), failures, unarranged: notes.filter((n) => n.startsWith('unarranged: ')).map((n) => n.slice(12)), ...(shots ? { shots } : {}), ...(end ? { end } : {}), ...(store ? { store } : {}) })));
    } catch (e) { errors.push(files.length > 1 ? `${basename(file)}: ${e.message}` : e.message); }
  }
  return errors.length ? { ...out, error: errors.join('\n') } : out;
}

/** The web's tour of the screens the tests pass through, at the largest text size: the tests run again on the phone's
 * width, and after an input that shows a testId no screen of the tour has shown yet (its digits aside: a row's id that
 * holds a date or a count is the same row), the screen is read as `largestText` reads one. A screen only passed through
 * (a flashcard's Again and Got it, shown once the card turns) is no test's end, and its overflow went unreported
 * (flashcards, round 22). What the tests assert is not judged here. At most LARGE_TOUR_SCREENS screens, so a run
 * reports the same ones each time, and no more than LARGE_TOUR_MS. The `largestText` results, each screen named by the
 * line of the test it followed. On round 22's flashcards, notes and book-club (12 to 26 s without it) it added 1 to 11 s,
 * 6 on average; most on an app with its own server, whose tour runs after the rest. */
const LARGE_TOUR_SCREENS = 12, LARGE_TOUR_MS = 40_000;
const kindOf = (id) => String(id).replace(/\d+/g, '#');
async function largeTour(app) {
  const [{ runTests }, { authoredTests }] = await Promise.all([import('./agent-test.mjs'), import('./app.mjs')]);
  const shown = new Set(), read = [], started = Date.now();
  const spent = () => read.length >= LARGE_TOUR_SCREENS || Date.now() - started > LARGE_TOUR_MS;
  const atStep = async (s, { name, line }) => {
    if (spent()) return 'done';
    const tree = await s.op({ op: 'tree' }).catch(() => null);
    const ids = (tree?.nodes ?? []).filter((n) => !n.inactive && n.props?.testId != null).map((n) => kindOf(n.props.testId));
    if (ids.every((id) => shown.has(id))) return;
    for (const id of ids) shown.add(id);
    const r = await largestText(s, `line ${line} of ${quoted(name, 24)}`);
    if (r && !r.error) read.push(r);
  };
  for (const file of authoredTests(app.dir)) {
    if (spent()) break;
    try { await runTests({ host: 'web', file, app: app.name, size: WEB_SIZES[0], storage: 'verify-tour', atStep }); }
    catch { /* the tests' own run says what does not compile */ }
  }
  return read;
}

/** The tour's screens with only what the screens verify visits did not say: a line is the same with its box and digits
 * aside (another run's row ids hold another date). */
export function tourNews(visited, toured) {
  const key = (line) => line.replace(BOX, '').replace(/\d+/g, '#');
  const said = new Set(visited.flatMap((l) => l.lines ?? []).map(key));
  return toured.map((l) => ({ ...l, lines: l.lines.filter((line) => !said.has(key(line))) }));
}

/** One launch at `size` (null: the device's own) on a fresh scratch store: the app's data landed and its
 * transitions ended, then `check both`, its captures kept in `shots`. A run per scheme, `{ screen, size, scheme,
 * asked, shot, findings, lines, hidden }` (`lines`: check's own text for it, with a line for a resource that
 * failed), and at the phone's width what it shows (`seen`), for the relaunch. */
async function checkAt(host, app, size, store, shots) {
  const [{ open }, { driveStore, removeBrowserProfile }] = await Promise.all([import('./agent.mjs'), import('./agent-launch.mjs')]);
  const fresh = { EXACT_AGENT_STORAGE_FRESH: '1' };
  const s = await open({ host, app: app.name, storage: store, env: fresh, ...(size ? { size } : hostOpen(host, app)) });
  try {
    await s.clock('data');
    await s.clock('settle');
    const state = await s.state().catch(() => ({}));
    const facts = !size || size === WEB_SIZES[0] ? await seen(s, state) : null;
    const runs = await checkHere(s, { screen: FIRST, shot: (vp, scheme) => resolve(shots, `${host}-${vp.w}x${vp.h}-${scheme}.png`), state });
    if (facts) Object.defineProperty(runs, 'seen', { value: facts });
    Object.defineProperty(runs, 'refusals', { value: await refusalsIn(s) });
    if (host === 'ios' || size === WEB_SIZES[0]) Object.defineProperty(runs, 'large', { value: await largestText(s, FIRST) });
    if (host === 'ios') Object.defineProperties(runs, { tall: { value: await tallContent(s) }, udid: { value: s.carrier?.udid ?? null } });
    return runs;
  } finally {
    await s.close();
    if (host === 'web') await removeBrowserProfile(driveStore(app, store).profile).catch(() => {});
  }
}

/** Whether the screen `s` shows lays content out well beyond its height, below it or above (a chat opens at its
 * end): the feel probe's own test (`pickList`, 1.5 × the list's height + 100 pt) made on the screen's height, so a
 * screen that cannot hold a long list costs no probe. */
async function tallContent(s) {
  const l = await s.layout().catch(() => null), h = l?.viewport?.h ?? l?.screen?.h;
  if (!h) return true;
  const boxes = (l.nodes ?? []).filter((n) => Number.isFinite(n.y + n.h) && n.h > 0);
  return boxes.length > 0 && Math.max(...boxes.map((n) => n.y + n.h)) - Math.min(...boxes.map((n) => n.y)) > h * 1.5 + 100;
}

/** Whether a probe's timing can be judged: the 1-minute load average, at its start or end, no higher than the
 * cores. Above them, other work (builds) takes the simulator's frames, so a hitch says nothing about the app. */
const timingJudged = (load) => !load || load.avg <= load.cores;
/** `--json`'s `scrolling`: the verdict, the metrics found a tell or a bad with UIKit's values, remedies and owner
 * (`exact`, `app`, `either`: host/apple/feel/analyze.mjs `ownerOf`), the load it ran under, where feel.json
 * (every number) is, and when it was measured if this run reused it (`reused`). */
const scrollingOut = (sc) => sc.error ? { error: sc.error, ms: sc.ms }
  : { ms: sc.ms, verdict: sc.result.verdict, list: sc.result.list ?? null, summary: feelSummary(sc.result),
    findings: feelFindings(sc.result, null, { timing: timingJudged(sc.load) }), ...(sc.load ? { load: { ...sc.load, timingJudged: timingJudged(sc.load) } } : {}), feel: resolve(sc.out, 'feel.json'),
    ...(sc.reused ? { reused: new Date(sc.reused).toISOString() } : {}) };

const digest = (value) => createHash('sha256').update(value).digest('hex');
const fileDigest = (path) => { try { return digest(readFileSync(path)); } catch { return null; } };
/** What the probe is: its recorder, gestures and analysis, and the touch runner that plays them. */
const PROBE = ['host/apple/feel/feel.m', 'host/apple/feel/probe.mjs', 'host/apple/feel/analyze.mjs', 'host/apple/touches.mjs'];

/** What can change how the first screen's list scrolls, as one digest, or null when it cannot be told (no build, or
 * one stale in more than its plan): the build the drive launches (its receipt) and the bake of a plan-only edit it
 * runs on it (agent-build.mjs `planOverride`), the app's own server and the modules of its own that it imports, the
 * simulator, and the probe. Any edit to the plan's sources makes another bake or build, so it measures again. */
export async function scrollKey(app, { udid, server = null } = {}) {
  const [{ nativeBuild, planOverride, planSource }, { appleArtifacts }] = await Promise.all([import('./agent-build.mjs'), import('../host/apple/build.mjs')]);
  const receipt = resolve(appleArtifacts(app, { destination: 'ios-simulator' }).bundle, 'receipt.json'), built = fileDigest(receipt);
  if (!built || !udid) return null;
  const state = nativeBuild('ios', app, { checked: built });
  if (state.missing || state.changed.some((path) => !planSource(path, app))) return null;
  const override = state.changed.length ? planOverride('ios', app, state) : null;
  if (state.changed.length && !override) return null;
  const served = server ? [...serverModules(app.dir, server.entry)].sort().map((p) => [p, fileDigest(resolve(app.dir, p))]) : null;
  return digest(JSON.stringify({ built, plan: override && fileDigest(resolve(override.dir, 'override.json')), served, udid,
    probe: PROBE.map((p) => fileDigest(resolve(ROOT, p))) }));
}

/** A server's entry and the modules of the app's own it imports, transitively (at most 40). */
function serverModules(dir, entry) {
  const seen = new Set([entry]);
  for (const path of seen) for (const next of importsOf(dir, path)) if (seen.size < 40) seen.add(next);
  return seen;
}

/** Whether the measurement `last` (`{ key, load }`) stands for this run, whose key is `key` and whose load is `now`:
 * the same key, and timing judged then or not judgeable now either. */
export const reusable = (last, key, now) => !!key && last?.key === key && (timingJudged(last.load) || !timingJudged(now));

/** How long ago `at` was, in a few words. */
export const ago = (at, now = Date.now()) => {
  const min = (now - at) / 60_000;
  return min < 1 ? 'under a minute ago' : min < 90 ? `${Math.round(min)} min ago` : `${Math.round(min / 60)} h ago`;
};

/** How the first screen's list scrolls against UIKit's (host/apple/feel): the app launched again on a fresh scratch
 * store, on real time (`timing: 'platform'`, no `clock`), with the recorder inserted, and the probe's short script of
 * real touches played on its largest list. `{ ms, out, result, load }` (`result.verdict` is `no-scrollable-list` when
 * no shown list is long; `load`: the higher 1-minute load average of its start and end, and the cores), or
 * `{ error }`. A measurement is kept with its `key` (`scrollKey`) and taken again only when the key changed, `force`
 * (`--rescroll`) asks, or it could not judge timing and the machine now can: a reused one has `reused`, when it was
 * measured. */
async function scrolling(app, shots, { key = null, force = false } = {}) {
  const started = Date.now(), out = resolve(shots, 'ios-feel'), tracePath = resolve(out, 'trace.jsonl'), kept = resolve(out, 'measured.json');
  const load0 = loadavg()[0];
  if (!force) {
    try {
      const last = JSON.parse(readFileSync(kept, 'utf8'));
      if (reusable(last, key, { avg: load0, cores: availableParallelism() })) return { ms: last.ms, out, result: JSON.parse(readFileSync(resolve(out, 'feel.json'), 'utf8')), load: last.load, reused: last.at };
    } catch {}
  }
  try {
    const [{ open }, feel] = await Promise.all([import('./agent.mjs'), import('../host/apple/feel/probe.mjs')]);
    rmSync(out, { recursive: true, force: true });
    mkdirSync(out, { recursive: true });
    writeFileSync(tracePath, '');
    const s = await open({ host: 'ios', app: app.name, storage: 'verify', env: { EXACT_AGENT_STORAGE_FRESH: '1', ...feel.recorderEnv(tracePath) }, chrome: 'platform', timing: 'platform', touch: 'drag' });
    let played;
    try {
      await Bun.sleep(2500); // its first loads, on real time; the accessibility runtime the touch runner turns on
      played = await feel.play({ touches: s.carrier.touches, tracePath, script: 'short' });
    } finally { await s.close(); }
    const load = { avg: Math.round(Math.max(load0, loadavg()[0]) * 10) / 10, cores: availableParallelism() };
    const result = feel.finish(out, app.id, played, s.carrier.udid), ms = Date.now() - started;
    if (key) writeFileSync(kept, JSON.stringify({ key, at: Date.now(), ms, load }) + '\n');
    return { ms, out, result, load };
  } catch (e) { return { ms: Date.now() - started, error: e.message }; }
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
export function judgeRelaunch(test, fresh, after, { busy = [], failed = {}, heldBy = [] } = {}) {
  const problems = [];
  // Requests the server holds (`exactHeld`, LLP 1016.000 D9): named, never waited on, no finding.
  const notes = heldBy.length ? { notes: [`  held     ${heldBy.join(', ')}: the server holds ${heldBy.length === 1 ? 'it' : 'them'} (exactHeld); no clock waited on ${heldBy.length === 1 ? 'it' : 'them'}`] } : {};
  if (busy.length) problems.push(`  loading  still in flight after ${secs(REAL_MS)} of real time: ${busy.join(', ')}; a store or the network that slow leaves the first screen empty`);
  for (const [name, why] of Object.entries(failed ?? {})) problems.push(`  failed   resource ${name}: ${why}; the screen shows its placeholder or last value`);
  if (!fresh) return { shows: [], problems, ...notes, said: 'it relaunched' };
  const shows = [...after.texts].filter((t) => !fresh.texts.has(t)), wrote = test.end.reload || test.end.ops > fresh.ops;
  const held = !same(after.resources, fresh.resources), kept = held || !same(after.slots, fresh.slots);
  if (!shows.length && !busy.length && wrote && !same(test.end.resources, fresh.resources) && !held)
    problems.push('  lost     its first screen and resources are a fresh launch\'s, though the test wrote storage and ended holding data: what it stored did not come back');
  const said = shows.length ? `the first screen shows what it stored: ${shows.slice(0, 3).map((t) => quoted(t, 24)).join(', ')}`
    : kept ? 'its state holds what the test left, though the first screen\'s text is a fresh launch\'s'
    : wrote ? 'the first screen is a fresh launch\'s, as the test left it' : 'the first screen is a fresh launch\'s; that test wrote no storage';
  return { shows, problems, ...notes, said };
}

/** The facts a launch is made under that a test may pin (agent-launch.mjs `launchFacts`): the date at the clock's zero, the
 * zone, the locale and the seed. Media preferences are applied through the session before observations. */
const LAUNCH_FACTS = ['epoch', 'timeZone', 'locale', 'seed'];
const launchOf = (launch) => Object.fromEntries(LAUNCH_FACTS.filter((k) => launch?.[k] != null).map((k) => [k, launch[k]]));

/** The facts a relaunch on the store `test` left is made under: its launch facts, the date being the one it had reached
 * (its launch date plus its `clock` steps), as a `reload` relaunches (LLP 1116.009 D1): a phone restarted at 9:05 wakes at 9:05. */
export const relaunchFacts = (test) => ({ ...test.end?.launch, ...(test.end?.reached != null ? { epoch: test.end.reached } : {}) });

/** Whether the test ended at a date other than the one it launched at: its relaunch's first screen may say another day,
 * so a fresh launch to compare with is made at that date too. */
export const movedClock = (test) => test.end?.reached != null && test.end.launch?.epoch != null && test.end.reached !== test.end.launch.epoch;

/** Whether `test` launched under facts other than the driver's own, which the first screens launch under: a date, zone,
 * locale or seed its leading lines pinned, or a media preference it applied. What it stored is read back under those facts, and a fresh launch to compare
 * with must be made under them too (LLP 1116.007 D3.4). */
export function pinnedLaunch(test, defaults) {
  const launch = test.end?.launch;
  return !!launch && (LAUNCH_FACTS.some((k) => launch[k] != null && launch[k] !== defaults[k]) || Object.entries(launch.media ?? {}).some(([k, v]) => v !== (defaults.media?.[k] ?? LAUNCH_MEDIA[k])));
}

/** What a fresh launch shows under `launch`'s facts on an empty scratch store, for `seen`'s comparison. */
async function freshUnder(host, app, launch, store) {
  const [{ open }, { driveStore, removeBrowserProfile }] = await Promise.all([import('./agent.mjs'), import('./agent-launch.mjs')]);
  const s = await open({ host, app: app.name, storage: store, env: { EXACT_AGENT_STORAGE_FRESH: '1' }, ...launchOf(launch), ...(host === 'web' ? { size: WEB_SIZES[0] } : hostOpen(host, app)) });
  try {
    for (const [name, value] of Object.entries(launch?.media ?? {})) await s.prefer({ [name]: value });
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
  const s = await open({ host, app: app.name, storage: test.store, ...launchOf(relaunchFacts(test)), ...(host === 'web' ? { size: WEB_SIZES[0], ...(test.end.port ? { port: test.end.port } : {}) } : hostOpen(host, app)) });
  try {
    for (const [name, value] of Object.entries(test.end?.launch?.media ?? {})) await s.prefer({ [name]: value });
    const started = performance.now(), b = (st) => (st.background?.queued ?? 0) + (st.background?.inFlight ?? 0);
    const waiting = (st) => [...(st.pending ?? []).filter((p) => !p.device && !p.held).map((p) => p.name), ...(b(st) ? ['storage'] : [])];
    let state, busy;
    do { await s.clock(`+${REAL_STEP} real`); state = await s.state().catch(() => ({})); busy = waiting(state); } while (busy.length && performance.now() - started < REAL_MS);
    const ms = performance.now() - started;
    if (!busy.length) { await s.clock('settle'); state = await s.state().catch(() => state); }
    const after = await seen(s, state);
    const path = resolve(shots, `${host}-relaunch.png`);
    const runs = await checkHere(s, { screen: RELAUNCH, sizes: host === 'web' ? [null, WEB_SIZES[1]] : [null], shot: (vp, scheme) => (scheme === 'light' && (host !== 'web' || vp.w === WEB_SIZES[0][0]) ? path : undefined) });
    const refusals = await refusalsIn(s), large = await largestText(s, RELAUNCH);
    return { test: test.name, ms, shot: path, runs, refusals, large, ...judgeRelaunch(test, fresh, after, { busy, failed: state.failed, heldBy: (state.pending ?? []).filter((p) => p.held).map((p) => p.name) }) };
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
const screenName = (screen) => (screen === FIRST || screen === RELAUNCH || screen.startsWith('line ') ? screen : quoted(screen, 28));
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

const PLACES = 4; // where a rule's findings are, named after its first in full
const KINDS_FROM = 10; // findings from which the closing line says their kinds

/** A finding line's rule: a compiler warning's id, else its first word after any `(… only)` tag (`contrast`). */
export function ruleOf(line) {
  const t = line.trim(), id = /^\S+:\d+:\d+ warning \[([\w-]+)\]/.exec(t);
  return id ? id[1] : t.replace(/^\([^)]*\) /, '').split(/\s+/)[0];
}

/** Where a finding line points: a compiler warning's `file:line:col`, else what follows its rule, up to its first `: `. */
function placeOf(line) {
  const t = line.trim(), id = /^(\S+:\d+:\d+) warning \[/.exec(t);
  if (id) return id[1];
  const body = t.replace(/^\([^)]*\) /, '').replace(/^\S+\s+/, ''), at = body.indexOf(': ');
  return at < 0 ? body : body.slice(0, at);
}

/** `lines` (each indented two) by rule, the commonest first: a rule's one finding as it is; several as the rule and
 *  their count, the first in full, then where the next PLACES are and how many more. */
export function byRule(lines) {
  const rules = new Map();
  for (const l of lines) rules.set(ruleOf(l), [...(rules.get(ruleOf(l)) ?? []), l]);
  return [...rules].sort((a, b) => b[1].length - a[1].length).flatMap(([rule, all]) => {
    if (all.length === 1) return all;
    const rest = all.slice(1), more = rest.length - PLACES;
    return [`  ${rule} ×${all.length}`, `    ${all[0].trim()}`, `    also ${rest.slice(0, PLACES).map(placeOf).join('; ')}${more > 0 ? ` and ${more} more` : ''}`];
  });
}

/** The report's lines and its problem count, from what each stage gave; `full`, every finding of each group, one a line,
 *  which verify writes to `listed` (a path the report names when the groups leave any line out). */
export function report({ host, built, compiler = [], server, tested, checks, relaunched, refused = [], reloads = [], large = [], fixed = { lines: [], accepted: [] }, scrolled = null, desktop = null, shots, copied, own = [], accept = [], ms, cwd = process.cwd(), listed = null }) {
  const out = [], full = [];
  const all = listed ? `${listed} lists them all` : 'verify --json lists them all';
  let failedTests = 0, findings = 0, broken = 0;
  // Each counted finding's kind, for the closing line (LLP 1143 D5).
  const kinds = new Map(), counted = (kind, n = 1) => { if (n > 0) kinds.set(kind, (kinds.get(kind) ?? 0) + n); };
  const acceptable = new Set(); // the kinds `verify.accept` can take: check's, and fixed type
  // What the host refused while the app ran (LLP 1116.010 D5): first, since each is a call or a wiring the app got wrong.
  if (refused.length) {
    const shownAt = (where) => where.length > 2 ? `${where.slice(0, 2).join(', ')} and ${where.length - 2} more` : where.join(', ');
    const head = `refused at run time: ${refused.length} (the host's journal, 'logs'; each is a call or a wiring the app got wrong)`;
    out.push(head, ...refused.slice(0, SHOWN).map((r) => `  ${r.line.length > 260 ? `${r.line.slice(0, 259)}…` : r.line} (${shownAt(r.where)})`));
    if (refused.length > SHOWN) out.push(`  … and ${refused.length - SHOWN} more (${all})`);
    full.push(head, ...refused.map((r) => `  ${r.line} (${r.where.join(', ')})`));
    findings += refused.length; counted('refused at run time', refused.length);
  }
  // The app's own server (LLP 1116.006 D3): run on an empty database, or why not; one that died mid-run is a failure.
  if (server?.died) { broken++; out.push(`server: ${server.died}`); }
  else if (server) out.push(`server: ${server.said}`);
  if (built) {
    const said = built.said?.length ? built.said.join('\n') : `built ${host}`;
    out.push(`${said} (${secs(built.ms)}${built.warnings.length ? `, ${plural(built.warnings.length, 'warning')}` : ''})`);
    for (const w of built.warnings.slice(0, WARNINGS)) out.push(`  ${w}`);
    if (built.warnings.length > WARNINGS) out.push(`  … and ${built.warnings.length - WARNINGS} more (bun exact.mjs web-build prints them all)`);
    findings += built.warnings.length; counted('build warning', built.warnings.length);
  }
  // The compiler's warnings, each as `contract build` says it (LLP 1116.004 D7): findings, never a failure. Those that
  // are information (`information`: the platform's look stands) are said apart and not counted.
  const warned = compiler.filter((w) => !information(w)), noted = compiler.filter(information);
  if (warned.length) {
    const lines = warned.map((w) => `  ${w}`);
    out.push(`compiler: ${plural(warned.length, 'warning')}`, ...byRule(lines));
    full.push(`compiler: ${plural(warned.length, 'warning')} (bun exact.mjs contract build app.contract quotes each line)`, ...lines);
    findings += warned.length; for (const w of warned) counted(ruleOf(w));
  }
  if (noted.length) {
    const head = `compiler: ${plural(noted.length, 'note')}, not counted (the platform's own look stands there; LLP 1121 D9)`, lines = noted.map((w) => `  ${w}`);
    out.push(head, ...byRule(lines));
    full.push(head, ...lines);
  }
  if (tested?.none) out.push('tests: none (no *.test.contract)');
  if (tested?.error) {
    broken++;
    out.push(`tests: did not compile`, ...tested.error.split('\n').filter((l) => l.trim()).slice(0, 6).map((l) => `  ${l}`));
  }
  if (tested?.results && !(tested.error && !tested.results.length)) {
    failedTests = tested.failed;
    out.push(`tests: ${tested.passed} passed, ${tested.failed} failed`);
    for (const t of tested.results) for (const f of t.failures) out.push(`  FAIL "${t.name}": ${firstLine(f).replace(`${t.name}: `, '')}`);
    const unarranged = tested.results.flatMap((t) => (t.unarranged ?? []).map((u) => `  "${t.name}": ${u}`));
    if (unarranged.length) {
      out.push(`failed and not arranged: ${plural(unarranged.length, 'finding')}`, ...unarranged);
      full.push(`failed and not arranged: ${plural(unarranged.length, 'finding')}`, ...unarranged);
      findings += unarranged.length; counted('failed and not arranged', unarranged.length);
    }
  }
  // Every screen, launch and scheme in one group: each finding once, said where unless it was everywhere.
  for (const e of checks.filter((c) => c.error)) { broken++; out.push(`check ${e.label}: could not drive: ${firstLine(e.error)}`); }
  const runs = checks.filter((c) => !c.error && !c.unchecked);
  for (const l of new Set(checks.filter((c) => c.unchecked).flatMap((c) => c.lines))) { out.push(l); findings++; counted('failed resource'); }
  if (runs.length) {
    const shown = distinctFindings(runs), count = shown.length + Math.max(0, ...runs.map((r) => r.hidden));
    const kept = runs.some((r) => r.asked && r.asked !== r.scheme) ? '; the app keeps its own scheme' : '';
    const screens = [...new Set(runs.map(screenOf))], ends = screens.filter((sc) => sc !== FIRST && sc !== RELAUNCH).length;
    const where = [screens.includes(FIRST) && FIRST, ends && plural(ends, 'test end'), screens.includes(RELAUNCH) && RELAUNCH].filter(Boolean).join(', ');
    const head = `check ${where} · ${[...new Set(runs.map(sizeOf))].join(', ')} · ${[...new Set(runs.map(schemeOf))].join(', ')}${kept}: ${count ? plural(count, 'finding') : 'no findings'}`;
    out.push(head, ...byRule(shown));
    if (count > shown.length) out.push(`  … and ${count - shown.length} more (verify --json lists them all)`);
    if (shown.length) full.push(head, ...shown);
    out.push(...new Set(runs.flatMap((r) => r.advice ?? [])));
    findings += count; for (const l of shown) { counted(ruleOf(l)); acceptable.add(ruleOf(l)); } counted('check (not listed)', count - shown.length);
  }
  // Font and symbol sizes the app's Contract fixes and words it breaks (LLP 1116.020 D4, LLP 1116.021 D6): findings, as
  // check's are.
  if (fixed.lines.length) {
    const said = (fixed.fonts ?? fixed.lines.length) === fixed.lines.length ? `${plural(fixed.lines.length, 'font size')} the reader's text size does not move`
      : `${plural(fixed.lines.length, 'finding')}: a size the reader's text size does not move, or a word broken inside`;
    out.push(`fixed type: ${said}`, ...byRule(fixed.lines));
    full.push(`fixed type: ${said}`, ...fixed.lines);
    findings += fixed.lines.length; for (const l of fixed.lines) { counted(ruleOf(l)); acceptable.add(ruleOf(l)); }
  }
  // What app.json's `verify.accept` names as the app's design: said once each, not counted; an entry that took nothing is named.
  const accepted = [...distinctFindings(runs.map((r) => ({ ...r, lines: (r.accepted ?? []).map((f) => f.line) }))), ...fixed.accepted.map((f) => f.line)];
  const took = (e, f) => f.accept?.finding === e.finding && f.accept?.node === e.node;
  const idle = accept.filter((e) => !runs.some((r) => (r.accepted ?? []).some((f) => took(e, f))) && !fixed.accepted.some((f) => took(e, f)));
  if (accepted.length || idle.length) {
    out.push(`accepted (app.json verify.accept, not counted): ${plural(accepted.length, 'finding')}`, ...accepted.slice(0, SHOWN));
    if (accepted.length > SHOWN) out.push(`  … and ${accepted.length - SHOWN} more (${all})`);
    if (accepted.length) full.push(`accepted (app.json verify.accept, not counted): ${plural(accepted.length, 'finding')}`, ...accepted);
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
    if (lines.length > SHOWN) out.push(`  … and ${lines.length - SHOWN} more (${all})`);
    if (lines.length) full.push(`reload ${plural(ran.length, 'test end')}: ${plural(lines.length, 'finding')}`, ...lines);
    findings += lines.length; counted('reload', lines.length);
    if (notes.length) {
      out.push(`reload note: ${plural(notes.length, 'screen')} ${notes.length === 1 ? 'reloads' : 'reload'} into another state (not counted)`, ...notes.slice(0, SHOWN));
      if (notes.length > SHOWN) out.push(`  … and ${notes.length - SHOWN} more (${all})`);
      full.push(`reload note: ${plural(notes.length, 'screen')} (not counted)`, ...notes);
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
    out.push(`largest text size (${size}), ${plural(read.length, 'screen')}: ${shown.length ? `${plural(shown.length, 'finding')} (the app's own layout: give the text room; a smaller or fixed font is the tell)` : 'nothing spills'}`, ...byRule(shown));
    largeCount = shown.length; for (const l of shown) counted(`${ruleOf(l)} at the largest text size`);
    if (shown.length) full.push(`largest text size (${size}): ${plural(shown.length, 'finding')}`, ...shown);
    for (const l of large.filter((l) => l.error)) out.push(`  could not read ${screenName(l.screen)}: ${firstLine(l.error)}`);
  }
  // How the first screen's list scrolls against UIKit's (host/apple/feel): each metric a tell or a bad the app can
  // fix (owner `app` or `either`) is a finding, with UIKit's value and what causes it; those only Exact can fix are
  // one line, not counted; on a busy machine the timing metrics are left out. A list like UIKit's is one line; no
  // long list, nothing; a probe that could not run is said, not counted, since it is the tool's.
  if (scrolled?.error) out.push(`scrolling: could not measure: ${firstLine(scrolled.error)}`);
  else if (scrolled?.result?.verdict === 'probed') {
    const r = scrolled.result, place = relative(cwd, scrolled.out), timing = timingJudged(scrolled.load);
    const all = feelFindings(r, null, { timing }), found = all.filter((f) => !f.unsure && f.owner !== 'exact'), own = all.filter((f) => !f.unsure && f.owner === 'exact');
    const said = `${secs(scrolled.ms)} of real touches on the first screen's list, ${r.list?.cls?.split('.').at(-1) ?? 'a scroll view'}${scrolled.reused ? `, measured ${ago(scrolled.reused)} on this build (--rescroll measures again)` : ''}`;
    const value = (f) => { const u = f.unit ? ` ${f.unit}` : ''; return `${f.metric} ${f.value}${u} (${f.verdict}; UIKit ${f.uikit}${u})`; };
    const line = (f) => `scrolling ${value(f)}: ${f.remedy}`;
    const notes = all.filter((f) => f.unsure).map((f) => `  note, not counted (the probe also reads plain regions as blank): ${line(f)}`);
    const busy = timing ? [] : [`  the machine is busy (load ${scrolled.load.avg} on ${scrolled.load.cores} cores): timing not judged; physics only`];
    const exacts = own.length ? [`  Exact's own (not yours to fix; nothing in your app changes them): ${own.map(value).join('; ')}; report with ${place}/feel.json`] : [];
    if (!found.length) out.push(`scrolling: ${own.length ? 'nothing for the app to fix' : 'like UIKit'} (${feelSummary(r)}; ${said})`, ...busy, ...exacts, ...notes);
    else {
      const lines = found.map((f) => `  ${line(f)}`), head = `scrolling: ${plural(found.length, 'finding')} against UIKit (${said}); each says whose it is`;
      out.push(head, ...busy, ...lines, ...exacts, ...notes, `  the numbers: ${place}/feel.txt`);
      full.push(head, ...lines);
      findings += found.length; counted('scrolling', found.length);
    }
  }
  // The relaunch on a test's store (LLP 1116.005 D7): what came back, or each problem, counted as findings.
  if (relaunched?.error) { broken++; out.push(`relaunch: could not drive: ${firstLine(relaunched.error)}`); }
  else if (relaunched) {
    out.push(`relaunch on ${quoted(relaunched.test)}'s store (${secs(relaunched.ms)} of real time): ${relaunched.problems.length ? plural(relaunched.problems.length, 'problem') : relaunched.said}`, ...relaunched.problems, ...(relaunched.notes ?? []));
    findings += relaunched.problems.length; counted('relaunch', relaunched.problems.length);
  }
  if (shots.length) {
    const place = (p) => { const r = relative(cwd, p); return r.startsWith('..') ? p : r || '.'; }, name = (p) => relative(resolve(p, '..'), p);
    const first = shots.filter((p) => /^\w+-\d+x\d+-(light|dark)\.png$/.test(name(p))), ends = shots.filter((p) => /^\w+-t\d+[-.]/.test(name(p))), again = shots.filter((p) => /^\w+-relaunch\.png$/.test(name(p)));
    out.push(`screenshots (${place(resolve(shots[0], '..'))}${copied ? `, copied to ${place(copied)}` : ''}): ${first.map(name).join(' ')}`);
    const more = [ends.length && `test ends: ${ends.map(name).join(' ')}`, again.length && `relaunch: ${again.map(name).join(' ')}`, own.length && `its own: ${own.join(' ')}`].filter(Boolean);
    if (more.length) out.push(`  ${more.join('; ')}`);
  }
  // Windows: the drives above are headless (DejaVu Sans, no frame); the desktop's picture is one more, never counted.
  if (desktop) out.push(desktop.error ? `window: not captured (${firstLine(desktop.error)})` : `window: ${relative(cwd, desktop.shot)}, the app opened on the desktop at ${desktop.size?.join('x')} with the system's font, title bar and theme${desktop.foreground ? '' : ' (not the foreground window: its title bar is drawn inactive)'}`,
    '  not on Windows yet: check\'s findings (the host has no check observation; each screen is captured unjudged), the largest text size, the scrolling probe (iOS\'s), a test end\'s reload (the web\'s)');
  // How a person opens what ran (LLP 1116.008 D2): the dev loop serves the web app, the simulator build launches on iOS.
  if (host === 'android') out.push(ANDROID);
  if (!failedTests && !broken) out.push(host === 'ios' ? 'open it: bun exact.mjs ios --run (launches what verify checked)' : host === 'windows' ? 'open it: bun exact.mjs windows --run (opens what verify checked)' : host === 'android' ? 'open it: bun exact.mjs android --run (installs and launches what verify checked)' : 'open it: bun exact.mjs web (serves what verify checked)');
  // Every finding, one a line, where the groups above left any out.
  const grouped = full.length && out.some((l) => /^ {4}also |^ {2}… and \d+ more/.test(l));
  if (grouped && listed) out.push(`every finding, one a line: ${listed}`);
  const problems = failedTests + findings + largeCount + broken;
  // Many findings are said by kind, the commonest first, so the line an agent copies says what they are; a kind that is
  // most of them is named with the way to accept those the app's design means (LLP 1143 D5).
  const byKind = [...kinds].sort((a, b) => b[1] - a[1]), many = findings + largeCount >= KINDS_FROM;
  const shownKinds = byKind.slice(0, 3), others = byKind.slice(3).reduce((n, [, c]) => n + c, 0);
  const kindsSaid = [...shownKinds.map(([k, c]) => `${c} ${k}`), others && `${others} other`].filter(Boolean).join(', ');
  const why = [failedTests && plural(failedTests, 'failed test'), broken && plural(broken, 'failure', 'failures') + ' to compile or drive',
    ...(many ? [`${plural(findings + largeCount, 'finding')}: ${kindsSaid}`] : [findings && plural(findings, 'finding'), largeCount && `${largeCount} at the largest text size`])].filter(Boolean);
  const [top, topCount] = byKind[0] ?? [];
  if (many && acceptable.has(top) && topCount * 2 > findings + largeCount) out.push(`most findings are ${top} (${topCount}): fix them, or, where the design means them, app.json's verify.accept takes each by its kind and testId (docs/recipes/tests.md)`);
  out.push(problems ? `verify: ${plural(problems, 'problem')} (${why.join(', ')}${failedTests || broken ? '' : '; the tests passed'}) in ${secs(ms)}` : `verify: clean in ${secs(ms)}`);
  return { lines: out, problems, exit: failedTests || broken ? 1 : 0, full: grouped ? full : [] };
}

// `ended` takes the report's last line, which `main` prints last, after `contract verify`'s summary.
async function deviceMain(argv, emitJSON, declined, ended) {
  const args = guardFlags('verify', argv);
  const value = (flag) => args.indexOf(flag) >= 0 ? args[args.indexOf(flag) + 1] : undefined;
  const json = args.includes('--json'), sim = value('--sim'), copyTo = value('--shots');
  const flagged = new Set([args.indexOf('--app') + 1, args.indexOf('--sim') + 1, args.indexOf('--shots') + 1].filter((i) => i > 0));
  if (args.includes('--shots') && (!copyTo || copyTo.startsWith('-'))) { console.error('verify: --shots takes a directory: the screenshots are copied there, under the names in .exact/verify/'); return declined(2); }
  const host = args.find((a, i) => !a.startsWith('-') && !flagged.has(i)) ?? 'web';
  if (!['web', 'ios', 'windows', 'android'].includes(host)) { console.error(`verify: web, ios, windows or android, not ${host} (test ${host} runs the tests there)`); return declined(2); }
  if (host === 'windows' && process.platform !== 'win32') { console.error('verify windows runs on Windows (bun exact.mjs verify windows there)'); return declined(2); }
  if (host === 'android') process.env.EXACT_ANDROID_READER = '1'; // the APK, as `test android` drives it
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
  const refuse = (line) => { if (json) emitJSON({ host, server: { error: line }, problems: 1, exit: 1 }); else console.error(`verify: ${line}`); return declined(1); };
  if (server?.refusal) return refuse(server.refusal);
  // (A test run that holds the port's turn is waited for when the server starts.)
  if (owned && await answering(server.url) && !(await holderOf(server.port))) return refuse(heldLine(server));

  const last = lastRuns(app)[host];
  console.error(`verify ${host}: about ${Math.round((last ?? ESTIMATE[host]) / 1000)} s${last ? ', as its last run took' : ''}; one report when it ends`);
  const built = host === 'web' ? await webBuild(app) : host === 'windows' ? await windowsBuild(app) : host === 'android' ? await (await import('./agent-android.mjs')).apkBuildFirst(app) : await iosBuild(app);
  // The build said the compiler's warnings when it ran; the report's own group says them whether it ran or not.
  if (built?.said) { const { compilerWarnings } = await import('./agent-launch.mjs'); built.said = built.said.filter((line) => !compilerWarnings(line).length && !/^compiler: \d+ (?:warnings?|notes?)\b/.test(line)); }
  if (built?.code) {
    const tail = host === 'web' ? built.output.split('\n').filter((l) => l.trim()).slice(-20) : built.said;
    if (json) emitJSON({ host, built: { ms: built.ms, code: built.code, said: tail }, problems: 1, exit: built.code });
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
  let tested, firsts, relaunched = null, scrolled = null, desktop = null, results = [], running = null, toured = [], serverSaid = server?.why ? { said: `${server.why}; the tests use whatever answers there` } : null;
  const compiling = compilerWarningsOf(app);
  if (owned) {
    try { running = await startServer(app.dir, server, 'verify'); }
    catch (e) { return refuse(e.message); }
    serverSaid = { said: serverRun(server, 'verify') };
  }
  try {
    // An app with a server opens its first screen before the tests write to it: an empty database's.
    if (running) { firsts = host === 'web' ? await Promise.all(WEB_SIZES.map(launch)) : [await launch(null, 0)]; tested = await tests(host, app, shots); }
    else if (host === 'web') [tested, toured, ...firsts] = await Promise.all([tests(host, app, shots), largeTour(app), ...WEB_SIZES.map(launch)]);
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
    // An app's own server holds what the tests wrote by now: the tour, which writes again, goes last.
    if (running && host === 'web') toured = await largeTour(app);
    if (host === 'ios' && firsts.some((runs) => runs.tall)) {
      const key = await scrollKey(app, { udid: firsts.find((runs) => runs.udid)?.udid, server: owned ? server : null }).catch(() => null);
      scrolled = await scrolling(app, shots, { key, force: args.includes('--rescroll') });
    }
    if (host === 'windows') desktop = await desktopShot(app, shots);
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
  const kept = [...new Set([...checks.map((c) => c.shot), relaunched?.shot, desktop?.shot])].filter((p) => p && existsSync(p));
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
  const visited = [...firsts.map((runs) => runs.large), ...results.map((t) => t.end?.large), relaunched?.large].filter(Boolean);
  const large = [...visited, ...tourNews(visited, toured)];
  const fixed = await fixedType(app.dir, host);
  const findingsFile = resolve(shots, `${host}-findings.txt`), listed = relative(process.cwd(), findingsFile);
  const r = report({ host, built, compiler, server: serverSaid, tested, checks, relaunched, refused: refusedList, reloads, large, fixed, scrolled, desktop, shots: kept, copied: copyTo && resolve(copyTo), own, accept: ACCEPT, ms: Date.now() - started, listed: listed.startsWith('..') ? findingsFile : listed });
  if (r.full.length) writeFileSync(findingsFile, r.full.join('\n') + '\n'); else rmSync(findingsFile, { force: true });
  writeFileSync(resolve(shots, 'last.json'), JSON.stringify({ ...lastRuns(app), [host]: Date.now() - started }) + '\n');
  if (json) {
    const testsOut = tested?.results ? { ...tested, results: results.map(({ name, file, failures, end }) => ({ name, file, failures, ...(end?.shot && existsSync(end.shot) ? { shot: end.shot } : {}), ...(end?.error ? { endError: end.error } : {}) })) } : tested;
    const relaunchOut = relaunched && (({ runs, large, ...rest }) => rest)(relaunched);
    emitJSON({ host, built: built && { ms: built.ms, said: built.said ?? [], warnings: built.warnings }, compiler, ...(serverSaid ? { server: serverSaid } : {}), refused: refusedList, tests: testsOut, checks, reloads, large, fixed, relaunch: relaunchOut, ...(scrolled ? { scrolling: scrollingOut(scrolled) } : {}), ...(desktop ? { window: desktop } : {}), screenshots: kept, problems: r.problems, exit: r.exit, ms: Date.now() - started });
  } else { say(r.lines.slice(0, -1)); ended(r.lines.at(-1)); }
  // The run, against the build it used (LLP 1146 D2: `bun scripts/evidence.mjs`).
  (await import('./evidence.mjs')).recordRun(app.name, { host, what: 'verify', result: r.problems ? `${r.problems} problem${r.problems === 1 ? '' : 's'}${tested?.failed ? `, ${tested.failed} failed test${tested.failed === 1 ? '' : 's'}` : ''}` : 'clean', timing: 'agent', sim });
  return r.exit;
}

// Both runs contribute to the exit status, including when the device build or drive fails. A refusal (a bad flag,
// a server verify will not start or write to) stops both: it is one line and nothing runs (LLP 1116.008 D1).
async function main(argv) {
  const args = guardFlags('verify', argv), json = args.includes('--json');
  const { resolveApp } = await import('./app.mjs');
  const app = resolveApp(args.includes('--app') ? args[args.indexOf('--app') + 1] : undefined);
  const semantics = contractVerify(app, json);
  let device = null, code, refused = false, verdict = null;
  try { code = await deviceMain(args, result => { device = result; }, (exit) => { refused = true; return exit; }, line => { verdict = line; }); }
  catch (e) { code = 1; device = { error: e.message, exit: 1 }; if (!json) console.error(`verify: ${e.message}`); }
  if (refused) { semantics.stop(); if (json && device) console.log(JSON.stringify(device)); return code; }
  const done = await semantics.done;
  const exit = code || done.exit;
  if (json) { console.log(JSON.stringify({ ...device, semantics: { exit: done.exit, output: done.output }, exit })); return exit; }
  const kept = resolve(app.dir, '.exact', 'verify', 'properties.txt');
  mkdirSync(dirname(kept), { recursive: true });
  writeFileSync(kept, `${done.output}${done.errors}`);
  const listed = relative(process.cwd(), kept);
  console.log(semanticsSummary(done, listed.startsWith('..') ? kept : listed).join('\n'));
  if (verdict) console.log(done.exit ? `${verdict}; contract verify failed (exit ${done.exit}), as above` : verdict);
  return exit;
}

// The design tool, named here and not through its exact.mjs: on the checkout's own port (tool-drive.mjs).
if (import.meta.main) (await import('./tool-drive.mjs')).toolDrive('verify', process.argv.slice(2)).then(main).then((code) => process.exit(code), (e) => { console.error(e.message); process.exit(1); });
