// `exact contract build`'s app.ts check, supervised (the design tool's `build.mjs --check` spun 4.7 days with nothing
// to stop it, found 2026-10-26). It runs in its caller's process group; the builder runs under
// scripts/check-leader.mjs, outside that group, in a process group of its own, its output this process's (which the
// caller collects whole, as it did the builder's). The whole builder group, the builder and any `tsc` under it, is
// killed (scripts/check-group.mjs) when it outlasts the bound (checkBound in js/bake/src/typescript.mjs), when this
// process is told to stop (SIGINT, SIGTERM, SIGHUP), when it exits any other way (the leader killed alone
// included), or when its caller (the pid it was given) is gone; and the leader kills it itself if this process is
// killed outright. Exits as the builder did, or 1 with the bound's words.
//   bun scripts/check-supervise.mjs <caller pid> <builder.mjs> <app> [args…]
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { boundWords, checkBound, CHECK_BOUND_MS } from '../js/bake/src/typescript.mjs';
import { killGroup, orphaned } from './check-group.mjs';

const [caller, builder, ...args] = process.argv.slice(2);
let child = null, builderPid = 0, ended = false;
// The leader told to stop (woken first, should it be stopped; on Windows, which can't signal it, its tree killed
// while it still roots one), and the builder's group killed by the pid the leader sent, then the leader itself, so
// nothing holding the caller's pipes is left. (Before the pid comes, the leader's own SIGTERM handler takes the
// builder's group: the window that leaves a builder is a SIGKILL of the leader alone between its spawn and its
// fd-3 write.)
const group = () => {
  if (!child?.pid) return;
  // (The leader signalled only while it runs: once it has exited and been reaped, its pid may be another process's.)
  const running = child.exitCode === null && child.signalCode === null;
  if (running && process.platform === 'win32') killGroup(child.pid);
  else if (running) for (const signal of ['SIGCONT', 'SIGTERM']) try { process.kill(child.pid, signal); } catch {}
  if (killGroup(builderPid) && running && process.platform !== 'win32') killGroup(child.pid);
};
const end = (code, said) => {
  if (ended) return;
  ended = true;
  group();
  if (said) console.error(said);
  process.exit(code);
};
// Before anything is started: a stop now leaves nothing behind.
for (const [signal, code] of [['SIGINT', 130], ['SIGTERM', 143], ['SIGHUP', 129]]) process.on(signal, () => end(code));
process.on('exit', group);
// (Its caller gone before it ran: nothing is started.)
if (orphaned(Number(caller))) process.exit(1);
const bound = checkBound();
if (bound !== CHECK_BOUND_MS) console.error(`check: TEST bound ${bound} ms (EXACT_TEST_TIMING=1)`);
child = spawn(process.execPath, [fileURLToPath(new URL('check-leader.mjs', import.meta.url)), String(process.pid), builder, ...args], { stdio: ['inherit', 'inherit', 'inherit', 'pipe'], detached: true });
child.stdio[3].on('data', (said) => { builderPid ||= parseInt(String(said), 10) || 0; });
const timer = setTimeout(() => end(1, `error: ${args[0]}: app.ts's type check did not finish in ${boundWords(bound)}, so it was stopped: bun ${[builder, ...args].join(' ')}`), bound);
const watch = setInterval(() => { if (orphaned(Number(caller))) end(1); }, 250);
child.on('error', (e) => end(1, `error: ${args[0]}: app.ts's type check could not start: ${e.message}`));
child.on('exit', (code, signal) => {
  if (ended) return;
  ended = true;
  clearTimeout(timer);
  clearInterval(watch);
  // (The builder done: anything it left running in its group goes with it, by the exit listener.)
  process.exit(code ?? (signal ? 1 : 0));
});
