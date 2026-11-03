// LLP 1091 §11: capture admission uses native paths, not POSIX spellings.
// Only Contract compiler discovery below runs Cargo; no install, signing, publication or host process is needed here.
import { afterAll, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, resolve, sep, toNamespacedPath } from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertCapturedContractSources, contractCompiler, installedContractPackage, namesNative, nativePlatforms, sourcePathspec, unrelocatableContractDependency } from './deploy.mjs';
import * as asyncLane from './async.mjs';
import { pinned } from './test-workspace.mjs';
const {changedPaths, failures, owners, retainSkippedFailures} = asyncLane;

// LLP 1153 D1.1, D1.5: the native platforms a deploy ships are the ones the manifest names, under deploy, else under
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

// LLP 1153 D3: a deploy's snapshot takes the app's ignored files as source, but not a web build in `dist` nor what it
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

test('async keeps a failure only when it was not found again and nothing ran to it', () => {
  const previous = ['ios: failed', 'ios-touch: failed', 'lane: failed', 'tvos: failed', 'install: failed'];
  const skipped = {checks:{}, failures:[]};
  retainSkippedFailures(previous, skipped);
  expect(skipped.failures).toEqual(previous.slice(0, 4));
  const passed = {checks:{'ios-touch':{status:0}}, failures:[]};
  retainSkippedFailures(previous, passed);
  expect(passed.failures).toEqual(['ios: failed', 'lane: failed', 'tvos: failed']);
  const other = {checks:{}, failures:[]};
  retainSkippedFailures(['optional: failed'], other, {}, new Set(['optional']));
  expect(other.failures).toEqual(['optional: failed']);
  // A check that ran a part keeps the failures of the rest, and those it cannot place, until a run covers them.
  const was = {'test: -p exact-linux --lib a::b': ['cargo:exact-linux'], 'test: -p exact-kernel --lib c': ['cargo:exact-kernel'], 'glue: d': ['bun:host/web/d.test.mjs'], 'glue: twice': ['bun:a.test.mjs', 'bun:b.test.mjs']};
  const part = {checks:{test:{status:0, covers:['cargo:exact-kernel']}, glue:{status:1, covers:['bun:a.test.mjs']}}, failures:['glue: e'], owners:{}};
  retainSkippedFailures([...Object.keys(was), 'glue: exit 1 (see log)'], part, was);
  expect(part.failures).toEqual(['glue: e', 'test: -p exact-linux --lib a::b', 'glue: d', 'glue: twice', 'glue: exit 1 (see log)']);
  expect(part.owners['glue: twice']).toEqual(['bun:a.test.mjs', 'bun:b.test.mjs']);
  // Found again, a failure is not kept a second time; a full run keeps none.
  const again = {checks:{test:{status:101, covers:['cargo:exact-kernel']}}, failures:['test: -p exact-linux --lib a::b']};
  retainSkippedFailures(['test: -p exact-linux --lib a::b'], again, was);
  expect(again.failures).toEqual(['test: -p exact-linux --lib a::b']);
  // Keep what a check that stopped early held (a crate did not compile), whatever parts it was given
  const stopped = {checks:{test:{status:101, covers:['cargo:exact-kernel'], complete:false}}, failures:['test: exact-linux (lib test) does not compile']};
  retainSkippedFailures(['test: -p exact-kernel --lib c'], stopped, was);
  expect(stopped.failures).toEqual(['test: exact-linux (lib test) does not compile', 'test: -p exact-kernel --lib c']);
  // Judge a run by its own stops, not one kept from an earlier run: a part it finished drops the earlier failures it
  // no longer finds, in either order
  const stop = 'test: exact-web (lib test) does not compile', owned = {...was, [stop]: ['cargo:exact-web']};
  for (const previous of [[stop, 'test: -p exact-kernel --lib c'], ['test: -p exact-kernel --lib c', stop]]) {
    const later = {checks:{test:{status:0, covers:['cargo:exact-kernel'], complete:true}}, failures:[]};
    retainSkippedFailures(previous, later, owned);
    expect(later.failures).toEqual([stop]);
  }
  const full = {checks:{test:{status:0}}, failures:[], full:true};
  retainSkippedFailures(['test: -p exact-linux --lib a::b', 'ios: failed'], full, was);
  expect(full.failures).toEqual([]);
});

test("async's changed paths name both of a rename's", () => {
  const repo = mkdtempSync(join(tmpdir(), 'exact-async-rename-'));
  try {
    // Turn Git's rename detection on, as it is by default, so a diff that left it on would fold the two paths
    const git = (args) => { const r = spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', '-c', 'diff.renames=true', ...args], { cwd: repo, encoding: 'utf8' }); if (r.status !== 0) throw new Error(r.stderr); return r.stdout; };
    git(['init', '-q']);
    writeFileSync(join(repo, 'fixture.contract'), 'component App\n');
    git(['add', '.']); git(['commit', '-q', '-m', 'a']);
    git(['mv', 'fixture.contract', 'renamed.contract']); git(['commit', '-q', '-m', 'b']);
    expect(changedPaths(git(['rev-parse', 'HEAD']).trim(), null, git).sort()).toEqual(['fixture.contract', 'renamed.contract']);
  } finally { rmSync(repo, { recursive: true, force: true }); }
});

test('async names the suites of each failure its log attributes', () => {
  const glue = ['bun test v1.4.2 (744846f84) 2x PARALLEL', 'host/web/a.test.mjs:', '(fail) a fails [0.44ms]', './scripts/b.test.mjs:', '(fail) group > b fails [1.20ms]', '(fail) a fails [3ms]', ' 3 fail'].join('\n');
  expect(owners('glue', glue, ['glue: a fails', 'glue: group > b fails'])).toEqual({'glue: a fails': ['bun:host/web/a.test.mjs', 'bun:scripts/b.test.mjs'], 'glue: group > b fails': ['bun:scripts/b.test.mjs']});
  expect(owners('test', '', ['test: -p exact-linux --lib a::b', 'test: exact-web (lib test) does not compile'])).toEqual({'test: -p exact-linux --lib a::b': ['cargo:exact-linux'], 'test: exact-web (lib test) does not compile': ['cargo:exact-web']});
  // Name and own a build script that failed, a binary that failed with no failing test (it crashed) and a Bun file
  // whose code threw outside its tests
  const cargo = ['error: failed to run custom build command for `exact-svg v0.1.0 (/x/svg)`', '     Running unittests src/lib.rs (target/debug/deps/a)',
    'error: test failed, to rerun pass `-p exact-render --lib`'].join('\n');
  const named = failures('test', cargo, 101);
  expect(named).toEqual(['test: exact-svg build script failed', 'test: -p exact-render --lib did not finish']);
  expect(owners('test', cargo, named)).toEqual({'test: exact-svg build script failed': ['cargo:exact-svg'], 'test: -p exact-render --lib did not finish': ['cargo:exact-render']});
  const unloaded = ['bun test v1.4.2 (744846f84) 2x PARALLEL', 'host/web/c.test.mjs:', '# Unhandled error between tests', 'error: Cannot find module', ' 1 fail'].join('\n');
  expect(failures('glue', unloaded, 1)).toEqual(['glue: host/web/c.test.mjs: an error between tests']);
  expect(owners('glue', unloaded, failures('glue', unloaded, 1))).toEqual({'glue: host/web/c.test.mjs: an error between tests': ['bun:host/web/c.test.mjs']});
  // conform.mjs's own lines: `FAIL <app> <step>:`, and in a cross-browser step `FAIL <browser> <app> <step>:`.
  const conform = name => failures(name, `FAIL ${name === 'conform' ? '' : 'firefox '}caltrain tap-station: moved\nFAIL engine launch: no browser\n`, 1);
  expect(owners('conform', '', conform('conform'))).toEqual({'conform: caltrain tap-station': ['conform:caltrain']});
  expect(owners('conform-firefox', '', conform('conform-firefox'))).toEqual({'conform-firefox: firefox caltrain tap-station': ['conform:caltrain']});
});

test('async runs a check only for the suites selected, its Rust tests built as a full run builds them', () => {
  const none = asyncLane.nothing();
  expect(asyncLane.checks('new', 'old', none).map(([name]) => name)).toEqual(['build', 'clippy', 'fmt', 'caps', 'boot', 'lean', 'vm-extract', 'difftest', 'difftest-js', 'metrics']);
  const selection = {...none, cargo:['exact-linux'], conform:['caltrain'], ios:true, glue:['./host/web/a.test.mjs'], lane:['a_lane_test']};
  const some = asyncLane.checks('new', 'old', selection);
  const step = name => some.find(([n]) => n === name);
  // (The lane's own runner, which a commit it checks may predate.)
  const runner = resolve(import.meta.dir, 'affected.mjs');
  expect(step('test')[2]).toEqual(expect.arrayContaining(['bun', runner, '--cargo-tests', 'exact-linux']));
  expect(step('lane').slice(1)).toEqual(['bun', [runner, '--cargo-tests', 'exact-linux', '--', '--ignored', 'a_lane_test']]);
  for (const name of ['conform', 'conform-firefox', 'conform-webkit']) expect(step(name)[2].slice(0, 3)).toEqual(['host/web-js/conform.mjs', 'caltrain', '--synthetic']);
  expect(step('glue')[2].at(-1)).toContain('./host/web/a.test.mjs');
  for (const name of ['prepare', 'ios']) expect(step(name)).toBeDefined();
  for (const name of ['ios-touch', 'tvos', 'watchos', 'harmony', 'game', 'web-ax-test']) expect(step(name)).toBeUndefined();
  expect(asyncLane.covers('test', selection)).toEqual(['cargo:exact-linux']);
  expect(asyncLane.covers('conform-webkit', selection)).toEqual(['conform:caltrain']);
  expect(asyncLane.covers('ios', selection)).toBe(null);
  // The iOS classes its test files reach run alone, and a failure of one is that class's
  const classes = {...selection, iosOnly:['ATests', 'BTests']};
  expect(asyncLane.checks('new', 'old', classes).find(([n]) => n === 'ios')[2]).toEqual(['host/apple/build.mjs', '--test', '--ios', '--only', 'ATests,BTests']);
  expect(asyncLane.covers('ios', classes)).toEqual(['xctest:ios:ATests', 'xctest:ios:BTests']);
  expect(owners('ios', '', ['ios: ExactKitTests.CTests testC failed', 'ios: exit 65 (see log)'])).toEqual({'ios: ExactKitTests.CTests testC failed': ['xctest:ios:CTests']});
  const ranTwo = {checks:{ios:{status:0, covers:asyncLane.covers('ios', classes)}}, failures:[]};
  retainSkippedFailures(['ios: ExactKitTests.ATests testA failed', 'ios: ExactKitTests.CTests testC failed', 'ios: exit 65 (see log)'], ranTwo, {'ios: ExactKitTests.ATests testA failed': ['xctest:ios:ATests'], 'ios: ExactKitTests.CTests testC failed': ['xctest:ios:CTests']});
  expect(ranTwo.failures).toEqual(['ios: ExactKitTests.CTests testC failed', 'ios: exit 65 (see log)']);
  const full = asyncLane.checks('new', 'old', {lane:[], glue:['./x.test.mjs']});
  expect(full.find(([n]) => n === 'test')[2]).toEqual(expect.arrayContaining(['cargo', 'test', '--workspace']));
  for (const name of ['ios', 'ios-touch', 'tvos', 'watchos', 'harmony', 'game', 'web-ax-test', 'conform']) expect(full.some(([n]) => n === name)).toBe(true);
  // What a selection can leave out is what a full run has and an empty selection has not.
  expect([...asyncLane.gated()].sort()).toEqual(['conform', 'conform-firefox', 'conform-webkit', 'game', 'game-assets-scaffold', 'game-lock', 'game-pins', 'game-recipes', 'game-scaffold', 'game-vocab', 'glue', 'harmony', 'harmony-check', 'ios', 'ios-touch', 'lane', 'prepare', 'test', 'tvos', 'watchos', 'web-ax-test', 'web-build-test']);
});

test('async compiles tvOS per commit with no simulator, and tier 2 installs it', () => {
  const steps = asyncLane.checks('new', 'old', {lane: [], glue: []});
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

test('async compiles watchOS per commit with no simulator, and tier 2 does not again', () => {
  const steps = asyncLane.checks('new', 'old', {lane: [], glue: []});
  expect(steps.filter(([name]) => name === 'watchos')).toEqual([['watchos', 'bun', ['host/apple/build.mjs', '--watchos', '--no-install']]]);
  expect(asyncLane.tierTwoChecks().some(([name]) => name === 'watchos')).toBe(false);
  // Skipped on a documentation-only range, the step keeps its last failure until it runs again.
  const skipped = {checks: {}, failures: []};
  retainSkippedFailures(["watchos: ExactCore/HostPhases.swift: 'CACurrentMediaTime()' is unavailable in watchOS"], skipped);
  expect(skipped.failures).toEqual(["watchos: ExactCore/HostPhases.swift: 'CACurrentMediaTime()' is unavailable in watchOS"]);
});

test('tier 2 drops failures for the watchOS check moved to the first lane', () => {
  const script = `
    process.argv.push('--tier', '2');
    const lane = await import('./async.mjs');
    const result = {checks: Object.fromEntries(lane.checks('new', 'old').map(([name]) => [name, {status: 0}])), failures: []};
    lane.retainSkippedFailures(['watchos: previous compile failed'], result);
    console.log(JSON.stringify(result.failures));
  `;
  const ran = spawnSync(process.execPath, ['--eval', script], {cwd: import.meta.dir, encoding: 'utf8'});
  expect(ran.status).toBe(0);
  expect(JSON.parse(ran.stdout)).toEqual([]);
});

test('tier 2 checks platform host and app crates for their actual target', () => {
  const checks = asyncLane.tierTwoChecks();
  for (const name of ['linux', 'windows', 'windows-arm64']) expect(checks.find(([step]) => step === name)?.[2]).toContain('CRATE_CC_NO_DEFAULTS=1');
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('x86_64-unknown-linux-gnu');
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('--keep-going');
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('CC_x86_64_unknown_linux_gnu=zig cc -target x86_64-linux-gnu');
  // Hermes's JSI bindings (ibex2, exact-js) are C++: a Mac has no x86_64-linux-gnu-g++.
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('CXX_x86_64_unknown_linux_gnu=zig c++ -target x86_64-linux-gnu');
  // Every Linux app crate, by Cargo's package glob, which matches workspace members only:
  // each `apps/<app>/linux` is a listed member named `<app>-linux`.
  expect(checks.find(([name]) => name === 'linux')?.[2]).toContain('*-linux');
  const root = resolve(import.meta.dir, '..');
  const [, list = ''] = /^members = \[([^\]]*)\]/m.exec(readFileSync(join(root, 'Cargo.toml'), 'utf8')) ?? [];
  const members = new Set([...list.matchAll(/"([^"]+)"/g)].map(([, path]) => path));
  const apps = [...new Bun.Glob('apps/*/linux/Cargo.toml').scanSync({ cwd: root })];
  expect(apps.length).toBeGreaterThan(1);
  for (const manifest of apps) {
    const dir = dirname(manifest);
    expect(members.has(dir) ? dir : `${dir} (not a workspace member)`).toBe(dir);
    const [, name] = /^name = "([^"]+)"$/m.exec(readFileSync(join(root, manifest), 'utf8')) ?? [];
    expect(`${manifest}: ${name}`).toMatch(/: [\w-]+-linux$/);
  }
  expect(checks.find(([name]) => name === 'windows')?.[2]).toContain('x86_64-pc-windows-gnu');
  expect(checks.find(([name]) => name === 'windows')?.[2]).toContain('--keep-going');
  expect(checks.find(([name]) => name === 'windows')?.[2]).toContain('windows-desk');
  expect(checks.find(([name]) => name === 'windows')?.[2]).toContain('CC_x86_64_pc_windows_gnu=zig cc -target x86_64-windows-gnu');
  expect(checks.find(([name]) => name === 'windows-arm64')?.[2]).toEqual(expect.arrayContaining(['aarch64-pc-windows-gnullvm', '--keep-going', 'exact-windows', 'windows-desk', 'CC_aarch64_pc_windows_gnullvm=zig cc -target aarch64-windows-gnu']));
});

test('async game step checks the separate workspace with its lockfile', () => {
  const steps = asyncLane.checks('new', 'old', {lane: [], glue: []});
  expect(steps.find(([name]) => name === 'game')).toEqual(['game', 'cargo', ['test', '--manifest-path', 'game/Cargo.toml', '--workspace', '--locked', '--lib', '--bins', '--tests', '--no-fail-fast']]);
});

test('async glue runs the capture fixtures and the web JS target\'s tests, less those it leaves out', () => {
  const glue = asyncLane.glueTests(resolve(import.meta.dir, '..'));
  expect(glue).toContain('./scripts/app-workspace.test.mjs');
  expect(glue).toContain('./host/web-js/conformance/net.test.mjs');
  expect(glue).toContain('./host/web-js/live-clock.test.js');
  expect(glue).not.toContain('./host/web-js/bundle-size.test.mjs');
  expect(glue).not.toContain('./host/web-js/document-agent.test.mjs');
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
