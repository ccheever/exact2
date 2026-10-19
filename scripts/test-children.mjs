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
// The end is synchronous (bun:test runs afterEach after a timeout but does not
// wait out an asynchronous hook, and fires no 'exit' handler): SIGTERM, a
// bounded wait while anything ends, then SIGKILL to what is left. POSIX only:
// on Windows the child alone is ended, not what it started.
import { spawn, spawnSync } from 'node:child_process';

const owned = new Set();
const posix = process.platform !== 'win32';

/** The processes on this machine now: pid → { ppid, start } (`ps`; start
 * is the process's start time, so a pid reused later is not taken for it). */
function table() {
  const listed = spawnSync('ps', ['-A', '-o', 'pid=,ppid=,stat=,lstart='], { encoding: 'utf8', timeout: 5_000 });
  const rows = new Map();
  for (const line of (listed.stdout ?? '').split('\n')) {
    const m = /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(.+?)\s*$/.exec(line);
    if (m) rows.set(Number(m[1]), { ppid: Number(m[2]), zombie: m[3].startsWith('Z'), start: m[4] });
  }
  return rows;
}

/** Add to `known` every process now below `pid` (pid → start time), and
 * forget any noted process that has exited. */
function note(known, pid, rows = table()) {
  const children = new Map();
  for (const [p, { ppid }] of rows) children.set(ppid, [...(children.get(ppid) ?? []), p]);
  const queue = [pid];
  while (queue.length) for (const p of children.get(queue.shift()) ?? []) { known.set(p, rows.get(p).start); queue.push(p); }
  for (const p of [...known.keys()]) if (rows.get(p)?.start !== known.get(p)) known.delete(p);
}

/** `spawn`, owned until it is stopped.
 * What it starts is noted as it starts it (a dev loop's tools lead their own
 * groups, and one may be orphaned before the test's end could look for it;
 * a grandchild that starts and loses its parent between two looks is missed:
 * best effort). The child gains `stop(grace)`. */
export function spawnOwned(command, args = [], options = {}) {
  const child = spawn(command, args, options);
  child.known = new Map();
  child.watch = posix ? setInterval(() => note(child.known, child.pid), 250) : null;
  child.watch?.unref();
  child.stop = (grace = 30_000) => stopChild(child, grace);
  owned.add(child);
  return child;
}

const sleep = ms => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
/** What of the child's still runs: the child itself while it is unreaped
 * (its pid cannot be reused until then) and not a zombie, and each noted
 * process that is still the one noted (same start time). */
function remaining(child, rows = table()) {
  const left = new Set();
  if (child.exitCode === null && child.signalCode === null && rows.has(child.pid) && !rows.get(child.pid).zombie) left.add(child.pid);
  for (const [pid, start] of child.known) { const row = rows.get(pid); if (row?.start === start && !row.zombie) left.add(pid); }
  return left;
}
const kill = (pids, name) => { for (const pid of pids) { try { process.kill(pid, name); } catch { /* gone */ } } };

/** SIGTERM to all of it at once, up to `grace` ms for it to end, then SIGKILL
 * to what is left; synchronous (see the head of this file). */
function stopChild(child, grace) {
  if (!owned.has(child)) return Promise.resolve();
  clearInterval(child.watch);
  if (!posix) {
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
  } else {
    note(child.known, child.pid);
    let left = remaining(child);
    kill(left, 'SIGTERM');
    const deadline = Date.now() + grace;
    while (left.size && Date.now() < deadline) { sleep(50); left = remaining(child); }
    kill(left, 'SIGKILL');
  }
  owned.delete(child);
  return Promise.resolve();
}

/** End every owned child (a test file's afterEach calls it). */
export function stopAll(grace = 5_000) {
  for (const child of [...owned]) stopChild(child, grace);
  return Promise.resolve();
}
