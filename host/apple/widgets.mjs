// @ref LLP 1126 D2/D8 — one runtime framework, one generated widget bundle.
import { resolve } from 'node:path';
import { mkdirSync, readdirSync, existsSync, writeFileSync, readFileSync, copyFileSync, cpSync, rmSync, renameSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { plistFile, wrapFramework } from './assets.mjs';
import { assertCapabilityFrameworks } from './link.mjs';
import { installedProfiles, decodeProfile, profilesFor, profileMisfit, byExpiry, METHODS } from './devices.mjs';
const root = resolve(new URL('../..', import.meta.url).pathname), pkg = resolve(root, 'host/apple');
export const widgetEntries = app => app.manifest.widgets ?? [];
export const activityEntries = app => app.manifest.live_activities ?? [];
/** Whether the app has an entry of either kind: what the extension, the shared framework, the App Group and the
 * Keychain group are gated on (LLP 1126.001 WP13). */
export const hasEntries = app => widgetEntries(app).length > 0 || activityEntries(app).length > 0;
export const widgetGroup = app => `group.${app.widgetAppId ?? app.id}`;
export const widgetId = app => `${app.id}.widgets`;

/** The two profiles a device build of an app with entries embeds, the app's and its extension's (LLP 1126.001
 * WP13): of one team, App ID prefix and method, each granting the App Group and the one Keychain group both sign
 * (`<prefix>.<app id>`, or the team wildcard), the app's also `aps-environment` (as its method signs it) when an
 * activity takes pushes and `required` (the grants' signing entitlements, which the extension never signs); or
 * one refusal naming, for each App ID, the profiles that were close and what each lacked or failed. A build for a
 * phone (`udid`) takes development profiles, signed with the Apple Development identity; an archive (no phone)
 * takes distribution ones, the extension's of the named one's method and, for Ad Hoc, its devices. EXACT_PROFILE
 * names the app's, under the same conditions; the extension's is found by its own id, since a team wildcard
 * grants no App Group. `installed` and `named` are the decoded profiles, which a test replaces. */
export function entryProfiles({ app, udid, required = [], installed = installedProfiles(), named = process.env.EXACT_PROFILE ? decodeProfile(process.env.EXACT_PROFILE) : null }) {
  const group = widgetGroup(app), extension = widgetId(app), push = activityEntries(app).some(entry => entry.push === true);
  const shared = prefix => `${prefix}.${app.id}`;
  const wants = (id, prefix) => [
    { has: p => p.groups.includes(group), missing: `the App Group ${group}`, portal: `App Groups (${group})` },
    { has: p => p.keychain.includes(`${p.prefix}.*`) || p.keychain.includes(shared(p.prefix)), missing: `the Keychain group ${shared(prefix)}`, portal: `Keychain Sharing (the group ${shared(prefix)}, or the team's ${prefix}.*)` },
    ...(id === app.id && push ? [{ has: p => p.aps === (p.dev ? 'development' : 'production'), missing: 'aps-environment (development on a development profile, production otherwise; an activity takes pushes)', portal: 'Push Notifications' }] : []),
    ...(id === app.id ? required.map(name => ({ has: p => p.entitlements.includes(name), missing: name, portal: name })) : []),
  ];
  // The build's method, which a profile that `like` does not decide must have.
  const mode = p => udid ? (p.dev ? null : `is ${METHODS[p.method]}, not development`) : (p.dev ? 'is development; an archive takes a distribution profile' : null);
  // `candidates` are the profiles for the id; `like` the profile the choice must match, else the build's method;
  // `label` how a profile is named; `reject` a reason that stands before the rest (a named profile for another App ID).
  const assess = (id, candidates, like, { label = p => `"${p.name}"`, reject = () => null } = {}) => {
    const needs = wants(id, candidates[0]?.prefix ?? '<prefix>');
    const fits = [], misfits = [];
    for (const p of candidates) {
      const why = reject(p) ?? (like ? profileMisfit(p, udid, like) : mode(p) ?? profileMisfit(p, udid));
      if (why) misfits.push([p, why]); else fits.push(p);
    }
    fits.sort(byExpiry);
    const found = fits.find(p => needs.every(want => want.has(p)));
    if (found) return { found };
    // What to register: a grant no profile for the id makes, fit or not (a misfit profile already has the rest).
    const lacking = needs.filter(want => !candidates.some(p => want.has(p)));
    const fitsLack = needs.filter(want => !fits.some(p => want.has(p)));
    const parts = [];
    if (fits.length) parts.push(`${fits.map(label).join(', ')} ${fits.length > 1 ? 'cover' : 'covers'} it but ${fitsLack.length ? `none grants ${fitsLack.map(w => w.missing).join(', ')}` : 'none grants all of these at once'}`);
    for (const [p, why] of misfits) parts.push(`${label(p)} is for it but ${why}`);
    return { lacking, why: parts.join('; ') || 'no profile covers it' };
  };
  const forApp = named
    ? assess(app.id, [named], null, { label: p => `EXACT_PROFILE (${p.name})`, reject: p => profilesFor(app.id, [p]).length ? null : `is for ${p.appId}, not ${app.id}` })
    : assess(app.id, profilesFor(app.id, installed), null);
  const like = forApp.found ?? null;
  const forExtension = assess(extension, profilesFor(extension, installed), like);
  if (like && forExtension.found) return { app: like, extension: forExtension.found };
  const method = like ? METHODS[like.method] : udid ? 'development' : 'distribution';
  const register = [[app.id, forApp], [extension, forExtension]].filter(([, choice]) => !choice.found && choice.lacking?.length)
    .map(([id, choice]) => `the App ID ${id} with ${[...new Set(choice.lacking.map(w => w.portal))].join(' and ')}`);
  throw new Error(`host/apple: a device build of an app with widgets or Live Activities embeds one ${method} profile in the app and one in its extension, of one team, App ID prefix and method, each granting the App Group and the Keychain group they share (LLP 1126.001 WP13). On this Mac${udid ? `, for this phone (${udid})` : ', for an archive'}:
  ${app.id}: ${forApp.found ? 'provisioned' : forApp.why}
  ${extension}: ${forExtension.found ? 'provisioned' : forExtension.why}
${register.length ? `In the Apple Developer portal, register ${register.join(', and ')}; then make` : 'Make'} one ${method} profile for each App ID${udid ? ' that includes this phone' : like?.devices.length ? ' naming the same devices' : ''}; install both (open the files, or Xcode ▸ Settings ▸ Accounts ▸ Download Manual Profiles). EXACT_PROFILE names the app's.`);
}
const tree = dir => readdirSync(dir, { withFileTypes: true }).flatMap(e => e.isDirectory() ? tree(resolve(dir, e.name)) : [resolve(dir, e.name)]);

/** SwiftPM owns the sources, settings, incremental objects and module. The
 * wrapper's stable path depends on that interface, never on the Rust archive. */
export function prepareWidgetCore({ app, directory, swiftBuildRoot, swiftArgs, swiftc, triple, device }) {
  const scratch = resolve(swiftBuildRoot, 'core');
  const args = swiftArgs.map(a => a === swiftBuildRoot ? scratch : a);
  const key = createHash('sha256').update(JSON.stringify([app.id, args, swiftc, triple]));
  for (const file of [resolve(pkg, 'Package.swift'), resolve(pkg, 'include/exact.h'), ...['ExactCore', 'ExactSnapshot', 'CExact'].flatMap(name => tree(resolve(pkg, 'Sources', name)))].sort()) key.update(file).update(readFileSync(file));
  const base = resolve(directory, key.digest('hex').slice(0, 20));
  return { base, scratch, args, triple, device, framework: resolve(base, 'ExactCore.framework'), xcframework: resolve(base, 'ExactCore.xcframework') };
}
export function sharedWidgetCore({ app, prepared, libDir, crate, env, runApple }) {
  const { base, args, framework, xcframework, device } = prepared;
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
    writeFileSync(resolve(framework, 'Info.plist'), plistFile({ CFBundleIdentifier: `${app.id}.runtime`, CFBundleName: 'ExactCore', CFBundleExecutable: 'ExactCore', CFBundlePackageType: 'FMWK', CFBundleVersion: '1', CFBundleShortVersionString: '0.1.0', CFBundleSupportedPlatforms: [device ? 'iPhoneOS' : 'iPhoneSimulator'], MinimumOSVersion: app.manifest.host?.ios?.minimumOS ?? '17.0' }));
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
export function packageWidgets({ app, bundle, shared, directory, sdk, triple, device, entitlements, sign, runApple, production, profile = null }) {
  const extension = resolve(bundle, 'PlugIns/ExactWidgets.appex');
  mkdirSync(extension, { recursive: true });
  // A phone installs the extension only with its own profile embedded (entryProfiles chose it).
  if (profile) copyFileSync(profile.path, resolve(extension, 'embedded.mobileprovision'));
  // The widgets' plans and a TypeScript app's module pair, which the bake wrote or the build fails; an app with
  // activities alone has no `plans` tree until its entries bake (LLP 1126.001 WP8).
  for (const [tree, expected] of [['plans', widgetEntries(app).length > 0], ['typescript', existsSync(resolve(app.dir, 'app.ts'))]]) {
    if (existsSync(resolve(bundle, tree))) cpSync(resolve(bundle, tree), resolve(extension, tree), { recursive: true });
    else if (expected) throw new Error(`host/apple: the bake wrote no ${tree} tree for the widget extension (${resolve(bundle, tree)})`);
  }
  const appInfo = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', resolve(bundle, 'Info.plist')]).stdout);
  const versions = Object.fromEntries(['CFBundleVersion', 'CFBundleShortVersionString', 'MinimumOSVersion'].map(key => [key, appInfo[key]]));
  const runtimeInfo = resolve(bundle, 'Frameworks/ExactCore.framework/Info.plist');
  const runtime = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', runtimeInfo]).stdout);
  writeFileSync(runtimeInfo, plistFile({ ...runtime, ...versions }));
  const frameworks = resolve(bundle, 'Frameworks'), reload = 'libexact_widget_reload.dylib';
  copyFileSync(widgetReload({ directory, triple, sdk, device, runApple }), resolve(frameworks, reload));
  wrapFramework(frameworks, reload, 'ExactWidgetReload', app, device ? 'iPhoneOS' : 'iPhoneSimulator');
  const entries = widgetEntries(app), names = entries.map((_, i) => `Entry${i}`);
  const human = kind => kind.replace(/[-_]+/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
  const generated = entries.map((entry, i) => `struct ${names[i]}: Widget {
    let kind = ${JSON.stringify(entry.kind)}
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: ExactTimelineProvider(kind: kind)) { entry in
            ExactWidgetView(entry: entry).environment(\\.widgetDisplayGamut, UITraitCollection.current.displayGamut)
        }
        .configurationDisplayName(${JSON.stringify(entry.title ?? human(entry.kind))}).description(${JSON.stringify(entry.description ?? human(entry.kind))})
        .supportedFamilies([${entry.families.map(f => `.${f}`).join(', ')}]).contentMarginsDisabled()
    }
}`).join('\n');
  // The one ActivityConfiguration for every activity kind (`widget/Activity.swift`, LLP 1126.001 WP10): the
  // envelope's attributes name the kind and the generation at render time, so the generated bundle holds no
  // activity string at all, and an app with activities and no widgets has it as its one member.
  const activities = activityEntries(app).length > 0;
  if (activities) names.push('ExactActivities');
  const source = resolve(directory, 'WidgetBundle.swift');
  writeFileSync(source, `import SwiftUI\nimport WidgetKit\nimport UIKit\n${generated}\n@main struct ExactWidgets: WidgetBundle { var body: some Widget { ${names.map(n => `${n}()`).join('; ')} } }\n`);
  const entitlementPath = resolve(directory, 'widget-entitlements.plist');
  writeFileSync(entitlementPath, entitlements);
  writeFileSync(resolve(extension, 'Info.plist'), plistFile({ CFBundleIdentifier: widgetId(app), CFBundleName: 'ExactWidgets', CFBundleDisplayName: app.displayName, CFBundleExecutable: 'ExactWidgets', CFBundlePackageType: 'XPC!', ...versions, UIDeviceFamily: appInfo.UIDeviceFamily,
    CFBundleSupportedPlatforms: appInfo.CFBundleSupportedPlatforms, ...(appInfo.NSAppTransportSecurity ? { NSAppTransportSecurity: appInfo.NSAppTransportSecurity } : {}), ExactAppGroup: widgetGroup(app), ExactAppIdentifier: app.id, ExactWidgetTypeScript: existsSync(resolve(app.dir, 'app.ts')), ExactKeychainAccessGroup: appInfo.ExactKeychainAccessGroup, ExactWidgetGeneration: appInfo.ExactWidgetGeneration, ...(typeof app.origin === 'string' ? { ExactLinkBase: app.origin } : {}),
    NSExtension: { NSExtensionPointIdentifier: 'com.apple.widgetkit-extension' } }));
  runApple('xcrun', ['--sdk', device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-O', '-application-extension', ...(!production ? ['-D', 'EXACT_INSPECT'] : []), '-Xlinker', '-e', '-Xlinker', '_NSExtensionMain', '-Xlinker', '-u', '-Xlinker', '_main', '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'), '-I', resolve(root, 'host/apple/Sources/CExact'), '-I', resolve(shared.framework, 'Modules'), '-F', resolve(shared.framework, '..'), '-framework', 'ExactCore',
    '-Xlinker', '-rpath', '-Xlinker', '@executable_path/../../Frameworks',
    ...(!device ? ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', entitlementPath] : []),
    shared.snapshotArchive, resolve(root, 'host/apple/widget/Timeline.swift'), ...(activities ? [resolve(root, 'host/apple/widget/Activity.swift')] : []), source, '-o', resolve(extension, 'ExactWidgets')]);
  sign(extension, entitlementPath);
}
