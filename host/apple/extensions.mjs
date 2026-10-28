// @ref LLP 1133.024 D4, D22 item 5 — app extensions of more than one kind:
// what an appex compile links with, and the share extension, which links
// no Exact runtime and writes only its inbox in the App Group.
import { resolve } from 'node:path';
import { mkdirSync, writeFileSync, copyFileSync, rmSync } from 'node:fs';
import { plistFile } from './assets.mjs';
import { appleActivationRule, appleShareMembers } from '../../scripts/share-target.mjs';
const root = resolve(new URL('../..', import.meta.url).pathname);

/** An extension executable's link: application-extension API only, entered at `NSExtensionMain`. A bundle with an
 * `@main` (WidgetKit's) keeps its `main` with `keepMain`. */
export const appexArgs = ['-application-extension', '-Xlinker', '-e', '-Xlinker', '_NSExtensionMain'];
export const keepMain = ['-Xlinker', '-u', '-Xlinker', '_main'];

export const shareId = app => `${app.id}.share`;

/** The app's Info.plist keys for the claim (`ShareReceive.swift`): its `share_target`, and the group its inbox
 * is in when the extension was built (no group, no inbox). */
export const shareInfo = (app, group = null) => app.manifest.share_target
  ? { ExactShareTarget: JSON.stringify(app.manifest.share_target), ExactAppIdentifier: app.id, ...(group ? { ExactShareGroup: group } : {}) }
  : {};

/** The share extension (D4) in `bundle`'s `PlugIns/ExactShare.appex`: the host's one Swift extension, parameterized
 * by its Info.plist (the group, the app's id, the mapped members, each files member's UTIs, the activation rule and
 * the post button's word). `entitlements` is the extension's plist text; `sign(extension, path)` signs it. */
export function packageShareExtension({ app, bundle, directory, sdk, triple, device = false, platform = 'ios', group, entitlements, sign, runApple, profile = null }) {
  const mac = platform === 'macos', contents = mac ? resolve(bundle, 'Contents') : bundle;
  const target = app.manifest.share_target;
  const extension = resolve(contents, 'PlugIns/ExactShare.appex');
  rmSync(extension, { recursive: true, force: true });
  const extensionContents = mac ? resolve(extension, 'Contents') : extension;
  const binary = mac ? resolve(extensionContents, 'MacOS/ExactShare') : resolve(extension, 'ExactShare');
  mkdirSync(mac ? resolve(extensionContents, 'MacOS') : extension, { recursive: true });
  if (profile) copyFileSync(profile.path, resolve(extensionContents, mac ? 'embedded.provisionprofile' : 'embedded.mobileprovision'));
  const appInfo = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', resolve(contents, 'Info.plist')]).stdout);
  const versions = Object.fromEntries(['CFBundleVersion', 'CFBundleShortVersionString', mac ? 'LSMinimumSystemVersion' : 'MinimumOSVersion'].map(key => [key, appInfo[key]]).filter(([, v]) => v !== undefined));
  const params = target.params ?? {};
  writeFileSync(resolve(extensionContents, 'Info.plist'), plistFile({
    CFBundleIdentifier: shareId(app), CFBundleName: 'ExactShare', CFBundleDisplayName: appInfo.CFBundleDisplayName ?? appInfo.CFBundleName ?? app.displayName,
    CFBundleExecutable: 'ExactShare', CFBundlePackageType: 'XPC!', CFBundleInfoDictionaryVersion: '6.0', ...versions,
    ...(!mac ? { UIDeviceFamily: appInfo.UIDeviceFamily } : {}), CFBundleSupportedPlatforms: mac ? ['MacOSX'] : appInfo.CFBundleSupportedPlatforms,
    ExactAppGroup: group, ExactAppIdentifier: app.id, ExactShareParams: ['title', 'text', 'url'].filter(k => params[k]), ExactShareMembers: appleShareMembers(target),
    ...(target.apple?.postTitle ? { ExactSharePostTitle: target.apple.postTitle } : {}),
    NSExtension: { NSExtensionPointIdentifier: 'com.apple.share-services', NSExtensionPrincipalClass: 'ExactShareViewController',
      NSExtensionAttributes: { NSExtensionActivationRule: appleActivationRule(target) } },
  }));
  mkdirSync(directory, { recursive: true });
  const entitlementPath = resolve(directory, 'share-entitlements.plist');
  writeFileSync(entitlementPath, entitlements);
  runApple('xcrun', ['--sdk', mac ? 'macosx' : device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-O', ...appexArgs,
    '-module-name', 'ExactShare', '-target', triple, '-sdk', sdk, '-module-cache-path', resolve(directory, 'cache'),
    ...(!device && !mac ? ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', entitlementPath] : []),
    resolve(root, 'host/apple/share/ShareInbox.swift'), resolve(root, 'host/apple/share/ShareViewController.swift'), '-o', binary]);
  sign(extension, entitlementPath);
  return extension;
}

/** The Mac extension's entitlements: sandboxed, as every Mac app extension is, with the app's group. */
export const macShareEntitlements = group => plistFile({ 'com.apple.security.app-sandbox': true, 'com.apple.security.application-groups': [group] });
