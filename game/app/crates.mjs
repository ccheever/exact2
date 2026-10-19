// Registry units stay in coherent generations; Cargo validates every copied fingerprint.
// A rebuilt copied unit is a failed warm, never tolerated.
// Path packages and their dependants remain private to each game target.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve } from 'node:path';

/** What stands for the target directory in a kept `.d` file. */
const TARGET = '@exact-target@', BUILD = '@exact-build@';
/** Generations kept: the newest, and the one a build may still be cloning. */
const GENERATIONS = 2;
const PARTS = ['build', 'deps', '.fingerprint'];

const text = (path) => { try { return readFileSync(path, 'utf8'); } catch { return null; } };
const names = (dir) => { try { return readdirSync(dir); } catch { return []; } };
const changed = (path) => { try { return statSync(path).mtimeMs; } catch { return 0; } };
const unitOf = (name) => /^(.+)-([0-9a-f]{16})$/.exec(name);
const hashOf = (file) => /-([0-9a-f]{16})\.[\w.]+$/.exec(file)?.[1];
/** A dependency-info file with one path prefix swapped for another, its time kept. */
function rewrite(from, to, replacements) {
  const stat = statSync(from);
  mkdirSync(dirname(to), { recursive: true });
  let content = readFileSync(from, 'utf8');
  for (const [was, now] of replacements) content = content.replaceAll(was, now);
  writeFileSync(to, content);
  utimesSync(to, stat.atime, stat.mtime);
}
/** Every rustc dep-info file of a profile directory, relative to it: the units' in `deps`, the build scripts' beside each script. */
const depInfo = (dir) => [...names(resolve(dir, 'deps')).map(file => `deps/${file}`),
  ...names(resolve(dir, 'build')).flatMap(unit => names(resolve(dir, 'build', unit)).map(file => `build/${unit}/${file}`))].filter(file => file.endsWith('.d'));
/** Clone whole trees into `into` (APFS copy-on-write; a plain copy elsewhere), times kept. */
function clone(sources, into) {
  if (!sources.length) return;
  mkdirSync(into, { recursive: true });
  for (let at = 0; at < sources.length; at += 400) {
    const batch = sources.slice(at, at + 400);
    if (spawnSync('cp', ['-c', '-R', '-p', ...batch, into], { stdio: 'ignore' }).status !== 0
        && spawnSync('cp', ['-R', '-p', ...batch, into], { stdio: 'ignore' }).status !== 0) throw new Error(`cp to ${into} failed`);
  }
}

/** The registry packages of `lock` that hold nothing of a checkout: see the file comment. */
export function registryPackages(lock) {
  const packages = Bun.TOML.parse(readFileSync(lock, 'utf8')).package ?? [];
  const tainted = new Set(packages.filter(p => !p.source?.startsWith('registry+')).map(p => p.name));
  // A lock names a dependency by name, and by version where two share it: by name alone here, so either taints both.
  for (let grew = true; grew;) {
    grew = false;
    for (const p of packages) if (!tainted.has(p.name) && (p.dependencies ?? []).some(dep => tainted.has(dep.split(' ')[0]))) { tainted.add(p.name); grew = true; }
  }
  return new Set(packages.filter(p => p.source?.startsWith('registry+') && !tainted.has(p.name)).map(p => p.name));
}

/** Compiler identity and effective flags, without a game's directory or authored inputs. */
export function gameCrateKey({channel,profile,triple,rustc,flags,strip,env = {}}) {
  const names = ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTUP_TOOLCHAIN','CARGO_INCREMENTAL','CARGO_BUILD_RUSTFLAGS',
    ...Object.keys(env).filter(name => name.startsWith('CARGO_PROFILE_') || name.startsWith('CARGO_TARGET_') && name.endsWith('_RUSTFLAGS')).sort()];
  return createHash('sha256').update(JSON.stringify({channel,profile,triple,rustc,flags,strip,
    environment:Object.fromEntries(names.map(name => [name,env[name] ?? null]))})).digest('hex');
}

/** Resolve existing ancestors too: Cargo may spell /tmp as /private/tmp on macOS. */
function canonical(path) {
  try { return realpathSync(path); }
  catch (error) {
    if (error.code !== 'ENOENT') throw error;
    return resolve(canonical(dirname(path)), path.slice(dirname(path).length + 1));
  }
}
/** Cargo metadata, not app.target or an environment override, owns both directories. */
function placement(metadata) {
  const target = metadata.target_directory, build = metadata.build_directory;
  if (!target || !build) throw new Error('Cargo metadata must resolve target_directory and build_directory');
  const pairs = [[target, TARGET], [canonical(target), TARGET], [build, BUILD], [canonical(build), BUILD]];
  return { target: canonical(build),
    keep: pairs.sort(([a], [b]) => b.length - a.length),
    take: [[BUILD, canonical(build)], [TARGET, canonical(target)]],
    plain: pairs.every(([path]) => /^[\w./+@-]+$/.test(path)) };
}

/** Registry generations for a profile. Pass resolved Cargo metadata to take/keep.
 * Only fresh profile directories are seeded; existing host or target profiles
 * stay private. Both sides of a split Cargo directory layout are relocated. */
export function keptGameCrates({ root, env, profile, triple, flags, strip = false, directory = resolve(homedir(), '.cache/exact/game-crates') }) {
  const idle = { take() {}, keep() {} };
  if (typeof Bun === 'undefined') return idle;
  const channel = env.RUSTUP_TOOLCHAIN ?? /channel\s*=\s*"([^"]+)"/.exec(text(resolve(root, 'rust-toolchain.toml')) ?? '')?.[1];
  if (!channel || !/^[\w.+-]+$/.test(channel)) return idle;
  const compiler = spawnSync(env.RUSTC ?? 'rustc', ['-vV'], {cwd:root, env, encoding:'utf8'});
  if (compiler.status !== 0) throw new Error('rustc -vV failed');
  const key = gameCrateKey({channel,profile,triple,rustc:compiler.stdout,flags,strip,env});
  const store = resolve(directory, channel, profile, triple, key);
  const generations = (kind) => names(resolve(store, kind)).filter(name => name.startsWith('gen-')).sort();
  // Build dependencies and target units belong to the same generation.
  const profileDirs = (dir) => [profile, `${triple}/${profile}`].filter(d => existsSync(resolve(dir, d, '.fingerprint')));
  const stampOf = (target) => resolve(target, `kept-game-crates-${key}.json`);
  const warn = (what, error) => console.warn(`game/cache: ${what} (${error.message}); compiling as usual`);
  return {
    store, key,
    take(metadata, kind) {
      try {
        const paths = placement(metadata), target = paths.target;
        const newest = generations(kind).at(-1);
        if (!paths.plain || !newest) return;
        const kept = resolve(store, kind, newest);
        let units = 0;
        const skipped = [];
        for (const dir of profileDirs(kept)) {
          const here = resolve(target, dir);
          if (existsSync(here)) { skipped.push(dir); continue; }
          clone(PARTS.map(part => resolve(kept, dir, part)).filter(existsSync), here);
          for (const file of depInfo(here)) rewrite(resolve(kept, dir, file), resolve(here, file), paths.take);
          units += names(resolve(here, '.fingerprint')).length;
        }
        if (!units) return;
        writeFileSync(stampOf(target), JSON.stringify({ from: newest, saw: {}, skipped }));
        console.log(`game/cache: ${target.replace(root + '/', '')} starts with the ${units} registry units this machine has compiled (${kept.replace(homedir(), '~')})`);
      } catch (error) { warn('no kept crates', error); }
    },
    keep(metadata, kind, lock) {
      try {
        const paths = placement(metadata), target = paths.target;
        if (!paths.plain) return;
        const stamp = JSON.parse(text(stampOf(target)) ?? '{}');
        const dirs = profileDirs(target).filter(dir => !stamp.skipped?.includes(dir));
        const saw = Object.fromEntries(dirs.map(dir => [dir, changed(resolve(target, dir, '.fingerprint'))]));
        if (dirs.every(dir => stamp.saw?.[dir] === saw[dir])) return;
        const newest = generations(kind).at(-1);
        // A directory that compiled its crates apart from every generation is not of their line: its units with theirs would be the two builds.
        if (!stamp.from && newest) return;
        const registry = registryPackages(lock), has = (dir) => new Set(newest ? names(resolve(store, kind, newest, dir, '.fingerprint')) : []);
        // This lock's registry units, and those it took, which another lock (an app outside the repository) may be the one to use.
        const mine = (dir, kept = has(dir)) => names(resolve(target, dir, '.fingerprint')).filter(name => registry.has(unitOf(name)?.[1]) || kept.has(name));
        let from = stamp.from ?? null;
        if (dirs.some(dir => { const kept = has(dir); return mine(dir, kept).some(name => !kept.has(name)); })) {
          const making = resolve(store, kind, `.making-${process.pid}`);
          rmSync(making, { recursive: true, force: true });
          for (const dir of stamp.skipped ?? []) {
            const previous = resolve(store, kind, newest, dir);
            if (existsSync(previous)) clone([previous], resolve(making, dirname(dir)));
          }
          for (const dir of dirs) {
            const units = mine(dir), taken = new Set(units), hashes = new Set(units.map(name => unitOf(name)[2]));
            const here = resolve(target, dir), files = names(resolve(here, 'deps')).filter(file => hashes.has(hashOf(file)));
            clone(units.map(name => resolve(here, 'build', name)).filter(existsSync), resolve(making, dir, 'build'));
            clone(files.filter(file => !file.endsWith('.d')).map(file => resolve(here, 'deps', file)), resolve(making, dir, 'deps'));
            for (const file of depInfo(here)) {
              const [part, name] = file.split('/');
              if (part === 'deps' ? hashes.has(hashOf(name)) : taken.has(name)) rewrite(resolve(here, file), resolve(making, dir, file), paths.keep);
            }
            clone(units.map(name => resolve(here, '.fingerprint', name)), resolve(making, dir, '.fingerprint'));
          }
          from = `gen-${Date.now()}-${process.pid}`;
          renameSync(making, resolve(store, kind, from));
          for (const old of generations(kind).slice(0, -GENERATIONS)) rmSync(resolve(store, kind, old), { recursive: true, force: true });
          for (const left of names(resolve(store, kind))) if (left.startsWith('.making-') && Date.now() - changed(resolve(store, kind, left)) > 3_600_000) rmSync(resolve(store, kind, left), { recursive: true, force: true });
        }
        writeFileSync(stampOf(target), JSON.stringify({ from, saw, skipped: stamp.skipped }));
      } catch (error) { warn('this build\'s crates were not kept', error); }
    },
  };
}
