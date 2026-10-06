/**
 * Build products kept for the machine (LLP 1036.000 §10): the Apple host's
 * Rust modules (host/apple/modules.mjs), the web's leaf wasm modules and its
 * compiler (host/web-js/module.mjs). Each holds nothing of an app, so every
 * checkout whose sources agree builds the same bytes, and a build in a
 * checkout that has not compiled one takes the one another checkout kept in
 * `~/.cache/exact/<cache>`.
 *
 * What says two builds are the same:
 * - the group: the crate and what the caller adds (target, profile, the flags
 *   and generated manifest it builds with), `rustc -vV`, the crate's closure
 *   in `Cargo.lock` (each registry crate by checksum), the workspace manifest
 *   apart from its member list, Cargo's config files, the variables a compile
 *   reads, and the tools the caller names;
 * - the entry: every file and directory in Cargo's dep-info for the product
 *   (the path crates' sources, their build scripts' inputs), each of those
 *   crates' manifests, and each variable a build script asked to rerun for.
 *
 * Only committed sources are kept, so an edit loop does not push out the
 * entries other checkouts would take. A checkout that has compiled a product
 * itself goes on asking Cargo, whose answer costs it nothing.
 */
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, relative, resolve } from 'node:path';

/** Entries kept per group, and groups kept: at 5 MB an entry, under 1 GB. */
const KEPT = 6, GROUPS = 24;
/** The variables a compile or a link reads, whatever a build script says. */
const READS = /^(CARGO_(PROFILE|BUILD|ENCODED)_|CARGO_TARGET_.+_(RUSTFLAGS|LINKER)$|RUST(C|FLAGS|UP_TOOLCHAIN)|[A-Z]+_DEPLOYMENT_TARGET$|(SDKROOT|DEVELOPER_DIR|CC|CXX|CFLAGS|CXXFLAGS|AR)$)/;

const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const text = (path) => { try { return readFileSync(path, 'utf8'); } catch { return null; } };
const newestFirst = (dir) => { try { return readdirSync(dir).filter(n => !n.includes('.')).map(n => [statSync(resolve(dir, n)).mtimeMs, n]).sort((a, b) => b[0] - a[0]).map(e => e[1]); } catch { return []; } };
/** Every file at or under `path` as `[name, stat, path]`; `null` when there is nothing there. */
function under(path, name = '') {
  let stat;
  try { stat = statSync(path); } catch { return null; }
  if (!stat.isDirectory()) return [[name, stat, path]];
  return readdirSync(path).sort().flatMap(child => under(resolve(path, child), `${name}/${child}`) ?? []);
}
const digest = (path) => { const all = under(path); return all && sha(all.map(([name, , file]) => `${name}\0${sha(readFileSync(file))}`).join('\n')); };
const seen = (path) => under(path)?.map(([name, s]) => `${name}:${s.mtimeMs}:${s.size}`).join('\n') ?? null;

/**
 * The kept products of one cache. `kept(spec)` is `{ find(), keep(started) }`
 * for one product, where `spec` says:
 * - `crate`: the package whose `Cargo.lock` closure pins it;
 * - `product`: what Cargo writes; while it exists the checkout asks Cargo, so
 *   `find` answers `null`;
 * - `depInfo`: Cargo's dep-info for it;
 * - `file`: what is kept and found (`product` unless the caller transforms
 *   it, as `wasm-opt` does a leaf module);
 * - `group`: what else decides its bytes, beside the crate;
 * - `buildDirs`: the build-script directories whose `rerun-if-env-changed`
 *   variables it reads;
 * - `toolchain`: the `+toolchain` its `rustc` runs as, if not the checkout's;
 * - `stamp`: where a checkout notes the entry it took.
 * A compile that `keep` is told of leaves a receipt beside `file` (when it
 * started, and the group): `keepAgain()` keeps a product built earlier only
 * on that receipt, never on the product's own time, and is `null` without
 * one. Both say whether the cache now holds the product for these sources.
 * `skip` names directories of generated sources (the target directories) a
 * dep-info may list; a source outside the checkout otherwise keeps nothing.
 */
export function keptProducts({ root, cache: name, env, tools = [], skip = [], label }) {
  if (typeof Bun === 'undefined') return () => ({ find: () => null, keep: () => false, keepAgain: () => null });
  const cache = resolve(env.HOME ?? homedir(), '.cache/exact', name);
  const reads = () => Object.entries(env).filter(([k]) => READS.test(k)).sort();
  // Every configuration file Cargo would read for a build in the checkout: each
  // ancestor's `.cargo/config` and `.cargo/config.toml`, then Cargo's home's.
  const ancestors = []; for (let dir = root; ; dir = dirname(dir)) { ancestors.push(dir); if (dirname(dir) === dir) break; }
  const configs = [...ancestors.map(dir => resolve(dir, '.cargo')), env.CARGO_HOME ?? resolve(homedir(), '.cargo')].flatMap(dir => [resolve(dir, 'config'), resolve(dir, 'config.toml')]);
  const rustcs = new Map();
  /** The crate's packages in `Cargo.lock`, each with what pins its bytes. */
  const closure = (crate) => {
    const packages = Bun.TOML.parse(readFileSync(resolve(root, 'Cargo.lock'), 'utf8')).package ?? [], pinned = new Set(), walk = [crate];
    while (walk.length) {
      const [name, version] = walk.pop().split(' '), pkg = packages.find(p => p.name === name && (!version || p.version === version));
      if (!pkg) throw new Error(`Cargo.lock has no ${name}`);
      const id = `${pkg.name} ${pkg.version} ${pkg.checksum ?? pkg.source ?? 'path'}`;
      if (!pinned.has(id)) { pinned.add(id); walk.push(...(pkg.dependencies ?? [])); }
    }
    return [...pinned].sort();
  };
  const group = (spec) => {
    const toolchain = spec.toolchain ? [`+${spec.toolchain}`] : [];
    if (!rustcs.has(spec.toolchain)) rustcs.set(spec.toolchain, spawnSync('rustc', [...toolchain, '-vV'], { cwd: root, env, encoding: 'utf8' }).stdout);
    const workspace = Bun.TOML.parse(readFileSync(resolve(root, 'Cargo.toml'), 'utf8'));
    for (const members of ['members', 'exclude', 'default-members']) delete workspace.workspace?.[members];
    return sha(JSON.stringify([1, spec.crate, ...(spec.group ?? []), rustcs.get(spec.toolchain), closure(spec.crate), workspace, configs.map(text), reads(), tools])).slice(0, 24);
  };
  /** What a checkout that took an entry saw of its inputs: while that is unchanged, the search is not repeated. */
  const glance = (kept, spec) => sha(JSON.stringify([[spec.crate, spec.group ?? [], spec.toolchain ?? null], kept.files.map(([file]) => seen(resolve(root, file))), kept.env.map(([k]) => env[k] ?? null),
    ['Cargo.lock', 'Cargo.toml', 'rust-toolchain.toml'].map(file => seen(resolve(root, file))), configs.map(seen), seen(resolve(env.RUSTUP_HOME ?? resolve(homedir(), '.rustup'), 'settings.toml')), reads(), tools]));
  /** The product's inputs from Cargo's dep-info, relative to the checkout; `null` when one is outside it. */
  const inputs = (spec) => {
    const line = (text(spec.depInfo) ?? '').split('\n').find(l => l.includes(': '));
    if (!line) return null;
    const files = new Set();
    for (const raw of line.slice(line.indexOf(': ') + 2).split(/(?<!\\) /)) {
      const file = resolve(raw.replace(/\\ /g, ' '));
      if (skip.some(dir => file.startsWith(dir + '/'))) continue;
      if (relative(root, file).startsWith('..')) return null;
      files.add(relative(root, file));
      for (let dir = dirname(file); dir !== root && dir.startsWith(root); dir = dirname(dir)) if (existsSync(resolve(dir, 'Cargo.toml'))) { files.add(relative(root, resolve(dir, 'Cargo.toml'))); break; }
    }
    return [...files].sort();
  };
  /** Each variable a build script of the product asked to be rerun for. */
  const asked = (spec) => {
    const names = new Set();
    for (const builds of spec.buildDirs ?? []) for (const dir of existsSync(builds) ? readdirSync(builds) : []) {
      for (const m of (text(resolve(builds, dir, 'output')) ?? '').matchAll(/^cargo::?rerun-if-env-changed=(.+)$/gm)) names.add(m[1]);
    }
    return [...names].sort();
  };
  return (spec) => {
    const file = spec.file ?? spec.product, stored = basename(file);
    const entryOf = (entry) => existsSync(resolve(entry, stored)) ? JSON.parse(text(resolve(entry, 'inputs.json')) ?? 'null') : null;
    return {
      find() {
        try {
          if (existsSync(spec.product)) return null;
          const last = JSON.parse(text(spec.stamp) ?? 'null'), again = last && entryOf(last.entry);
          if (again && glance(again, spec) === last.glance) {
            // Taken again: the newest, so another checkout's keep does not evict it under this build.
            const now = new Date();
            utimesSync(last.entry, now, now); utimesSync(dirname(last.entry), now, now);
            return resolve(last.entry, stored);
          }
          const dir = resolve(cache, group(spec));
          for (const name of newestFirst(dir)) {
            const entry = resolve(dir, name), kept = entryOf(entry);
            if (!kept || !kept.files.every(([file, bytes]) => digest(resolve(root, file)) === bytes) || !kept.env.every(([k, v]) => (env[k] ?? null) === v)) continue;
            const now = new Date();
            utimesSync(entry, now, now); utimesSync(dir, now, now);
            mkdirSync(dirname(spec.stamp), { recursive: true });
            writeFileSync(spec.stamp, JSON.stringify({ entry, glance: glance(kept, spec) }));
            console.log(`${label}: ${spec.name ?? spec.crate} as this machine already compiled it from these sources (${entry.replace(homedir(), '~')})`);
            return resolve(entry, stored);
          }
        } catch (error) { console.warn(`${label}: no kept ${spec.name ?? spec.crate} (${error.message}); compiling it`); }
        return null;
      },
      keep(started) {
        try {
          const built = statSync(file).mtimeMs;
          if (built < started) return false;
          const id = group(spec);
          writeFileSync(`${file}.kept.json`, JSON.stringify({ started, built, group: id }));
          const files = inputs(spec);
          if (!files) return false;
          // Not while a file was changing under the compile, and only what git has: a kept entry is a committed state.
          if (!files.every(file => under(resolve(root, file))?.every(([, s]) => s.mtimeMs < started))) return false;
          const git = spawnSync('git', ['-C', root, 'status', '--porcelain', '--', ...files], { encoding: 'utf8' });
          if (git.status !== 0 || git.stdout !== '') return false;
          const manifest = JSON.stringify({ files: files.map(file => [file, digest(resolve(root, file))]), env: asked(spec).map(k => [k, env[k] ?? null]) });
          const dir = resolve(cache, id), entry = resolve(dir, sha(manifest).slice(0, 16)), making = `${entry}.${process.pid}`;
          if (existsSync(entry)) return true;
          mkdirSync(making, { recursive: true });
          copyFileSync(file, resolve(making, stored));
          writeFileSync(resolve(making, 'inputs.json'), manifest);
          try { renameSync(making, entry); } catch { rmSync(making, { recursive: true, force: true }); }
          for (const old of newestFirst(dir).slice(KEPT)) rmSync(resolve(dir, old), { recursive: true, force: true });
          for (const old of newestFirst(cache).slice(GROUPS)) rmSync(resolve(cache, old), { recursive: true, force: true });
          return existsSync(entry);
        } catch (error) { console.warn(`${label}: ${spec.name ?? spec.crate} was not kept for other checkouts (${error.message})`); return false; }
      },
      /** Keeps `file` as its receipt says it was built, if it is still that build
       * and the group is the same; `null` when there is no such receipt. */
      keepAgain() {
        try {
          const receipt = JSON.parse(text(`${file}.kept.json`) ?? 'null');
          if (!receipt || statSync(file).mtimeMs !== receipt.built || group(spec) !== receipt.group) return null;
          return this.keep(receipt.started);
        } catch { return null; }
      },
    };
  };
}
