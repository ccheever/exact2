#!/usr/bin/env bun
// Build an app's Linux host, the development build the agent drives (LLP 1086;
// LLP 1107 builds the same crate for Android):
//   bun scripts/build-linux.mjs <app>      then: bun scripts/agent.mjs linux --app <app> …
// An app outside this checkout is named by EXACT_APP_DIR, as its exact.mjs sets it.
import { spawnSync } from 'node:child_process';
import { bakeTarget, bakeWatchEnv, cargoEnvironment, linuxBinary, linuxBuild, resolveApp } from './app.mjs';
import { placeLibrary, prepareLibrary } from './package-libraries.mjs';
import { guardFlags } from './help.mjs';
import { desktopEntry } from './desktop-entry.mjs';
import { recordBuiltSettings } from '../js/bake/src/__exact_env.mjs';
import { writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

const [name] = guardFlags('linux', process.argv.slice(2));
if (!name) {
  console.error('usage: bun scripts/build-linux.mjs <app>   (then: bun scripts/agent.mjs linux --app <app> …)');
  process.exit(2);
}
const app = resolveApp(name), [cmd, ...args] = linuxBuild(app);
// A package's prebuilt library, fetched and checked before Cargo links it (LLP 1154 D3).
let library;
try { library = prepareLibrary(app.dir, bakeTarget('linux')); } catch (error) { console.error(error.message); process.exit(1); }
// With what its bake watches named (`bakeWatchEnv`): unset, an absent settings file is a watched input that is gone.
const built = spawnSync(cmd, args, { cwd: app.workspace ?? app.dir, env: cargoEnvironment({ ...process.env, ...bakeWatchEnv(app), CARGO_TARGET_DIR: app.target }), stdio: 'inherit' });
if (built.status === 0) {
  // Its desktop entry beside it, with the manifest's shortcuts as actions (LLP 1133.023 D17).
  const binary = linuxBinary(app), entry = resolve(dirname(binary), `${app.id}.desktop`);
  // Beside the executable, where its run path ($ORIGIN, or @executable_path on a Mac) finds it.
  if (library) placeLibrary(library, dirname(binary));
  writeFileSync(entry, desktopEntry(app, binary));
  // What it inlined of each setting (LLP 1153 D4), bound to the executable this build linked.
  recordBuiltSettings(binary, app);
  console.log(binary);
  console.log(entry);
}
process.exit(built.status ?? 1);
