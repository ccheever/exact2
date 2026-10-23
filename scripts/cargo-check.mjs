import { existsSync, mkdirSync, readFileSync, renameSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** What `resolveApp` asked Cargo of an outside app (`cargo locate-project`, then `cargo metadata --locked`, ~0.1 s a
 * call), kept while the files Cargo reads for it are as they were (`cargoInputs`): in this process, and for the next in
 * the app's `.exact/cargo-check.json`. */
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
