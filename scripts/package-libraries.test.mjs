// LLP 1154 D3, D6 and LLP 1133.001 D9: a package's prebuilt library is fetched by its hash before a build, refused
// by name when the bytes have another, and taken from the cache when they are there; what the cache lays out for the
// link is checked on every build. A Linux target's library is the shared object itself, made here as a minimal ELF
// file, so these cases need no toolchain; the xcframework cases need a Mac's `ditto` and `plutil`.
import { afterAll, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { fetchLibrary, libraryFor, prepareLibrary, sharedObjectName } from './package-libraries.mjs';

const scratch = mkdtempSync(join(tmpdir(), 'exact-package-libraries-'));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));
const TARGET = 'x86_64-unknown-linux-gnu';
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const quiet = { log: () => {} };

/** A 64-bit little-endian ELF shared object holding a dynamic section with `soname` (none when null) and `tail`. */
function elf(soname, tail = '') {
  const strings = Buffer.from(`\0${soname ?? ''}\0${tail}\0`), dynamic = Buffer.alloc(32), header = Buffer.alloc(64);
  if (soname !== null) { dynamic.writeBigInt64LE(14n, 0); dynamic.writeBigUInt64LE(1n, 8); }
  const stringsAt = 64, dynamicAt = stringsAt + strings.length, sectionsAt = dynamicAt + dynamic.length, sections = Buffer.alloc(3 * 64);
  const section = (i, type, offset, size, link) => { const b = i * 64; sections.writeUInt32LE(type, b + 4); sections.writeBigUInt64LE(BigInt(offset), b + 0x18); sections.writeBigUInt64LE(BigInt(size), b + 0x20); sections.writeUInt32LE(link, b + 0x28); };
  section(1, 3, stringsAt, strings.length, 0);
  section(2, 6, dynamicAt, dynamic.length, 1);
  header.writeUInt32BE(0x7f454c46, 0); header[4] = 2; header[5] = 1; header[6] = 1;
  header.writeUInt16LE(3, 16); header.writeUInt16LE(62, 18); header.writeUInt32LE(1, 20);
  header.writeBigUInt64LE(BigInt(sectionsAt), 0x28); header.writeUInt16LE(64, 0x34); header.writeUInt16LE(64, 0x3a); header.writeUInt16LE(3, 0x3c);
  return Buffer.concat([header, strings, dynamic, sections]);
}

/** An app depending on each of `packages` ({name: exact.json text or object}) by an installed directory. */
function fixture(name, packages) {
  const app = join(scratch, name);
  mkdirSync(app, { recursive: true });
  writeFileSync(join(app, 'package.json'), JSON.stringify({ dependencies: Object.fromEntries(Object.keys(packages).map(n => [n, '1.0.0'])) }));
  for (const [pkg, declaration] of Object.entries(packages)) {
    mkdirSync(join(app, 'node_modules', pkg), { recursive: true });
    writeFileSync(join(app, 'node_modules', pkg, 'package.json'), JSON.stringify({ name: pkg, version: '1.0.0' }));
    writeFileSync(join(app, 'node_modules', pkg, 'exact.json'), typeof declaration === 'string' ? declaration : JSON.stringify(declaration));
  }
  return app;
}
const library = (name, bytes) => { const at = join(scratch, `${name}-libvault.so`); writeFileSync(at, bytes); return pathToFileURL(at).href; };
const declares = (url, bytes, link = 'vault', target = TARGET) => ({ vault: { native: { link, library: { [target]: { url, sha256: sha256(bytes) } } } } });
const refusal = (run) => { try { run(); return null; } catch (e) { return e.message; } };

test('a library is fetched from a file: URL by its hash, laid out for the link, and taken from the cache after', () => {
  const bytes = elf('libvault.so'), cache = join(scratch, 'cache-fetch');
  const app = fixture('fetch', declares(library('fetch', bytes), bytes));
  const said = [];
  const lib = prepareLibrary(app, TARGET, { cache, log: line => said.push(line) });
  expect(lib).toMatchObject({ package: 'vault', target: TARGET, link: 'vault', kind: 'linux' });
  expect(lib.placed).toBe(join(cache, sha256(bytes), TARGET, 'vault', 'libvault.so'));
  expect(readFileSync(lib.placed)).toEqual(bytes);
  expect(said).toEqual([expect.stringContaining(`vault: fetching its ${TARGET} library`)]);
  // The second build reads the cache: nothing is fetched, and the laid-out file is the one the first build linked.
  const at = statSync(lib.placed).mtimeMs;
  rmSync(join(scratch, 'fetch-libvault.so'));
  const again = prepareLibrary(app, TARGET, { cache, log: line => said.push(line) });
  expect([again.placed, statSync(again.placed).mtimeMs, said.length]).toEqual([lib.placed, at, 1]);
  // A laid-out file that changed is laid out again from the checked download, and a removed one too; what another
  // build is staging or setting aside beside it is left alone.
  const others = ['.view-1-other', '.stale-1-other'].map(name => join(cache, sha256(bytes), TARGET, name));
  for (const other of others) mkdirSync(other);
  writeFileSync(lib.placed, 'other bytes');
  expect(readFileSync(prepareLibrary(app, TARGET, { cache, ...quiet }).placed)).toEqual(bytes);
  rmSync(lib.placed);
  expect(readFileSync(prepareLibrary(app, TARGET, { cache, ...quiet }).placed)).toEqual(bytes);
  expect(others.map(existsSync)).toEqual([true, true]);
  // The same bytes under another link name are laid out apart (by link name) and refused there, since their SONAME
  // names the first link; nothing collides with the view above, which stays.
  const renamed = fixture('renamed', declares(library('renamed', bytes), bytes, 'vault2'));
  expect(refusal(() => prepareLibrary(renamed, TARGET, { cache, ...quiet }))).toBe(`vault: its ${TARGET} library's SONAME is libvault.so, and it is carried beside the executable as libvault2.so; link it with -Wl,-soname,libvault2.so`);
  expect(readFileSync(lib.placed)).toEqual(bytes);
});

test('bytes with another hash are refused by package, target and both hashes, and leave nothing in the cache', () => {
  const bytes = elf('libvault.so'), other = elf('libvault.so', 'served instead'), cache = join(scratch, 'cache-wrong');
  const app = fixture('wrong', declares(library('wrong', other), bytes));
  const refused = refusal(() => prepareLibrary(app, TARGET, { cache, ...quiet }));
  expect(refused).toBe(`vault: its ${TARGET} library ${library('wrong', other)} has SHA-256 ${sha256(other)}, and its exact.json says ${sha256(bytes)}; nothing was linked`);
  expect(existsSync(join(cache, sha256(bytes), 'wrong-libvault.so'))).toBe(false);
  // A cached file whose bytes changed is fetched again, refused the same way, and removed.
  mkdirSync(join(cache, sha256(bytes)), { recursive: true });
  writeFileSync(join(cache, sha256(bytes), 'wrong-libvault.so'), other);
  expect(refusal(() => fetchLibrary(libraryFor(app, TARGET), { cache, ...quiet }))).toBe(refused);
  expect(existsSync(join(cache, sha256(bytes), 'wrong-libvault.so'))).toBe(false);
});

test('a shared object whose SONAME is not lib<link>.so is refused before it is linked', () => {
  expect([sharedObjectName(elf('libvault.so')), sharedObjectName(elf(null)), sharedObjectName(Buffer.from('not elf'))]).toEqual([{ soname: 'libvault.so' }, { soname: null }, { error: 'not an ELF file' }]);
  const cache = join(scratch, 'cache-soname');
  for (const [name, soname, said] of [['versioned', 'libvault.so.1', 'libvault.so.1'], ['unnamed', null, 'missing']]) {
    const bytes = elf(soname), app = fixture(name, declares(library(name, bytes), bytes));
    expect(refusal(() => prepareLibrary(app, TARGET, { cache, ...quiet }))).toBe(`vault: its ${TARGET} library's SONAME is ${said}, and it is carried beside the executable as libvault.so; link it with -Wl,-soname,libvault.so`);
  }
  const text = Buffer.from('a text file'), app = fixture('text', declares(library('text', text), text));
  expect(refusal(() => prepareLibrary(app, TARGET, { cache, ...quiet }))).toBe(`vault: its ${TARGET} library text-libvault.so is not an ELF file`);
});

test('a target the declaration does not name, one exact2 cannot link, an http: URL and a bad link name are refused by name', () => {
  const bytes = elf('libvault.so'), url = library('targets', bytes);
  const app = fixture('targets', { vault: { native: { link: 'vault', library: { [TARGET]: { url, sha256: sha256(bytes) }, 'aarch64-linux-android': { url, sha256: sha256(bytes) }, 'armv7-linux-androideabi': { url, sha256: sha256(bytes) } } } } });
  expect(refusal(() => libraryFor(app, 'aarch64-unknown-linux-gnu'))).toBe(`vault has no native library for aarch64-unknown-linux-gnu: its exact.json native.library names ${TARGET}, aarch64-linux-android, armv7-linux-androideabi. An app that builds for aarch64-unknown-linux-gnu waits for the package to publish one (LLP 1154 D6)`);
  for (const android of ['aarch64-linux-android', 'armv7-linux-androideabi']) expect(refusal(() => libraryFor(app, android))).toStartWith(`vault declares a native library for ${android}, and exact2 links a package's library only for Apple and Linux targets`);
  expect(refusal(() => libraryFor(fixture('http', declares('http://example.com/libvault.so', bytes)), TARGET))).toBe(`vault's exact.json: native.library.${TARGET}.url is a file: or https: URL, not http:`);
  expect(refusal(() => libraryFor(fixture('apple-file', declares(url, bytes, 'vault', 'aarch64-apple-darwin')), 'aarch64-apple-darwin'))).toContain('native.library.aarch64-apple-darwin is a zip holding the .xcframework');
  expect(refusal(() => libraryFor(fixture('newline', declares(url, bytes, 'vault\ncargo:rustc-link-arg=-x')), TARGET))).toStartWith("vault's exact.json: native.link names the library");
  // A declaration that does not parse and two native packages are the bake's to refuse (js/bake/src/sources_tests.rs):
  // nothing is fetched for them.
  expect(libraryFor(fixture('unparsed', { vault: '{"native": ' }), TARGET)).toBe(null);
  const two = fixture('two', { vault: declares(url, bytes).vault, safe: declares(url, bytes).vault });
  expect(libraryFor(two, TARGET)).toBe(null);
  // A native module without a library, an app with none, and a web target link nothing.
  expect(libraryFor(fixture('shim', { vault: { native: { rust: 'native/lib.rs', class: 'Vault' } } }), TARGET)).toBe(null);
  expect(libraryFor(fixture('none', { plain: { kind: 'interface' } }), TARGET)).toBe(null);
  expect(prepareLibrary(app, 'wasm32-unknown-unknown')).toBe(null);
});

test.skipIf(process.platform !== 'darwin')('an xcframework whose slice or links lead outside the archive is refused', () => {
  const plist = (identifier) => `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>AvailableLibraries</key><array><dict><key>LibraryIdentifier</key><string>${identifier}</string><key>LibraryPath</key><string>vault.framework</string><key>SupportedArchitectures</key><array><string>arm64</string></array><key>SupportedPlatform</key><string>macos</string></dict></array><key>CFBundlePackageType</key><string>XFWK</string></dict></plist>
`;
  const zipped = (name, identifier, link) => {
    const dir = join(scratch, `xc-${name}`), fw = join(dir, 'vault.xcframework');
    mkdirSync(join(fw, 'macos-arm64/vault.framework'), { recursive: true });
    writeFileSync(join(fw, 'Info.plist'), plist(identifier));
    if (link) symlinkSync(link, join(fw, 'macos-arm64/vault.framework/vault'));
    const zip = join(scratch, `${name}.xcframework.zip`);
    expect(spawnSync('ditto', ['-c', '-k', '--keepParent', 'vault.xcframework', zip], { cwd: dir }).status).toBe(0);
    const bytes = readFileSync(zip);
    return fixture(`xc-app-${name}`, declares(pathToFileURL(zip).href, bytes, 'vault', 'aarch64-apple-darwin'));
  };
  const cache = join(scratch, 'cache-xc'), at = (app) => refusal(() => prepareLibrary(app, 'aarch64-apple-darwin', { cache, ...quiet }));
  expect(at(zipped('absolute', '/System/Library/Frameworks'))).toBe('vault: its aarch64-apple-darwin library: vault.xcframework\'s macos slice names /System/Library/Frameworks/vault.framework, outside the xcframework');
  expect(at(zipped('escape', 'macos-arm64', '../../../../outside'))).toBe('vault: escape.xcframework.zip holds a link that leaves it (vault.xcframework/macos-arm64/vault.framework/vault -> ../../../../outside)');
  expect(at(zipped('framework-escape', 'macos-arm64', '../../Info.plist'))).toBe('vault: its aarch64-apple-darwin framework holds a link that leaves it (vault -> ../../Info.plist)');
});

/** A zipped xcframework holding one macOS framework, `vault.framework`, with `edit(fw)` to change its tree first;
 * its binary a dylib clang links, with `@rpath` install name and `bytes` padding it to that size. */
function xcframework(name, { bytes = 0, edit = () => {} } = {}) {
  const dir = join(scratch, `xcf-${name}`), fw = join(dir, 'vault.xcframework'), framework = join(fw, 'macos-arm64/vault.framework');
  mkdirSync(join(framework, 'Versions/A'), { recursive: true });
  writeFileSync(join(dir, 'vault.c'), `int vaultlib_answer(void) { return 4217; }\nconst char vault_padding[${Math.max(bytes, 1)}] = {1};\n`);
  expect(spawnSync('clang', ['-dynamiclib', '-arch', 'arm64', '-install_name', '@rpath/vault.framework/Versions/A/vault', join(dir, 'vault.c'), '-o', join(framework, 'Versions/A/vault')]).status).toBe(0);
  symlinkSync('A', join(framework, 'Versions/Current'));
  symlinkSync('Versions/Current/vault', join(framework, 'vault'));
  writeFileSync(join(fw, 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>AvailableLibraries</key><array><dict><key>LibraryIdentifier</key><string>macos-arm64</string><key>LibraryPath</key><string>vault.framework</string><key>SupportedArchitectures</key><array><string>arm64</string></array><key>SupportedPlatform</key><string>macos</string></dict></array><key>CFBundlePackageType</key><string>XFWK</string></dict></plist>
`);
  edit(fw);
  const zip = join(scratch, `${name}.xcframework.zip`);
  expect(spawnSync('ditto', ['-c', '-k', '--keepParent', 'vault.xcframework', zip], { cwd: dir }).status).toBe(0);
  const zipped = readFileSync(zip);
  return fixture(`xcf-app-${name}`, declares(pathToFileURL(zip).href, zipped, 'vault', 'aarch64-apple-darwin'));
}

test.skipIf(process.platform !== 'darwin')('an Apple view is checked against a layout made now from the zip, and a link chain is followed on disk', () => {
  const cache = join(scratch, 'cache-provenance'), app = xcframework('provenance');
  const lib = prepareLibrary(app, 'aarch64-apple-darwin', { cache, ...quiet }), binary = join(lib.placed, 'Versions/A/vault'), good = readFileSync(binary);
  // A view swapped for another version's, whole, is laid out again from the hash-checked zip.
  writeFileSync(binary, Buffer.concat([good, Buffer.from('another version')]));
  writeFileSync(join(lib.view, 'stray'), 'left by someone else');
  prepareLibrary(app, 'aarch64-apple-darwin', { cache, ...quiet });
  expect([readFileSync(binary).equals(good), existsSync(join(lib.view, 'stray'))]).toEqual([true, false]);
  // What the check costs on a framework of 30 MB: said, not bounded, as the machine's load decides it.
  const big = xcframework('big', { bytes: 30 << 20 }), started = performance.now();
  prepareLibrary(big, 'aarch64-apple-darwin', { cache, ...quiet });
  const first = performance.now(); prepareLibrary(big, 'aarch64-apple-darwin', { cache, ...quiet });
  console.log(`a 30 MB framework: laid out in ${Math.round(first - started)} ms, checked in ${Math.round(performance.now() - first)} ms`);
  // `up` leads back to the xcframework on disk, so `l`, through it, leads four levels above the xcframework, out of
  // the extraction into the cache's own directory, though its path read lexically ends at the xcframework's top.
  // Both places exist, so a resolver that normalized the path first would find the inside one and pass it.
  const chained = xcframework('chained', { edit: (fw) => {
    mkdirSync(join(fw, 'a/b/c'), { recursive: true });
    symlinkSync('../../..', join(fw, 'a/b/c/up'));
    symlinkSync('a/b/c/up/../../../../outside', join(fw, 'l'));
    writeFileSync(join(fw, 'outside'), 'the lexical destination, inside the archive');
  } });
  const outside = join(cache, sha256(readFileSync(join(scratch, 'chained.xcframework.zip'))), 'outside');
  mkdirSync(join(outside, '..'), { recursive: true });
  writeFileSync(outside, 'the destination on disk, outside the extraction');
  expect(refusal(() => prepareLibrary(chained, 'aarch64-apple-darwin', { cache, ...quiet }))).toBe('vault: chained.xcframework.zip holds a link that leaves it (vault.xcframework/l -> a/b/c/up/../../../../outside)');
});
