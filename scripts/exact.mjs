#!/usr/bin/env bun
// exact — run an Exact app from a terminal on macOS, and install one so it
// keeps running from a terminal after you close the laptop.
//
//   exact run <app> [file …]     build if stale, launch in the foreground
//   exact install <app>          a real .app in ~/Applications, plus a shim
//   exact uninstall <app>        take both away again
//   exact release <app>          sign for distribution, notarise, staple, package
//   exact list                   the apps this repo has, and what is installed
//   exact new <path> [--update]  an app outside this repo, ready to run
//   exact new <path> --game      a game outside this repo, ready to run
//
// The two things this exists to get right, because they are the two that make
// a Mac GUI app awkward from a shell:
//
//   1. **Bundle identity.** A bare Mach-O has no Info.plist, so it has no
//      name, no document types, no Dock tile, and Launch Services cannot find
//      it — `open -a` fails and Open With never lists it. `run` and `install`
//      both launch `<Name>.app/Contents/MacOS/<Name>`: the executable inside
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
import { accessSync, chmodSync, constants, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import { BINARYEN } from '../host/web/stages.mjs';
import { delimiter, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cargoEnvironment, executableName, HERMES_INSTALLER, hermesBundle, hermesTarget, resolveApp, WEB_TOOLCHAIN, webToolchainEnv } from './app.mjs';
import { createApp, createGame } from '../game/new.mjs';
import { sdkFetch } from '../game/app/shells.mjs';
import { appleArtifacts, assertAppleIdentity, macReleaseEntitlements, stripForDistribution } from '../host/apple/build.mjs';
import { closeFilesystemReader } from './filesystem.mjs';
import { builtAppMatches, jsTargetBuild } from '../host/web/serve.mjs';
import { signingOrder } from '../host/apple/assets.mjs';
import { chromium } from './agent-launch.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const APPLICATIONS = resolve(homedir(), 'Applications');

/** The app's assembled bundle in this repo — `host/apple/build.mjs --bundle`'s one stable output. */
export const bundleOf = (app) => appleArtifacts(app).bundle;
/** The executable inside a bundle: what a terminal launches to keep stdio. */
export const executableIn = (bundle, app) => resolve(bundle, 'Contents/MacOS', executableName(app));
/** The name this app answers to on the command line (`app.command`, else its directory's name). */
export const commandOf = (app) => app.manifest.app?.command ?? app.name;
/** Where `install` puts the app. */
export const installedAt = (app) => resolve(APPLICATIONS, `${executableName(app)}.app`);

/** The cross bundles a Mac's TypeScript builds use, each needed only for its
 * destination (and only with Xcode). The host bundle is the one required row. */
export function hermesCrossTargets(os = process.platform, cpu = process.arch) {
  if (os !== 'darwin') return [];
  return [
    { target: cpu === 'x64' ? 'x86_64-apple-ios' : 'aarch64-apple-ios-sim', need: 'iOS Simulator TypeScript' },
    { target: 'aarch64-apple-ios', need: 'iOS device TypeScript' },
    { target: 'aarch64-apple-tvos-sim', need: 'tvOS Simulator TypeScript' },
  ];
}

/** The simulator bundles `setup` installs after the host's (device bundles are
 * fetched on demand: `host/apple/build.mjs --device` names the command). */
export const hermesSetupTargets = (os = process.platform, cpu = process.arch) =>
  hermesCrossTargets(os, cpu).map(entry => entry.target).filter(target => target !== 'aarch64-apple-ios');

// Locked always: the installer's own lock is committed, and a check must
// neither rewrite it nor fetch crates (`--offline`); HERMES_LEAN_SYS_OFFLINE
// only constrains the Hermes resolver, not Cargo.
const hermesInstallerArguments = (check, targets = []) => [
  'run', '--locked', ...(check ? ['--offline'] : []), '--manifest-path', HERMES_INSTALLER, '--',
  ...(check ? ['--check'] : []), ...targets.flatMap(target => ['--target', target]),
];

/** Ask the vendored resolver itself whether the host bundle (and `targets`, each
 * validated against the host compiler) is installed and valid. */
export function checkHermesBundles(env = process.env, execute = spawnSync, os = process.platform, cpu = process.arch, targets = []) {
  const host = hermesTarget(os, cpu);
  if (!host) return { ok: false, message: `no pinned bundle exists for ${os}/${cpu}`, fix: '' };
  const args = hermesInstallerArguments(true, targets);
  const result = execute('cargo', args, { cwd: ROOT, encoding: 'utf8', env: cargoEnvironment(env) });
  const stderr = result.stderr?.trim().split(/\r?\n/).filter(Boolean).at(-1);
  const message = [result.stdout?.trim(), stderr, result.error?.message].filter(Boolean).join('\n');
  const install = ['cargo', ...hermesInstallerArguments(false, targets)].join(' ');
  return { ok: result.status === 0, message: message || `cargo exited ${result.status ?? 'without a status'}`, fix: install };
}

/** Where a shim goes: `EXACT_BIN_DIR`, else the first of these already on PATH, else `~/.local/bin` (made, and named in the advice). */
export function binDirectory() {
  if (process.env.EXACT_BIN_DIR) return resolve(process.env.EXACT_BIN_DIR);
  const path = (process.env.PATH ?? '').split(':').map((p) => p && resolve(p));
  const candidates = [resolve(homedir(), '.local/bin'), '/usr/local/bin', resolve(homedir(), 'bin')];
  return candidates.find((dir) => path.includes(dir) && writable(dir)) ?? candidates[0];
}

const writable = (dir) => { try { accessSync(dir, constants.W_OK); return true; } catch { return false; } };
const onPath = (dir) => (process.env.PATH ?? '').split(':').some((p) => p && resolve(p) === dir);

/** Cheap assertion on the executable's baked identity, shared with the driver. */
function refuseForeignBundle(app, bundle) {
  assertAppleIdentity(app, executableIn(bundle, app));
}

/** Build the app's macOS bundle. Cargo and SwiftPM decide what is stale; this always asks them. */
function build(app, { quiet = false, distribution = false } = {}) {
  const r = spawnSync(process.execPath, [resolve(ROOT, 'host/apple/build.mjs'), app.crate('apple'), '--bundle', ...(distribution ? ['--distribution'] : [])], {
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
async function devPlan(app) {
  if (process.env.EXACT_DEV_PLAN) return { path: process.env.EXACT_DEV_PLAN, why: 'EXACT_DEV_PLAN' };
  const dist = resolve(ROOT, 'host/web/dist');
  const plan = resolve(dist, 'app.plan');
  if (!existsSync(plan)) return { path: null, why: `no dev server has built into ${dist.replace(ROOT + '/', '')}` };
  if (!await builtAppMatches(dist, app).finally(closeFilesystemReader)) return { path: null, why: `${dist.replace(ROOT + '/', '')} holds another app's build` };
  if (jsTargetBuild(dist)) return { path: null, why: `the dev server runs ${app.name} on the JS target; a native window live-reloads from the wasm loop (bun host/web/dev.mjs --app ${app.name} --wasm)` };
  return { path: plan, why: null };
}

/** `exact run` — the foreground app: its log is this terminal's, and ^C ends it. */
async function run(app, files) {
  const bundle = build(app);
  const documents = files.map((f) => resolve(process.cwd(), f));
  for (const document of documents) if (!existsSync(document)) throw new Error(`no such file: ${document}`);
  const dev = await devPlan(app);
  console.log(dev.path
    ? `live reload: watching ${dev.path.replace(ROOT + '/', '')} — edit ${app.name}/app.contract and this window restarts from it`
    : `live reload: off (${dev.why}). Start it with: bun host/web/dev.mjs --app ${app.name}`);
  const child = spawn(executableIn(bundle, app), documents, {
    stdio: 'inherit',
    // Use the merged shader/asset generation captured by this bake.
    env: { ...process.env, EXACT_ASSETS: appleArtifacts(app).capture, ...(dev.path ? { EXACT_DEV_PLAN: dev.path } : {}) },
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

/** A tool this verb depends on, run to completion. A failure here is the
 *  end of the verb: half a signed bundle is worse than none. */
function sh(cmd, args) {
  const r = spawnSync(cmd, args, { stdio: 'inherit' });
  if (r.status !== 0) throw new Error(`${cmd} ${args[0]} failed (exit ${r.status ?? 'signal'})`);
  return r;
}

/** The identity a distributed build must be signed with.
 *
 * Not the same certificate a local build uses. "Apple Development" signs
 * something you run on your own machines; Apple will not notarise it, and an
 * un-notarised app is refused on a machine that downloaded it. Only
 * "Developer ID Application" is for distribution outside the App Store. */
function developerID() {
  if (process.env.EXACT_DEVELOPER_ID) return process.env.EXACT_DEVELOPER_ID;
  const found = spawnSync('security', ['find-identity', '-v', '-p', 'codesigning'], { encoding: 'utf8' }).stdout ?? '';
  return /\b([0-9A-F]{40})\s+"Developer ID Application: /.exec(found)?.[1] ?? null;
}

/** `exact release` — the build a teammate can actually open.
 *
 * Three things separate this from `install`, and all three are required by
 * the next one: a Developer ID signature, the hardened runtime with a secure
 * timestamp, and Apple's notarisation stapled to the artifact. Skip any and
 * macOS refuses the app on a machine that downloaded it — which is what the
 * unsigned build's "cannot be opened because the developer cannot be
 * verified" is.
 *
 * Credentials are never arguments here. `notarytool` keeps them in the
 * keychain (`xcrun notarytool store-credentials`), and this passes the
 * profile's name; an app-specific password on a command line ends up in the
 * shell history and the process table. */
function release(app) {
  const identity = developerID();
  if (!identity) {
    throw new Error(`no "Developer ID Application" certificate is on this Mac, and notarisation needs one.
  An "Apple Development" certificate is not it — Apple will not notarise a build signed with one.
  Get it from https://developer.apple.com/account/resources/certificates (a paid Apple Developer
  account), download it, and open it once so it lands in the login keychain. Then run this again.
  EXACT_DEVELOPER_ID=<sha1> names one explicitly.`);
  }
  const profile = process.env.EXACT_NOTARY_PROFILE ?? 'exact-notary';
  // The whole-module Swift host and the receipt a shipped bundle carries (host/apple/build.mjs).
  const bundle = build(app, { distribution: true });
  const out = resolve(app.target, 'dist', app.name);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const staged = resolve(out, `${executableName(app)}.app`);
  sh('/usr/bin/ditto', [bundle, staged]);
  // What ships carries no local symbols; they stay here as a dSYM (before signing: stripping changes the bytes signed).
  const symbols = stripForDistribution(executableIn(staged, app), resolve(out, `${app.displayName}.dSYM`));
  console.log(`symbols: ${symbols.dsym} (${(symbols.saved / 1048576).toFixed(1)} MB off the executable)`);

  // Sign inside out, with the hardened runtime and a timestamp. Both are
  // notarisation's requirements, not preferences: a build without them is
  // rejected at submission rather than at launch. The app itself carries the
  // entitlements its `device.*` grants derive (LLP 1069.008 D4), read from the
  // bake receipt inside the bundle; the code nested inside carries none.
  const built = JSON.parse(readFileSync(resolve(bundle, 'Contents/Resources/receipt.json'), 'utf8'));
  const entitled = macReleaseEntitlements(built.build?.compat);
  const entitlements = resolve(out, 'entitlements.plist');
  if (entitled) writeFileSync(entitlements, entitled);
  for (const path of signingOrder(staged)) {
    sh('codesign', ['--force', '--sign', identity, '--options', 'runtime', '--timestamp',
      ...(path === staged ? ['--identifier', app.id, ...(entitled ? ['--entitlements', entitlements] : [])] : []), path]);
  }
  sh('codesign', ['--verify', '--deep', '--strict', '--verbose=1', staged]);

  // A zip is what notarytool takes for an app; ditto and not zip, which
  // mangles the bundle's symlinks and its signature.
  const zip = resolve(out, `${app.displayName}.zip`);
  sh('/usr/bin/ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', staged, zip]);

  console.log(`notarising ${app.displayName} — Apple's turn, usually a minute or two`);
  // Both streams: notarytool reports progress on stdout and every failure —
  // including a missing credential profile — on stderr.
  const submit = spawnSync('xcrun', ['notarytool', 'submit', zip, '--keychain-profile', profile, '--wait'], { encoding: 'utf8' });
  const said = `${submit.stdout ?? ''}${submit.stderr ?? ''}`;
  process.stdout.write(said);
  const id = /id: ([0-9a-f-]{36})/.exec(said)?.[1];
  if (submit.status !== 0 || !/status: Accepted/.test(said)) {
    if (/Keychain (profile|password item)/i.test(said)) {
      throw new Error(`no notarytool credentials named "${profile}". Store them once:
  xcrun notarytool store-credentials ${profile} --apple-id <your-apple-id> --team-id <TEAMID> --password <app-specific-password>
  The password is an app-specific one from https://account.apple.com, not your Apple ID password.
  EXACT_NOTARY_PROFILE names a different profile.`);
    }
    if (id) {
      console.error(`\nwhat Apple objected to (submission ${id}):`);
      spawnSync('xcrun', ['notarytool', 'log', id, '--keychain-profile', profile], { stdio: 'inherit' });
    }
    throw new Error('notarisation did not come back Accepted; nothing was stapled');
  }

  // Staple the ticket into the artifacts, so they open on a machine that is
  // offline or that Apple's service cannot be reached from.
  sh('xcrun', ['stapler', 'staple', staged]);
  const dmg = resolve(out, `${app.displayName}.dmg`);
  const image = mkdtempSync(resolve(out, '.dmg-'));
  sh('/usr/bin/ditto', [staged, resolve(image, `${executableName(app)}.app`)]);
  symlinkSync('/Applications', resolve(image, 'Applications'));
  sh('hdiutil', ['create', '-volname', app.displayName, '-srcfolder', image, '-ov', '-format', 'UDZO', '-quiet', dmg]);
  rmSync(image, { recursive: true, force: true });
  sh('xcrun', ['stapler', 'staple', dmg]);
  // The zip carries no ticket of its own; rebuild it from the stapled app.
  rmSync(zip, { force: true });
  sh('/usr/bin/ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', staged, zip]);

  // What a teammate's Mac will decide, asked here rather than discovered
  // there. `spctl` is the same assessment Gatekeeper makes.
  const assess = spawnSync('spctl', ['--assess', '--type', 'execute', '--verbose=2', staged], { encoding: 'utf8' });
  const verdict = `${assess.stdout ?? ''}${assess.stderr ?? ''}`.trim();
  console.log(`\n${app.displayName} ${assess.status === 0 ? 'is accepted by Gatekeeper' : 'is NOT accepted by Gatekeeper'}: ${verdict}`);
  console.log(`  ${dmg}`);
  console.log(`  ${zip}`);
  if (assess.status !== 0) throw new Error('the artifacts were built but Gatekeeper refuses them; do not send these');
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

/** Binaryen release archives append their tag; package-manager builds may omit it. */
export function binaryenVersion(output) {
  return /^wasm-opt (version \d+)(?: \([^)]*\))?$/.exec(output.trim())?.[1] ?? output.trim();
}

export function binaryenArchive(version, os = process.platform, cpu = process.arch) {
  const platform = {darwin:'macos',linux:'linux',win32:'windows'}[os];
  const arch = cpu === 'x64' ? 'x86_64' : cpu === 'arm64' && os !== 'win32'
    ? (os === 'linux' ? 'aarch64' : 'arm64') : null;
  if (!platform || !arch) throw new Error(`no Binaryen setup for ${os}/${cpu}`);
  return `binaryen-${version}-${arch}-${platform}.tar.gz`;
}

/** Install the versions declared by the SDK, once per machine. */
export function setup({check = false} = {}) {
  const pin = Bun.TOML.parse(readFileSync(resolve(ROOT, 'rust-toolchain.toml'), 'utf8')).toolchain;
  const bindgen = Bun.TOML.parse(readFileSync(resolve(ROOT, 'game/Cargo.toml'), 'utf8')).workspace.dependencies['wasm-bindgen'].replace(/^=/, '');
  const version = BINARYEN.replace(' ', '_');
  const binaryen = resolve(homedir(), '.cache/exact/binaryen', version);
  const run = (cmd, args) => {
    console.log([cmd, ...args].join(' '));
    const result = spawnSync(cmd, args, {cwd: ROOT, stdio: 'inherit', env: cargoEnvironment()});
    if (result.status !== 0) throw new Error(`${cmd} failed: ${result.error?.message ?? result.status}`);
  };
  const output = (cmd, args) => {
    const result = spawnSync(cmd, args, {cwd: ROOT, encoding: 'utf8'});
    return result.status === 0 ? result.stdout.trim() : '';
  };
  if (!check && !output('rustup', ['--version'])) throw new Error('install rustup from https://rustup.rs, then rerun exact setup');
  if (!check) {
    run('rustup', ['toolchain', 'install', pin.channel, '--profile', pin.profile,
      ...pin.components.flatMap(c => ['--component', c]), ...pin.targets.flatMap(t => ['--target', t])]);
    // The nightly builds the web's wasm: with -Zbuild-std from rust-src for size, and without it (a --wasm dev build) from its own wasm32 std.
    // A game's web bake lints its logic on the same nightly, so it takes clippy too.
    run('rustup', ['toolchain', 'install', WEB_TOOLCHAIN, '--profile', 'minimal', '--component', 'rust-src', '--component', 'clippy', '--target', 'wasm32-unknown-unknown']);
    webToolchainEnv(process.env); // Fetch the nightly standard library's locked sources too.
    if (output('wasm-bindgen', ['--version']) !== `wasm-bindgen ${bindgen}`)
      run('cargo', [`+${pin.channel}`, 'install', 'wasm-bindgen-cli', '--version', bindgen, '--locked', '--force']);
    if (binaryenVersion(output('wasm-opt', ['--version'])) !== BINARYEN) {
      const name = binaryenArchive(version);
      const url = `https://github.com/WebAssembly/binaryen/releases/download/${version}/${name}`;
      const stage = mkdtempSync(resolve(tmpdir(), 'exact-binaryen-'));
      try {
        const archive = resolve(stage, name), sum = `${archive}.sha256`;
        run('curl', ['-fL', '--retry', '3', '-o', archive, url]);
        run('curl', ['-fL', '--retry', '3', '-o', sum, `${url}.sha256`]);
        const expected = readFileSync(sum, 'utf8').trim().split(/\s+/)[0];
        if (createHash('sha256').update(readFileSync(archive)).digest('hex') !== expected) throw new Error('Binaryen checksum mismatch');
        mkdirSync(binaryen, {recursive: true});
        run('tar', ['-xzf', archive, '--strip-components=1', '-C', binaryen]);
        process.env.PATH = `${resolve(binaryen, 'bin')}${delimiter}${process.env.PATH ?? ''}`;
      } finally { rmSync(stage, {recursive: true, force: true}); }
    }
    run(process.execPath, ['install', '--frozen-lockfile']);
    if (!hermesTarget()) throw new Error(`the pinned Hermes release has no host bundle for ${process.platform}/${process.arch}`);
    run('cargo', hermesInstallerArguments(false));
    // Simulator bundles separately: a failed cross download must not undo the
    // host install every build needs; `setup --check` reports it on its own row.
    const cross = hermesSetupTargets();
    if (cross.length && spawnSync('cargo', hermesInstallerArguments(false, cross), {cwd: ROOT, stdio: 'inherit', env: cargoEnvironment(process.env)}).status !== 0) {
      console.error(`exact setup: the ${cross.join(', ')} Hermes bundles did not install; Apple simulator TypeScript builds will name the command`);
    }
    // Every bake resolves offline and locked: the checkout's crates, and the game SDK's for a game's shell.
    run('cargo', [`+${pin.channel}`, 'fetch', '--locked', '--manifest-path', resolve(ROOT, 'Cargo.toml')]);
    console.log('cargo fetch (the game SDK lock, game/app/shells.lock)');
    const fetched = sdkFetch();
    if (!fetched.ok) throw new Error(`cargo fetch of the game SDK lock failed${fetched.message ? `: ${fetched.message}` : ''}`);
  }
  const report = sdkReport();
  printReport(report);
  const missing = report.filter(row => row.required && !row.ok);
  if (missing.length) throw new Error(`${missing.map(row => row.name).join(', ')} ${missing.length === 1 ? 'is' : 'are'} missing or differ; run exact setup (it installs what it can and names the rest)`);
  // "Ready" is said only of what is: a row some apps need is named with the apps it holds back.
  const needed = report.filter(row => !row.ok);
  console.log(needed.length
    ? `SDK tools ready, except for ${needed.map(row => `${row.need} (${row.name})`).join('; ')}. Build scripts select the pinned stable/nightly without changing rustup default.`
    : 'SDK tools ready. Build scripts select the pinned stable/nightly without changing rustup default.');
}

/** Every prerequisite at once (LLP 1086 D6): what is installed, what the SDK
 * pins, and, for what only some apps need, which ones. A row is `required`
 * when no app here builds without it. */
export function sdkReport(env = process.env) {
  const pin = Bun.TOML.parse(readFileSync(resolve(ROOT, 'rust-toolchain.toml'), 'utf8')).toolchain;
  const bindgen = Bun.TOML.parse(readFileSync(resolve(ROOT, 'game/Cargo.toml'), 'utf8')).workspace.dependencies['wasm-bindgen'].replace(/^=/, '');
  const output = (cmd, args) => {
    const result = spawnSync(cmd, args, {cwd: ROOT, encoding: 'utf8', env: cmd === 'cargo' ? cargoEnvironment(env) : env});
    return result.status === 0 ? result.stdout.trim() : '';
  };
  const row = (name, have, want, ok, fix, need = null) => ({name, have: have || 'missing', want, ok, fix, required: !need, need});
  const rows = [];
  const rustup = output('rustup', ['--version']);
  rows.push(row('rustup', rustup.split(' ')[1], 'any', !!rustup, 'install rustup from https://rustup.rs'));
  const rust = output('rustc', [`+${pin.channel}`, '--version']).split(' ')[1];
  rows.push(row('Rust', rust, pin.channel, rust === pin.channel, 'exact setup'));
  const nightlyRoot = output('rustc', [`+${WEB_TOOLCHAIN}`, '--print', 'sysroot']);
  const nightly = !!nightlyRoot && existsSync(resolve(nightlyRoot, 'lib/rustlib/src/rust/library/Cargo.toml'));
  rows.push(row('web nightly', nightlyRoot ? (nightly ? WEB_TOOLCHAIN : 'no rust-src') : '', `${WEB_TOOLCHAIN} + rust-src`, nightly, 'exact setup'));
  // An app's web build makes its own std (-Zbuild-std); a GPU module's and a --wasm dev build use the prebuilt one.
  const nightlyWasm = !!nightlyRoot && existsSync(resolve(nightlyRoot, 'lib/rustlib/wasm32-unknown-unknown'));
  rows.push(row('web nightly wasm32', nightlyWasm ? 'installed' : '', `${WEB_TOOLCHAIN} wasm32-unknown-unknown`, nightlyWasm, 'exact setup', 'GPU modules and --wasm builds'));
  // A game's web bake runs the determinism lints under the nightly (game/app/shells.mjs lintGame).
  const nightlyClippy = !!nightlyRoot && output('cargo', [`+${WEB_TOOLCHAIN}`, 'clippy', '--version']) !== '';
  rows.push(row('web nightly clippy', nightlyClippy ? 'installed' : '', `${WEB_TOOLCHAIN} clippy`, nightlyClippy, 'exact setup', "games' web builds"));
  // A game's shell resolves offline against the SDK lock; a crate missing from Cargo's cache stops its first bake.
  const crates = sdkFetch({offline: true, env});
  rows.push(row('game SDK crates', crates.ok ? 'in Cargo cache' : (/no matching package named `[^`]+`/.exec(crates.message)?.[0] ?? 'not all fetched'), 'game/app/shells.lock', crates.ok, 'exact setup', 'games'));
  const bun = JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')).packageManager.slice(4);
  rows.push(row('Bun', process.versions.bun, bun, process.versions.bun === bun, `use Bun ${bun} (README, Quick start)`));
  const bindgenHave = output('wasm-bindgen', ['--version']);
  rows.push(row('wasm-bindgen', bindgenHave, `wasm-bindgen ${bindgen}`, bindgenHave === `wasm-bindgen ${bindgen}`, 'exact setup'));
  const opt = binaryenVersion(output('wasm-opt', ['--version']));
  rows.push(row('wasm-opt', opt, BINARYEN, opt === BINARYEN, 'exact setup'));
  const modules = existsSync(resolve(ROOT, 'node_modules/.bin/rolldown'));
  rows.push(row('node_modules', modules ? 'installed' : '', 'bun.lock', modules, `bun install --frozen-lockfile in ${ROOT}`));
  const browser = chromium(env);
  rows.push(row('Chrome', browser.unavailable ? '' : browser.executable, 'any', !browser.unavailable, 'install Google Chrome, or set CHROME'));
  if (process.platform === 'darwin') {
    const developer = output('xcode-select', ['-p']);
    const xcode = /\.app\/Contents\/Developer$/.test(developer);
    rows.push(row('Xcode', developer, 'Xcode.app', xcode, 'install Xcode, then sudo xcode-select -s /Applications/Xcode.app', 'macOS and iOS'));
  }
  const host = checkHermesBundles(env);
  rows.push(row(
    `Hermes bundle (${hermesTarget() ?? 'no host bundle'})`,
    host.ok ? 'verified by hermes-lean-sys' : host.message,
    'the pinned Ibex release', host.ok, host.fix,
    // Only a native host's TypeScript data sources run Hermes; the web runs them in the browser
    // (builders read a bare MISSING here as a blocker for a web-only app: bench diaries, 2026-10-07).
    'TypeScript data sources on native hosts (macOS, iOS, Linux); not the web',
  ));
  for (const {target, need} of hermesCrossTargets()) {
    const cross = checkHermesBundles(env, spawnSync, process.platform, process.arch, [target]);
    rows.push(row(`Hermes bundle (${target})`, cross.ok ? 'verified by hermes-lean-sys' : 'not installed or invalid',
      'the pinned Ibex release', cross.ok, cross.fix, need));
  }
  return rows;
}

/** The install-once, digest-addressed Ibex bundle selected by hermes-lean-sys. */
export function hermesSources(env = process.env) {
  const bundle = hermesBundle(hermesTarget(), env);
  return {
    hermesc: existsSync(bundle.hermesc) ? bundle.hermesc : '', hermescOk: existsSync(bundle.hermesc),
    engine: bundle.installed ? bundle.root : '', engineOk: bundle.installed,
    fix: bundle.fix,
  };
}

/** The table, then one line per thing to do. */
export function printReport(rows, {onlyMissing = false} = {}) {
  const shown = onlyMissing ? rows.filter(row => !row.ok) : rows;
  for (const row of shown) {
    const mark = row.ok ? 'ok     ' : row.required ? 'MISSING' : 'needed ';
    console.log(`${mark} ${row.name}: ${row.have} (SDK ${row.want})${row.need ? ` — for ${row.need}` : ''}${row.ok ? '' : `\n          ${row.fix}`}`);
  }
}

/** `exact contract …` — this checkout's compiler, from anywhere (LLP 1086 D4).
 * Debug, as the guides use it; the pinned toolchain whatever rustup would pick
 * from the caller's directory; exact2's own target/, never an inherited one
 * (an app's, or another checkout's). */
export function contract(args, env = process.env) {
  const pin = Bun.TOML.parse(readFileSync(resolve(ROOT, 'rust-toolchain.toml'), 'utf8')).toolchain.channel;
  const clean = cargoEnvironment(env);
  delete clean.RUSTUP_TOOLCHAIN;
  delete clean.CARGO_TARGET_DIR;
  const binary = resolve(ROOT, 'target/debug', process.platform === 'win32' ? 'contract.exe' : 'contract');
  if (!existsSync(binary)) console.error('exact contract: building the Contract compiler once (a few minutes on a cold checkout)…');
  const result = spawnSync('cargo', [`+${pin}`, 'run', '-q', '--manifest-path', resolve(ROOT, 'Cargo.toml'), '-p', 'contract', '--', ...args], {stdio: 'inherit', env: clean});
  if (result.error) throw new Error(`cargo: ${result.error.message} (install rustup from https://rustup.rs)`);
  return result.status ?? 1;
}

/** `exact new`: check the machine first, since a missing Cargo fails the
 * scaffold itself; then write the app and say what its builds will still need. */
function newApp(path, {update = false, game = false, assets = false} = {}) {
  if (update) {
    console.log(createApp(path, {update}));
    // A file names what it uses (LLP 1091 D1): an app written before module
    // scope gets the `use` lines its files lack, as `contract fmt --uses` writes them.
    if (contract(['fmt', '--uses', resolve(path, 'app.contract')]) !== 0)
      console.log('The `use` lines above that no rule writes are the author\'s; `bun exact.mjs contract build app.contract` names them.');
    return;
  }
  if (game && !path) throw new Error('Usage: exact new <path> --game [--assets] (the last part names the game: lowercase-hyphenated, no host suffix)');
  const report = sdkReport();
  if (report.some(row => row.name === 'rustup' && !row.ok)) {
    printReport(report, {onlyMissing: true});
    throw new Error('exact new needs Cargo (rustup) to resolve the new app; install it, then run exact new again');
  }
  // A game is a path here, as an app is: `game/new.mjs` alone takes a bare name for game/games.
  console.log(game ? createGame(resolve(path), undefined, {assets}) : createApp(path));
  if (report.some(row => !row.ok)) {
    // Builders read a bare "still needs" list as the reason a later command failed (bench diaries,
    // 2026-10-07/08): say when nothing on it blocks the app.
    const blocking = report.some(row => !row.ok && row.required);
    console.log(blocking
      ? '\nThis machine still needs (exact setup --check shows the whole table):'
      : '\nThe app was created; nothing below blocks it. Each is needed only for what it names (exact setup --check shows the whole table):');
    printReport(report, {onlyMissing: true});
  }
}

const USAGE = `exact — run an Exact app from the command line (macOS)

  exact run <app> [file …]     build and launch it here; ^C ends it
  exact install <app>          put it in ~/Applications and its name on PATH
  exact release <app>          sign with a Developer ID, notarise, staple, package
  exact uninstall <app>        take both away
  exact setup [--check]        install pinned Rust, wasm-bindgen, Binaryen and
                               this machine's host/iOS/tvOS Hermes bundles;
                               fetch the crates every bake reads offline
  exact hatch <word> [<app>]   an access hatch: a stub for each target the app builds
                               (--app, --window for those scopes), and app.json's entry
  exact list                   the apps in this repo
  exact new <path> [--update]  a new app outside this repo, using this checkout;
                               --update follows a moved checkout or a new patch
  exact new <path> --game      a new game (game/README.md): a Rust world under
                               Contract's menus, with the same exact.mjs verbs;
                               --assets declares game.assets
  exact contract <args…>       the Contract compiler (build, types, vocab, …),
                               with paths relative to where you run it

An app is a directory under apps/ (or EXACT_APP_DIR for one outside this repo).
EXACT_BIN_DIR names where a shim goes; the default is the first of ~/.local/bin,
/usr/local/bin, ~/bin that is already on PATH.

release needs a "Developer ID Application" certificate and notarytool
credentials in the keychain; it says how to get each if one is missing.
EXACT_DEVELOPER_ID and EXACT_NOTARY_PROFILE name them explicitly.`;

/** `exact hatch <word> [<app>]`, `exact hatch --app [<app>]`, `exact hatch --window [<app>]` (LLP 1075.003.000.001 §5):
 *  an access hatch's stub for each target the app builds. A word is added to app.json `hatches` with the platforms
 *  stubs were written for; each stub has the hatch's moments, an end that undoes what built added, a region and one
 *  diagnostics line, and says what a hatched node gives up there. An app with no module gets each platform's. The
 *  files are the app's from then on. The app is `<app>`, or `EXACT_APP_DIR`'s (an app `exact new` made). */
export function hatch(args, say = console.log) {
  const scope = args.includes('--app') ? 'app' : args.includes('--window') ? 'window' : 'element';
  const names = args.filter(a => !a.startsWith('--')), word = scope === 'element' ? names.shift() : null;
  if (scope === 'element' && !/^[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*$/.test(word ?? '')) throw new Error('hatch: name a word, lowercase letters and digits joined by "-" (exact hatch avatar), or --app, or --window');
  const app = resolveApp(names[0]), dir = app.dir, manifestPath = resolve(dir, 'app.json');
  const manifest = existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, 'utf8')) : {};
  const title = manifest.app?.name ?? manifest.name ?? app.name;
  const camel = (word ?? scope).replace(/-([a-z0-9])/g, (_, c) => c.toUpperCase()), Camel = camel[0].toUpperCase() + camel.slice(1);
  const wrote = [], todo = [];
  const write = (path, text) => { if (existsSync(path)) return false; mkdirSync(resolve(path, '..'), { recursive: true }); writeFileSync(path, text); wrote.push(path); return true; };
  // A line added above a marker in a file this verb wrote; in a module of the app's own there is none, and the line is shown.
  const mark = (path, marker, line, why) => {
    const text = readFileSync(path, 'utf8'), at = text.indexOf(marker);
    if (text.includes(line.trim())) return;
    if (at < 0) return todo.push(`${path}: ${why}\n    ${line.trim()}`);
    const start = text.lastIndexOf('\n', at) + 1;
    writeFileSync(path, text.slice(0, start) + line + '\n' + text.slice(start));
  };
  const platforms = [];
  if (existsSync(resolve(dir, 'apple'))) {
    platforms.push(...['ios', 'macos'].filter(p => !manifest.host || manifest.host[p]));
    const mod = resolve(dir, 'modules/apple'), has = existsSync(mod) && readdirSync(mod).some(f => f.endsWith('.swift'));
    const main = has ? readdirSync(mod).map(f => resolve(mod, f)).find(f => readFileSync(f, 'utf8').includes('// exact:element')) ?? resolve(mod, readdirSync(mod).find(f => f.endsWith('.swift'))) : resolve(mod, 'Hatches.swift');
    if (!has) write(main, `// ${title}'s native module on Apple: its access hatches. Written by \`exact hatch\`; yours from here.
// A hatch is handed the platform object Exact built (a view, a window), configures it, and changes Contract
// state only by acting on authored nodes: click(), focus(), blur(), input(text). The app works without it
// (EXACT_HATCHES=off in a development build). \`perf hatches\`, \`state\` and \`logs\` show what each one did.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

final class AppHatches: ExactModule {
    override func element(_ element: ExactElement) {
        // exact:element (exact hatch adds a word's line above this one)
    }

    override func elementEnded(_ element: ExactElement) {
        // exact:elementEnded
    }

    override func app(_ app: ExactApp) {
        // exact:app
    }

    override func appEnded(_ app: ExactApp) {
        // exact:appEnded
    }

    override func window(_ window: ExactWindow) {
        // exact:window
    }

    override func windowEnded(_ window: ExactWindow) {
        // exact:windowEnded
    }
}

let exactModule: ExactModule.Type = AppHatches.self
`);
    const stub = resolve(mod, `${Camel}Hatch.swift`);
    if (scope === 'element') {
      write(stub, `// The \`${word}\` hatch on Apple: a node marked hatch="${word}" hands this code its view. Yours to edit.
// What it gives up: on iOS a hatched node is a view of its own, not a flat leaf, and its list row is not
// reused unless the hatch sets \`element.reusable\` and undoes what it adds. On macOS, nothing beyond the call.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Built (\`element.isNew\`) and changed (a \`data-*\` word moved): configure what Exact made, above Exact's own.
func ${camel}Hatch(_ element: ExactElement) {
    guard let view = element.view else { return }
    if element.isNew {
        // Say what you add, bound to it, so the agent's tree shows whose it is:
        // element.owns(view: added, "what it is and why")
        element.diagnostics.log("built on \\(type(of: view))")
    }
}

/// The node is leaving, its view still there: take back everything built added.
func ${camel}HatchEnded(_ element: ExactElement) {
}
`);
      mark(main, '// exact:elementEnded', `        if element.hatch == .${camel} { ${camel}HatchEnded(element) }`, 'call it from your module\'s elementEnded(_:)');
      mark(main, '// exact:element ', `        if element.hatch == .${camel} { ${camel}Hatch(element) }`, 'call it from your module\'s element(_:)');
    } else {
      const type = scope === 'app' ? 'ExactApp' : 'ExactWindow';
      write(stub, `// The ${scope} hatch on Apple (LLP 1075.003.000.001 §2.1). Yours to edit.
// ${scope === 'app' ? '`app.application` is nil unless the embedder gave this session the process; `app.prefersColorScheme` and the other facts are the ones Contract sees.' : '`window.window` is nil unless the window is this session\'s own (`window.exclusive`); its frame and moments are always there.'}
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// When the hatches connect (\`${scope}.isNew\`), and when ${scope === 'app' ? 'a fact changes' : 'its size or safe area changes'}.
func ${scope}Hatch(_ ${scope}: ${type}) {
    ${scope === 'app' ? '// app.owns(appearance: "what you set app-wide")' : '// window.owns(recognizer: added, "what it does", surface: true)'}
}

/// ${scope === 'app' ? 'The session is ending or reloading' : 'The session is leaving the window'}: undo what the hatch set.
func ${scope}HatchEnded(_ ${scope}: ${type}) {
}
`);
      mark(main, `// exact:${scope}Ended`, `        ${scope}HatchEnded(${scope})`, `call it from your module's ${scope}Ended(_:)`);
      mark(main, `// exact:${scope}\n`, `        ${scope}Hatch(${scope})`, `call it from your module's ${scope}(_:)`);
    }
  }
  if (existsSync(resolve(dir, 'web'))) {
    platforms.push('web');
    const mod = resolve(dir, 'modules/web'), main = resolve(mod, 'index.js');
    write(main, `// ${title}'s page module: its access hatches on the web. Written by \`exact hatch\`; yours from here.
// A hatch is handed the DOM element Exact built, configures it, and changes Contract state only by acting on
// authored nodes: e.click(), e.focus(), e.blur(), e.input(text). The app works without it (?hatches=off on a
// development page). \`perf hatches\`, \`state\` and \`logs\` show what each one did.
const hatches = {};
// exact:hatches (exact hatch adds a word's line above this one)

export function element(e) { hatches[e.hatch]?.element?.(e); }
export function elementEnded(e) { hatches[e.hatch]?.elementEnded?.(e); }
`);
    const file = `hatch-${word ?? scope}.js`;
    if (scope === 'element') {
      write(resolve(mod, file), `// The \`${word}\` hatch on the web: a node marked hatch="${word}" hands this code its element. Yours to edit.
// What it gives up: nothing beyond the call. Leave the node's own box, position and children to Exact.

/** Built (\`e.isNew\`) and changed (a \`data-*\` word moved, in \`e.data\`). */
export function element(e) {
  if (e.isNew) {
    // Say what you add, bound to it, so the agent's tree shows whose it is:
    // e.owns(added, 'what it is and why');
    e.diagnostics.log(\`built on <\${e.element.localName}>\`);
  }
}

/** The node is leaving: take back everything built added. */
export function elementEnded(e) {
}
`);
      mark(main, '// exact:hatches', `import * as ${camel}Hatch from './${file}';\nhatches[${JSON.stringify(word)}] = ${camel}Hatch;`, 'import it and call its element and elementEnded from yours');
    } else {
      write(resolve(mod, file), `// The ${scope} hatch on the web (LLP 1075.003.000.001 §2.1). Yours to edit.
// ${scope === 'app' ? 'The page is the app\'s own: its facts (`a.prefersColorScheme`, `a.visibilityState`, …) are the ones Contract sees.' : 'The page\'s window; `w.frame` is its inner size. Reach the global as `globalThis` in a file that exports `window`.'}

/** When the page module connects (\`isNew\`), and when ${scope === 'app' ? 'a fact changes' : 'its size changes'}. */
export function built(${scope[0]}) {
}

/** The page is going: undo what the hatch set. */
export function ended(${scope[0]}) {
}
`);
      mark(main, '// exact:hatches', `import * as ${scope}Hatch from './${file}';\nexport function ${scope}(x) { ${scope}Hatch.built(x); }\nexport function ${scope}Ended(x) { ${scope}Hatch.ended(x); }`, `import it and export ${scope} and ${scope}Ended from yours`);
    }
  }
  if (existsSync(resolve(dir, 'linux'))) {
    // Linux, and Windows and Android over the same presenter: one file the app's Linux crate includes (its
    // build.rs, as apps/native-fixture/linux/build.rs: contract::native::rust_hatch_entry).
    platforms.push('linux');
    const main = resolve(dir, 'modules/linux/hatches.rs'), snake = (word ?? scope).replaceAll('-', '_');
    if (write(main, `// ${title}'s hatches on a painting host (Linux, and Windows over the same presenter). Written by
// \`exact hatch\`; yours from here. There is no platform object here: a hatch draws into its node's overlay
// (a whole recording that replaces the last), observes the input that lands in its box, and changes Contract
// state only by acting on authored nodes: click(), focus(), blur(), input(text).
use exact_linux::hatches::{App, Context, Element, Hatches, Window};

#[derive(Default)]
pub struct AppHatches;

impl Hatches for AppHatches {
    fn element(&mut self, element: &Element, context: &mut Context<'_, Self>) {
        match element.hatch() {
            // exact:element (exact hatch adds a word's arm above this one)
            _ => {}
        }
        let _ = context;
    }

    fn element_ended(&mut self, element: &Element, context: &mut Context<'_, Self>) {
        match element.hatch() {
            // exact:elementEnded
            _ => {}
        }
        let _ = context;
    }

    fn app(&mut self, app: &App, context: &mut Context<'_, Self>) {
        // exact:app
        let _ = (app, context);
    }

    fn app_ended(&mut self, app: &App, context: &mut Context<'_, Self>) {
        // exact:appEnded
        let _ = (app, context);
    }

    fn window(&mut self, window: &Window, context: &mut Context<'_, Self>) {
        // exact:window
        let _ = (window, context);
    }

    fn window_ended(&mut self, window: &Window, context: &mut Context<'_, Self>) {
        // exact:windowEnded
        let _ = (window, context);
    }
}

pub type ExactHatches = AppHatches;

// exact:functions (exact hatch adds a word's functions above this line)
`)) {
      // The crate's build script includes the file. One that asks for its hatches only has to run again (a new mtime).
      const build = resolve(dir, 'linux/build.rs'), text = existsSync(build) ? readFileSync(build, 'utf8') : '';
      if (/\brust_hatch(es|_entry)\b/.test(text)) writeFileSync(build, text);
      else todo.push(`${build}: include the hatches in the app's Linux crate, as apps/native-fixture/linux/build.rs does\n    contract::native::rust_hatch_entry(&app_dir, &manifest, &target) gives the entry's text and the run call its main makes`);
    }
    if (scope === 'element') {
      mark(main, '// exact:functions', `/// The \`${word}\` hatch: built (\`element.is_new()\`) and changed (a \`data-*\` word or the box's size moved).
/// What it gives up here: nothing beyond the call.
fn ${snake}_hatch<H: Hatches>(element: &Element, _context: &mut Context<'_, H>) {
    if element.is_new() {
        // element.overlay().draw(|c, w, h| { c.set_fill_style_str("#f59e0b"); c.fill_rect(0.0, 0.0, w, h); });
        element.diagnostics().log("built");
    }
}

/// The node has left: stop what the hatch started for it.
fn ${snake}_hatch_ended<H: Hatches>(_element: &Element, _context: &mut Context<'_, H>) {}
`, 'add its functions to your hatches file');
      mark(main, '// exact:elementEnded', `            ${JSON.stringify(word)} => ${snake}_hatch_ended(element, context),`, 'call it from your element_ended');
      mark(main, '// exact:element ', `            ${JSON.stringify(word)} => ${snake}_hatch(element, context),`, 'call it from your element');
    }
  }
  if (!platforms.length) throw new Error(`hatch: ${app.name} builds no target a hatch can be written for (apple/, web/ or linux/)`);
  if (scope === 'element') {
    // The declaration (§5): the word, with the platforms a stub was written for. A list means every platform.
    const had = manifest.hatches;
    if (Array.isArray(had)) { if (!had.includes(word)) had.push(word); }
    else manifest.hatches = { ...(had ?? {}), [word]: [...new Set([...(had?.[word] ?? []), ...platforms])] };
    writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
  }
  say(`${scope === 'element' ? `hatch="${word}"` : `the ${scope} hatch`} for ${platforms.join(', ')}${wrote.length ? `:\n  wrote ${wrote.map(f => f.slice(dir.length + 1)).join('\n  wrote ')}` : ': its files were already there'}`);
  if (scope === 'element') say(`  app.json hatches: ${JSON.stringify(manifest.hatches)}\n  mark a node in app.contract: column hatch="${word}"`);
  for (const line of todo) say(`  to do, in ${line}`);
  return { wrote, todo, platforms };
}

function main(argv) {
  const [verb, name, ...rest] = argv;
  if (!verb || verb === '--help' || verb === '-h' || verb === 'help') return console.log(USAGE);
  if (verb === 'setup') return setup({check: name === '--check'});
  if (verb === 'list') return list();
  if (verb === 'contract') return process.exit(contract(argv.slice(1)));
  if (verb === 'hatch') return hatch(argv.slice(1));
  if (verb === 'new') {
    // Flags may come before the path: `exact new --game ./my-game`.
    const args = argv.slice(1), path = args.find(arg => !arg.startsWith('--'));
    return newApp(path, {update: args.includes('--update'), game: args.includes('--game'), assets: args.includes('--assets')});
  }
  if (!['run', 'install', 'uninstall', 'release'].includes(verb)) { console.error(`exact: no verb ${verb}\n\n${USAGE}`); process.exit(2); }
  if (!name) { console.error(`exact ${verb}: name an app (exact list)`); process.exit(2); }
  if (process.platform !== 'darwin') { console.error(`exact ${verb} is macOS's; on Linux build the app's own executable (cargo build --profile host-dev -p ${name}-linux to drive it, --release to ship it)`); process.exit(2); }
  const app = resolveApp(name);
  if (verb === 'run') return run(app, rest);
  // `--release` on `install` is the same path, since that is what it is for.
  if (verb === 'release' || (verb === 'install' && rest.includes('--release'))) return release(app);
  if (verb === 'install') return install(app);
  return uninstall(app);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { await main(process.argv.slice(2)); }
  catch (e) { console.error(`exact: ${e.message}`); process.exit(1); }
}
