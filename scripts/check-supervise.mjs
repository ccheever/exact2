// `exact contract build`'s app.ts check, supervised (the design tool's `build.mjs --check` spun 4.7 days with nothing
// to stop it, found 2026-10-26). It runs in its caller's process group; the builder runs under
// scripts/check-leader.mjs in a process group of its own, its output this process's (which the caller collects
// whole, as it did the builder's). The whole builder group, the builder and any `tsc` under it, is killed
// (scripts/check-group.mjs) when it outlasts the bound (js/bake/src/check-bound.mjs), when this process is told to
// stop (SIGINT, SIGTERM, SIGHUP), when it exits any other way, or when its caller (the pid it was given) is gone;
// and the leader kills it itself if this process is killed outright. Exits as the builder did, or 1 with the
// bound's words.
//   bun scripts/check-supervise.mjs <caller pid> <builder.mjs> <app> [args…]
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { boundWords, checkBound, CHECK_BOUND_MS } from '../js/bake/src/check-bound.mjs';
import { killGroup } from './check-group.mjs';

const [caller, builder, ...args] = process.argv.slice(2);
let child = null, ended = false;
const group = () => { if (child?.pid) killGroup(child.pid); };
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
const orphaned = () => process.ppid !== Number(caller);
// (Its caller gone before it ran: nothing is started.)
if (orphaned()) process.exit(1);
const bound = checkBound();
if (bound !== CHECK_BOUND_MS) console.error(`check: TEST bound ${bound} ms (EXACT_TEST_TIMING=1)`);
child = spawn(process.execPath, [fileURLToPath(new URL('check-leader.mjs', import.meta.url)), String(process.pid), builder, ...args], { stdio: 'inherit', detached: true });
const timer = setTimeout(() => end(1, `error: ${args[0]}: app.ts's type check did not finish in ${boundWords(bound)}, so it was stopped: bun ${[builder, ...args].join(' ')}`), bound);
const watch = setInterval(() => { if (orphaned()) end(1); }, 250);
child.on('error', (e) => end(1, `error: ${args[0]}: app.ts's type check could not start: ${e.message}`));
child.on('exit', (code, signal) => {
  if (ended) return;
  ended = true;
  clearTimeout(timer);
  clearInterval(watch);
  // (The builder done: anything it left running in its group goes with it, by the exit listener.)
  process.exit(code ?? (signal ? 1 : 0));
});
