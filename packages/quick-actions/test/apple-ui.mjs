// LLP 1133.023 stage 2's simulator drives: ShelfReachUITests.swift, built as
// an XCUITest bundle with swiftc and run through an .xctestrun on a simulator
// that already has Shelf installed (`bun host/apple/build.mjs --ios shelf-apple
// --sim <udid> --run`), with no Xcode project:
//   bun packages/quick-actions/test/apple-ui.mjs <udid> <out dir> [<test name>…]
// The runner is Xcode's XCTRunner.app with the test bundle in its PlugIns and
// the platform's testing frameworks in its Frameworks, as Xcode lays it out.
// Each step's screenshot and accessibility hierarchy land in <out dir>.
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const [udid, outArg, ...only] = process.argv.slice(2);
if (!udid || !outArg) {
  console.error('usage: bun packages/quick-actions/test/apple-ui.mjs <simulator udid> <out dir> [<test name>…]');
  process.exit(2);
}
const out = resolve(outArg), name = 'ShelfReachUITests', source = resolve(import.meta.dirname, `${name}.swift`);
const run = (cmd, args) => {
  const r = spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 64 << 20 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.slice(0, 3).join(' ')}: ${r.stderr || r.stdout}`);
  return r.stdout;
};
const platform = `${run('xcode-select', ['-p']).trim()}/Platforms/iPhoneSimulator.platform/Developer`;
const sdk = run('xcrun', ['--sdk', 'iphonesimulator', '--show-sdk-path']).trim();
const work = resolve(out, 'build'), runner = resolve(work, `${name}-Runner.app`), bundle = resolve(runner, 'PlugIns', `${name}.xctest`);
rmSync(work, { recursive: true, force: true });
mkdirSync(bundle, { recursive: true });
cpSync(`${platform}/Library/Xcode/Agents/XCTRunner.app`, runner, { recursive: true });
run('plutil', ['-replace', 'CFBundleIdentifier', '-string', 'dev.exact.reach.uitests.xctrunner', resolve(runner, 'Info.plist')]);
const frameworks = resolve(runner, 'Frameworks');
mkdirSync(frameworks, { recursive: true });
for (const f of ['XCTest.framework', 'XCUIAutomation.framework', 'Testing.framework']) cpSync(`${platform}/Library/Frameworks/${f}`, resolve(frameworks, f), { recursive: true });
for (const f of ['XCTestCore.framework', 'XCUnit.framework', 'XCTAutomationSupport.framework', 'XCTestSupport.framework']) cpSync(`${platform}/Library/PrivateFrameworks/${f}`, resolve(frameworks, f), { recursive: true });
for (const f of ['libXCTestSwiftSupport.dylib', 'lib_TestingInterop.dylib']) cpSync(`${platform}/usr/lib/${f}`, resolve(frameworks, f));
run('xcrun', ['--sdk', 'iphonesimulator', 'swiftc', '-parse-as-library', '-module-name', name, '-target', 'arm64-apple-ios17.0-simulator', '-sdk', sdk,
  '-F', `${platform}/Library/Frameworks`, '-I', `${platform}/usr/lib`, '-L', `${platform}/usr/lib`, '-framework', 'XCTest', '-framework', 'XCUIAutomation',
  '-Xlinker', '-bundle', '-Xlinker', '-rpath', '-Xlinker', '@loader_path/../../Frameworks', source, '-o', resolve(bundle, name)]);
const plist = (value) => typeof value === 'boolean' ? (value ? '<true/>' : '<false/>') : typeof value === 'number' ? `<integer>${value}</integer>`
  : Array.isArray(value) ? `<array>${value.map(plist).join('')}</array>`
  : typeof value === 'object' ? `<dict>${Object.entries(value).map(([k, v]) => `<key>${k}</key>${plist(v)}`).join('')}</dict>`
  : `<string>${String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;')}</string>`;
const file = (path, value) => writeFileSync(path, `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0">${plist(value)}</plist>\n`);
file(resolve(bundle, 'Info.plist'), { CFBundleIdentifier: `dev.exact.reach.${name}`, CFBundleExecutable: name, CFBundleName: name, CFBundlePackageType: 'BNDL',
  CFBundleVersion: '1', CFBundleShortVersionString: '1.0', CFBundleSupportedPlatforms: ['iPhoneSimulator'], MinimumOSVersion: '17.0' });
for (const f of readdirSync(frameworks)) run('codesign', ['--force', '--sign', '-', '--timestamp=none', resolve(frameworks, f)]);
for (const signed of [bundle, runner]) run('codesign', ['--force', '--sign', '-', '--timestamp=none', signed]);
// The app under test is Shelf as installed; the runner names it so XCTest may launch and terminate it.
const shelf = run('xcrun', ['simctl', 'get_app_container', udid, 'com.exact.shelf', 'app']).trim();
const env = { REACH_OUT: out };
const xctestrun = resolve(work, `${name}.xctestrun`);
file(xctestrun, {
  [name]: { TestBundlePath: `__TESTHOST__/PlugIns/${name}.xctest`, TestHostPath: runner, TestHostBundleIdentifier: 'dev.exact.reach.uitests.xctrunner',
    IsUITestBundle: true, IsXCTRunnerHostedTestBundle: true, UITargetAppPath: shelf, UITargetAppBundleIdentifier: 'com.exact.shelf',
    TestingEnvironmentVariables: env, EnvironmentVariables: env, ...(only.length ? { OnlyTestIdentifiers: only.map(t => `${name}/${t}`) } : {}) },
  __xctestrun_metadata__: { FormatVersion: 1 },
});
const r = spawnSync('xcodebuild', ['test-without-building', '-xctestrun', xctestrun, '-destination', `platform=iOS Simulator,id=${udid}`, '-resultBundlePath', resolve(out, `results-${Date.now()}.xcresult`)],
  { encoding: 'utf8', maxBuffer: 64 << 20 });
writeFileSync(resolve(out, 'xcodebuild.log'), r.stdout + r.stderr);
console.log(r.stdout.split('\n').filter(l => /Test Case|error:|\*\* TEST/.test(l)).join('\n'));
process.exit(r.status ?? 1);
