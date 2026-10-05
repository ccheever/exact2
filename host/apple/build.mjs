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
//   bun host/apple/build.mjs [crate] --bundle --distribution                 macOS, the bundle `exact release` signs
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
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { tmpdir } from 'node:os';
import { basename, dirname, isAbsolute, resolve } from 'node:path';
import { closeSync, copyFileSync, cpSync, existsSync, linkSync, mkdirSync, mkdtempSync, openSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { DOCUMENT_UTIS, ownDocumentType, HOST_DEV, checkModuleRoster, copyShaders, appleCargoClaims, awaitBuildOutput, cargoLibraryTarget, claimBuildOutput, appSourceKey, bakeOutput, buildBake, contractLast, bakeTarget, developmentBuildEnv, developmentURLScheme, gpuModules, injectedProfiles, resolveApp, verifyBakeFiles } from '../../scripts/app.mjs';
import { copyStaticTreeIfPresent, listAssets } from '../web/serve.mjs';
import { provisionHermesIos } from './hermes.mjs';
import { startSweep } from '../../scripts/sweep.mjs';
import { writeDataKeys } from './data-keys.mjs';
import { appIcon, iosAssets } from './assets.mjs';
import { keptModules } from './modules.mjs';
import { keptCrates } from './crates.mjs';
export { appIcon, iosAssets };
import { deviceLaunchArgs, developmentLaunchEnvironment, identity, macIdentity, phone, profile, showSimulator, simulator, simulators, useXcode } from './devices.mjs';

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
  refuseMixedTargets(cmd, args, `${r.stdout ?? ''}\n${r.stderr ?? ''}`);
  return r;
};
const refuseMixedTargets = (cmd, args, output) => {
  if (/object file .* was built for newer|using sysroot for|incompatible.*sysroot/i.test(output)) {
    console.error(`host/apple: refused mixed Apple deployment targets from ${cmd} ${args[0] ?? ''}`);
    throw new Error('Apple deployment target mismatch');
  }
};
/** A toolchain step started beside the build's own, which block: the Swift
 * host's compile under the app's Rust, the host's Rust modules under the
 * Swift link. Its output goes to `log` (nothing reads a pipe meanwhile);
 * `done` waits for it, says what it said, and refuses what runApple refuses.
 * `repeated` is a step the build runs again in the foreground, which then
 * says its own failure. A build that ends early stops it. */
function startApple(cmd, args, log, opts = {}) {
  const out = openSync(log, 'w');
  const child = spawn(cmd, args, { cwd: root, ...opts, stdio: ['ignore', out, out] });
  closeSync(out);
  const exited = new Promise((settle) => {
    child.once('error', (error) => settle({ status: null, error }));
    child.once('close', (status) => settle({ status }));
  });
  const stop = () => { if (child.exitCode === null && !child.killed) child.kill(); };
  process.once('exit', stop);
  return { stop, async done({ repeated = false } = {}) {
    const r = await exited;
    process.removeListener('exit', stop);
    if (repeated && r.status !== 0) return false;
    const output = readFileSync(log, 'utf8');
    process.stderr.write(output);
    if (r.status !== 0) throw new Error(`${cmd} failed (${r.status ?? r.error?.message})`);
    refuseMixedTargets(cmd, args, output);
    return true;
  } };
}

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
  // The Swift host is the same code for every app, so its compile is shared:
  // one SwiftPM scratch per destination and deployment target, whatever the
  // app, its composition or its trust (a per-app scratch recompiled ExactKit
  // for each app: 45–56 s on an M4, 100 s under load). Only the link is the
  // app's, made under `swiftLock` and copied out to the app's private stage
  // before the lock is released. `scratch` keeps what is the app's alone.
  const swift = resolve(app.target, 'apple-swift', `${destination}-${deploymentTargets(app)[platform]}`);
  return { owner, namespace, target, product, products, composition, scratch: resolve(namespace, 'link'), swift, swiftLock: `${swift}.lock`,
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
    const exported = exportedTypes(app);
    if (exported.length) dict.UTExportedTypeDeclarations = exported;
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

// The types iOS does not declare itself, which the bundle imports.
const IMPORTED = new Set(['net.daringfireball.markdown']);

/** `CFBundleDocumentTypes` from the manifest's `file_handlers` (LLP 1033 D1):
 *  per handler, its system types as `Viewer` and `Alternate`, so declaring
 *  one never takes it away from whatever already owns it, and its app-owned
 *  types (`ownDocumentType`) as the `Editor` and `Owner` an app is of its
 *  own format (studio diary R13). */
export function documentTypes(app) {
  return (app.manifest.file_handlers ?? []).flatMap((handler) => {
    const accept = Object.entries(handler.accept);
    const extensionsOf = (entries) => [...new Set(entries.flatMap(([, e]) => [e].flat()).map((e) => e.replace(/^\./, '')).filter(Boolean))];
    const entry = (entries, types, role, rank) => {
      const extensions = extensionsOf(entries);
      return {
        CFBundleTypeName: handler.name ?? `${app.displayName} document`,
        CFBundleTypeRole: role,
        LSHandlerRank: rank,
        LSItemContentTypes: types,
        ...(extensions.length ? { CFBundleTypeExtensions: extensions } : {}),
      };
    };
    // readManifest refused a type that is neither (DOCUMENT_UTIS, ownDocumentType).
    const system = accept.filter(([mime]) => Object.hasOwn(DOCUMENT_UTIS, mime));
    const own = accept.filter(([mime]) => !Object.hasOwn(DOCUMENT_UTIS, mime));
    return [
      ...(own.length ? [entry(own, own.map(([mime]) => ownDocumentType(app.id, mime).identifier), 'Editor', 'Owner')] : []),
      ...(system.length ? [entry(system, system.map(([mime]) => DOCUMENT_UTIS[mime]), 'Viewer', 'Alternate')] : []),
    ];
  });
}

/** `UTExportedTypeDeclarations` for the app's own formats (studio diary
 *  R13): each type `ownDocumentType` names, with its extensions and MIME
 *  type, so Finder, the open panel and Launch Services know the format is
 *  this app's. An extension another app's type already has on a Mac (Freeform
 *  has `.board`) may still resolve to that one; the hosts' panels and drops
 *  accept the declared extensions whichever type the Mac gives them. */
export function exportedTypes(app) {
  return (app.manifest.file_handlers ?? []).flatMap((handler) => Object.entries(handler.accept)
    .filter(([mime]) => !Object.hasOwn(DOCUMENT_UTIS, mime))
    .map(([mime, extensions]) => {
      const own = ownDocumentType(app.id, mime);
      return {
        UTTypeIdentifier: own.identifier,
        UTTypeDescription: handler.name ?? mime,
        UTTypeConformsTo: own.conformsTo,
        UTTypeTagSpecification: { 'public.filename-extension': [extensions].flat().map((e) => e.replace(/^\./, '')), 'public.mime-type': [mime] },
      };
    }));
}

/** `UTImportedTypeDeclarations` for the types `file_handlers` names that
 *  iOS does not declare itself (Markdown's `net.daringfireball.markdown`):
 *  its extensions and MIME type, conforming to plain text. */
export function importedTypes(app) {
  return (app.manifest.file_handlers ?? []).flatMap((handler) => Object.entries(handler.accept)
    .filter(([mime]) => IMPORTED.has(DOCUMENT_UTIS[mime]))
    .map(([mime, extensions]) => ({
      UTTypeIdentifier: DOCUMENT_UTIS[mime],
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

/** A distributed executable without its local symbols — 3.6 of Caltrain's
 * 13.1 MB on the Mac — which are first written beside it as a dSYM, so a
 * crash report from the field is still symbolicated. `strip` keeps the
 * image's UUID, which is how the dSYM is matched; a relink without symbols
 * would be another UUID. Development builds keep theirs. */
export function stripForDistribution(executable, dsym) {
  mkdirSync(dirname(dsym), { recursive: true });
  rmSync(dsym, { recursive: true, force: true });
  // dsymutil reports each symbol it could not find in an object file on stderr; only a failure is said.
  run('dsymutil', [executable, '-o', dsym], { stdio: 'ignore' });
  const before = statSync(executable).size;
  run('strip', ['-x', executable], { stdio: 'ignore' });
  return { dsym, saved: before - statSync(executable).size };
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
  ...(exportedTypes(app).length ? { UTExportedTypeDeclarations: exportedTypes(app) } : {}),
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

/** The receipt a distributed bundle carries: the same, without the list of
 * every file the binary was compiled from (3,600 rows with this machine's
 * paths, 1.4 of the receipt's 1.9 MB), which is the driver's staleness
 * evidence and nothing an installed app reads, and with each Cargo product
 * by its name alone. The whole receipt is written beside the artifact. */
export const shippedReceipt = (text) => {
  const whole = JSON.parse(text), build = whole.build ?? {};
  return JSON.stringify({ ...whole, build: { ...build, binary: { sha256: build.binary?.sha256 },
    products: (build.products ?? []).map((product) => ({ ...product, path: basename(product.path) })) } }, null, 2) + '\n';
};

// ---------------------------------------------------------------- the build

async function main(args) {
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
  // What leaves this machine: an .ipa, or the Mac bundle `exact release` signs and notarises.
  const distribution = !!ipa || args.includes('--distribution');
  if ((unsigned && !ipa) || ipa && (!device || (!unsigned && (!process.env.EXACT_IDENTITY || !process.env.EXACT_PROFILE)) || args.includes('--run') || args.includes('--host'))) {
    console.error('--archive needs --device and EXACT_IDENTITY and EXACT_PROFILE (or --unsigned, which needs --archive), and takes neither --run nor --host');
    process.exitCode = 1; return;
  }
  const app = resolveApp(args.find((a, i) => !a.startsWith('--') && !['--sim', '--phone', '--url', '--archive'].includes(args[i - 1])));
  const release = appleBuildLock(app);
  const cleanup = [], beside = []; // what to remove, and the steps started beside the build, when it ends
  try {
  const crate = app.crate(ios ? 'ios' : 'macos');
  const modules = app.modulesFor(ios ? 'ios' : 'macos');
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
  // A production bake is `release`; any other builds `host-dev` (Cargo.toml),
  // the same optimizations without whole-graph LTO, for the touch-one-line budget.
  // An app outside this repo gets it with the root's other profiles, injected
  // at build (injectedProfiles, LLP 1036.001 D1).
  const cargoProfile = cargoEnv.EXACT_UPDATE_TRUST === 'production' ? 'release' : HOST_DEV;
  // host-dev says `incremental = true`, and Cargo lets an inherited
  // CARGO_INCREMENTAL=0 (another project's shell setup) overrule the profile:
  // a touched kernel line then recompiles it whole, 17 s for 6 (an M5 Pro).
  // The profile is this build's choice, so it keeps it and says so, as
  // developmentBuildEnv does an inherited RUSTUP_TOOLCHAIN.
  if (cargoProfile === HOST_DEV && cargoEnv.CARGO_INCREMENTAL === '0') {
    console.error('ignoring CARGO_INCREMENTAL=0: a host-dev build compiles incrementally (Cargo.toml)');
    cargoEnv.CARGO_INCREMENTAL = '1';
  }
  const cargoLibDir = resolve(app.target, target, cargoProfile);
  // Named Cargo products can alias in external workspaces or two checkouts
  // sharing a target. Claim those names through bake-and-capture only.
  let bakedPlan, paths;
  const development = cargoEnv.EXACT_UPDATE_TRUST === 'development' && args.includes('--url') ? developmentAdmission(app, launchEnv.EXACT_DEV_PLAN) : null;
  cargoEnv.EXACT_BAKE_OUTPUT = bakeOutput(app, cargoEnv);
  if (ios && existsSync(resolve(app.dir, 'app.ts')) && cargoEnv.EXACT_JS_ENGINE !== 'stub') provisionHermesIos(tv ? 'tvos-simulator' : device ? 'ios' : 'ios-simulator');
  // What the bake is expected to decide (the manifest's store level). The
  // shared Swift scratch is the same whatever it decides; the composition and
  // capture directory name the Swift host's environment before the bake ends.
  const expected = appleArtifacts(app, { destination, trust: cargoEnv.EXACT_UPDATE_TRUST });
  const swiftBuildRoot = expected.swift;
  // A development build compiles the Swift host as it compiles its Rust
  // (`host-dev`): optimized, but file by file and incrementally, so an edited
  // Swift file is a 3-second build and not the whole module's 45 (an M4; LLP
  // 1036.000 §6). SwiftPM compiles that way only in its debug configuration,
  // so the optimization is asked for on top of it. A production bake and
  // what is distributed keep the whole-module build, the smaller and faster.
  const swiftWhole = cargoProfile === 'release' || distribution;
  // One `swift build` per product: given two `--product` flags SwiftPM
  // builds only the last; the second build is incremental and quick.
  const swiftArgs = ['build', '-c', swiftWhole ? 'release' : 'debug', '--scratch-path', swiftBuildRoot, ...(swiftWhole ? [] : ['-Xswiftc', '-O'])];
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
  // SwiftPM compiles Package.swift itself for macOS before applying the iOS
  // product triple; an iPhone SDKROOT in its environment breaks that host
  // manifest compile. The target SDK stays in the explicit Swift arguments.
  const swiftEnv = (libDir, composition) => ({
    ...process.env,
    ...(tv ? { TVOS_DEPLOYMENT_TARGET: targets.ios } : ios ? { IPHONEOS_DEPLOYMENT_TARGET: targets.ios } : { MACOSX_DEPLOYMENT_TARGET: targets.macos }),
    EXACT_LIB_DIR: libDir,
    EXACT_LIB: crate.replace(/-/g, '_'),
    EXACT_APP_COMPOSITION: composition,
  });
  // ExactKit is the same for every app and needs nothing of this one's Rust,
  // so its compile starts here and runs beside the bake; only the link waits
  // for the archive. Another app's build compiling the same scratch is
  // SwiftPM's own lock's; the claim below is the link's. `--embed` alone
  // builds no Swift.
  const embedOnly = args.includes('--embed') && !args.includes('--run') && !args.includes('--host');
  // The compiler, by its version: part of the name of what is kept from one build to the next.
  const swiftc = read('xcrun', ['--sdk', sdkName, 'swiftc', '--version']).stdout ?? '';
  // Where SwiftPM puts its products, once it has been asked (below).
  const binAnswer = resolve(swiftBuildRoot, `bin-path-${createHash('sha256').update(JSON.stringify([swiftc, swiftArgs])).digest('hex').slice(0, 16)}`);
  // The simulator's entitlements go in the executable's text section (below); the arguments name the file.
  const entitledArgs = (scratch, p) => ios && !device ? ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', resolve(scratch, `${p}-entitlements.plist`)] : [];
  // A development build whose last link in this scratch was this app's starts the product's build here, where
  // it only started ExactKit's compile: when the bake leaves the archive as it was, that build is the link,
  // and the half second SwiftPM takes to find nothing to do is spent beside Cargo, not after it. It holds the
  // link's claim from here; with another app linking, or anything else, the build is as it was.
  const sole = args.includes('--host') ? null : ios ? 'ExactIOS' : 'ExactMac';
  const early = (() => {
    if (embedOnly || cargoProfile !== HOST_DEV || !sole || !existsSync(binAnswer)) return null;
    const bin = readFileSync(binAnswer, 'utf8');
    let was = null;
    try { was = JSON.parse(readFileSync(resolve(bin, `${sole}.linked`), 'utf8')); } catch { return null; }
    if (was.libDir !== expected.capture || was.lib !== crate.replace(/-/g, '_') || !existsSync(resolve(bin, sole)) || !existsSync(resolve(expected.capture, `lib${was.lib}.a`))) return null;
    if (ios && !device && !existsSync(resolve(expected.scratch, `${sole}-entitlements.plist`))) return null;
    let claim;
    try { claim = claimBuildOutput(app, expected.swiftLock); } catch { return null; }
    const step = startApple('swift', [...swiftArgs, ...entitledArgs(expected.scratch, sole), '--product', sole], resolve(webBuildDir, 'swift-compile.log'),
      { cwd: pkg, env: swiftEnv(expected.capture, expected.composition) });
    beside.push(step);
    return { claim, step };
  })();
  const hostCompile = embedOnly || early ? null : startApple('swift', [...swiftArgs, '--target', 'ExactKit'], resolve(webBuildDir, 'swift-compile.log'),
    { cwd: pkg, env: swiftEnv(expected.capture, expected.composition) });
  if (hostCompile) beside.push(hostCompile);
  // The host's two Rust modules (SVG islands, Canvas 2D on the GPU) are the
  // same for every app too, and start with it, in the app's profile: at
  // `release` the kernel went through one codegen unit and thin LTO, 27 of
  // the 35 s a touched kernel line cost (LLP 1036.000 §5, §7). Their build
  // directory is their own, since Cargo runs one build at a time in a
  // directory and they would wait for the whole bake; the kernel is compiled
  // for both at once. Unstripped: Xcode 27's strip leaves the SVG dylib with
  // a mis-aligned LINKEDIT string pool that dyld refuses (as Cargo.toml says
  // of build scripts) and the iOS canvas module unloadable the same way
  // (2026-09-30); that crate's build.rs omits its local symbols (-Wl,-x).
  // The canvas module's shaders are Metal libraries compiled at build time:
  // without Xcode's Metal toolchain there is none, and Core Graphics draws.
  const moduleTarget = resolve(process.env.CARGO_TARGET_DIR ?? resolve(root, 'target'), 'apple-modules');
  const moduleLibDir = resolve(moduleTarget, target, cargoProfile);
  mkdirSync(moduleTarget, { recursive: true });
  startSweep(moduleTarget);
  const metalTools = read('xcrun', ['-sdk', 'macosx', 'metal', '--version']), metal = metalTools.status === 0;
  if (!metal) console.warn('host/apple: no Metal toolchain, so no Canvas 2D GPU module: canvases draw with Core Graphics. Install it with `xcodebuild -downloadComponent MetalToolchain`.');
  // A development build takes a module some checkout of this machine has
  // already compiled from these bytes, and keeps each one it compiles itself
  // (modules.mjs): a fresh checkout's first build is 106 → 79 s on the M4.
  const moduleEnv = { ...process.env, ...cargoEnv, [`CARGO_PROFILE_${cargoProfile.toUpperCase().replace(/-/g, '_')}_STRIP`]: 'false' };
  const kept = cargoProfile === HOST_DEV ? keptModules({ root, moduleTarget, target, profile: cargoProfile, env: moduleEnv, sdk, metal: metalTools.stdout }) : null;
  const moduleLib = {};
  // And a target directory that has compiled nothing starts with the registry
  // crates this machine has compiled, which Cargo takes or not by its own
  // fingerprints (crates.mjs): a second checkout's first build 78 → 57 s.
  const keptRegistry = cargoProfile === HOST_DEV ? keptCrates({ root, env: moduleEnv, profile: cargoProfile }) : null;
  keptRegistry?.take(app.target, 'app');
  const buildModules = (crates, log) => {
    const started = Date.now(), compile = crates.filter(crate => !(moduleLib[crate] = kept?.find(crate)));
    for (const crate of compile) moduleLib[crate] = resolve(moduleLibDir, `lib${crate.replaceAll('-', '_')}.dylib`);
    if (compile.length) keptRegistry?.take(moduleTarget, 'modules');
    const step = compile.length ? startApple('sh', ['-c', 'profile=$1 target=$2 dir=$3 manifest=$4; shift 4; for crate; do cargo build --profile "$profile" -p "$crate" --lib --target "$target" --target-dir "$dir" --manifest-path "$manifest" || exit $?; done',
      'host-modules', cargoProfile, target, moduleTarget, resolve(root, 'Cargo.toml'), ...compile], resolve(webBuildDir, log), { env: moduleEnv }) : null;
    if (step) beside.push(step);
    return { async done() { await step?.done(); for (const crate of compile) kept?.keep(crate, started); if (compile.length) keptRegistry?.keep(moduleTarget, 'modules', resolve(root, 'Cargo.lock')); } };
  };
  // A production binary whose plan is its own for good (store level 0) may
  // reach neither module, and then compiles neither: its build waits for the
  // bake to say (below). Every other build starts both now.
  const fixedPlan = cargoProfile === 'release' && expected.composition === 'embedded';
  const hostModules = embedOnly || fixedPlan ? null : buildModules(['exact-svg-raster', ...(metal ? ['exact-canvas-vello'] : [])], 'host-modules.log');
  const buildReceipt = contractLast(() => buildBake(app, ios ? 'ios' : 'macos', target, { env: cargoEnv, profile: cargoProfile, prepareGpu(product) {
    // Cargo puts its own unsigned file back on every build, and a signature
    // carries its signing time: signing in place made the app's bake (which
    // names this product's digest) run on every build. Sign a copy beside it,
    // again only when Cargo's bytes or the identity change.
    const signed = resolve(dirname(product), 'signed', basename(product)), record = `${signed}.source`;
    // A macOS distribution build signs it as it will ship — Developer ID
    // (`exact release` passes EXACT_IDENTITY), hardened runtime, a secure
    // timestamp — because the bake names these bytes and the host refuses a
    // module whose digest differs: a re-signature after the bake is a refusal.
    // `exact release --check` signs ad hoc (`-`), which takes no timestamp.
    const shipping = distribution && !ios;
    const signing = shipping ? ['--options', 'runtime', (sha1 ?? '-') === '-' ? '--timestamp=none' : '--timestamp'] : ['--timestamp=none'];
    const source = `${createHash('sha256').update(readFileSync(product)).digest('hex')} ${sha1 ?? '-'}${shipping ? ' runtime' : ''}\n`;
    // An ad-hoc signature (`-`: every simulator build) names no certificate
    // to require; asked for one, the verification failed, and the copy was
    // signed again by every build, which reran the app's bake and compile.
    const current = existsSync(signed) && existsSync(record) && readFileSync(record, 'utf8') === source
      && read('codesign', ['--verify', '--strict', ...(sha1 && sha1 !== '-' ? ['-R', `=certificate leaf = H"${sha1}"`] : []), signed]).status === 0;
    if (!current) {
      mkdirSync(dirname(signed), { recursive: true });
      copyFileSync(product, signed);
      run('codesign', ['--force', '--sign', sha1 ?? '-', ...signing, signed], {stdio:'ignore'});
      writeFileSync(record, source);
    }
    return signed;
  }, capture(buildReceipt) {
    const composition = buildReceipt.compat.inputs?.store?.L === '0' ? 'embedded' : 'updating';
    paths = appleArtifacts(app, { destination, composition, trust: cargoEnv.EXACT_UPDATE_TRUST });
    mkdirSync(paths.namespace, { recursive: true });
    const capture = mkdtempSync(resolve(paths.namespace, '.capture-'));
    cleanup.push(capture);
    // SwiftPM links again whenever the archive is a new file, and a capture's copy of it was one on every build
    // (0.5 s with nothing changed). A development capture takes a product whose bytes the last one holds from it,
    // the same file; `held` says which bytes those were.
    const heldAt = resolve(paths.namespace, 'captured-products.json'), holds = {};
    let held = {};
    if (cargoProfile === HOST_DEV) try { held = JSON.parse(readFileSync(heldAt, 'utf8')); } catch { /* the first capture */ }
    const take = (source, file) => {
      const product = buildReceipt.products.find(p => p.path === source), was = resolve(paths.capture, file);
      holds[file] = product?.sha256 ?? null;
      if (product && held[file] === product.sha256 && existsSync(was) && statSync(was).size === product.bytes) linkSync(was, resolve(capture, file));
      else captureAppleProduct(buildReceipt, source, resolve(capture, file));
    };
    take(resolve(cargoLibDir, `lib${crate.replace(/-/g, '_')}.a`), `lib${crate.replace(/-/g, '_')}.a`);
    // GPU products are the signed copies prepareGpu made (the primary and each module).
    for (const file of [...(hasGpu ? [dylib] : []), ...moduleDylibs.map(m => m.built)]) take(resolve(cargoLibDir, 'signed', file), file);
    bakedPlan = readFileSync(resolve(cargoEnv.EXACT_BAKE_OUTPUT, `${ios ? 'ios' : 'macos'}-${target}.plan`));
    copyAppleStaticTrees(app.dir, capture, [['assets', 'assets'], ['deck', 'deck']]);
    copyShaders(app, resolve(capture, 'shaders'));
    if (buildReceipt.rust) copyStaticTreeIfPresent(buildReceipt.rust, resolve(capture, 'rust'));
    verifyBakeFiles(buildReceipt.compat, bakedPlan, listAssets(capture, true));
    placeAppleArtifact(capture, paths.capture);
    writeFileSync(heldAt, JSON.stringify(holds));
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
  // The host's loaded modules only where the plan can reach them (LLP 1047
  // D1's loaded tier; the bake's `loads`, by the runner's own rule for each):
  // a production binary at store level 0 runs the plan it was baked with and
  // no other, so a module that plan cannot reach is neither compiled nor
  // bundled. The canvas module alone is 3.9 MB of every bundle. Any other
  // build carries all four: a development plan or a later bundle may reach
  // one. Nothing is refused for a module left out: each loader says by name
  // that its module is absent, and a canvas then draws with Core Graphics.
  const reaches = buildReceipt.graph.loads;
  const settled = cargoProfile === 'release' && level === '0' && Array.isArray(reaches);
  const carries = (module) => !settled || reaches.includes(module);
  const leftOut = settled ? ['canvas', 'svg', 'video', 'web'].filter((module) => !reaches.includes(module)) : [];
  if (leftOut.length) console.log(`host/apple: the plan cannot change (production, store level 0) and reaches no ${leftOut.join(', ')} module: left out of this build`);
  const canvasGpu = metal && carries('canvas');
  const lateCrates = fixedPlan && !embedOnly ? [...(carries('svg') ? ['exact-svg-raster'] : []), ...(canvasGpu ? ['exact-canvas-vello'] : [])] : [];
  const lateModules = lateCrates.length ? buildModules(lateCrates, 'late-modules.log') : null;
  const env = swiftEnv(libDir, composition);
  // The products: the standalone app, and with --host the sample host too
  // (LLP 1031 D10 — the fixture the smoke drives).
  const products = [ios ? 'ExactIOS' : 'ExactMac', ...(args.includes('--host') ? [ios ? 'ExactHostIOS' : 'ExactHostMac'] : [])];
  const product = products[0];
  // The scratch is shared by every app of this destination (appleArtifacts);
  // `linkRoot` is this app's own, for what names it.
  const linkRoot = paths.scratch;
  mkdirSync(linkRoot, { recursive: true });
  rmSync(resolve(paths.namespace, 'swift'), { recursive: true, force: true }); // the per-app scratch this replaced
  const binDir = mkdtempSync(resolve(paths.namespace, '.products-'));
  cleanup.push(binDir);
  // The shared scratch links one app at a time: another app's build of this
  // destination waits here (seconds once ExactKit is compiled), and the
  // executable is in this build's private stage before the claim is let go.
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
  // two cost 93 s of every build at load 120. The host's own arms are kept
  // in the shared scratch, one compile for every app; an app's module
  // artifact (`own`) in its own directory, so two apps never evict each
  // other's. Each is written under this process's name and renamed, since
  // another app's build may be compiling the same arm. A compile starts here,
  // beside the Swift link, and what is returned waits for it and places it.
  const arm = (args, source, built, extra = '', own = false) => {
    const key = createHash('sha256').update(JSON.stringify([args.map(a => a === built ? '<out>' : a), [].concat(source).map(f => readFileSync(f, 'utf8')), swiftc, extra])).digest('hex').slice(0, 16);
    const dir = resolve(own ? linkRoot : swiftBuildRoot, 'arms'), cached = resolve(dir, `${key}-${basename(built)}`);
    if (existsSync(cached)) return async () => copyFileSync(cached, built);
    mkdirSync(dir, { recursive: true });
    const making = `${cached}.${process.pid}.tmp`;
    const compile = startApple('xcrun', args.map(a => a === built ? making : a), resolve(webBuildDir, `arm-${beside.length}.log`));
    beside.push(compile);
    return async () => {
      await compile.done();
      renameSync(making, cached);
      for (const old of readdirSync(dir)) if (old.endsWith(`-${basename(built)}`) && resolve(dir, old) !== cached) rmSync(resolve(dir, old), { force: true });
      copyFileSync(cached, built);
    };
  };
  const arms = [];
  // tvOS has no WebKit, so no iframe arm there.
  const hasWeb = !tv && carries('web'), hasVideo = carries('video'), hasSvg = carries('svg');
  if (hasWeb) arms.push(arm(webArgs, resolve(root, 'host/apple/webarm/WebArm.swift'), webBuilt));
  const videoBuilt = resolve(webBuildDir, videoLoadName);
  const videoArgs = webArgs.map(value => value === 'ExactWebArm' ? 'ExactVideoArm' : value === resolve(root, 'host/apple/webarm/WebArm.swift') ? resolve(root, 'host/apple/videoarm/VideoArm.swift') : value === webBuilt ? videoBuilt : value === 'WebKit' ? 'AVKit' : value);
  if (hasVideo) arms.push(arm(videoArgs, resolve(root, 'host/apple/videoarm/VideoArm.swift'), videoBuilt));
  // @ref LLP 1024 D3/D8.4 — the app's one module artifact, only when the app
  // has modules (the GPU gate): the host's table glue and the app's own
  // `modules/apple/*.swift`, one dylib under one load name. A release build
  // whose roster names a tag the artifact lacks fails here, named.
  const modulesLoadName = 'libexact_modules.dylib';
  // @ref LLP 1075.003 Q2 — the app's `data-*` words as typed keys, written
  // from app.json `data` beside the glue (built, never committed).
  const dataKeys = resolve(linkRoot, `ExactDataKeys-${app.id}.swift`);
  if (modules.apple.length) writeDataKeys(app, dataKeys);
  const moduleSources = modules.apple.length ? [resolve(root, 'host/apple/modules/ExactNativeModule.swift'), dataKeys, ...modules.apple] : [];
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
  const frameworkArgs = (forIos, simulator, arch) => (modules.frameworks ?? []).flatMap(fw => frameworkSlice(fw, forIos, simulator, arch).args);
  // Linker flags the manifest declares for the module artifact
  // (`host.macos.link`, `host.ios.link`; one argument each), for what an
  // archive needs but cannot say: `-lc++` for a static library with C++ inside.
  const linkArgs = (forIos) => app.manifest.host?.[forIos ? 'ios' : 'macos']?.link ?? [];
  // What the arm cache must see change: the flags, and each library's bytes.
  const frameworkStamp = (forIos, simulator, arch) => JSON.stringify([linkArgs(forIos), ...(modules.frameworks ?? []).flatMap(fw => frameworkSlice(fw, forIos, simulator, arch).stamped).map(f => { const st = statSync(f); return [f, st.size, st.mtimeMs]; })]);
  const macArch = process.arch === 'arm64' ? 'arm64' : 'x86_64';
  const iosArch = device ? 'arm64' : (iosTriple.startsWith('arm64') ? 'arm64' : 'x86_64');
  const moduleArgs = (sdkFor, targetArgs, out, forIos = false, simulator = false, arch = macArch) => ['--sdk', sdkFor, 'swiftc', '-module-cache-path', resolve(swiftBuildRoot, 'modules-module-cache'), '-parse-as-library', '-emit-library', '-O', '-swift-version', '5', '-module-name', 'ExactAppModules', ...moduleSources, ...frameworkArgs(forIos, simulator, arch), ...linkArgs(forIos), '-o', out, ...targetArgs];
  const macTarget = ['-target', `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macos14.0`];
  if (modulesBuilt) arms.push(arm(moduleArgs(sdkName, ios ? ['-target', device ? 'arm64-apple-ios17.0' : tv ? iosTripleFor(app, false, true) : iosTriple, '-sdk', sdk] : macTarget, modulesBuilt, ios, ios && !device, ios ? iosArch : macArch), moduleSources, modulesBuilt, frameworkStamp(ios, ios && !device, ios ? iosArch : macArch), true));
  // The roster the artifact serves is read from its table: a macOS slice (the
  // one this process can load) of the same sources for an iOS build. That
  // probe is a macOS build of the same Swift, so an iOS build whose
  // xcframework has no macOS slice cannot be probed: its roster is taken from
  // the manifest, as it is without Bun.
  const probeable = !ios || (modules.frameworks ?? []).every(fw => { try { frameworkSlice(fw, false, false, macArch); return true; } catch { return false; } });
  const probed = modulesBuilt && modules.tags.length && typeof Bun !== 'undefined' && probeable;
  const probe = ios ? resolve(webBuildDir, 'probe-' + modulesLoadName) : modulesBuilt;
  if (probed && ios) arms.push(arm(moduleArgs('macosx', macTarget, probe), moduleSources, probe, frameworkStamp(false, false, macArch), true));
  await hostCompile?.done({ repeated: true });
  // The early build stands when it succeeded and the bake came out as it was expected to.
  const linkedEarly = early ? await early.step.done({ repeated: true }) && expected.capture === libDir && expected.composition === composition && expected.scratch === paths.scratch : false;
  let stripped = null;
  mkdirSync(dirname(paths.swiftLock), { recursive: true });
  if (early && expected.swiftLock !== paths.swiftLock) early.claim();
  const releaseSwift = early && expected.swiftLock === paths.swiftLock ? early.claim
    : awaitBuildOutput(app, paths.swiftLock, (owner) => console.log(`host/apple: waiting for the Swift build of ${owner} in ${swiftBuildRoot.replace(root + '/', '')}`));
  try {
    // SwiftPM owns its output layout. Swift Build and the native build system
    // use different directories; ask with the same destination arguments —
    // once for each compiler and arguments, since asking is half a second of
    // every build: the answer is kept in the scratch it names.
    const answer = binAnswer;
    let swiftBinDir = existsSync(answer) ? readFileSync(answer, 'utf8') : '';
    if (!swiftBinDir || !existsSync(swiftBinDir)) {
      const located = read('swift', [...swiftArgs, '--show-bin-path'], { cwd: pkg, env });
      if (located.status !== 0) throw new Error(`swift output path: ${located.stderr}`);
      swiftBinDir = located.stdout.trim();
      if (!swiftBinDir || !isAbsolute(swiftBinDir)) throw new Error('swift returned no absolute binary output path');
      mkdirSync(swiftBuildRoot, { recursive: true });
      writeFileSync(answer, swiftBinDir);
    }
    const archive = buildReceipt.products.find(made => made.path === resolve(cargoLibDir, `lib${crate.replace(/-/g, '_')}.a`))?.sha256;
    for (const p of products) {
      const productArgs = [...swiftArgs, ...entitledArgs(linkRoot, p)];
      let entitled = null;
      if (ios && !device) {
        // Simulator Security reads entitlements from the Mach-O text section.
        // Device-style entitlements in its ad-hoc signature can prevent launch.
        entitled = entitlements({...app, id: p === 'ExactHostIOS' ? `${app.id}.host` : app.id}, null, true, bakedCompat.reach);
        const ent = resolve(linkRoot, `${p}-entitlements.plist`);
        // Written only when it differs: the early build may be reading it.
        if (!existsSync(ent) || readFileSync(ent, 'utf8') !== entitled) writeFileSync(ent, entitled);
      }
      // swift build does not see the Rust archive (or that plist) change, so the executable is dropped to be
      // linked again: 0.4 s, and it was every build's. A development build drops only one linked from other
      // bytes, which `<product>.linked` beside it says: the archive's hash, whose it is, and the plist.
      const linked = resolve(swiftBinDir, `${p}.linked`), from = JSON.stringify({ archive: archive ?? null, libDir, lib: env.EXACT_LIB ?? null, entitled });
      const same = cargoProfile === HOST_DEV && archive && existsSync(resolve(swiftBinDir, p)) && existsSync(linked) && readFileSync(linked, 'utf8') === from;
      if (!same) for (const stale of [p, `${p}.linked`]) rmSync(resolve(swiftBinDir, stale), { force: true });
      // The early build was this one's when nothing it linked from has changed.
      if (!(same && linkedEarly && p === sole)) runApple('swift', [...productArgs, '--product', p], { cwd: pkg, env });
      writeFileSync(linked, from);
      const executable = resolve(binDir, p);
      copyFileSync(resolve(swiftBinDir, p), executable);
      assertAppleIdentity(app, executable, bakedCompat.id);
      const platform = ios ? 'ios' : 'macos';
      assertLinkedSdk(executable, designCompatible(app, platform) ? COMPATIBLE_SDK[platform] : read('xcrun', ['--sdk', sdkName, '--show-sdk-version']).stdout.trim());
      if (ipa) stripped = stripForDistribution(executable, `${ipa.replace(/\.ipa$/, '')}.dSYM`);
    }
  } finally { releaseSwift(); }
  const tSwift = Date.now();
  for (const placed of arms) await placed();
  if (modules.tags.length) {
    let provided = [];
    if (modulesBuilt && typeof Bun !== 'undefined' && !probeable) console.warn(`host/apple: an xcframework has no macOS slice, so the iOS module roster is not probed; the manifest's roster stands`);
    if (probed) {
      const { dlopen, read, CString } = import.meta.require('bun:ffi');
      const table = dlopen(probe, { exact_native_abi: { args: [], returns: 'ptr' } }).symbols.exact_native_abi();
      provided = Object.keys(JSON.parse(new CString(read.ptr(table, 8)).toString()));
    } else if (modulesBuilt) console.warn('host/apple: the module roster check needs Bun (bun:ffi); skipped');
    checkModuleRoster(app, modulesBuilt && (typeof Bun === 'undefined' || !probeable) ? modules.tags : provided, `host/apple ${ios ? 'iOS' : 'macOS'}`, cargoEnv.EXACT_UPDATE_TRUST === 'production');
  }
  const tArms = Date.now();
  // The SVG island module (@ref LLP 1055.000 §8 ruling 4: exact-svg-raster,
  // which SvgIsland.swift dlopens the first time a mask or filter needs pixel
  // work) and the Canvas 2D GPU module (@ref LLP 1056 §8.5: exact-canvas-vello,
  // which Canvas2DGpu.swift dlopens the first time a canvas that animates
  // draws), started above: each its own dylib, never linked into the presenter.
  await hostModules?.done();
  await lateModules?.done();
  keptRegistry?.keep(app.target, 'app', resolve(app.workspace, 'Cargo.lock'));
  const svgBuilt = resolve(webBuildDir, svgLoadName);
  if (hasSvg) copyFileSync(moduleLib['exact-svg-raster'], svgBuilt);
  const canvasGpuLoadName = 'libexact_canvas_gpu.dylib';
  const canvasGpuBuilt = canvasGpu ? resolve(webBuildDir, canvasGpuLoadName) : null;
  if (canvasGpu) copyFileSync(moduleLib['exact-canvas-vello'], canvasGpuBuilt);
  const t2 = Date.now();
  // Where the build's time went, said at its end: the app's Rust and bake; the
  // Swift host's link, and what of its compile the bake did not cover; the
  // arms and the app's module artifact; what of the host's two Rust modules
  // those did not cover; and assembling, signing and installing.
  const seconds = (ms) => (ms / 1000).toFixed(1);
  const timing = () => `cargo ${seconds(t1 - t0)} s, swift ${seconds(tSwift - t1)} s, arms ${seconds(tArms - tSwift)} s, modules ${seconds(t2 - tArms)} s, package ${seconds(Date.now() - t2)} s`;
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
    const loaded = [...(hasWeb ? [webLoadName] : []), ...(hasVideo ? [videoLoadName] : []), ...(hasSvg ? [svgLoadName] : [])];
    const webDest = resolve(binDir, webLoadName);
    rmSync(webDest, { force: true });
    if (hasWeb) copyFileSync(webBuilt, webDest);
    rmSync(resolve(binDir, videoLoadName), { force: true });
    if (hasVideo) copyFileSync(videoBuilt, resolve(binDir, videoLoadName));
    rmSync(resolve(binDir, modulesLoadName), { force: true });
    if (modulesBuilt) copyFileSync(modulesBuilt, resolve(binDir, modulesLoadName));
    rmSync(resolve(binDir, svgLoadName), { force: true });
    if (hasSvg) copyFileSync(svgBuilt, resolve(binDir, svgLoadName));
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
    if (hasWeb) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', webDest], { stdio: 'ignore' });
    if (modulesBuilt) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, modulesLoadName)], { stdio: 'ignore' });
    if (hasSvg) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, svgLoadName)], { stdio: 'ignore' });
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
      for (const file of ['ExactMac', ...loaded, ...(canvasGpuBuilt ? [canvasGpuLoadName] : []), ...(modulesBuilt ? [modulesLoadName] : []), ...(hasGpu ? [loadName] : []), ...moduleDylibs.map(m => m.load)]) copyFileSync(resolve(binDir, file), resolve(executables, file));
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development, reach: bakedCompat.reach }));
      copyAppleStaticTrees(paths.capture, resources);
      verifyBakeFiles(bakedCompat, bakedPlan, listAssets(resources, true));
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development, reach: bakedCompat.reach, icon: appIcon(app, resources, 'macos') }));
      writeUsageStrings(bakedCompat.reach, resources);
      const whole = readFileSync(resolve(binDir, 'receipt.json'), 'utf8');
      writeFileSync(resolve(resources, 'receipt.json'), distribution ? shippedReceipt(whole) : whole);
      // GPU artifacts were signed before their digests entered the baked receipt.
      // Preserve those exact bytes, as the iOS bundle assembly does below.
      for (const file of [...loaded, ...(canvasGpuBuilt ? [canvasGpuLoadName] : []), ...(modulesBuilt ? [modulesLoadName] : [])]) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(executables, file)], { stdio: 'ignore' });
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
    console.log(`host/apple: ${resolve(paths.products, product).replace(root + '/', '')} (${timing()}; ${sha1 ? 'signed ' + sha1.slice(0, 8) : 'ad-hoc signed'}); GPU: ${gpuNote}; web arm: ${hasWeb ? webLoadName : 'none'}${modulesBuilt ? `; modules: ${modulesLoadName}` : ''}`);
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
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, { development, reach: bakedCompat.reach, icon: tv ? {} : iosAssets(app, bundle, device, { catalog: !!ipa, kept: { dir: resolve(linkRoot, 'assets'), stamp: swiftc } }), distribution: ipa ? distributionKeys() : null, tv }));
  writeUsageStrings(bakedCompat.reach, bundle);
  if (hasGpu) copyFileSync(resolve(libDir, dylib), resolve(bundle, 'Frameworks', loadName));
  for (const m of moduleDylibs) copyFileSync(resolve(libDir, m.built), resolve(bundle, 'Frameworks', m.load));
  if (hasWeb) copyFileSync(webBuilt, resolve(bundle, 'Frameworks', webLoadName));
  if (hasVideo) copyFileSync(videoBuilt, resolve(bundle, 'Frameworks', videoLoadName));
  if (modulesBuilt) copyFileSync(modulesBuilt, resolve(bundle, 'Frameworks', modulesLoadName));
  if (hasSvg) copyFileSync(svgBuilt, resolve(bundle, 'Frameworks', svgLoadName));
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
    const whole = receipt(app, { compatibilityId: bakedCompat.id, build: buildReceipt, composition,
      platform: destination, target, sdk, identity: signingIdentity,
      profile: signingProfile ? { name: signingProfile.name, team: signingProfile.team, expires: signingProfile.expires } : null,
      entitlements: readFileSync(ent, 'utf8'), gpu: hasGpu ? dylib : null, development: host ? null : development });
    writeFileSync(resolve(assembled, 'receipt.json'), ipa ? shippedReceipt(whole) : whole);
    if (ipa) { mkdirSync(dirname(ipa), { recursive: true }); writeFileSync(`${ipa.replace(/\.ipa$/, '')}.receipt.json`, whole); }
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
    console.log(`host/apple: ${ipa} (${prof ? `signed by ${prof.name}` : 'ad-hoc signed, for re-signing'}, ${timing()}${svgFilterBuilt ? '' : '; no SVG filter kernels (no Metal toolchain)'}); symbols: ${stripped.dsym} (${(stripped.saved / 1048576).toFixed(1)} MB off the executable)`);
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
  console.log(`host/apple: ${paths.bundle} on ${dev.name}${dev.udid ? ` ${dev.udid}` : ''} (${timing()}); GPU: ${gpuNote}${ios && !svgFilterBuilt ? '; no SVG filter kernels (no Metal toolchain)' : ''}; web arm: ${hasWeb ? webLoadName : 'none'}${modulesBuilt ? `; modules: ${modulesLoadName} (Frameworks, signed)` : ''}`);
  if (args.includes('--run')) {
    if (device) run('xcrun', deviceLaunchArgs(ph.udid, app.id, launchEnv));
    else {
      showSimulator(dev);
      run('xcrun', ['simctl', 'launch', '--terminate-running-process', dev.udid, app.id], {
        env: { ...process.env, ...(launchEnv.EXACT_DEV_PLAN ? { SIMCTL_CHILD_EXACT_DEV_PLAN: launchEnv.EXACT_DEV_PLAN } : {}), SIMCTL_CHILD_EXACT_ASSETS: paths.capture },
      });
    }
  }
  } finally { for (const step of beside) step.stop(); for (const path of cleanup.reverse()) rmSync(path, { recursive: true, force: true }); release(); }

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
  const crate = app.crate(ios ? 'ios' : 'macos');
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
    // The development profile, as an app's build: `release` put the whole graph through LTO for a test run.
    run('cargo', ['rustc', '--crate-type', 'staticlib', ...injectedProfiles(app), '--profile', HOST_DEV, '-p', crate, '--lib', ...(ios ? ['--target', iosTarget] : [])], { cwd: app.workspace, env: cargoEnv });
    const libDir = ios ? resolve(app.target, iosTarget, HOST_DEV) : resolve(app.target, HOST_DEV);
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
  const failed = (error) => { console.error(error.message); process.exitCode = 1; };
  try { if (args.includes('--test')) test(args); else main(args).catch(failed); }
  catch (error) { failed(error); }
}
