#!/usr/bin/env node
// exact — run an Exact app from a terminal on macOS, and install one so it
// keeps running from a terminal after you close the laptop.
//
//   exact run <app> [file …]     build if stale, launch in the foreground
//   exact install <app>          a real .app in ~/Applications, plus a shim
//   exact uninstall <app>        take both away again
//   exact list                   the apps this repo has, and what is installed
//
// The two things this exists to get right, because they are the two that make
// a Mac GUI app awkward from a shell:
//
//   1. **Bundle identity.** A bare Mach-O has no Info.plist, so it has no
//      name, no document types, no Dock tile, and Launch Services cannot find
//      it — `open -a` fails and Open With never lists it. `run` and `install`
//      both launch `<Name>.app/Contents/MacOS/ExactMac`: the executable inside
//      a bundle, which *is* the app, with stdout still attached to the
//      terminal that started it. @ref LLP 1033 D2
//   2. **Where the file argument goes.** `exact run markdown README.md` and
//      `mdview README.md` and a Finder double-click are three different OS
//      routes; all three end at the app's `open-file` node (LLP 1033 D3).
//      `run` passes paths as arguments; the installed shim goes through
//      `open`, which hands them to an already-running copy instead of
//      starting a second one.
//
// The shim is a shell script, not a symlink: a symlink into a bundle is a
// second executable path for the same binary, and macOS gives it the bundle
// identity of whatever the *symlink* is beside — which is not the app.
import { spawn, spawnSync } from 'node:child_process';
import { accessSync, chmodSync, constants, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { resolve } from 'node:path';
import { resolveApp } from './app.mjs';
import { builtAppMatches } from '../host/web/serve.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const APPLICATIONS = resolve(homedir(), 'Applications');

/** The app's assembled bundle in this repo — `host/apple/build.mjs --bundle`'s one stable output. */
export const bundleOf = (app) => resolve(app.target, 'clients', app.id, 'macos', `${app.displayName}.app`);
/** The executable inside a bundle: what a terminal launches to keep stdio. */
export const executableIn = (bundle) => resolve(bundle, 'Contents/MacOS/ExactMac');
/** The name this app answers to on the command line (`app.command`, else its directory's name). */
export const commandOf = (app) => app.manifest.app?.command ?? app.name;
/** Where `install` puts the app. */
export const installedAt = (app) => resolve(APPLICATIONS, `${app.displayName}.app`);

/** Where a shim goes: `EXACT_BIN_DIR`, else the first of these already on PATH, else `~/.local/bin` (made, and named in the advice). */
export function binDirectory() {
  if (process.env.EXACT_BIN_DIR) return resolve(process.env.EXACT_BIN_DIR);
  const path = (process.env.PATH ?? '').split(':').map((p) => p && resolve(p));
  const candidates = [resolve(homedir(), '.local/bin'), '/usr/local/bin', resolve(homedir(), 'bin')];
  return candidates.find((dir) => path.includes(dir) && writable(dir)) ?? candidates[0];
}

const writable = (dir) => { try { accessSync(dir, constants.W_OK); return true; } catch { return false; } };
const onPath = (dir) => (process.env.PATH ?? '').split(':').some((p) => p && resolve(p) === dir);

/** Refuse a bundle that is not this app's.
 *
 * `host/apple/build.mjs` links every app's `ExactMac` into one shared
 * product path and then copies it into that app's bundle, so two builds of
 * different apps running at once — two terminals, two agents — race, and the
 * loser ships a bundle whose Info.plist says one app and whose executable is
 * another. It launches, it looks wrong, and nothing says why.
 *
 * The app's identity is compiled into its archive (the baked compatibility
 * id), so the executable carries its own id and no other app's. Signing adds
 * this bundle's id to any binary, which is why presence alone proves
 * nothing: a foreign id is the tell. Cheap, and it turns a silent wrong app
 * into a sentence. */
function refuseForeignBundle(app, bundle) {
  const executable = executableIn(bundle);
  const bytes = readFileSync(executable, 'latin1');
  const others = readdirSync(resolve(ROOT, 'apps'))
    .filter((name) => name !== app.name && existsSync(resolve(ROOT, 'apps', name, 'app.contract')))
    .map((name) => { try { return resolveApp(name).id; } catch { return null; } })
    .filter((id) => id && id !== app.id);
  const foreign = others.filter((id) => bytes.includes(id));
  if (!bytes.includes(app.id)) throw new Error(`${executable.replace(ROOT + '/', '')} does not carry ${app.id}; another build raced this one — run it again with nothing else building`);
  if (foreign.length) throw new Error(`${executable.replace(ROOT + '/', '')} carries ${foreign.join(', ')}, not just ${app.id}: another app's build raced this one (host/apple/build.mjs links every app into one product path). Wait for it to finish and run this again.`);
}

/** Build the app's macOS bundle. Cargo and SwiftPM decide what is stale; this always asks them. */
function build(app, { quiet = false } = {}) {
  const r = spawnSync(process.execPath, [resolve(ROOT, 'host/apple/build.mjs'), app.crate('apple'), '--bundle'], {
    cwd: ROOT,
    stdio: quiet ? ['inherit', 'ignore', 'inherit'] : 'inherit',
    env: { EXACT_UPDATE_TRUST: 'development', ...process.env },
  });
  if (r.status !== 0) process.exit(r.status ?? 1);
  const bundle = bundleOf(app);
  if (!existsSync(bundle)) throw new Error(`host/apple/build.mjs left no bundle at ${bundle}`);
  refuseForeignBundle(app, bundle);
  return bundle;
}

/** The dev loop's plan, when the one on disk is *this* app's.
 *
 * `host/web/dev.mjs` writes every rebuild to one shared `host/web/dist`, so
 * the plan sitting there belongs to whichever app the dev server is running.
 * Pointing a client at another app's plan does not live-reload it — the
 * runner refuses the boot outright (`AppMismatch`) and you get no window.
 * The dist marker says whose it is, so this asks rather than assumes: a
 * match is live reload, anything else is the baked plan and a printed line
 * saying so. `EXACT_DEV_PLAN` in the environment always wins. */
function devPlan(app) {
  if (process.env.EXACT_DEV_PLAN) return { path: process.env.EXACT_DEV_PLAN, why: 'EXACT_DEV_PLAN' };
  const dist = resolve(ROOT, 'host/web/dist');
  const plan = resolve(dist, 'app.plan');
  if (!existsSync(plan)) return { path: null, why: `no dev server has built into ${dist.replace(ROOT + '/', '')}` };
  if (!builtAppMatches(dist, app)) return { path: null, why: `${dist.replace(ROOT + '/', '')} holds another app's build` };
  return { path: plan, why: null };
}

/** `exact run` — the foreground app: its log is this terminal's, and ^C ends it. */
function run(app, files) {
  const bundle = build(app);
  const documents = files.map((f) => resolve(process.cwd(), f));
  for (const document of documents) if (!existsSync(document)) throw new Error(`no such file: ${document}`);
  const dev = devPlan(app);
  console.log(dev.path
    ? `live reload: watching ${dev.path.replace(ROOT + '/', '')} — edit ${app.name}/app.contract and this window restarts from it`
    : `live reload: off (${dev.why}). Start it with: node host/web/dev.mjs --app ${app.name}`);
  const child = spawn(executableIn(bundle), documents, {
    stdio: 'inherit',
    // Assets from the app directory, not the copy in the bundle, so editing
    // one and relaunching shows the edit.
    env: { ...process.env, EXACT_ASSETS: app.dir, ...(dev.path ? { EXACT_DEV_PLAN: dev.path } : {}) },
  });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
  child.on('exit', (code, signal) => process.exit(signal ? 1 : code ?? 0));
}

/** `exact install` — the app where the OS looks for apps, and its name where a shell looks for names. */
function install(app) {
  const bundle = build(app);
  const target = installedAt(app);
  mkdirSync(APPLICATIONS, { recursive: true });
  // A copy, not a symlink: Launch Services registers what it finds at the
  // path, and a symlinked bundle registers the repo's copy — which moves,
  // and which a `cargo clean` deletes underneath the Dock.
  rmSync(target, { recursive: true, force: true });
  const copy = spawnSync('/bin/cp', ['-R', bundle, target], { stdio: 'inherit' });
  if (copy.status !== 0) process.exit(copy.status ?? 1);
  // Register it now rather than whenever the OS next rescans, so `open -a`
  // and Open With work in the same second this returns.
  spawnSync('/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister', ['-f', target], { stdio: 'ignore' });

  const command = commandOf(app);
  const dir = binDirectory();
  mkdirSync(dir, { recursive: true });
  const shim = resolve(dir, command);
  // `open` and not the executable: it hands the files to a copy that is
  // already running instead of starting a second one, and it returns to the
  // shell instead of holding it. `--args` after the files keeps a leading
  // `-` in a filename from being read as one of open's own switches.
  writeFileSync(shim, `#!/bin/sh
# ${app.displayName} — written by \`exact install ${app.name}\`. Delete it or
# run \`exact uninstall ${app.name}\` to take it away.
exec /usr/bin/open -a ${JSON.stringify(target)} ${'"$@"'}
`);
  chmodSync(shim, 0o755);

  console.log(`${app.displayName}: ${target}`);
  console.log(`${command}: ${shim}`);
  if (!onPath(dir)) console.log(`\n  ${dir} is not on your PATH. Add it:\n    echo 'export PATH="${dir}:$PATH"' >> ~/.zshrc && exec zsh`);
  else console.log(`\n  ${command} <file> opens it — in the copy already running, if there is one.`);
}

/** `exact uninstall` — both halves, and nothing else. */
function uninstall(app) {
  const target = installedAt(app), shim = resolve(binDirectory(), commandOf(app));
  for (const path of [target, shim]) {
    if (!existsSync(path)) { console.log(`not installed: ${path}`); continue; }
    rmSync(path, { recursive: true, force: true });
    console.log(`removed ${path}`);
  }
}

/** `exact list` — every app in this repo, its command, and whether it is installed. */
function list() {
  const apps = readdirSync(resolve(ROOT, 'apps')).filter((name) => existsSync(resolve(ROOT, 'apps', name, 'app.contract')));
  const rows = apps.map((name) => { const app = resolveApp(name); return [name, commandOf(app), existsSync(installedAt(app)) ? installedAt(app).replace(homedir(), '~') : '—']; });
  const width = (i) => Math.max(...rows.map((r) => r[i].length), ['app', 'command', 'installed'][i].length);
  const line = (r) => `  ${r[0].padEnd(width(0))}  ${r[1].padEnd(width(1))}  ${r[2]}`;
  console.log(line(['app', 'command', 'installed']));
  for (const row of rows) console.log(line(row));
}

const USAGE = `exact — run an Exact app from the command line (macOS)

  exact run <app> [file …]     build and launch it here; ^C ends it
  exact install <app>          put it in ~/Applications and its name on PATH
  exact uninstall <app>        take both away
  exact list                   the apps in this repo

An app is a directory under apps/ (or EXACT_APP_DIR for one outside this repo).
EXACT_BIN_DIR names where a shim goes; the default is the first of ~/.local/bin,
/usr/local/bin, ~/bin that is already on PATH.`;

function main(argv) {
  const [verb, name, ...rest] = argv;
  if (!verb || verb === '--help' || verb === '-h' || verb === 'help') return console.log(USAGE);
  if (verb === 'list') return list();
  if (!['run', 'install', 'uninstall'].includes(verb)) { console.error(`exact: no verb ${verb}\n\n${USAGE}`); process.exit(2); }
  if (!name) { console.error(`exact ${verb}: name an app (exact list)`); process.exit(2); }
  if (process.platform !== 'darwin') { console.error(`exact ${verb} is macOS's; on Linux build the app's own executable (cargo build --release -p ${name}-linux)`); process.exit(2); }
  const app = resolveApp(name);
  if (verb === 'run') return run(app, rest);
  if (verb === 'install') return install(app);
  return uninstall(app);
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  try { main(process.argv.slice(2)); }
  catch (e) { console.error(`exact: ${e.message}`); process.exit(1); }
}
