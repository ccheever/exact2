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
//                                                          --release: the release profile, for measuring)
//   bun scripts/agent.mjs android --reader --app <app> …   drive that APK (EXACT_ANDROID_BIN names another)
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
import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import { createConnection } from 'node:net';
import { homedir } from 'node:os';
import { basename, dirname, resolve } from 'node:path';
import { HOST_DEV, buildBake, injectedProfiles, linuxBuild, resolveApp } from './app.mjs';
import { hermesAndroidBundle } from './hermes-android.mjs';
import { guardFlags } from './help.mjs';

export const ANDROID_TARGET = 'aarch64-linux-android';
/** The oldest Android the binary runs on (the NDK's clang wrapper names it). */
const API = 30;
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
  return { ...env, CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER: cc, CC_aarch64_linux_android: cc,
    CXX_aarch64_linux_android: `${cc}++`, AR_aarch64_linux_android: resolve(bin, 'llvm-ar'),
    // One C++ runtime, static, as the Hermes bundle links it (ANDROID_STL=c++_static):
    // the cc crate's Android default, c++_shared, needs libc++_shared.so beside the binary.
    CXXSTDLIB_aarch64_linux_android: 'c++_static',
    ...(hermes ? { HERMES_LEAN_SYS_DIR_aarch64_linux_android: hermes } : {}) };
}

/** The device a drive uses: ANDROID_SERIAL, else the only (or first) one adb lists as ready. */
export function androidDevice(env = process.env) {
  if (env.ANDROID_SERIAL) return env.ANDROID_SERIAL;
  const listed = spawnSync(adb(env), ['devices'], { encoding: 'utf8' });
  if (listed.error) throw new Error(`adb: ${listed.error.message} (install the SDK's platform-tools, or set ANDROID_HOME)`);
  const ready = listed.stdout.split('\n').map((l) => l.split('\t')).filter(([, s]) => s?.trim() === 'device').map(([id]) => id);
  if (!ready.length) throw new Error('no Android device or emulator is ready: start one (`emulator -avd <name>`) or attach a phone with USB debugging on');
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

/** The Gradle project that holds the Canvas reader and the shell that packages one app with it. */
const READER = resolve(import.meta.dir, '../host/android');
/** The APK `build <app> --apk` makes. */
const readerApk = (app) => resolve(READER, 'app/build', app.name, 'outputs/apk/release/app-release.apk');

/** The reader carrier (LLP 1107 D1): install the APK and return what `androidDeploy` returns, with a process that
 * launches the app under a driver and relays the agent protocol's lines between its stdio and the app (`relay`).
 * The package is the app's id with the suffix the APK was built with (ORG_GRADLE_PROJECT_idSuffix, as the build
 * reads it). The app boots its own baked plan with its own assets, at the phone's size, in the phone's fonts: a
 * drive's `--plan`, `--world`, `--size` and launch URL do not reach it, and all but the size are refused. */
function readerDeploy(app, apk, env) {
  for (const [name, flag] of [['EXACT_PLAN', '--plan'], ['EXACT_DEV_PLAN', 'a development plan'], ['EXACT_WORLD', '--world'], ['EXACT_LAUNCH_URL', 'a launch URL']])
    if (env[name]) throw new Error(`android --reader: ${flag} does not reach the reader, which boots the plan its APK bakes`);
  const serial = androidDevice(env), tool = adb(env), id = app.id.replaceAll('-', '_') + (env.ORG_GRADLE_PROJECT_idSuffix ?? ''); // as the Gradle shell names the package
  const dir = `/data/local/tmp/exact/${id}`, shot = `${dir}/reader-shot-${process.pid}.png`;
  const run = (args, what) => {
    const r = spawnSync(tool, ['-s', serial, ...args], { encoding: 'utf8' });
    if (r.status !== 0 || /^Failure/m.test(r.stdout)) throw new Error(`adb ${what}: ${(r.stderr || r.stdout || r.error?.message || '').trim()}`);
  };
  run(['install', '-r', apk], `install ${apk}`);
  run(['shell', `mkdir -p ${quote(dir)}`], 'prepare');
  // What the app reads of the drive: EXACT_AGENT and every EXACT_AGENT_* (the store, the epoch, the locale, the
  // seed, the fetch faults), which reach it as the reader's `x_<name>` launch extras.
  const vars = Object.fromEntries(Object.entries(env).filter(([k]) => k === 'EXACT_AGENT' || k.startsWith('EXACT_AGENT_')));
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
  const extras = Object.entries(JSON.parse(vars)).flatMap(([k, v]) => ['--es', `x_${k.slice('EXACT_'.length).toLowerCase()}`, quote(v)]);
  sh('shell', 'am', 'force-stop', id);
  sh('logcat', '-c');
  // `lower=0`: the engine samples every animation, so none plays on the reader's own clock.
  const started = sh('shell', ['am', 'start', '-W', '-n', `${id}/dev.exact.app.MainActivity`, '--es', 'agent', name, '--es', 'lower', '0', ...extras].join(' '));
  if (started.status !== 0 || /Error/.test(started.stdout + started.stderr)) throw new Error(`am start ${id}: ${(started.stderr || started.stdout).trim()}`);
  const port = Number(sh('forward', 'tcp:0', `localabstract:${name}`).stdout.trim());
  if (!port) throw new Error('adb forward gave no port');
  const logs = spawn(tool, ['-s', serial, 'logcat', '-v', 'raw', 'exact:V', 'AndroidRuntime:E', '*:S'], { stdio: ['ignore', 'pipe', 'ignore'] });
  logs.stdout.pipe(process.stderr);
  const done = (code) => { logs.kill(); sh('forward', '--remove', `tcp:${port}`); sh('shell', 'am', 'force-stop', id); process.exit(code); };
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
  say(await answer()); // the app's `ready`
  let pending = '', turn = Promise.resolve();
  const one = async (line) => {
    let req = null;
    try { req = JSON.parse(line); } catch {}
    if (req?.op === 'quit') return done(0);
    if (req?.op !== 'screenshot') { socket.write(line + '\n'); return say(await answer()); }
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

/** The app on the Canvas reader, as an APK: its `android/` crate (a cdylib over `exact_linux::canvas_jni!`; a game's
 * is generated under `.shells/`) built for the phone's ABI and put, with the app's assets, where the Gradle shell
 * packages them (host/android/app), then `:app:assembleRelease`. An app with a GPU module (LLP 1009) is built as a
 * Linux bake is (`buildBake`): the module first, then the library with the module's digest baked in, so the module
 * ships beside it and must reach the device byte for byte (the Gradle shell extracts it and keeps its symbols).
 * Gradle wants a JDK (17 or newer) and the SDK's platform. The APK's path, or null. */
function buildApk(app, env, profile) {
  const crate = app.crate('android');
  if (!app.manifest.game && !existsSync(resolve(app.dir, 'android/Cargo.toml'))) throw new Error(`${app.dir} has no android/ crate: the app's library for the Canvas host (apps/caltrain/android is one)`);
  const built = (name) => resolve(app.target, ANDROID_TARGET, profile, `lib${name.replaceAll('-', '_')}.so`);
  let gpu = null;
  if (app.manifest.game || app.hasGpu) {
    try { buildBake(app, 'linux', ANDROID_TARGET, { profile, env, kind: 'android' }); }
    catch (error) { console.error(error.message); return null; }
    gpu = built(app.crate('gpu'));
  } else {
    const cargo = spawnSync('cargo', ['build', ...injectedProfiles(app), '--profile', profile, '-p', crate, '--target', ANDROID_TARGET], { cwd: app.workspace ?? app.dir, env, stdio: 'inherit' });
    if (cargo.status !== 0) return null;
  }
  const staged = resolve(READER, 'app/build', app.name, 'rust');
  rmSync(staged, { recursive: true, force: true });
  mkdirSync(resolve(staged, 'jniLibs/arm64-v8a'), { recursive: true });
  // The reader loads one name whatever the app; the plan names `assets/…` under the APK's `app/`.
  copyFileSync(built(crate), resolve(staged, 'jniLibs/arm64-v8a/libexactcanvas.so'));
  // The host opens the module by the name its bake recorded, in the APK's extracted library directory.
  if (gpu) copyFileSync(gpu, resolve(staged, 'jniLibs/arm64-v8a', basename(gpu)));
  if (existsSync(resolve(app.dir, 'assets'))) cpSync(resolve(app.dir, 'assets'), resolve(staged, 'assets/app/assets'), { recursive: true });
  // A game's resolved manifest (with its `app` identity) is the one its shells were generated with.
  const gradle = spawnSync(resolve(READER, 'gradlew'), [':app:assembleRelease', `-Papp=${app.name}`, `-PappDir=${app.manifest.game ? app.workspace : app.dir}`, ...(gpu ? [`-PgpuModule=${basename(gpu)}`] : [])], { cwd: READER, env: { ...env, ANDROID_HOME: androidSdk(env) }, stdio: 'inherit' });
  return gradle.status === 0 ? resolve(READER, 'app/build', app.name, 'outputs/apk/release/app-release.apk') : null;
}

if (import.meta.main && process.argv[2] === 'relay') {
  // The reader carrier's own process (`readerDeploy`): not a verb a person runs.
  await relay(process.argv.slice(3)).catch((e) => { console.error(e.message); process.exit(1); });
} else if (import.meta.main) {
  const args = guardFlags('android', process.argv.slice(2));
  const [verb, name] = args.filter((arg) => !arg.startsWith('--'));
  if (verb !== 'build' || !name) {
    console.error('usage: bun scripts/agent-android.mjs build <app> [--apk]   (then: bun scripts/agent.mjs android --app <app> …; --apk: the app on the Canvas reader, host/android/README.md)');
    process.exit(2);
  }
  // Without rustup (a toolchain installed by hand) Cargo names a missing target itself.
  const rustup = spawnSync('rustup', ['target', 'list', '--installed'], { cwd: resolveApp(name).dir, encoding: 'utf8' });
  if (rustup.status === 0 && !rustup.stdout.split('\n').includes(ANDROID_TARGET)) {
    console.error(`the toolchain has no ${ANDROID_TARGET} target: rustup target add ${ANDROID_TARGET} (once, in the app's directory so its pinned toolchain gets it)`);
    process.exit(1);
  }
  const app = resolveApp(name), env = { ...androidToolchainEnv(), CARGO_TARGET_DIR: app.target };
  if (args.includes('--apk')) {
    const apk = buildApk(app, env, args.includes('--release') ? 'release' : HOST_DEV);
    if (apk) console.log(apk);
    process.exit(apk ? 0 : 1);
  }
  const [cmd, ...rest] = androidBuild(app);
  const built = spawnSync(cmd, rest, { cwd: app.workspace ?? app.dir, env, stdio: 'inherit' });
  if (built.status === 0) console.log(androidBinary(app));
  process.exit(built.status ?? 1);
}
