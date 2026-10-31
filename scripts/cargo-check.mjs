import { existsSync, mkdirSync, readFileSync, renameSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** What `resolveApp` asked Cargo of an outside app's authored workspace (`cargo metadata --locked`, ~0.1 s), asked when a
 * build first reads `app.workspace` (LLP 1153 D2.1: a web verb never does), kept while the files Cargo reads for it are
 * as they were (`cargoInputs`): in this process, and for the next in the app's `.exact/cargo-check.json`. The workspace
 * itself is found from the files (`authoredWorkspace`), never by `cargo locate-project`. */
const outsideChecks = new Map();
/** The files whose size and time decide the answer: the app's crate manifests (`manifests`), a `Cargo.toml` above the
 * app, where a workspace could start; the workspace's manifest, lock and toolchain pin, and each manifest its `members`
 * name, a glob's too; Cargo's configuration, the workspace's, its parents' and `CARGO_HOME`'s; exact2's manifest and pin
 * (`outsideWorkspaceProblems` holds the workspace's to them); and each path package's manifest `cargo metadata` listed,
 * with each `Cargo.toml` above it, where the workspace it inherits from is. */
function cargoInputs(dir, manifests, workspace, packages) {
  const above = [], configs = [];
  for (let at = dirname(dir); at !== dirname(at); at = dirname(at)) above.push(resolve(at, 'Cargo.toml'));
  for (let at = workspace; ; at = dirname(at)) { configs.push(resolve(at, '.cargo/config.toml'), resolve(at, '.cargo/config')); if (at === dirname(at)) break; }
  const home = process.env.CARGO_HOME ?? resolve(homedir(), '.cargo');
  let members = [];
  try {
    members = (Bun.TOML.parse(readFileSync(resolve(workspace, 'Cargo.toml'), 'utf8')).workspace?.members ?? [])
      .flatMap(pattern => [...new Bun.Glob(`${pattern}/Cargo.toml`).scanSync({ cwd: workspace, absolute: true, onlyFiles: true })]).sort();
  } catch { /* an unreadable manifest is the signature's own '-' */ }
  const inherited = new Set();
  for (const manifest of packages) for (let at = dirname(dirname(manifest)); at !== dirname(at); at = dirname(at)) inherited.add(resolve(at, 'Cargo.toml'));
  return [...manifests, ...above, ...['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'].map(file => resolve(workspace, file)), `members ${members.join(' ')}`, ...members,
    ...configs, resolve(home, 'config.toml'), resolve(home, 'config'), resolve(ROOT, 'Cargo.toml'), resolve(ROOT, 'rust-toolchain.toml'), ...packages, ...[...inherited].sort()];
}
/** Each file's size and time, or that it is missing; a line that names no file (`members …`) as it is. */
const fileSignature = (paths) => paths.map(path => { if (path.startsWith('members ')) return path; try { const s = statSync(path); return `${s.size}:${s.mtimeMs}`; } catch { return '-'; } }).join('\n');
export function knownWorkspace(dir, outside, manifests) {
  const key = `${dir}\0${outside}`;
  try {
    const known = outsideChecks.get(key) ?? JSON.parse(readFileSync(resolve(dir, '.exact/cargo-check.json'), 'utf8'));
    if (known.key !== key || typeof known.workspace !== 'string' || !Array.isArray(known.packages)) return null;
    if (known.signature !== fileSignature(cargoInputs(dir, manifests, known.workspace, known.packages))) return null;
    outsideChecks.set(key, known);
    return known.workspace;
  } catch { return null; }
}
/** Remembers what `check` found for `workspace` (the path packages' manifests), unless a file it read may have changed
 * while it ran, when Cargo may have read either version: the inputs it was known to read (with the path packages the
 * last check found) differ before and after it, a path package's manifest is gone, or any input was written since it
 * started (a second's slack for the clock). */
export function rememberWorkspace(dir, outside, manifests, workspace, check) {
  const key = `${dir}\0${outside}`;
  let earlier = outsideChecks.get(key)?.packages;
  try { earlier ??= JSON.parse(readFileSync(resolve(dir, '.exact/cargo-check.json'), 'utf8')).packages; } catch { /* none yet */ }
  if (!Array.isArray(earlier)) earlier = [];
  const started = Date.now(), before = fileSignature(cargoInputs(dir, manifests, workspace, earlier)), packages = check();
  if (fileSignature(cargoInputs(dir, manifests, workspace, earlier)) !== before || !packages.every(path => existsSync(path))) return;
  const inputs = cargoInputs(dir, manifests, workspace, packages);
  if (inputs.some(path => { try { return statSync(path).mtimeMs >= started - 1000; } catch { return false; } })) return;
  const known = { key, workspace, packages, signature: fileSignature(inputs) };
  outsideChecks.set(key, known);
  try { mkdirSync(resolve(dir, '.exact'), { recursive: true }); const at = resolve(dir, `.exact/cargo-check.json.${process.pid}`); writeFileSync(at, JSON.stringify(known)); renameSync(at, resolve(dir, '.exact/cargo-check.json')); } catch { /* asked again next time */ }
}

/** The generated native workspace of an app that authored none (LLP 1153 D1): under its `.exact/`, which is ignored. */
export const shellsDir = (dir) => resolve(dir, '.exact/shells');
const readToml = (path) => { try { return Bun.TOML.parse(readFileSync(path, 'utf8')); } catch { return null; } };
/** The crates an app writes beside app.json. */
export const APP_CRATES = ['apple', 'ios', 'macos', 'linux', 'web', 'windows', 'android', 'data', 'gpu'];
/** Whether the workspace at `root` owns the package at `path`, as Cargo reads it: a `members` glob names it, or a
 * member depends on it by a path under the root (Cargo's automatic membership), and `exclude` (a prefix or a glob)
 * does not name it. Read from the files alone. */
function owns(root, workspace, path) {
  const under = (p) => { const rel = relative(root, p).split(sep).join('/'); return rel && !rel.startsWith('..') ? rel : null; };
  const excluded = (rel) => (workspace.exclude ?? []).some(pattern => { const prefix = pattern.replace(/\/+$/, ''); return rel === prefix || rel.startsWith(prefix + '/') || new Bun.Glob(prefix).match(rel); });
  const rel = under(path);
  if (!rel || excluded(rel)) return false;
  if ((workspace.members ?? []).some(pattern => new Bun.Glob(pattern.replace(/\/+$/, '')).match(rel))) return true;
  // The members' path dependencies under the root, transitively, each a member unless excluded.
  const seen = new Set(), queue = (workspace.members ?? []).flatMap(pattern => [...new Bun.Glob(`${pattern.replace(/\/+$/, '')}/Cargo.toml`).scanSync({ cwd: root, absolute: true, onlyFiles: true })].map(dirname));
  while (queue.length) {
    const member = queue.shift();
    if (seen.has(member)) continue;
    seen.add(member);
    const manifest = readToml(resolve(member, 'Cargo.toml'));
    for (const table of ['dependencies', 'dev-dependencies', 'build-dependencies']) {
      for (const dep of Object.values(manifest?.[table] ?? {})) {
        if (!dep?.path) continue;
        const at = resolve(member, dep.path), depRel = under(at);
        if (!depRel || excluded(depRel)) continue;
        if (at === path) return true;
        queue.push(at);
      }
    }
  }
  return false;
}
/** The authored Cargo workspace an app builds in, read from the files alone (no `cargo locate-project`: a web verb runs
 * no Cargo, LLP 1153 D2.1), as Cargo would find it: the app's own `Cargo.toml` when it declares `[workspace]`; the
 * workspace a package names (`package.workspace`, the app's or a crate's beside app.json); else the nearest enclosing
 * workspace whose `members` own the app or one of its crates; else the app itself, a root of its own package or
 * crates. `null` when the app authored no Cargo manifest at all: its workspace is generated (scripts/app-shells.mjs).
 * An enclosing workspace that does not list the app (another repository the app sits in) is not the app's. */
export function authoredWorkspace(dir) {
  const own = readToml(resolve(dir, 'Cargo.toml'));
  if (own?.workspace !== undefined) return dir;
  if (own?.package?.workspace) return resolve(dir, own.package.workspace);
  const crates = APP_CRATES.map(kind => resolve(dir, kind)).filter(crate => existsSync(resolve(crate, 'Cargo.toml')));
  for (const crate of crates) { const declared = readToml(resolve(crate, 'Cargo.toml'))?.package?.workspace; if (declared) return resolve(crate, declared); }
  if (!own && !crates.length) return null;
  for (let at = dirname(dir); at !== dirname(at); at = dirname(at)) {
    const workspace = readToml(resolve(at, 'Cargo.toml'))?.workspace;
    if (workspace && [dir, ...crates].some(path => owns(at, workspace, path))) return at;
  }
  return dir;
}
