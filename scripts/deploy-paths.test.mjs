// LLP 1091 §11: capture admission uses native paths, not POSIX spellings.
// Only Contract compiler discovery below runs Cargo; no install, signing, publication or host process is needed here.
import { afterAll, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, resolve, sep, toNamespacedPath } from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertCapturedContractSources, contractCompiler, installedContractPackage, unrelocatableContractDependency } from './deploy.mjs';
import * as asyncLane from './async.mjs';
const {appleTouched, glueTests, retainSkippedFailures} = asyncLane;

test('async retains only failures from checks skipped on this commit', () => {
  const previous = ['ios: failed', 'ios-touch: failed', 'lane: failed', 'tvos: failed', 'install: failed'];
  const skipped = {checks:{}, failures:[]};
  retainSkippedFailures(previous, skipped);
  expect(skipped.failures).toEqual(previous.slice(0, 4));
  expect(['ios-touch: failed'].filter(f => !new Set(skipped.failures).has(f))).toEqual([]);
  const passed = {checks:{'ios-touch':{status:0}}, failures:[]};
  retainSkippedFailures(previous, passed);
  expect(passed.failures).toEqual(['ios: failed', 'lane: failed', 'tvos: failed']);
  const iosPassed = {checks:{ios:{status:0}}, failures:[]};
  retainSkippedFailures(previous, iosPassed);
  expect(iosPassed.failures).toEqual(['ios-touch: failed', 'lane: failed', 'tvos: failed']);
  const other = {checks:{}, failures:[]};
  retainSkippedFailures(['optional: failed'], other, ['optional']);
  expect(other.failures).toEqual(['optional: failed']);
});

test('async newest detects Apple edits anywhere in the coalesced range', () => {
  const calls = [];
  expect(appleTouched('new', 'old', args => {
    calls.push(args);
    return args[2] === 'old' ? 'host/apple/changed.swift' : '';
  })).toBe(true);
  expect(calls[0]).toEqual(['diff', '--name-only', 'old', 'new']);
  appleTouched('new', null, args => { calls.push(args); return ''; });
  expect(calls[1]).toEqual(['diff', '--name-only', 'new^', 'new']);
});

test('async iOS schedules every non-documentation change in the coalesced range', () => {
  const changed = [
    'scripts/fixtures/keyboard-form.contract',
    'scripts/smoke-touch.mjs',
    'scripts/app.mjs',
    'host/web/src/lib.rs',
    'kernel/src/lib.rs',
    'gpu/src/acquire.rs',
    'host/svg-raster/src/lib.rs',
    'apps/exact-live/runbook.md',
    'apps/caltrain/assets/info.md',
  ];
  for (const path of changed) {
    expect(appleTouched('new', 'old', () => `docs/reference.md\n${path}`)).toBe(true);
  }
  const docs = [
    'llp/1000-exact2-root.explainer.md',
    'llp/current/1000',
    'issues/20261006-ios-lane-dependency-trigger.md',
    'QUEUE.md',
    'rules/RULES.md',
    'docs/reference.md',
    'docs/generated/index.html',
  ].join('\n');
  expect(appleTouched('new', 'old', () => docs)).toBe(false);
  expect(appleTouched('new', 'old', () => '')).toBe(false);
  for (const [paths, scheduled] of [['docs/reference.md', false], ['docs/reference.md\nscripts/app.mjs', true]]) {
    const apple = appleTouched('new', 'old', () => paths);
    expect(apple).toBe(scheduled);
    const steps = asyncLane.checks('new', 'old', {lane: [], apple, glue: []});
    for (const name of ['ios', 'ios-touch', 'tvos']) expect(steps.some(([step]) => step === name)).toBe(scheduled);
  }
});

test('async compiles tvOS per commit with no simulator, and tier 2 installs it', () => {
  const steps = asyncLane.checks('new', 'old', {lane: [], apple: true, glue: []});
  expect(steps.find(([name]) => name === 'tvos')).toEqual(['tvos', 'env', ['EXACT_JS_ENGINE=stub', 'bun', 'host/apple/build.mjs', '--tvos', '--no-install']]);
  expect(steps.filter(([name]) => name === 'tvos').length).toBe(1);
  expect(asyncLane.tierTwoChecks().find(([name]) => name === 'tvos')).toEqual(['tvos', 'env', ['EXACT_JS_ENGINE=stub', 'bun', 'host/apple/build.mjs', '--tvos']]);
  // Skipped on a documentation-only range, the step keeps its last failure until it runs again.
  const skipped = {checks: {}, failures: []};
  retainSkippedFailures(['tvos: IOS/RemoteTVOS.swift: cannot find type'], skipped);
  expect(skipped.failures).toEqual(['tvos: IOS/RemoteTVOS.swift: cannot find type']);
  const ran = {checks: {tvos: {status: 0}}, failures: []};
  retainSkippedFailures(['tvos: IOS/RemoteTVOS.swift: cannot find type'], ran);
  expect(ran.failures).toEqual([]);
});

test('tier 2 checks platform host and app crates for their actual target', () => {
  const checks = asyncLane.tierTwoChecks();
  for (const name of ['linux', 'windows']) expect(checks.find(([step]) => step === name)?.[2]).toContain('CRATE_CC_NO_DEFAULTS=1');
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('x86_64-unknown-linux-gnu');
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('--keep-going');
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('caltrain-linux');
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('CC_x86_64_unknown_linux_gnu=zig cc -target x86_64-linux-gnu');
  expect(checks.find(([name]) => name === 'windows')?.[2]).toContain('x86_64-pc-windows-gnu');
  expect(checks.find(([name]) => name === 'windows')?.[2]).toContain('--keep-going');
  expect(checks.find(([name]) => name === 'windows')?.[2]).toContain('windows-desk');
  expect(checks.find(([name]) => name === 'windows')?.[2]).toContain('CC_x86_64_pc_windows_gnu=zig cc -target x86_64-windows-gnu');
});

test('async game step checks the separate workspace with its lockfile', () => {
  const steps = asyncLane.checks('new', 'old', {lane: [], apple: false, glue: []});
  expect(steps.find(([name]) => name === 'game')).toEqual(['game', 'cargo', ['test', '--manifest-path', 'game/Cargo.toml', '--workspace', '--locked', '--lib', '--bins', '--tests', '--no-fail-fast']]);
});

test('async glue runs the capture fixtures', () => {
  expect(glueTests(resolve(import.meta.dir, '..'))).toContain('./scripts/app.test.mjs');
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
