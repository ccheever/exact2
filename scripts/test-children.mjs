// Child processes a test spawns, ended with everything they started however
// the test ends: its body's own `stop`, a failure, or bun:test's timeout,
// which skips the rest of the body (a timed-out dev-loop test left its loop,
// the resident wasm loop, `contract tamber serve` and the filesystem tool
// running for two hours, holding the fixture's lock). Importing this file in
// a bun:test file registers an `afterEach` that ends every child still
// running, and the process's exit ends any a crash left.
import { spawn, spawnSync } from 'node:child_process';
import { afterEach } from 'bun:test';

const running = new Set();
const posix = process.platform !== 'win32';

/** `spawn`, leading a process group of its own, registered until it is
 * stopped. The child gains `stop(grace)`: SIGTERM to its group, then SIGKILL
 * to the group and to every process it had started (which may have left the
 * group: a dev loop's tools lead their own) still alive after `grace` ms, or
 * once the child has exited. */
export function spawnOwned(command, args = [], options = {}) {
  const child = spawn(command, args, { ...options, detached: posix });
  running.add(child);
  // What it starts, as it starts it: by the time the test ends a grandchild
  // may already have lost its parent (and with it the way to find it).
  child.known = new Set();
  const look = () => { for (const pid of descendants(child.pid)) child.known.add(pid); };
  child.watch = posix ? setInterval(look, 250) : null;
  child.watch?.unref();
  child.stop = (grace = 30_000) => stopChild(child, grace);
  return child;
}

/** The processes below `pid`, by their parents now (`ps`); none on Windows. */
function descendants(pid) {
  if (!posix) return [];
  const listed = spawnSync('ps', ['-A', '-o', 'pid=,ppid='], { encoding: 'utf8' });
  const children = new Map();
  for (const line of (listed.stdout ?? '').split('\n')) {
    const [p, parent] = line.trim().split(/\s+/).map(Number);
    if (p && parent) children.set(parent, [...(children.get(parent) ?? []), p]);
  }
  const out = [], queue = [pid];
  while (queue.length) for (const p of children.get(queue.shift()) ?? []) { out.push(p); queue.push(p); }
  return out;
}

const alive = pid => { try { process.kill(pid, 0); return true; } catch { return false; } };
const signalGroup = (child, name) => {
  try { posix ? process.kill(-child.pid, name) : child.kill(name); } catch { /* gone */ }
};

async function stopChild(child, grace) {
  if (!running.has(child)) return;
  running.delete(child);
  clearInterval(child.watch);
  const below = [...new Set([...child.known, ...descendants(child.pid)])];
  // Told at once, before any await: after a timeout bun:test may not wait out
  // an asynchronous hook, so what runs synchronously is what certainly runs.
  const tell = name => { for (const pid of below) if (alive(pid)) { try { process.kill(pid, name); } catch { /* gone */ } } };
  if (child.exitCode === null && child.signalCode === null) {
    const exited = new Promise(ok => child.once('exit', ok));
    signalGroup(child, 'SIGTERM');
    tell('SIGTERM');
    const timer = new Promise(ok => setTimeout(() => ok(false), grace).unref());
    if (!(await Promise.race([exited.then(() => true), timer]))) signalGroup(child, 'SIGKILL');
  }
  // What it started and did not end (its own group, or orphaned by a kill).
  signalGroup(child, 'SIGKILL');
  tell('SIGKILL');
}

/** End every child still running (the hooks below call it). */
export async function stopAll(grace = 5_000) {
  await Promise.all([...running].map(child => stopChild(child, grace)));
}

afterEach(() => stopAll());
process.once('exit', () => {
  for (const child of running) { for (const pid of new Set([...child.known, ...descendants(child.pid)])) { try { process.kill(pid, 'SIGKILL'); } catch {} } signalGroup(child, 'SIGKILL'); }
});
