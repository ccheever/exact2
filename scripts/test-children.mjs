// Child processes a test spawns, ended with what they started however the
// test ends: its body's own `stop`, a failure, or bun:test's timeout, which
// skips the rest of the body (a timed-out dev-loop test left its loop, the
// resident wasm loop, `contract tamber serve` and the filesystem tool running
// for two hours, holding the fixture's lock). A test file registers the end:
//
//   import { afterEach } from 'bun:test';
//   import { spawnOwned, stopAll } from './test-children.mjs';
//   afterEach(() => stopAll());
//
// `child.stop(grace)` is asynchronous: SIGTERM, up to `grace` ms while what it
// owns ends, then SIGKILL. `stopAll` is the backstop and synchronous, since
// after a timeout bun:test runs afterEach but does not wait out an
// asynchronous hook (and fires no 'exit' handler). A process is known by its
// pid and its start time (`ps` lstart, to the second), so a pid reused later is
// not taken for it. POSIX only: on Windows the child alone is ended.
import { spawn, spawnSync } from 'node:child_process';

const owned = new Set();
let watcher = null;
const posix = process.platform !== 'win32';

/** The processes on this machine now, pid → { ppid, zombie, start }; null
 * when `ps` could not say (then nothing is taken for gone). */
function table() {
  const listed = spawnSync('ps', ['-A', '-o', 'pid=,ppid=,stat=,lstart='], { encoding: 'utf8', timeout: 5_000, env: { ...process.env, LC_ALL: 'C' } });
  if (listed.status !== 0 || !listed.stdout) return null;
  const rows = new Map();
  for (const line of listed.stdout.split('\n')) {
    const m = /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(.+?)\s*$/.exec(line);
    if (m) rows.set(Number(m[1]), { ppid: Number(m[2]), zombie: m[3].startsWith('Z'), start: m[4] });
  }
  return rows.size ? rows : null;
}

/** Whether `pid` is still the process noted with `start`, and running. */
const same = (rows, pid, start) => start !== undefined && rows.get(pid)?.start === start && !rows.get(pid).zombie;

/** Note every process now below the child or below any process it owns that
 * is still the one noted (pid → start), and forget those that ended. The
 * child is known by the start time it had when spawned. */
function note(child, rows) {
  if (child.start === undefined && rows.has(child.pid)) child.start = rows.get(child.pid).start;
  const roots = [...child.known].filter(([pid, start]) => same(rows, pid, start)).map(([pid]) => pid);
  if (same(rows, child.pid, child.start)) roots.push(child.pid);
  const children = new Map();
  for (const [p, { ppid }] of rows) children.set(ppid, [...(children.get(ppid) ?? []), p]);
  const queue = roots;
  while (queue.length) for (const p of children.get(queue.shift()) ?? []) { if (!child.known.has(p)) { child.known.set(p, rows.get(p).start); queue.push(p); } }
  for (const [p, start] of [...child.known]) if (rows.get(p)?.start !== start) child.known.delete(p);
}

/** What the child owns that still runs: itself and what it started, each
 * still the process noted. */
function remaining(child, rows) {
  const left = [...child.known].filter(([pid, start]) => same(rows, pid, start)).map(([pid]) => pid);
  if (same(rows, child.pid, child.start)) left.push(child.pid);
  return left;
}
const kill = (pids, name) => { for (const pid of pids) { try { process.kill(pid, name); } catch { /* gone */ } } };

/** `spawn`, owned until it is stopped. What it starts is noted as it starts
 * it, and through a stop's grace (a dev loop's tools lead their own groups and
 * may be orphaned before a test's end could look for them; one started and
 * orphaned between two looks is missed: best effort). */
export function spawnOwned(command, args = [], options = {}) {
  const child = spawn(command, args, options);
  child.known = new Map();
  if (posix) {
    const rows = table();
    if (rows) note(child, rows);
    // One look for every owned child (each `ps -A` is the whole machine).
    watcher ??= setInterval(() => { const now = owned.size && table(); if (now) for (const c of owned) note(c, now); }, 250);
    watcher.unref();
  }
  child.stop = (grace = 30_000) => (child.stopping ??= stopSoon(child, grace));
  owned.add(child);
  return child;
}

/** One look: signal what still runs with `name`; true when nothing does. */
function step(child, name) {
  if (!posix) {
    if (child.exitCode === null && child.signalCode === null) child.kill(name);
    return child.exitCode !== null || child.signalCode !== null;
  }
  const rows = table();
  if (!rows) {
    // No table, no identity: only the child itself, by its handle.
    if (child.exitCode === null && child.signalCode === null) child.kill(name);
    console.warn(`test-children: ps failed; what pid ${child.pid} started may be left running`);
    return name === 'SIGKILL';
  }
  note(child, rows);
  const left = remaining(child, rows);
  kill(left, name);
  return !left.length;
}

async function stopSoon(child, grace) {
  const end = Date.now() + grace;
  // SIGTERM again at each look: what was started since (a cleanup helper) hears it too.
  for (let done = step(child, 'SIGTERM'); !done && owned.has(child) && Date.now() < end; done = step(child, 'SIGTERM')) {
    await new Promise(ok => setTimeout(ok, 50));
    if (!owned.has(child)) return; // the backstop took it
  }
  if (owned.has(child)) { step(child, 'SIGKILL'); finish(child); }
}

function stopNow(child, grace) {
  const end = Date.now() + grace;
  let done = step(child, 'SIGTERM');
  while (!done && Date.now() < end) { Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 50); done = step(child, 'SIGTERM'); }
  if (!done) step(child, 'SIGKILL');
  finish(child);
}

function finish(child) {
  owned.delete(child);
  if (!owned.size && watcher) { clearInterval(watcher); watcher = null; }
}

/** The backstop: end every owned child now, synchronously, a stop in
 * progress included (a test file's afterEach calls it). */
export function stopAll(grace = 5_000) {
  for (const child of [...owned]) stopNow(child, grace);
  return Promise.resolve();
}
