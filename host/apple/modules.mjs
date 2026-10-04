/**
 * The host's Rust modules, kept for the machine (LLP 1036.000 §10).
 *
 * `libexact_canvas_vello` and `libexact_svg_raster` hold nothing of an app,
 * so every checkout whose sources agree builds the same module. A first build
 * in a fresh checkout compiled both from nothing beside the bake: 190 CPU
 * seconds, 27 s of a 106 s build on a ten-core M4. A development build keeps
 * each module it compiles in `~/.cache/exact/apple-modules` (beside Hermes),
 * with the SHA-256 of every file Cargo says it was built from, and a build in
 * any checkout whose files are those bytes takes the kept module instead of
 * compiling it.
 *
 * What says two builds are the same:
 * - the group: the crate, target and profile, `rustc -vV`, the crate's
 *   closure in `Cargo.lock` (each registry crate by checksum), the workspace
 *   manifest apart from its member list, Cargo's config files, the variables
 *   a compile reads, and the SDK and Metal toolchain the caller names;
 * - the entry: every file and directory in Cargo's dep-info for the module
 *   (the path crates' sources, their build scripts' inputs), each of those
 *   crates' manifests, and each variable a build script asked to rerun for.
 *
 * Only committed sources are kept, so an edit loop does not push out the
 * entries other checkouts would take. A checkout that has compiled a module
 * itself goes on asking Cargo, whose answer costs it nothing. A production
 * build never reads a kept module.
 */
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, relative, resolve } from 'node:path';

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
 * The kept modules for one build: `find(crate)` is a kept dylib built from
 * this checkout's bytes, or `null`; `keep(crate, started)` keeps the one
 * Cargo linked since `started`. `sdk` is the SDK's path and `metal` what the
 * Metal toolchain says of its version: with Xcode's own version they are the
 * tools a module's link and its shaders' compile run.
 */
export function keptModules({ root, moduleTarget, target, profile, env, sdk, metal }) {
  if (typeof Bun === 'undefined') return { find: () => null, keep() {} };
  const settings = text(resolve(sdk, 'SDKSettings.json')), xcode = /^(.*\.app\/Contents)\//.exec(sdk)?.[1];
  const tools = [sdk, settings && sha(settings), xcode ? text(resolve(xcode, 'version.plist')) : null, metal];
  const cache = resolve(env.HOME ?? homedir(), '.cache/exact/apple-modules');
  const libDir = resolve(moduleTarget, target, profile), dylib = (crate) => `lib${crate.replaceAll('-', '_')}.dylib`;
  const reads = () => Object.entries(env).filter(([k]) => READS.test(k)).sort();
  const configs = [resolve(root, '.cargo/config.toml'), resolve(env.CARGO_HOME ?? resolve(homedir(), '.cargo'), 'config.toml')];
  let rustc;
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
  const group = (crate) => {
    rustc ??= spawnSync('rustc', ['-vV'], { cwd: root, env, encoding: 'utf8' }).stdout;
    const workspace = Bun.TOML.parse(readFileSync(resolve(root, 'Cargo.toml'), 'utf8'));
    for (const members of ['members', 'exclude', 'default-members']) delete workspace.workspace?.[members];
    return sha(JSON.stringify([1, crate, target, profile, rustc, closure(crate), workspace, configs.map(text), reads(), tools])).slice(0, 24);
  };
  /** What a checkout that took an entry saw of its inputs: while that is unchanged, the search is not repeated. */
  const glance = (kept) => sha(JSON.stringify([kept.files.map(([file]) => seen(resolve(root, file))), kept.env.map(([k]) => env[k] ?? null),
    ['Cargo.lock', 'Cargo.toml', 'rust-toolchain.toml'].map(file => seen(resolve(root, file))), configs.map(seen), seen(resolve(env.RUSTUP_HOME ?? resolve(homedir(), '.rustup'), 'settings.toml')), reads(), tools]));
  const entryOf = (entry, crate) => existsSync(resolve(entry, dylib(crate))) ? JSON.parse(text(resolve(entry, 'inputs.json')) ?? 'null') : null;
  /** The module's inputs from Cargo's dep-info, relative to the checkout; `null` when one is outside it. */
  const inputs = (crate) => {
    const line = (text(resolve(libDir, dylib(crate).replace(/dylib$/, 'd'))) ?? '').split('\n').find(l => l.includes(': '));
    if (!line) return null;
    const files = new Set();
    for (const raw of line.slice(line.indexOf(': ') + 2).split(/(?<!\\) /)) {
      const file = resolve(raw.replace(/\\ /g, ' '));
      if (file.startsWith(moduleTarget + '/')) continue;
      if (relative(root, file).startsWith('..')) return null;
      files.add(relative(root, file));
      for (let dir = dirname(file); dir !== root && dir.startsWith(root); dir = dirname(dir)) if (existsSync(resolve(dir, 'Cargo.toml'))) { files.add(relative(root, resolve(dir, 'Cargo.toml'))); break; }
    }
    return [...files].sort();
  };
  /** Each variable a build script of this target directory asked to be rerun for, for the target or for the host. */
  const asked = () => {
    const names = new Set();
    for (const builds of [resolve(libDir, 'build'), resolve(moduleTarget, profile, 'build')]) for (const dir of existsSync(builds) ? readdirSync(builds) : []) {
      for (const m of (text(resolve(builds, dir, 'output')) ?? '').matchAll(/^cargo::?rerun-if-env-changed=(.+)$/gm)) names.add(m[1]);
    }
    return [...names].sort();
  };
  const stamp = (crate) => resolve(moduleTarget, 'kept', `${crate}-${target}-${profile}.json`);
  return {
    find(crate) {
      try {
        if (existsSync(resolve(libDir, dylib(crate)))) return null;
        const last = JSON.parse(text(stamp(crate)) ?? 'null'), again = last && entryOf(last.entry, crate);
        if (again && glance(again) === last.glance) return resolve(last.entry, dylib(crate));
        const dir = resolve(cache, group(crate));
        for (const name of newestFirst(dir)) {
          const entry = resolve(dir, name), kept = entryOf(entry, crate);
          if (!kept || !kept.files.every(([file, bytes]) => digest(resolve(root, file)) === bytes) || !kept.env.every(([k, v]) => (env[k] ?? null) === v)) continue;
          const now = new Date();
          utimesSync(entry, now, now); utimesSync(dir, now, now);
          mkdirSync(dirname(stamp(crate)), { recursive: true });
          writeFileSync(stamp(crate), JSON.stringify({ entry, glance: glance(kept) }));
          console.log(`host/apple: ${crate} as this machine already compiled it from these sources (${entry.replace(homedir(), '~')})`);
          return resolve(entry, dylib(crate));
        }
      } catch (error) { console.warn(`host/apple: no kept ${crate} (${error.message}); compiling it`); }
      return null;
    },
    keep(crate, started) {
      try {
        const lib = resolve(libDir, dylib(crate)), files = inputs(crate);
        if (!files || statSync(lib).mtimeMs < started) return;
        // Not while a file was changing under the compile, and only what git has: a kept entry is a committed state.
        if (!files.every(file => under(resolve(root, file))?.every(([, s]) => s.mtimeMs < started))) return;
        const git = spawnSync('git', ['-C', root, 'status', '--porcelain', '--', ...files], { encoding: 'utf8' });
        if (git.status !== 0 || git.stdout !== '') return;
        const manifest = JSON.stringify({ files: files.map(file => [file, digest(resolve(root, file))]), env: asked().map(k => [k, env[k] ?? null]) });
        const dir = resolve(cache, group(crate)), entry = resolve(dir, sha(manifest).slice(0, 16)), making = `${entry}.${process.pid}`;
        if (existsSync(entry)) return;
        mkdirSync(making, { recursive: true });
        copyFileSync(lib, resolve(making, dylib(crate)));
        writeFileSync(resolve(making, 'inputs.json'), manifest);
        try { renameSync(making, entry); } catch { rmSync(making, { recursive: true, force: true }); }
        for (const old of newestFirst(dir).slice(KEPT)) rmSync(resolve(dir, old), { recursive: true, force: true });
        for (const old of newestFirst(cache).slice(GROUPS)) rmSync(resolve(cache, old), { recursive: true, force: true });
      } catch (error) { console.warn(`host/apple: ${crate} was not kept for other checkouts (${error.message})`); }
    },
  };
}
