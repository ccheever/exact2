// `exact contract build`'s app.ts check, supervised (the design tool's `build.mjs --check` spun 4.7 days with nothing
// to stop it, found 2026-10-26). The builder runs in a process group of its own, its output this process's (which
// the caller collects whole, as it did the builder's), and the whole group, the builder and any `tsc` under it, is
// killed (SIGKILL) when it outlasts the bound (js/bake/src/check-bound.mjs), when this process is told to stop
// (SIGINT, SIGTERM, SIGHUP), or when its caller is gone past any handler (its parent changes). Exits as the builder
// did, or 1 with the bound's words.
//   bun scripts/check-supervise.mjs <builder.mjs> <app> [args…]
import { spawn } from 'node:child_process';
import { boundWords, checkBound, CHECK_BOUND_MS } from '../js/bake/src/check-bound.mjs';

const [builder, ...args] = process.argv.slice(2);
const bound = checkBound();
if (bound !== CHECK_BOUND_MS) console.error(`check: TEST bound ${bound} ms (EXACT_TEST_TIMING=1)`);
const child = spawn(process.execPath, [builder, ...args], { stdio: 'inherit', detached: true });
const parent = process.ppid;
let ended = false;
const group = () => { try { process.kill(-child.pid, 'SIGKILL'); } catch { /* none left */ } };
const end = (code, said) => {
  if (ended) return;
  ended = true;
  group();
  if (said) console.error(said);
  process.exit(code);
};
const timer = setTimeout(() => end(1, `error: ${args[0]}: app.ts's type check did not finish in ${boundWords(bound)}, so it was stopped: bun ${[builder, ...args].join(' ')}`), bound);
const watch = setInterval(() => { if (process.ppid !== parent) end(1); }, 500);
for (const [signal, code] of [['SIGINT', 130], ['SIGTERM', 143], ['SIGHUP', 129]]) process.on(signal, () => end(code));
child.on('error', (e) => end(1, `error: ${args[0]}: app.ts's type check could not start: ${e.message}`));
child.on('exit', (code, signal) => {
  if (ended) return;
  ended = true;
  clearTimeout(timer);
  clearInterval(watch);
  // (The builder done: anything it left running in its group goes with it.)
  group();
  process.exit(code ?? (signal ? 1 : 0));
});
