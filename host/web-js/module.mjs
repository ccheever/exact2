// An app's Rust data source as a standalone logic module (LLP 1029.000,
// ABI 3) for the JS runtime, when the app declares none of its own: a
// generated crate exporting the same DataSource the web build bakes with
// (`apps/<app>/web/build.rs`'s `contract::bake(plan, <expr>)`).
// usage: bun host/web-js/module.mjs <app> [--out <dir>] → <dir>/app.module.wasm
//        bun host/web-js/module.mjs --prebuild: what every app's web build
//        shares (the compiler, the leaf modules), compiled and kept for the
//        machine, so no app's first build compiles them.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
// Also for its PATH: the pinned binaryen (and Cargo) as every web build has
// them, so a prebuild optimizes and keeps what an app's build would.
import { WEB_STD, WEB_TOOLCHAIN, wasmRemapFlags } from '../../scripts/app.mjs';
import { keptProducts } from '../../scripts/kept.mjs';
import { BINARYEN_DOWNLOAD } from '../web/stages.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
// The web toolchain's size build (scripts/app.mjs WEB_TOOLCHAIN): std built
// for size, panics abort without text, and no path of this machine or
// checkout in the bytes (`wasmRemapFlags`, as every web-profile wasm build).
// Asked of rustc once, when a module is built.
const WEB = WEB_TOOLCHAIN;
let wasmFlags;
const WASM_FLAGS = () => (wasmFlags ??= [...WEB_STD, ...wasmRemapFlags(null, WEB)]);
// What those flags say apart from this checkout's path, for a kept module's group.
const WASM_FLAGS_KEY = () => WASM_FLAGS().map(f => f.replaceAll(root, '.'));
// Under this checkout's own target/ (AGENTS.md: never another checkout's):
// a module built from one worktree's sources is never another's.
const TARGET_DIR = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target');
const MODULES = resolve(TARGET_DIR, 'web-js-modules');
const TARGET = resolve(MODULES, 'target');
// A leaf module's own target directory: the leaves build at once, and one
// Cargo holds a target directory's lock for the whole of its build.
const leafTarget = (name) => resolve(MODULES, `target-${name}`);

// A leaf module, and the compiler, hold nothing of an app: one some checkout
// of this machine compiled from these bytes is taken (scripts/kept.mjs,
// `~/.cache/exact/web-modules`), and one compiled here is kept. A production
// build takes one too: a leaf is built at the same release profile either way,
// and an entry is taken only for the same bytes of every input.
const kept = keptProducts({ root, cache: 'web-modules', env: process.env, skip: [TARGET_DIR], label: 'web-js' });
// A leaf is kept as `wasm-opt` left it: which one ran is part of what it is.
let wasmOpt;
const WASM_OPT = () => (wasmOpt ??= spawnSync('wasm-opt', ['--version'], { encoding: 'utf8' }).stdout ?? null);
// What a prebuild left in the cache, by product: whether it is kept for these sources.
const held = new Map();

// `wasm-opt -Oz` from `input` to `output`, or, when binaryen is missing or
// fails, `input` copied there unoptimized and said so, as host/web/build.mjs
// does: a missing tool is named, never left for a later ENOENT to report.
const unoptimized = new Set();
const OPT = ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals'];
function optimized(opt, input, output, label) {
  if (opt.status === 0) return output;
  // Not optimized: a leaf is not kept for other checkouts (`buildLeaf`).
  unoptimized.add(output);
  if (input !== output) cpSync(input, output);
  process.stderr.write(`${label}: ${opt.error?.code === 'ENOENT' ? `wasm-opt not on PATH (binaryen: brew install binaryen, or ${BINARYEN_DOWNLOAD})` : `wasm-opt failed: ${(opt.stderr ?? '').trim().split('\n').at(-1)}`}; unoptimized\n`);
  return output;
}
function optimize(input, output, label) {
  return optimized(spawnSync('wasm-opt', [...OPT, input, '-o', output], { stdio: ['ignore', 'ignore', 'pipe'], encoding: 'utf8' }), input, output, label);
}
/** `spawnSync`'s answer, without blocking: `{ status, error, stderr }`. */
function run(cmd, args, options, capture = false) {
  return new Promise(done => {
    const child = spawn(cmd, args, { ...options, stdio: capture ? ['ignore', 'ignore', 'pipe'] : ['ignore', 'inherit', 'inherit'] });
    let stderr = '';
    child.stderr?.on('data', d => { stderr += d; });
    child.on('error', error => done({ status: null, error, stderr }));
    child.on('close', status => done({ status, stderr }));
  });
}
const optimizeAsync = async (input, output, label) => optimized(await run('wasm-opt', [...OPT, input, '-o', output], {}, true), input, output, label);
const cargoWasmArgs = (manifest) => [`+${WEB}`, 'build', '--release', '--target', 'wasm32-unknown-unknown', '--manifest-path', manifest, ...WASM_FLAGS()];
function cargoWasm(manifest) {
  return spawnSync('cargo', cargoWasmArgs(manifest), { stdio: ['ignore', 'inherit', 'inherit'], env: { ...process.env, CARGO_TARGET_DIR: TARGET } });
}
/** Whether `product` is newer than everything Cargo's dep-info `dep` says
 * it was built from, and than what the build scripts behind it declared
 * (`cargo:rerun-if-changed`, beside each `out/` it read). An unchanged
 * module then skips Cargo, whose no-op check under `-Zbuild-std` costs
 * about a second an edit (LLP 1071 §7, the dev loop). */
export function fresh(product, dep, also = []) {
  try {
    const built = statSync(product).mtimeMs, text = readFileSync(dep, 'utf8');
    const files = text.slice(text.indexOf(': ') + 2).trim().split(/(?<!\\)\s+/).map(f => f.replace(/\\ /g, ' '));
    // A build script's relative paths are its package's: the package is
    // found by name among the crate roots the dep-info lists.
    const roots = new Map();
    for (const f of files) {
      const at = f.lastIndexOf('/src/'), dir = at < 0 ? null : f.slice(0, at);
      if (dir && !roots.has(dir)) roots.set(dir, /^name\s*=\s*"([^"]+)"/m.exec(readFileSync(resolve(dir, 'Cargo.toml'), 'utf8'))?.[1]);
    }
    const byName = new Map([...roots].map(([dir, name]) => [name, dir]));
    for (const o of new Set(files.map(f => /^(.*\/build\/[^/]+)\/out\//.exec(f)?.[1]).filter(Boolean))) {
      const dir = byName.get(/\/build\/(.+)-[0-9a-f]+$/.exec(o)[1]);
      for (const m of readFileSync(resolve(o, 'output'), 'utf8').matchAll(/^cargo:(?::)?rerun-if-changed=(.+)$/gm)) files.push(resolve(dir, m[1]));
    }
    // A directory is watched whole, as Cargo scans one.
    const newer = f => { const st = statSync(f); return st.mtimeMs > built || (st.isDirectory() && readdirSync(f).some(n => newer(resolve(f, n)))); };
    return ![...files, ...also].some(newer);
  } catch { return false; }
}
/** Write `text` to `path` only when it differs: an unchanged generated
 * crate keeps its mtime, so `fresh` holds. */
const keep = (path, text) => { if (!existsSync(path) || readFileSync(path, 'utf8') !== text) writeFileSync(path, text); };
/** A generated crate's `Cargo.lock`: the workspace's, again whenever the
 * workspace's changes, so it builds the versions the workspace pins (and
 * that a kept module's group names). */
function lockFrom(crate) {
  const lock = readFileSync(resolve(root, 'Cargo.lock')), from = resolve(crate, 'Cargo.lock.from'), id = createHash('sha256').update(lock).digest('hex');
  if (existsSync(resolve(crate, 'Cargo.lock')) && existsSync(from) && readFileSync(from, 'utf8') === id) return;
  writeFileSync(resolve(crate, 'Cargo.lock'), lock);
  writeFileSync(from, id);
}
/** Cargo's module at `wasm`, optimized to `<out>/app.module.wasm` unless
 * that is still newer than everything the module was built from. */
function module(manifest, name, out, label) {
  const wasm = resolve(TARGET, 'wasm32-unknown-unknown/release', `${name}.wasm`), done = resolve(out, 'app.module.wasm');
  mkdirSync(out, { recursive: true });
  if (fresh(done, wasm.replace(/\.wasm$/, '.d'), [manifest])) return done;
  if (cargoWasm(manifest).status !== 0) throw new Error(`${label} did not build`);
  cpSync(wasm, done);
  return optimize(done, done, 'module');
}

export function buildModule(app, out = resolve(MODULES, app), draw = false, dir = resolve(root, 'apps', app)) {
  const build = readFileSync(resolve(dir, 'web/build.rs'), 'utf8');
  const expr = /contract::bake\(\s*plan,\s*([\w:]+(?:::(?:default|new)\(\))?)\s*\)/.exec(build)?.[1];
  if (!expr) return ownModule(app, out, dir);
  const ty = expr.replace(/::(default|new)\(\)$/, '');
  const dataToml = readFileSync(resolve(dir, 'data/Cargo.toml'), 'utf8');
  const pkg = /^name\s*=\s*"([^"]+)"/m.exec(dataToml)[1];
  const crate = resolve(out, 'crate');
  mkdirSync(resolve(crate, 'src'), { recursive: true });
  const ws = readFileSync(resolve(root, 'Cargo.toml'), 'utf8');
  const patch = ws.slice(ws.indexOf('[patch.crates-io]')).split('\n[')[0].replace(/path = "vendor/g, `path = "${root}/vendor`);
  keep(resolve(crate, 'Cargo.toml'), `[package]
name = "exact-js-module-${app}"
version = "0.1.0"
edition = "2021"
publish = false
[lib]
crate-type = ["cdylib"]
[dependencies]
${pkg} = { path = "${resolve(dir, 'data')}" }
exact-logic-abi = { path = "${resolve(root, 'logic/abi')}" }
[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
[workspace]
${ws.slice(ws.indexOf('[workspace.dependencies]')).split('\n[workspace.package]')[0]}
${patch}
`);
  // Canvas 2D draws only for a plan that has a 2D surface (logic/abi/src/draw.rs).
  keep(resolve(crate, 'src/lib.rs'), `exact_logic_abi::export!(${ty}, ${expr});\n${draw ? `exact_logic_abi::export_draw!(${ty});\n` : ''}`);
  lockFrom(crate);
  return module(resolve(crate, 'Cargo.toml'), `exact_js_module_${app.replaceAll('-', '_')}`, out, `${app}: the module`);
}

/** The app's own logic module (`rust.module.package` in app.json, LLP
 * 1029.000), for an app whose web build bakes with no single Rust source (a
 * TypeScript and a Rust source composed, LLP 1027.002): its crate already
 * exports the ABI, built as the generated one is. */
function ownModule(app, out, dir) {
  const pkg = JSON.parse(readFileSync(resolve(dir, 'app.json'), 'utf8')).rust?.module?.package;
  if (!pkg) throw new Error(`${app}: its web build bakes with no Rust data source, and app.json names no rust.module.package`);
  const manifest = readdirSync(dir).map(d => resolve(dir, d, 'Cargo.toml')).find(f => existsSync(f) && new RegExp(`^name\\s*=\\s*"${pkg}"`, 'm').test(readFileSync(f, 'utf8')));
  if (!manifest) throw new Error(`${app}: no crate in ${dir} is ${pkg}`);
  return module(manifest, pkg.replaceAll('-', '_'), out, `${app}: ${pkg}`);
}

/** A logic module's grants (ABI 3's first call, `meta`): what its storage
 * requests may name, which decides whether the page ships the storage
 * adapters. */
export async function moduleGrants(path) {
  const { instance } = await WebAssembly.instantiate(readFileSync(path), {}), e = instance.exports;
  const session = e.exact_logic_create(), request = new Uint8Array([3, 0, 0, 0, 0]), p = e.exact_logic_alloc(request.length);
  new Uint8Array(e.memory.buffer, p, request.length).set(request);
  if (e.exact_logic_call(session, p, request.length) !== 0) return '';
  const out = new Uint8Array(e.memory.buffer, e.exact_logic_output(session), e.exact_logic_output_len(session)).slice(), d = new DataView(out.buffer);
  let at = 5;
  const str = () => { const n = d.getUint32(at, true), t = new TextDecoder().decode(out.subarray(at + 4, at + 4 + n)); at += 4 + n; return t; };
  str();
  return str();
}


/** Markdown pieces (LLP 1045 D3) as a loaded capability for the runtime:
 * the web host's own `exact_web_capabilities::markdown::pieces`, exported
 * alone from a wasm module the page fetches on its first Markdown node. */
export function buildMarkdown({ out = resolve(MODULES, 'markdown'), keepFresh = false } = {}) {
  const ws = readFileSync(resolve(root, 'Cargo.toml'), 'utf8');
  const patch = ws.slice(ws.indexOf('[patch.crates-io]')).split('\n[')[0].replace(/path = "vendor/g, `path = "${root}/vendor`);
  return buildLeaf('markdown', 'exact-web-capabilities', 'host/web-capabilities', { out, keepFresh, patch, lib: `//! Generated: Markdown pieces for the JS runtime.
static mut OUT: Vec<u8> = Vec::new();
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 { let mut v = Vec::<u8>::with_capacity(len); let p = v.as_mut_ptr(); std::mem::forget(v); p }
/// # Safety: \`ptr\` holds \`len\` UTF-8 bytes from \`alloc\`.
#[no_mangle]
pub unsafe extern "C" fn pieces(ptr: *mut u8, len: usize) -> usize {
    let source = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    drop(Vec::from_raw_parts(ptr, len, len));
    OUT = exact_web_capabilities::markdown::pieces(&source).into_bytes();
    #[allow(static_mut_refs)]
    OUT.len()
}
#[no_mangle]
#[allow(static_mut_refs)]
pub extern "C" fn output() -> *const u8 { unsafe { OUT.as_ptr() } }
` });
}

/** The motion capability (LLP 1071 §7): `exact-web-js-motion`'s exports,
 * the one `exact_motion` Engine, as a wasm module the page fetches after
 * first paint when its plan uses motion. */
export function buildMotion({ out = resolve(MODULES, 'motion'), keepFresh = false } = {}) {
  return buildLeaf('motion', 'exact-web-js-motion', 'host/web-js/motion', { out, keepFresh });
}

/** The Markdown editor's rules (LLP 1045 D5), the crate the wasm host's
 * build makes its `markup-editor.wasm` from. */
export function buildEditor({ out = resolve(MODULES, 'editor'), keepFresh = false } = {}) {
  return buildLeaf('markup-editor', 'exact-markdown-editor', 'markdown/editor', { out, keepFresh });
}

/** The exclusions walker (LLP 1043.000 §3 D7): `exact-textflow`'s
 * `textflow-web` binary, as the wasm build builds its `textflow.wasm`. */
export async function buildFlow({ out = resolve(MODULES, 'flow'), keepFresh = false } = {}) {
  mkdirSync(out, { recursive: true });
  const target = leafTarget('flow'), product = resolve(target, 'wasm32-unknown-unknown/web/textflow-web.wasm'), wasm = resolve(out, 'textflow.wasm');
  const depInfo = product.replace(/\.wasm$/, '.d');
  // Its group asks rustc and wasm-opt for their versions: only when it is needed.
  const at = () => kept({ name: 'textflow.wasm', crate: 'exact-textflow', product, depInfo, file: wasm, toolchain: WEB, group: ['textflow-web', 'web', WASM_FLAGS_KEY(), OPT, WASM_OPT()],
    buildDirs: [resolve(target, 'wasm32-unknown-unknown/web/build'), resolve(target, 'web/build')], stamp: resolve(MODULES, 'kept', 'textflow.json') });
  // Built again only when something it was built from changed (`fresh`). A
  // prebuild keeps one built earlier on its receipt, or builds it again.
  if (fresh(wasm, depInfo)) {
    if (!keepFresh) return wasm;
    const again = at().keepAgain();
    if (again !== null) { held.set('textflow.wasm', again); return wasm; }
    rmSync(product, { force: true });
  }
  const leaf = at(), taken = leaf.find();
  if (taken) { cpSync(taken, wasm); held.set(wasm.split('/').at(-1), true); return wasm; }
  const started = Date.now();
  const r = await run('cargo', [`+${WEB}`, 'build', '--profile', 'web', '--target', 'wasm32-unknown-unknown', '-p', 'exact-textflow', '--bin', 'textflow-web', ...WASM_FLAGS()],
    { cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target } });
  if (r.status !== 0) throw new Error('textflow.wasm did not build');
  held.set('textflow.wasm', !unoptimized.has(await optimizeAsync(product, wasm, 'textflow.wasm')) && leaf.keep(started));
  return wasm;
}

// A workspace crate's exports as a size-built wasm module: `<name>.wasm`,
// in its own target directory, so the leaves an app uses build at once.
async function buildLeaf(name, pkg, dir, { out, keepFresh, patch = '', lib }) {
  const crate = resolve(out, 'crate');
  mkdirSync(resolve(crate, 'src'), { recursive: true });
  const ws = readFileSync(resolve(root, 'Cargo.toml'), 'utf8');
  const toml = `[package]
name = "exact-js-${name}"
version = "0.1.0"
edition = "2021"
publish = false
[lib]
crate-type = ["cdylib"]
[dependencies]
${pkg} = { path = "${resolve(root, dir)}" }
[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
[workspace]
${ws.slice(ws.indexOf('[workspace.dependencies]')).split('\n[workspace.package]')[0]}
${patch}`;
  const source = lib ?? `//! Generated: ${name}.wasm for the JS runtime.\npub use ${pkg.replaceAll('-', '_')}::*;\n`;
  keep(resolve(crate, 'Cargo.toml'), toml);
  keep(resolve(crate, 'src/lib.rs'), source);
  lockFrom(crate);
  const target = leafTarget(name), release = resolve(target, 'wasm32-unknown-unknown/release');
  const product = resolve(release, `exact_js_${name.replaceAll('-', '_')}.wasm`), depInfo = product.replace(/\.wasm$/, '.d'), wasm = resolve(out, `${name}.wasm`);
  // The generated crate names this checkout's paths; what it says otherwise is the group's.
  const at = () => kept({ name: `${name}.wasm`, crate: pkg, product, depInfo, file: wasm, toolchain: WEB, group: ['leaf', name, toml.replaceAll(root, '.'), source, WASM_FLAGS_KEY(), OPT, WASM_OPT()],
    buildDirs: [resolve(release, 'build'), resolve(target, 'release/build')], stamp: resolve(MODULES, 'kept', `${name}.json`) });
  // Built again only when something it was built from changed (`fresh`). A
  // prebuild keeps one built earlier on its receipt, or builds it again.
  if (fresh(wasm, depInfo, [resolve(crate, 'Cargo.toml')])) {
    if (!keepFresh) return wasm;
    const again = at().keepAgain();
    if (again !== null) { held.set(`${name}.wasm`, again); return wasm; }
    rmSync(product, { force: true });
  }
  const leaf = at(), taken = leaf.find();
  if (taken) { cpSync(taken, wasm); held.set(wasm.split('/').at(-1), true); return wasm; }
  const started = Date.now();
  const r = await run('cargo', cargoWasmArgs(resolve(crate, 'Cargo.toml')), { env: { ...process.env, CARGO_TARGET_DIR: target } });
  if (r.status !== 0) throw new Error(`${name}.wasm did not build`);
  cpSync(product, wasm);
  held.set(`${name}.wasm`, !unoptimized.has(await optimizeAsync(wasm, wasm, `${name}.wasm`)) && leaf.keep(started));
  return wasm;
}

/** The plan-to-JavaScript compiler (`exact-web-js`) as `[cmd, args]`: its
 * binary when nothing it was built from changed, one kept for the machine,
 * or `cargo run`, whose binary `done()` keeps. */
const COMPILER = resolve(TARGET_DIR, 'debug/exact-web-js');
const compilerKept = () => kept({ crate: 'exact-web-js', product: COMPILER, depInfo: `${COMPILER}.d`, group: ['debug'], buildDirs: [resolve(TARGET_DIR, 'debug/build')], stamp: resolve(MODULES, 'kept', 'exact-web-js.json') });
export function webCompiler() {
  if (fresh(COMPILER, `${COMPILER}.d`)) return { cmd: COMPILER, pre: [], done() {} };
  const at = compilerKept();
  const taken = at.find();
  if (taken) return { cmd: taken, pre: [], done() {} };
  const started = Date.now();
  return { cmd: 'cargo', pre: ['run', '-q', '-p', 'exact-web-js', '--'], done: () => at.keep(started) };
}

/** Everything an app's web build shares, compiled where it is not yet and
 * kept for the machine: after it, no app's first web build on this machine
 * compiles them at this checkout's sources. A product already built is kept
 * on the receipt its compile left (`keepAgain`); one without is built again. */
async function prebuild() {
  if (spawnSync('git', ['-C', root, 'status', '--porcelain'], { encoding: 'utf8' }).stdout) console.warn('web-js: this checkout has uncommitted changes; what they touch is built but not kept');
  const compiler = compilerKept();
  const compiling = (async () => {
    const again = fresh(COMPILER, `${COMPILER}.d`) ? compiler.keepAgain() : null;
    if (again !== null) return held.set('exact-web-js', again);
    rmSync(COMPILER, { force: true });
    // Already kept from these sources (this checkout took it): nothing to do.
    if (compiler.find()) return held.set('exact-web-js', true);
    const started = Date.now();
    if ((await run('cargo', ['build', '-q', '-p', 'exact-web-js'], { cwd: root })).status !== 0) throw new Error('exact-web-js did not build');
    held.set('exact-web-js', compiler.keep(started));
  })();
  await Promise.all([buildMarkdown, buildMotion, buildEditor, buildFlow].map(build => build({ keepFresh: true })));
  await compiling;
  const names = (kept) => [...held].filter(([, k]) => k === kept).map(([name]) => name).join(', ');
  console.log(`web-js: kept for this machine (~/.cache/exact/web-modules): ${names(true) || 'nothing'}`);
  if (names(false)) console.warn(`web-js: built, not kept (uncommitted or changed while compiling): ${names(false)}`);
}

if (import.meta.main) {
  if (process.argv.includes('--prebuild')) await prebuild();
  else {
    const [app] = process.argv.slice(2);
    const o = process.argv.indexOf('--out');
    console.log(buildModule(app, o < 0 ? undefined : process.argv[o + 1]));
  }
}
