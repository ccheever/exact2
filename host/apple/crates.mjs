/**
 * Compiled registry crates, kept for the machine (LLP 1036.000 §11).
 *
 * Nine of ten crates under an app are from the registry, the same bytes in
 * every checkout, and the first build of every checkout compiled them again:
 * 190 of the 228 a fresh checkout built for Caltrain on a ten-core M4, 20 of
 * its 78 s. A development build keeps what Cargo compiled of them in
 * `~/.cache/exact/apple-crates`, and a target directory that has compiled
 * nothing starts as a copy-on-write clone of what is kept.
 *
 * Nothing here decides whether a unit may be used: Cargo does, by the unit's
 * own fingerprint (compiler, profile, features, target, every crate under it)
 * and the order of its files' times, as it would had the directory been
 * moved. So what is kept is always one target directory's units, whole and
 * with their times (a generation), never units of two directories together:
 * two builds of one unit are not interchangeable to the units compiled
 * against them (a crate that includes a file from its build script's
 * directory has that directory in its hash), and only the times say which
 * were built together. A directory that started from a generation, or made
 * the first one, makes the next when it has compiled a registry unit the
 * newest lacks; any other directory keeps nothing.
 *
 * A kept unit is of a registry package no version of which is a path crate
 * or has one under it: a path crate's unit names its checkout, so it and all
 * above it are no use to another. rustc's own dep-info (`*.d`) names the
 * target directory it was written in, and the bake's receipt reads it as the
 * evidence of what a unit was compiled from (`scripts/app.mjs`), so it is
 * kept with that directory as a token and written out with the taker's.
 *
 * A production build compiles everything it ships and keeps nothing.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, resolve } from 'node:path';

/** What stands for the target directory in a kept `.d` file. */
const TOKEN = '@exact-target@';
/** Generations kept: the newest, and the one a build may still be cloning. */
const GENERATIONS = 2;
const PARTS = ['build', 'deps', '.fingerprint'];

const text = (path) => { try { return readFileSync(path, 'utf8'); } catch { return null; } };
const names = (dir) => { try { return readdirSync(dir); } catch { return []; } };
const changed = (path) => { try { return statSync(path).mtimeMs; } catch { return 0; } };
const unitOf = (name) => /^(.+)-([0-9a-f]{16})$/.exec(name);
const hashOf = (file) => /-([0-9a-f]{16})\.[\w.]+$/.exec(file)?.[1];
/** A dependency-info file with one path prefix swapped for another, its time kept. */
function rewrite(from, to, was, now) {
  const stat = statSync(from);
  mkdirSync(dirname(to), { recursive: true });
  writeFileSync(to, readFileSync(from, 'utf8').replaceAll(was, now));
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
  const tainted = new Set(packages.filter(p => !p.source).map(p => p.name));
  // A lock names a dependency by name, and by version where two share it: by name alone here, so either taints both.
  for (let grew = true; grew;) {
    grew = false;
    for (const p of packages) if (!tainted.has(p.name) && (p.dependencies ?? []).some(dep => tainted.has(dep.split(' ')[0]))) { tainted.add(p.name); grew = true; }
  }
  return new Set(packages.map(p => p.name).filter(name => !tainted.has(name)));
}

/**
 * The kept crates for one build at `profile`. `take(target, kind)` starts a
 * Cargo target directory that has compiled nothing at the profile from the
 * newest generation of its kind (`app` and `modules` are different graphs);
 * `keep(target, kind, lock)` makes the next generation when this directory
 * may and has something new.
 */
export function keptCrates({ root, env, profile }) {
  const idle = { take() {}, keep() {} };
  if (typeof Bun === 'undefined') return idle;
  const channel = env.RUSTUP_TOOLCHAIN ?? /channel\s*=\s*"([^"]+)"/.exec(text(resolve(root, 'rust-toolchain.toml')) ?? '')?.[1];
  if (!channel || !/^[\w.+-]+$/.test(channel)) return idle;
  const store = resolve(env.HOME ?? homedir(), '.cache/exact/apple-crates', `${channel}-${profile}`);
  const generations = (kind) => names(resolve(store, kind)).filter(name => name.startsWith('gen-')).sort();
  // The host's profile directory, and each Apple triple's beside it.
  const profileDirs = (dir) => [profile, ...names(dir).filter(name => /^[a-z0-9_]+-apple-[a-z0-9-]+$/.test(name)).map(triple => `${triple}/${profile}`)].filter(d => existsSync(resolve(dir, d, '.fingerprint')));
  // A path rustc would escape in a `.d` is not worth a second spelling: such a target keeps and takes nothing.
  const plain = (target) => /^[\w./+@-]+$/.test(target);
  const stampOf = (target) => resolve(target, `kept-crates-${profile}.json`);
  const warn = (what, error) => console.warn(`host/apple: ${what} (${error.message}); compiling as usual`);
  return {
    take(target, kind) {
      try {
        const newest = generations(kind).at(-1);
        if (!plain(target) || existsSync(resolve(target, profile)) || !newest) return;
        const kept = resolve(store, kind, newest);
        let units = 0;
        for (const dir of profileDirs(kept)) {
          const here = resolve(target, dir);
          if (existsSync(here)) continue;
          clone(PARTS.map(part => resolve(kept, dir, part)).filter(existsSync), here);
          for (const file of depInfo(here)) rewrite(resolve(kept, dir, file), resolve(here, file), TOKEN, target);
          units += names(resolve(here, '.fingerprint')).length;
        }
        writeFileSync(stampOf(target), JSON.stringify({ from: newest, saw: {} }));
        console.log(`host/apple: ${target.replace(root + '/', '')} starts with the ${units} registry units this machine has compiled (${kept.replace(homedir(), '~')})`);
      } catch (error) { warn('no kept crates', error); }
    },
    keep(target, kind, lock) {
      try {
        if (!plain(target)) return;
        const stamp = JSON.parse(text(stampOf(target)) ?? '{}'), dirs = profileDirs(target);
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
          for (const dir of dirs) {
            const units = mine(dir), taken = new Set(units), hashes = new Set(units.map(name => unitOf(name)[2]));
            const here = resolve(target, dir), files = names(resolve(here, 'deps')).filter(file => hashes.has(hashOf(file)));
            clone(units.map(name => resolve(here, 'build', name)).filter(existsSync), resolve(making, dir, 'build'));
            clone(files.filter(file => !file.endsWith('.d')).map(file => resolve(here, 'deps', file)), resolve(making, dir, 'deps'));
            for (const file of depInfo(here)) {
              const [part, name] = file.split('/');
              if (part === 'deps' ? hashes.has(hashOf(name)) : taken.has(name)) rewrite(resolve(here, file), resolve(making, dir, file), target, TOKEN);
            }
            clone(units.map(name => resolve(here, '.fingerprint', name)), resolve(making, dir, '.fingerprint'));
          }
          from = `gen-${Date.now()}-${process.pid}`;
          renameSync(making, resolve(store, kind, from));
          for (const old of generations(kind).slice(0, -GENERATIONS)) rmSync(resolve(store, kind, old), { recursive: true, force: true });
          for (const left of names(resolve(store, kind))) if (left.startsWith('.making-') && Date.now() - changed(resolve(store, kind, left)) > 3_600_000) rmSync(resolve(store, kind, left), { recursive: true, force: true });
        }
        writeFileSync(stampOf(target), JSON.stringify({ from, saw }));
      } catch (error) { warn('this build\'s crates were not kept', error); }
    },
  };
}
