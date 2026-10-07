// The Android carrier (LLP 1107): an app's Linux host built for a phone's ABI
// (`aarch64-linux-android`, the host's Android-gated code, bionic, the phone's
// own fonts) and run headless under `adb shell`, where the agent protocol is
// the Linux host's own (LLP 1012 over stdio, `host/linux/src/agent.rs`). It
// drives the app's runner, kernel and painter on Android; it is not the
// Android lane's Canvas reader (LLP 1076 §3.3), which paints through its own
// Kotlin view.
//
//   bun scripts/agent-android.mjs build <app>     build the app's Linux crate for Android
//   bun scripts/agent.mjs android --app <app> …   drive it on the first adb device or emulator
//
// The NDK comes from ANDROID_NDK_HOME, else the newest under the SDK's ndk/
// (ANDROID_HOME, else ~/Library/Android/sdk or ~/Android/Sdk); ANDROID_SERIAL
// picks a device. Fonts are the phone's (/system/fonts) unless the drive names
// EXACT_FONTS. The painter is the CPU one unless EXACT_PAINTER says
// otherwise: the GPU painter needs a real GPU's Vulkan (an emulator's
// SwiftShader crashes it).
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, resolve } from 'node:path';
import { HOST_DEV, linuxBuild, resolveApp } from './app.mjs';

export const ANDROID_TARGET = 'aarch64-linux-android';
/** The oldest Android the binary runs on (the NDK's clang wrapper names it). */
const API = 30;
export const androidBinary = (app, bin = app.crate('linux')) => resolve(app.target, ANDROID_TARGET, HOST_DEV, bin);
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
  return { ...env, CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER: cc, CC_aarch64_linux_android: cc,
    CXX_aarch64_linux_android: `${cc}++`, AR_aarch64_linux_android: resolve(bin, 'llvm-ar') };
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

if (import.meta.main) {
  const [verb, name] = process.argv.slice(2);
  if (verb !== 'build' || !name) {
    console.error('usage: bun scripts/agent-android.mjs build <app>   (then: bun scripts/agent.mjs android --app <app> …)');
    process.exit(2);
  }
  const installed = spawnSync('rustup', ['target', 'list', '--installed'], { cwd: resolveApp(name).dir, encoding: 'utf8' }).stdout ?? '';
  if (!installed.split('\n').includes(ANDROID_TARGET)) {
    console.error(`the toolchain has no ${ANDROID_TARGET} target: rustup target add ${ANDROID_TARGET} (once, in the app's directory so its pinned toolchain gets it)`);
    process.exit(1);
  }
  const app = resolveApp(name), [cmd, ...args] = androidBuild(app);
  const built = spawnSync(cmd, args, { cwd: app.workspace ?? app.dir, env: { ...androidToolchainEnv(), CARGO_TARGET_DIR: app.target }, stdio: 'inherit' });
  if (built.status === 0) console.log(androidBinary(app));
  process.exit(built.status ?? 1);
}
