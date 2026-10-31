// An app's Rust data source as a standalone logic module (LLP 1029.000,
// ABI 4) for the JS runtime, when the app declares none of its own: a
// generated crate exporting the same DataSource the web build bakes with
// (`apps/<app>/web/build.rs`'s `contract::bake(plan, <expr>)`).
// usage: bun host/web-js/module.mjs <app> [--out <dir>] → <dir>/app.module.wasm
//        bun host/web-js/module.mjs --prebuild: what every app's web build
//        shares (the compiler, the leaf modules), compiled and kept for the
//        machine, so no app's first build compiles them.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
// Also for its PATH: the pinned binaryen (and Cargo) as every web build has
// them, so a prebuild optimizes and keeps what an app's build would.
import { WEB_STD, WEB_SIZE, WEB_TOOLCHAIN, wasmRemapFlags } from '../../scripts/app.mjs';
import { withHostDeployment } from '../../scripts/filesystem.mjs';
import { replaceBuild } from '../web/serve.mjs';
import { proofInputs } from '../../game/inputs.mjs';
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
// Every build of this checkout that uses a module copies it from here, and
// builds run at once (conformance's --jobs), while a module taken from the
// machine's cache is copied here again at each build. So a module is written
// beside its place by `write` and renamed onto it: a reader copies the old
// bytes or the new, never a file still being written (a page got an empty
// motion.wasm, had no motion engine, and its drags did nothing).
function replace(path, write) {
  const temp = `${path}.${process.pid}.tmp`, clean = () => rmSync(temp, { force: true });
  const land = (value) => { try { renameSync(temp, path); return value; } finally { clean(); } };
  let written;
  try { written = write(temp); } catch (error) { clean(); throw error; }
  return written instanceof Promise ? written.then(land, (error) => { clean(); throw error; }) : land(written);
}
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
  replace(done, (temp) => optimize(wasm, temp, 'module'));
  return done;
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

/** A logic module's grants (the seam's first call, `meta`): what its storage
 * requests may name, which decides whether the page ships the storage
 * adapters. */
export async function moduleGrants(path) {
  const { instance } = await WebAssembly.instantiate(readFileSync(path), {}), e = instance.exports;
  const session = e.exact_logic_create(), request = new Uint8Array(5), p = e.exact_logic_alloc(request.length);
  new DataView(request.buffer).setUint32(0, e.exact_logic_abi(), true);
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
/// # Safety: \`ptr\` holds \`len\` UTF-8 bytes from \`alloc\`: the source, then
/// \`atoms\` bytes of its \`atoms\` JSON and \`faces\` of its \`atomFaces\`.
#[no_mangle]
pub unsafe extern "C" fn pieces(ptr: *mut u8, len: usize, atoms: usize, faces: usize) -> usize {
    let all = std::slice::from_raw_parts(ptr, len);
    let (source, rest) = all.split_at(len - atoms - faces);
    let (atoms, faces) = rest.split_at(atoms);
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    let (source, atoms, faces) = (text(source), text(atoms), text(faces));
    drop(Vec::from_raw_parts(ptr, len, len));
    OUT = exact_web_capabilities::markdown::pieces(&source, &atoms, &faces).into_bytes();
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
  if (taken) { replace(wasm, (temp) => cpSync(taken, temp)); held.set(wasm.split('/').at(-1), true); return wasm; }
  const started = Date.now();
  const r = await run('cargo', [`+${WEB}`, 'build', '--profile', 'web', '--target', 'wasm32-unknown-unknown', '-p', 'exact-textflow', '--bin', 'textflow-web', ...WASM_FLAGS()],
    { cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target } });
  if (r.status !== 0) throw new Error('textflow.wasm did not build');
  const optimizedFlow = await replace(wasm, async (temp) => !unoptimized.has(await optimizeAsync(product, temp, 'textflow.wasm')));
  held.set('textflow.wasm', optimizedFlow && leaf.keep(started));
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
  if (taken) { replace(wasm, (temp) => cpSync(taken, temp)); held.set(wasm.split('/').at(-1), true); return wasm; }
  const started = Date.now();
  const r = await run('cargo', cargoWasmArgs(resolve(crate, 'Cargo.toml')), { env: { ...process.env, CARGO_TARGET_DIR: target } });
  if (r.status !== 0) throw new Error(`${name}.wasm did not build`);
  const optimizedLeaf = await replace(wasm, async (temp) => !unoptimized.has(await optimizeAsync(product, temp, `${name}.wasm`)));
  held.set(`${name}.wasm`, optimizedLeaf && leaf.keep(started));
  return wasm;
}

/** `text` without its `// dev-only {` … `// dev-only }` regions: what a
 * build that is not the dev loop's copies of a runtime file (modes.js's
 * live table, LLP 1121 stage 4), so no release page carries it. */
/** A development build's copy of rt.js with inspect mode's two hooks
 * (LLP 1122 T2b; inspect-dev.js), so rt.js itself, and every other build's
 * copy, has none: an element's view id (`$exactViewId`), and every `each`
 * region with its rows and key function (`$exactEaches`), which a
 * development build's emitter names (regions.rs passes the region id) and
 * which leaves with its scope. Throws when rt.js no longer has the lines
 * they hang on. */
export function withInspectHooks(text) {
  const hooks = [
    [/^export function viewId\(e\) \{.*\n/m, (line) => `${line}globalThis.$exactViewId = viewId;\n`],
    [/^  let rows = new Map\(\), single = false, order = null;\n/m, (line) => `${line}  if (arguments[5] !== undefined) { const e = { region: arguments[5], rows: () => order ?? [], key, live: () => a.isConnected }; (globalThis.$exactEaches ??= new Set()).add(e); onEnd(() => globalThis.$exactEaches.delete(e)); }\n`],
  ];
  for (const [at, hook] of hooks) {
    if (!at.test(text)) throw new Error(`rt.js: inspect mode's hook has no line to hang on (${at})`);
    text = text.replace(at, hook);
  }
  return text;
}
/** B5 (lower.js `RUNTIME`): the arms sheet's host stamp, a digest of the runtime files the build copies first (`files`:
 * `{ path, text }`, lower.js's unstamped source among them), so a runtime whose helpers or rule builder changed under a
 * kept document starts its sheet over, where the arm names (the recipe, its literals, its constants) did not change. */
export const withRuntimeStamp = (text, files) => text.replace('"exact-runtime-unstamped"', JSON.stringify(createHash('sha256').update(files.map(f => `${f.path}\0${f.text}`).join('\0')).digest('hex').slice(0, 24)));
export const withoutDev = (text) => text.replace(/^[ \t]*\/\/ dev-only \{.*\n[\s\S]*?^[ \t]*\/\/ dev-only \}.*\n/gm, '');

/** The plan-to-JavaScript compiler (`exact-web-js`) as `[cmd, args]`: its
 * binary when nothing it was built from changed, one kept for the machine,
 * or `cargo run`, whose binary `done()` keeps. */
const COMPILER = resolve(TARGET_DIR, 'debug/exact-web-js');
const compilerKept = () => kept({ crate: 'exact-web-js', product: COMPILER, depInfo: `${COMPILER}.d`, group: ['debug'], buildDirs: [resolve(TARGET_DIR, 'debug/build')], stamp: resolve(MODULES, 'kept', 'exact-web-js.json') });
/** `fresh`, for a compiler binary: also stale after an edit to the lockfile
 * or to the manifest of the workspace or of any crate it was built from. */
export function freshCompiler(product) {
  try {
    const text = readFileSync(`${product}.d`, 'utf8');
    const files = text.slice(text.indexOf(': ') + 2).trim().split(/(?<!\\)\s+/).map(f => f.replace(/\\ /g, ' '));
    const crates = new Set(files.filter(f => f.lastIndexOf('/src/') > 0).map(f => resolve(f.slice(0, f.lastIndexOf('/src/')), 'Cargo.toml')));
    return fresh(product, `${product}.d`, [resolve(root, 'Cargo.lock'), resolve(root, 'Cargo.toml'), ...crates]);
  } catch { return false; }
}
export function webCompiler() {
  // The optimised compiler (`host-dev`) the JS dev loop built for its design
  // reader: a copy it makes for one build while that build is done and
  // fresh (dev.mjs), the same output, about ten times faster on a large app
  // (the Bluesky clone: 1.2 s, not 11.8).
  const fast = process.env.EXACT_WEB_COMPILER;
  if (fast && existsSync(fast)) return { cmd: fast, pre: [], done() {} };
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

// A game's resident producer and its proof share completed development web
// products locally. The snapshot supplies its own inputs; the live world is never read.
export function warmGameEnvironment(env) {
  // The bake's own environment, which every cargo call's host deployment target is in (scripts/filesystem.mjs).
  return {...withHostDeployment(env), EXACT_UPDATE_TRUST:env.EXACT_UPDATE_TRUST ?? 'development',
    EXACT_GAME_PARANOID:env.EXACT_GAME_PARANOID ?? '0', EXACT_WEB_LINK:env.EXACT_WEB_LINK ?? 'all',
    RUSTUP_TOOLCHAIN:WEB_TOOLCHAIN, EXACT_WEB_SIZE:WEB_SIZE};
}
// Cargo-provided paths and package metadata follow the captured sources. Other
// compiler env-deps must match the caller or a recorded build-script output;
// the script's own watched environment must match the caller in either case.
function warmConfigurationMatches(app, configuration, environment) {
  if (!configuration || configuration.target !== 'wasm32-unknown-unknown'
      || !Array.isArray(configuration.units) || !Array.isArray(configuration.builders)
      || !configuration.units.length || !configuration.rustc || !configuration.flags) return false;
  const env = warmGameEnvironment(environment);
  const hash = value => value == null ? null : createHash('sha256').update(value).digest('hex');
  for (const [key,value] of Object.entries(configuration.flags)) if ((env[key] ?? null) !== value) return false;
  for (const builder of configuration.builders) {
    if (!Array.isArray(builder.environment) || !Array.isArray(builder.env)) return false;
    for (const [key,value] of builder.environment) if (hash(env[key]) !== value) return false;
  }
  for (const unit of configuration.units) {
    if (!Array.isArray(unit.environment)) return false;
    for (const [key,value] of unit.environment) {
      if (key === 'OUT_DIR' && value === `generated:${unit.package}:${unit.role}/`) continue;
      if (key === 'CARGO_MANIFEST_DIR' && /^crate:[^/]+\/$/.test(value ?? '')) continue;
      if (/^CARGO_PKG_(VERSION(?:_MAJOR|_MINOR|_PATCH|_PRE)?|NAME|AUTHORS|DESCRIPTION|HOMEPAGE|REPOSITORY|LICENSE|LICENSE_FILE|RUST_VERSION|README)$/.test(key)) continue;
      const generated = configuration.builders.filter(b => b.package === unit.package && b.role === unit.role)
        .flatMap(b => b.env).filter(([name]) => name === key);
      if (generated.length) { if (generated.some(([,v]) => v !== value)) return false; }
      else if (hash(env[key]) !== value) return false;
    }
  }
  const compiler = spawnSync('rustc', ['-vV'], {cwd:app.workspace ?? root, env, encoding:'utf8'});
  return compiler.status === 0 && compiler.stdout === configuration.rustc;
}

// Cache the owned stage before the directory swap can expose another build's dist.
export function publishWarmGame(app, stage, dist, before, build, env = process.env, publish = replaceBuild) {
  keepWarmGame(app, stage, before, env, build);
  publish(stage, dist);
}
export function warmGameInputs(app, env = process.env) {
  if (!app.manifest.game || env.EXACT_GAME_PROOF_PROFILE !== 'gpu-dev'
      || (env.EXACT_UPDATE_TRUST ?? 'development') !== 'development') return null;
  env = warmGameEnvironment(env);
  const inputs = proofInputs(root, app.dir, resolve(app.target, 'proof-web-inputs.json'));
  const flags = Object.entries(env).filter(([key]) => /^(RUST|CARGO_PROFILE_|CARGO_ENCODED_|CARGO_TARGET_.*_(RUSTFLAGS|LINKER)$|EXACT_GAME_PARANOID_COMPARE$)/.test(key)).sort();
  const configuration = JSON.stringify([app.id, app.manifest, env.EXACT_GAME_PARANOID ?? '0', env.EXACT_UPDATE_ORIGIN ?? null, env.EXACT_BACKEND_ORIGIN ?? null, env.EXACT_WEB_LINK, env.EXACT_WEB_NAMES ?? null, flags]);
  return {...inputs, configuration, key:createHash('sha256').update(inputs.all + configuration).digest('hex')};
}
function warmFiles(directory) {
  const files = [];
  const walk = (dir, prefix = '') => {
    for (const entry of readdirSync(dir, {withFileTypes:true}).sort((a,b) => a.name.localeCompare(b.name))) {
      const name = prefix + entry.name, path = resolve(dir, entry.name);
      if (entry.isDirectory()) walk(path, name + '/');
      else if (entry.isFile()) files.push([name, createHash('sha256').update(readFileSync(path)).digest('hex')]);
      else throw new Error('warm web product contains a non-file');
    }
  };
  walk(directory);
  return files;
}
export function keepWarmGame(app, dist, before, env = process.env, build) {
  if (!before || !warmConfigurationMatches(app, build?.binary?.configuration, env)) return false;
  let stage;
  try {
    const after = warmGameInputs(app, env);
    // A bake may create assets, but may not take credit for moving authored sources.
    if (!after || before.authored !== after.authored || before.authoredState !== after.authoredState
        || before.configuration !== after.configuration) return false;
    const cache = resolve(app.target, 'proof-web'); mkdirSync(cache, {recursive:true});
    stage = mkdtempSync(resolve(cache, 'build-'));
    const copied = resolve(stage, 'dist');
    cpSync(dist, copied, {recursive:true});
    const current = warmGameInputs(app, env);
    if (current?.key !== after.key || current.authoredState !== after.authoredState || current.assetState !== after.assetState) return false;
    const receipt = {key:after.key, directory:stage, files:warmFiles(copied), configuration:build.binary.configuration};
    const path = resolve(cache, `${after.key}.json`);
    let previous;
    try { previous = JSON.parse(readFileSync(path, 'utf8')); } catch {}
    replace(path, temp => writeFileSync(temp, JSON.stringify(receipt)));
    stage = null;
    if (typeof previous?.directory === 'string' && previous.directory !== receipt.directory && dirname(previous.directory) === cache)
      rmSync(previous.directory, {recursive:true,force:true});
    // Evict by receipt publication mtime; this never enters simulation state.
    const entries = readdirSync(cache).filter(name => /^[a-f0-9]{64}\.json$/.test(name)).sort((a,b) => statSync(resolve(cache,b)).mtimeMs - statSync(resolve(cache,a)).mtimeMs);
    for (const name of entries.slice(6)) {
      const old = JSON.parse(readFileSync(resolve(cache,name),'utf8'));
      if (dirname(old.directory) === cache) rmSync(old.directory,{recursive:true,force:true});
      rmSync(resolve(cache,name),{force:true});
    }
    return copied;
  } catch (error) { console.warn(`web: warm game product not kept (${error.message})`); return false; }
  finally { if (stage) rmSync(stage, {recursive:true,force:true}); }
}
export function takeWarmGame(app, dist, env = process.env) {
  const inputs = warmGameInputs(app, env);
  if (!inputs) return false;
  let stage;
  try {
    const cache = resolve(app.target, 'proof-web');
    const receipt = JSON.parse(readFileSync(resolve(cache, `${inputs.key}.json`), 'utf8'));
    if (receipt.key !== inputs.key || dirname(receipt.directory) !== cache
        || !warmConfigurationMatches(app, receipt.configuration, env)) return false;
    stage = mkdtempSync(resolve(app.target, 'proof-web-copy-'));
    cpSync(resolve(receipt.directory, 'dist'), stage, {recursive:true});
    if (JSON.stringify(warmFiles(stage)) !== JSON.stringify(receipt.files)) return false;
    // Driver staleness paths are local to the captured source tree. Relocate only
    // after the complete portable input identity and copied products agree.
    // The caller refreshes the driver's path-based marker after this copy.
    rmSync(dist, {recursive:true,force:true});
    renameSync(stage, dist); stage = null;
    console.log('BUILD warm web producer: matching input identity');
    return true;
  } catch { return false; }
  finally { if (stage) rmSync(stage, {recursive:true,force:true}); }
}

if (import.meta.main) {
  if (process.argv.includes('--prebuild')) await prebuild();
  else {
    const [app] = process.argv.slice(2);
    const o = process.argv.indexOf('--out');
    console.log(buildModule(app, o < 0 ? undefined : process.argv[o + 1]));
  }
}
