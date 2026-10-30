// The app.ts check's builder, watched from outside its caller's process group (scripts/check-supervise.mjs runs
// this detached, so a SIGKILL of the caller's group never reaches it). It runs the builder (host/web-js/build.mjs
// --check) leading a process group of its own (so the builder can't leave it: a group leader can't setsid), sends
// the builder's pid to the supervisor on fd 3, and kills the builder's whole group itself when it's told to stop or
// when the supervisor (the pid it was given) is gone by any means, a SIGKILL of it included, whether the builder is
// still running or has just finished. Exits as the builder did.
//   bun scripts/check-leader.mjs <supervisor pid> <builder.mjs> <app> [args…]
import { spawn } from 'node:child_process';
import { writeSync } from 'node:fs';
import { killGroup, orphaned } from './check-group.mjs';

const [supervisor, builder, ...args] = process.argv.slice(2);
const parent = Number(supervisor);
let child = null;
const group = () => { if (child?.pid) killGroup(child.pid); };
// Before anything is started: a stop now leaves nothing behind.
for (const [signal, code] of [['SIGINT', 130], ['SIGTERM', 143], ['SIGHUP', 129]]) process.on(signal, () => { group(); process.exit(code); });
// (Its supervisor gone before it ran: nothing is started.)
if (orphaned(parent)) process.exit(1);
child = spawn(process.execPath, [builder, ...args], { stdio: 'inherit', detached: true });
if (child.pid) try { writeSync(3, `${child.pid}\n`); } catch {}
const watch = setInterval(() => { if (orphaned(parent)) { group(); process.exit(1); } }, 250);
child.on('error', () => process.exit(1));
child.on('exit', (code, signal) => {
  clearInterval(watch);
  // (Finished with the supervisor already gone: what the builder left running goes too, as the supervisor's exit
  // would have taken it.)
  if (orphaned(parent)) group();
  process.exit(code ?? (signal ? 1 : 0));
});
