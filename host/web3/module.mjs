// An app's Rust data source as a standalone logic module (LLP 1029.000,
// ABI 3) for the exact3 runtime, when the app declares none of its own: a
// generated crate exporting the same DataSource the web build bakes with
// (`apps/<app>/web/build.rs`'s `contract::bake(plan, <expr>)`).
// usage: bun host/web3/module.mjs <app> [--out <dir>] → <dir>/app.module.wasm
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');

export function buildModule(app, out = `/tmp/e3-mod/${app}`) {
  const build = readFileSync(resolve(root, 'apps', app, 'web/build.rs'), 'utf8');
  const expr = /contract::bake\(\s*plan,\s*([\w:]+(?:::(?:default|new)\(\))?)\s*\)/.exec(build)?.[1];
  if (!expr) throw new Error(`${app}: its web build bakes with no Rust data source`);
  const ty = expr.replace(/::(default|new)\(\)$/, '');
  const dataToml = readFileSync(resolve(root, 'apps', app, 'data/Cargo.toml'), 'utf8');
  const pkg = /^name\s*=\s*"([^"]+)"/m.exec(dataToml)[1];
  const crate = resolve(out, 'crate');
  mkdirSync(resolve(crate, 'src'), { recursive: true });
  const ws = readFileSync(resolve(root, 'Cargo.toml'), 'utf8');
  const patch = ws.slice(ws.indexOf('[patch.crates-io]')).split('\n[')[0].replace(/path = "vendor/g, `path = "${root}/vendor`);
  writeFileSync(resolve(crate, 'Cargo.toml'), `[package]
name = "exact3-module-${app}"
version = "0.1.0"
edition = "2021"
publish = false
[lib]
crate-type = ["cdylib"]
[dependencies]
${pkg} = { path = "${resolve(root, 'apps', app, 'data')}" }
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
  writeFileSync(resolve(crate, 'src/lib.rs'), `exact_logic_abi::export!(${ty}, ${expr});\n`);
  if (!existsSync(resolve(crate, 'Cargo.lock'))) cpSync(resolve(root, 'Cargo.lock'), resolve(crate, 'Cargo.lock'));
  const r = spawnSync('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown', '--manifest-path', resolve(crate, 'Cargo.toml')],
    { stdio: ['ignore', 'inherit', 'inherit'], env: { ...process.env, CARGO_TARGET_DIR: '/tmp/e3-mod/target' } });
  if (r.status !== 0) throw new Error(`${app}: the module did not build`);
  const wasm = `/tmp/e3-mod/target/wasm32-unknown-unknown/release/exact3_module_${app.replaceAll('-', '_')}.wasm`;
  cpSync(wasm, resolve(out, 'app.module.wasm'));
  const opt = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', resolve(out, 'app.module.wasm'), '-o', resolve(out, 'app.module.wasm')]);
  if (opt.status !== 0) process.stderr.write('module: wasm-opt unavailable or failed; unoptimized\n');
  return resolve(out, 'app.module.wasm');
}

if (import.meta.main) {
  const [app] = process.argv.slice(2);
  const o = process.argv.indexOf('--out');
  console.log(buildModule(app, o < 0 ? undefined : process.argv[o + 1]));
}
