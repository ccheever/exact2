#!/usr/bin/env bun
// Build the macOS app — or, with --ios, the iOS app: the app's static
// library (cargo, release), then the presenter (swift build) linked against
// it. Usage:
//   bun host/apple/build.mjs [crate=caltrain-apple] [--run]                 macOS
//   bun host/apple/build.mjs [crate] --test                                  the Swift host tests
//   bun host/apple/build.mjs [crate] --test --ios [--sim <udid|name>]        the UIKit ones (*IOSTests) on a simulator
//   bun host/apple/build.mjs --ios [crate] [--run] [--sim <udid|name>]        iOS, on a simulator
//   bun host/apple/build.mjs --tvos [crate] [--run] [--sim <udid|name>]       tvOS, on an Apple TV simulator
//   bun host/apple/build.mjs --device [crate] [--run] [--phone <udid|name>]   iOS, on a phone
//   bun host/apple/build.mjs --device [crate] --archive <out.ipa>            iOS, an .ipa to distribute
//   bun host/apple/build.mjs --device [crate] --archive <out.ipa> --unsigned an .ipa a service re-signs
// Add --url <http(s) app URL> to connect any of these clients to the same
// address as the browser (LLP 1030.000 §7): with --run it is the launch
// locator, and a development client (--bundle, --ios, --device) registers
// its opening link for that server's origin alone, with a per-build token.
// --ios builds the same archive for the simulator's Rust target, the UIKit
// presenter for the simulator triple, assembles the .app here (its
// Info.plist written, never committed), installs it on a simulator — --sim
// or EXACT_SIM names one; else a booted iPhone; else the iPhone Pro on the
// newest iOS — and with --run shows it in Simulator.app. --device is the
// same for a phone: the aarch64-apple-ios target and the iphoneos SDK, the
// bundle signed with a development identity and a provisioning profile on
// this Mac that covers the phone and the bundle id (EXACT_IDENTITY, a
// SHA-1, and EXACT_PROFILE, a path, override the automatic choice), then
// devicectl to install and, with --run, launch — the phone connected,
// unlocked, paired, Developer Mode on. --archive is the same device build
// with no phone: it signs with EXACT_IDENTITY and EXACT_PROFILE (both
// required: an ad hoc or App Store profile and its distribution identity,
// as a build service such as EAS supplies them), takes get-task-allow from
// that profile, and writes the signed bundle as an .ipa instead of
// installing it. With --unsigned it needs neither: the bundle is ad-hoc
// signed with no profile and no team, for a consumer that re-signs it with
// its own (AppDrop, a store's resigner). The simulator and phone helpers live
// in devices.mjs, shared with scripts/agent.mjs, which launches the same bundle.
// These developer builds explicitly allow unsigned updates. Set
// EXACT_UPDATE_TRUST=production for a signed-update-only artifact.
import { spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { homedir, tmpdir } from 'node:os';
import { basename, dirname, isAbsolute, resolve } from 'node:path';
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { checkModuleRoster, copyShaders, appleCargoClaims, cargoLibraryTarget, claimBuildOutput, appSourceKey, bakeOutput, buildBake, contractLast, bakeTarget, developmentBuildEnv, developmentURLScheme, gpuModules, hermesIos, resolveApp, verifyBakeFiles } from '../../scripts/app.mjs';
import { copyStaticTreeIfPresent, listAssets } from '../web/serve.mjs';
import { writeDataKeys } from './data-keys.mjs';
import { deviceLaunchArgs, developmentLaunchEnvironment, identity, macIdentity, phone, profile, showSimulator, simulator, simulators } from './devices.mjs';

const root = resolve(new URL('../..', import.meta.url).pathname);
const run = (cmd, args, opts = {}) => {
  const r = spawnSync(cmd, args, { cwd: root, ...opts,
    stdio: opts.stdio === 'ignore' ? ['ignore', 'ignore', 'pipe'] : opts.stdio ?? 'inherit' });
  if (r.status !== 0) throw new Error(`${cmd} failed (${r.status ?? r.error?.message})${r.stderr?.length ? ': ' + String(r.stderr).trim() : ''}`);
  return r;
};
const read = (cmd, args, opts = {}) => spawnSync(cmd, args, { cwd: root, encoding: 'utf8', ...opts });
// Every Apple toolchain invocation goes through here — cargo, swift build, and
// the webarm swiftc alike: a mixed deployment target or an incompatible sysroot
// is a warning the toolchain prints and then links anyway, so the build fails on
// it here instead. @ref LLP 1008
const runApple = (cmd, args, opts = {}) => {
  const { cargoMessages = false, ...spawnOptions } = opts;
  const r = spawnSync(cmd, args, { cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, ...spawnOptions });
  if (!cargoMessages) process.stdout.write(r.stdout ?? '');
  process.stderr.write(r.stderr ?? '');
  if (r.status !== 0) throw new Error(`${cmd} failed (${r.status ?? r.error?.message})`);
  const output = `${r.stdout ?? ''}\n${r.stderr ?? ''}`;
  if (/object file .* was built for newer|using sysroot for|incompatible.*sysroot/i.test(output)) {
    console.error(`host/apple: refused mixed Apple deployment targets from ${cmd} ${args[0] ?? ''}`);
    throw new Error('Apple deployment target mismatch');
  }
  return r;
};

// ---------------------------------------------------------------- iOS: the bundle and the simulator

/** The app's bundle identifier: the manifest's `app.id` (LLP 1030 D2 — derived once, in `scripts/app.mjs`), which was `com.exact.<crate>` before the manifest existed and still is for an app without one. */
export const bundleId = (crate = 'caltrain-apple') => resolveApp(crate).id;
/** The one Swift package (LLP 1031 D6): ExactKit and the four executables. */
export const pkg = resolve(root, 'host/apple');
/** The simulator's Rust target and Swift triple on this machine. */
export const iosTarget = process.arch === 'arm64' ? 'aarch64-apple-ios-sim' : 'x86_64-apple-ios';
export const iosTriple = `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-ios17.0-simulator`;
/** The Apple TV simulator's Rust target (arm64 only: Rust ships no x86_64-apple-tvos std). */
export const tvosTarget = 'aarch64-apple-tvos-sim';
const newer = (a, b) => (a.split('.').map(Number).reduce((x, n, i) => x || n - (b.split('.').map(Number)[i] ?? 0), 0) > 0 ? a : b);
/** The deployment targets an app builds for: its manifest's `minimumOS`, never
 * below the host's own floor (Package.swift's). Every Rust, Swift and linker
 * step of one build uses the same pair, so an app's own native code may target
 * the newer OS it asked for without a mixed-target refusal. */
export function deploymentTargets(app) {
  return {
    ios: newer(String(app.manifest.host?.ios?.minimumOS ?? '17.0'), '17.0'),
    macos: newer(String(app.manifest.host?.macos?.minimumOS ?? '14.0'), '14.0'),
  };
}
/** `host/apple/metal/SvgFilter.metal` compiled for `sdkName` into `out`
 *  (`ExactSvgFilter.metallib`, @ref LLP 1055.000 D14): a filter picture's
 *  kernels, compiled here and not at run time, where the first launch after
 *  an install paid 113–264 ms inside the commit that shows it. The host
 *  tests need Xcode's Metal toolchain for it (`required`); an app built
 *  without it gets `null`, and its filter pictures draw with Core Image. */
export function svgFilterLibrary(sdkName, minimum, out, required = false) {
  const install = 'xcodebuild -downloadComponent MetalToolchain', missing = read('xcrun', ['-sdk', sdkName, 'metal', '--version']).status !== 0;
  if (missing && required) throw new Error(`host/apple: this build compiles the SVG filter kernels with Xcode's Metal toolchain, which is not installed. Install it with \`${install}\``);
  if (missing) return console.warn(`host/apple: no Metal toolchain, so no ${svgFilterLibraryName}: SVG filter pictures draw with Core Image. Install it with \`${install}\`.`), null;
  const flag = { iphoneos: `-mios-version-min=${minimum}`, iphonesimulator: `-mios-simulator-version-min=${minimum}`, appletvsimulator: `-mtvos-simulator-version-min=${minimum}`, macosx: `-mmacosx-version-min=${minimum}` }[sdkName];
  const air = out.replace(/\.metallib$/, '') + '.air';
  run('xcrun', ['-sdk', sdkName, 'metal', '-std=metal3.0', flag, '-c', resolve(root, 'host/apple/metal/SvgFilter.metal'), '-o', air], { stdio: 'pipe' });
  run('xcrun', ['-sdk', sdkName, 'metallib', air, '-o', out], { stdio: 'pipe' });
  rmSync(air, { force: true });
  return out;
}
export const svgFilterLibraryName = 'ExactSvgFilter.metallib';

/** The Swift triple for an app's iOS build. */
export const iosTripleFor = (app, device, tv = false) =>
  tv ? `arm64-apple-tvos${deploymentTargets(app).ios}-simulator` : device ? `arm64-apple-ios${deploymentTargets(app).ios}` : `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-ios${deploymentTargets(app).ios}-simulator`;
export const macTriple = `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macosx`;

/** App-owned Apple paths, shared by builder and launchers. @ref LLP 1036.000 §2 */
export function appleArtifacts(app, { destination = 'macos', composition, trust = process.env.EXACT_UPDATE_TRUST ?? 'development', host = false } = {}) {
  if (!['macos', 'ios-simulator', 'ios', 'tvos-simulator'].includes(destination)) throw new Error(`unknown Apple destination ${destination}`);
  const platform = destination === 'macos' ? 'macos' : 'ios';
  composition ??= app.manifest.deploy?.store?.[platform] === '0' ? 'embedded' : 'updating';
  if (!['embedded', 'updating'].includes(composition) || !['development', 'production'].includes(trust)) throw new Error('invalid Apple composition or trust policy');
  const target = destination === 'macos' ? bakeTarget('macos') : destination === 'ios' ? 'aarch64-apple-ios' : destination === 'tvos-simulator' ? tvosTarget : iosTarget;
  const owner = resolve(app.target, 'clients', appSourceKey(app), app.id);
  const namespace = resolve(owner, destination, target, composition, trust);
  const product = destination === 'macos' ? (host ? 'ExactHostMac' : 'ExactMac') : (host ? 'ExactHostIOS' : 'ExactIOS');
  const products = resolve(namespace, host ? 'host' : 'standalone');
  return { owner, namespace, target, product, products, scratch: resolve(namespace, 'swift'),
    lock: resolve(owner, '.apple-build.lock'), capture: resolve(namespace, 'capture'),
    binary: resolve(products, product), bundle: destination === 'macos'
      ? resolve(owner, 'macos', `${host ? app.displayName + ' Host' : app.displayName}.app`)
      : resolve(products, `${product}.app`),
    embed: resolve(namespace, 'embed') };
}
/** An ephemeral exclusive writer claim. Never steal: even a dead PID needs
 * explicit removal after the operator verifies its owner. */
export const appleBuildLock = (app, path = appleArtifacts(app).lock) => claimBuildOutput(app, path);

/** Replace a complete directory using new inodes, restoring the previous
 * artifact if its final rename fails. Caller holds the app writer claim. */
export function placeAppleArtifact(stage, destination) {
  const parent = resolve(destination, '..');
  mkdirSync(parent, { recursive: true });
  const previous = existsSync(destination) ? mkdtempSync(resolve(parent, '.previous-')) : null;
  let moved = false, placed = false;
  try {
    if (previous) { renameSync(destination, resolve(previous, 'artifact')); moved = true; }
    renameSync(stage, destination);
    placed = true;
  } catch (error) {
    if (moved) { renameSync(resolve(previous, 'artifact'), destination); moved = false; }
    throw error;
  } finally { if (previous && (!moved || placed)) rmSync(previous, { recursive: true, force: true }); }
}

/** Capture exactly the named Cargo product the completed bake measured. */
export function captureAppleProduct(build, source, destination) {
  const product = build.products.find(p => p.path === source);
  const bytes = readFileSync(source);
  if (!product || product.bytes !== bytes.length || product.sha256 !== createHash('sha256').update(bytes).digest('hex')) {
    throw new Error(`Cargo product changed before Apple capture: ${source}`);
  }
  writeFileSync(destination, bytes);
}

/** Read the baked compatibility identity from executable bytes, before
 * codesign can add a different identifier. Plists/sidecars are not evidence. */
export function assertAppleIdentity(app, executable, compatibilityId) {
  const bytes = readFileSync(executable, 'utf8');
  const identities = [];
  for (const match of bytes.matchAll(/\{"id":"[0-9a-f]{32}","inputs":/g)) {
    let depth = 0, quoted = false, escaped = false;
    for (let i = match.index; i < bytes.length; i++) {
      const char = bytes[i];
      if (quoted) {
        if (escaped) escaped = false;
        else if (char === '\\') escaped = true;
        else if (char === '"') quoted = false;
      } else if (char === '"') quoted = true;
      else if (char === '{' || char === '[') depth++;
      else if (char === '}' || char === ']') {
        if (--depth === 0) {
          try { identities.push(JSON.parse(bytes.slice(match.index, i + 1))); } catch { /* malformed means no identity */ }
          break;
        }
      }
    }
  }
  if (!identities.length || identities.some((m) => m.inputs.app !== app.id || (compatibilityId && m.id !== compatibilityId))) {
    throw new Error(`${executable} embedded app identity is ${[...new Set(identities.map(m => m.inputs.app))].join(', ') || 'missing'}, expected ${app.id}${compatibilityId ? ` (${compatibilityId})` : ''}`);
  }
  return identities[0].inputs.app;
}

/** Copy the app-visible static trees into a private Apple package stage.
 * Every leaf goes through the web host's no-symlink policy; `trees` maps an
 * app-relative source (notably `gpu/shaders`) to its bundle-visible name. */
export function copyAppleStaticTrees(source, target, trees = [['assets', 'assets'], ['deck', 'deck'], ['shaders', 'shaders'], ['rust', 'rust']]) {
  for (const [from, to] of trees) {
    copyStaticTreeIfPresent(resolve(source, from), resolve(target, to));
  }
}

/** Install the assembled bundle on the simulator. */
export function install(dev, bundle, app, host = false) {
  if (!bundle || !existsSync(bundle)) throw new Error('build the selected app with --ios first');
  assertAppleIdentity(app, resolve(bundle, host ? 'ExactHostIOS' : 'ExactIOS'));
  const r = read('xcrun', ['simctl', 'install', dev.udid, bundle]);
  if (r.status !== 0) throw new Error('simctl install: ' + r.stderr);
}

/** A plist from a plain object: strings, booleans, numbers, arrays, and objects, the four spellings Apple's DTD has. */
const plist = (value, indent = '  ') => {
  if (typeof value === 'string') return `<string>${value.replace(/&/g, '&amp;').replace(/</g, '&lt;')}</string>`;
  if (typeof value === 'boolean') return value ? '<true/>' : '<false/>';
  if (typeof value === 'number') return Number.isInteger(value) ? `<integer>${value}</integer>` : `<real>${value}</real>`;
  if (Array.isArray(value)) return `<array>${value.map((v) => plist(v, indent)).join('')}</array>`;
  return `<dict>\n${Object.entries(value).map(([k, v]) => `${indent}<key>${k}</key>${plist(v, indent + '  ')}`).join('\n')}\n${indent.slice(2)}</dict>`;
};
const plistFile = (dict) => `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0">${plist(dict)}</plist>\n`;

/** What a development client admits through its opening link (LLP 1030.000
 * §7): the dev server it was built against (`--url`) and a random token made
 * for this build. The scheme alone is derived from the public app id, so any
 * page could otherwise point an installed client at its own plan and logic.
 * No URL, no admission: the scheme is then not registered at all — `exact
 * install`'s builds and production bakes. */
export function developmentAdmission(app, locator) {
  let url;
  try { url = new URL(locator); } catch { return null; }
  if (!['http:', 'https:'].includes(url.protocol) || !url.hostname) return null;
  return { scheme: developmentURLScheme(app.id), origins: [url.origin], token: randomBytes(32).toString('hex') };
}

/** The opening links for `page` that this Mac's current development clients
 * admit — what a dev server's opening page offers: one per destination whose
 * placed bundle was built with `--url` at `page`'s origin, carrying that
 * build's token (its receipt keeps it). */
export function developmentLinks(app, page) {
  const origin = new URL(page).origin;
  return ['macos', 'ios-simulator', 'ios'].flatMap((destination) => {
    const { bundle } = appleArtifacts(app, { destination, trust: 'development' });
    let admitted;
    try { admitted = JSON.parse(readFileSync(resolve(bundle, destination === 'macos' ? 'Contents/Resources' : '.', 'receipt.json'), 'utf8')).development; }
    catch { return []; }
    if (!admitted?.origins?.includes(origin) || !/^[0-9a-f]{64}$/.test(admitted.token ?? '')) return [];
    return [{ destination, href: `${admitted.scheme}://open?url=${encodeURIComponent(page)}&token=${admitted.token}` }];
  });
}

function openingLinks(app, platform, development) {
  const schemes = [...new Set([...(app.manifest.host?.[platform]?.urlSchemes ?? []), ...(development ? [development.scheme] : [])])];
  return { ...(schemes.length ? { CFBundleURLTypes: [{ CFBundleURLName: app.id, CFBundleURLSchemes: schemes }] } : {}),
    ...(development ? { ExactDevelopmentURLScheme: development.scheme, ExactDevelopmentOrigins: development.origins, ExactDevelopmentToken: development.token } : {}) };
}

/** Device identity comes from the profile. Simulators need an app identity too:
 * Keychain's default access group is the application-identifier. */
export const entitlements = (app, team = null, debuggable = true, reach = null) => {
  const ios = app.manifest.host?.ios ?? {};
  const dict = {
    'application-identifier': team ? `${team}.${app.id}` : app.id,
    ...(team ? { 'com.apple.developer.team-identifier': team } : {}),
    // A distribution profile grants no debugger; its entitlements must not ask.
    'get-task-allow': debuggable,
  };
  // @ref LLP 1038 D8 — explicit applinks entries, or the declared origin.
  const domains = Array.isArray(ios.associatedDomains) ? ios.associatedDomains : ios.associatedDomains && app.origin ? [`applinks:${new URL(app.origin).host}`] : [];
  // @ref LLP 1069.006 D2 — a claimed https auth callback needs `webcredentials:`
  // (the bake's derivation from the `auth.callback` grants, LLP 1069.008).
  const all = [...new Set([...domains, ...(reach?.auth?.associatedDomains ?? [])])];
  if (all.length) dict['com.apple.developer.associated-domains'] = all;
  return plistFile(dict);
};

/** The device grants' derivations (LLP 1069.008 D4), from the bake's
 * `reach` (`bake/src/reach.rs`, over the runner's one table): each usage key
 * with the base locale's purpose, and `CFBundleLocalizations` with every
 * strings table's tag. Nothing when the app grants no device. */
export const usageKeys = (reach) => {
  const usage = Object.entries(reach?.usage ?? {});
  if (!usage.length) return {};
  return {
    ...Object.fromEntries(usage.map(([key, texts]) => [key, texts[reach.base]])),
    CFBundleDevelopmentRegion: reach.base,
    CFBundleLocalizations: reach.locales,
  };
};

/** `<tag>.lproj/InfoPlist.strings` under `dir`, one per strings table, with
 * that table's purposes (a missing translation already fell back to the
 * base in the bake, as `t` does). */
export function writeUsageStrings(reach, dir) {
  const usage = Object.entries(reach?.usage ?? {});
  if (!usage.length) return;
  for (const locale of reach.locales) {
    mkdirSync(resolve(dir, `${locale}.lproj`), { recursive: true });
    writeFileSync(resolve(dir, `${locale}.lproj`, 'InfoPlist.strings'), plistFile(Object.fromEntries(usage.map(([key, texts]) => [key, texts[locale]]))));
  }
}

/** The hardened-runtime entitlements `exact release` signs the Mac app with:
 * the bake's derivation from the app's `device.*` grants (a usage string
 * alone is not enough; a hardened app without the entitlement is silently
 * refused). Null when the app grants no such device. */
export const macReleaseEntitlements = (compat) => {
  const names = compat?.reach?.entitlements ?? [];
  // An https auth callback's `webcredentials:` domain (LLP 1069.006 D2),
  // which a Developer ID build signs only with a profile that grants it.
  const domains = compat?.reach?.auth?.associatedDomains ?? [];
  if (!names.length && !domains.length) return null;
  return plistFile({ ...Object.fromEntries(names.map((name) => [name, true])),
    ...(domains.length ? { 'com.apple.developer.associated-domains': domains } : {}) });
};

/** Build with Xcode when `xcode-select` names the Command Line Tools, which
 * carry no iOS SDK (LLP 1054 O2): the iOS build otherwise fails deep in a
 * crate's build script with `SDK "iphonesimulator" cannot be located`. An
 * explicit `DEVELOPER_DIR` is kept. */
export function useXcode() {
  if (process.platform !== 'darwin' || process.env.DEVELOPER_DIR) return;
  const selected = spawnSync('xcode-select', ['-p'], { encoding: 'utf8' }).stdout?.trim() ?? '';
  const xcode = '/Applications/Xcode.app/Contents/Developer';
  if (selected.includes('CommandLineTools') && existsSync(xcode)) {
    process.env.DEVELOPER_DIR = xcode;
    console.log(`host/apple: xcode-select names the Command Line Tools (${selected}); building with ${xcode}`);
  }
}

/** A loose `Frameworks/lib….dylib` as `Frameworks/<name>.framework/<name>`,
 * which is the only form App Store Connect accepts for an embedded library
 * (ITMS-90171). The presenter loads either (`embeddedModule` in ExactKit). */
function wrapFramework(frameworks, loose, name, app) {
  const from = resolve(frameworks, loose);
  if (!existsSync(from)) return;
  const dir = resolve(frameworks, `${name}.framework`);
  mkdirSync(dir, { recursive: true });
  renameSync(from, resolve(dir, name));
  run('install_name_tool', ['-id', `@rpath/${name}.framework/${name}`, resolve(dir, name)], { stdio: 'ignore' });
  writeFileSync(resolve(dir, 'Info.plist'), plistFile({
    CFBundleExecutable: name, CFBundleIdentifier: `${app.id}.${name.toLowerCase()}`, CFBundleName: name,
    CFBundlePackageType: 'FMWK', CFBundleVersion: process.env.EXACT_BUILD_NUMBER ?? '1', CFBundleShortVersionString: process.env.EXACT_VERSION ?? '0.1.0',
    CFBundleSupportedPlatforms: ['iPhoneOS'], MinimumOSVersion: app.manifest.host?.ios?.minimumOS ?? '17.0',
  }));
}

/** The iOS `Info.plist` from the manifest (LLP 1030 D2: one declaration; `build.mjs` consumes what it generates). The dev client's local-networking permission is `host.ios.localNetworking` (a string: the prompt); the store-required version numbers are counters bake owns, not authored. */
export const infoPlist = (app, device = false, { executable = 'ExactIOS', id = app.id, name = app.displayName, development = null, icon = {}, distribution = null, reach = null, tv = false } = {}) => {
  const ios = app.manifest.host?.ios ?? {};
  // tvOS reuses the manifest's iOS section; Apple TV is device family 3.
  const families = tv ? [3] : (ios.deviceFamily ?? ['iphone', 'ipad']).map((f) => (f === 'ipad' ? 2 : 1));
  const dict = {
    CFBundleExecutable: executable,
    CFBundleIdentifier: id,
    CFBundleName: name,
    CFBundleDisplayName: name,
    CFBundlePackageType: 'APPL',
    CFBundleVersion: '1',
    CFBundleShortVersionString: '0.1.0',
    CFBundleSupportedPlatforms: [tv ? 'AppleTVSimulator' : device ? 'iPhoneOS' : 'iPhoneSimulator'],
    DTPlatformName: tv ? 'appletvsimulator' : device ? 'iphoneos' : 'iphonesimulator',
    MinimumOSVersion: ios.minimumOS ?? '17.0',
    UIDeviceFamily: families,
    UILaunchScreen: {},
    UIApplicationSceneManifest: { UIApplicationSupportsMultipleScenes: false },
    ...(tv ? {} : { CADisableMinimumFrameDurationOnPhone: true }),
  };
  if (ios.localNetworking) {
    dict.NSAppTransportSecurity = { NSAllowsLocalNetworking: true };
    dict.NSLocalNetworkUsageDescription = typeof ios.localNetworking === 'string' ? ios.localNetworking : 'Connects to your dev server on the local network.';
  }
  if (ios.backgroundModes?.length) dict.UIBackgroundModes = ios.backgroundModes;
  // The manifest's `file_handlers`, as on the Mac, opened in place from
  // Files ("Open in", LLP 1069.010 slice 4), and the non-system types they
  // name (Markdown) imported so Files can match them.
  if (documentTypes(app).length) {
    dict.CFBundleDocumentTypes = documentTypes(app);
    dict.LSSupportsOpeningDocumentsInPlace = true;
    const imported = importedTypes(app);
    if (imported.length) dict.UTImportedTypeDeclarations = imported;
  }
  Object.assign(dict, openingLinks(app, 'ios', development));
  Object.assign(dict, usageKeys(reach));
  Object.assign(dict, icon);
  if (distribution) Object.assign(dict, distribution);
  return plistFile(dict);
};

/** What App Store Connect reads from a distributed bundle and Xcode would
 * have written (`--archive`): the build's toolchain (DT* keys) and the store's
 * counters, EXACT_VERSION and EXACT_BUILD_NUMBER (the version and build
 * numbers App Store Connect requires to rise). */
export function distributionKeys() {
  const xcode = read('xcodebuild', ['-version']).stdout ?? '';
  const [major, minor = '0', patch = '0'] = (/Xcode (\d+)(?:\.(\d+))?(?:\.(\d+))?/.exec(xcode) ?? []).slice(1);
  const sdk = (flag) => read('xcrun', ['--sdk', 'iphoneos', flag]).stdout.trim();
  const sdkVersion = sdk('--show-sdk-version');
  const sdkBuild = sdk('--show-sdk-build-version');
  return {
    CFBundleVersion: process.env.EXACT_BUILD_NUMBER ?? '1',
    CFBundleShortVersionString: process.env.EXACT_VERSION ?? '0.1.0',
    DTCompiler: 'com.apple.compilers.llvm.clang.1_0',
    DTPlatformBuild: sdkBuild,
    DTPlatformVersion: sdkVersion,
    DTSDKBuild: sdkBuild,
    DTSDKName: `iphoneos${sdkVersion}`,
    DTXcode: `${major.padStart(2, '0')}${minor}${patch}`,
    DTXcodeBuild: /Build version (\S+)/.exec(xcode)?.[1] ?? '',
    BuildMachineOSBuild: read('sw_vers', ['-buildVersion']).stdout.trim(),
    UIRequiredDeviceCapabilities: ['arm64'],
    // What iOS assumes when the key is absent, written out: App Store Connect
    // requires it (all four on iPad, for multitasking).
    UISupportedInterfaceOrientations: ['UIInterfaceOrientationPortrait', 'UIInterfaceOrientationLandscapeLeft', 'UIInterfaceOrientationLandscapeRight'],
    'UISupportedInterfaceOrientations~ipad': ['UIInterfaceOrientationPortrait', 'UIInterfaceOrientationPortraitUpsideDown', 'UIInterfaceOrientationLandscapeLeft', 'UIInterfaceOrientationLandscapeRight'],
    // Export compliance is the developer's declaration, so it is written only
    // when EXACT_NON_EXEMPT_ENCRYPTION says it (`false` for HTTPS alone).
    ...(['true', 'false'].includes(process.env.EXACT_NON_EXEMPT_ENCRYPTION) ? { ITSAppUsesNonExemptEncryption: process.env.EXACT_NON_EXEMPT_ENCRYPTION === 'true' } : {}),
  };
}

// The UTI each MIME type names on Apple platforms. `inode/directory` is the
// one deviation from IANA's registry — freedesktop's spelling for a folder,
// because the web has no MIME type for one and an app that opens a directory
// (the LLP reader) must be able to say so. An unmapped type fails the bake
// rather than guessing `public.data` (LLP 0382: fail closed, loudly), and
// before anything is compiled (`main`): the common document and image types
// are here (ledger diary F9).
const UTIS = {
  'text/markdown': 'net.daringfireball.markdown',
  'text/plain': 'public.plain-text',
  'text/html': 'public.html',
  'text/csv': 'public.comma-separated-values-text',
  'text/tab-separated-values': 'public.tab-separated-values-text',
  'application/json': 'public.json',
  'application/pdf': 'com.adobe.pdf',
  'application/zip': 'public.zip-archive',
  'image/png': 'public.png',
  'image/jpeg': 'public.jpeg',
  'image/gif': 'com.compuserve.gif',
  'image/webp': 'org.webmproject.webp',
  'image/svg+xml': 'public.svg-image',
  'inode/directory': 'public.folder',
};
// The types iOS does not declare itself, which the bundle imports.
const IMPORTED = new Set(['net.daringfireball.markdown']);

/** `CFBundleDocumentTypes` from the manifest's `file_handlers` (LLP 1033 D1):
 *  one entry per handler, `Viewer` and `Alternate` so declaring a type never
 *  takes it away from whatever already owns it. */
export function documentTypes(app) {
  return (app.manifest.file_handlers ?? []).map((handler) => {
    const types = Object.keys(handler.accept).map((mime) => {
      const uti = UTIS[mime];
      if (!uti) throw new Error(`host/apple: ${app.name}'s file_handlers accepts ${mime}, which names no Apple type this host maps (it maps ${Object.keys(UTIS).join(', ')})`);
      return uti;
    });
    const extensions = [...new Set(Object.values(handler.accept).flat().map((e) => e.replace(/^\./, '')).filter(Boolean))];
    return {
      CFBundleTypeName: handler.name ?? `${app.displayName} document`,
      CFBundleTypeRole: 'Viewer',
      LSHandlerRank: 'Alternate',
      LSItemContentTypes: types,
      ...(extensions.length ? { CFBundleTypeExtensions: extensions } : {}),
    };
  });
}

/** `UTImportedTypeDeclarations` for the types `file_handlers` names that
 *  iOS does not declare itself (Markdown's `net.daringfireball.markdown`):
 *  its extensions and MIME type, conforming to plain text. */
export function importedTypes(app) {
  return (app.manifest.file_handlers ?? []).flatMap((handler) => Object.entries(handler.accept)
    .filter(([mime]) => IMPORTED.has(UTIS[mime]))
    .map(([mime, extensions]) => ({
      UTTypeIdentifier: UTIS[mime],
      UTTypeDescription: handler.name ?? mime,
      UTTypeConformsTo: ['public.plain-text'],
      UTTypeTagSpecification: { 'public.filename-extension': extensions.map((e) => e.replace(/^\./, '')), 'public.mime-type': [mime] },
    })));
}

/** The manifest's `launch_handler.client_mode` (LLP 1069.010 D4): the first
 *  mode the Mac host has, as the W3C list is read; `auto` and absence are
 *  the host's default, `navigate-existing`. */
export function launchMode(app) {
  const modes = [app.manifest.launch_handler?.client_mode ?? []].flat();
  return modes.find((m) => m !== 'auto') ?? 'navigate-existing';
}

/** The app icon from the manifest's first square icon of at least 512 px
 * (`icons`, the web manifest's own field): loose PNGs named by
 * `CFBundleIcons` on iOS, an `.icns` built by `iconutil` on macOS. Returns
 * the plist keys to merge; nothing when the app declares no such icon. */
export function appIcon(app, dir, platform, { catalog = false } = {}) {
  const icon = (app.manifest.icons ?? []).find((i) => { const m = /^(\d+)x(\d+)$/.exec(i.sizes ?? ''); return m && m[1] === m[2] && Number(m[1]) >= 512; });
  if (!icon) return {};
  const source = resolve(app.dir, icon.src);
  if (!existsSync(source)) throw new Error(`host/apple: ${app.name}'s icon ${icon.src} does not exist`);
  const sized = (px, out) => run('sips', ['-z', String(px), String(px), source, '--out', out], { stdio: 'ignore' });
  if (platform === 'ios') {
    for (const [name, px] of [['AppIcon60x60@2x.png', 120], ['AppIcon60x60@3x.png', 180], ['AppIcon76x76@2x~ipad.png', 152], ['AppIcon83.5x83.5@2x~ipad.png', 167]]) sized(px, resolve(dir, name));
    // A distributed build also compiles the icon into Assets.car: App Store
    // Connect requires the asset catalog, not loose PNGs.
    if (catalog) {
      const set = resolve(catalog, 'AppIcon.appiconset');
      mkdirSync(set, { recursive: true });
      sized(1024, resolve(set, 'icon.png'));
      writeFileSync(resolve(set, 'Contents.json'), JSON.stringify({ images: [{ filename: 'icon.png', idiom: 'universal', platform: 'ios', size: '1024x1024' }], info: { author: 'exact', version: 1 } }));
    }
    const primary = (files) => ({ CFBundlePrimaryIcon: { CFBundleIconFiles: files, CFBundleIconName: 'AppIcon' } });
    return { CFBundleIcons: primary(['AppIcon60x60']), 'CFBundleIcons~ipad': primary(['AppIcon60x60', 'AppIcon76x76', 'AppIcon83.5x83.5']) };
  }
  const set = mkdtempSync(resolve(dir, '.icon-')) + '.iconset';
  mkdirSync(set);
  for (const base of [16, 32, 128, 256, 512]) {
    sized(base, resolve(set, `icon_${base}x${base}.png`));
    sized(base * 2, resolve(set, `icon_${base}x${base}@2x.png`));
  }
  run('iconutil', ['-c', 'icns', set, '-o', resolve(dir, 'AppIcon.icns')], { stdio: 'ignore' });
  rmSync(set, { recursive: true, force: true });
  return { CFBundleIconFile: 'AppIcon' };
}

/** All iOS asset sets share one actool pass: each pass replaces Assets.car. */
export function iosAssets(app, dir, device, { catalog = false } = {}) {
  const work = mkdtempSync(resolve(tmpdir(), 'exact-ios-assets-'));
  try {
    const assets = resolve(work, 'Assets.xcassets');
    const keys = { ...appIcon(app, dir, 'ios', { catalog: catalog ? assets : false }), ...launchScreen(app, assets) };
    const hasIcon = existsSync(resolve(assets, 'AppIcon.appiconset'));
    if (catalog && !hasIcon) throw new Error(`host/apple: ${app.name}'s distribution bundle requires an AppIcon; declare a square icon of at least 512 px`);
    if (hasIcon || keys.UILaunchScreen) {
      writeFileSync(resolve(assets, 'Contents.json'), JSON.stringify({ info: { author: 'exact', version: 1 } }));
      const partial = resolve(work, 'partial.plist');
      run('xcrun', ['actool', assets, '--compile', dir, '--platform', device ? 'iphoneos' : 'iphonesimulator',
        '--minimum-deployment-target', app.manifest.host?.ios?.minimumOS ?? '17.0',
        ...(hasIcon ? ['--app-icon', 'AppIcon', '--target-device', 'iphone', '--target-device', 'ipad'] : []),
        '--output-partial-info-plist', partial, '--output-format', 'human-readable-text'], { stdio: 'ignore' });
      Object.assign(keys, JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', partial], { encoding: 'utf8', stdio: 'pipe' }).stdout));
    }
    if (catalog) {
      const contents = JSON.parse(run('xcrun', ['assetutil', '--info', resolve(dir, 'Assets.car')], { encoding: 'utf8', stdio: 'pipe' }).stdout);
      if (!contents.some(asset => asset.Name === 'AppIcon')) throw new Error(`host/apple: ${dir}/Assets.car has no AppIcon`);
    }
    return keys;
  } finally { rmSync(work, { recursive: true, force: true }); }
}

/** The launch screen in the app's own background, light and dark
 * (the manifest's `background_color` and `background_color_dark`): iOS crossfades
 * from the launch screen to the first frame, and between two screens of one
 * colour that crossfade is invisible, so the app opens on its first frame.
 * `UILaunchScreen` names colours only from an asset catalog, so this
 * writes its colour set for the shared compile. Returns the plist keys to merge. */
function launchScreen(app, catalog) {
  const light = app.manifest.background_color, dark = app.manifest.background_color_dark;
  if (!light) return {};
  const components = (hex, field) => {
    const m = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(hex ?? '');
    if (!m) throw new Error(`${field} must be a #RGB, #RRGGBB or #RRGGBBAA colour, not ${JSON.stringify(hex)}`);
    const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
    const a = h.length === 8 ? parseInt(h.slice(6), 16) : 255;
    return { 'color-space': 'srgb', components: { red: `0x${h.slice(0, 2)}`, green: `0x${h.slice(2, 4)}`, blue: `0x${h.slice(4, 6)}`, alpha: (a / 255).toFixed(3) } };
  };
  const colors = [{ idiom: 'universal', color: components(light, 'background_color') }];
  if (dark) colors.push({ idiom: 'universal', appearances: [{ appearance: 'luminosity', value: 'dark' }], color: components(dark, 'background_color_dark') });
  mkdirSync(resolve(catalog, 'ExactLaunch.colorset'), { recursive: true });
  writeFileSync(resolve(catalog, 'ExactLaunch.colorset', 'Contents.json'), JSON.stringify({ colors, info: { author: 'exact', version: 1 } }));
  return { UILaunchScreen: { UIColorName: 'ExactLaunch' } };
}

/** The SDK an Apple app records when its manifest asks for the design before
 * iOS 26 and macOS 26 (`host.<platform>.designRequiresCompatibility`): the
 * last before it. iOS 27 and macOS 27 ignore UIDesignRequiresCompatibility
 * (probed 2026-10-02: the key is read, the new design drawn); both draw the
 * design the recorded SDK had. */
export const COMPATIBLE_SDK = { ios: '18.0', macos: '15.0' };

/** Whether an app draws UIKit's or AppKit's design before 26 on `platform`;
 * refused for an app whose `minimumOS` there is 26 or later, which has no
 * earlier design to keep. */
export function designCompatible(app, platform) {
  if (!app.manifest.host?.[platform]?.designRequiresCompatibility) return false;
  const minimum = deploymentTargets(app)[platform];
  if (Number(minimum.split('.')[0]) >= 26) throw new Error(`host/apple: ${app.id}: host.${platform}.designRequiresCompatibility with minimumOS ${minimum} — an app that needs ${platform === 'ios' ? 'iOS' : 'macOS'} 26 has no earlier design to keep; remove one of them`);
  return true;
}

/** The SDK the linker recorded in an executable (`LC_BUILD_VERSION`) is the
 * one asked for: AppKit draws its design by that number, so a link that
 * records another (as SwiftPM's did, LLP 1069.011 §7) changes every app's
 * look without a word. Compared to major.minor. */
function assertLinkedSdk(executable, expected) {
  const loads = read('otool', ['-l', executable]).stdout ?? '';
  const recorded = /cmd LC_BUILD_VERSION[\s\S]*?\n\s*sdk (\S+)/.exec(loads)?.[1];
  const majorMinor = (v) => String(v).split('.').slice(0, 2).map(Number).join('.');
  if (!recorded || majorMinor(recorded) !== majorMinor(expected)) {
    throw new Error(`host/apple: ${basename(executable)} records SDK ${recorded ?? '(none)'}, not ${expected}: AppKit would draw it in another design`);
  }
}

/** The macOS `Info.plist` for a bundled build, from the same manifest. */
export const macInfoPlist = (app, { development = null, icon = {}, reach = null } = {}) => plistFile({
  ...icon,
  CFBundleExecutable: 'ExactMac',
  CFBundleIdentifier: app.id,
  CFBundleName: app.displayName,
  CFBundleDisplayName: app.displayName,
  CFBundlePackageType: 'APPL',
  CFBundleVersion: '1',
  CFBundleShortVersionString: '0.1.0',
  LSMinimumSystemVersion: app.manifest.host?.macos?.minimumOS ?? '14.0',
  NSHighResolutionCapable: true,
  // Usage strings for the devices the app's grants name (LLP 1069.008).
  ...usageKeys(reach),
  ...(app.manifest.host?.macos?.window ? { ExactWindow: app.manifest.host.macos.window } : {}),
  ...(documentTypes(app).length ? { CFBundleDocumentTypes: documentTypes(app) } : {}),
  // Where a launch lands (LLP 1069.010 D4) is the manifest's `launch_handler`'s, with or
  // without `file_handlers`: File ▸ New Window needs no document type. Always written, so
  // absence, `auto` and an explicit `navigate-existing` are one plist.
  ExactLaunchMode: launchMode(app),
  ...openingLinks(app, 'macos', development),
});

/** The build receipt (LLP 1030 D2): what this binary was actually built from and with — the toolchain, the SDK, the identity and profile, the entitlements as signed — written beside it, never committed, so "what did this binary contain" is answered by a file. */
export function receipt(app, fields) {
  const version = (cmd, args) => (read(cmd, args).stdout ?? '').trim().split('\n')[0];
  return JSON.stringify({
    app: { id: app.id, name: app.displayName, origin: app.origin, declared: app.declared },
    built: new Date().toISOString(),
    toolchain: { rustc: version('rustc', ['--version']), swift: version('swift', ['--version']), xcode: version('xcodebuild', ['-version']) },
    ...fields,
  }, null, 2) + '\n';
}

// ---------------------------------------------------------------- the lean Hermes an iOS app links

/** The archives js/build.rs links from each platform's CMake build. */
export const HERMES_IOS_ARCHIVES = ['lib/libhermesvmlean_a.a', 'jsi/libjsi.a', 'external/boost/boost_1_86_0/libs/context/libboost_context.a'];
// Checks the source, then builds and publishes one platform; ibex's lock is held throughout.
const HERMES_IOS_SCRIPT = `set -eu
src=$1 pin=$2 root=$3 platform=$4 sdk=$5 arch=$6; shift 6
out=$root/$platform build=$root/.build-$platform
fix="run ./scripts/build-hermes.sh --vanilla $pin in ibex"
[ -d "$src/.git" ] || { echo "no Hermes source at $src: $fix" >&2; exit 1; }
head=$(git -C "$src" rev-parse HEAD)
[ "$head" = "$pin" ] || { echo "ibex's Hermes source $src is at $head; js/build.rs pins $pin: $fix" >&2; exit 1; }
[ -z "$(git -C "$src" status --porcelain --untracked-files=no)" ] || { echo "ibex's Hermes source $src is patched; the lean VM is pristine upstream: $fix" >&2; exit 1; }
[ -f "$src/build_host_hermesc/ImportHostCompilers.cmake" ] || { echo "no host compiler in $src/build_host_hermesc: $fix" >&2; exit 1; }
if [ -e "$out" ]; then
  for a; do [ -f "$out/$a" ] || { echo "$out lacks $a: remove it and build again" >&2; exit 1; }; done
  exit 0
fi
cmake -S "$src" -B "$build" -DHERMES_APPLE_TARGET_PLATFORM="$sdk" -DCMAKE_OSX_ARCHITECTURES="$arch" \\
  -DCMAKE_OSX_DEPLOYMENT_TARGET=17.0 -DHERMES_ENABLE_DEBUGGER=OFF -DHERMES_ENABLE_INTL=ON \\
  -DHERMES_ENABLE_TEST_SUITE=OFF -DHERMES_ENABLE_BITCODE=OFF -DHERMES_BUILD_APPLE_FRAMEWORK=OFF \\
  -DHERMES_BUILD_SHARED_JSI=OFF -DIMPORT_HOST_COMPILERS="$src/build_host_hermesc/ImportHostCompilers.cmake" \\
  -DCMAKE_BUILD_TYPE=MinSizeRel
cmake --build "$build" --target hermesvmlean_a jsi boost_context -j "$(sysctl -n hw.ncpu)"
stage=$(mktemp -d "$root/.stage-$platform.XXXXXX") && chmod 755 "$stage"
for a; do mkdir -p "$stage/$(dirname "$a")"; cp "$build/$a" "$stage/$a"; done
mv "$stage" "$out"
rm -rf "$build"`;

/** An iOS app with an `app.ts` links lean Hermes for its platform. Missing
 * from the per-pin cache every checkout and outside app shares, it is built
 * here, once per machine: only that platform's three CMake targets, from
 * ibex's pristine source cache with ibex's host compiler, under ibex's own
 * source-build lock, so neither build moves the checkout under the other.
 * EXACT_HERMES_IOS_DIR's archives are provisioned elsewhere; js/build.rs
 * refuses missing ones. @ref LLP 1036.001 D5 */
export function provisionHermesIos(platform, env = process.env) {
  const { pin, root, cached } = hermesIos(env), out = resolve(root, platform);
  if (!cached || HERMES_IOS_ARCHIVES.every(a => existsSync(resolve(out, a)))) return;
  const cache = resolve(env.HOME ?? homedir(), '.cache/exact');
  console.error(`host/apple: building lean Hermes for ${platform} (facebook/hermes ${pin.slice(0, 12)}) into ${out}, once for this machine`);
  mkdirSync(root, { recursive: true });
  const { SDKROOT, ...clean } = env; // the platform names its own SDK
  const sdk = platform === 'ios' ? 'iphoneos' : 'iphonesimulator', arch = platform === 'ios' || process.arch === 'arm64' ? 'arm64' : 'x86_64';
  const r = spawnSync('perl', ['-MFcntl=:flock', '-e', 'open(my $l, ">>", shift) or die "lock: $!\\n"; flock($l, LOCK_EX) or die "flock: $!\\n"; exit(system(@ARGV) == 0 ? 0 : 1)',
    resolve(cache, 'hermes-source-build.lock'), 'sh', '-c', HERMES_IOS_SCRIPT, 'hermes', resolve(cache, 'hermes/hermes-src'), pin, root, platform, sdk, arch, ...HERMES_IOS_ARCHIVES],
    { env: clean, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (r.status !== 0) throw new Error(`lean Hermes for ${platform} did not build (${r.error?.message ?? `exit ${r.status}`}):\n${`${r.stdout ?? ''}${r.stderr ?? ''}`.trim().split('\n').slice(-30).join('\n')}`);
}

// ---------------------------------------------------------------- the build

function main(args) {
  let launchEnv;
  try { launchEnv = developmentLaunchEnvironment(args); }
  catch (e) { console.error(e.message); process.exitCode = 1; return; }
  const device = args.includes('--device');
  // tvOS runs the iOS simulator path with tvOS's target, SDK and plist.
  const tv = args.includes('--tvos');
  const ios = device || tv || args.includes('--ios');
  if (tv && (device || args.includes('--archive') || args.includes('--host') || args.includes('--embed'))) {
    console.error('--tvos builds for an Apple TV simulator only; it takes none of --device, --archive, --host, --embed');
    process.exitCode = 1; return;
  }
  if (tv && !read('rustup', ['target', 'list', '--installed'], { cwd: root }).stdout?.split('\n').includes(tvosTarget)) {
    console.error(`--tvos needs Rust's ${tvosTarget} std, which this toolchain lacks. Install it with \`rustup target add ${tvosTarget}\``);
    process.exitCode = 1; return;
  }
  const ipa = args.includes('--archive') ? resolve(process.cwd(), args[args.indexOf('--archive') + 1] ?? '') : null;
  const unsigned = args.includes('--unsigned');
  if ((unsigned && !ipa) || ipa && (!device || (!unsigned && (!process.env.EXACT_IDENTITY || !process.env.EXACT_PROFILE)) || args.includes('--run') || args.includes('--host'))) {
    console.error('--archive needs --device and EXACT_IDENTITY and EXACT_PROFILE (or --unsigned, which needs --archive), and takes neither --run nor --host');
    process.exitCode = 1; return;
  }
  const app = resolveApp(args.find((a, i) => !a.startsWith('--') && !['--sim', '--phone', '--url', '--archive'].includes(args[i - 1])));
  documentTypes(app); // an unmapped `file_handlers` type is refused before a long build
  const release = appleBuildLock(app);
  const cleanup = [];
  try {
  const crate = app.crate('apple');
  const gpuCrate = app.crate('gpu');
  const hasGpu = app.hasGpu;
  // The bake names each GPU module's digest, checked at load: a re-signer's bytes would be refused.
  if (unsigned && (hasGpu || gpuModules(app.manifest).length)) throw new Error(`--unsigned: ${app.name} has GPU modules, whose baked digests a re-signer would break; archive it signed (EXACT_IDENTITY and EXACT_PROFILE)`);
  let ph, prof;
  const sha1 = unsigned ? '-' : device ? (() => {
    ph = ipa ? null : phone(args.includes('--phone') ? args[args.indexOf('--phone') + 1] : undefined);
    prof = profile(ph?.udid, app.id);
    return identity(prof.team);
  })() : ios ? '-' : macIdentity();
  const dylib = `lib${gpuCrate.replace(/-/g, '_')}.dylib`;
  // What the presenter dlopens is the same name whatever the app is: one
  // Swift binary serves every app, and two apps' modules would otherwise
  // are captured and packaged together under the selected app's owner.
  const loadName = 'libexact_gpu.dylib';
  // Each declared GPU module's dylib beside it (LLP 1009 D6), under the name
  // GpuModule.loadName gives it; signed before the host bake binds its digest.
  const moduleDylibs = gpuModules(app.manifest).map(({ name }) => ({
    built: `lib${app.crate(`gpu-${name}`).replace(/-/g, '_')}.dylib`, load: `libexact_gpu_${name.replace(/-/g, '_')}.dylib` }));
  const webLoadName = 'libexact_web.dylib';
  const videoLoadName = 'libexact_video.dylib';
  const svgLoadName = 'libexact_svg.dylib';
  const webBuildDir = mkdtempSync(resolve(tmpdir(), 'exact-webarm-'));
  cleanup.push(webBuildDir);
  const webBuilt = resolve(webBuildDir, webLoadName);
  const t0 = Date.now();
  const target = ios ? (device ? 'aarch64-apple-ios' : tv ? tvosTarget : iosTarget) : bakeTarget('macos');
  const sdkName = ios ? (device ? 'iphoneos' : tv ? 'appletvsimulator' : 'iphonesimulator') : 'macosx';
  const destination = ios ? (device ? 'ios' : tv ? 'tvos-simulator' : 'ios-simulator') : 'macos';
  const sdk = read('xcrun', ['--sdk', sdkName, '--show-sdk-path']).stdout.trim();
  const targets = deploymentTargets(app);
  // The filter pictures' kernels (iOS; `SvgFilterMetal`), for the bundle,
  // when Xcode's Metal toolchain is installed.
  const svgFilterBuilt = ios ? svgFilterLibrary(sdkName, targets.ios, resolve(webBuildDir, svgFilterLibraryName)) : null;
  const cargoEnv = {
    ...developmentBuildEnv(),
    SDKROOT: sdk,
    MACOSX_DEPLOYMENT_TARGET: targets.macos,
    ...(ios ? {
      ...(tv ? { TVOS_DEPLOYMENT_TARGET: targets.ios } : { IPHONEOS_DEPLOYMENT_TARGET: targets.ios }),
      // The bake's host dependencies compile Objective-C++ too. cc-rs
      // inherits SDKROOT; target the Mac SDK explicitly for those units.
      HOST_CXXFLAGS: `${process.env.HOST_CXXFLAGS ?? ''} -isysroot ${read('xcrun', ['--sdk', 'macosx', '--show-sdk-path']).stdout.trim()}`,
    } : {}),
  };
  // A production bake is `release`; any other builds `apple-dev` (Cargo.toml),
  // the same optimizations without whole-graph LTO, for the touch-one-line budget.
  // An app outside this repo gets it with the root's other profiles, injected
  // at build (injectedProfiles, LLP 1036.001 D1).
  const devProfile = 'apple-dev';
  const cargoProfile = cargoEnv.EXACT_UPDATE_TRUST === 'production' ? 'release' : devProfile;
  const cargoLibDir = resolve(app.target, target, cargoProfile);
  // Named Cargo products can alias in external workspaces or two checkouts
  // sharing a target. Claim those names through bake-and-capture only.
  let bakedPlan, paths;
  const development = cargoEnv.EXACT_UPDATE_TRUST === 'development' && args.includes('--url') ? developmentAdmission(app, launchEnv.EXACT_DEV_PLAN) : null;
  cargoEnv.EXACT_BAKE_OUTPUT = bakeOutput(app, cargoEnv);
  if (tv && existsSync(resolve(app.dir, 'app.ts')) && cargoEnv.EXACT_JS_ENGINE !== 'stub') throw new Error(`--tvos: ${app.name} has app.ts, and Hermes is not built for tvOS yet; set EXACT_JS_ENGINE=stub`);
  if (ios && !tv && existsSync(resolve(app.dir, 'app.ts')) && cargoEnv.EXACT_JS_ENGINE !== 'stub') provisionHermesIos(device ? 'ios' : 'ios-simulator');
  const buildReceipt = contractLast(() => buildBake(app, ios ? 'ios' : 'macos', target, { env: cargoEnv, profile: cargoProfile, prepareGpu(product) {
    // Cargo puts its own unsigned file back on every build, and a signature
    // carries its signing time: signing in place made the app's bake (which
    // names this product's digest) run on every build. Sign a copy beside it,
    // again only when Cargo's bytes or the identity change.
    const signed = resolve(dirname(product), 'signed', basename(product)), record = `${signed}.source`;
    const source = `${createHash('sha256').update(readFileSync(product)).digest('hex')} ${sha1 ?? '-'}\n`;
    const current = existsSync(signed) && existsSync(record) && readFileSync(record, 'utf8') === source
      && read('codesign', ['--verify', '--strict', ...(sha1 ? ['-R', `=certificate leaf = H"${sha1}"`] : []), signed]).status === 0;
    if (!current) {
      mkdirSync(dirname(signed), { recursive: true });
      copyFileSync(product, signed);
      run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', signed], {stdio:'ignore'});
      writeFileSync(record, source);
    }
    return signed;
  }, capture(buildReceipt) {
    const composition = buildReceipt.compat.inputs?.store?.L === '0' ? 'embedded' : 'updating';
    paths = appleArtifacts(app, { destination, composition, trust: cargoEnv.EXACT_UPDATE_TRUST });
    mkdirSync(paths.namespace, { recursive: true });
    const capture = mkdtempSync(resolve(paths.namespace, '.capture-'));
    cleanup.push(capture);
    captureAppleProduct(buildReceipt, resolve(cargoLibDir, `lib${crate.replace(/-/g, '_')}.a`), resolve(capture, `lib${crate.replace(/-/g, '_')}.a`));
    // GPU products are the signed copies prepareGpu made (the primary and each module).
    for (const file of [...(hasGpu ? [dylib] : []), ...moduleDylibs.map(m => m.built)]) captureAppleProduct(buildReceipt, resolve(cargoLibDir, 'signed', file), resolve(capture, file));
    bakedPlan = readFileSync(resolve(cargoEnv.EXACT_BAKE_OUTPUT, `${ios ? 'ios' : 'macos'}-${target}.plan`));
    copyAppleStaticTrees(app.dir, capture, [['assets', 'assets'], ['deck', 'deck']]);
    copyShaders(app, resolve(capture, 'shaders'));
    if (buildReceipt.rust) copyStaticTreeIfPresent(buildReceipt.rust, resolve(capture, 'rust'));
    verifyBakeFiles(buildReceipt.compat, bakedPlan, listAssets(capture, true));
    placeAppleArtifact(capture, paths.capture);
  } }));
  const libDir = paths.capture;
  const bakedCompat = buildReceipt.compat;
  const level = bakedCompat.inputs?.store?.L;
  if (!['0', 'A'].includes(level)) throw new Error(`host/apple: unsupported baked store level ${level}`);
  const composition = level === '0' ? 'embedded' : 'updating';
  console.log(`host/apple: baked L=${level}; Swift ${composition} composition`);
  // The app's GPU module (LLP 1009 D2): a dylib beside the executable (in
  // the bundle's Frameworks on iOS), loaded on demand by the presenter.
  const gpuNote = [...(hasGpu ? [dylib] : []), ...moduleDylibs.map(m => m.built)].join(', ') || 'no GPU crate';
  // --embed (LLP 1031 D10, the developer-facing promise): what a consumer
  // without a Rust toolchain links — the archive, the C header, the GPU
  // module, the shaders and assets, and the cohort's `compat.json` — under
  // the resolver's app-owned `embed` directory, with `ExactKit` at
  // `host/apple`. No Swift is built for it; the sample hosts are the proof
  // that the same pieces link.
  if (args.includes('--embed')) {
    const platform = ios ? (device ? 'ios' : 'ios-simulator') : 'macos';
    const embed = mkdtempSync(resolve(paths.namespace, '.embed-'));
    cleanup.push(embed);
    mkdirSync(resolve(embed, 'include'), { recursive: true });
    const archive = `lib${crate.replace(/-/g, '_')}.a`;
    copyFileSync(resolve(libDir, archive), resolve(embed, archive));
    copyFileSync(resolve(pkg, 'include/exact.h'), resolve(embed, 'include/exact.h'));
    if (hasGpu) copyFileSync(resolve(libDir, dylib), resolve(embed, loadName));
    for (const m of moduleDylibs) copyFileSync(resolve(libDir, m.built), resolve(embed, m.load));
    // Beside the executable in the embedding app's bundle, as ExactIOS has it.
    if (svgFilterBuilt) copyFileSync(svgFilterBuilt, resolve(embed, svgFilterLibraryName));
    copyAppleStaticTrees(paths.capture, embed);
    verifyBakeFiles(bakedCompat, bakedPlan, listAssets(embed, true));
    writeFileSync(resolve(embed, 'compat.json'), JSON.stringify(bakedCompat, null, 2) + '\n');
    writeFileSync(resolve(embed, 'receipt.json'), receipt(app, { platform, target: ios ? target : (process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin'), sdk, archive, gpu: hasGpu ? loadName : null, package: pkg, composition, compatibilityId:bakedCompat.id, build:buildReceipt }));
    const bytes = statSync(resolve(embed, archive)).size;
    placeAppleArtifact(embed, paths.embed);
    console.log(`host/apple: ${paths.embed.replace(root + '/', '')} — ${archive} ${(bytes / 1048576).toFixed(2)} MB, include/exact.h${hasGpu ? `, ${loadName}` : ''}${svgFilterBuilt ? `, ${svgFilterLibraryName} (the bundle's top level)` : ios ? ', no SVG filter kernels (no Metal toolchain)' : ''}, shaders, assets, compat.json; link it with ExactKit${composition === 'updating' ? ' + ExactUpdates' : ''} from the package at ${pkg.replace(root + '/', '')}`);
    if (!args.includes('--run') && !args.includes('--host')) return;
  }
  const t1 = Date.now();
  // SwiftPM compiles Package.swift itself for macOS before applying the iOS
  // product triple; an iPhone SDKROOT in its environment breaks that host
  // manifest compile. The target SDK stays in the explicit Swift arguments.
  const env = {
    ...process.env,
    ...(tv ? { TVOS_DEPLOYMENT_TARGET: targets.ios } : ios ? { IPHONEOS_DEPLOYMENT_TARGET: targets.ios } : { MACOSX_DEPLOYMENT_TARGET: targets.macos }),
    EXACT_LIB_DIR: libDir,
    EXACT_LIB: crate.replace(/-/g, '_'),
    EXACT_APP_COMPOSITION: composition,
  };
  // The products: the standalone app, and with --host the sample host too
  // (LLP 1031 D10 — the fixture the smoke drives).
  const products = [ios ? 'ExactIOS' : 'ExactMac', ...(args.includes('--host') ? [ios ? 'ExactHostIOS' : 'ExactHostMac'] : [])];
  const product = products[0];
  // swift build does not see the Rust archive change; drop the executables so
  // they relink against the archive cargo just built (a relink is ~0.4 s).
  const swiftBuildRoot = paths.scratch;
  const binDir = mkdtempSync(resolve(paths.namespace, '.products-'));
  cleanup.push(binDir);
  // One `swift build` per product: given two `--product` flags SwiftPM
  // builds only the last; the second build is incremental and quick.
  const swiftArgs = ['build', '-c', 'release', '--scratch-path', swiftBuildRoot];
  if (ios) {
    swiftArgs.push(
      '--triple', iosTripleFor(app, device, tv),
      '--sdk', sdk,
      '-Xcc', '-isysroot', '-Xcc', sdk,
      '-Xlinker', '-syslibroot', '-Xlinker', sdk,
      // SwiftPM leaves SDKROOT naming the *host* SDK — it compiled Package.swift
      // for macOS — in the environment of every tool it then spawns, and its link
      // step drives clang with `--sysroot`, which is not the flag clang reads on
      // Darwin: with no `-isysroot` of its own clang takes SDKROOT instead and
      // links an iPhone target against a MacOSX sysroot. Naming it explicitly at
      // the linker driver is what closes it (`-Xcc` reaches only compiles).
      '-Xswiftc', '-Xclang-linker', '-Xswiftc', '-isysroot',
      '-Xswiftc', '-Xclang-linker', '-Xswiftc', sdk,
    );
    // `designRequiresCompatibility`: the link records the iOS 18 SDK (macOS's
    // case below says why); this later `-platform_version` wins.
    if (designCompatible(app, 'ios')) swiftArgs.push('-Xlinker', '-platform_version', '-Xlinker', destination, '-Xlinker', targets.ios, '-Xlinker', COMPATIBLE_SDK.ios);
  } else {
    // The same `--sysroot` on macOS: clang reads no SDK version from it, so the
    // link recorded the deployment target as the SDK (`sdk 14.0`), and AppKit,
    // which keys its macOS 26 design on the recorded SDK, drew every Exact app
    // as on macOS 14. `-isysroot` records the SDK the app is built with.
    swiftArgs.push('-Xswiftc', '-Xclang-linker', '-Xswiftc', '-isysroot', '-Xswiftc', '-Xclang-linker', '-Xswiftc', sdk);
    // `designRequiresCompatibility`: the earlier design, by the one lever macOS
    // 27 keeps (it ignores UIDesignRequiresCompatibility) — the link records
    // the macOS 15 SDK, the last before the new design; this later
    // `-platform_version` wins over the driver's.
    if (designCompatible(app, 'macos')) swiftArgs.push('-Xlinker', '-platform_version', '-Xlinker', 'macos', '-Xlinker', targets.macos, '-Xlinker', COMPATIBLE_SDK.macos);
  }
  // SwiftPM owns its output layout. Swift Build and the native build system
  // use different directories; ask with the same destination arguments.
  const located = read('swift', [...swiftArgs, '--show-bin-path'], { cwd: pkg, env });
  if (located.status !== 0) throw new Error(`swift output path: ${located.stderr}`);
  const swiftBinDir = located.stdout.trim();
  if (!swiftBinDir || !isAbsolute(swiftBinDir)) throw new Error('swift returned no absolute binary output path');
  for (const p of products) rmSync(resolve(swiftBinDir, p), { force: true });
  for (const p of products) {
    const productArgs = [...swiftArgs];
    if (ios && !device) {
      // Simulator Security reads entitlements from the Mach-O text section.
      // Device-style entitlements in its ad-hoc signature can prevent launch.
      const ent = resolve(swiftBuildRoot, `${p}-entitlements.plist`);
      mkdirSync(swiftBuildRoot, {recursive: true});
      writeFileSync(ent, entitlements({...app, id: p === 'ExactHostIOS' ? `${app.id}.host` : app.id}, null, true, bakedCompat.reach));
      productArgs.push('-Xlinker', '-sectcreate', '-Xlinker', '__TEXT',
        '-Xlinker', '__entitlements', '-Xlinker', ent);
    }
    runApple('swift', [...productArgs, '--product', p], { cwd: pkg, env });
    const executable = resolve(binDir, p);
    copyFileSync(resolve(swiftBinDir, p), executable);
    assertAppleIdentity(app, executable, bakedCompat.id);
    const platform = ios ? 'ios' : 'macos';
    assertLinkedSdk(executable, designCompatible(app, platform) ? COMPATIBLE_SDK[platform] : read('xcrun', ['--sdk', sdkName, '--show-sdk-version']).stdout.trim());
  }
  // The iframe arm (@ref LLP 1020 D3): the only artifact that links WebKit.
  // It is built beside the presenter but never linked into it; WebModule.swift
  // dlopens this file at the first iframe create commit.
  // `--sdk` and not a bare `xcrun`: xcrun exports SDKROOT for the tool it runs,
  // and the default is macosx — the same MacOSX-sysroot-for-an-iPhone-target the
  // presenter's link step hits above. The module cache stays in this app-owned
  // Swift scratch while the completed dylib remains invocation-private.
  const webArgs = ['--sdk', sdkName, 'swiftc', '-module-cache-path', resolve(swiftBuildRoot, 'webarm-module-cache'), '-parse-as-library', '-emit-library', '-O', '-module-name', 'ExactWebArm', resolve(root, 'host/apple/webarm/WebArm.swift'), '-o', webBuilt, '-framework', 'WebKit'];
  if (ios) {
    webArgs.push('-target', iosTripleFor(app, device, tv), '-sdk', sdk);
  } else {
    webArgs.push('-target', `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macos${targets.macos}`);
  }
  // Each arm is one Swift file: compile it once per source, arguments and
  // compiler, kept in the scratch path. Rebuilt into a fresh directory, the
  // two cost 93 s of every build at load 120.
  const swiftc = read('xcrun', ['--sdk', sdkName, 'swiftc', '--version']).stdout ?? '';
  const arm = (args, source, built, extra = '') => {
    const key = createHash('sha256').update(JSON.stringify([args.map(a => a === built ? '<out>' : a), [].concat(source).map(f => readFileSync(f, 'utf8')), swiftc, extra])).digest('hex').slice(0, 16);
    const dir = resolve(swiftBuildRoot, 'arms'), cached = resolve(dir, `${key}-${basename(built)}`);
    if (!existsSync(cached)) {
      mkdirSync(dir, { recursive: true });
      runApple('xcrun', args.map(a => a === built ? `${cached}.tmp` : a));
      renameSync(`${cached}.tmp`, cached);
      for (const old of readdirSync(dir)) if (old.endsWith(`-${basename(built)}`) && resolve(dir, old) !== cached) rmSync(resolve(dir, old), { force: true });
    }
    copyFileSync(cached, built);
  };
  // tvOS has no WebKit, so no iframe arm there.
  if (!tv) arm(webArgs, resolve(root, 'host/apple/webarm/WebArm.swift'), webBuilt);
  const videoBuilt = resolve(webBuildDir, videoLoadName);
  const videoArgs = webArgs.map(value => value === 'ExactWebArm' ? 'ExactVideoArm' : value === resolve(root, 'host/apple/webarm/WebArm.swift') ? resolve(root, 'host/apple/videoarm/VideoArm.swift') : value === webBuilt ? videoBuilt : value === 'WebKit' ? 'AVKit' : value);
  arm(videoArgs, resolve(root, 'host/apple/videoarm/VideoArm.swift'), videoBuilt);
  // @ref LLP 1024 D3/D8.4 — the app's one module artifact, only when the app
  // has modules (the GPU gate): the host's table glue and the app's own
  // `modules/apple/*.swift`, one dylib under one load name. A release build
  // whose roster names a tag the artifact lacks fails here, named.
  const modulesLoadName = 'libexact_modules.dylib';
  // @ref LLP 1075.003 Q2 — the app's `data-*` words as typed keys, written
  // from app.json `data` beside the glue (built, never committed).
  const dataKeys = resolve(swiftBuildRoot, `ExactDataKeys-${app.id}.swift`);
  if (app.modules.apple.length) writeDataKeys(app, dataKeys);
  const moduleSources = app.modules.apple.length ? [resolve(root, 'host/apple/modules/ExactNativeModule.swift'), dataKeys, ...app.modules.apple] : [];
  const modulesBuilt = moduleSources.length ? resolve(webBuildDir, modulesLoadName) : null;
  // The slice of each `modules/apple/*.xcframework` for this build, read
  // from the xcframework's own Info.plist (`AvailableLibraries`: platform,
  // variant, architectures), never guessed from a directory name; Mac
  // Catalyst is a variant of its own and never matches. A static library's
  // slice gives `-I` its headers, `-L` it and `-l` each archive in it; a
  // framework's slice gives `-F` it and `-framework` its name.
  const frameworkSlice = (fw, forIos, simulator, arch) => {
    const read = spawnSync('plutil', ['-convert', 'json', '-o', '-', resolve(fw, 'Info.plist')], { encoding: 'utf8' });
    if (read.status !== 0) throw new Error(`host/apple: ${basename(fw)} has no readable Info.plist: ${(read.stderr || '').trim()}`);
    let plist; try { plist = JSON.parse(read.stdout || '{}'); } catch (e) { throw new Error(`host/apple: ${basename(fw)}'s Info.plist is not a dictionary: ${e.message}`); }
    const platform = forIos ? 'ios' : 'macos', variant = forIos && simulator ? 'simulator' : undefined;
    const lib = (plist.AvailableLibraries ?? []).find(l => l.SupportedPlatform === platform && (l.SupportedPlatformVariant || undefined) === variant && (l.SupportedArchitectures ?? []).includes(arch));
    if (!lib) throw new Error(`host/apple: ${basename(fw)} has no ${platform}${variant ? ' ' + variant : ''} ${arch} slice in its Info.plist`);
    const dir = resolve(fw, lib.LibraryIdentifier), framework = lib.LibraryPath.endsWith('.framework');
    // The plist names one library; a slice composed by hand may hold more archives beside it.
    const files = framework ? [resolve(dir, lib.LibraryPath, basename(lib.LibraryPath, '.framework'))] : readdirSync(dir).filter(f => /^lib.*\.a$/.test(f)).map(f => resolve(dir, f));
    if (!files.length) throw new Error(`host/apple: ${basename(fw)}'s ${lib.LibraryIdentifier} slice holds no static library (lib*.a) or framework; a dynamic library is not linked into the module`);
    // A slice made without `-headers` (a dependency archive nothing imports) has no HeadersPath and no `-I`.
    const headers = !framework && lib.HeadersPath && existsSync(resolve(dir, lib.HeadersPath)) ? resolve(dir, lib.HeadersPath) : null;
    const args = framework ? ['-F', dir, '-framework', basename(lib.LibraryPath, '.framework')] : [...(headers ? ['-I', headers] : []), '-L', dir, ...files.map(f => `-l${basename(f).slice(3, -2)}`)];
    // Every header file, so an edit in place rebuilds the module too.
    const walk = (d) => readdirSync(d, { withFileTypes: true }).flatMap(e => e.isDirectory() ? walk(resolve(d, e.name)) : [resolve(d, e.name)]);
    return { files, args, stamped: [...files, ...(headers ? walk(headers) : [])] };
  };
  const frameworkArgs = (forIos, simulator, arch) => (app.modules.frameworks ?? []).flatMap(fw => frameworkSlice(fw, forIos, simulator, arch).args);
  // Linker flags the manifest declares for the module artifact
  // (`host.macos.link`, `host.ios.link`; one argument each), for what an
  // archive needs but cannot say: `-lc++` for a static library with C++ inside.
  const linkArgs = (forIos) => app.manifest.host?.[forIos ? 'ios' : 'macos']?.link ?? [];
  // What the arm cache must see change: the flags, and each library's bytes.
  const frameworkStamp = (forIos, simulator, arch) => JSON.stringify([linkArgs(forIos), ...(app.modules.frameworks ?? []).flatMap(fw => frameworkSlice(fw, forIos, simulator, arch).stamped).map(f => { const st = statSync(f); return [f, st.size, st.mtimeMs]; })]);
  const macArch = process.arch === 'arm64' ? 'arm64' : 'x86_64';
  const iosArch = device ? 'arm64' : (iosTriple.startsWith('arm64') ? 'arm64' : 'x86_64');
  const moduleArgs = (sdkFor, targetArgs, out, forIos = false, simulator = false, arch = macArch) => ['--sdk', sdkFor, 'swiftc', '-module-cache-path', resolve(swiftBuildRoot, 'modules-module-cache'), '-parse-as-library', '-emit-library', '-O', '-swift-version', '5', '-module-name', 'ExactAppModules', ...moduleSources, ...frameworkArgs(forIos, simulator, arch), ...linkArgs(forIos), '-o', out, ...targetArgs];
  const macTarget = ['-target', `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macos14.0`];
  if (modulesBuilt) arm(moduleArgs(sdkName, ios ? ['-target', device ? 'arm64-apple-ios17.0' : tv ? iosTripleFor(app, false, true) : iosTriple, '-sdk', sdk] : macTarget, modulesBuilt, ios, ios && !device, ios ? iosArch : macArch), moduleSources, modulesBuilt, frameworkStamp(ios, ios && !device, ios ? iosArch : macArch));
  if (app.modules.tags.length) {
    // The roster the artifact serves, read from its table: a macOS slice (the
    // one this process can load) of the same sources for an iOS build.
    let provided = [];
    // The probe is a macOS build of the same Swift, so an iOS build whose
    // xcframework has no macOS slice cannot be probed: its roster is taken
    // from the manifest, as it is without Bun.
    const probeable = !ios || (app.modules.frameworks ?? []).every(fw => { try { frameworkSlice(fw, false, false, macArch); return true; } catch { return false; } });
    if (modulesBuilt && typeof Bun !== 'undefined' && !probeable) console.warn(`host/apple: an xcframework has no macOS slice, so the iOS module roster is not probed; the manifest's roster stands`);
    if (modulesBuilt && typeof Bun !== 'undefined' && probeable) {
      const probe = ios ? resolve(webBuildDir, 'probe-' + modulesLoadName) : modulesBuilt;
      if (ios) arm(moduleArgs('macosx', macTarget, probe), moduleSources, probe, frameworkStamp(false, false, macArch));
      const { dlopen, read, CString } = import.meta.require('bun:ffi');
      const table = dlopen(probe, { exact_native_abi: { args: [], returns: 'ptr' } }).symbols.exact_native_abi();
      provided = Object.keys(JSON.parse(new CString(read.ptr(table, 8)).toString()));
    } else if (modulesBuilt) console.warn('host/apple: the module roster check needs Bun (bun:ffi); skipped');
    checkModuleRoster(app, modulesBuilt && (typeof Bun === 'undefined' || !probeable) ? app.modules.tags : provided, `host/apple ${ios ? 'iOS' : 'macOS'}`, cargoEnv.EXACT_UPDATE_TRUST === 'production');
  }
  // The SVG island module (@ref LLP 1055.000 §8 ruling 4): exact-svg-raster
  // as its own dylib, never linked into the presenter; SvgIsland.swift
  // dlopens it the first time a mask or filter needs pixel work.
  const svgBuilt = resolve(webBuildDir, svgLoadName);
  const svgTarget = process.env.CARGO_TARGET_DIR ?? resolve(root, 'target');
  // Unstripped: Xcode 27's strip leaves this dylib with a mis-aligned
  // LINKEDIT string pool that dyld refuses (as Cargo.toml says of build
  // scripts), even at `strip = "debuginfo"`.
  runApple('cargo', ['build', '--release', '-p', 'exact-svg-raster', '--lib', '--target', target, '--target-dir', svgTarget, '--manifest-path', resolve(root, 'Cargo.toml')], { cwd: root, env: { ...process.env, ...cargoEnv, CARGO_PROFILE_RELEASE_STRIP: 'false' } });
  copyFileSync(resolve(svgTarget, target, 'release', 'libexact_svg_raster.dylib'), svgBuilt);
  // The Canvas 2D GPU module (@ref LLP 1056 §8.5): exact-canvas-vello as its
  // own dylib, never linked into the presenter; Canvas2DGpu.swift dlopens it
  // the first time a canvas that animates draws. Its shaders are Metal
  // libraries compiled at build time, which needs Xcode's Metal toolchain;
  // without it the app is built without the module (canvases draw with Core
  // Graphics), and this says how to install it.
  const canvasGpuLoadName = 'libexact_canvas_gpu.dylib';
  const metal = read('xcrun', ['-sdk', 'macosx', 'metal', '--version']).status === 0;
  const canvasGpuBuilt = metal ? resolve(webBuildDir, canvasGpuLoadName) : null;
  if (metal) {
    // Unstripped by strip: Xcode 27's strip leaves the iOS module unloadable
    // ("mis-aligned LINKEDIT string pool", as it does the SVG module; found by
    // dlopen in the simulator, 2026-09-30). The crate's build.rs has the linker
    // omit its local symbols instead (-Wl,-x).
    runApple('cargo', ['build', '--release', '-p', 'exact-canvas-vello', '--lib', '--target', target, '--target-dir', svgTarget, '--manifest-path', resolve(root, 'Cargo.toml')], { cwd: root, env: { ...process.env, ...cargoEnv, CARGO_PROFILE_RELEASE_STRIP: 'false' } });
    copyFileSync(resolve(svgTarget, target, 'release', 'libexact_canvas_vello.dylib'), canvasGpuBuilt);
  } else {
    console.warn('host/apple: no Metal toolchain, so no Canvas 2D GPU module: canvases draw with Core Graphics. Install it with `xcodebuild -downloadComponent MetalToolchain`.');
  }
  const t2 = Date.now();
  const bin = resolve(binDir, product);
  const hostPaths = appleArtifacts(app, { destination, composition, trust: cargoEnv.EXACT_UPDATE_TRUST, host: true });
  const publishProducts = () => {
    if (args.includes('--host')) {
      const hostStage = mkdtempSync(resolve(paths.namespace, '.host-'));
      cleanup.push(hostStage);
      cpSync(binDir, hostStage, { recursive: true });
      for (const name of [product, `${product}.app`]) rmSync(resolve(hostStage, name), { recursive: true, force: true });
      for (const name of [hostPaths.product, `${hostPaths.product}.app`]) rmSync(resolve(binDir, name), { recursive: true, force: true });
      placeAppleArtifact(hostStage, hostPaths.products);
    }
    placeAppleArtifact(binDir, paths.products);
  };

  if (!ios) {
    // `--bundle`'s assembled `.app`, when one was asked for: what `--run`
    // then launches, so the running process has the app's bundle identity —
    // its Info.plist, its document types, its Dock tile (LLP 1033 D2).
    let bundlePath = null;
    // Replace the dylib, never overwrite it in place: a running app may still
    // have the old one mapped, and rewriting a mapped, ad-hoc-signed file poisons
    // the kernel's cached signature for that inode — every later dlopen dies with
    // SIGKILL (Code Signature Invalid). A new file is a new inode.
    const gpuDest = resolve(binDir, loadName);
    rmSync(gpuDest, { force: true });
    if (hasGpu) copyFileSync(resolve(libDir, dylib), gpuDest);
    for (const m of moduleDylibs) { rmSync(resolve(binDir, m.load), { force: true }); copyFileSync(resolve(libDir, m.built), resolve(binDir, m.load)); }
    const webDest = resolve(binDir, webLoadName);
    rmSync(webDest, { force: true });
    copyFileSync(webBuilt, webDest);
    rmSync(resolve(binDir, videoLoadName), { force: true });
    copyFileSync(videoBuilt, resolve(binDir, videoLoadName));
    rmSync(resolve(binDir, modulesLoadName), { force: true });
    if (modulesBuilt) copyFileSync(modulesBuilt, resolve(binDir, modulesLoadName));
    rmSync(resolve(binDir, svgLoadName), { force: true });
    copyFileSync(svgBuilt, resolve(binDir, svgLoadName));
    rmSync(resolve(binDir, canvasGpuLoadName), { force: true });
    if (canvasGpuBuilt) {
      copyFileSync(canvasGpuBuilt, resolve(binDir, canvasGpuLoadName));
      run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, canvasGpuLoadName)], { stdio: 'ignore' });
    }
    // The app's kept secrets live in the login keychain, whose ACL trusts the
    // creating app by its code signature (LLP 1018 D7): signed with the team's
    // identity a rebuild keeps them; ad-hoc, every rebuild is a new app and
    // the keychain asks again — before the first frame.

    // The bundle's plist — what a `.app` would carry when one is assembled —
    // is written beside the bare executable under its product's name, never
    // as `Info.plist`: codesign treats an `Info.plist` adjacent to a bare
    // Mach-O as a bundle's and seals the whole directory (169 files), so the
    // next write there — the receipt, another product, another app's plist —
    // fails verification and the binary is killed at launch (Weird Castle's
    // manifest found it). The signing identifier is the app's id, explicit,
    // so two apps built here are two identities to the keychain (LLP 1018 D7).
    rmSync(resolve(binDir, 'Info.plist'), { force: true });
    rmSync(resolve(binDir, '_CodeSignature'), { recursive: true, force: true });
    writeFileSync(resolve(binDir, `${products[0]}-Info.plist`), macInfoPlist(app, { development, reach: bakedCompat.reach }));
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', webDest], { stdio: 'ignore' });
    if (modulesBuilt) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, modulesLoadName)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, svgLoadName)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', app.id, bin], { stdio: 'ignore' });
    for (const p of products.slice(1)) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', `${app.id}.${p.toLowerCase()}`, resolve(binDir, p)], { stdio: 'ignore' });
    // The receipt beside it (LLP 1030 D2).
    writeFileSync(resolve(binDir, 'receipt.json'), receipt(app, { compatibilityId:bakedCompat.id, build:buildReceipt, composition, platform: 'macos', target: process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin', sdk, identity: sha1 ?? 'ad-hoc', profile: null, entitlements: null, gpu: hasGpu ? dylib : null, development }));
    // A local .app gives Launch Services a real owner for development links.
    // It is not a notarized distribution artifact or a public download.
    if (args.includes('--bundle')) {
      const bundleDestination = appleArtifacts(app).bundle;
      const output = resolve(bundleDestination, '..');
      mkdirSync(output, { recursive: true });
      // One stable path per app — `<target>/clients/<source-key>/<id>/macos/<Name>.app` —
      // so `exact run`, `exact install`, Launch Services, and a Dock tile all
      // name the same bundle across rebuilds (LLP 1033 D2). Assembled beside
      // it and moved into place: a half-written bundle is never launchable,
      // and a running app keeps the inodes it already mapped.
      const stage = mkdtempSync(resolve(output, '.build-'));
      cleanup.push(stage);
      const bundle = resolve(stage, `${app.displayName}.app`), contents = resolve(bundle, 'Contents');
      const executables = resolve(contents, 'MacOS'), resources = resolve(contents, 'Resources');
      mkdirSync(executables, { recursive: true });
      mkdirSync(resources);
      for (const file of ['ExactMac', webLoadName, videoLoadName, svgLoadName, ...(canvasGpuBuilt ? [canvasGpuLoadName] : []), ...(modulesBuilt ? [modulesLoadName] : []), ...(hasGpu ? [loadName] : []), ...moduleDylibs.map(m => m.load)]) copyFileSync(resolve(binDir, file), resolve(executables, file));
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development, reach: bakedCompat.reach }));
      copyAppleStaticTrees(paths.capture, resources);
      verifyBakeFiles(bakedCompat, bakedPlan, listAssets(resources, true));
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development, reach: bakedCompat.reach, icon: appIcon(app, resources, 'macos') }));
      writeUsageStrings(bakedCompat.reach, resources);
      copyFileSync(resolve(binDir, 'receipt.json'), resolve(resources, 'receipt.json'));
      // GPU artifacts were signed before their digests entered the baked receipt.
      // Preserve those exact bytes, as the iOS bundle assembly does below.
      for (const file of [webLoadName, videoLoadName, svgLoadName, ...(canvasGpuBuilt ? [canvasGpuLoadName] : []), ...(modulesBuilt ? [modulesLoadName] : [])]) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(executables, file)], { stdio: 'ignore' });
      run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', bundle], { stdio: 'ignore' });
      const placed = bundleDestination;
      assertAppleIdentity(app, resolve(executables, 'ExactMac'), bakedCompat.id);
      placeAppleArtifact(bundle, placed);
      rmSync(stage, { recursive: true, force: true });
      bundlePath = placed;
      console.log(`local client: ${placed}\n  ${development ? `Open this app once to register its opening link for ${development.origins[0]}.` : 'No development opening link: build with --url <dev server URL> to register one.'}`);
    }
    publishProducts();
    release();
    rmSync(webBuildDir, { recursive: true, force: true });
    console.log(`host/apple: ${resolve(paths.products, product).replace(root + '/', '')} (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s; ${sha1 ? 'signed ' + sha1.slice(0, 8) : 'ad-hoc signed'}); GPU: ${gpuNote}; web arm: ${webLoadName}${modulesBuilt ? `; modules: ${modulesLoadName}` : ''}`);
    // A live source is explicit (--url or EXACT_DEV_PLAN). The shared web
    // output may belong to another app, and TypeScript edits publish complete
    // URL generations rather than rewriting its initial app.plan.
    if (args.includes('--run')) spawnSync(bundlePath ? resolve(bundlePath, 'Contents/MacOS/ExactMac') : resolve(paths.products, product), [], { stdio: 'inherit', env: { ...env, ...launchEnv, EXACT_ASSETS: paths.capture } });
    return;
  }

  // The bundle, assembled from scratch (new inodes, see above), with the
  // app's assets (a phone reads no other machine's paths); ad-hoc signed
  // for a simulator, signed with the team's identity, profile, and
  // entitlements for a phone.
  const bundle = resolve(binDir, 'ExactIOS.app');
  mkdirSync(resolve(bundle, 'Frameworks'), { recursive: true });
  copyFileSync(bin, resolve(bundle, product));
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, { development, reach: bakedCompat.reach, tv }));
  // The GPU crate's shaders (LLP 1030 D8): files the presenter registers
  // with the module before a surface is created, never strings in the dylib.
  copyAppleStaticTrees(paths.capture, bundle);
  verifyBakeFiles(bakedCompat, bakedPlan, listAssets(bundle, true));
  // tvOS icons are layered brand assets, which actool's iPhone/iPad icon set does not make; tvOS builds have none yet.
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, { development, reach: bakedCompat.reach, icon: tv ? {} : iosAssets(app, bundle, device, { catalog: !!ipa }), distribution: ipa ? distributionKeys() : null, tv }));
  writeUsageStrings(bakedCompat.reach, bundle);
  if (hasGpu) copyFileSync(resolve(libDir, dylib), resolve(bundle, 'Frameworks', loadName));
  for (const m of moduleDylibs) copyFileSync(resolve(libDir, m.built), resolve(bundle, 'Frameworks', m.load));
  if (!tv) copyFileSync(webBuilt, resolve(bundle, 'Frameworks', webLoadName));
  copyFileSync(videoBuilt, resolve(bundle, 'Frameworks', videoLoadName));
  if (modulesBuilt) copyFileSync(modulesBuilt, resolve(bundle, 'Frameworks', modulesLoadName));
  copyFileSync(svgBuilt, resolve(bundle, 'Frameworks', svgLoadName));
  if (svgFilterBuilt) copyFileSync(svgFilterBuilt, resolve(bundle, svgFilterLibraryName));
  if (canvasGpuBuilt) copyFileSync(canvasGpuBuilt, resolve(bundle, 'Frameworks', canvasGpuLoadName));
  const bundles = [[bundle, false]];
  if (args.includes('--host')) {
    const hostBundle = resolve(binDir, 'ExactHostIOS.app');
    mkdirSync(resolve(hostBundle, 'Frameworks'), { recursive: true });
    copyFileSync(resolve(binDir, 'ExactHostIOS'), resolve(hostBundle, 'ExactHostIOS'));
    // The sample host takes no development link: it would share the scheme.
    writeFileSync(resolve(hostBundle, 'Info.plist'), infoPlist(app, device, { executable: 'ExactHostIOS', id: `${app.id}.host`, name: 'Host (not Exact)', reach: bakedCompat.reach }));
    writeUsageStrings(bakedCompat.reach, hostBundle);
    copyAppleStaticTrees(paths.capture, hostBundle);
    for (const f of readdirSync(resolve(bundle, 'Frameworks'))) copyFileSync(resolve(bundle, 'Frameworks', f), resolve(hostBundle, 'Frameworks', f));
    if (svgFilterBuilt) copyFileSync(svgFilterBuilt, resolve(hostBundle, svgFilterLibraryName));
    bundles.push([hostBundle, true]);
  }
  for (const [assembled, host] of bundles) {
    const id = host ? `${app.id}.host` : app.id;
    const signingProfile = device && !unsigned ? (host ? profile(ph.udid, id) : prof) : null;
    const signingIdentity = signingProfile ? identity(signingProfile.team) : sha1;
    const ent = resolve(binDir, host ? 'host-entitlements.plist' : 'entitlements.plist');
    if (signingProfile) copyFileSync(signingProfile.path, resolve(assembled, 'embedded.mobileprovision'));
    // Unsigned, the re-signer's profile decides; ask for no debugger, as a distribution profile grants none.
    writeFileSync(ent, entitlements({ ...app, id }, signingProfile?.team, signingProfile?.dev ?? !unsigned, bakedCompat.reach));
    verifyBakeFiles(bakedCompat, bakedPlan, listAssets(assembled, true));
    assertAppleIdentity(app, resolve(assembled, host ? 'ExactHostIOS' : 'ExactIOS'), bakedCompat.id);
    writeFileSync(resolve(assembled, 'receipt.json'), receipt(app, { compatibilityId: bakedCompat.id, build: buildReceipt, composition,
      platform: destination, target, sdk, identity: signingIdentity,
      profile: signingProfile ? { name: signingProfile.name, team: signingProfile.team, expires: signingProfile.expires } : null,
      entitlements: readFileSync(ent, 'utf8'), gpu: hasGpu ? dylib : null, development: host ? null : development }));
    if (ipa) for (const [loose, name] of [[webLoadName, 'ExactWeb'], [videoLoadName, 'ExactVideo']]) wrapFramework(resolve(assembled, 'Frameworks'), loose, name, app);
    for (const f of readdirSync(resolve(assembled, 'Frameworks')).filter(f => f !== loadName && !moduleDylibs.some(m => m.load === f))) run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', resolve(assembled, 'Frameworks', f)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', ...(device ? ['--entitlements', ent] : []), assembled], { stdio: 'ignore' });
  }
  publishProducts();
  release();
  if (ipa) {
    const payload = mkdtempSync(resolve(tmpdir(), 'exact-ipa-'));
    cleanup.push(payload);
    cpSync(paths.bundle, resolve(payload, 'Payload', basename(paths.bundle)), { recursive: true, verbatimSymlinks: true });
    mkdirSync(dirname(ipa), { recursive: true });
    rmSync(ipa, { force: true });
    run('ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', resolve(payload, 'Payload'), ipa]);
    console.log(`host/apple: ${ipa} (${prof ? `signed by ${prof.name}` : 'ad-hoc signed, for re-signing'}, cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s${svgFilterBuilt ? '' : '; no SVG filter kernels (no Metal toolchain)'})`);
    return;
  }
  const dev = device ? ph : simulator(args.includes('--sim') ? args[args.indexOf('--sim') + 1] : undefined, { tv });
  for (const [, host] of bundles) {
    const placed = host ? hostPaths.bundle : paths.bundle;
    if (device) {
      if (!ph.reachable) throw new Error(`${ph.name} is not connected; signed bundle retained at ${placed}`);
      run('xcrun', ['devicectl', 'device', 'install', 'app', '--device', ph.udid, placed]);
    } else install(dev, placed, app, host);
  }
  console.log(`host/apple: ${paths.bundle} on ${dev.name} (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s); GPU: ${gpuNote}${ios && !svgFilterBuilt ? '; no SVG filter kernels (no Metal toolchain)' : ''}; web arm: ${webLoadName}${modulesBuilt ? `; modules: ${modulesLoadName} (Frameworks, signed)` : ''}`);
  if (args.includes('--run')) {
    if (device) run('xcrun', deviceLaunchArgs(ph.udid, app.id, launchEnv));
    else {
      showSimulator(dev);
      run('xcrun', ['simctl', 'launch', '--terminate-running-process', dev.udid, app.id], {
        env: { ...process.env, ...(launchEnv.EXACT_DEV_PLAN ? { SIMCTL_CHILD_EXACT_DEV_PLAN: launchEnv.EXACT_DEV_PLAN } : {}), SIMCTL_CHILD_EXACT_ASSETS: paths.capture },
      });
    }
  }
  } finally { for (const path of cleanup.reverse()) rmSync(path, { recursive: true, force: true }); release(); }

}

/** `--test`: the Swift host tests (LLP 1033 D4a). They link `ExactKit`,
 *  which links an app's archive, so cargo builds one first — Caltrain's by
 *  default, any app's by name. Not one of the five checks (`rules/RULES.md`
 *  caps those at five); run it when the host's own behaviour changes. With
 *  `--ios`, the UIKit tests (`*IOSTests.swift`) through xcodebuild on a
 *  simulator: `--sim`/EXACT_SIM, else one that is not running (another
 *  session may be driving a booted one), shut down again only if booted here.
 *  The async lane runs these for commits under host/apple. */
function test(args) {
  const ios = args.includes('--ios');
  const app = resolveApp(args.find((a, i) => !a.startsWith('--') && args[i - 1] !== '--sim'));
  app.prepare?.();
  const crate = app.crate('apple');
  const release = appleBuildLock(app);
  const paths = appleArtifacts(app, { composition: 'embedded' });
  let cargoRelease;
  try {
    const cargoEnv = { ...developmentBuildEnv(), CARGO_TARGET_DIR: app.target, ...(ios ? {
      SDKROOT: read('xcrun', ['--sdk', 'iphonesimulator', '--show-sdk-path']).stdout.trim(),
      MACOSX_DEPLOYMENT_TARGET: '14.0', IPHONEOS_DEPLOYMENT_TARGET: '17.0',
      // Build scripts compile Objective-C++ for the Mac; SDKROOT names the phone's.
      HOST_CXXFLAGS: `${process.env.HOST_CXXFLAGS ?? ''} -isysroot ${read('xcrun', ['--sdk', 'macosx', '--show-sdk-path']).stdout.trim()}`,
    } : {}) };
    const metadata = read('cargo', ['metadata', '--no-deps', '--format-version', '1'], { cwd: app.workspace, env: cargoEnv });
    if (metadata.status !== 0) throw new Error(`cargo metadata: ${metadata.stderr}`);
    const package_ = JSON.parse(metadata.stdout).packages.find(p => p.name === crate);
    const unit = package_ && cargoLibraryTarget(package_);
    if (!unit) throw new Error(`Cargo has no library target for ${crate}`);
    cargoRelease = claimBuildOutput(app, appleCargoClaims(app, ios ? iosTarget : 'host', [unit])[0]);
    run('cargo', ['rustc', '--crate-type', 'staticlib', '--release', '-p', crate, '--lib', ...(ios ? ['--target', iosTarget] : [])], { cwd: app.workspace, env: cargoEnv });
    const libDir = ios ? resolve(app.target, iosTarget, 'release') : resolve(app.target, 'release');
    const env = { ...process.env, EXACT_TESTS: '1', EXACT_LIB_DIR: libDir, EXACT_LIB: unit.name.replace(/-/g, '_'), EXACT_APP_COMPOSITION: 'embedded' };
    // The filter kernels the Metal chain's tests run (no bundle to find them in).
    mkdirSync(paths.namespace, { recursive: true });
    env.EXACT_SVG_METALLIB = svgFilterLibrary(ios ? 'iphonesimulator' : 'macosx', ios ? '17.0' : '14.0', resolve(paths.namespace, svgFilterLibraryName), true);
    if (ios) env.TEST_RUNNER_EXACT_SVG_METALLIB = env.EXACT_SVG_METALLIB;
    if (!ios) {
      // Tests that compile their own Contract source run this compiler, built
      // here as `cargo build` would, never assumed from an earlier build.
      run('cargo', ['build', '-q', '-p', 'contract', '--bin', 'contract', '--manifest-path', resolve(root, 'Cargo.toml')]);
      env.EXACT_CONTRACT = resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'debug', 'contract');
      runApple('swift', ['test', '--scratch-path', resolve(paths.namespace, 'tests')], {
        cwd: pkg, stdio: 'inherit', env: { ...env, MACOSX_DEPLOYMENT_TARGET: '14.0' },
      });
      return;
    }
    // The fixture's plan and hook module (LLP 1075.003 §3.9), built for the
    // tests' simulator so they run its hooks over its routes: the glue, its
    // typed keys, its Swift.
    const fixture = resolveApp('native-fixture'), fixtureDir = resolve(paths.namespace, 'fixture-module');
    writeDataKeys(fixture, resolve(fixtureDir, 'ExactDataKeys.swift'));
    // Its Contract under the identity of the app the tests link, which the
    // runner requires of a plan (an app's plan boots in no other app).
    const source = resolve(fixtureDir, 'app');
    mkdirSync(source, { recursive: true });
    cpSync(resolve(fixture.dir, 'app.contract'), resolve(source, 'app.contract'));
    writeFileSync(resolve(source, 'app.json'), JSON.stringify({ ...fixture.manifest, $schema: undefined, id: app.id, app: { ...fixture.manifest.app, id: app.id } }));
    env.TEST_RUNNER_EXACT_FIXTURE_PLAN = resolve(fixtureDir, 'app.plan');
    run('cargo', ['run', '-q', '-p', 'contract', '--bin', 'contract', '--manifest-path', resolve(root, 'Cargo.toml'), '--',
      'build', resolve(source, 'app.contract'), '-o', env.TEST_RUNNER_EXACT_FIXTURE_PLAN]);
    env.TEST_RUNNER_EXACT_FIXTURE_MODULE = resolve(fixtureDir, 'libexact_modules.dylib');
    runApple('xcrun', ['--sdk', 'iphonesimulator', 'swiftc', '-parse-as-library', '-emit-library', '-O', '-swift-version', '5', '-module-name', 'ExactAppModules',
      '-module-cache-path', resolve(fixtureDir, 'cache'), resolve(root, 'host/apple/modules/ExactNativeModule.swift'), resolve(fixtureDir, 'ExactDataKeys.swift'),
      ...fixture.modules.apple, '-target', iosTriple, '-o', env.TEST_RUNNER_EXACT_FIXTURE_MODULE]);
    const classes = readdirSync(resolve(pkg, 'tests/ExactKitTests')).filter(f => f.endsWith('IOSTests.swift')).map(f => f.slice(0, -'.swift'.length));
    if (!classes.length) { console.log('host/apple: no *IOSTests to run'); return; }
    const pick = args.includes('--sim') ? args[args.indexOf('--sim') + 1] : process.env.EXACT_SIM;
    const before = simulators();
    const idle = before.filter(d => /SimRuntime\.iOS/.test(d.runtime) && /^iPhone \d+ Pro$/.test(d.name) && d.state !== 'Booted');
    const newest = (d) => Number(/iOS-(\d+)-(\d+)/.exec(d.runtime)?.slice(1).join('.') ?? 0);
    const dev = simulator(pick ?? idle.sort((a, b) => newest(b) - newest(a))[0]?.udid);
    const bootedHere = before.find(d => d.udid === dev.udid)?.state !== 'Booted';
    try {
      runApple('xcodebuild', ['test', '-scheme', 'Exact', '-destination', `platform=iOS Simulator,id=${dev.udid}`,
        '-derivedDataPath', resolve(paths.namespace, 'ios-tests'), ...classes.map(c => `-only-testing:ExactKitTests/${c}`)], {
        cwd: pkg, stdio: 'inherit', env: { ...env, IPHONEOS_DEPLOYMENT_TARGET: '17.0' },
      });
    } finally { if (bootedHere) read('xcrun', ['simctl', 'shutdown', dev.udid]); }
  } finally { cargoRelease?.(); release(); }
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  const args = process.argv.slice(2);
  useXcode();
  try { if (args.includes('--test')) test(args); else main(args); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
