// @ref LLP 1126 D2/D8 — one runtime framework, one generated widget bundle.
import { resolve } from 'node:path';
import { mkdirSync, readdirSync, existsSync, writeFileSync, readFileSync, copyFileSync, cpSync, rmSync, renameSync } from 'node:fs';
import { createHash, X509Certificate } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { plistFile, wrapFramework, frameworkLayout, frameworkInstallName } from './assets.mjs';
import { assertCapabilityFrameworks } from './link.mjs';
import { linkedPlanArgs } from '../../scripts/linked-plan.mjs';
import { appexArgs, keepMain } from './extensions.mjs';
import { installedProfiles, decodeProfile, profilesFor, profileMisfit, byExpiry, METHODS, iosProfile } from './devices.mjs';
const root = resolve(new URL('../..', import.meta.url).pathname), pkg = resolve(root, 'host/apple');
export const accessoryFamilies = ['accessoryCircular', 'accessoryRectangular', 'accessoryInline', 'accessoryCorner'];
export function platformWidgetEntries(app, platform = 'ios') {
  return widgetEntries(app).flatMap(entry => {
    const families = (platform === 'watchos' ? entry.watchFamilies ?? entry.families : entry.families).filter(family => platform === 'watchos' ? accessoryFamilies.includes(family) : platform === 'macos' ? ['systemSmall', 'systemMedium', 'systemLarge', 'systemExtraLarge'].includes(family) : family !== 'accessoryCorner');
    return families.length ? [{ ...entry, families }] : [];
  });
}
export const widgetEntries = app => app.manifest.widgets ?? [];
export const activityEntries = app => app.manifest.live_activities ?? [];
/** Whether the app has an entry of either kind: what the extension, the shared framework, the App Group and the
 * Keychain group are gated on (LLP 1126.001 WP13). */
export const hasEntries = app => platformWidgetEntries(app).length > 0 || activityEntries(app).length > 0;
export const widgetGroup = app => `group.${app.widgetAppId ?? app.clipParent ?? app.id}`;
export const widgetId = app => `${app.id}.widgets`;

// LLP 1126.001 WP16: the Mac container is authorized by the signing team,
// independently of iOS's registered group and of restricted Keychain access.
export const macWidgetGroup = (app, team) => `${team}.${app.id}.widgets`;
export function widgetSigningTeam(identity, { distribution = false, certificates = null } = {}) {
  certificates ??= (spawnSync('security', ['find-certificate', '-a', '-p'], { encoding: 'utf8' }).stdout
    ?.match(/-----BEGIN CERTIFICATE-----[\s\S]*?-----END CERTIFICATE-----/g) ?? []).map(pem => new X509Certificate(pem));
  const kind = distribution ? 'Developer ID Application' : 'Apple Development';
  for (const cert of certificates) {
    const cn = /^CN=(.*)$/m.exec(cert.subject)?.[1];
    if (cert.fingerprint.replaceAll(':', '') === identity?.toUpperCase() || cn === identity) {
      if (!distribution && cn?.startsWith('Developer ID Application: ')) return null;
      if (!cn?.startsWith(kind + ': ')) throw new Error(`host/apple: Mac widgets need an ${kind} certificate for this build; EXACT_IDENTITY names ${cn}`);
      const team = /^OU=([A-Z0-9]{10})$/m.exec(cert.subject)?.[1];
      if (team) return team;
    }
  }
  throw new Error(`host/apple: Mac widgets need an ${kind} identity; name its SHA-1 or certificate name with EXACT_IDENTITY (security find-identity -v -p codesigning)`);
}
/** The Provisioning UDID is distinct from the hardware UUID on Apple silicon. */
export function macHardwareID(read = (command, args) => spawnSync(command, args, { encoding: 'utf8' }).stdout ?? '') {
  try {
    const hardware = JSON.parse(read('system_profiler', ['SPHardwareDataType', '-json']));
    const id = hardware.SPHardwareDataType?.find(row => row.provisioning_UDID)?.provisioning_UDID;
    if (id) return id;
  } catch { /* Older system_profiler versions may supply only IOPlatformUUID. */ }
  return /"IOPlatformUUID"\s*=\s*"([^"]+)"/.exec(read('ioreg', ['-rd1', '-c', 'IOPlatformExpertDevice']))?.[1] ?? null;
}
export function macWidgetProfiles({ app, team, distribution = false, installed = installedProfiles(), named = process.env.EXACT_PROFILE ? decodeProfile(process.env.EXACT_PROFILE) : null, udid, hardwareID = macHardwareID, log = console.log }) {
  if (named && !named.platforms?.includes('OSX')) {
    log(`Mac widgets: ignoring EXACT_PROFILE (${named.name}) for ${named.platforms?.join(', ') || 'another platform'}; shared secrets need macOS profiles (LLP 1126.001 §5 Q5).`);
    named = null;
  }
  if (udid === undefined) udid = !distribution && installed.some(p => p.platforms?.includes('OSX') && p.team === team) ? hardwareID() : null;
  const group = macWidgetGroup(app, team);
  const fits = (p, id) => p.platforms?.includes('OSX') && p.team === team && p.dev === !distribution && !profileMisfit(p, udid)
    && profilesFor(id, [p]).length && (p.groups.includes(group) || p.groups.includes(`${team}.*`))
    && (p.keychain.includes(`${p.prefix}.${app.id}`) || p.keychain.includes(`${p.prefix}.*`));
  for (const owner of (named ? [named] : installed).filter(p => fits(p, app.id)).sort(byExpiry)) {
    const extension = installed.filter(p => fits(p, widgetId(app)) && !profileMisfit(p, udid, owner)).sort(byExpiry)[0];
    if (extension) return { app: owner, extension, keychain: `${owner.prefix}.${app.id}` };
  }
  if (named) throw new Error(`host/apple: EXACT_PROFILE (${named.name}) needs a matching macOS profile for ${app.id} and ${widgetId(app)}, of team ${team}, granting ${group} (or ${team}.*) and the shared Keychain group <prefix>.${app.id}`);
  return null;
}
/** Decide before compilation. A machine without a signing identity still builds the ordinary app. */
export function macWidgetSigning(app, identity, { distribution = false, log = console.log, certificates, profiles } = {}) {
  if (!platformWidgetEntries(app, 'macos').length) return null;
  const team = identity && identity !== '-' ? widgetSigningTeam(identity, { distribution, certificates }) : null;
  if (!team) {
    const reason = identity && identity !== '-' ? 'EXACT_IDENTITY names a Developer ID identity' : 'no signing identity';
    log(`Mac widgets: ${reason}; building the app ad hoc without an extension. Set EXACT_IDENTITY to ${distribution ? 'a Developer ID Application' : 'an Apple Development'} identity to include widgets.`);
    return null;
  }
  return { team, profiles: macWidgetProfiles({ app, team, distribution, log, ...profiles }) };
}
export function macWidgetEntitlements({ app, team, profiles = null, extension = false }) {
  const id = extension ? widgetId(app) : app.id;
  return plistFile({
    'com.apple.security.application-groups': [macWidgetGroup(app, team)],
    ...(extension ? { 'com.apple.security.app-sandbox': true, 'com.apple.security.network.client': true } : {}),
    ...(profiles ? { 'com.apple.application-identifier': `${profiles.app.prefix}.${id}`, 'com.apple.developer.team-identifier': team, 'keychain-access-groups': [profiles.keychain] } : {}),
  });
}

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
  // installedProfiles also finds native Mac profiles; they cannot provision iOS.
  installed = installed.filter(iosProfile);
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
    ? assess(app.id, [named], null, { label: p => `EXACT_PROFILE (${p.name})`, reject: p => !iosProfile(p) ? 'is not an iOS profile' : profilesFor(app.id, [p]).length ? null : `is for ${p.appId}, not ${app.id}` })
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
export function prepareWidgetCore({ app, directory, swiftBuildRoot, swiftArgs, swiftc, triple, device, platform = 'ios' }) {
  const scratch = resolve(swiftBuildRoot, 'core');
  const args = swiftArgs.map(a => a === swiftBuildRoot ? scratch : a);
  const key = createHash('sha256').update(JSON.stringify([app.id, args, swiftc, triple, 'versioned-mac-framework']));
  for (const file of [resolve(pkg, 'Package.swift'), resolve(pkg, 'include/exact.h'), ...['ExactCore', 'ExactSnapshot', 'CExact'].flatMap(name => tree(resolve(pkg, 'Sources', name)))].sort()) key.update(file).update(readFileSync(file));
  const base = resolve(directory, key.digest('hex').slice(0, 20));
  return { base, scratch, args, triple, device, platform, framework: resolve(base, 'ExactCore.framework'), xcframework: resolve(base, 'ExactCore.xcframework') };
}
export function sharedWidgetCore({ app, prepared, libDir, crate, env, runApple, linkedPlan = null }) {
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
  const digest = createHash('sha256').update(readFileSync(archive)).update(linkedPlan?.digest ?? '').digest('hex');
  const stamp = resolve(base, 'archive');
  // SwiftPM tracks neither an external archive nor the linked plan. Invalidate only its link;
  // the objects/module stay incremental, including on a kernel-only edit.
  if (!existsSync(stamp) || readFileSync(stamp, 'utf8') !== digest) rmSync(dylib, { force: true });
  runApple('swift', [...args, ...linkedPlanArgs(linkedPlan?.dir), '--product', 'ExactCore'], options);
  // Only the extension links the snapshot target. Keeping it out of the shared
  // runtime keeps SwiftUI and WidgetKit out of an ordinary app launch.
  runApple('swift', [...args, ...linkedPlanArgs(linkedPlan?.dir), '--product', 'ExactSnapshot'], options);
  const changed = !existsSync(stamp) || readFileSync(stamp, 'utf8') !== digest || !existsSync(resolve(framework, 'ExactCore'));
  if (changed) {
    const layout = frameworkLayout(framework, 'ExactCore', platform === 'macos' ? 'MacOSX' : 'iPhoneOS');
    const temporary = resolve(base, 'ExactCore.tmp');
    copyFileSync(dylib, temporary);
    runApple('xcrun', ['install_name_tool', '-id', frameworkInstallName('ExactCore', platform === 'macos' ? 'MacOSX' : 'iPhoneOS'), temporary]);
    renameSync(temporary, layout.binary);
    for (const name of ['ExactCore', 'ExactSnapshot']) cpSync(resolve(bin, `${name}.swiftmodule`), resolve(layout.modules, `${name}.swiftmodule`), { recursive: true, preserveTimestamps: true });
    writeFileSync(layout.info, plistFile({ CFBundleIdentifier: `${app.id}.runtime`, CFBundleName: 'ExactCore', CFBundleExecutable: 'ExactCore', CFBundlePackageType: 'FMWK', CFBundleVersion: '1', CFBundleShortVersionString: '0.1.0', CFBundleSupportedPlatforms: [platform === 'macos' ? 'MacOSX' : platform === 'watchos' ? 'WatchSimulator' : device ? 'iPhoneOS' : 'iPhoneSimulator'], ...(platform === 'macos' ? { LSMinimumSystemVersion: app.manifest.host?.macos?.minimumOS ?? '14.0' } : { MinimumOSVersion: platform === 'watchos' ? '10.0' : app.manifest.host?.ios?.minimumOS ?? '17.0' }) }));
    if (!existsSync(resolve(xcframework, 'Info.plist'))) runApple('xcrun', ['xcodebuild', '-create-xcframework', '-allow-internal-distribution', '-framework', framework, '-output', xcframework]);
    else {
      const slice = readdirSync(xcframework).find(name => existsSync(resolve(xcframework, name, 'ExactCore.framework')));
      cpSync(framework, resolve(xcframework, slice, 'ExactCore.framework'), { recursive: true, verbatimSymlinks: true });
    }
    writeFileSync(stamp, digest);
  }
  assertCapabilityFrameworks(resolve(framework, 'ExactCore'), []);
  return { ...prepared, snapshotArchive: resolve(bin, 'libExactSnapshot.a') };
}
/** The reload artifact has no app-specific inputs. Reuse it until its source or target changes. */
export function widgetReload({ directory, triple, sdk, device, platform = 'ios', production = false, runApple, source = resolve(pkg, 'Sources/ExactCore/WidgetContainer.swift') }) {
  const digest = createHash('sha256').update(readFileSync(source)).update(JSON.stringify([triple, sdk, production])).digest('hex');
  const binary = resolve(directory, 'libexact_widget_reload.dylib'), stamp = resolve(directory, 'widget-reload.sha256');
  if (existsSync(binary) && existsSync(stamp) && readFileSync(stamp, 'utf8') === digest) return binary;
  mkdirSync(directory, { recursive: true });
  runApple('xcrun', ['--sdk', platform === 'macos' ? 'macosx' : device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-emit-library', '-O',
    '-D', 'EXACT_WIDGET_RELOAD', ...(!production ? ['-D', 'EXACT_INSPECT'] : []), '-module-name', 'ExactWidgetReload', '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'), source, '-o', binary + '.tmp']);
  renameSync(binary + '.tmp', binary);
  writeFileSync(stamp, digest);
  return binary;
}
export function packageWidgets({ app, bundle, shared, directory, sdk, triple, device, entitlements, sign, runApple, production, profile = null, platform = 'ios', entries = platformWidgetEntries(app, platform), alarm = null }) {
  const mac = platform === 'macos', contents = mac ? resolve(bundle, 'Contents') : bundle;
  const resources = mac ? resolve(contents, 'Resources') : contents;
  const extension = resolve(contents, 'PlugIns/ExactWidgets.appex');
  const extensionContents = mac ? resolve(extension, 'Contents') : extension;
  const extensionResources = mac ? resolve(extensionContents, 'Resources') : extension;
  const extensionBinary = mac ? resolve(extensionContents, 'MacOS/ExactWidgets') : resolve(extension, 'ExactWidgets');
  mkdirSync(extensionResources, { recursive: true });
  if (mac) mkdirSync(resolve(extensionContents, 'MacOS'));
  // A phone installs the extension only with its own profile embedded (entryProfiles chose it).
  if (profile) copyFileSync(profile.path, resolve(extensionContents, mac ? 'embedded.provisionprofile' : 'embedded.mobileprovision'));
  // The widgets' plans and a TypeScript app's module pair, which the bake wrote or the build fails; an app with
  // activities alone has no `plans` tree until its entries bake (LLP 1126.001 WP8).
  for (const [tree, expected] of [['plans', entries.length > 0], ['typescript', platform !== 'watchos' && existsSync(resolve(app.dir, 'app.ts'))]]) {
    if (existsSync(resolve(resources, tree))) cpSync(resolve(resources, tree), resolve(extensionResources, tree), { recursive: true });
    else if (expected) throw new Error(`host/apple: the bake wrote no ${tree} tree for the widget extension (${resolve(resources, tree)})`);
  }
  const appInfo = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', resolve(contents, 'Info.plist')]).stdout);
  const versions = Object.fromEntries(['CFBundleVersion', 'CFBundleShortVersionString', mac ? 'LSMinimumSystemVersion' : 'MinimumOSVersion'].map(key => [key, appInfo[key]]));
  const runtimeInfo = resolve(contents, 'Frameworks/ExactCore.framework', mac ? 'Resources/Info.plist' : 'Info.plist');
  const runtime = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', runtimeInfo]).stdout);
  writeFileSync(runtimeInfo, plistFile({ ...runtime, ...versions }));
  const frameworks = resolve(contents, 'Frameworks'), reload = 'libexact_widget_reload.dylib';
  if (platform !== 'watchos') {
    copyFileSync(widgetReload({ directory, triple, sdk, device, platform, production, runApple }), resolve(frameworks, reload));
    wrapFramework(frameworks, reload, 'ExactWidgetReload', app, mac ? 'MacOSX' : device ? 'iPhoneOS' : 'iPhoneSimulator');
  }
  const names = entries.map((_, i) => `Entry${i}`);
  const human = kind => kind.replace(/[-_]+/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
  const generated = entries.map((entry, i) => `struct ${names[i]}: Widget {
    let kind = ${JSON.stringify(entry.kind)}
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: ExactTimelineProvider(kind: kind)) { entry in
            ExactWidgetView(entry: entry)${platform === 'ios' ? '.environment(\\.widgetDisplayGamut, UITraitCollection.current.displayGamut == .P3 ? \"p3\" : \"srgb\")' : mac ? '.environment(\\.widgetDisplayGamut, NSScreen.main?.colorSpace?.cgColorSpace?.isWideGamutRGB == true ? "p3" : "srgb")' : ''}
        }
        .configurationDisplayName(${JSON.stringify(entry.title ?? human(entry.kind))}).description(${JSON.stringify(entry.description ?? human(entry.kind))})
        .supportedFamilies([${entry.families.map(f => `.${f}`).join(', ')}]).contentMarginsDisabled()
    }
}`).join('\n');
  // The one ActivityConfiguration for every activity kind (`widget/Activity.swift`, LLP 1126.001 WP10): the
  // envelope's attributes name the kind and the generation at render time, so the generated bundle holds no
  // activity string at all, and an app with activities and no widgets has it as its one member.
  // ActivityKit exists on iOS alone: the watch's and the Mac's bundles have no activities.
  const activities = platform === 'ios' && activityEntries(app).length > 0;
  if (activities) names.push('ExactActivities');
  const source = resolve(directory, 'WidgetBundle.swift');
  // An alarm's countdown beside them, from iOS 26 (LLP 1133.030 D8 item 2).
  const alarmMember = alarm && platform === 'ios' ? '; if #available(iOS 26.0, *) { ExactAlarmCountdown() }' : '';
  writeFileSync(source, `import SwiftUI\nimport WidgetKit\n${platform === 'ios' ? 'import UIKit\n' : mac ? 'import AppKit\n' : ''}${generated}\n@main struct ExactWidgets: WidgetBundle { var body: some Widget { ${names.map(n => `${n}()`).join('; ')}${alarmMember} } }\n`);
  const entitlementPath = resolve(directory, 'widget-entitlements.plist');
  writeFileSync(entitlementPath, entitlements);
  writeFileSync(resolve(extensionContents, 'Info.plist'), plistFile({ CFBundleIdentifier: widgetId(app), CFBundleName: 'ExactWidgets', CFBundleDisplayName: app.displayName, CFBundleExecutable: 'ExactWidgets', CFBundlePackageType: 'XPC!', ...versions, ...(!mac ? { UIDeviceFamily: appInfo.UIDeviceFamily } : {}),
    CFBundleSupportedPlatforms: mac ? ['MacOSX'] : appInfo.CFBundleSupportedPlatforms, ...(appInfo.DTPlatformName ? { DTPlatformName: appInfo.DTPlatformName } : {}), ...(appInfo.NSAppTransportSecurity ? { NSAppTransportSecurity: appInfo.NSAppTransportSecurity } : {}), ExactAppGroup: appInfo.ExactAppGroup ?? widgetGroup(app), ExactAppIdentifier: appInfo.ExactAppIdentifier ?? app.id, ExactWidgetTypeScript: platform !== 'watchos' && existsSync(resolve(app.dir, 'app.ts')), ...(appInfo.ExactKeychainAccessGroup ? { ExactKeychainAccessGroup: appInfo.ExactKeychainAccessGroup } : {}), ...(mac ? { ExactWidgetSecretsAvailable: !!appInfo.ExactKeychainAccessGroup, ExactWidgetRequiresSecrets: appInfo.ExactWidgetRequiresSecrets ?? [] } : {}), ExactWidgetGeneration: appInfo.ExactWidgetGeneration, ...(typeof app.origin === 'string' ? { ExactLinkBase: app.origin } : {}),
    NSExtension: { NSExtensionPointIdentifier: 'com.apple.widgetkit-extension' } }));
  runApple('xcrun', ['--sdk', mac ? 'macosx' : platform === 'watchos' ? 'watchsimulator' : device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-O', ...appexArgs, ...keepMain, ...(!production ? ['-D', 'EXACT_INSPECT'] : []), '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'), '-I', resolve(root, 'host/apple/Sources/CExact'), '-I', resolve(shared.framework, 'Modules'), '-F', resolve(shared.framework, '..'), '-framework', 'ExactCore',
    '-Xlinker', '-rpath', '-Xlinker', mac ? '@executable_path/../../../../Frameworks' : '@executable_path/../../Frameworks',
    ...(!device && !mac ? ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', entitlementPath] : []),
    ...(alarmMember ? alarmArgs(alarm) : []),
    shared.snapshotArchive, resolve(root, 'host/apple/widget/Timeline.swift'), resolve(root, 'host/apple/widget/WidgetTimelineWaiters.swift'), ...(activities ? [resolve(root, 'host/apple/widget/Activity.swift')] : []), source, '-o', extensionBinary]);
  sign(extension, entitlementPath);
}

/** What compiles an alarm's countdown into a widget extension (LLP 1133.030 D8 items 1 and 2): the host's two files,
 * AlarmKit weak-linked (the app's minimum is below iOS 26), and the module named as the package's, which compiles the
 * same `ExactAlarmMetadata` into the app: ActivityKit names an attributes type by its module and type name, as
 * `activities.mjs` keeps the envelope's in `ExactSnapshot` on both sides. */
const alarmArgs = module => ['-module-name', module, '-Xlinker', '-weak_framework', '-Xlinker', 'AlarmKit',
  resolve(root, 'host/apple/widget/Alarm.swift'), resolve(root, 'host/apple/widget/AlarmAttributes.swift')];

/** The package that asks the host for AlarmKit's countdown (`"apple": {"activity": "alarmkit"}`, LLP 1133.030 D12
 * item 3), by its Swift module's name; one at most, since the extension's module takes that name. */
export function alarmActivity(packages) {
  const asking = packages.filter(p => p.activity === 'alarmkit');
  if (asking.length > 1) throw new Error(`host/apple: ${asking.map(p => p.extension).join(' and ')} each ask for AlarmKit's countdown; one package may (LLP 1133.030 D12 item 3)`);
  return asking[0]?.module ?? null;
}

/** An app with no widgets and no Live Activities whose package asks for AlarmKit's countdown gets an extension that
 * holds the countdown alone (LLP 1133.030 D8 item 2): no ExactCore, no snapshot archive, no app group and no cohort,
 * so nothing of the app's state is reachable from it. */
export function packageAlarmExtension({ app, bundle, directory, sdk, triple, device, entitlements, sign, runApple, profile = null, alarm }) {
  const extension = resolve(bundle, 'PlugIns/ExactWidgets.appex');
  mkdirSync(extension, { recursive: true });
  if (profile) copyFileSync(profile.path, resolve(extension, 'embedded.mobileprovision'));
  const appInfo = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', resolve(bundle, 'Info.plist')]).stdout);
  const source = resolve(directory, 'AlarmWidgetBundle.swift');
  writeFileSync(source, 'import SwiftUI\nimport WidgetKit\n@main struct ExactWidgets: WidgetBundle { var body: some Widget { if #available(iOS 26.0, *) { ExactAlarmCountdown() } } }\n');
  const entitlementPath = resolve(directory, 'alarm-widget-entitlements.plist');
  writeFileSync(entitlementPath, entitlements);
  writeFileSync(resolve(extension, 'Info.plist'), plistFile({ CFBundleIdentifier: widgetId(app), CFBundleName: 'ExactWidgets', CFBundleDisplayName: app.displayName, CFBundleExecutable: 'ExactWidgets', CFBundlePackageType: 'XPC!',
    CFBundleVersion: appInfo.CFBundleVersion, CFBundleShortVersionString: appInfo.CFBundleShortVersionString, MinimumOSVersion: appInfo.MinimumOSVersion, UIDeviceFamily: appInfo.UIDeviceFamily,
    CFBundleSupportedPlatforms: appInfo.CFBundleSupportedPlatforms, NSExtension: { NSExtensionPointIdentifier: 'com.apple.widgetkit-extension' } }));
  runApple('xcrun', ['--sdk', device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-O', ...appexArgs, ...keepMain, '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'), ...(!device ? ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', entitlementPath] : []),
    ...alarmArgs(alarm), source, '-o', resolve(extension, 'ExactWidgets')]);
  sign(extension, entitlementPath);
}
