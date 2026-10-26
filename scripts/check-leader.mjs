// The app.ts check's builder's group (scripts/check-supervise.mjs runs it detached): it leads a process group of
// its own, runs the builder (host/web-js/build.mjs --check) in it, and kills the whole group itself if its supervisor
// goes by any means (a SIGKILL of it, or of its caller's process group, which never reaches this group): its parent,
// the supervisor's pid it was given, changes. Exits as the builder did.
//   bun scripts/check-leader.mjs <supervisor pid> <builder.mjs> <app> [args…]
import { spawn } from 'node:child_process';
import { killGroup } from './check-group.mjs';

const [supervisor, builder, ...args] = process.argv.slice(2);
const orphaned = () => process.ppid !== Number(supervisor);
// (Its supervisor gone before it ran: nothing is started.)
if (orphaned()) process.exit(1);
const watch = setInterval(() => { if (orphaned()) killGroup(process.pid); }, 250);
const child = spawn(process.execPath, [builder, ...args], { stdio: 'inherit' });
child.on('error', () => process.exit(1));
child.on('exit', (code, signal) => { clearInterval(watch); process.exit(code ?? (signal ? 1 : 0)); });
