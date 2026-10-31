// LLP 1154 D3, D6 and LLP 1133.001 D9: a package's prebuilt library is fetched by its hash before a build, refused
// by name when the bytes have another, and taken from the cache when they are there. A Linux target's library is
// the shared object itself, so these cases need no toolchain.
import { afterAll, expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { fetchLibrary, libraryFor, prepareLibrary } from './package-libraries.mjs';

const scratch = mkdtempSync(join(tmpdir(), 'exact-package-libraries-'));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));
const TARGET = 'x86_64-unknown-linux-gnu';
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** An app depending on each of `packages` ({name: exact.json}) by an installed directory, and the library's bytes. */
function fixture(name, packages) {
  const app = join(scratch, name);
  mkdirSync(app, { recursive: true });
  writeFileSync(join(app, 'package.json'), JSON.stringify({ dependencies: Object.fromEntries(Object.keys(packages).map(n => [n, '1.0.0'])) }));
  for (const [pkg, declaration] of Object.entries(packages)) {
    mkdirSync(join(app, 'node_modules', pkg), { recursive: true });
    writeFileSync(join(app, 'node_modules', pkg, 'package.json'), JSON.stringify({ name: pkg, version: '1.0.0' }));
    writeFileSync(join(app, 'node_modules', pkg, 'exact.json'), JSON.stringify(declaration));
  }
  return app;
}
const library = (name, bytes) => { const at = join(scratch, `${name}-libvault.so`); writeFileSync(at, bytes); return pathToFileURL(at).href; };

test('a library is fetched from a file: URL by its hash, laid out for the link, and taken from the cache after', async () => {
  const bytes = Buffer.from('a shared object only the hash knows'), cache = join(scratch, 'cache-fetch');
  const app = fixture('fetch', { vault: { native: { link: 'vault', library: { [TARGET]: { url: library('fetch', bytes), sha256: sha256(bytes) } } } } });
  const said = [];
  const lib = await prepareLibrary(app, TARGET, { cache, log: line => said.push(line) });
  expect(lib).toMatchObject({ package: 'vault', target: TARGET, link: 'vault', kind: 'linux' });
  expect(lib.placed).toBe(join(cache, sha256(bytes), TARGET, 'libvault.so'));
  expect(readFileSync(lib.placed)).toEqual(bytes);
  expect(said).toEqual([expect.stringContaining(`vault: fetching its ${TARGET} library`)]);
  // The second build reads the cache: nothing is fetched, and the laid-out file is the one the first build linked.
  const at = statSync(lib.placed).mtimeMs;
  rmSync(join(scratch, 'fetch-libvault.so'));
  const again = await prepareLibrary(app, TARGET, { cache, log: line => said.push(line) });
  expect([again.placed, statSync(again.placed).mtimeMs, said.length]).toEqual([lib.placed, at, 1]);
});

test('bytes with another hash are refused by package, target and both hashes, and leave nothing in the cache', async () => {
  const bytes = Buffer.from('the published library'), other = Buffer.from('what the URL now serves'), cache = join(scratch, 'cache-wrong');
  const app = fixture('wrong', { vault: { native: { link: 'vault', library: { [TARGET]: { url: library('wrong', other), sha256: sha256(bytes) } } } } });
  const refused = await prepareLibrary(app, TARGET, { cache, log: () => {} }).then(() => null, e => e.message);
  expect(refused).toBe(`vault: its ${TARGET} library ${library('wrong', other)} has SHA-256 ${sha256(other)}, and its exact.json says ${sha256(bytes)}; nothing was linked`);
  expect(existsSync(join(cache, sha256(bytes)))).toBe(false);
  // A cached file whose bytes changed is fetched again, and refused the same way.
  mkdirSync(join(cache, sha256(bytes)), { recursive: true });
  writeFileSync(join(cache, sha256(bytes), 'wrong-libvault.so'), other);
  expect(await fetchLibrary(libraryFor(app, TARGET), { cache, log: () => {} }).then(() => null, e => e.message)).toBe(refused);
});

test('a target the declaration does not name, a target exact2 cannot link, and two native packages are refused by name', () => {
  const bytes = Buffer.from('linux only'), url = library('targets', bytes);
  const app = fixture('targets', { vault: { native: { link: 'vault', library: { [TARGET]: { url, sha256: sha256(bytes) }, 'aarch64-linux-android': { url, sha256: sha256(bytes) } } } } });
  expect(() => libraryFor(app, 'aarch64-unknown-linux-gnu')).toThrow(`vault has no native library for aarch64-unknown-linux-gnu: its exact.json native.library names ${TARGET}, aarch64-linux-android. An app that builds for aarch64-unknown-linux-gnu waits for the package to publish one (LLP 1154 D6)`);
  expect(() => libraryFor(app, 'aarch64-linux-android')).toThrow('vault declares a native library for aarch64-linux-android, and exact2 links a package\'s library only for Apple and Linux targets');
  const apple = fixture('apple-file', { vault: { native: { link: 'vault', library: { 'aarch64-apple-darwin': { url, sha256: sha256(bytes) } } } } });
  expect(() => libraryFor(apple, 'aarch64-apple-darwin')).toThrow('native.library.aarch64-apple-darwin is a zip holding the .xcframework');
  const two = fixture('two', { vault: { native: { rust: 'native/lib.rs', class: 'Vault' } }, safe: { native: { rust: 'native/lib.rs', class: 'Safe' } } });
  expect(() => libraryFor(two, TARGET)).toThrow('bake-native-twice: safe and vault each declare `native`');
  // A native module without a library, and an app with none, link nothing.
  expect(libraryFor(fixture('shim', { vault: { native: { rust: 'native/lib.rs', class: 'Vault' } } }), TARGET)).toBe(null);
  expect(libraryFor(fixture('none', { plain: { kind: 'interface' } }), TARGET)).toBe(null);
});
