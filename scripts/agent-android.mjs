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
// picks a device. The painter is the CPU one unless EXACT_PAINTER says
// otherwise: the GPU painter needs a real GPU's Vulkan (an emulator's
// SwiftShader crashes it).
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, resolve } from 'node:path';
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

/** Put the binary and the app's assets on the device and return how to run it there: `spawn()` the
 * process (stdio is the agent's), the device path a screenshot is written to, and `pull(path)` to
 * copy that screenshot here. Only EXACT_* variables cross, with the device's own HOME, assets,
 * native-module directory and fonts. */
export function androidDeploy(app, bin, env) {
  const serial = androidDevice(env), tool = adb(env);
  const remote = `/data/local/tmp/exact/${app.id}`;
  const run = (args, what) => {
    const r = spawnSync(tool, ['-s', serial, ...args], { encoding: 'utf8' });
    if (r.status !== 0) throw new Error(`adb ${what}: ${(r.stderr || r.stdout || r.error?.message || '').trim()}`);
  };
  run(['shell', `rm -rf ${quote(remote)} && mkdir -p ${quote(`${remote}/home`)}`], 'prepare');
  run(['push', bin, `${remote}/${basename(bin)}`], 'push the binary');
  run(['shell', `chmod 755 ${quote(`${remote}/${basename(bin)}`)}`], 'chmod');
  if (existsSync(resolve(app.dir, 'assets'))) run(['push', resolve(app.dir, 'assets'), `${remote}/`], 'push the assets');
  const vars = Object.fromEntries(Object.entries(env).filter(([k]) => k.startsWith('EXACT_')));
  delete vars.EXACT_FONT; // the desktop carriers' pinned DejaVu; a phone draws with its own faces
  Object.assign(vars, { HOME: `${remote}/home`, EXACT_ASSETS: remote, EXACT_NATIVE_LIBS: remote,
    EXACT_FONTS: env.EXACT_ANDROID_FONTS ?? '/system/fonts', EXACT_PAINTER: env.EXACT_PAINTER ?? 'cpu' });
  const assignments = Object.entries(vars).map(([k, v]) => `${k}=${quote(v)}`).join(' ');
  const shot = `${remote}/agent-shot.png`;
  return {
    serial, remote, shot,
    spawn: () => spawn(tool, ['-s', serial, 'shell', '-T', `cd ${quote(remote)} && exec env ${assignments} ./${basename(bin)}`], { stdio: ["pipe", "pipe", "pipe"] }),
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
  const app = resolveApp(name), [cmd, ...args] = androidBuild(app);
  const built = spawnSync(cmd, args, { cwd: app.dir, env: androidToolchainEnv(), stdio: 'inherit' });
  if (built.status === 0) console.log(androidBinary(app));
  process.exit(built.status ?? 1);
}
