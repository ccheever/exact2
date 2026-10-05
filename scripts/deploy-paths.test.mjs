// LLP 1091 §11: capture admission uses native paths, not POSIX spellings.
// No Cargo, install, signing, publication or host process is needed here.
import { afterAll, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, resolve, toNamespacedPath } from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertCapturedContractSources, installedContractPackage, unrelocatableContractDependency } from './deploy.mjs';

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
  expect(() => assertCapturedContractSources(graph(inside, 'app', [join(fixture, 'stage/missing-package.json')]), join(fixture, 'stage'))).toThrow('ENOENT');
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
