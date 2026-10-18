// @ref LLP 1126.001 WP17 — watch complications use the snapshot driver and their own store.
import { resolve, relative } from 'node:path';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, writeFileSync, copyFileSync, cpSync, rmSync, readFileSync } from 'node:fs';
import { buildBake, bakeOutput, developmentBuildEnv, HOST_DEV, executableName } from '../../scripts/app.mjs';
import { plistFile } from './assets.mjs';
import { platformWidgetEntries, prepareWidgetCore, sharedWidgetCore, packageWidgets } from './widgets.mjs';
const root = resolve(new URL('../..', import.meta.url).pathname), pkg = resolve(root, 'host/apple');

export function watchEntries(app, rustSources = []) {
  return platformWidgetEntries(app, 'watchos').filter(entry => {
    // The mixed bake's receipt names Rust-owned refreshes without running JS.
    if (!existsSync(resolve(app.dir, 'app.ts')) || rustSources.includes(entry.refresh)) return true;
    console.log(`host/apple: watchOS omits TypeScript entry ${entry.kind} until LLP 1126.001 §5 Q3 passes`);
    return false;
  });
}
export function watchSimulator(args) {
  const at = args.indexOf('--watch-sim');
  if (at < 0) return undefined;
  const id = args[at + 1];
  if (!/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(id ?? '')) throw new Error('--watch-sim requires a simulator id');
  return id;
}
export async function buildWatch(app, args, { runApple, appleArtifacts, appleBuildLock, receipt, buildParent }) {
  if (args.some(a => ['--device', '--archive', '--tvos', '--host', '--embed', '--test', '--url'].includes(a))) throw new Error('--watchos currently builds the watch simulator companion; use --sim for its iPhone and --watch-sim for its paired watch');
  if (args.includes('--run') && args.includes('--no-install')) throw new Error('--no-install takes no --run');
  const watchSim = watchSimulator(args);
  const release = appleBuildLock(app);
  try {
    const parentArgs = args.filter((a, i) => !['--watchos', '--run', '--no-install', '--watch-sim'].includes(a) && args[i - 1] !== '--watch-sim');
    await buildParent([...parentArgs, '--ios', '--no-install']);
    const parent = appleArtifacts(app, { destination: 'ios-simulator' });
    const sourceReceipt = resolve(parent.bundle, 'typescript/app.module.json');
    const rustSources = existsSync(sourceReceipt) ? JSON.parse(readFileSync(sourceReceipt, 'utf8')).rustSources ?? [] : [];
    const entries = watchEntries(app, rustSources);
    const watch = { ...app, id: `${app.id}.watchkitapp` };
    const bundle = resolve(parent.bundle, 'Watch', `${executableName(app)}.app`);
    rmSync(resolve(parent.bundle, 'Watch'), { recursive: true, force: true });
    if (!entries.length) {
      console.log(`host/apple: no Rust accessory entries; watch bundle omitted${args.includes('--run') ? '; --run skipped' : ''}`);
      return;
    }
    const production = args.includes('--release') || process.env.EXACT_UPDATE_TRUST === 'production';
    const profile = production ? 'release' : HOST_DEV;
    const directory = resolve(parent.namespace, 'watchos'), target = 'aarch64-apple-watchos-sim', triple = 'arm64-apple-watchos10.0-simulator';
    mkdirSync(directory, { recursive: true });
    const sdk = runApple('xcrun', ['--sdk', 'watchsimulator', '--show-sdk-path']).stdout.trim();
    const env = { ...developmentBuildEnv(), EXACT_UPDATE_TRUST: production ? 'production' : 'development', WATCHOS_DEPLOYMENT_TARGET: '10.0', SDKROOT: sdk, EXACT_APPLE_LINK: 'plan' };
    const baked = buildBake({ ...app, hasGpu: false }, 'ios', target, { env, profile });
    const output = resolve(bakeOutput(app, env), `ios-${target}.entries`);
    const libDir = resolve(app.target, target, profile), crate = app.crate('ios');
    const swiftRoot = resolve(directory, 'swift');
    const swiftArgs = ['build', '-c', production ? 'release' : 'debug', '--scratch-path', swiftRoot, ...(!production ? ['-Xswiftc', '-O'] : []),
      '--triple', triple, '--sdk', sdk, '-Xcc', '-isysroot', '-Xcc', sdk, '-Xlinker', '-syslibroot', '-Xlinker', sdk,
      '-Xswiftc', '-Xclang-linker', '-Xswiftc', '-isysroot', '-Xswiftc', '-Xclang-linker', '-Xswiftc', sdk];
    const swiftEnv = { ...process.env, WATCHOS_DEPLOYMENT_TARGET: '10.0', EXACT_LIB_DIR: libDir, EXACT_LIB: crate.replaceAll('-', '_'), EXACT_WIDGET_GALLERY: '0', EXACT_UPDATE_TRUST: production ? 'production' : 'development' };
    delete swiftEnv.SDKROOT;
    const prepared = prepareWidgetCore({ app: watch, directory: resolve(directory, 'core'), swiftBuildRoot: swiftRoot, swiftArgs, triple, platform: 'watchos', device: false,
      swiftc: runApple('xcrun', ['swiftc', '--version']).stdout });
    const shared = sharedWidgetCore({ app: watch, prepared, libDir, crate, env: swiftEnv, runApple });
    const group = `group.${watch.id}`, entitlements = plistFile({ 'com.apple.security.application-groups': [group] });
    const entitlementPath = resolve(directory, 'watch-entitlements.plist');
    writeFileSync(entitlementPath, entitlements);
    // Simulator entitlements are embedded in __TEXT by each link.
    const sign = (path, _embeddedEntitlements) => runApple('codesign', ['--force', '--sign', '-', path]);
    mkdirSync(resolve(bundle, 'Frameworks'), { recursive: true });
    cpSync(shared.framework, resolve(bundle, 'Frameworks/ExactCore.framework'), { recursive: true });
    mkdirSync(resolve(bundle, 'plans'), { recursive: true });
    for (const entry of entries) copyFileSync(resolve(output, `plans/${entry.kind}.plan`), resolve(bundle, `plans/${entry.kind}.plan`));
    for (const name of ['assets', 'deck']) if (existsSync(resolve(app.dir, name))) cpSync(resolve(app.dir, name), resolve(bundle, name), { recursive: true });
    const appInfo = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', resolve(parent.bundle, 'Info.plist')]).stdout);
    const info = { CFBundleIdentifier: watch.id, CFBundleName: app.displayName, CFBundleDisplayName: app.displayName, CFBundleExecutable: 'ExactWatch', CFBundlePackageType: 'APPL',
      CFBundleVersion: appInfo.CFBundleVersion, CFBundleShortVersionString: appInfo.CFBundleShortVersionString, CFBundleSupportedPlatforms: ['WatchSimulator'], MinimumOSVersion: '10.0', UIDeviceFamily: [4],
      WKApplication: true, WKCompanionAppBundleIdentifier: app.id, WKRunsIndependentlyOfCompanionApp: true,
      ExactAppGroup: group, ExactAppIdentifier: app.id, ExactWidgetTypeScript: false, ExactWidgetGeneration: createHash('sha256').update(JSON.stringify([baked.compat.id, baked.graph.artifacts])).digest('hex'),
      ExactWatchEntries: entries.map(({ kind, families, title }) => ({ kind, families, title: title ?? kind })) };
    writeFileSync(resolve(bundle, 'Info.plist'), plistFile(info));
    const executableArgs = [...swiftArgs, '-Xlinker', '-rpath', '-Xlinker', '@executable_path/Frameworks',
      '-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', entitlementPath];
    const options = { cwd: pkg, env: { ...swiftEnv, EXACT_CORE_FRAMEWORK: relative(pkg, shared.xcframework) } };
    runApple('swift', [...executableArgs, '--product', 'ExactWatch'], options);
    const bin = runApple('swift', [...executableArgs, '--show-bin-path'], options).stdout.trim();
    copyFileSync(resolve(bin, 'ExactWatch'), resolve(bundle, 'ExactWatch'));
    packageWidgets({ app: watch, bundle, shared, directory, sdk, triple, device: false, entitlements, sign, runApple, production, platform: 'watchos', entries });
    sign(resolve(bundle, 'Frameworks/ExactCore.framework'));
    sign(bundle);
    sign(parent.bundle);
    writeFileSync(resolve(directory, 'receipt.json'), receipt(app, { platform: 'watchos-simulator', target, sdk, bundle, compatibilityId: baked.compat.id, build: baked }));
    console.log(`host/apple: ${bundle}; watch group ${group}; ${entries.map(e => `${e.kind} [${e.families.join(', ')}]`).join('; ')}`);
    if (args.includes('--no-install')) return;
    const pick = flag => args.includes(flag) ? args[args.indexOf(flag) + 1] : undefined;
    const pairs = JSON.parse(runApple('xcrun', ['simctl', 'list', 'pairs', '-j']).stdout).pairs;
    const devices = Object.values(pairs).filter(pair => pair.watch && pair.phone);
    const matches = devices.filter(pair => (!watchSim || pair.watch.udid.toLowerCase() === watchSim.toLowerCase()) && (!pick('--sim') || [pair.phone.udid, pair.phone.name].includes(pick('--sim'))));
    if (matches.length !== 1) throw new Error('name one existing simulator pair with --sim <iPhone udid> --watch-sim <watch udid>; this build creates and shuts down no simulators');
    const pair = matches[0];
    for (const device of [pair.phone, pair.watch]) {
      if (device.state !== 'Booted') throw new Error(`boot your simulator first: xcrun simctl boot ${device.udid}`);
    }
    runApple('xcrun', ['simctl', 'install', pair.phone.udid, parent.bundle]);
    runApple('xcrun', ['simctl', 'install', pair.watch.udid, bundle]);
    if (args.includes('--run')) runApple('xcrun', ['simctl', 'launch', pair.watch.udid, watch.id]);
  } finally { release(); }
}
