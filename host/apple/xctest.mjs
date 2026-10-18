// How `build.mjs --test` runs the Swift host tests once they are built:
// on this Mac, a test class to an `xctest` process; on a simulator, the
// `.xctestrun` xcodebuild wrote, without the Thread Performance Checker.
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { resolve } from 'node:path';

const read = (cmd, args, opts = {}) => spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, ...opts });

/** The fixtures tests boot (scripts/fixtures), compiled at this commit with `run` and named in `env` as `<prefix>_<KEY>_PLAN`:
 *  a plan carries the kernel schema's digest, so one compiled once and kept in a test goes stale. The snapshot driver's
 *  entries (LLP 1126.000 WP4) likewise: Caltrain's and the open-question fixture's, each an entry plan, compiled from a
 *  copy whose manifest carries the test app's identity (`appId`), as the native fixture is: a plan boots in no other app. */
export function compileFixtures(root, run, env, dir, prefix, appId) {
  const contract = (args) => run('cargo', ['run', '-q', '-p', 'contract', '--bin', 'contract', '--manifest-path', resolve(root, 'Cargo.toml'), '--', 'build', ...args]);
  for (const [name, key] of [['headless', 'HEADLESS'], ['headless-markdown', 'HEADLESS_MARKDOWN'], ['headless-hidden-markdown', 'HEADLESS_HIDDEN_MARKDOWN'], ['native-navigation-scroll', 'NATIVE_SCROLL'], ['intersect', 'INTERSECT'], ['native-buttons', 'BUTTONS'], ['keyboard-form', 'KEYBOARD'], ['tab-edge', 'TAB_EDGE'], ['repaint', 'REPAINT'], ['still-title', 'STILL_TITLE'], ['navigation-bars', 'NAVIGATION_BARS'], ['large-title', 'LARGE_TITLE'], ['bar-band', 'BAR_BAND'], ['pad-bar', 'PAD_BAR'], ['pushed-title', 'PUSHED_TITLE'], ['inline-frame', 'INLINE_FRAME'], ['fieldless', 'FIELDLESS'], ['web-documents', 'WEB_DOCUMENTS'], ['tab-owner', 'TAB_OWNER'], ['grouped-list-reveal', 'GROUPED_REVEAL'], ['due-now', 'DUE_NOW'], ['caret-reveal', 'CARET_REVEAL'], ['cover-scroll', 'COVER_SCROLL']]) {
    contract([resolve(root, `scripts/fixtures/${name}.contract`), '-o', env[`${prefix}_${key}_PLAN`] = resolve(dir, `${name}.plan`)]);
  }
  // HeldStateTests' own (a card that presses over a flat leaf), beside them.
  contract([resolve(root, 'host/apple/tests/fixtures/held-state.contract'), '-o', env[`${prefix}_HELD_STATE_PLAN`] = resolve(dir, 'held-state.plan')]);
  const question = 'host/apple/tests/ExactKitTests/SnapshotFixtures/open-question';
  const refusing = resolve(dir, 'entries/refusing');
  mkdirSync(refusing, { recursive: true });
  writeFileSync(resolve(refusing, 'app.contract'), 'shape Props\n  title: string\ncomponent App\n  view\n    text "App"\ncomponent Card\n  props\n    title: string\n  view\n    text title\n');
  writeFileSync(resolve(refusing, 'app.json'), JSON.stringify({ name: 'Refusing refresh', app: { id: appId, name: 'Refusing refresh' }, widgets: [{ kind: 'refusing', families: ['systemSmall'], component: 'Card', props: 'Props', refresh: 'nextTrains', preview: { title: 'Preview' } }] }));
  // Caltrain's nextTrains answers a different Props shape: the refresh must refuse.
  contract([resolve(refusing, 'app.contract'), '--entry', 'refusing', '-o', env[`${prefix}_SNAPSHOT_REFUSING_PLAN`] = resolve(dir, 'refusing.plan')]);
  // An activity whose Lock Screen placement has a background and a link (ActivityDriverTests: the tint, the URL).
  const tinted = resolve(dir, 'entries/tinted');
  mkdirSync(tinted, { recursive: true });
  writeFileSync(resolve(tinted, 'app.contract'), 'shape TintedState\n  title: string\ncomponent App\n  view\n    text "App"\ncomponent Tinted\n  props\n    title: string\n  view\n    view slot="lockScreen" background-color="#1e3a5f" padding=8\n      link href="/alerts" display="block"\n        text title color="#ffffff"\n    view slot="compactLeading"\n      text "T"\n    view slot="compactTrailing"\n      text title text-align="center"\n    view slot="minimal"\n      text "T"\n    view slot="expanded"\n      view slot="leading"\n        text title\n      view slot="trailing"\n        text title width=120\n      view slot="center"\n        link href="/alerts" display="block"\n          text title\n      view slot="bottom"\n        text title\n');
  writeFileSync(resolve(tinted, 'app.json'), JSON.stringify({ name: 'Tinted activity', app: { id: appId, name: 'Tinted activity', origin: 'https://tinted.exact.invalid' }, live_activities: [{ kind: 'tinted', component: 'Tinted', state: 'TintedState', preview: { title: 'Preview title' } }] }));
  contract([resolve(tinted, 'app.contract'), '--entry', 'tinted', '-o', env[`${prefix}_SNAPSHOT_TINTED_PLAN`] = resolve(dir, 'tinted.plan')]);
  const copies = new Map();
  for (const [app, kind, key] of [['apps/caltrain', 'next-trains', 'SNAPSHOT_TRAINS'], ['apps/caltrain', 'departure', 'SNAPSHOT_DEPARTURE'], ['apps/caltrain', 'service-alert', 'SNAPSHOT_SERVICE_ALERT'], [question, 'question-small', 'SNAPSHOT_QUESTION_SMALL'],
    [question, 'question-medium', 'SNAPSHOT_QUESTION_MEDIUM'], [question, 'question-rectangular', 'SNAPSHOT_QUESTION_RECTANGULAR'], [question, 'question-adaptive', 'SNAPSHOT_QUESTION_ADAPTIVE'], [question, 'gradient-card', 'SNAPSHOT_GRADIENT']]) {
    let copy = copies.get(app);
    if (!copy) {
      copy = resolve(dir, 'entries', app.split('/').pop());
      mkdirSync(copy, { recursive: true });
      for (const file of readdirSync(resolve(root, app)).filter(f => f.endsWith('.contract'))) cpSync(resolve(root, app, file), resolve(copy, file));
      const manifest = JSON.parse(readFileSync(resolve(root, app, 'app.json'), 'utf8'));
      writeFileSync(resolve(copy, 'app.json'), JSON.stringify({ ...manifest, $schema: undefined, id: appId, app: { ...manifest.app, id: appId } }));
      copies.set(app, copy);
    }
    contract([resolve(copy, 'app.contract'), '--entry', kind, '-o', env[`${prefix}_${key}_PLAN`] = resolve(dir, `${kind}.plan`)]);
  }
}

/** The Swift host tests on this Mac, built under `scratch`: each test class
 *  in an `xctest` process of its own, as many at once as there are cores, so
 *  a crash ends its class and not the run. The classes whose files use what
 *  every process on the Mac shares (the general pasteboard, the active app,
 *  the mouse, a capture of the screen) share one process beside them, one
 *  class after another. A window's key status and first responder belong
 *  to its own process. The Canvas 2D GPU module (`canvasGpu`) is loaded only
 *  in its own tests' process: loaded, every canvas draws on the GPU. */
export async function macTests(pkg, scratch, env, canvasGpu) {
  const listed = read('swift', ['test', '--skip-build', '--list-tests', '--scratch-path', scratch], { cwd: pkg, env });
  if (listed.status !== 0) throw new Error(`swift test --list-tests failed: ${listed.stderr}`);
  const classes = [...new Set(listed.stdout.match(/^ExactKitTests\.\w+(?=\/)/gm) ?? [])];
  const macWide = /NSPasteboard\.general|\bactivate\(|NSEvent\.mouseLocation|CGWindowListCreateImage/;
  const shared = new Set();
  for (const file of readdirSync(resolve(pkg, 'tests/ExactKitTests')).filter(f => f.endsWith('.swift'))) {
    const source = readFileSync(resolve(pkg, 'tests/ExactKitTests', file), 'utf8');
    if (macWide.test(source)) for (const [, name] of source.matchAll(/class (\w+)\s*:\s*XCTestCase/g)) shared.add(`ExactKitTests.${name}`);
  }
  const gpuClass = 'ExactKitTests.Canvas2DGpuStressTests';
  shared.delete(gpuClass);
  const groups = [classes.filter(c => shared.has(c)), ...classes.filter(c => !shared.has(c)).map(c => [c])].filter(g => g.length);
  const bundle = resolve(read('swift', ['build', '--show-bin-path', '--scratch-path', scratch], { cwd: pkg, env }).stdout.trim(), 'ExactKitTests.xctest');
  const failed = [];
  const one = (group) => new Promise((done) => {
    const gpu = group.includes(gpuClass) ? { EXACT_CANVAS_GPU_DYLIB: canvasGpu } : {};
    const child = spawn('xcrun', ['xctest', '-XCTest', group.join(','), bundle], { cwd: pkg, env: { ...env, ...gpu }, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '';
    child.stdout.on('data', (d) => { out += d; });
    child.stderr.on('data', (d) => { out += d; });
    child.on('error', (e) => { out += `${e.message}\n`; });
    child.on('close', (status, signal) => {
      process.stdout.write(out);
      if (status !== 0) failed.push(`${group.join(', ')} (xctest ${signal ?? `exited ${status}`})`);
      done();
    });
  });
  // One queue, several takers: `next` is read and advanced with no await between.
  let next = 0;
  const taker = async () => { while (next < groups.length) await one(groups[next++]); };
  await Promise.all(Array.from({ length: Math.min(groups.length, availableParallelism()) }, taker));
  console.log(`host/apple: ${classes.length} test classes in ${groups.length} processes, ${failed.length} failed`);
  if (failed.length) throw new Error(`swift tests failed:\n  ${failed.join('\n  ')}`);
}

/** The iOS tests `build-for-testing` wrote in `products`, run on the simulator `udid` in three
 *  runs that always all run; throws the first that failed. Most run bare in the xctest agent.
 *  Those that need what only an app gives (a window scene to present in, UIKit's accessibility
 *  runtime; `hostedTests`) run hosted in an app with one scene (`hostSource`, built here) with the
 *  accessibility runtime on for that run alone. The Canvas 2D GPU module's own run apart with it
 *  loaded (`canvasGpu`): loaded, every canvas draws on the GPU, and the others read the Core
 *  Graphics path a runner without it draws. `runApple` and `run` are build.mjs's: they throw on
 *  failure, and `runApple` refuses a mixed deployment target. */
export function iosTestRuns({ pkg, products, destination, udid, env, canvasGpu, hostSource, triple, runApple, run }) {
  const bare = withoutPerformanceChecker(products), host = resolve(products, 'ExactTestHost.app');
  rmSync(host, { recursive: true, force: true });
  mkdirSync(host, { recursive: true });
  writeFileSync(resolve(host, 'Info.plist.json'), JSON.stringify({ CFBundleIdentifier: 'com.exact.testhost', CFBundleExecutable: 'ExactTestHost', CFBundleName: 'ExactTestHost',
    CFBundlePackageType: 'APPL', CFBundleVersion: '1', CFBundleShortVersionString: '1.0', CFBundleSupportedPlatforms: ['iPhoneSimulator'], MinimumOSVersion: '17.0',
    LSRequiresIPhoneOS: true, UIDeviceFamily: [1, 2], UILaunchScreen: {}, UIApplicationSceneManifest: { UIApplicationSupportsMultipleScenes: false } }));
  run('plutil', ['-convert', 'xml1', '-o', resolve(host, 'Info.plist'), resolve(host, 'Info.plist.json')]);
  rmSync(resolve(host, 'Info.plist.json'));
  runApple('xcrun', ['--sdk', 'iphonesimulator', 'swiftc', '-parse-as-library', '-target', triple, hostSource, '-o', resolve(host, 'ExactTestHost')]);
  const hosted = hostedIn(products, bare, 'ExactTestHost.app');
  run('codesign', ['-s', '-', '--force', '--deep', host], { stdio: 'ignore' });
  const hostedOnly = hostedTests(resolve(pkg, 'tests/ExactKitTests')), gpuTests = 'ExactKitTests/Canvas2DGpuStressTests';
  const testRun = (xctestrun, selection, runEnv) => {
    try {
      runApple('xcodebuild', ['test-without-building', '-xctestrun', xctestrun, '-destination', destination, '-collect-test-diagnostics', 'never',
        '-parallel-testing-enabled', 'YES', '-parallel-testing-worker-count', '2', ...selection], { cwd: pkg, stdio: 'inherit', env: runEnv });
      return null;
    } catch (e) { return e; }
  };
  const bareRun = testRun(bare, [gpuTests, ...hostedOnly].map(t => `-skip-testing:${t}`), env);
  // UIKit loads its accessibility code only for an assistive technology or automation
  // (AgentAccessibility.axRuntimeLoaded): on for the hosted run, then as it was.
  read('xcrun', ['simctl', 'bootstatus', udid, '-b']);
  const ax = ['com.apple.Accessibility', 'ApplicationAccessibilityEnabled'], before = read('xcrun', ['simctl', 'spawn', udid, 'defaults', 'read', ...ax]);
  let hostedRun = null;
  try {
    run('xcrun', ['simctl', 'spawn', udid, 'defaults', 'write', ...ax, '-bool', 'true']);
    hostedRun = hostedOnly.length ? testRun(hosted, hostedOnly.map(t => `-only-testing:${t}`), env) : null;
  } finally {
    read('xcrun', ['simctl', 'spawn', udid, 'defaults', ...(before.status === 0 ? ['write', ...ax, '-bool', before.stdout.trim() === '1' ? 'true' : 'false'] : ['delete', ...ax])]);
  }
  const gpuRun = testRun(bare, [`-only-testing:${gpuTests}`], { ...env, TEST_RUNNER_EXACT_CANVAS_GPU_DYLIB: canvasGpu });
  const failed = bareRun ?? hostedRun ?? gpuRun;
  if (failed) throw failed;
}

/** The newest `.xctestrun` `build-for-testing` wrote in `products`, as
 *  `exact-tests.xctestrun` beside it without the Thread Performance Checker:
 *  the scheme Xcode makes for the package inserts it (libRPAC), and it
 *  writes a symbolicated backtrace at every wait on a thread of lower
 *  priority, which is most of the button tests' time, and fails no test.
 *  The Main Thread Checker stays. Returns the new file's path. */
export function withoutPerformanceChecker(products) {
  const built = readdirSync(products).filter(f => f.endsWith('.xctestrun') && f !== 'exact-tests.xctestrun')
    .map(f => resolve(products, f)).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
  if (!built) throw new Error(`no .xctestrun in ${products}`);
  const converted = read('plutil', ['-convert', 'json', '-o', '-', built]);
  if (converted.status !== 0) throw new Error(`plutil ${built}: ${converted.stderr}`);
  const plan = JSON.parse(converted.stdout);
  const strip = (o) => {
    if (!o || typeof o !== 'object') return;
    for (const [key, value] of Object.entries(o)) {
      if (!/^(Testing)?EnvironmentVariables$/.test(key)) { strip(value); continue; }
      for (const name of Object.keys(value)) if (name.startsWith('PERFC_')) delete value[name];
      const inserted = (value.DYLD_INSERT_LIBRARIES ?? '').split(':').filter(l => l && !l.endsWith('/libRPAC.dylib'));
      if (inserted.length) value.DYLD_INSERT_LIBRARIES = inserted.join(':'); else delete value.DYLD_INSERT_LIBRARIES;
    }
  };
  strip(plan);
  const xctestrun = resolve(products, 'exact-tests.xctestrun'), json = `${xctestrun}.json`;
  writeFileSync(json, JSON.stringify(plan));
  const written = read('plutil', ['-convert', 'xml1', '-o', xctestrun, json]);
  if (written.status !== 0) throw new Error(`plutil ${json}: ${written.stderr}`);
  return xctestrun;
}

/** `xctestrun` with its tests hosted in `app` (an iOS app bundle beside it in
 *  `products`), as Xcode runs app-hosted unit tests: the bundle is copied into
 *  the app's PlugIns and injected into it at launch. Returns the new file's path. */
export function hostedIn(products, xctestrun, app) {
  const converted = read('plutil', ['-convert', 'json', '-o', '-', xctestrun]);
  if (converted.status !== 0) throw new Error(`plutil ${xctestrun}: ${converted.stderr}`);
  const plan = JSON.parse(converted.stdout);
  for (const configuration of plan.TestConfigurations ?? []) for (const target of configuration.TestTargets ?? []) {
    const bundle = target.TestBundlePath.replace('__TESTROOT__', products), name = bundle.split('/').pop();
    mkdirSync(resolve(products, app, 'PlugIns'), { recursive: true });
    rmSync(resolve(products, app, 'PlugIns', name), { recursive: true, force: true });
    cpSync(bundle, resolve(products, app, 'PlugIns', name), { recursive: true });
    Object.assign(target, { TestHostPath: `__TESTROOT__/${app}`, TestBundlePath: `__TESTHOST__/PlugIns/${name}`, IsAppHostedTestBundle: true, DependentProductPaths: [`__TESTROOT__/${app}`] });
    const env = target.TestingEnvironmentVariables ??= {};
    env.DYLD_INSERT_LIBRARIES = ['__PLATFORMS__/iPhoneSimulator.platform/Developer/usr/lib/libXCTestBundleInject.dylib', env.DYLD_INSERT_LIBRARIES].filter(Boolean).join(':');
    env.XCInjectBundleInto = 'unused';
  }
  const hosted = resolve(products, 'exact-tests-hosted.xctestrun'), json = `${hosted}.json`;
  writeFileSync(json, JSON.stringify(plan));
  const written = read('plutil', ['-convert', 'xml1', '-o', hosted, json]);
  if (written.status !== 0) throw new Error(`plutil ${json}: ${written.stderr}`);
  return hosted;
}

/** The tests in `dir` that need what only an app gives them, `ExactKitTests/<Class>/<test>`:
 *  those that throw `missing("scene", …)` (a presentation completes only in a scene's
 *  window) or `missing("accessibility", …)` (UIKit's accessibility runtime), which run
 *  hosted. Each must say so in the test method itself, so a helper's need is not missed. */
export function hostedTests(dir) {
  const tests = new Set();
  for (const file of readdirSync(dir).filter(f => f.endsWith('.swift'))) {
    const lines = readFileSync(resolve(dir, file), 'utf8').split('\n');
    let className = null, func = null;
    lines.forEach((line, i) => {
      className = /class (\w+)\s*:\s*XCTestCase/.exec(line)?.[1] ?? className;
      func = /\bfunc (\w+)\s*\(/.exec(line)?.[1] ?? func;
      if (!/missing\("(scene|accessibility)"/.test(line)) return;
      if (!className || !func?.startsWith('test')) throw new Error(`${file}:${i + 1}: missing(scene or accessibility) outside a test method (${func ?? 'none'}); say it in the test so the test runs hosted`);
      tests.add(`ExactKitTests/${className}/${func}`);
    });
  }
  return [...tests].sort();
}
