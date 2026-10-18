// @ref LLP 1126 D2/D8 — one runtime framework, one generated widget bundle.
import { resolve, basename } from 'node:path';
import { mkdirSync, readdirSync, existsSync, writeFileSync, readFileSync, cpSync, rmSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { plistFile } from './assets.mjs';
const root = resolve(new URL('../..', import.meta.url).pathname);
function run(args) {
  const result = spawnSync('xcrun', args, { stdio: 'inherit' });
  if (result.status !== 0) throw new Error(`widget build failed: xcrun ${args[0]}`);
}
export const widgetEntries = app => app.manifest.widgets ?? [];
export const widgetGroup = app => `group.${app.widgetAppId ?? app.id}`;
export const widgetId = app => `${app.id}.widgets`;
export function sharedWidgetCore({ app, directory, libDir, crate, sdk, triple, device }) {
  const sources = readdirSync(resolve(root, 'host/apple/Sources/ExactCore')).filter(f => f.endsWith('.swift')).map(f => resolve(root, 'host/apple/Sources/ExactCore', f));
  sources.push(resolve(root, 'host/apple/widget/Renderer.swift'));
  const archive = resolve(libDir, `lib${crate.replaceAll('-', '_')}.a`);
  const key = createHash('sha256').update(JSON.stringify([triple, sdk])).update(readFileSync(archive));
  for (const source of sources) key.update(readFileSync(source));
  key.update(readFileSync(resolve(root, 'host/apple/include/exact.h')));
  key.update(readFileSync(new URL(import.meta.url)));
  const base = resolve(directory, key.digest('hex').slice(0, 20));
  const framework = resolve(base, 'ExactCore.framework'), xcframework = resolve(base, 'ExactCore.xcframework');
  if (existsSync(resolve(xcframework, 'Info.plist'))) return { framework, xcframework };
  const module = resolve(framework, 'Modules/ExactCore.swiftmodule');
  mkdirSync(module, { recursive: true });
  run(['--sdk', device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-emit-library', '-emit-module', '-whole-module-optimization', '-O', '-swift-version', '5', '-package-name', 'apple', '-module-name', 'ExactCore', '-D', 'EXACT_WIDGETS', '-enable-testing', '-application-extension', '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'), '-I', resolve(root, 'host/apple/Sources/CExact'),
    ...sources, '-Xlinker', '-force_load', '-Xlinker', archive, '-lc++', '-framework', 'ImageIO',
    '-Xlinker', '-install_name', '-Xlinker', '@rpath/ExactCore.framework/ExactCore',
    '-emit-module-path', resolve(module, `${triple.replace(/ios[\d.]+/, 'ios')}.swiftmodule`), '-o', resolve(framework, 'ExactCore')]);
  writeFileSync(resolve(framework, 'Info.plist'), plistFile({ CFBundleIdentifier: `${app.id}.runtime`, CFBundleName: 'ExactCore', CFBundleExecutable: 'ExactCore', CFBundlePackageType: 'FMWK', CFBundleVersion: '1', CFBundleShortVersionString: '0.1.0', CFBundleSupportedPlatforms: [device ? 'iPhoneOS' : 'iPhoneSimulator'], MinimumOSVersion: '17.0' }));
  rmSync(xcframework, { recursive: true, force: true });
  run(['xcodebuild', '-create-xcframework', '-allow-internal-distribution', '-framework', framework, '-output', xcframework]);
  return { framework, xcframework };
}
export function packageWidgets({ app, bundle, shared, directory, sdk, triple, device, entitlements, sign, minimumOS }) {
  const extension = resolve(bundle, 'PlugIns/ExactWidgets.appex');
  mkdirSync(extension, { recursive: true });
  cpSync(shared.framework, resolve(bundle, 'Frameworks/ExactCore.framework'), { recursive: true });
  const entries = widgetEntries(app);
  const names = entries.map((_, i) => `Entry${i}`);
  const generated = entries.map((entry, i) => `struct ${names[i]}: Widget {
    let kind = ${JSON.stringify(entry.kind)}
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: ExactTimelineProvider(kind: kind)) { entry in ExactWidgetView(entry: entry) }
        .configurationDisplayName(${JSON.stringify(entry.kind)}).description(${JSON.stringify(app.displayName)})
        .supportedFamilies([${entry.families.map(f => `.${f}`).join(', ')}]).contentMarginsDisabled()
    }
}`).join('\n');
  const source = resolve(directory, 'WidgetBundle.swift');
  writeFileSync(source, `import SwiftUI\nimport WidgetKit\n${generated}\n@main struct ExactWidgets: WidgetBundle { var body: some Widget { ${names.map(n => `${n}()`).join('; ')} } }\n`);
  const entitlementPath = resolve(directory, 'widget-entitlements.plist');
  writeFileSync(entitlementPath, entitlements);
  writeFileSync(resolve(extension, 'Info.plist'), plistFile({ CFBundleIdentifier: widgetId(app), CFBundleName: 'ExactWidgets', CFBundleDisplayName: app.displayName, CFBundleExecutable: 'ExactWidgets', CFBundlePackageType: 'XPC!', UIDeviceFamily: [1, 2], CFBundleVersion: '1', CFBundleShortVersionString: '0.1.0', MinimumOSVersion: minimumOS,
    CFBundleSupportedPlatforms: [device ? 'iPhoneOS' : 'iPhoneSimulator'], ExactAppGroup: widgetGroup(app), ExactAppIdentifier: app.id,
    NSExtension: { NSExtensionPointIdentifier: 'com.apple.widgetkit-extension' } }));
  // Preview reads the embedded entry, never a group's current generation.
  cpSync(resolve(bundle, 'plans'), resolve(extension, 'plans'), { recursive: true });
  run(['--sdk', device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-O', '-application-extension', '-Xlinker', '-e', '-Xlinker', '_NSExtensionMain', '-Xlinker', '-u', '-Xlinker', '_main', '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'), '-I', resolve(root, 'host/apple/Sources/CExact'), '-F', resolve(shared.framework, '..'), '-framework', 'ExactCore',
    '-Xlinker', '-rpath', '-Xlinker', '@executable_path/../../Frameworks',
    ...(!device ? ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', entitlementPath] : []),
    resolve(root, 'host/apple/widget/Timeline.swift'), source, '-o', resolve(extension, 'ExactWidgets')]);
  sign(extension, entitlementPath);
}
