// The workspace crates a web app links for wasm32, checked for that target (`cargo check --target
// wasm32-unknown-unknown`): caps.mjs runs `wasmCheck` before every push. A native build and its tests never compile
// them for wasm, so a name only a `cfg(unix)` arm gives (G2's gallery, 5540abbfc0) broke every wasm web build for 70
// minutes, until the lane's wasm loop saw it.
//
// When it runs: when the target directory already holds this check, always (Cargo's fingerprints then decide what
// changed, `include_str!` inputs and build scripts' too, and a check with nothing to do takes under a second); in a
// target directory with none yet, when a change since origin/main touches Rust, Cargo or the toolchain, or any file
// but a test inside a checked crate's directory (cold, the check takes about a minute and 300 MB, so a push of
// scripts or docs alone does not pay it; an `include_str!` input outside the crates, docs/recipes' say, waits for the
// next check here or the lane's); with no origin/main, always. What it leaves out is the lane's: the app crates themselves (two minutes cold, Caltrain's data) and the
// GPU leaves (wgpu, three). EXACT_CAPS_WASM=0 skips it (a test that runs caps on this checkout for something else).
import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { assertOwnTarget } from './app.mjs';

const TARGET = 'wasm32-unknown-unknown';
// What a cold target directory waits for: Rust, a manifest, the lock, a build script, the toolchain, Cargo's config.
const RUST = /(^|\/)(Cargo\.toml|Cargo\.lock|build\.rs|rust-toolchain(\.toml)?)$|\.rs$|(^|\/)\.cargo\//;
// Leaves the JS target builds for wasm beside the apps' graph (host/web-js/module.mjs, build.mjs): the markdown
// editor, the motion module and the text flow's binary. (A second on top of the apps' graph; the GPU leaves are not.)
const LIBS = ['exact-markdown-editor', 'exact-web-js-motion'];
const BINS = [['exact-textflow', 'textflow-web']];

/** A command's result, or a throw naming it: a spawn error, or output past the buffer (ENOBUFS, which can come
 * with status 0 and output cut short), is never a result. */
function run(root, cmd, args, { ok = [0] } = {}) {
  const r = spawnSync(cmd, args, { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (r.error) throw new Error(`${cmd} ${args[0]}: ${r.error.message}`);
  if (!ok.includes(r.status)) throw new Error(`${cmd} ${args[0]} exited ${r.status}: ${(r.stderr ?? '').trim().split('\n').slice(-3).join(' ')}`);
  return r;
}

/** The files changed since `base` (committed or not, and untracked; a rename as both its names), or null when there
 * is no `base` to compare. */
export function changedSince(root, base = 'origin/main') {
  let fork;
  try { fork = run(root, 'git', ['merge-base', 'HEAD', base]).stdout.trim(); } catch { return null; }
  const diff = run(root, 'git', ['diff', '--name-only', '--no-renames', '-z', fork]).stdout;
  const untracked = run(root, 'git', ['ls-files', '--others', '--exclude-standard', '-z']).stdout;
  return [...diff.split('\0'), ...untracked.split('\0')].filter(Boolean);
}

/** The workspace's library crates in the wasm32 graph of its web apps (`apps/<app>/web`), the apps and vendored
 * crates aside; and Cargo's target directory. Throws rather than answering none. */
export function wasmCrates(root) {
  const meta = JSON.parse(run(root, 'cargo', ['metadata', '--no-deps', '--offline', '--format-version', '1']).stdout);
  const top = meta.workspace_root.replaceAll('\\', '/'), members = new Set(meta.workspace_members);
  const rel = (p) => p.manifest_path.replaceAll('\\', '/').slice(top.length + 1, -'/Cargo.toml'.length);
  const local = meta.packages.filter((p) => members.has(p.id)).map((p) => ({ name: p.name, dir: rel(p), p }));
  const apps = local.filter(({ dir }) => /^apps\/[^/]+\/web$/.test(dir));
  if (!apps.length) throw new Error('no apps/<app>/web crate in the workspace');
  const tree = run(root, 'cargo', ['tree', ...apps.flatMap((a) => ['-p', a.name]), '--target', TARGET, '-e', 'normal', '--prefix', 'none', '--offline', '--locked']);
  const named = new Set(tree.stdout.split('\n').map((l) => /^(\S+) v/.exec(l)?.[1]).filter(Boolean));
  const gone = LIBS.filter((name) => !local.some((c) => c.name === name));
  if (gone.length) throw new Error(`a wasm leaf is gone: ${gone.join(', ')}`);
  const crates = local.filter(({ name, dir }) => (named.has(name) || LIBS.includes(name)) && !/^(apps|vendor)\//.test(dir))
    .filter(({ p }) => p.targets.some((t) => t.kind.some((k) => k.endsWith('lib')))).map(({ name }) => name).sort();
  if (!crates.includes('exact-web')) throw new Error(`the web apps' wasm32 graph has no exact-web (found ${crates.length} crates)`);
  const bins = BINS.filter(([pkg, bin]) => meta.packages.some((p) => members.has(p.id) && p.name === pkg && p.targets.some((t) => t.name === bin)));
  if (bins.length !== BINS.length) throw new Error(`a wasm leaf is gone: ${BINS.filter((b) => !bins.includes(b)).map(([p, b]) => `${p} --bin ${b}`).join(', ')}`);
  const dirs = local.filter(({ name }) => crates.includes(name) || bins.some(([pkg]) => pkg === name)).map(({ dir }) => dir + '/');
  return { crates, bins, dirs, target: meta.target_directory };
}

/** Whether `target` holds this check's output already (Cargo's fingerprints for wasm32's debug profile). */
const warm = (target) => {
  const prints = resolve(target, TARGET, 'debug', '.fingerprint');
  return existsSync(prints) && readdirSync(prints).some((d) => d.startsWith('exact-web-'));
};

/** `{ problems: [{ code, message, detail }], counted }`, as caps.mjs reports them. */
export function wasmCheck(root, { env = process.env, changed, hot, dry = false } = {}) {
  if (env.EXACT_CAPS_WASM === '0') return { problems: [], counted: 'wasm32: skipped (EXACT_CAPS_WASM=0)' };
  const problem = (message, detail) => ({ problems: [{ code: 'wasm:check', message, detail }], counted: 'wasm32: not checked' });
  let found;
  try {
    found = wasmCrates(root);
    assertOwnTarget(found.target, root);
    if (changed === undefined) changed = changedSince(root);
  } catch (e) { return problem(`the crates the web apps link could not be listed, so none was checked for wasm32: ${e.message}`); }
  const rust = changed?.map((f) => f.replaceAll('\\', '/'))
    .filter((f) => RUST.test(f) || found.dirs.some((d) => f.startsWith(d)) && !/(^|\/)tests\/|\.test\.[cm]?[jt]s$/.test(f));
  hot ??= warm(found.target);
  if (!hot && rust && !rust.length) return { problems: [], counted: 'wasm32: no Rust changed since origin/main and no earlier check in this target directory, not checked' };
  const why = hot ? 'Cargo decides what changed' : rust ? `Rust or a crate's files changed since origin/main: ${rust.length}` : 'no origin/main to compare';
  if (dry) return { problems: [], counted: `wasm32: would check (${why})` }; // a test's: when it runs, not the compile
  const results = [];
  try {
    results.push(run(root, 'cargo', ['check', '--offline', '--locked', '--target', TARGET, '--lib', '--keep-going', '--message-format', 'short', ...found.crates.flatMap((c) => ['-p', c])], { ok: [0, 101] }));
    for (const [pkg, bin] of found.bins) results.push(run(root, 'cargo', ['check', '--offline', '--locked', '--target', TARGET, '-p', pkg, '--bin', bin, '--message-format', 'short'], { ok: [0, 101] }));
  } catch (e) { return problem(`cargo check --target ${TARGET} could not run, so nothing was checked: ${e.message}`); }
  const count = `${found.crates.length} crates and ${found.bins.length} binary the web apps link`;
  if (results.every((r) => r.status === 0)) return { problems: [], counted: `wasm32: ${count} checked for ${TARGET} (${why}), 0 errors` };
  const stderr = results.map((r) => r.stderr ?? '').join('\n').split('\n');
  // Each diagnostic, at its file or not (`error: …` from Cargo or a build script), but not Cargo's per-crate summary
  const errors = [...new Set(stderr.filter((l) => /(^|: )error(\[E\d+\])?:/.test(l) && !/^error: (could not compile|aborting|build failed)/.test(l)))];
  const failed = [...new Set(stderr.map((l) => /could not compile `([^`]+)`/.exec(l)?.[1]).filter(Boolean))];
  const cfg = errors.some((l) => /error\[E0(425|433|412|432)\]/.test(l)) ? ' A name only a cfg(unix) or cfg(windows) arm gives needs an arm for neither.' : '';
  return {
    problems: [{ code: 'wasm:check', message: `cargo check --target ${TARGET} failed for the crates the web apps link: ${errors.length} error${errors.length === 1 ? '' : 's'}${failed.length ? ` in ${failed.join(', ')}` : ''}.${cfg}`,
      detail: `${(errors.length ? errors : stderr.filter(Boolean).slice(-6)).join('\n    ')}\n    Rerun: cargo check --target ${TARGET} --lib${(failed.length ? failed : found.crates).map((c) => ` -p ${c}`).join('')}` }],
    counted: `wasm32: ${count} checked (${why}), ${errors.length} error${errors.length === 1 ? '' : 's'}`,
  };
}
