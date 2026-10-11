#!/usr/bin/env bun
// Build an app's Linux host, the development build the agent drives (LLP 1086;
// LLP 1107 builds the same crate for Android):
//   bun scripts/build-linux.mjs <app>      then: bun scripts/agent.mjs linux --app <app> …
// An app outside this checkout is named by EXACT_APP_DIR, as its exact.mjs sets it.
import { spawnSync } from 'node:child_process';
import { linuxBinary, linuxBuild, resolveApp } from './app.mjs';
import { guardFlags } from './help.mjs';

const [name] = guardFlags('linux', process.argv.slice(2));
if (!name) {
  console.error('usage: bun scripts/build-linux.mjs <app>   (then: bun scripts/agent.mjs linux --app <app> …)');
  process.exit(2);
}
const app = resolveApp(name), [cmd, ...args] = linuxBuild(app);
const built = spawnSync(cmd, args, { cwd: app.workspace ?? app.dir, env: { ...process.env, CARGO_TARGET_DIR: app.target }, stdio: 'inherit' });
if (built.status === 0) console.log(linuxBinary(app));
process.exit(built.status ?? 1);
