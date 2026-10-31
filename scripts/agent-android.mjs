// The Android carrier (LLP 1107): an app's Linux host built for a phone's ABI
// (`aarch64-linux-android`, the host's Android-gated code, bionic, the phone's
// own fonts) and run headless under `adb shell`, where the agent protocol is
// the Linux host's own (LLP 1012 over stdio, `host/linux/src/agent.rs`). It
// drives the app's runner, kernel and painter on Android; it is not the Canvas
// reader (LLP 1076 §3.3, `host/android`), which paints through its own Kotlin
// view. `--reader` drives that one instead (LLP 1107 D1): the APK, installed
// and launched, answering the same protocol in its own process
// (`host/linux/src/canvas/agent.rs`) over a socket adb forwards.
//
//   bun scripts/agent-android.mjs build <app>              build the app's Linux crate for Android
//   bun scripts/agent.mjs android --app <app> …            drive it on the first adb device or emulator
//   bun scripts/agent-android.mjs build <app> --apk        the app on the Canvas reader, as an APK (host/android/README.md;
//                                                          --release: the release profile, for measuring;
//                                                          --write-locks: write the lock and checksums of the
//                                                          Maven libraries the app's modules name;
//                                                          --install: then onto the device; --run: and launch it)
//   bun scripts/agent.mjs android --reader --app <app> …   drive that APK (EXACT_ANDROID_BIN names another)
//
// An `exact new` app's `bun exact.mjs android` is `build <app> --apk --install` (`--headless`: the Linux host), and its
// `agent android` and `test android` drive the APK.
//
// An app with a TypeScript data module also needs the Android Hermes bundle:
// `bun scripts/hermes-android.mjs build` once (scripts/hermes-android.mjs).
//
// The NDK comes from ANDROID_NDK_HOME, else the newest under the SDK's ndk/
// (ANDROID_HOME, else ~/Library/Android/sdk or ~/Android/Sdk); ANDROID_SERIAL
// picks a device. Fonts are the phone's (/system/fonts) unless the drive names
// EXACT_FONTS. The painter is the CPU one unless EXACT_PAINTER says
// otherwise: the GPU painter needs a real GPU's Vulkan (an emulator's
// SwiftShader crashes it).
import { writeTamberNames } from './tamber-names.mjs';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createConnection } from 'node:net';
import { homedir } from 'node:os';
import { basename, dirname, resolve } from 'node:path';
import { HOST_DEV, bakeWatchEnv, buildBake, cargoEnvironment, copyShaders, injectedProfiles, linuxBuild, resolveApp, wholeProgram } from './app.mjs';
import { hermesAndroidBundle } from './hermes-android.mjs';
import { guardFlags } from './help.mjs';
import { androidImplementations, packageConfig } from '../js/bake/src/typescript.mjs';
import { bakedSettings } from '../js/bake/src/__exact_env.mjs';
import { configMetaData, fragmentPermissions, manifestFragment, withMetaData } from './android-manifest.mjs';
import { androidShareActivity } from './share-target.mjs';

export const ANDROID_TARGET = 'aarch64-linux-android';
/** The oldest Android the binary runs on (the NDK's clang wrapper names it). */
const API = 29;
/** What a drive runs: the app's Linux host built for the phone, or with `--reader` (EXACT_ANDROID_READER) its APK. */
export const androidBinary = (app, bin) => process.env.EXACT_ANDROID_READER ? readerApk(app)
  : resolve(app.target, ANDROID_TARGET, HOST_DEV, bin ?? app.crate('linux'));
export const androidBuild = (app, bin = null) => [...linuxBuild(app, bin), '--target', ANDROID_TARGET];

export const androidSdk = (env = process.env) => env.ANDROID_HOME ?? env.ANDROID_SDK_ROOT
  ?? resolve(homedir(), process.platform === 'darwin' ? 'Library/Android/sdk' : 'Android/Sdk');
const adb = (env = process.env) => {
  const tool = resolve(androidSdk(env), 'platform-tools/adb');
  return existsSync(tool) ? tool : 'adb';
};

/** The environment `androidBuild` needs: the NDK's clang as the target's linker and C compiler. */
export function androidToolchainEnv(env = process.env) {
  let ndk = env.ANDROID_NDK_HOME;
  if (!ndk) {
    const root = resolve(androidSdk(env), 'ndk');
    const versions = existsSync(root) ? readdirSync(root).filter((v) => /^\d/.test(v)).sort((a, b) => a.localeCompare(b, undefined, { numeric: true })) : [];
    if (!versions.length) throw new Error(`no Android NDK: install one with the SDK manager (ndk;27.1.12297006), or set ANDROID_NDK_HOME (looked in ${root})`);
    ndk = resolve(root, versions.at(-1));
  }
  const bin = resolve(ndk, 'toolchains/llvm/prebuilt', process.platform === 'darwin' ? 'darwin-x86_64' : 'linux-x86_64', 'bin');
  const cc = resolve(bin, `aarch64-linux-android${API}-clang`);
  if (!existsSync(cc)) throw new Error(`the NDK at ${ndk} has no ${basename(cc)}`);
  // A TypeScript data module's lean Hermes VM: the Android bundle
  // `hermes-android.mjs build` made, for the target only (the host keeps its pinned one).
  const hermes = env.HERMES_LEAN_SYS_DIR_aarch64_linux_android ?? hermesAndroidBundle();
  return { ...cargoEnvironment(env), ANDROID_NDK_HOME: ndk, CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER: cc, CC_aarch64_linux_android: cc,
    CXX_aarch64_linux_android: `${cc}++`, AR_aarch64_linux_android: resolve(bin, 'llvm-ar'),
    // One C++ runtime, static, as the Hermes bundle links it (ANDROID_STL=c++_static):
    // the cc crate's Android default, c++_shared, needs libc++_shared.so beside the binary.
    CXXSTDLIB_aarch64_linux_android: 'c++_static',
    ...(hermes ? { HERMES_LEAN_SYS_DIR_aarch64_linux_android: hermes } : {}) };
}

/** The device a drive uses: ANDROID_SERIAL, else the one adb lists as ready. With several and none named it refuses to
 * choose: another lane's emulator or a person's phone may be among them. */
export function androidDevice(env = process.env) {
  if (env.ANDROID_SERIAL) return env.ANDROID_SERIAL;
  const listed = spawnSync(adb(env), ['devices'], { encoding: 'utf8' });
  if (listed.error) throw new Error(`adb: ${listed.error.message} (install the SDK's platform-tools, or set ANDROID_HOME)`);
  const ready = listed.stdout.split('\n').map((l) => l.split('\t')).filter(([, s]) => s?.trim() === 'device').map(([id]) => id);
  if (!ready.length) throw new Error('no Android device or emulator is ready: start one (`emulator -avd <name>`), `adb connect <host:port>` one elsewhere, or attach a phone with USB debugging on');
  if (ready.length > 1) throw new Error(`${ready.length} Android devices are ready (${ready.join(', ')}): set ANDROID_SERIAL to the one to use`);
  return ready[0];
}

const quote = (s) => `'${String(s).replace(/'/g, "'\\''")}'`;

/** Put the binary, the app's assets (and a local `EXACT_PLAN`) in a directory of this drive's own on the
 * device and return how to run it there: `spawn(args)` the process (stdio is the agent's), the device
 * path its screenshots are written to, and `pull(path)` to copy one here. HOME is the app's, kept
 * across drives so a named scratch store (`--storage`) survives, as on the desktop carriers; a run
 * directory is this drive's, so two drives of one app do not share a binary or a screenshot. Only
 * EXACT_* variables cross, with the device's own HOME, assets, native-module directory and fonts. */
export function androidDeploy(app, bin, env) {
  if (bin.endsWith('.apk')) return readerDeploy(app, bin, env);
  const serial = androidDevice(env), tool = adb(env);
  const base = `/data/local/tmp/exact/${app.id}`, home = `${base}/home`;
  const dir = `${base}/run-${process.pid}-${Date.now().toString(36)}`, exe = basename(bin);
  const run = (args, what) => {
    const r = spawnSync(tool, ['-s', serial, ...args], { encoding: 'utf8' });
    if (r.status !== 0) throw new Error(`adb ${what}: ${(r.stderr || r.stdout || r.error?.message || '').trim()}`);
  };
  // A drive removes its own run directory when its process ends (below); one a killed adb orphaned
  // goes after a day. HOME stays.
  run(['shell', `mkdir -p ${quote(home)} ${quote(dir)} && find ${quote(base)} -maxdepth 1 -name 'run-*' -mmin +1440 -exec rm -rf {} + ; true`], 'prepare');
  run(['push', bin, `${dir}/${exe}`], 'push the binary');
  run(['shell', `chmod 755 ${quote(`${dir}/${exe}`)}`], 'chmod');
  if (existsSync(resolve(app.dir, 'assets'))) run(['push', resolve(app.dir, 'assets'), `${dir}/`], 'push the assets');
  const vars = Object.fromEntries(Object.entries(env).filter(([k]) => k.startsWith('EXACT_')));
  // A local plan (`--plan`) is read on the device: copied there with the replacement Rust module the host
  // reads beside it (`rust/`, host/linux/src/delivery.rs), or the baked plan or logic would run instead.
  if (vars.EXACT_PLAN && existsSync(vars.EXACT_PLAN)) {
    const plan = resolve(vars.EXACT_PLAN), rust = resolve(dirname(plan), 'rust');
    run(['push', plan, `${dir}/${basename(plan)}`], 'push the plan');
    if (existsSync(rust)) run(['push', rust, `${dir}/`], 'push the plan\'s Rust module');
    vars.EXACT_PLAN = `${dir}/${basename(plan)}`;
  }
  Object.assign(vars, { HOME: home, EXACT_ASSETS: dir, EXACT_NATIVE_LIBS: dir, EXACT_PAINTER: env.EXACT_PAINTER ?? 'cpu' });
  const assignments = Object.entries(vars).map(([k, v]) => `${k}=${quote(v)}`).join(' ');
  const shot = `${dir}/agent-shot.png`;
  return {
    serial, dir, shot,
    spawn: (args = []) => spawn(tool, ['-s', serial, 'shell', '-T',
      `cd ${quote(dir)} && env ${assignments} ${[`./${exe}`, ...args].map(quote).join(' ')}; s=$?; cd / && rm -rf ${quote(dir)}; exit $s`], { stdio: ['pipe', 'pipe', 'pipe'] }),
    pull(path) {
      const r = spawnSync(tool, ['-s', serial, 'pull', shot, resolve(path)], { encoding: 'utf8' });
      if (r.status !== 0) throw new Error(`adb pull the screenshot: ${(r.stderr || r.stdout).trim()}`);
    },
  };
}

/** The app's `kind` crate's Cargo.toml, beside app.json or in the workspace generated for it (scripts/app-shells.mjs). */
const crateManifest = (app, kind) => [resolve(app.dir, kind, 'Cargo.toml'), resolve(app.workspace, kind, 'Cargo.toml')].find(existsSync);

/** The Gradle project that holds the Canvas reader and the shell that packages one app with it. */
const READER = resolve(import.meta.dir, '../host/android');
/** Where one app's Gradle build writes: the shell's build (its staged library and assets, its APK), the reader's, and
 * Gradle's project cache and Kotlin's session files. An app of this checkout's keeps the shell's under
 * host/android/app/build/<name>; an app outside it builds under its own target directory. No two apps share any of it,
 * so builds of two apps at once neither mix outputs nor wait on one project's locks. */
export function apkDirs(app) {
  const own = !app.dir.startsWith(resolve(READER, '../..') + '/'), root = resolve(app.target, 'android');
  return own ? { app: resolve(root, 'app'), canvas: resolve(root, 'canvas'), cache: resolve(root, 'gradle'), kotlin: resolve(root, 'kotlin') }
    : { app: resolve(READER, 'app/build', app.name), canvas: resolve(READER, 'canvas/build', app.name), cache: resolve(READER, '.gradle/apps', app.name), kotlin: resolve(READER, '.kotlin/apps', app.name) };
}
/** The APK `build <app> --apk` makes. */
const readerApk = (app) => resolve(apkDirs(app).app, 'outputs/apk/release/app-release.apk');
/** What `build --apk` records beside the APK, for a drive to tell whether it is older than its sources: when the build
 * began, the library it packaged (whose Cargo dep-info names the Rust, the plan's and the module's sources), and the
 * app's Kotlin. Null without one. */
export function apkRecord(app, apk = readerApk(app)) {
  try { return JSON.parse(readFileSync(resolve(dirname(apk), 'exact-build.json'), 'utf8')); } catch { return null; }
}
/** The reader's own sources, which every APK compiles. */
export const READER_SOURCES = ['canvas/src', 'app/src'].map((d) => resolve(READER, d));

/** The reader carrier (LLP 1107 D1): install the APK and return what `androidDeploy` returns, with a process that
 * launches the app under a driver and relays the agent protocol's lines between its stdio and the app (`relay`).
 * The package is the app's id with the suffix the APK was built with (ORG_GRADLE_PROJECT_idSuffix, as the build
 * reads it). The app boots its own baked plan with its own assets, at the phone's size, in the phone's fonts, or a
 * drive's local `--plan`, pushed to the device and named by the launch extra `x_plan` (EXACT_PLAN, which the reader
 * reads as the Linux host does). A drive's `--world`, `--size` and launch URL do not reach it, and all but the size
 * are refused. A dev loop's `--url` does: the reader polls it (LLP 1122 D4), through `adb reverse` when it is this
 * Mac's loopback. */
function readerDeploy(app, apk, env) {
  for (const [name, flag] of [['EXACT_WORLD', '--world'], ['EXACT_LAUNCH_URL', 'a launch URL']])
    if (env[name]) throw new Error(`android --reader: ${flag} does not reach the reader, which boots the plan its APK bakes`);
  if (env.EXACT_PLAN && (!existsSync(env.EXACT_PLAN) || URL.parse(env.EXACT_PLAN)?.protocol?.startsWith('http')))
    throw new Error('android --reader: --plan is a local plan file, which the drive pushes to the phone');
  // A dev loop's URL (`--url`, LLP 1122 D4): the reader polls it, as the Linux display does, through
  // `adb reverse` when it is this Mac's loopback; a file path is no URL a phone reads.
  const devUrl = env.EXACT_DEV_PLAN ? URL.parse(env.EXACT_DEV_PLAN) : null;
  if (env.EXACT_DEV_PLAN && !['http:', 'https:'].includes(devUrl?.protocol)) throw new Error('android --reader: a development plan is a dev loop\'s http(s) URL (--url), not a file');
  if (devUrl) env = { ...env, EXACT_DEV_PLAN: devUrl.href };
  const serial = androidDevice(env), tool = adb(env), id = app.id.replaceAll('-', '_') + (env.ORG_GRADLE_PROJECT_idSuffix ?? ''); // as the Gradle shell names the package
  const dir = `/data/local/tmp/exact/${id}`, shot = `${dir}/reader-shot-${process.pid}.png`;
  const run = (args, what) => {
    const r = spawnSync(tool, ['-s', serial, ...args], { encoding: 'utf8' });
    if (r.status !== 0 || /^Failure/m.test(r.stdout)) throw new Error(`adb ${what}: ${(r.stderr || r.stdout || r.error?.message || '').trim()}`);
  };
  // An APK the device already holds, byte for byte, is not installed again: an install is tens of seconds on an emulator.
  const local = createHash('sha256').update(readFileSync(apk)).digest('hex');
  const held = spawnSync(tool, ['-s', serial, 'shell', `p=$(pm path ${quote(id)} 2>/dev/null | head -n 1); [ -n "$p" ] && sha256sum "\${p#package:}"`], { encoding: 'utf8' }).stdout?.split(/\s/)[0];
  if (held !== local) run(['install', '-r', apk], `install ${apk}`);
  run(['shell', `mkdir -p ${quote(dir)}`], 'prepare');
  // The app reads the plan from the shell's directory, which it may enter but not list.
  if (env.EXACT_PLAN) {
    const plan = `${dir}/plan-${process.pid}.plan`;
    run(['push', env.EXACT_PLAN, plan], 'push the plan');
    run(['shell', `chmod 644 ${quote(plan)}`], 'chmod the plan');
    env = { ...env, EXACT_PLAN: plan };
  }
  if (devUrl && ['127.0.0.1', 'localhost', '[::1]'].includes(devUrl.hostname)) {
    const port = devUrl.port || (devUrl.protocol === 'https:' ? '443' : '80');
    run(['reverse', `tcp:${port}`, `tcp:${port}`], `reverse tcp:${port}`);
  }
  // What the app reads of the drive: EXACT_AGENT and every EXACT_AGENT_* (the store, the epoch, the locale, the
  // seed, the fetch faults), EXACT_STORE (`real`: the platform's stores, a conformance suite's), a pushed plan and a
  // dev loop's URL, which reach it as the reader's `x_<name>` launch extras.
  const vars = Object.fromEntries(Object.entries(env).filter(([k]) => k === 'EXACT_AGENT' || k.startsWith('EXACT_AGENT_') || ['EXACT_STORE', 'EXACT_DEV_PLAN', 'EXACT_PLAN'].includes(k)));
  return {
    serial, dir, shot,
    spawn: () => spawn(process.execPath, [import.meta.path, 'relay', serial, id, JSON.stringify(vars)], { env, stdio: ['pipe', 'pipe', 'pipe'] }),
    pull(path) {
      const r = spawnSync(tool, ['-s', serial, 'pull', shot, resolve(path)], { encoding: 'utf8' });
      if (r.status !== 0) throw new Error(`adb pull the screenshot: ${(r.stderr || r.stdout).trim()}`);
      spawnSync(tool, ['-s', serial, 'shell', `rm -f ${quote(shot)}`]);
    },
  };
}

/** How long after an operation's answer a screenshot is taken: the reader draws what the operation painted over
 * the frames that follow, and a window of the top layer comes with its own animation. */
const SHOT_SETTLE_MS = 400;

/** The reader carrier's process: launch the app with a driver's socket (the shell's `agent=<name>` extra), have
 * adb forward it, and carry the protocol's lines between this process's stdio and the app. Every line is the app's
 * to answer but `screenshot`, whose picture is the device's (`screencap`), tagged as the app tags a reply. The
 * app's own stderr (logcat's `exact` tag) is this process's. Ends when stdin does, and the app with it. */
async function relay([serial, id, vars]) {
  const tool = adb(), sh = (...args) => spawnSync(tool, ['-s', serial, ...args], { encoding: 'utf8' });
  const name = `exact-agent-${process.pid}`;
  // EXACT_AGENT_INTENT: `am start` words the launch carries besides the drive's, for a drive that starts the app
  // as another app would (a share's `-a android.intent.action.SEND …`, LLP 1133.024 D5).
  const { EXACT_AGENT_INTENT: intent = '', ...launch } = JSON.parse(vars);
  const extras = Object.entries(launch).flatMap(([k, v]) => ['--es', `x_${k.slice('EXACT_'.length).toLowerCase()}`, quote(v)]);
  sh('shell', 'am', 'force-stop', id);
  sh('logcat', '-c');
  // `lower=0`: the engine samples every animation, so none plays on the reader's own clock.
  const started = sh('shell', ['am', 'start', '-W', '-n', `${id}/dev.exact.app.MainActivity`, '--es', 'agent', name, '--es', 'lower', '0', ...extras, intent].join(' '));
  if (started.status !== 0 || /Error/.test(started.stdout + started.stderr)) throw new Error(`am start ${id}: ${(started.stderr || started.stdout).trim()}`);
  const port = Number(sh('forward', 'tcp:0', `localabstract:${name}`).stdout.trim());
  if (!port) throw new Error('adb forward gave no port');
  const logs = spawn(tool, ['-s', serial, 'logcat', '-v', 'raw', 'exact:V', 'AndroidRuntime:E', '*:S'], { stdio: ['ignore', 'pipe', 'ignore'] });
  logs.stdout.pipe(process.stderr);
  // A dev loop's port the deploy reversed (`readerDeploy`) goes with the drive.
  const dev = URL.parse(JSON.parse(vars).EXACT_DEV_PLAN ?? '');
  const reversed = dev && ['127.0.0.1', 'localhost', '[::1]'].includes(dev.hostname) ? dev.port || (dev.protocol === 'https:' ? '443' : '80') : null;
  const done = (code) => { logs.kill(); sh('forward', '--remove', `tcp:${port}`); if (reversed) sh('reverse', '--remove', `tcp:${reversed}`); sh('shell', 'am', 'force-stop', id); process.exit(code); };
  // The app opens its socket as its activity is made; the forward connects when it is there.
  let socket;
  for (let tries = 0; !socket; tries++) {
    socket = await new Promise((r) => { const c = createConnection(port, '127.0.0.1', () => r(c)); c.once('error', () => r(null)); });
    if (!socket && tries > 50) throw new Error(`the reader opened no agent socket (${name}): is the APK built from a tree that has AgentDrive?`);
    if (!socket) await new Promise((r) => setTimeout(r, 100));
  }
  // Lines from the app, in order: each goes to whoever waits for one.
  const waiting = [], ready = [];
  let rest = '';
  socket.setEncoding('utf8');
  socket.on('data', (d) => { const lines = (rest + d).split('\n'); rest = lines.pop(); for (const l of lines) (waiting.shift() ?? ((x) => ready.push(x)))(l); });
  socket.on('close', () => done(1));
  socket.on('error', () => done(1));
  const answer = () => ready.length ? Promise.resolve(ready.shift()) : new Promise((r) => waiting.push(r));
  const say = (line) => process.stdout.write(line + '\n');
  const pollsLanded = async () => {
    for (const end = Date.now() + 2000; Date.now() < end;) {
      socket.write('{"op":"polls"}\n');
      if (!JSON.parse(await answer()).asking) return;
      await new Promise((r) => setTimeout(r, 10));
    }
  };
  say(await answer()); // the app's `ready`
  let pending = '', turn = Promise.resolve();
  const one = async (line) => {
    let req = null;
    try { req = JSON.parse(line); } catch {}
    if (req?.op === 'quit') return done(0);
    if (req?.op !== 'screenshot') {
      socket.write(line + '\n');
      const reply = await answer();
      // A jump's asks of polled module sources land before its reply is given (LLP 1133.009 D10 item 7). The
      // reader's extensions answer on its main thread, which takes their asks between requests, so the wait is here.
      if (req?.op === 'clock' && !req.settle) await pollsLanded();
      return say(reply);
    }
    if (req.film) return say(JSON.stringify({ error: 'the reader carrier films nothing: its picture is a screencap of the device' }));
    await new Promise((r) => setTimeout(r, SHOT_SETTLE_MS));
    const shot = sh('shell', `screencap -p ${quote(req.path)}`);
    socket.write('{"op":"tags"}\n');
    const tags = JSON.parse(await answer());
    say(JSON.stringify(shot.status === 0 ? { screenshot: req.path, device: true, ...tags } : { error: `screencap: ${(shot.stderr || shot.stdout).trim()}` }));
  };
  process.stdin.setEncoding('utf8');
  process.stdin.on('data', (d) => { const lines = (pending + d).split('\n'); pending = lines.pop(); for (const l of lines) if (l.trim()) turn = turn.then(() => one(l)); });
  process.stdin.on('end', () => { turn.then(() => done(0)); });
}

/** The build script's output directory of `crate`, where it wrote its plan, `compat.json` and `uses.txt`
 * (`contract::native::android_entry`), found by Cargo's JSON messages. */
function planOut(messages, crate) {
  for (const line of messages.split('\n')) {
    if (!line.startsWith('{')) continue;
    const m = JSON.parse(line);
    if (m.reason === 'build-script-executed' && (m.package_id.includes(`#${crate}@`) || m.package_id.startsWith(`${crate} `))) return m.out_dir;
  }
  return null;
}

/** The capabilities the plan uses (LLP 1047 D2), one a line in `uses.txt`; none when the crate writes none. */
const planUses = (out) => out && existsSync(resolve(out, 'uses.txt')) ? readFileSync(resolve(out, 'uses.txt'), 'utf8').split('\n').filter(Boolean) : [];

/** The same, where the build ran without JSON (a bake): the crate's build script output that wrote `uses.txt` last,
 * under Cargo's `build` directory. */
function bakedOut(dir, crate) {
  const prefix = `${crate.replaceAll('-', '_')}-`, alt = `${crate}-`;
  let newest = null;
  for (const name of existsSync(dir) ? readdirSync(dir) : []) {
    if (!name.startsWith(prefix) && !name.startsWith(alt)) continue;
    const file = resolve(dir, name, 'out/uses.txt');
    if (existsSync(file) && (!newest || statSync(file).mtimeMs > newest.at)) newest = { out: resolve(dir, name, 'out'), at: statSync(file).mtimeMs };
  }
  return newest?.out ?? null;
}

/** The app on the Canvas reader, as an APK: its `android/` crate (a cdylib over `exact_linux::canvas_jni!`; a game's
 * is generated under `.shells/`) built for the phone's ABI and put, with the app's assets, where the Gradle shell
 * packages them (host/android/app), then `:app:assembleRelease`. An app with a GPU module (LLP 1009) is built as a
 * Linux bake is (`buildBake`): the module first, then the library with the module's digest baked in, so the module
 * ships beside it and must reach the device byte for byte (the Gradle shell extracts it and keeps its symbols).
 * Gradle wants a JDK (17 or newer) and the SDK's platform. The APK's path, or null. */
function buildApk(app, env, profile, writeLocks = false) {
  const crate = app.crate('android');
  if (!app.manifest.game && !crateManifest(app, 'android')) throw new Error(`${app.dir} has no android/ crate, the app's library for the Canvas host: bun exact.mjs update adds one to an app that names android in app.json (apps/shelf/android is one)`);
  const built = (name) => resolve(app.target, ANDROID_TARGET, profile, `lib${name.replaceAll('-', '_')}.so`);
  // The packages' configuration (LLP 1133.021 §9 item 7), before anything compiles: a required value that is absent
  // refuses the build by its code (`maps-key-missing` for a release without the Maps key).
  let config;
  try { config = packageConfig(app.dir, 'android', { release: profile === 'release' }); } catch (error) { console.error(`${app.name}: ${error.message}`); return null; }
  let gpu = null, out = null;
  if (app.manifest.game || app.hasGpu) {
    try { buildBake(app, 'linux', ANDROID_TARGET, { profile, env, kind: 'android' }); }
    catch (error) { console.error(error.message); return null; }
    gpu = built(app.crate('gpu'));
    out = bakedOut(resolve(app.target, ANDROID_TARGET, profile, 'build'), crate);
  } else {
    // JSON on stdout, for the crate's build-script output directory; the compiler's own messages still on stderr.
    const cargo = spawnSync('cargo', ['build', ...injectedProfiles(app), ...wholeProgram('android', profile), '--profile', profile, '-p', crate, '--target', ANDROID_TARGET, '--message-format=json-render-diagnostics'], { cwd: app.workspace ?? app.dir, env, stdio: ['inherit', 'pipe', 'inherit'], maxBuffer: 1 << 28 });
    if (cargo.status !== 0) return null;
    out = planOut(cargo.stdout.toString(), crate);
  }
  const uses = planUses(out);
  const dirs = apkDirs(app), started = new Date().toISOString(), staged = resolve(dirs.app, 'rust');
  rmSync(staged, { recursive: true, force: true });
  mkdirSync(resolve(staged, 'jniLibs/arm64-v8a'), { recursive: true });
  // The reader loads one name whatever the app; the plan names `assets/…` under the APK's `app/`.
  copyFileSync(built(crate), resolve(staged, 'jniLibs/arm64-v8a/libexactcanvas.so'));
  // The host opens the module by the name its bake recorded, in the APK's extracted library directory.
  if (gpu) copyFileSync(gpu, resolve(staged, 'jniLibs/arm64-v8a', basename(gpu)));
  if (existsSync(resolve(app.dir, 'assets'))) cpSync(resolve(app.dir, 'assets'), resolve(staged, 'assets/app/assets'), { recursive: true });
  // The module's shader text (LLP 1030 D8), which the host registers from `shaders/` under the asset root.
  if (gpu) copyShaders(app, resolve(staged, 'assets/app/shaders'));
  // A game's resolved manifest (with its `app` identity) is the one its shells were generated with.
  // A date or time control's picker is Compose Material 3's, linked only for a plan that has one (LLP 1115.001.000 D2).
  const compose = uses.includes('dates') ? ['-Pcompose=1'] : [];
  // The token names its hatches read (LLP 1121 D12), beside them in the build.
  if (existsSync(resolve(app.dir, 'modules/android'))) writeTamberNames(app, 'kotlin');
  const graph = mavenGraph(app, writeLocks);
  if (!graph) return null;
  let gradle;
  try {
    gradle = spawnSync(resolve(READER, 'gradlew'), [':app:assembleRelease', `-Papp=${app.name}`, `-PappDir=${app.manifest.game ? app.workspace : app.dir}`,
      `-PappBuildDir=${dirs.app}`, `-PcanvasBuildDir=${dirs.canvas}`, `-PndkPath=${env.ANDROID_NDK_HOME}`, '--project-cache-dir', dirs.cache, `-Pkotlin.project.persistent.dir=${dirs.kotlin}`,
      ...(gpu ? [`-PgpuModule=${basename(gpu)}`] : []), ...compose, ...extensionArgs(app, out, config), ...graph.args], { cwd: READER, env: { ...env, ANDROID_HOME: androidSdk(env) }, stdio: 'inherit' });
    if (gradle.status === 0) graph.kept();
  } finally {
    graph.done();
  }
  if (gradle.status !== 0) return null;
  const apk = readerApk(app);
  writeFileSync(resolve(dirname(apk), 'exact-build.json'), JSON.stringify({ started, library: built(crate), workspace: app.workspace ?? app.dir,
    sources: [...READER_SOURCES, ...[resolve(app.dir, 'modules/android')].filter(existsSync)] }) + '\n');
  // What its bake inlined of each setting (LLP 1153 D4), for a drive to compare with their sources (`bakedSettingChanges`).
  writeFileSync(`${apk}.settings.json`, JSON.stringify(bakedSettings(out, app.dir).settings?.env ?? {}));
  return apk;
}

/** The Maven libraries the bound modules name (LLP 1133.008 D8 item 5), as Gradle arguments: one graph with the
 * reader's own AndroidX, locked to the app's `gradle.lockfile` and verified against its `gradle-verification.xml`,
 * both beside its other locks. The verification file is Gradle's, which it reads only from the reader's `gradle/`:
 * it is put there for this build (`done` takes it away). `writeLocks` writes both from the graph as it resolves
 * (`kept` copies the verification back). null, said why, when the app names libraries but not their lock. */
function mavenGraph(app, writeLocks) {
  const maven = [...new Set(androidImplementations(app.dir).flatMap((b) => b.maven ?? []))].sort();
  const placed = resolve(READER, 'gradle/verification-metadata.xml');
  const none = { args: [], kept() {}, done() {} };
  if (!maven.length) return none;
  const lock = resolve(app.dir, 'gradle.lockfile'), checksums = resolve(app.dir, 'gradle-verification.xml');
  if (!writeLocks && !(existsSync(lock) && existsSync(checksums))) {
    console.error(`${app.name}: its modules name Maven libraries (${maven.join(', ')}), whose one Gradle graph is locked and verified: write ${basename(lock)} and ${basename(checksums)} beside the app's other locks with bun scripts/agent-android.mjs build ${app.name} --apk --write-locks, and keep them with the app`);
    return null;
  }
  rmSync(placed, { force: true });
  if (existsSync(checksums)) copyFileSync(checksums, placed);
  return {
    args: [`-Pmaven=${maven.join(',')}`, `-PmavenLock=${lock}`, ...(writeLocks ? ['--write-locks', '--write-verification-metadata', 'sha256'] : ['--dependency-verification=strict'])],
    kept() { if (writeLocks && existsSync(placed)) copyFileSync(placed, checksums); },
    done() { rmSync(placed, { force: true }); },
  };
}

/** What the modules the app uses add to the APK (LLP 1133.001 D4, D5; LLP 1133.002 D6): each bound implementation's
 * Kotlin with the roster that names them (`exact.app.ExactExtensions`, written here, never committed), and the
 * permissions the app's grant lines derive (the bake's `reach.android`) with those its names derive, and the
 * components its packages' manifest fragments declare with the meta-data `config` (`packageConfig`'s) carries, as
 * Gradle properties. */
function extensionArgs(app, out, config) {
  const bound = androidImplementations(app.dir), generated = resolve(apkDirs(app).app, 'extensions');
  rmSync(generated, { recursive: true, force: true });
  let permissions = [], services = {}, features = [];
  try {
    const reach = JSON.parse(readFileSync(resolve(out, 'compat.json'), 'utf8')).reach;
    permissions = reach?.android ?? [];
    // The reader's foreground services the lines declare, with their purposes' text (LLP 1133.033 D2).
    services = reach?.androidServices ?? {};
    // Hardware the lines use where it is present, never required (LLP 1133.037 D17: USB host mode).
    features = reach?.androidFeatures ?? [];
  } catch { /* no bake output: none */ }
  // And what the module names the app calls derive beside their lines (`android_entry`'s `permissions.txt`, LLP 1133.018 D11).
  try { permissions = [...new Set([...permissions, ...readFileSync(resolve(out, 'permissions.txt'), 'utf8').split('\n').filter(Boolean)])].sort(); } catch { /* none */ }
  // And the normal permissions the packages' manifest fragments declare (LLP 1133.031 D12 item 1).
  permissions = [...new Set([...permissions, ...fragmentPermissions(bound)])].sort();
  if (bound.length) {
    mkdirSync(resolve(generated, 'exact/app'), { recursive: true });
    writeFileSync(resolve(generated, 'exact/app/ExactExtensions.kt'), ['// Written by scripts/agent-android.mjs (LLP 1133.001 D5); never edited.', 'package exact.app', '',
      'class ExactExtensions : dev.exact.canvas.ExtensionRoster {',
      '    override val extensions: Map<String, (dev.exact.canvas.ExtensionContext) -> dev.exact.canvas.ExactExtension> = mapOf(',
      ...bound.map((b) => `        ${JSON.stringify(b.extension)} to { c -> ${b.className}(c) },`), '    )',
      // Each view a package serves, by the extension that makes it (LLP 1133.001 D1).
      `    override val views: Map<String, String> = mapOf(${bound.flatMap((b) => (b.views ?? []).map((tag) => `${JSON.stringify(tag)} to ${JSON.stringify(b.extension)}`)).join(', ')})`,
      // The extensions made after the reader's first frame, not at their first call (LLP 1133.015 D16 item 1).
      `    override val startAtLoad: List<String> = listOf(${bound.filter((b) => b.start).map((b) => JSON.stringify(b.extension)).join(', ')})`,
      '}', ''].join('\n'));
  }
  // The queries and components the packages' manifest fragments declare, under the allow-list (LLP 1133.030 D9), with
  // the app's privacy policy for an activity that shows it (LLP 1133.026 D4).
  // And the meta-data the packages' configuration carries (LLP 1133.021 §9 item 7): the Maps key, read from the
  // variable `app.json` names, never from the repo. A development build without it says so and builds.
  for (const [name, targets] of Object.entries(app.manifest?.packages ?? {})) for (const [key, value] of Object.entries(targets?.android ?? {})) {
    if (typeof value?.env === 'string' && !process.env[value.env] && config.has(name.split('/').at(-1))) console.warn(`${app.name}: ${value.env} is unset, so ${name}'s ${key} is absent from this development build (a release build refuses it)`);
  }
  // The share target's filters on the app's activity (LLP 1133.024 D5), inside the same `<application>`.
  const fragment = withMetaData(manifestFragment(bound, { privacyPolicy: app.manifest?.privacy_policy ?? null }), [...configMetaData(config), ...(androidShareActivity(app.manifest?.share_target) ?? [])]), fragmentFile = resolve(apkDirs(app).app, 'manifest-fragment.xml');
  rmSync(fragmentFile, { force: true });
  if (fragment) { mkdirSync(dirname(fragmentFile), { recursive: true }); writeFileSync(fragmentFile, fragment); }
  return [...(bound.length ? [`-PextensionDirs=${[...bound.flatMap((b) => b.dirs), generated].join(',')}`] : []), ...(permissions.length ? [`-Ppermissions=${permissions.join(',')}`] : []),
    ...(fragment ? [`-PmanifestFragment=${fragmentFile}`] : []), ...(Object.keys(services).length ? [`-PforegroundServices=${JSON.stringify(services)}`] : []),
    ...(features.length ? [`-PandroidFeatures=${features.join(',')}`] : [])];
}

/** `--install` (`--run`: and launch it): the APK onto the device `androidDevice` names. With none ready the build
 * stands and says how to install it; with several and no ANDROID_SERIAL it refuses. The exit code. */
function installApk(app, apk, { run = false, built = 0 } = {}) {
  let serial;
  try { serial = androidDevice(); } catch (error) {
    if (/^no Android device/.test(error.message)) { console.error(`built in ${(built / 1000).toFixed(1)} s; not installed: ${error.message}`); return run ? 1 : 0; }
    console.error(error.message);
    return 1;
  }
  const id = app.id.replaceAll('-', '_') + (process.env.ORG_GRADLE_PROJECT_idSuffix ?? ''), started = Date.now();
  const installed = spawnSync(adb(), ['-s', serial, 'install', '-r', apk], { encoding: 'utf8' });
  if (installed.status !== 0 || /^Failure/m.test(installed.stdout)) { console.error(`adb install: ${(installed.stderr || installed.stdout).trim()}`); return 1; }
  console.error(`built in ${(built / 1000).toFixed(1)} s, installed on ${serial} as ${id} in ${((Date.now() - started) / 1000).toFixed(1)} s`);
  if (!run) return 0;
  const launched = spawnSync(adb(), ['-s', serial, 'shell', 'am', 'start', '-W', '-n', `${id}/dev.exact.app.MainActivity`], { encoding: 'utf8' });
  if (launched.status !== 0 || /Error/.test(launched.stdout + launched.stderr)) { console.error(`am start: ${(launched.stderr || launched.stdout).trim()}`); return 1; }
  return 0;
}

/** `verify android`'s build (scripts/verify.mjs): the APK, built when it is missing or older than its sources, as a
 * drive would refuse it. Null when fresh, else what the build said (its last lines when it failed). */
export async function apkBuildFirst(app) {
  const { apkChanges } = await import('./agent-stdio.mjs'), apk = readerApk(app);
  const why = !existsSync(apk) ? 'no APK yet' : apkChanges(app, apk).length ? 'the APK is older than its sources' : null;
  if (!why) return null;
  console.error(`building the APK (${why}): bun exact.mjs android`);
  const started = Date.now(), r = spawnSync(process.execPath, [import.meta.path, 'build', app.dir, '--apk'], { encoding: 'utf8', maxBuffer: 256 << 20 });
  const lines = `${r.stdout ?? ''}${r.stderr ?? ''}`.split('\n').filter((l) => l.trim());
  return { ms: Date.now() - started, code: r.status ?? 1, said: r.status ? lines.slice(-20) : [`built the APK in ${((Date.now() - started) / 1000).toFixed(1)} s`], warnings: [] };
}

/** `verify android`'s look at a screen: the host has no `check` observation yet, so each scheme's screen is captured,
 * `unchecked`, as Windows' is (the device's pixels, shrunk to a pixel a point as check keeps an iOS capture). */
export async function androidCaptures(s, { screen, shot, failed }) {
  const [{ decodePng, encodePng, shrink }, { tmpdir }] = await Promise.all([import('./png.mjs'), import('node:os')]);
  const seen = (await s.layout().catch(() => null))?.viewport ?? {}, vp = { w: Math.round(seen.w), h: Math.round(seen.h) }, runs = [];
  for (const scheme of ['light', 'dark']) {
    await s.prefer({ 'prefers-color-scheme': scheme });
    await s.clock('settle');
    const path = shot?.(vp, scheme) ?? resolve(tmpdir(), `exact-verify-${process.pid}-${scheme}.png`);
    await s.carrier.screenshot(path);
    const png = decodePng(readFileSync(path)), scale = vp.w ? png.width / vp.w : 1;
    if (scale > 1) writeFileSync(path, encodePng(shrink(png, Math.round(png.width / scale), Math.round(png.height / scale))));
    runs.push({ screen, size: [vp.w, vp.h], scheme, asked: scheme, unchecked: true, ...(shot?.(vp, scheme) ? { shot: path } : {}), findings: [], accepted: [], lines: failed, hidden: 0, advice: [] });
  }
  await s.prefer({ 'prefers-color-scheme': 'light' });
  return runs;
}

if (import.meta.main && process.argv[2] === 'relay') {
  // The reader carrier's own process (`readerDeploy`): not a verb a person runs.
  await relay(process.argv.slice(3)).catch((e) => { console.error(e.message); process.exit(1); });
} else if (import.meta.main) {
  const args = guardFlags('android', process.argv.slice(2));
  const [verb, name] = args.filter((arg) => !arg.startsWith('--'));
  if (verb !== 'build' || !name) {
    console.error('usage: bun scripts/agent-android.mjs build <app> [--apk [--install | --run]] [--headless]   (then: bun scripts/agent.mjs android [--reader] --app <app> …; --apk: the app on the Canvas reader, host/android/README.md; --headless: the Linux host for Android, never the APK)');
    process.exit(2);
  }
  // `--headless` wins: an app's `bun exact.mjs android --headless` asks for the Linux host its verb's `--apk` would replace.
  const apkWanted = args.includes('--apk') && !args.includes('--headless');
  // Without rustup (a toolchain installed by hand) Cargo names a missing target itself.
  const rustup = spawnSync('rustup', ['target', 'list', '--installed'], { cwd: resolveApp(name).dir, encoding: 'utf8' });
  if (rustup.status === 0 && !rustup.stdout.split('\n').includes(ANDROID_TARGET)) {
    console.error(`the toolchain has no ${ANDROID_TARGET} target: rustup target add ${ANDROID_TARGET} (once, in the app's directory so its pinned toolchain gets it)`);
    process.exit(1);
  }
  // With what the bake watches named (`bakeWatchEnv`), as every baking build.
  const app = resolveApp(name), env = { ...androidToolchainEnv(), ...bakeWatchEnv(app), CARGO_TARGET_DIR: app.target };
  // A TypeScript app's library links the Android Hermes bundle: said before minutes of compiling, not at the link.
  const crateToml = crateManifest(app, apkWanted ? 'android' : 'linux');
  if (!env.HERMES_LEAN_SYS_DIR_aarch64_linux_android && crateToml && /^exact-js\s*=/m.test(readFileSync(crateToml, 'utf8'))) {
    console.error(`${app.name}'s TypeScript needs the Android Hermes bundle, which is not installed: bun scripts/hermes-android.mjs install <archive> (or copy ~/.cache/exact/hermes-android from a machine that has one; LLP 1107 §4.1)`);
    process.exit(1);
  }
  if (apkWanted) {
    const started = Date.now();
    const apk = buildApk(app, env, args.includes('--release') ? 'release' : HOST_DEV, args.includes('--write-locks'));
    if (!apk) process.exit(1);
    console.log(apk);
    if (args.includes('--install') || args.includes('--run')) process.exit(installApk(app, apk, { run: args.includes('--run'), built: Date.now() - started }));
    process.exit(0);
  }
  const [cmd, ...rest] = androidBuild(app);
  const built = spawnSync(cmd, rest, { cwd: app.workspace ?? app.dir, env, stdio: 'inherit' });
  if (built.status === 0) console.log(androidBinary(app));
  process.exit(built.status ?? 1);
}
