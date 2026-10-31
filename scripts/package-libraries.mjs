// A package's prebuilt dynamic library (LLP 1154 D3 "The library", LLP 1133.001 D9): the one the app's direct
// dependencies declare in `exact.json` `native.library`, fetched for the target a native build compiles, by its
// SHA-256, before Cargo runs, into `~/.cache/exact/package-libraries/<sha256>/`. Nothing fetches at compile time.
//
// What a library file is, by target:
// - an Apple target (`*-apple-*`): a zip holding one `.xcframework` at its top, as a SwiftPM binary target ships one
//   ([SwiftPM: binaryTarget(name:url:checksum:)](https://developer.apple.com/documentation/packagedescription/target/binarytarget(name:url:checksum:))).
//   The hash covers the zip, the one file that has a hash; a directory has none. The slice for the target, read from
//   the xcframework's Info.plist, is a dynamic framework named `<link>.framework` whose install name is
//   `@rpath/<link>.framework/…`, so an app that embeds it in `Frameworks` finds it by its run path.
// - a Linux or Android target: the shared object itself, `lib<link>.so`, whose SONAME, if it has one, is that name.
//
// Each target's link inputs are laid out under `<sha256>/<target>/`, where the bake's build script points the linker
// (js/bake/src/library.rs): `-L <that directory> -l<link>` on every target, which on Apple finds `lib<link>.dylib`, a
// link to the framework's binary, so the link records the framework's install name.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const LIBRARY_CACHE = resolve(homedir(), '.cache/exact/package-libraries');

const read = (path) => { try { return JSON.parse(readFileSync(path, 'utf8')); } catch { return null; } };
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** The packages among the app's direct dependencies (the `dependencies` of the nearest `package.json` above it, as
 * contract/cli/src/package_views.rs reads them) whose `exact.json`, at the package's root, declares `native`. */
export function nativePackages(appDir) {
  let manifest = null;
  for (let dir = resolve(appDir); ; dir = dirname(dir)) {
    if (existsSync(join(dir, 'package.json'))) { manifest = read(join(dir, 'package.json')); break; }
    if (dirname(dir) === dir) break;
  }
  const located = (name) => {
    for (let dir = resolve(appDir); ; dir = dirname(dir)) {
      const root = join(dir, 'node_modules', name);
      if (existsSync(join(root, 'package.json'))) return root;
      if (dirname(dir) === dir) return null;
    }
  };
  return Object.keys(manifest?.dependencies ?? {}).sort().flatMap((name) => {
    const root = located(name), declaration = root && read(join(root, 'exact.json'));
    return declaration?.native && typeof declaration.native === 'object' ? [{ name, root, native: declaration.native }] : [];
  });
}

const APPLE_PLATFORMS = { darwin: 'macos', ios: 'ios', tvos: 'tvos', watchos: 'watchos', visionos: 'xros' };
/** The xcframework slice a Rust target triple runs: Info.plist's platform, variant and architecture. */
export function appleSlice(target) {
  const [cpu, , os, abi] = target.split('-');
  const platform = APPLE_PLATFORMS[os], arch = { aarch64: 'arm64', arm64_32: 'arm64_32', x86_64: 'x86_64' }[cpu];
  if (!platform || !arch) return null;
  // An x86_64 iOS-family target is a simulator's; a Mac's has no variant.
  const variant = abi === 'sim' || (os !== 'darwin' && cpu === 'x86_64') ? 'simulator' : abi === 'macabi' ? 'maccatalyst' : undefined;
  return { platform, variant, arch };
}

/** The library of `slice` in the xcframework at `fw`, from its Info.plist's `AvailableLibraries` (its platform,
 * variant and architectures), never guessed from a directory name: its directory and `LibraryPath`. */
export function xcframeworkSlice(fw, { platform, variant, arch }) {
  const plist = spawnSync('plutil', ['-convert', 'json', '-o', '-', resolve(fw, 'Info.plist')], { encoding: 'utf8' });
  if (plist.status !== 0) throw new Error(`${basename(fw)} has no readable Info.plist: ${(plist.stderr || '').trim()}`);
  let info;
  try { info = JSON.parse(plist.stdout || '{}'); } catch (e) { throw new Error(`${basename(fw)}'s Info.plist is not a dictionary: ${e.message}`); }
  const lib = (info.AvailableLibraries ?? []).find(l => l.SupportedPlatform === platform && (l.SupportedPlatformVariant || undefined) === variant && (l.SupportedArchitectures ?? []).includes(arch));
  if (!lib) throw new Error(`${basename(fw)} has no ${platform}${variant ? ' ' + variant : ''} ${arch} slice in its Info.plist`);
  return { dir: resolve(fw, lib.LibraryIdentifier), lib };
}

/** What the app's build for `target` links of its package's library: the declaration's `{url, sha256}` for the
 * target, with the package and its link name; null when no direct dependency declares `native.library`. A target
 * the declaration does not name is refused by name (LLP 1154 D6), as is a declaration exact2 cannot use. */
export function libraryFor(appDir, target) {
  const declaring = nativePackages(appDir);
  if (declaring.length > 1) throw new Error(`bake-native-twice: ${declaring.map(p => p.name).join(' and ')} each declare \`native\` in exact.json, and an app has one native module; depend on one of them`);
  const [pkg] = declaring;
  if (!pkg?.native.library) return null;
  const { library, link } = pkg.native, at = `${pkg.name}'s exact.json`;
  if (typeof library !== 'object' || Array.isArray(library)) throw new Error(`${at}: native.library is an object of {url, sha256} by Rust target`);
  if (typeof link !== 'string' || !/^[A-Za-z0-9_][A-Za-z0-9_.+-]*$/.test(link)) throw new Error(`${at}: native.link names the library (\`lib<link>.so\`, \`<link>.framework\`), not ${JSON.stringify(link)}`);
  const entry = library[target];
  if (!entry) throw new Error(`${pkg.name} has no native library for ${target}: its exact.json native.library names ${Object.keys(library).join(', ') || 'no target'}. An app that builds for ${target} waits for the package to publish one (LLP 1154 D6)`);
  if (typeof entry.url !== 'string' || !/^[0-9a-f]{64}$/.test(entry.sha256 ?? '')) throw new Error(`${at}: native.library.${target} is {"url": …, "sha256": <64 lowercase hex digits>}`);
  let url;
  try { url = new URL(entry.url); } catch { throw new Error(`${at}: native.library.${target}.url is not a URL: ${entry.url}`); }
  if (!['file:', 'https:', 'http:'].includes(url.protocol)) throw new Error(`${at}: native.library.${target}.url is a file:, https: or http: URL, not ${url.protocol}`);
  const kind = target.includes('-apple-') ? 'apple' : target.includes('-linux-') && !target.endsWith('-android') ? 'linux' : null;
  if (!kind) throw new Error(`${pkg.name} declares a native library for ${target}, and exact2 links a package's library only for Apple and Linux targets; Android's APK carries none yet (LLP 1154 D6)`);
  const file = basename(decodeURIComponent(url.pathname));
  if (kind === 'apple' && !file.endsWith('.zip')) throw new Error(`${at}: native.library.${target} is a zip holding the .xcframework, not ${file}`);
  return { package: pkg.name, root: pkg.root, target, link, url: url.href, sha256: entry.sha256, file, kind };
}

/** `lib` (from `libraryFor`) in the cache: its file fetched when it is not there with its hash, refused by name when
 * the fetched bytes have another, and its link inputs for the target laid out. Returns `lib` with `view`, the link
 * directory, and `placed`, the file or framework a product carries beside its executable or in its `Frameworks`. */
export async function fetchLibrary(lib, { cache = LIBRARY_CACHE, log = (line) => console.error(line) } = {}) {
  const dir = join(cache, lib.sha256), file = join(dir, lib.file);
  if (!existsSync(file) || digest(readFileSync(file)) !== lib.sha256) {
    log(`${lib.package}: fetching its ${lib.target} library (${lib.url})`);
    const url = new URL(lib.url);
    let bytes;
    if (url.protocol === 'file:') {
      try { bytes = readFileSync(fileURLToPath(url)); } catch (e) { throw new Error(`${lib.package}: its ${lib.target} library ${lib.url} could not be read: ${e.message}`); }
    } else {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`${lib.package}: its ${lib.target} library ${lib.url} answered HTTP ${response.status}`);
      bytes = Buffer.from(await response.arrayBuffer());
    }
    const actual = digest(bytes);
    if (actual !== lib.sha256) throw new Error(`${lib.package}: its ${lib.target} library ${lib.url} has SHA-256 ${actual}, and its exact.json says ${lib.sha256}; nothing was linked`);
    mkdirSync(dir, { recursive: true });
    const temporary = `${file}.${process.pid}.tmp`;
    writeFileSync(temporary, bytes);
    renameSync(temporary, file);
  }
  const view = join(dir, lib.target), placed = join(view, lib.kind === 'apple' ? `${lib.link}.framework` : `lib${lib.link}.so`);
  if (!existsSync(placed)) {
    mkdirSync(dir, { recursive: true });
    const stage = mkdtempSync(join(dir, '.view-'));
    try {
      if (lib.kind === 'apple') appleView(lib, file, stage);
      else cpSync(file, join(stage, `lib${lib.link}.so`));
      // Another build may have laid it out meanwhile: the same bytes, kept.
      try { renameSync(stage, view); } catch (e) { if (!existsSync(placed)) throw e; }
    } finally { rmSync(stage, { recursive: true, force: true }); }
  }
  return { ...lib, view, placed };
}

/** The target's framework from the zipped xcframework at `zip`, and the `lib<link>.dylib` the linker's `-l` finds. */
function appleView(lib, zip, stage) {
  if (process.platform !== 'darwin') throw new Error(`${lib.package}: its ${lib.target} library is an xcframework, which is read on a Mac`);
  const unzipped = join(stage, '.unzipped');
  const r = spawnSync('ditto', ['-x', '-k', zip, unzipped], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`${lib.package}: its ${lib.target} library ${lib.file} is not a zip: ${(r.stderr || '').trim()}`);
  const found = readdirSync(unzipped).filter(name => name.endsWith('.xcframework'));
  if (found.length !== 1) throw new Error(`${lib.package}: ${lib.file} holds ${found.length ? found.join(', ') : 'no .xcframework'} at its top; it holds one .xcframework`);
  const fw = join(unzipped, found[0]), slice = appleSlice(lib.target);
  let at;
  try { at = xcframeworkSlice(fw, slice); } catch (e) { throw new Error(`${lib.package}: its ${lib.target} library: ${e.message}`); }
  if (at.lib.LibraryPath !== `${lib.link}.framework`) throw new Error(`${lib.package}: its ${lib.target} slice is ${at.lib.LibraryPath}, and native.link names ${lib.link}: the slice is ${lib.link}.framework, a dynamic framework`);
  const framework = join(stage, `${lib.link}.framework`);
  cpSync(join(at.dir, at.lib.LibraryPath), framework, { recursive: true, verbatimSymlinks: true });
  const binary = join(framework, lib.link);
  if (!existsSync(binary)) throw new Error(`${lib.package}: its ${lib.target} framework has no binary ${lib.link}.framework/${lib.link}`);
  const id = spawnSync('otool', ['-D', binary], { encoding: 'utf8' }).stdout?.trim().split('\n').slice(1).join('').trim();
  const expected = `@rpath/${lib.link}.framework/${relative(realpathSync(framework), realpathSync(binary))}`;
  if (id !== expected) throw new Error(`${lib.package}: its ${lib.target} framework's install name is ${id || 'missing (not a dynamic library)'}; an embedded framework is named ${expected} (link it with -install_name ${expected})`);
  symlinkSync(`${lib.link}.framework/${lib.link}`, join(stage, `lib${lib.link}.dylib`));
  rmSync(unzipped, { recursive: true, force: true });
}

/** The library for the app's build for `target`, fetched; null when the app's dependencies declare none. What a
 * native build calls before Cargo (scripts/build-linux.mjs, host/apple/build.mjs). */
export async function prepareLibrary(appDir, target, options) {
  const lib = libraryFor(appDir, target);
  return lib && fetchLibrary(lib, options);
}

/** Put the fetched library into `dir`: the framework on Apple, the shared object on Linux, each as a new file, so a
 * running process that has the old one mapped keeps its pages (host/apple/bundle.mjs says why). */
export function placeLibrary(lib, dir) {
  const to = join(dir, basename(lib.placed));
  rmSync(to, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  cpSync(lib.placed, to, { recursive: true, verbatimSymlinks: true });
  return to;
}

/** The Swift link's arguments for an Apple product that links `lib` (none without one): the framework's search path
 * and name, and the run path an app bundle's `Frameworks` is at (`rpath` false where another part of the link names
 * it already). */
export const appleLinkArgs = (lib, ios, rpath = true) => !lib ? [] : ['-Xlinker', `-F${lib.view}`, '-Xlinker', '-framework', '-Xlinker', lib.link,
  ...(rpath ? ['-Xlinker', '-rpath', '-Xlinker', ios ? '@executable_path/Frameworks' : '@executable_path/../Frameworks'] : [])];
