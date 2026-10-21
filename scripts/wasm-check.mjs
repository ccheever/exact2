// The workspace crates a web app links for wasm32, checked for that target (`cargo check --target
// wasm32-unknown-unknown --lib`) when a change touches Rust: caps.mjs runs `wasmCheck` before every push. A native
// build and its tests never compile them for wasm, so a name only a `cfg(unix)` arm gives (G2's gallery, 5540abbfc0)
// broke every wasm web build for 70 minutes, until the lane's wasm loop saw it. Cold, the check takes about a
// minute and a target directory's 300 MB; warm, under a second; with no Rust changed since origin/main, nothing.
// EXACT_CAPS_WASM=0 skips it (a test that runs caps on this checkout for something else).
import { spawnSync } from 'node:child_process';

const TARGET = 'wasm32-unknown-unknown';
// What changes a crate's compile: its sources, a manifest, the lock, a build script's inputs beside them.
const RUST = /(^|\/)(Cargo\.toml|Cargo\.lock|build\.rs|rust-toolchain\.toml)$|\.rs$/;

const run = (root, cmd, args) => spawnSync(cmd, args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });

/** The files changed since `base` (committed or not, and untracked), or null when there is no `base` to compare. */
export function changedSince(root, base = 'origin/main') {
  const fork = run(root, 'git', ['merge-base', 'HEAD', base]);
  if (fork.status !== 0) return null;
  const diff = run(root, 'git', ['diff', '--name-only', fork.stdout.trim()]), untracked = run(root, 'git', ['ls-files', '--others', '--exclude-standard']);
  if (diff.status !== 0 || untracked.status !== 0) return null;
  return [...diff.stdout.split('\n'), ...untracked.stdout.split('\n')].filter(Boolean);
}

/** The workspace's library crates in the wasm32 graph of its web apps (`apps/*\/web`), apps and vendored ones aside. */
export function wasmCrates(root) {
  const meta = run(root, 'cargo', ['metadata', '--no-deps', '--offline', '--format-version', '1']);
  if (meta.status !== 0) throw new Error(`cargo metadata failed: ${(meta.stderr ?? '').trim().split('\n').slice(-2).join(' ')}`);
  const apps = JSON.parse(meta.stdout).packages.filter((p) => p.name.endsWith('-web') && /\/apps\/[^/]+\/web\/Cargo\.toml$/.test(p.manifest_path));
  if (!apps.length) return [];
  const tree = run(root, 'cargo', ['tree', ...apps.flatMap((p) => ['-p', p.name]), '--target', TARGET, '-e', 'normal', '--prefix', 'none', '--offline']);
  if (tree.status !== 0) throw new Error(`cargo tree failed: ${(tree.stderr ?? '').trim().split('\n').slice(-2).join(' ')}`);
  const local = tree.stdout.split('\n').map((l) => /^(\S+) v\S+ \((\/[^)]+)\)/.exec(l)).filter(Boolean)
    .filter(([, , dir]) => !/\/(apps|vendor)\//.test(dir + '/'));
  return [...new Set(local.map(([, name]) => name))].sort();
}

/** `{ problems: [{ code, message, detail }], counted }`, as caps.mjs reports them. */
export function wasmCheck(root, { env = process.env, changed = changedSince(root) } = {}) {
  if (env.EXACT_CAPS_WASM === '0') return { problems: [], counted: 'wasm32: skipped (EXACT_CAPS_WASM=0)' };
  const rust = changed?.filter((f) => RUST.test(f));
  if (rust && !rust.length) return { problems: [], counted: 'wasm32: no Rust changed since origin/main, not checked' };
  let crates;
  try { crates = wasmCrates(root); } catch (e) {
    return { problems: [{ code: 'wasm:check', message: `the web apps' wasm32 crates could not be listed, so none was checked: ${e.message}` }], counted: 'wasm32: not checked' };
  }
  if (!crates.length) return { problems: [], counted: 'wasm32: no web app crates' };
  const check = run(root, 'cargo', ['check', '--target', TARGET, '--lib', '--message-format', 'short', ...crates.flatMap((c) => ['-p', c])]);
  const why = changed ? `Rust changed since origin/main: ${rust.length} files` : 'no origin/main to compare';
  if (check.status === 0) return { problems: [], counted: `wasm32: ${crates.length} crates the web apps link checked for ${TARGET} (${why}), 0 errors` };
  const errors = (check.stderr ?? '').split('\n').filter((l) => /: error(\[E\d+\])?:/.test(l)); // each diagnostic, at its file
  return {
    problems: [{ code: 'wasm:check', message: `cargo check --target ${TARGET} failed for the crates the web apps link: ${errors.length} errors.`,
      detail: `${errors.slice(0, 6).join('\n    ') || (check.stderr ?? '').trim().split('\n').slice(-4).join('\n    ')}\n    A name only a cfg(unix) or cfg(windows) arm gives needs an arm for neither; rerun: cargo check --target ${TARGET} --lib -p <crate>` }],
    counted: `wasm32: ${crates.length} crates checked (${why}), ${errors.length} errors`,
  };
}
