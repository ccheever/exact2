// What scripts/design-tool*.test.mjs and design-tool-pair.test.mjs share: the
// tool on this checkout's own port (EXACT_TOOL_PORT), taken in turn; a drive
// of it; and the tool through a proxy in front of an app's design server.
import { afterEach, beforeAll, beforeEach } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { checkoutPort, claimPort, takePort } from './port-lock.mjs';
import { spawnOwned, stopAll } from './test-children.mjs';
import { upstream } from './tamber-request.mjs';

export const ROOT = resolve(new URL('..', import.meta.url).pathname);
// The tool's port for this run: one of its own, held for the whole run (`claimPort`, from this checkout's own on,
// one of 200 above 17740, never the 7740 a person's tool holds), so two runs at once (one checkout's or two's) each
// have one and neither waits; EXACT_TOOL_PORT names one instead, taken in turn by each test. Every child the tests
// spawn serves and frames that origin (and has its turn, EXACT_PORT_TURNS).
// (From one past the checkout's own, which its contract tests' stub takes in turn, so neither keeps the other waiting.)
const claimed = process.env.EXACT_TOOL_PORT === undefined ? await claimPort(checkoutPort(ROOT) + 1, { label: `the design tool's tests in ${ROOT}`, except: [checkoutPort(ROOT)] }) : null;
if (claimed) process.env.EXACT_TOOL_PORT = String(claimed.port);
// The port module reads the port once, at its first import: a file run before these in the same process
// (tamber-server.test.mjs) loads it with none (7740), where these runs' children serve this checkout's; then
// these take an instance of their own (a query is a module of its own).
const loaded = await import('./tamber-tool-port.mjs');
export const { TOOL_DIST, TOOL_ORIGIN, TOOL_PORT, stageTool } = String(loaded.TOOL_PORT) === process.env.EXACT_TOOL_PORT ? loaded : await import(`./tamber-tool-port.mjs?port=${process.env.EXACT_TOOL_PORT}`);
// The app a drive names: the staged tool (no server of its own) on another port, so no drive starts the stub on 7740.
export const DRIVEN = TOOL_PORT === 7740 ? { args: ['--app', 'design-tool'], env: process.env } : { args: [], env: { ...process.env, EXACT_APP_DIR: stageTool().dir } };
// The stub's app (its components' params, which the stub's design sets):
// the app beside a copy of stub/dials.tamber.
export const STUB_APP = readFileSync(resolve(ROOT, 'apps/design-tool/stub/app.contract'), 'utf8');

// The tool's port, in turn with other runs (the contract tests' stub, other
// checkouts'): each test of a file that calls this takes it, and a tool that
// took no turn there (one run by hand) is said by its pid; each test's
// children end with it (test-children.mjs).
export function toolTurns() {
let givePort = () => {};
// The tool built for this port before any test starts a server for it to go through: a cold build (minutes, several
// workers' at once) once left the server a test had started first unanswering by the time the tool asked it.
beforeAll(toolBuilt, 25 * 60_000);
beforeEach(async () => {
  const port = Number(new URL(TOOL_ORIGIN).port);
  // (A port this run claimed is its own already: no turn to wait for.)
  givePort = claimed ? () => {} : await takePort(port, { label: `the design tool's tests in ${ROOT}` });
  const pid = spawnSync('lsof', ['-nP', `-iTCP:${port}`, '-sTCP:LISTEN', '-t'], { encoding: 'utf8' }).stdout.trim().split('\n')[0];
  if (pid) {
    givePort();
    const command = spawnSync('ps', ['-o', 'command=', '-p', pid], { encoding: 'utf8' }).stdout.trim().slice(0, 80);
    throw new Error(`${TOOL_ORIGIN} already answers (pid ${pid}: ${command}), and took no turn (a design tool run by hand?): stop it (kill ${pid}) and run these again`);
  }
}, 25 * 60_000); // as long as the turn may take (its wait is 20 minutes)
// What a test started ends before its turn at the port does, however the test
// ends (a timed-out one skips its own stops).
afterEach(() => { stopAll(); givePort(); });
}

/** The slow lane (EXACT_SLOW_TESTS=1): the design server's real clocks, where other runs shorten them. */
export const SLOW = process.env.EXACT_SLOW_TESTS === '1';
/** The environment for a design server whose clocks a test shortens (tamber-timing.mjs: `{ POLL: 6000 }` is
 * EXACT_TAMBER_POLL_MS), under the test switch; the slow lane's keeps the real clocks. */
export const timed = knobs => (SLOW ? process.env : { ...process.env, EXACT_TAMBER_TEST_TIMING: '1', ...Object.fromEntries(Object.entries(knobs).map(([k, v]) => [`EXACT_TAMBER_${k}_MS`, String(v)])) });

/** Signals a drive waits on (`clock +N real until <path>`, a real wait ended early once the file exists): files in a
 * temp directory of this test's own, so no two runs share one. `pause(ms, n)` is the op for the n-th wait (N its
 * bound, today's wait); `fire(n)` writes its file at the event; `drop()` removes the directory. */
export function signals(name) {
  const dir = mkdtempSync(join(tmpdir(), `exact-design-tool-${name}-signals-`));
  return { pause: (ms, n) => `clock +${ms} real until ${join(dir, String(n))}`, fire: n => writeFileSync(join(dir, String(n)), ''), drop: () => rmSync(dir, { recursive: true, force: true }) };
}

/** This port's tool built once in this process, as `tamber tool` builds it (a run on a copy of the theme fixture, its
 * URL said once built, then stopped); nothing when its build is current (`tamber tool`'s own check, tamber-server.mjs
 * buildTool) or the port is not this run's own. */
let built = null;
export function toolBuilt() {
  return built ??= (async () => {
    if (!claimed) return;
    const staged = stageTool(), caller = process.env.EXACT_APP_DIR;
    process.env.EXACT_APP_DIR = staged.dir;
    let tool;
    try { tool = (await import('./app.mjs')).resolveApp(); } finally { if (caller === undefined) delete process.env.EXACT_APP_DIR; else process.env.EXACT_APP_DIR = caller; }
    const [{ builtAppMatches, jsTargetBuild }, { webChanges }] = await Promise.all([import('../host/web/serve.mjs'), import('./agent-launch.mjs')]);
    if (await builtAppMatches(TOOL_DIST, tool) && jsTargetBuild(TOOL_DIST) && !webChanges(TOOL_DIST, tool).all.length) return;
    const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-build-'));
    try {
      for (const f of ['app.contract', 'app.tamber', 'app.json']) writeFileSync(join(app, f), readFileSync(resolve(ROOT, 'apps/theme-fixture', f)));
      const tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
      try { await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 20 * 60_000); } finally { await stop(tool); }
    } finally { rmSync(app, { recursive: true, force: true }); }
  })();
}

/** What a wait after a signal says when the event it waited for is missing: a heartbeat the server sent before the
 * event, landed after the signal, can release `clock held` first (Astra, dt/dtt round 2; QUEUE.md, "A heartbeat in
 * transit"). Left open by theme2's ruling: a gate it fails in closes it, one more `clock held` after the signal. */
export const HEARTBEAT_RACE = 'the event the drive waited for is missing: if a heartbeat sent before it landed after the signal (QUEUE.md, "A heartbeat in transit can release a signalled wait"), close that race: one more `clock held changes` after the signal';

/** Lines of `child`'s stdout as they come; `until(pass)` waits for one. */
export function lines(child) {
  const seen = [];
  let buf = '';
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', d => { buf += d; const all = buf.split('\n'); buf = all.pop(); seen.push(...all); });
  // Its stderr's tail too, said when a wait fails (a child that exits, or waits on a lock, says why there).
  let err = '';
  child.stderr?.setEncoding?.('utf8');
  child.stderr?.on?.('data', d => { err = (err + d).slice(-3000); });
  const until = async (pass, ms) => {
    const end = Date.now() + ms;
    while (!seen.some(pass)) { if (Date.now() > end || child.exitCode !== null) throw new Error(`not seen${child.exitCode !== null ? ` (it exited ${child.exitCode})` : ` in ${ms} ms`}: ${seen.join('\n')}${err ? `\n--- its stderr's tail:\n${err}` : ''}`); await Bun.sleep(50); }
    return seen.find(pass);
  };
  return { seen, until };
}

/** Ends `child` and what it started (SIGTERM, then SIGKILL after 15 s). */
export const stop = child => child.stop(15_000);

/** `bun scripts/agent.mjs web --url <url>` running `ops`, each reply parsed
 * (spawned, not run synchronously: the proxy below answers in this
 * process). */
export async function drive(url, storage, ops, { keepStorage = false } = {}) {
  const child = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/agent.mjs'), 'web', ...DRIVEN.args, '--url', url, '--storage', storage, ...(keepStorage ? ['--keep-storage'] : []), '--json', ...ops], { cwd: ROOT, env: DRIVEN.env, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  child.stderr.on('data', d => { said = (said + d).slice(-4000); });
  const out = lines(child);
  const code = await new Promise(ok => child.once('close', ok));
  if (code !== 0) throw new Error(`the drive failed: ${said}\n${out.seen.slice(-3).join('\n').slice(0, 3000)}`);
  return out.seen.filter(l => l.startsWith('{')).map(l => JSON.parse(l));
}

/** The tool on `app` through `tamber tool --via` to a proxy in front of
 * the app's design server: the proxy records each write's file as the
 * server left it (read once its reply is in, so in the order written), and
 * calls `before(request)` before forwarding a request, so a hand edit lands
 * exactly between two of the tool's requests, with no timing. `env` is the
 * design server's (`timed(…)` for shortened clocks). */
export async function proxied(app, before = () => {}, { env = process.env } = {}) {
  const serve = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'serve', app], { cwd: ROOT, env, stdio: ['ignore', 'pipe', 'pipe'] });
  const origin = new URL(await lines(serve).until(l => /^http:\/\/127\.0\.0\.1:\d+\/__tamber$/.test(l), 600_000)).origin;
  const writes = [];
  const proxy = Bun.serve({ hostname: '127.0.0.1', port: 0, idleTimeout: 0, fetch: async req => {
    const url = new URL(req.url);
    const body = req.method === 'POST' ? await req.text() : undefined;
    let request = {};
    try { request = url.pathname === '/__tamber' && body ? JSON.parse(body) : {}; } catch { /* not ours */ }
    await before(request);
    const headers = new Headers(req.headers);
    headers.delete('host'); headers.delete('content-length');
    const res = await upstream(`${origin}${url.pathname}${url.search}`, { method: req.method, headers, body, redirect: 'manual' });
    const bytes = await res.arrayBuffer();
    if (request.method === 'transaction' || request.method === 'undo' || request.method === 'accept') {
      const reply = JSON.parse(new TextDecoder().decode(bytes));
      writes.push({ method: request.method, ok: !!reply.ok, error: reply.error, after: readFileSync(join(app, 'app.tamber'), 'utf8') });
    }
    const out = new Headers(res.headers);
    out.delete('content-encoding'); out.delete('content-length');
    return new Response(bytes, { status: res.status, headers: out });
  } });
  const tool = spawnOwned(process.execPath, [resolve(ROOT, 'scripts/exact.mjs'), 'tamber', 'tool', app, '--via', `http://127.0.0.1:${proxy.port}`, '--no-open'], { cwd: ROOT, stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  tool.stderr.on('data', d => { said = (said + d).slice(-4000); });
  const url = await lines(tool).until(l => l.startsWith(`${TOOL_ORIGIN}/__tamber/tool/`), 600_000).catch(e => { throw new Error(`${e.message}\n${said}`); });
  return { url, origin, writes, close: async () => { await stop(tool); proxy.stop(true); await stop(serve); } };
}

/** Where two texts differ, in order: the first line that differs, the
 * lines taken out there and the lines put in. */
export const edit = (a, b) => {
  const [x, y] = [a.split('\n'), b.split('\n')];
  let i = 0, j = 0;
  while (i < x.length && i < y.length && x[i] === y[i]) i++;
  while (j < x.length - i && j < y.length - i && x[x.length - 1 - j] === y[y.length - 1 - j]) j++;
  return { at: i, removed: x.slice(i, x.length - j), added: y.slice(i, y.length - j) };
};

/** A copy of the dials stub (its design and app) in a git repository of its own, one commit in. */
export function repo(name) {
  const app = mkdtempSync(join(tmpdir(), `exact-design-tool-${name}-`));
  writeFileSync(join(app, 'app.tamber'), readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8'));
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: app, encoding: 'utf8' });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  // Who the server's commit is by, here, whatever this machine's git says.
  git('config', 'user.email', 't@example.test'); git('config', 'user.name', 't');
  return { app, git };
}
