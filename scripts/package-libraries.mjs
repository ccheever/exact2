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
// - a Linux target: the shared object itself, whose SONAME is `lib<link>.so`, the name it is carried by beside the
//   executable. Only a 64-bit little-endian ELF file is read, and any other is refused. An Android target is refused:
//   the APK carries no package library yet.
//
// Each target's link inputs are laid out under `<sha256>/<target>/<link>/`, which every build checks against the
// hash-checked download before it links (`layOut`), and lays out again when they differ. A Rust link (the Linux host's
// executable) takes `-L <that directory> -l<link>` from the bake's build script (js/bake/src/library.rs); on an Apple
// target `-l` finds `lib<link>.dylib`, a link to the framework's binary, so the link records the framework's install
// name. An Apple app's Swift link takes the framework itself (`appleLinkArgs`).
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

export const LIBRARY_CACHE = resolve(homedir(), '.cache/exact/package-libraries');

const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
const inside = (root, path) => { const at = relative(root, path); return at !== '' && !isAbsolute(at) && at !== '..' && !at.startsWith(`..${sep}`); };

/** The packages among the app's direct dependencies (the `dependencies` of the nearest `package.json` above it, as
 * contract/cli/src/package_views.rs reads them) whose `exact.json`, at the package's root, declares `native`. A
 * declaration that is there and does not parse is refused by name. */
export function nativePackages(appDir) {
  let manifest = null;
  for (let dir = resolve(appDir); ; dir = dirname(dir)) {
    if (existsSync(join(dir, 'package.json'))) { try { manifest = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')); } catch { /* none */ } break; }
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
    const root = located(name), path = root && join(root, 'exact.json');
    if (!path || !existsSync(path)) return [];
    let declaration;
    try { declaration = JSON.parse(readFileSync(path, 'utf8')); } catch (e) { throw new Error(`${name}'s exact.json (${path}) cannot be read as JSON: ${e.message}`); }
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
 * variant and architectures), never guessed from a directory name: its directory and `LibraryPath`, both inside
 * the xcframework. */
export function xcframeworkSlice(fw, { platform, variant, arch }) {
  const plist = spawnSync('plutil', ['-convert', 'json', '-o', '-', resolve(fw, 'Info.plist')], { encoding: 'utf8' });
  if (plist.status !== 0) throw new Error(`${basename(fw)} has no readable Info.plist: ${(plist.stderr || '').trim()}`);
  let info;
  try { info = JSON.parse(plist.stdout || '{}'); } catch (e) { throw new Error(`${basename(fw)}'s Info.plist is not a dictionary: ${e.message}`); }
  const lib = (info.AvailableLibraries ?? []).find(l => l.SupportedPlatform === platform && (l.SupportedPlatformVariant || undefined) === variant && (l.SupportedArchitectures ?? []).includes(arch));
  if (!lib) throw new Error(`${basename(fw)} has no ${platform}${variant ? ' ' + variant : ''} ${arch} slice in its Info.plist`);
  const dir = resolve(fw, String(lib.LibraryIdentifier));
  if (!inside(fw, dir) || !inside(dir, resolve(dir, String(lib.LibraryPath)))) throw new Error(`${basename(fw)}'s ${platform} slice names ${lib.LibraryIdentifier}/${lib.LibraryPath}, outside the xcframework`);
  return { dir, lib };
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
  if (!['file:', 'https:'].includes(url.protocol)) throw new Error(`${at}: native.library.${target}.url is a file: or https: URL, not ${url.protocol}`);
  const kind = target.includes('-android') ? null : target.includes('-apple-') ? 'apple' : target.includes('-linux-') ? 'linux' : null;
  if (!kind) throw new Error(`${pkg.name} declares a native library for ${target}, and exact2 links a package's library only for Apple and Linux targets; Android's APK carries none yet (LLP 1154 D6)`);
  const file = basename(decodeURIComponent(url.pathname));
  if (kind === 'apple' && !file.endsWith('.zip')) throw new Error(`${at}: native.library.${target} is a zip holding the .xcframework, not ${file}`);
  return { package: pkg.name, root: pkg.root, target, link, url: url.href, sha256: entry.sha256, file, kind };
}

/** The bytes at `lib.url`: a file: URL read, an https: one fetched by curl (synchronous, so every build path can
 * call this before Cargo), redirects only to https:. */
function download(lib, temporary) {
  const url = new URL(lib.url);
  if (url.protocol === 'file:') {
    try { return readFileSync(fileURLToPath(url)); } catch (e) { throw new Error(`${lib.package}: its ${lib.target} library ${lib.url} could not be read: ${e.message}`); }
  }
  const r = spawnSync('curl', ['--fail', '--silent', '--show-error', '--location', '--proto', '=https', '--proto-redir', '=https', '--output', temporary, lib.url], { encoding: 'utf8' });
  if (r.status !== 0) { rmSync(temporary, { force: true }); throw new Error(`${lib.package}: its ${lib.target} library ${lib.url} could not be fetched: ${(r.stderr || r.error?.message || `curl exit ${r.status}`).trim()}`); }
  return readFileSync(temporary);
}

/** `lib` (from `libraryFor`) in the cache: its file fetched when it is not there with its hash, refused by name when
 * the fetched bytes have another, and its link inputs for the target laid out, or laid out again when they differ
 * from what was laid out. Returns `lib` with `view`, the link directory, and `placed`, the file or framework a
 * product carries beside its executable or in its `Frameworks`. */
export function fetchLibrary(lib, { cache = LIBRARY_CACHE, log = (line) => console.error(line) } = {}) {
  const dir = join(cache, lib.sha256), file = join(dir, lib.file);
  if (!existsSync(file) || digest(readFileSync(file)) !== lib.sha256) {
    log(`${lib.package}: fetching its ${lib.target} library (${lib.url})`);
    mkdirSync(dir, { recursive: true });
    const temporary = `${file}.${process.pid}.tmp`, bytes = download(lib, temporary), actual = digest(bytes);
    if (actual !== lib.sha256) {
      rmSync(temporary, { force: true });
      rmSync(file, { force: true });
      throw new Error(`${lib.package}: its ${lib.target} library ${lib.url} has SHA-256 ${actual}, and its exact.json says ${lib.sha256}; nothing was linked`);
    }
    writeFileSync(temporary, bytes);
    renameSync(temporary, file);
  }
  const view = join(dir, lib.target, lib.link), placed = join(view, lib.kind === 'apple' ? `${lib.link}.framework` : `lib${lib.link}.so`);
  layOut(lib, file, view, placed);
  return { ...lib, view, placed };
}

/** The view's tree: each path, link target and file's bytes under `root`, for comparing two layouts. */
function treeDigest(root) {
  const hash = createHash('sha256');
  const walk = (dir) => {
    for (const name of readdirSync(dir).sort()) {
      const path = join(dir, name), at = relative(root, path), info = lstatSync(path);
      if (info.isSymbolicLink()) hash.update(`L ${at} ${readlinkSync(path)}\n`);
      else if (info.isDirectory()) { hash.update(`D ${at}\n`); walk(path); }
      else hash.update(`F ${at} ${digest(readFileSync(path))}\n`);
    }
  };
  walk(root);
  return hash.digest('hex');
}

/** Check the view at `view` against the hash-checked download, on every build, and lay it out again when it
 * differs. Nothing kept beside the view vouches for it: a Linux view's one file is the download's bytes, so its hash
 * is the declared one; an Apple view is compared, file by file, with a layout made now from the zip, which is then
 * discarded. Each invocation stages and sets aside under names of its own and removes only those, and publishes
 * with a rename that leaves a view another build published in place when that one checks out. */
function layOut(lib, file, view, placed) {
  const parent = dirname(view);
  mkdirSync(parent, { recursive: true });
  const fresh = () => { const stage = mkdtempSync(join(parent, `.view-${process.pid}-`)); try { (lib.kind === 'apple' ? appleView : linuxView)(lib, file, stage); return stage; } catch (e) { rmSync(stage, { recursive: true, force: true }); throw e; } };
  let stage = null;
  try {
    const valid = () => {
      if (!existsSync(placed)) return false;
      if (lib.kind === 'linux') return lstatSync(placed).isFile() && readdirSync(view).length === 1 && digest(readFileSync(placed)) === lib.sha256;
      stage ??= fresh();
      return treeDigest(stage) === treeDigest(view);
    };
    for (let attempt = 0; attempt < 3; attempt++) {
      if (valid()) return;
      stage ??= fresh();
      if (existsSync(view)) {
        const aside = mkdtempSync(join(parent, `.stale-${process.pid}-`));
        try { renameSync(view, join(aside, 'view')); } catch (e) { if (e.code !== 'ENOENT') throw e; }
        rmSync(aside, { recursive: true, force: true });
      }
      try { renameSync(stage, view); stage = null; return; } catch (e) { if (!['ENOTEMPTY', 'EEXIST'].includes(e.code)) throw e; }
      // Another build published a view meanwhile: it is checked as this one's was.
    }
    throw new Error(`${lib.package}: its ${lib.target} library's link inputs at ${view} keep changing while this build lays them out`);
  } finally { if (stage) rmSync(stage, { recursive: true, force: true }); }
}

/** A Linux view: the shared object, carried as `lib<link>.so`, which its SONAME must be: the linker records the
 * SONAME as the executable's need. One without a SONAME is refused, since what the linker records for it then
 * depends on how it found the file, which can be a path on the build machine. */
function linuxView(lib, file, stage) {
  const name = `lib${lib.link}.so`, found = sharedObjectName(readFileSync(file));
  if (found.error) throw new Error(`${lib.package}: its ${lib.target} library ${lib.file} is ${found.error}`);
  if (found.soname !== name) throw new Error(`${lib.package}: its ${lib.target} library's SONAME is ${found.soname ?? 'missing'}, and it is carried beside the executable as ${name}; link it with -Wl,-soname,${name}`);
  cpSync(file, join(stage, name));
}

/** The DT_SONAME of a 64-bit little-endian ELF shared object (System V ABI, "Dynamic Section"), read through its
 * section headers: `{ soname }`, null when it has none, or `{ error }` for what is not such a file. */
export function sharedObjectName(bytes) {
  try {
    if (bytes.length < 64 || bytes.readUInt32BE(0) !== 0x7f454c46) return { error: 'not an ELF file' };
    if (bytes[4] !== 2 || bytes[5] !== 1) return { error: 'not a 64-bit little-endian ELF file' };
    if (bytes.readUInt16LE(16) !== 3) return { error: 'not a shared object (ELF type ET_DYN)' };
    const at = (o) => Number(bytes.readBigUInt64LE(o)), shoff = at(0x28), size = bytes.readUInt16LE(0x3a), count = bytes.readUInt16LE(0x3c);
    const sections = Array.from({ length: count }, (_, i) => { const b = shoff + i * size; return { type: bytes.readUInt32LE(b + 4), offset: at(b + 0x18), size: at(b + 0x20), link: bytes.readUInt32LE(b + 0x28) }; });
    const dynamic = sections.find(s => s.type === 6);
    if (!dynamic) return { soname: null };
    const strings = sections[dynamic.link];
    for (let o = dynamic.offset; o + 16 <= dynamic.offset + dynamic.size; o += 16) {
      const tag = bytes.readBigInt64LE(o);
      if (tag === 0n) break;
      if (tag === 14n) { const start = strings.offset + at(o + 8), end = bytes.indexOf(0, start); if (end < 0) return { error: 'a malformed ELF file' }; return { soname: bytes.toString('latin1', start, end) }; }
    }
    return { soname: null };
  } catch { return { error: 'a malformed ELF file' }; }
}

/** The first symbolic link under `root` whose real target, every link on the way followed on disk, is outside
 * `within` or is not there, or null. */
function escapingLink(root, within = realpathSync(root)) {
  const here = realpathSync(root);
  for (const name of readdirSync(root)) {
    const path = join(root, name), info = lstatSync(path);
    if (info.isSymbolicLink()) {
      let real = null;
      try { real = realpathSync(path); } catch { /* it leads nowhere */ }
      if (!real || !inside(within, real)) return `${relative(within, join(here, name))} -> ${readlinkSync(path)}`;
    } else if (info.isDirectory()) { const found = escapingLink(path, within); if (found) return found; }
  }
  return null;
}

/** The target's framework from the zipped xcframework at `zip`, and the `lib<link>.dylib` the linker's `-l` finds. */
function appleView(lib, zip, stage) {
  if (process.platform !== 'darwin') throw new Error(`${lib.package}: its ${lib.target} library is an xcframework, which is read on a Mac`);
  const unzipped = join(stage, '.unzipped');
  const r = spawnSync('ditto', ['-x', '-k', zip, unzipped], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`${lib.package}: its ${lib.target} library ${lib.file} is not a zip: ${(r.stderr || '').trim()}`);
  const escaped = escapingLink(unzipped);
  if (escaped) throw new Error(`${lib.package}: ${lib.file} holds a link that leaves it (${escaped})`);
  const found = readdirSync(unzipped).filter(name => name.endsWith('.xcframework'));
  if (found.length !== 1) throw new Error(`${lib.package}: ${lib.file} holds ${found.length ? found.join(', ') : 'no .xcframework'} at its top; it holds one .xcframework`);
  const fw = join(unzipped, found[0]), slice = appleSlice(lib.target);
  let at;
  try { at = xcframeworkSlice(fw, slice); } catch (e) { throw new Error(`${lib.package}: its ${lib.target} library: ${e.message}`); }
  if (at.lib.LibraryPath !== `${lib.link}.framework`) throw new Error(`${lib.package}: its ${lib.target} slice is ${at.lib.LibraryPath}, and native.link names ${lib.link}: the slice is ${lib.link}.framework, a dynamic framework`);
  // Each place resolved on disk, with every link followed, before anything is copied out of the archive.
  const root = realpathSync(unzipped), named = join(at.dir, at.lib.LibraryPath);
  let source = null;
  try { source = realpathSync(named); } catch { /* not there */ }
  if (!source || !inside(root, source) || !lstatSync(source).isDirectory()) throw new Error(`${lib.package}: its ${lib.target} slice's ${at.lib.LibraryPath} is ${source ? `outside ${lib.file}` : 'not there'}`);
  const leaves = escapingLink(source);
  if (leaves) throw new Error(`${lib.package}: its ${lib.target} framework holds a link that leaves it (${leaves})`);
  let real = null;
  try { real = realpathSync(join(source, lib.link)); } catch { /* not there */ }
  if (!real || !inside(source, real) || !lstatSync(real).isFile()) throw new Error(`${lib.package}: its ${lib.target} framework has no binary ${lib.link}.framework/${lib.link} inside it`);
  const framework = join(stage, `${lib.link}.framework`), binary = join(framework, lib.link), inner = relative(source, real);
  cpSync(source, framework, { recursive: true, verbatimSymlinks: true });
  const id = spawnSync('otool', ['-D', binary], { encoding: 'utf8' }).stdout?.trim().split('\n').slice(1).join('').trim();
  const expected = `@rpath/${lib.link}.framework/${inner}`;
  if (id !== expected) throw new Error(`${lib.package}: its ${lib.target} framework's install name is ${id || 'missing (not a dynamic library)'}; an embedded framework is named ${expected} (link it with -install_name ${expected})`);
  symlinkSync(`${lib.link}.framework/${lib.link}`, join(stage, `lib${lib.link}.dylib`));
  rmSync(unzipped, { recursive: true, force: true });
}

/** The library for the app's build for `target`, fetched and checked; null when the app's dependencies declare
 * none, and for a web target, which links none. What every native build calls before Cargo (scripts/app.mjs
 * `buildBake`, scripts/build-linux.mjs, host/apple/build.mjs). */
export function prepareLibrary(appDir, target, options) {
  if (target.startsWith('wasm32-')) return null;
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
 * and name, and `rpath`, by default where an app bundle's `Frameworks` is from its executable; null names none (the
 * link names it already). */
export const appleLinkArgs = (lib, ios, rpath = ios ? '@executable_path/Frameworks' : '@executable_path/../Frameworks') => !lib ? []
  : ['-Xlinker', `-F${lib.view}`, '-Xlinker', '-framework', '-Xlinker', lib.link, ...(rpath ? ['-Xlinker', '-rpath', '-Xlinker', rpath] : [])];

/** The framework in an `--embed` output (host/apple/build.mjs), at `Frameworks/<link>.framework`, and what its
 * `capabilities.json` tells the consumer's link: the framework, and the run path to the bundle's `Frameworks`,
 * where the consumer embeds it, since its install name is `@rpath/…`. None without a library. */
export function embedLibrary(lib, embed, ios) {
  if (!lib) return {};
  placeLibrary(lib, join(embed, 'Frameworks'));
  const runPath = ios ? '@executable_path/Frameworks' : '@executable_path/../Frameworks';
  return { frameworks: [{ path: `Frameworks/${lib.link}.framework`, package: lib.package, sha256: lib.sha256, linkerFlags: ['-F', 'Frameworks', '-framework', lib.link, '-Xlinker', '-rpath', '-Xlinker', runPath], embed: true }] };
}
