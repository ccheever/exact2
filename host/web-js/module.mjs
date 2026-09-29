// An app's Rust data source as a standalone logic module (LLP 1029.000,
// ABI 3) for the JS runtime, when the app declares none of its own: a
// generated crate exporting the same DataSource the web build bakes with
// (`apps/<app>/web/build.rs`'s `contract::bake(plan, <expr>)`).
// usage: bun host/web-js/module.mjs <app> [--out <dir>] → <dir>/app.module.wasm
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
// The web toolchain's size build (scripts/app.mjs WEB_TOOLCHAIN): std built
// for size, panics abort without text.
const WEB = /WEB_TOOLCHAIN\s*=\s*'([^']+)'/.exec(readFileSync(resolve(root, 'scripts/app.mjs'), 'utf8'))?.[1];
// Under this checkout's own target/ (AGENTS.md: never another checkout's):
// a module built from one worktree's sources is never another's.
const MODULES = resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'web-js-modules');
const TARGET = resolve(MODULES, 'target');
function cargoWasm(manifest) {
  return spawnSync('cargo', [...(WEB ? [`+${WEB}`] : []), 'build', '--release', '--target', 'wasm32-unknown-unknown', '--manifest-path', manifest,
    ...(WEB ? ['-Zbuild-std=std,panic_abort', '-Zbuild-std-features=optimize_for_size'] : [])],
  { stdio: ['ignore', 'inherit', 'inherit'], env: { ...process.env, CARGO_TARGET_DIR: TARGET, ...(WEB ? { RUSTFLAGS: '-Zunstable-options -Cpanic=immediate-abort -Zlocation-detail=none' } : {}) } });
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
/** Cargo's module at `wasm`, optimized to `<out>/app.module.wasm` unless
 * that is still newer than everything the module was built from. */
function module(manifest, name, out, label) {
  const wasm = resolve(TARGET, 'wasm32-unknown-unknown/release', `${name}.wasm`), done = resolve(out, 'app.module.wasm');
  mkdirSync(out, { recursive: true });
  if (fresh(done, wasm.replace(/\.wasm$/, '.d'), [manifest])) return done;
  if (cargoWasm(manifest).status !== 0) throw new Error(`${label} did not build`);
  cpSync(wasm, done);
  const opt = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', done, '-o', done]);
  if (opt.status !== 0) process.stderr.write('module: wasm-opt unavailable or failed; unoptimized\n');
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
  if (!existsSync(resolve(crate, 'Cargo.lock'))) cpSync(resolve(root, 'Cargo.lock'), resolve(crate, 'Cargo.lock'));
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

/** Markdown pieces (LLP 1045 D3) as a loaded capability for the runtime:
 * the web host's own `exact_web_capabilities::markdown::pieces`, exported
 * alone from a wasm module the page fetches on its first Markdown node. */
export function buildMarkdown(out = resolve(MODULES, 'markdown')) {
  const crate = resolve(out, 'crate');
  mkdirSync(resolve(crate, 'src'), { recursive: true });
  const ws = readFileSync(resolve(root, 'Cargo.toml'), 'utf8');
  const patch = ws.slice(ws.indexOf('[patch.crates-io]')).split('\n[')[0].replace(/path = "vendor/g, `path = "${root}/vendor`);
  writeFileSync(resolve(crate, 'Cargo.toml'), `[package]
name = "exact-js-markdown"
version = "0.1.0"
edition = "2021"
publish = false
[lib]
crate-type = ["cdylib"]
[dependencies]
exact-web-capabilities = { path = "${resolve(root, 'host/web-capabilities')}" }
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
  writeFileSync(resolve(crate, 'src/lib.rs'), `//! Generated: Markdown pieces for the JS runtime.
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
`);
  if (!existsSync(resolve(crate, 'Cargo.lock'))) cpSync(resolve(root, 'Cargo.lock'), resolve(crate, 'Cargo.lock'));
  const r = cargoWasm(resolve(crate, 'Cargo.toml'));
  if (r.status !== 0) throw new Error('markdown.wasm did not build');
  cpSync(resolve(TARGET, 'wasm32-unknown-unknown/release/exact_js_markdown.wasm'), resolve(out, 'markdown.wasm'));
  spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', resolve(out, 'markdown.wasm'), '-o', resolve(out, 'markdown.wasm')]);
  return resolve(out, 'markdown.wasm');
}

/** The motion capability (LLP 1071 §7): `exact-web-js-motion`'s exports,
 * the one `exact_motion` Engine, as a wasm module the page fetches after
 * first paint when its plan uses motion. */
export function buildMotion(out = resolve(MODULES, 'motion')) {
  return buildLeaf('motion', 'exact-web-js-motion', 'host/web-js/motion', out);
}

/** The Markdown editor's rules (LLP 1045 D5), the crate the wasm host's
 * build makes its `markup-editor.wasm` from. */
export function buildEditor(out = resolve(MODULES, 'editor')) {
  return buildLeaf('markup-editor', 'exact-markdown-editor', 'markdown/editor', out);
}

/** The exclusions walker (LLP 1043.000 §3 D7): `exact-textflow`'s
 * `textflow-web` binary, as the wasm build builds its `textflow.wasm`. */
export function buildFlow(out = resolve(MODULES, 'flow')) {
  mkdirSync(out, { recursive: true });
  // Built again only when something it was built from changed (`fresh`).
  const target = resolve(MODULES, 'target-flow'), built = resolve(target, 'wasm32-unknown-unknown/web/textflow-web.wasm'), wasm = resolve(out, 'textflow.wasm');
  if (fresh(wasm, built.replace(/\.wasm$/, '.d'))) return wasm;
  const r = spawnSync('cargo', [...(WEB ? [`+${WEB}`] : []), 'build', '--profile', 'web', '--target', 'wasm32-unknown-unknown', '-p', 'exact-textflow', '--bin', 'textflow-web',
    ...(WEB ? ['-Zbuild-std=std,panic_abort', '-Zbuild-std-features=optimize_for_size'] : [])],
  { cwd: root, stdio: ['ignore', 'inherit', 'inherit'], env: { ...process.env, CARGO_TARGET_DIR: target, ...(WEB ? { RUSTFLAGS: '-Zunstable-options -Cpanic=immediate-abort -Zlocation-detail=none' } : {}) } });
  if (r.status !== 0) throw new Error('textflow.wasm did not build');
  spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', built, '-o', wasm]);
  return wasm;
}

// A workspace crate's exports as a size-built wasm module: `<name>.wasm`.
function buildLeaf(name, pkg, dir, out) {
  const crate = resolve(out, 'crate');
  mkdirSync(resolve(crate, 'src'), { recursive: true });
  const ws = readFileSync(resolve(root, 'Cargo.toml'), 'utf8');
  keep(resolve(crate, 'Cargo.toml'), `[package]
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
`);
  keep(resolve(crate, 'src/lib.rs'), `//! Generated: ${name}.wasm for the JS runtime.\npub use ${pkg.replaceAll('-', '_')}::*;\n`);
  if (!existsSync(resolve(crate, 'Cargo.lock'))) cpSync(resolve(root, 'Cargo.lock'), resolve(crate, 'Cargo.lock'));
  // Built again only when something it was built from changed (`fresh`).
  const built = resolve(TARGET, 'wasm32-unknown-unknown/release', `exact_js_${name.replaceAll('-', '_')}.wasm`), wasm = resolve(out, `${name}.wasm`);
  if (fresh(wasm, built.replace(/\.wasm$/, '.d'), [resolve(crate, 'Cargo.toml')])) return wasm;
  const r = cargoWasm(resolve(crate, 'Cargo.toml'));
  if (r.status !== 0) throw new Error(`${name}.wasm did not build`);
  cpSync(built, wasm);
  spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', wasm, '-o', wasm]);
  return wasm;
}

if (import.meta.main) {
  const [app] = process.argv.slice(2);
  const o = process.argv.indexOf('--out');
  console.log(buildModule(app, o < 0 ? undefined : process.argv[o + 1]));
}
