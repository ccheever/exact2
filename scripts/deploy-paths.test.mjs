// Capture admission uses native paths, not POSIX spellings.
// Only Contract compiler discovery below runs Cargo; no install, signing, publication or host process is needed here.
import { afterAll, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, resolve, sep, toNamespacedPath } from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertCapturedContractSources, contractCompiler, installedContractPackage, namesNative, nativePlatforms, sourcePathspec, unrelocatableContractDependency } from './deploy.mjs';
import { pinned } from './test-workspace.mjs';

// The native platforms a deploy ships are the ones the manifest names, under deploy, else under
// host (library-gate and scale-gate name theirs there alone), and only a manifest that names none in either is web-only.
test('a deploy ships the native platforms the manifest names, and a manifest naming none is web-only', () => {
  for (const manifest of [{}, { deploy: {} }, { host: { web: {} }, deploy: { store: { web: '0' } } }]) {
    expect([nativePlatforms(manifest), namesNative(manifest)]).toEqual([[], false]);
  }
  expect(nativePlatforms({ host: { ios: {}, macos: {}, web: {} } })).toEqual(['ios', 'macos']);
  expect(nativePlatforms({ deploy: { store: { web: '0', linux: '0', ios: '0' } } })).toEqual(['ios', 'linux']);
  expect(nativePlatforms({ host: { ios: {}, macos: {} }, deploy: { store: { web: '0', ios: '0' } } })).toEqual(['ios']);
  expect(nativePlatforms({ deploy: { store: { web: '0' }, binaries: { macos: true } } })).toEqual(['macos']);
  for (const manifest of [{ host: { ios: {} } }, { host: { android: {} } }, { deploy: { binaries: { macos: true } } }]) expect(namesNative(manifest)).toBe(true);
  expect(() => nativePlatforms({ deploy: { store: { tvos: '0' } } })).toThrow(/names the platform tvos under deploy/);
});

// A deploy's snapshot takes the app's ignored files as source, but not a web build in `dist` nor what it
// keeps beside it (host/web/beside.mjs), here as a build, its replacements and a reap leave them.
test('a deploy snapshot leaves out dist and everything a build keeps beside it, after rebuilds and a reap', async () => {
  const { replaceBuild } = await import('../host/web/serve.mjs');
  const { leaseGeneration, reapGenerations } = await import('../host/web/generations.mjs');
  const repo = realpathSync(mkdtempSync(resolve(tmpdir(), 'exact-snapshot-dist-'))), dist = resolve(repo, 'dist');
  try {
    const write = (path, text) => { mkdirSync(dirname(resolve(repo, path)), { recursive: true }); writeFileSync(resolve(repo, path), text); };
    write('app.json', '{}'); write('.gitignore', '/dist/\n/dist.*\n/local.json\n/exact2/host/web/dist*\n'); write('local.json', '{}');
    // The checkout's own web build keeps the same names beside it (an app of the checkout builds into host/web/dist).
    for (const path of ['exact2/host/web/dist/app.js', 'exact2/host/web/dist.gen/a1/app.js', 'exact2/host/web/dist.lock', 'exact2/host/web/dist.gen.reaped', 'exact2/host/web/dist.stage-b2/app.js']) write(path, 'a build');
    const git = (...args) => spawnSync('git', args, { cwd: repo, encoding: 'utf8' });
    git('init', '-q'); git('add', '-A'); git('-c', 'user.name=t', '-c', 'user.email=t@example.invalid', 'commit', '-qm', 'app');
    for (const k of [1, 2, 3, 4]) {
      write(`stage-${k}/app.js`, `build ${k}`); write(`stage-${k}/.exact-build.json`, JSON.stringify({ files: [{ name: 'app.js', sha256: String(k) }] }));
      replaceBuild(resolve(repo, `stage-${k}`), dist);
    }
    reapGenerations(dist, { newest: 1, ageMs: 0, bytes: 2 ** 30 });
    const release = leaseGeneration(dist, readdirSync(`${dist}.gen`)[0]);
    write('dist.lock', '1'); write('dist.stage-a1b2c3/app.js', 'half a build'); write('dist.previous/app.js', 'aside');
    const ignored = git('ls-files', '-z', '--others', '--ignored', '--exclude-standard', '--', ...sourcePathspec(repo, { dir: repo, workspace: repo, manifest: {} }, resolve(repo, 'exact2')));
    expect(ignored.status, ignored.stderr).toBe(0);
    expect(existsSync(`${dist}.gen.reaped`)).toBe(true);
    expect(ignored.stdout.split('\0').filter(Boolean)).toEqual(['local.json']);
    release();
  } finally { rmSync(repo, { recursive: true, force: true }); }
});

const parent = realpathSync.native(tmpdir());
const fixture = mkdtempSync(join(parent, 'exact deploy paths café # '));
const put = (name, text = 'fixture') => {
  const path = join(fixture, name);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
  return realpathSync.native(path);
};
const graph = (path, origin = 'app', consulted = []) => ({ sources: [{ path, origin }], consulted });
afterAll(() => {
  if (dirname(resolve(fixture)) !== parent) throw new Error('Refusing fixture cleanup outside its owned parent');
  rmSync(fixture, { recursive: true, force: true });
});

function compilerRoot() {
  const root = mkdtempSync(join(fixture, 'compiler-'));
  mkdirSync(join(root, 'src'));
  writeFileSync(join(root, 'Cargo.toml'), '[package]\nname="contract"\nversion="0.1.0"\nedition="2021"\n');
  writeFileSync(join(root, 'src/main.rs'), 'fn main() {}\n');
  pinned(root);
  const lock = spawnSync('cargo', ['generate-lockfile', '--offline'], {cwd:root, encoding:'utf8'});
  expect(lock.status).toBe(0);
  return root;
}

test('Contract discovery builds in the selected private Cargo target', () => {
  const root = compilerRoot(), target = mkdtempSync(join(fixture, 'cargo-target-'));
  const previous = process.env.CARGO_TARGET_DIR;
  try {
    process.env.CARGO_TARGET_DIR = target;
    const compiler = contractCompiler(root);
    expect(compiler.startsWith(`${target}${sep}`)).toBe(true);
    expect(existsSync(compiler)).toBe(true);
    expect(existsSync(join(root, 'target', 'debug', 'contract'))).toBe(false);
  } finally {
    if (previous === undefined) delete process.env.CARGO_TARGET_DIR;
    else process.env.CARGO_TARGET_DIR = previous;
  }
}, 30000);

test('Contract discovery uses Cargo executable for the selected build target', () => {
  const root = compilerRoot();
  const rustc = spawnSync('rustc', ['-vV'], {encoding:'utf8'});
  expect(rustc.status).toBe(0);
  const host = /^host: (.+)$/m.exec(rustc.stdout)?.[1];
  expect(host).toBeTruthy();
  const previous = process.env.CARGO_BUILD_TARGET;
  try {
    process.env.CARGO_BUILD_TARGET = host;
    const compiler = contractCompiler(root);
    expect(compiler.includes(`${sep}${host}${sep}debug${sep}`)).toBe(true);
    expect(existsSync(compiler)).toBe(true);
  } finally {
    if (previous === undefined) delete process.env.CARGO_BUILD_TARGET;
    else process.env.CARGO_BUILD_TARGET = previous;
  }
}, 30000);

test('actual app and installed package paths stay within the captured source', () => {
  const app = put('stage/app/app.contract');
  const packageFile = put('stage/app/node_modules/@fixture/ui/card.contract');
  const manifest = put('stage/app/node_modules/@fixture/ui/package.json', '{}');
  expect(() => assertCapturedContractSources({
    sources: [{ path: app, origin: 'app' }, { path: packageFile, origin: 'package' }, { path: 'exact:motion', origin: 'builtin' }],
    consulted: [manifest],
  }, join(fixture, 'stage'))).not.toThrow();
  expect(() => assertCapturedContractSources(graph(toNamespacedPath(app)), toNamespacedPath(join(fixture, 'stage')))).not.toThrow();
});

test('outside physical sources and consulted manifests cannot be silently filtered', () => {
  const inside = put('stage/app/inside.contract');
  const outside = put('stage-other/outside.contract');
  const manifest = put('elsewhere/package.json', '{}');
  for (const origin of ['app', 'package']) {
    expect(() => assertCapturedContractSources(graph(outside, origin), join(fixture, 'stage'))).toThrow('outside the captured snapshot');
  }
  expect(() => assertCapturedContractSources(graph(inside, 'app', [manifest]), join(fixture, 'stage'))).toThrow('outside the captured snapshot');
  for (const path of ['./app.contract', 'exact:motion', '', null]) {
    expect(() => assertCapturedContractSources(graph(path), join(fixture, 'stage'))).toThrow('not an absolute filesystem path');
  }
  const missing = join(fixture, 'stage/missing.contract');
  expect(() => assertCapturedContractSources(graph(missing), join(fixture, 'stage'))).toThrow('ENOENT');
  // A consulted path that does not exist is where resolution looked and found
  // nothing (a nearer install candidate of a hoisted package): nothing to capture.
  expect(() => assertCapturedContractSources(graph(inside, 'app', [join(fixture, 'stage/missing-package.json')]), join(fixture, 'stage'))).not.toThrow();
  expect(() => assertCapturedContractSources(graph(inside, 'app', [join(fixture, 'nowhere/node_modules/ui/package.json')]), join(fixture, 'stage'))).not.toThrow();
});

test('a real junction or directory symlink cannot lead outside the capture', () => {
  const outside = put('outside-linked/card.contract');
  const link = join(fixture, 'stage/linked');
  symlinkSync(dirname(outside), link, process.platform === 'win32' ? 'junction' : 'dir');
  expect(() => assertCapturedContractSources(graph(join(link, 'card.contract'), 'package'), join(fixture, 'stage')))
    .toThrow('outside the captured snapshot');
});

test('file dependencies reject actual native roots and retain relocatable references', () => {
  const file = put('library/package.json', '{}');
  expect(unrelocatableContractDependency(`file:${dirname(file)}`)).toBe(true);
  expect(unrelocatableContractDependency(pathToFileURL(dirname(file)).href)).toBe(true);
  for (const spec of ['file:../library', 'file:./library', 'file:library', 'workspace:*', '^1.0.0', null])
    expect(unrelocatableContractDependency(spec)).toBe(false);
  for (const spec of ['file:/absolute', 'file://server/share/library', 'link:../library', 'link:library'])
    expect(unrelocatableContractDependency(spec)).toBe(true);
});

test.skipIf(process.platform !== 'win32')('Windows roots include drive-relative and UNC syntax without remote I/O', () => {
  // No share or second drive is opened. Existing extended paths are exercised above.
  for (const target of ['C:/library', 'C:\\library', 'C:library', '\\library', '\\\\server\\share\\library', '\\\\?\\C:\\library'])
    expect(unrelocatableContractDependency(`file:${target}`)).toBe(true);
  expect(isAbsolute('\\\\server\\share\\card.contract')).toBe(true);
  expect(() => assertCapturedContractSources(graph('C:relative.contract'), join(fixture, 'stage'))).toThrow('not an absolute filesystem path');
});

test('registry classification uses exact native directory components', () => {
  const app = dirname(put('registry-app/app.contract'));
  for (const name of ['registry-app/node_modules/ui/card.contract', 'registry-app/node_modules/@fixture/ui/card.contract', 'registry-app/deep/node_modules/ui/card.contract']) {
    expect(installedContractPackage(app, dirname(put(name)))).toBe(true);
  }
  for (const name of ['registry-app/node_modules-extra/ui/card.contract', 'local-library/card.contract', 'registry-app/node_modules/card.contract']) {
    expect(installedContractPackage(app, dirname(put(name)))).toBe(false);
  }
});

test.skipIf(process.platform !== 'win32')('ordinary case aliases canonicalize before containment', () => {
  const file = put('ordinary-stage/café/card.contract');
  expect(() => assertCapturedContractSources(graph(file.toUpperCase()), join(fixture, 'ordinary-stage').toUpperCase())).not.toThrow();
});

let caseRoot;
let caseMode = { available: false, reason: 'Windows-only NTFS qualification' };
if (process.platform === 'win32') {
  caseRoot = join(fixture, 'case-sensitive');
  mkdirSync(caseRoot);
  const result = spawnSync('fsutil.exe', ['file', 'setCaseSensitiveInfo', caseRoot, 'enable'], { encoding: 'utf8', windowsHide: true });
  if (result.error) throw result.error;
  caseMode = { available: result.status === 0, reason: `${result.status}: ${result.stdout}${result.stderr}`.trim() };
  if (!caseMode.available) console.warn(`SKIP real NTFS case-sensitive capture: ${caseMode.reason}`);
}
test.skipIf(!caseMode.available)('actual case-sensitive stage and STAGE are distinct capture roots', () => {
  const lower = join(caseRoot, 'stage');
  const upper = join(caseRoot, 'STAGE');
  mkdirSync(lower);
  mkdirSync(upper);
  const inside = join(lower, 'inside.contract'), outside = join(upper, 'outside.contract');
  writeFileSync(inside, 'inside');
  writeFileSync(outside, 'outside');
  expect(realpathSync.native(lower)).not.toBe(realpathSync.native(upper));
  expect(() => assertCapturedContractSources(graph(inside), lower)).not.toThrow();
  expect(() => assertCapturedContractSources(graph(outside), lower)).toThrow('outside the captured snapshot');
});
