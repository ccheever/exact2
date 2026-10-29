import { test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { evictArms, keepRoster, keptRoster, snapshotSources } from './arms.mjs';
import { packageModules } from './compose.mjs';
import { useXcode } from './devices.mjs';

test('an arm keeps its three last used copies, another arm whose name ends in it keeps its own, and a day-old half-written copy goes', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-arms-'));
  try {
    const name = 'libexact_modules.dylib';
    const at = (file, secondsAgo) => {
      writeFileSync(join(dir, file), '');
      const t = new Date(Date.now() - secondsAgo * 1000);
      utimesSync(join(dir, file), t, t);
    };
    const keys = ['0000000000000001', '0000000000000002', '0000000000000003', '0000000000000004'];
    keys.forEach((key, i) => at(`${key}-${name}`, i * 10));
    at(`0000000000000009-probe-${name}`, 1000);
    at(`0000000000000004-${name}.roster`, 0);
    at(`0000000000000003-${name}.roster`, 0);
    at(`0000000000000001-${name}.42.tmp`, 2 * 86_400);
    at(`0000000000000002-${name}.43.tmp`, 60);
    evictArms(dir, name);
    assert.deepEqual(readdirSync(dir).sort(), [
      `0000000000000001-${name}`, `0000000000000002-${name}`, `0000000000000002-${name}.43.tmp`,
      `0000000000000003-${name}`, `0000000000000003-${name}.roster`, `0000000000000009-probe-${name}`,
    ]);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test("a build compiles its sources as they were when it began, keyed and reported by the author's paths", () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-arms-'));
  try {
    const authored = join(dir, 'Pad.swift');
    writeFileSync(authored, 'let a = 1\n');
    const snapshot = snapshotSources([authored], join(dir, 'build'));
    writeFileSync(authored, 'let a = 2\n'); // the author's edit while the build runs
    assert.equal(readFileSync(snapshot.copies[0], 'utf8'), 'let a = 1\n');
    assert.equal(snapshot.key(snapshot.copies[0]), authored);
    assert.equal(snapshot.key('-O'), '-O');
    const real = join(realpathSync(join(dir, 'build')), 'Pad.swift');
    assert.equal(snapshot.authored(`${real}:1:5: error: x\n${snapshot.copies[0]}:2:1: note: y`), `${authored}:1:5: error: x\n${authored}:2:1: note: y`);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test("a probe's roster is kept beside its copy, read back by the next build, and none is kept for a copy that has none", () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-arms-'));
  try {
    const copy = join(dir, '0000000000000001-probe-roster-simulator');
    writeFileSync(copy, '');
    assert.equal(keptRoster(copy), undefined);
    assert.equal(keptRoster(undefined), undefined); // an arm placed with no copy (a macOS build's own artifact)
    keepRoster(copy, '{"map-view":{"firstFrame":true}}');
    assert.equal(keptRoster(copy), '{"map-view":{"firstFrame":true}}');
    assert.equal(keptRoster(join(dir, '0000000000000002-probe-roster-simulator')), undefined); // another key's
    keepRoster(undefined, '{}');
    assert.deepEqual(readdirSync(dir).sort(), ['0000000000000001-probe-roster-simulator', '0000000000000001-probe-roster-simulator.roster']);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test.skipIf(process.platform !== 'darwin')('device packages and roster probes compile and link for their own targets', () => {
  useXcode();
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-package-target-'));
  try {
    const seam = resolve(dir, 'ExactPackage.swift'), source = resolve(dir, 'RosterPackage.swift'), probe = resolve(dir, 'Probe.swift');
    writeFileSync(seam, '#if os(macOS)\nimport AppKit\npublic typealias PlatformView = NSView\n#else\nimport UIKit\npublic typealias PlatformView = UIView\n#endif\npublic func seamValue() -> String { "artifact" }\n');
    writeFileSync(source, `import ExactPackage
public func packageRoster() -> String {
    _ = PlatformView.self
    #if targetEnvironment(macCatalyst)
    return seamValue() + "-catalyst"
    #elseif targetEnvironment(simulator)
    return seamValue() + "-simulator"
    #elseif os(iOS)
    return seamValue() + "-device"
    #else
    return seamValue() + "-macos"
    #endif
}
`);
    writeFileSync(source, readFileSync(source, 'utf8') + `
public func developmentPackage() -> Bool {
    #if EXACT_DEVELOPMENT
    return true
    #else
    return false
    #endif
}
`);
    writeFileSync(probe, 'import RosterPackage\n@main enum Probe { static func main() { print(packageRoster()) } }\n');
    const packages = [{ module: 'RosterPackage', extension: 'recording', sources: [source] }];
    const cache = resolve(dir, 'cache'), directory = resolve(dir, 'packages');
    const forTarget = packageModules(packages, [[source]], { seam, directory, cache });
    const arch = process.arch === 'arm64' ? 'arm64' : 'x86_64';
    const sdkPath = (sdk) => {
      const r = spawnSync('xcrun', ['--sdk', sdk, '--show-sdk-path'], { encoding: 'utf8' });
      assert.equal(r.status, 0, r.stderr);
      return r.stdout.trim();
    };
    const catalyst = resolve(sdkPath('macosx'), 'System/iOSSupport');
    const targets = [
      ['iphoneos', ['-target', 'arm64-apple-ios17.0', '-sdk', sdkPath('iphoneos')], 'IOS', 'device'],
      ['macosx', ['-target', `${arch}-apple-ios17.0-macabi`, '-Fsystem', `${catalyst}/System/Library/Frameworks`, '-I', `${catalyst}/usr/lib/swift`, '-L', `${catalyst}/usr/lib/swift`], 'MACCATALYST', 'catalyst'],
      ['iphonesimulator', ['-target', `${arch}-apple-ios17.0-simulator`, '-sdk', sdkPath('iphonesimulator')], 'IOSSIMULATOR', 'simulator'],
      ['macosx', ['-target', `${arch}-apple-macos14.0`], 'MACOS', 'macos'],
    ];
    const compiled = [];
    for (const [sdk, target, platform, name] of targets) {
      const modules = forTarget(sdk, target), out = resolve(dir, name);
      assert.equal(forTarget(sdk, [...target]), modules, 'an app and probe sharing a target reuse its package compilation');
      modules.place();
      const linked = spawnSync('xcrun', ['--sdk', sdk, 'swiftc', '-parse-as-library', probe, ...modules.link, ...target, '-o', out], { encoding: 'utf8' });
      assert.equal(linked.status, 0, `${name}: ${linked.stderr}`);
      const load = spawnSync('xcrun', ['vtool', '-show-build', out], { encoding: 'utf8' });
      assert.equal(load.status, 0, load.stderr);
      assert.match(load.stdout, new RegExp(`platform ${platform}\\s`));
      if (name === 'catalyst' || name === 'macos') {
        const ran = spawnSync(out, [], { encoding: 'utf8' });
        assert.equal(ran.status, 0, ran.stderr);
        assert.equal(ran.stdout.trim(), `artifact-${name}`);
      }
      compiled.push({ ...modules, objects: modules.link.slice(2).map(path => readFileSync(path)) });
    }
    assert.equal(new Set(compiled.map(m => m.link[1])).size, targets.length);
    assert.equal(new Set(compiled.map(m => m.stamp)).size, targets.length);
    compiled.forEach(m => assert.deepEqual(m.link.slice(2).map(path => readFileSync(path)), m.objects, 'later probes keep earlier package objects intact'));
    const another = packageModules(packages, [[source]], { seam, directory: resolve(dir, 'another-build'), cache });
    targets.forEach(([sdk, target], i) => assert.equal(another(sdk, target).stamp, compiled[i].stamp, 'cache identity omits invocation-private outputs'));
    const [sdk, target] = targets.at(-1), production = compiled.at(-1);
    const development = packageModules(packages, [[source]], { seam, directory, cache, development: true })(sdk, target);
    assert.notEqual(development.stamp, production.stamp);
    assert.notEqual(development.link[1], production.link[1]);
    development.place();
    writeFileSync(probe, 'import RosterPackage\n@main enum Probe { static func main() { print(developmentPackage()) } }\n');
    for (const [name, modules, expected] of [['development', development, 'true'], ['production', production, 'false']]) {
      const out = resolve(dir, name);
      const linked = spawnSync('xcrun', ['--sdk', sdk, 'swiftc', '-parse-as-library', probe, ...modules.link, ...target, '-o', out], { encoding: 'utf8' });
      assert.equal(linked.status, 0, linked.stderr);
      const ran = spawnSync(out, [], { encoding: 'utf8' });
      assert.equal(ran.status, 0, ran.stderr);
      assert.equal(ran.stdout.trim(), expected);
    }
    assert.deepEqual(production.link.slice(2).map(path => readFileSync(path)), production.objects);

  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 60_000);
