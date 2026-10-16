#!/usr/bin/env node
// Build Snapback4's web device and client (exact-snapback4-web): the wasm
// an app serves (`node snapback4/web/build.mjs <app>/assets/snapback4.wasm`)
// and its wasm-bindgen glue, which `ts/web.ts` imports (`ts/generated/`,
// never committed). With no output it writes only the glue, which every
// host's bake type-checks. Runs under Node or Bun, and from an app's build.rs. Size first (profile `wasm`), then `wasm-opt -Oz` when
// binaryen is on PATH or in Exact's cache and it transfers smaller. Files are rewritten only when their bytes change.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, dirname, resolve, sep } from 'node:path';
import { createInterface } from 'node:readline';
import { gzipSync } from 'node:zlib';
import { deviceHash, embeddedDeviceStamp, validateDevice } from './device-build.mjs';

const out = process.argv[2];
// Help before any work: `--help` was taken as the output path, after minutes of building (app farm 016).
if (out === '--help' || out === '-h' || out?.startsWith('-')) {
  console[out === '--help' || out === '-h' ? 'log' : 'error'](`usage: node snapback4/web/build.mjs [<output.wasm>]
  Builds Snapback4's web device (the wasm an app serves) and its wasm-bindgen glue (snapback4/ts/generated/).
  The output is a path from the current directory; run it from the app's directory:
    node <exact2>/snapback4/web/build.mjs assets/snapback4.wasm
  The web host serves assets/snapback4.wasm at /assets/snapback4.wasm, the driver's default.
  With no output it writes only the glue, which every host's bake type-checks.`);
  process.exit(out === '--help' || out === '-h' ? 0 : 2);
}
const workspace = resolve(import.meta.dirname, '..');
// One absolute directory for Cargo (run in the workspace) and the paths below.
const target = resolve(process.env.SNAPBACK4_TARGET_DIR ?? resolve(workspace, 'target'));
// From an app's build.rs, the outer build's target, flags, toolchain and C
// compiler settings are in the environment; this build is its own (the
// checkout's toolchain, wasm32, no host C toolchain).
const env = Object.fromEntries(Object.entries(process.env).filter(([key]) =>
  !/^(CARGO_(?!HOME$)|RUST(FLAGS|DOCFLAGS|C|C_WRAPPER|C_WORKSPACE_WRAPPER|UP_TOOLCHAIN)$|TARGET$|HOST$|OUT_DIR$|PROFILE$|OPT_LEVEL$|DEBUG$|NUM_JOBS$|SDKROOT$|DEVELOPER_DIR$|\w+_DEPLOYMENT_TARGET$|(TARGET_|HOST_)?(CC|CXX|AR|RANLIB|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS|ARFLAGS)(_[\w-]+)?$)/.test(key)));
env.CARGO_TARGET_DIR = target;
env.CARGO_INCREMENTAL = '0';
env.CARGO_PROFILE_DEV_DEBUG = '0';
env.CARGO_PROFILE_TEST_DEBUG = '0';
// A package id, as cargo writes it: `name 1.0 (source)`, `source#name@1.0`, or
// `path+file:///…/name#1.0`.
const packageName = id => /^([\w-]+) \d/.exec(id)?.[1] ?? /#([\w-]+)@/.exec(id)?.[1] ?? /\/([\w-]+)#[\d.]+$/.exec(id)?.[1] ?? 'a dependency';
// A build script's stdout is cargo's instruction channel: report on stderr.
// Rust's diagnostics come as JSON and are shown here: every error, and this
// workspace's warnings; a dependency's warnings (Snapback's own crates, when
// built from a path) are counted, not shown: they are upstream's, and printed
// whole they buried the result (app farm wave 2: seven builds, ~30 each).
const cargo = await new Promise(done => {
  const child = spawn('cargo', ['build', '-p', 'exact-snapback4-web', '--profile', 'wasm', '--target', 'wasm32-unknown-unknown', '--locked',
    `--message-format=${process.stderr.isTTY ? 'json-diagnostic-rendered-ansi' : 'json'}`], { cwd: workspace, stdio: ['ignore', 'pipe', 'inherit'], env });
  const hidden = new Map();
  createInterface({ input: child.stdout }).on('line', line => {
    let message;
    try { message = JSON.parse(line); } catch { return void process.stderr.write(line + '\n'); }
    if (message.reason !== 'compiler-message' || !message.message?.rendered) return;
    const ours = typeof message.manifest_path === 'string' && message.manifest_path.startsWith(workspace + sep);
    if (message.message.level.startsWith('error') || ours) return void process.stderr.write(message.message.rendered);
    if (message.message.level !== 'warning') return;
    const name = packageName(message.package_id ?? '');
    hidden.set(name, (hidden.get(name) ?? 0) + 1);
  });
  child.on('error', error => done({ status: 1, error }));
  child.on('close', status => {
    if (hidden.size) console.error(`snapback4 web: not shown, ${[...hidden].map(([name, n]) => `${n} warning${n === 1 ? '' : 's'} in ${name}`).join(', ')}: a dependency's, not yours to fix (\`cargo build -p exact-snapback4-web --target wasm32-unknown-unknown\` in ${workspace} shows them)`);
    done({ status });
  });
});
if (cargo.error) console.error(`snapback4 web: ${cargo.error.code === 'ENOENT' ? 'cargo not on PATH (install Rust: https://rustup.rs)' : cargo.error.message}`);
if (cargo.status !== 0) process.exit(cargo.status ?? 1);
const compiled = resolve(target, 'wasm32-unknown-unknown/wasm/exact_snapback4_web.wasm');
// Cargo serializes compilation; bindgen and wasm-opt need private output too.
const glue = mkdtempSync(resolve(target, 'snapback4-web-bindgen-'));
process.on('exit', () => rmSync(glue, {recursive: true, force: true}));
const bindgen = spawnSync('wasm-bindgen', ['--target', 'web', '--out-dir', glue, '--out-name', 'device', compiled], { stdio: ['ignore', 2, 'inherit'], env });
if (bindgen.status !== 0) {
  console.error(`snapback4 web: ${bindgen.error?.code === 'ENOENT' ? 'wasm-bindgen not on PATH (cargo install wasm-bindgen-cli --version 0.2.127)' : 'wasm-bindgen failed'}`);
  process.exit(1);
}
const built = resolve(glue, 'device_bg.wasm');
validateDevice(readFileSync(built), 'wasm-bindgen output');
const optimized = resolve(glue, 'device_bg.opt.wasm');
// The binaryen `exact setup` installs (scripts/app.mjs puts it on PATH for
// Exact's own builds; this script also runs on its own and from build.rs).
const cached = resolve(homedir(), '.cache/exact/binaryen');
const optEnv = { ...process.env };
if (existsSync(cached)) {
  const bins = readdirSync(cached).sort().reverse().map(version => resolve(cached, version, 'bin'))
    .filter(bin => existsSync(resolve(bin, process.platform === 'win32' ? 'wasm-opt.exe' : 'wasm-opt')));
  if (bins.length) optEnv.PATH = `${bins[0]}${delimiter}${optEnv.PATH ?? ''}`;
}
const opt = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', built, '-o', optimized],
  { stdio: ['ignore', 'ignore', 'pipe'], encoding: 'utf8', env: optEnv });
if (opt.status !== 0) {
  console.error(`snapback4 web: ${opt.error?.code === 'ENOENT' ? 'wasm-opt not found (`bun scripts/exact.mjs setup` installs it)' : `wasm-opt failed: ${opt.stderr.trim().split('\n').at(-1)}`}; unoptimized`);
}
if (opt.status === 0) validateDevice(readFileSync(optimized), 'wasm-opt output');
// Keep whichever transfers smaller: -Oz shrinks this module raw but can grow
// it gzipped (as Snapback's own build-wasm.sh finds and compares).
const gzipped = path => gzipSync(readFileSync(path), { level: 9 }).length;
const bytes = readFileSync(opt.status === 0 && gzipped(optimized) < gzipped(built) ? optimized : built);
function write(path, bytes) {
  if (existsSync(path) && readFileSync(path).equals(bytes)) return;
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path + `.tmp.${process.pid}`, bytes);
  renameSync(path + `.tmp.${process.pid}`, path);
}
if (out) {
  const path = resolve(process.cwd(), out);
  write(path, bytes);
  const stamp = embeddedDeviceStamp(bytes);
  write(path + '.json', Buffer.from(JSON.stringify({ ...stamp, sha256: deviceHash(bytes) }, null, 2) + '\n'));
}
// The glue as TypeScript the bake captures; it is generated, so unchecked.
const source = readFileSync(resolve(glue, 'device.js'), 'utf8');
write(resolve(workspace, 'ts/generated/device.ts'), Buffer.from(`// @ts-nocheck\n// @generated by snapback4/web/build.mjs (wasm-bindgen 0.2.127). Do not edit.\n${source}`));
console.error(`snapback4 web: ${bytes.length} B, ${gzipSync(bytes, { level: 9 }).length} B gzip${out ? ` → ${out}` : ' (glue only)'}`);
