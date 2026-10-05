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
import { accessSync, chmodSync, constants, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import { BINARYEN } from '../host/web/stages.mjs';
import { delimiter, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveApp, WEB_TOOLCHAIN, webToolchainEnv } from './app.mjs';
import { createApp } from '../game/new.mjs';
import { appleArtifacts, assertAppleIdentity, macReleaseEntitlements, stripForDistribution } from '../host/apple/build.mjs';
import { closeFilesystemReader } from './filesystem.mjs';
import { builtAppMatches, jsTargetBuild } from '../host/web/serve.mjs';
import { chromium } from './agent-launch.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const APPLICATIONS = resolve(homedir(), 'Applications');

/** The app's assembled bundle in this repo — `host/apple/build.mjs --bundle`'s one stable output. */
export const bundleOf = (app) => appleArtifacts(app).bundle;
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

/** Cheap assertion on the executable's baked identity, shared with the driver. */
function refuseForeignBundle(app, bundle) {
  assertAppleIdentity(app, executableIn(bundle));
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
  const child = spawn(executableIn(bundle), documents, {
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

/** Everything in a bundle that carries its own signature, innermost first.
 *  A bundle is sealed over its contents, so a nested library re-signed after
 *  its container invalidates the container. */
function signingOrder(bundle) {
  const inner = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir, { withFileTypes: true })) {
      const path = resolve(dir, name.name);
      if (name.isDirectory()) walk(path);
      else if (name.name.endsWith('.dylib')) inner.push(path);
    }
  };
  walk(resolve(bundle, 'Contents'));
  return [...inner, bundle];
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
  const staged = resolve(out, `${app.displayName}.app`);
  sh('/usr/bin/ditto', [bundle, staged]);
  // What ships carries no local symbols; they stay here as a dSYM (before signing: stripping changes the bytes signed).
  const symbols = stripForDistribution(resolve(staged, 'Contents/MacOS/ExactMac'), resolve(out, `${app.displayName}.dSYM`));
  console.log(`symbols: ${symbols.dsym} (${(symbols.saved / 1048576).toFixed(1)} MB off the executable)`);

  // Sign inside out, with the hardened runtime and a timestamp. Both are
  // notarisation's requirements, not preferences: a build without them is
  // rejected at submission rather than at launch. The app itself carries the
  // entitlements its `device.*` grants derive (LLP 1069.008 D4), read from the
  // bake receipt inside the bundle; the libraries inside carry none.
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
  sh('/usr/bin/ditto', [staged, resolve(image, `${app.displayName}.app`)]);
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
    const result = spawnSync(cmd, args, {cwd: ROOT, stdio: 'inherit'});
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
    run('rustup', ['toolchain', 'install', WEB_TOOLCHAIN, '--profile', 'minimal', '--component', 'rust-src', '--target', 'wasm32-unknown-unknown']);
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
  }
  const report = sdkReport();
  printReport(report);
  const missing = report.filter(row => row.required && !row.ok);
  if (missing.length) throw new Error(`${missing.map(row => row.name).join(', ')} ${missing.length === 1 ? 'is' : 'are'} missing or differ; run exact setup (it installs what it can and names the rest)`);
  console.log('SDK tools ready. Build scripts select the pinned stable/nightly without changing rustup default.');
}

/** Every prerequisite at once (LLP 1086 D6): what is installed, what the SDK
 * pins, and, for what only some apps need, which ones. A row is `required`
 * when no app here builds without it. */
export function sdkReport(env = process.env) {
  const pin = Bun.TOML.parse(readFileSync(resolve(ROOT, 'rust-toolchain.toml'), 'utf8')).toolchain;
  const bindgen = Bun.TOML.parse(readFileSync(resolve(ROOT, 'game/Cargo.toml'), 'utf8')).workspace.dependencies['wasm-bindgen'].replace(/^=/, '');
  const output = (cmd, args) => {
    const result = spawnSync(cmd, args, {cwd: ROOT, encoding: 'utf8', env});
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
  const hermes = hermesSources(env);
  rows.push(row('hermesc', hermes.hermesc, 'facebook/hermes pin', hermes.hermescOk, hermes.fix, 'TypeScript apps on native hosts; the web needs none'));
  if (process.platform === 'darwin') rows.push(row('Hermes engine', hermes.engine, 'facebook/hermes pin', hermes.engineOk, hermes.fix, 'TypeScript on macOS (iOS builds its own)'));
  return rows;
}

/** Where js/build.rs will look for Hermes, in its order: a named path; the
 * machine cache, only with no sibling ibex at all; the sibling ibex. */
export function hermesSources(env = process.env) {
  const ibex = resolve(ROOT, '../ibex'), cache = resolve(env.HOME ?? homedir(), '.cache/exact/hermes-macos');
  const fromCache = process.platform === 'darwin' && !env.EXACT_HERMES_DIR && !existsSync(ibex) && existsSync(resolve(cache, 'engine'));
  const arch = process.arch === 'arm64' ? 'arm64' : 'x64';
  const hermesc = env.EXACT_HERMESC ? resolve(env.EXACT_HERMESC)
    : process.platform === 'linux' ? resolve(ibex, `tools/hermes-vanilla/hermesc-linux-${arch}`)
    : fromCache ? resolve(cache, 'hermesc') : resolve(ibex, `tools/hermes-vanilla/hermesc-macos-${arch}`);
  const engine = env.EXACT_HERMES_DIR ? resolve(env.EXACT_HERMES_DIR) : fromCache ? resolve(cache, 'engine') : resolve(ibex, 'ios/Frameworks-vanilla');
  const receipt = !fromCache || (() => { try { return readFileSync(resolve(cache, 'engine/hermes-input-receipt.json'), 'utf8').includes('"sourceCommit"'); } catch { return false; } })();
  return {
    hermesc: existsSync(hermesc) ? hermesc : '', hermescOk: existsSync(hermesc),
    engine: existsSync(engine) ? engine : '', engineOk: existsSync(engine) && receipt,
    fix: `git clone https://github.com/expo/ibex ${ibex} && (cd ${ibex} && ./scripts/build-hermes.sh --vanilla)`,
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
  const clean = {...env};
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
function newApp(path, update) {
  if (update) return console.log(createApp(path, {update}));
  const report = sdkReport();
  if (report.some(row => row.name === 'rustup' && !row.ok)) {
    printReport(report, {onlyMissing: true});
    throw new Error('exact new needs Cargo (rustup) to resolve the new app; install it, then run exact new again');
  }
  console.log(createApp(path));
  if (report.some(row => !row.ok)) {
    console.log('\nThis machine still needs (exact setup --check shows the whole table):');
    printReport(report, {onlyMissing: true});
  }
}

const USAGE = `exact — run an Exact app from the command line (macOS)

  exact run <app> [file …]     build and launch it here; ^C ends it
  exact install <app>          put it in ~/Applications and its name on PATH
  exact release <app>          sign with a Developer ID, notarise, staple, package
  exact uninstall <app>        take both away
  exact setup [--check]        install pinned Rust, wasm-bindgen and Binaryen
  exact list                   the apps in this repo
  exact new <path> [--update]  a new app outside this repo, using this checkout;
                               --update follows a moved checkout or a new patch
  exact contract <args…>       the Contract compiler (build, types, vocab, …),
                               with paths relative to where you run it

An app is a directory under apps/ (or EXACT_APP_DIR for one outside this repo).
EXACT_BIN_DIR names where a shim goes; the default is the first of ~/.local/bin,
/usr/local/bin, ~/bin that is already on PATH.

release needs a "Developer ID Application" certificate and notarytool
credentials in the keychain; it says how to get each if one is missing.
EXACT_DEVELOPER_ID and EXACT_NOTARY_PROFILE name them explicitly.`;

function main(argv) {
  const [verb, name, ...rest] = argv;
  if (!verb || verb === '--help' || verb === '-h' || verb === 'help') return console.log(USAGE);
  if (verb === 'setup') return setup({check: name === '--check'});
  if (verb === 'list') return list();
  if (verb === 'contract') return process.exit(contract(argv.slice(1)));
  if (verb === 'new') return newApp(name, rest.includes('--update'));
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
