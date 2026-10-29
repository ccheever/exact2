// How `build.mjs --test` runs the Swift host tests once they are built:
// on this Mac, a test class to an `xctest` process; on a simulator, the
// `.xctestrun` xcodebuild wrote, without the Thread Performance Checker.
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { resolve } from 'node:path';
import { plistFile } from './assets.mjs';
import { encodePng } from '../../scripts/png.mjs';

const read = (cmd, args, opts = {}) => spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, ...opts });

/** Rebuild fixture plans with `env.EXACT_CONTRACT`; write their paths to `<prefix>_<KEY>_PLAN`.
 * Plans embed the current schema digest. App entries need the test app's `appId` to boot. */
export function compileFixtures(root, run, env, dir, prefix, appId) {
  const contract = args => run(env.EXACT_CONTRACT, ['build', ...args]);
  for (const [name, key] of [['frame-health-live', 'FRAME_HEALTH_LIVE'], ['store-refresh', 'STORE_REFRESH'], ['headless', 'HEADLESS'], ['headless-markdown', 'HEADLESS_MARKDOWN'], ['headless-hidden-markdown', 'HEADLESS_HIDDEN_MARKDOWN'], ['headless-live', 'HEADLESS_LIVE'], ['headless-back', 'HEADLESS_BACK'], ['native-navigation-scroll', 'NATIVE_SCROLL'], ['intersect', 'INTERSECT'], ['native-buttons', 'BUTTONS'], ['keyboard-form', 'KEYBOARD'], ['tab-edge', 'TAB_EDGE'], ['repaint', 'REPAINT'], ['still-title', 'STILL_TITLE'], ['navigation-bars', 'NAVIGATION_BARS'], ['large-title', 'LARGE_TITLE'], ['transparent-bar', 'TRANSPARENT_BAR'], ['bar-band', 'BAR_BAND'], ['pad-bar', 'PAD_BAR'], ['pushed-title', 'PUSHED_TITLE'], ['inline-frame', 'INLINE_FRAME'], ['fieldless', 'FIELDLESS'], ['web-documents', 'WEB_DOCUMENTS'], ['iframe-post-messages', 'IFRAME_POST_MESSAGES'], ['iframe-partition', 'IFRAME_PARTITION'], ['iframe-windows', 'IFRAME_WINDOWS'], ['iframe-pdf', 'IFRAME_PDF'], ['iframe-resident-popover', 'IFRAME_RESIDENT_POPOVER'], ['tab-owner', 'TAB_OWNER'], ['grouped-list-reveal', 'GROUPED_REVEAL'], ['due-now', 'DUE_NOW'], ['caret-reveal', 'CARET_REVEAL'], ['cover-scroll', 'COVER_SCROLL'], ['page-edges', 'PAGE_EDGES'], ['barless-route', 'BARLESS_ROUTE'], ['page-edges-grouped', 'PAGE_EDGES_GROUPED'], ['control-bars', 'CONTROL_BARS'], ['search-scope', 'SEARCH_SCOPE'], ['held-tab', 'HELD_TAB'], ['pull-refresh', 'PULL_REFRESH'], ['edge-effect', 'EDGE_EFFECT'], ['sticky-bar', 'STICKY_BAR'], ['glass-press', 'GLASS_PRESS']]) {
    contract([resolve(root, `scripts/fixtures/${name}.contract`), '-o', env[`${prefix}_${key}_PLAN`] = resolve(dir, `${name}.plan`)]);
  }
  contract([resolve(root, 'host/apple/tests/fixtures/sticky-transition.contract'), '-o', env[`${prefix}_STICKY_TRANSITION_PLAN`] = resolve(dir, 'sticky-transition.plan')]);
  // HeldStateTests' own (a card that presses over a flat leaf), beside them.
  contract([resolve(root, 'host/apple/tests/fixtures/held-state.contract'), '-o', env[`${prefix}_HELD_STATE_PLAN`] = resolve(dir, 'held-state.plan')]);
  // AgentStickyReveal{Mac,IOS}Tests': rows under a sticky header and dock.
  contract([resolve(root, 'host/apple/tests/fixtures/sticky-tap.contract'), '-o', env[`${prefix}_STICKY_TAP_PLAN`] = resolve(dir, 'sticky-tap.plan')]);
  // RestartListIOSTests': a restart over a route whose bar covers a virtualized list.
  contract([resolve(root, 'host/apple/tests/fixtures/restart-list.contract'), '-o', env[`${prefix}_RESTART_LIST_PLAN`] = resolve(dir, 'restart-list.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/feedback-race.contract'), '-o', env[`${prefix}_FEEDBACK_RACE_PLAN`] = resolve(dir, 'feedback-race.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/async-advance.contract'), '-o', env[`${prefix}_ASYNC_ADVANCE_PLAN`] = resolve(dir, 'async-advance.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/reset-gate.contract'), '-o', env[`${prefix}_RESET_GATE_PLAN`] = resolve(dir, 'reset-gate.plan')]);
  // NavigationBarIOSTests': a header whose controls share and add a control.
  contract([resolve(root, 'host/apple/tests/fixtures/bar-share.contract'), '-o', env[`${prefix}_BAR_SHARE_PLAN`] = resolve(dir, 'bar-share.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/dialog-sheet-reads.contract'), '-o', env[`${prefix}_DIALOG_SHEET_READS_PLAN`] = resolve(dir, 'dialog-sheet-reads.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/progress-contexts.contract'), '-o', env[`${prefix}_PROGRESS_CONTEXTS_PLAN`] = resolve(dir, 'progress-contexts.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/agent-unmounted.contract'), '-o', env[`${prefix}_AGENT_UNMOUNTED_PLAN`] = resolve(dir, 'agent-unmounted.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/fractional-borders.contract'), '-o', env[`${prefix}_FRACTIONAL_BORDERS_PLAN`] = resolve(dir, 'fractional-borders.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/top-layer-pins.contract'), '-o', env[`${prefix}_TOP_LAYER_PINS_PLAN`] = resolve(dir, 'top-layer-pins.plan')]);
  // PositionAreaRegionIOSTests' pages: Chrome's position-area table, and mirrored.
  contract([resolve(root, 'host/apple/tests/fixtures/position-area-region.contract'), '-o', env[`${prefix}_POSITION_AREA_PLAN`] = resolve(dir, 'position-area-region.plan')]);
  contract([resolve(root, 'host/apple/tests/fixtures/position-area-region-rtl.contract'), '-o', env[`${prefix}_POSITION_AREA_RTL_PLAN`] = resolve(dir, 'position-area-region-rtl.plan')]);
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
  // An activity whose Lock Screen root has a gradient and a colour (the colour is the tint, the gradient paints).
  const graded = resolve(dir, 'entries/graded');
  mkdirSync(graded, { recursive: true });
  writeFileSync(resolve(graded, 'app.contract'), 'shape GradedState\n  title: string\ncomponent App\n  view\n    text "App"\ncomponent Graded\n  props\n    title: string\n  view\n    view slot="lockScreen" background-color="#1e3a5f" background-image="linear-gradient(to right, #ff0000, #0000ff)"\n      text title color="#ffffff"\n    view slot="compactLeading"\n      text "G"\n    view slot="compactTrailing"\n      text title\n    view slot="minimal"\n      text "G"\n    view slot="expanded"\n      view slot="leading"\n        text title\n      view slot="trailing"\n        text title\n      view slot="center"\n        text title\n      view slot="bottom"\n        text title\n');
  writeFileSync(resolve(graded, 'app.json'), JSON.stringify({ name: 'Graded activity', app: { id: appId, name: 'Graded activity' }, live_activities: [{ kind: 'graded', component: 'Graded', state: 'GradedState', preview: { title: 'Preview title' } }] }));
  contract([resolve(graded, 'app.contract'), '--entry', 'graded', '-o', env[`${prefix}_SNAPSHOT_GRADED_PLAN`] = resolve(dir, 'graded.plan')]);
  // An activity exercising the sizes a region takes (ActivityDriverTests): root margins, `flex-basis: 0` beside an
  // authored width, a `min-width` past the proposal, a percentage of the proposal, `margin: auto`, justified content,
  // a wrapping paragraph, viewport-dependent content; and no `data-privacy` word, so the Lock Screen redacts it.
  const sized = resolve(dir, 'entries/sized');
  mkdirSync(sized, { recursive: true });
  writeFileSync(resolve(sized, 'app.contract'), 'shape SizedState\n  title: string\n  long: string\nshape SizedViewport\n  width: number\n  height: number\ncomponent App\n  view\n    text "App"\ncomponent Sized\n  props\n    title: string\n    long: string\n  inject\n    snapshot_viewport: SizedViewport\n  view\n    view slot="lockScreen" margin=12\n      text title\n      text long\n    view slot="compactLeading" width=120 min-width=0 flex-basis=0\n      text title\n    view slot="compactTrailing" min-width=200\n      text title\n    view slot="minimal" width="50%"\n      text title\n    view slot="expanded"\n      view slot="leading" display="flex" flex-direction="row" justify-content="flex-end"\n        text title\n      view slot="trailing"\n        text long\n      view slot="center" margin="auto"\n        text title\n      view slot="bottom"\n        text title\n        if snapshot_viewport.width > 160\n          text long\n');
  writeFileSync(resolve(sized, 'app.json'), JSON.stringify({ name: 'Sized activity', app: { id: appId, name: 'Sized activity' }, live_activities: [{ kind: 'sized', component: 'Sized', state: 'SizedState', preview: { title: 'Hi', long: 'A long paragraph that wraps to several lines in any Island region or Lock Screen card.' } }] }));
  contract([resolve(sized, 'app.contract'), '--entry', 'sized', '-o', env[`${prefix}_SNAPSHOT_SIZED_PLAN`] = resolve(dir, 'sized.plan')]);
  const edges = resolve(dir, 'entries/activity-edges');
  mkdirSync(edges, { recursive: true });
  cpSync(resolve(root, 'host/apple/tests/fixtures/activity-edges.contract'), resolve(edges, 'app.contract'));
  writeFileSync(resolve(edges, 'app.json'), JSON.stringify({ app: { id: appId, name: 'Activity edges' }, live_activities: [{ kind: 'edges', component: 'Edges', state: 'EdgeState', preview: { title: 'Private account label' } }] }));
  contract([resolve(edges, 'app.contract'), '--entry', 'edges', '-o', env[`${prefix}_SNAPSHOT_EDGES_PLAN`] = resolve(dir, 'edges.plan')]);
  if (prefix === 'EXACT') {
    const app = resolve(dir, 'SnapshotAssets.app'), resources = resolve(app, 'Contents/Resources');
    const extension = resolve(app, 'Contents/PlugIns/Card.appex');
    const catalog = resolve(dir, 'SnapshotAssets.xcassets');
    for (const path of [resources, resolve(resources, 'assets'), resolve(extension, 'Contents/Resources'), resolve(catalog, 'WidgetInk.colorset')]) mkdirSync(path, { recursive: true });
    for (const [bundle, id] of [[app, 'dev.exact.snapshot-assets'], [extension, 'dev.exact.snapshot-assets.widget']])
      writeFileSync(resolve(bundle, 'Contents/Info.plist'), plistFile({ CFBundleIdentifier: id, CFBundlePackageType: bundle === app ? 'APPL' : 'XPC!', CFBundleExecutable: 'Fixture' }));
    writeFileSync(resolve(catalog, 'Contents.json'), JSON.stringify({ info: { version: 1, author: 'xcode' } }));
    writeFileSync(resolve(catalog, 'WidgetInk.colorset/Contents.json'), JSON.stringify({ info: { version: 1, author: 'xcode' }, colors: [{ idiom: 'universal', color: { 'color-space': 'srgb', components: { red: '1.0', green: '0.0', blue: '0.0', alpha: '1.0' } } }] }));
    run('xcrun', ['actool', '--compile', resources, '--platform', 'macosx', '--minimum-deployment-target', '14.0', '--target-device', 'mac', catalog]);
    writeFileSync(resolve(resources, 'assets/widget.png'), encodePng({ width: 1, height: 1, data: Buffer.from([255, 0, 0, 255]) }));
    env.EXACT_SNAPSHOT_ASSETS_EXTENSION = extension;
  }
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
 *  to its own process. A class whose file opens a menu that tracks, by a
 *  call of `.popUp(positioning:`, runs after all of them, alone, in a
 *  process of its own: an open menu closes when its app stops being the
 *  active one, as another test process brought forward makes it, and in the
 *  shared process it read the menu items other classes left. The Canvas 2D
 *  GPU module (`canvasGpu`) is loaded only in its own tests' process:
 *  loaded, every canvas draws on the GPU. */
export async function macTests(pkg, scratch, env, canvasGpu) {
  const listed = read('swift', ['test', '--skip-build', '--list-tests', '--scratch-path', scratch], { cwd: pkg, env });
  if (listed.status !== 0) throw new Error(`swift test --list-tests failed: ${listed.stderr}`);
  const classes = [...new Set(listed.stdout.match(/^ExactKitTests\.\w+(?=\/)/gm) ?? [])];
  const macWide = /NSPasteboard\.general|\bactivate\(|NSEvent\.mouseLocation|CGWindowListCreateImage/;
  const tracksMenu = /\.popUp\(positioning:/;
  const shared = new Set(), solo = new Set();
  for (const file of readdirSync(resolve(pkg, 'tests/ExactKitTests')).filter(f => f.endsWith('.swift'))) {
    const source = readFileSync(resolve(pkg, 'tests/ExactKitTests', file), 'utf8');
    const named = [...source.matchAll(/class (\w+)\s*:\s*XCTestCase/g)].map(([, name]) => `ExactKitTests.${name}`);
    for (const name of named) if (tracksMenu.test(source)) solo.add(name); else if (macWide.test(source)) shared.add(name);
  }
  const gpuClass = 'ExactKitTests.Canvas2DGpuStressTests';
  shared.delete(gpuClass);
  const groups = [classes.filter(c => shared.has(c)), ...classes.filter(c => !shared.has(c) && !solo.has(c)).map(c => [c])].filter(g => g.length);
  const alone = classes.filter(c => solo.has(c)).map(c => [c]);
  // SwiftPM names the test bundle after the test target up to Swift 6.2 and after the package from 6.3 on.
  const bin = read('swift', ['build', '--show-bin-path', '--scratch-path', scratch], { cwd: pkg, env }).stdout.trim();
  const bundle = ['ExactKitTests.xctest', 'ExactPackageTests.xctest'].map(name => resolve(bin, name)).find(existsSync) ?? resolve(bin, 'ExactKitTests.xctest');
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
  for (const group of alone) await one(group);
  console.log(`host/apple: ${classes.length} test classes in ${groups.length + alone.length} processes (${alone.length} alone after the rest), ${failed.length} failed`);
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
      className = /class (\w+)\s*:\s*(XCTestCase|\w+Fixture)\b/.exec(line)?.[1] ?? className;
      func = /\bfunc (\w+)\s*\(/.exec(line)?.[1] ?? func;
      if (!/missing\("(scene|accessibility)"/.test(line)) return;
      if (!className || !func?.startsWith('test')) throw new Error(`${file}:${i + 1}: missing(scene or accessibility) outside a test method (${func ?? 'none'}); say it in the test so the test runs hosted`);
      tests.add(`ExactKitTests/${className}/${func}`);
    });
  }
  return [...tests].sort();
}
