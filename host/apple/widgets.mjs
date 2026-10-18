// @ref LLP 1126 D2/D8 — one runtime framework, one generated widget bundle.
import { resolve } from 'node:path';
import { mkdirSync, readdirSync, existsSync, writeFileSync, readFileSync, copyFileSync, cpSync, rmSync, renameSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { plistFile, wrapFramework } from './assets.mjs';
import { assertCapabilityFrameworks } from './link.mjs';
const root = resolve(new URL('../..', import.meta.url).pathname), pkg = resolve(root, 'host/apple');
export const accessoryFamilies = ['accessoryCircular', 'accessoryRectangular', 'accessoryInline', 'accessoryCorner'];
export function platformWidgetEntries(app, platform = 'ios') {
  return widgetEntries(app).flatMap(entry => {
    const families = entry.families.filter(family => platform === 'watchos' ? accessoryFamilies.includes(family) : family !== 'accessoryCorner');
    return families.length ? [{ ...entry, families }] : [];
  });
}
export const widgetEntries = app => app.manifest.widgets ?? [];
export const widgetGroup = app => `group.${app.widgetAppId ?? app.id}`;
export const widgetId = app => `${app.id}.widgets`;
const tree = dir => readdirSync(dir, { withFileTypes: true }).flatMap(e => e.isDirectory() ? tree(resolve(dir, e.name)) : [resolve(dir, e.name)]);

/** SwiftPM owns the sources, settings, incremental objects and module. The
 * wrapper's stable path depends on that interface, never on the Rust archive. */
export function prepareWidgetCore({ app, directory, swiftBuildRoot, swiftArgs, swiftc, triple, device, platform = 'ios' }) {
  const scratch = resolve(swiftBuildRoot, 'core');
  const args = swiftArgs.map(a => a === swiftBuildRoot ? scratch : a);
  const key = createHash('sha256').update(JSON.stringify([app.id, args, swiftc, triple]));
  for (const file of [resolve(pkg, 'Package.swift'), resolve(pkg, 'include/exact.h'), ...['ExactCore', 'ExactSnapshot', 'CExact'].flatMap(name => tree(resolve(pkg, 'Sources', name)))].sort()) key.update(file).update(readFileSync(file));
  const base = resolve(directory, key.digest('hex').slice(0, 20));
  return { base, scratch, args, triple, device, platform, framework: resolve(base, 'ExactCore.framework'), xcframework: resolve(base, 'ExactCore.xcframework') };
}
export function sharedWidgetCore({ app, prepared, libDir, crate, env, runApple }) {
  const { base, args, framework, xcframework, device, platform } = prepared;
  mkdirSync(base, { recursive: true });
  const coreEnv = { ...env, EXACT_WIDGET_CORE: '1' };
  delete coreEnv.EXACT_CORE_FRAMEWORK;
  const options = { cwd: pkg, env: coreEnv };
  const answer = resolve(base, 'bin-path');
  let bin = existsSync(answer) ? readFileSync(answer, 'utf8') : '';
  if (!bin || !existsSync(bin)) {
    bin = runApple('swift', [...args, '--show-bin-path'], options).stdout.trim();
    writeFileSync(answer, bin);
  }
  const dylib = resolve(bin, 'libExactCore.dylib');
  const archive = resolve(libDir, `lib${crate.replaceAll('-', '_')}.a`);
  const digest = createHash('sha256').update(readFileSync(archive)).digest('hex');
  const stamp = resolve(base, 'archive');
  // SwiftPM does not track an external archive. Invalidate only its link;
  // the objects/module stay incremental, including on a kernel-only edit.
  if (!existsSync(stamp) || readFileSync(stamp, 'utf8') !== digest) rmSync(dylib, { force: true });
  runApple('swift', [...args, '--product', 'ExactCore'], options);
  // Only the extension links the snapshot target. Keeping it out of the shared
  // runtime keeps SwiftUI and WidgetKit out of an ordinary app launch.
  runApple('swift', [...args, '--product', 'ExactSnapshot'], options);
  const changed = !existsSync(stamp) || readFileSync(stamp, 'utf8') !== digest || !existsSync(resolve(framework, 'ExactCore'));
  if (changed) {
    mkdirSync(resolve(framework, 'Modules'), { recursive: true });
    const temporary = resolve(base, 'ExactCore.tmp');
    copyFileSync(dylib, temporary);
    runApple('xcrun', ['install_name_tool', '-id', '@rpath/ExactCore.framework/ExactCore', temporary]);
    renameSync(temporary, resolve(framework, 'ExactCore'));
    for (const name of ['ExactCore', 'ExactSnapshot']) cpSync(resolve(bin, `${name}.swiftmodule`), resolve(framework, 'Modules', `${name}.swiftmodule`), { recursive: true, preserveTimestamps: true });
    writeFileSync(resolve(framework, 'Info.plist'), plistFile({ CFBundleIdentifier: `${app.id}.runtime`, CFBundleName: 'ExactCore', CFBundleExecutable: 'ExactCore', CFBundlePackageType: 'FMWK', CFBundleVersion: '1', CFBundleShortVersionString: '0.1.0', CFBundleSupportedPlatforms: [platform === 'watchos' ? 'WatchSimulator' : device ? 'iPhoneOS' : 'iPhoneSimulator'], MinimumOSVersion: platform === 'watchos' ? '10.0' : app.manifest.host?.ios?.minimumOS ?? '17.0' }));
    if (!existsSync(resolve(xcframework, 'Info.plist'))) runApple('xcrun', ['xcodebuild', '-create-xcframework', '-allow-internal-distribution', '-framework', framework, '-output', xcframework]);
    else {
      const slice = readdirSync(xcframework).find(name => existsSync(resolve(xcframework, name, 'ExactCore.framework')));
      const target = resolve(xcframework, slice, 'ExactCore.framework', 'ExactCore');
      copyFileSync(resolve(framework, 'ExactCore'), target + '.tmp');
      renameSync(target + '.tmp', target);
    }
    writeFileSync(stamp, digest);
  }
  assertCapabilityFrameworks(resolve(framework, 'ExactCore'), []);
  return { ...prepared, snapshotArchive: resolve(bin, 'libExactSnapshot.a') };
}
/** The reload artifact has no app-specific inputs. Reuse it until its source or target changes. */
export function widgetReload({ directory, triple, sdk, device, runApple, source = resolve(pkg, 'Sources/ExactCore/WidgetContainer.swift') }) {
  const digest = createHash('sha256').update(readFileSync(source)).update(JSON.stringify([triple, sdk])).digest('hex');
  const binary = resolve(directory, 'libexact_widget_reload.dylib'), stamp = resolve(directory, 'widget-reload.sha256');
  if (existsSync(binary) && existsSync(stamp) && readFileSync(stamp, 'utf8') === digest) return binary;
  mkdirSync(directory, { recursive: true });
  runApple('xcrun', ['--sdk', device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-emit-library', '-O',
    '-D', 'EXACT_WIDGET_RELOAD', '-module-name', 'ExactWidgetReload', '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'), source, '-o', binary + '.tmp']);
  renameSync(binary + '.tmp', binary);
  writeFileSync(stamp, digest);
  return binary;
}
export function packageWidgets({ app, bundle, shared, directory, sdk, triple, device, entitlements, sign, runApple, production, platform = 'ios', entries = platformWidgetEntries(app, platform) }) {
  const extension = resolve(bundle, 'PlugIns/ExactWidgets.appex');
  mkdirSync(extension, { recursive: true });
  for (const tree of ['plans', ...(existsSync(resolve(app.dir, 'app.ts')) ? ['typescript'] : [])]) cpSync(resolve(bundle, tree), resolve(extension, tree), { recursive: true });
  const appInfo = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', resolve(bundle, 'Info.plist')]).stdout);
  const versions = Object.fromEntries(['CFBundleVersion', 'CFBundleShortVersionString', 'MinimumOSVersion'].map(key => [key, appInfo[key]]));
  const runtimeInfo = resolve(bundle, 'Frameworks/ExactCore.framework/Info.plist');
  const runtime = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', runtimeInfo]).stdout);
  writeFileSync(runtimeInfo, plistFile({ ...runtime, ...versions }));
  const frameworks = resolve(bundle, 'Frameworks'), reload = 'libexact_widget_reload.dylib';
  if (platform !== 'watchos') {
    copyFileSync(widgetReload({ directory, triple, sdk, device, runApple }), resolve(frameworks, reload));
    wrapFramework(frameworks, reload, 'ExactWidgetReload', app, device ? 'iPhoneOS' : 'iPhoneSimulator');
  }
  const names = entries.map((_, i) => `Entry${i}`);
  const human = kind => kind.replace(/[-_]+/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
  const generated = entries.map((entry, i) => `struct ${names[i]}: Widget {
    let kind = ${JSON.stringify(entry.kind)}
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: ExactTimelineProvider(kind: kind)) { entry in
            ExactWidgetView(entry: entry)${platform === 'ios' ? '.environment(\\.widgetDisplayGamut, UITraitCollection.current.displayGamut == .P3 ? \"p3\" : \"srgb\")' : ''}
        }
        .configurationDisplayName(${JSON.stringify(entry.title ?? human(entry.kind))}).description(${JSON.stringify(entry.description ?? human(entry.kind))})
        .supportedFamilies([${entry.families.map(f => `.${f}`).join(', ')}]).contentMarginsDisabled()
    }
}`).join('\n');
  const source = resolve(directory, 'WidgetBundle.swift');
  writeFileSync(source, `import SwiftUI\nimport WidgetKit\n${platform === 'ios' ? 'import UIKit\n' : ''}${generated}\n@main struct ExactWidgets: WidgetBundle { var body: some Widget { ${names.map(n => `${n}()`).join('; ')} } }\n`);
  const entitlementPath = resolve(directory, 'widget-entitlements.plist');
  writeFileSync(entitlementPath, entitlements);
  writeFileSync(resolve(extension, 'Info.plist'), plistFile({ CFBundleIdentifier: widgetId(app), CFBundleName: 'ExactWidgets', CFBundleDisplayName: app.displayName, CFBundleExecutable: 'ExactWidgets', CFBundlePackageType: 'XPC!', ...versions, UIDeviceFamily: appInfo.UIDeviceFamily,
    CFBundleSupportedPlatforms: appInfo.CFBundleSupportedPlatforms, ...(appInfo.NSAppTransportSecurity ? { NSAppTransportSecurity: appInfo.NSAppTransportSecurity } : {}), ExactAppGroup: appInfo.ExactAppGroup ?? widgetGroup(app), ExactAppIdentifier: appInfo.ExactAppIdentifier ?? app.id, ExactWidgetTypeScript: existsSync(resolve(app.dir, 'app.ts')), ...(appInfo.ExactKeychainAccessGroup ? { ExactKeychainAccessGroup: appInfo.ExactKeychainAccessGroup } : {}), ExactWidgetGeneration: appInfo.ExactWidgetGeneration, ...(typeof app.origin === 'string' ? { ExactLinkBase: app.origin } : {}),
    NSExtension: { NSExtensionPointIdentifier: 'com.apple.widgetkit-extension' } }));
  runApple('xcrun', ['--sdk', platform === 'watchos' ? 'watchsimulator' : device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-O', '-application-extension', ...(!production ? ['-D', 'EXACT_INSPECT'] : []), '-Xlinker', '-e', '-Xlinker', '_NSExtensionMain', '-Xlinker', '-u', '-Xlinker', '_main', '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'), '-I', resolve(root, 'host/apple/Sources/CExact'), '-I', resolve(shared.framework, 'Modules'), '-F', resolve(shared.framework, '..'), '-framework', 'ExactCore',
    '-Xlinker', '-rpath', '-Xlinker', '@executable_path/../../Frameworks',
    ...(!device ? ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', entitlementPath] : []),
    shared.snapshotArchive, resolve(root, 'host/apple/widget/Timeline.swift'), source, '-o', resolve(extension, 'ExactWidgets')]);
  sign(extension, entitlementPath);
}
