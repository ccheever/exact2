#!/usr/bin/env node
// Build the macOS app — or, with --ios, the iOS app: the app's static
// library (cargo, release), then the presenter (swift build) linked against
// it. Usage:
//   node host/apple/build.mjs [crate=caltrain-apple] [--run]                 macOS
//   node host/apple/build.mjs --ios [crate] [--run] [--sim <udid|name>]        iOS, on a simulator
//   node host/apple/build.mjs --device [crate] [--run] [--phone <udid|name>]   iOS, on a phone
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
// unlocked, paired, Developer Mode on. The simulator helpers are exported
// for scripts/agent.mjs, which launches the same bundle.
import { spawnSync } from 'node:child_process';
import { homedir, tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { resolveApp } from '../../scripts/app.mjs';
import { copyStaticTreeIfPresent } from '../web/serve.mjs';

const root = resolve(new URL('../..', import.meta.url).pathname);
const run = (cmd, args, opts = {}) => { const r = spawnSync(cmd, args, { cwd: root, stdio: 'inherit', ...opts }); if (r.status !== 0) process.exit(r.status ?? 1); return r; };
const read = (cmd, args, opts = {}) => spawnSync(cmd, args, { cwd: root, encoding: 'utf8', ...opts });
// Every Apple toolchain invocation goes through here — cargo, swift build, and
// the webarm swiftc alike: a mixed deployment target or an incompatible sysroot
// is a warning the toolchain prints and then links anyway, so the build fails on
// it here instead. @ref LLP 1008
const runApple = (cmd, args, opts = {}) => {
  const r = spawnSync(cmd, args, { cwd: root, encoding: 'utf8', ...opts });
  process.stdout.write(r.stdout ?? '');
  process.stderr.write(r.stderr ?? '');
  if (r.status !== 0) process.exit(r.status ?? 1);
  const output = `${r.stdout ?? ''}\n${r.stderr ?? ''}`;
  if (/object file .* was built for newer|using sysroot for|incompatible.*sysroot/i.test(output)) {
    console.error(`host/apple: refused mixed Apple deployment targets from ${cmd} ${args[0] ?? ''}`);
    process.exit(1);
  }
  return r;
};

// ---------------------------------------------------------------- iOS: the bundle and the simulator

/** The app's bundle identifier: the manifest's `app.id` (LLP 1030 D2 — derived once, in `scripts/app.mjs`), which was `com.exact.<crate>` before the manifest existed and still is for an app without one. */
export const bundleId = (crate = 'caltrain-apple') => resolveApp(crate).id;
/** The one Swift package (LLP 1031 D6): ExactKit and the four executables. */
export const pkg = resolve(root, 'host/apple');
/** Where `--ios` leaves the assembled app (the simulator's), and `--device` the phone's. */
export const appBundle = resolve(pkg, '.build/ExactIOS.app');
export const deviceBundle = resolve(pkg, '.build/device/ExactIOS.app');
/** The iOS sample host's bundle (LLP 1031 D10), assembled by `--ios --host` beside the app's: bundle id `<app id>.host`. */
export const hostBundle = resolve(pkg, '.build/ExactHostIOS.app');
/** The simulator's Rust target and Swift triple on this machine. */
export const iosTarget = process.arch === 'arm64' ? 'aarch64-apple-ios-sim' : 'x86_64-apple-ios';
export const iosTriple = `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-ios17.0-simulator`;
export const macTriple = `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macosx`;
/** Where SwiftPM leaves a product for a triple: the explicit directory (SwiftPM drops the OS version from the triple's directory name), never the `.build/release` symlink, which flips between the macOS and iOS builds of one package. */
export const productPath = (product, triple = macTriple) => resolve(pkg, '.build', triple.replace(/-ios[\d.]+/, '-ios'), 'release', product);
/** The standalone macOS app (`scripts/agent.mjs`, `metrics.mjs`) and the sample host. */
export const macBinary = productPath('ExactMac');
export const macHostBinary = productPath('ExactHostMac');

/** Copy the app-visible static trees into a private Apple package stage.
 * Every leaf goes through the web host's no-symlink policy; `trees` maps an
 * app-relative source (notably `gpu/shaders`) to its bundle-visible name. */
export function copyAppleStaticTrees(source, target, trees = [['assets', 'assets'], ['deck', 'deck'], ['shaders', 'shaders']]) {
  for (const [from, to] of trees) {
    copyStaticTreeIfPresent(resolve(source, from), resolve(target, to));
  }
}

/** Every available simulator: { udid, name, runtime, state }. */
export function simulators() {
  const r = read('xcrun', ['simctl', 'list', 'devices', 'available', '-j']);
  if (r.status !== 0) throw new Error('xcrun simctl list: ' + r.stderr);
  return Object.entries(JSON.parse(r.stdout).devices).flatMap(([runtime, list]) => list.map((d) => ({ udid: d.udid, name: d.name, runtime, state: d.state })));
}

/** The simulator to use — `pick` (a udid or a name; EXACT_SIM by default), else a booted iPhone, else the iPhone Pro on the newest iOS — booted and waited for. */
export function simulator(pick = process.env.EXACT_SIM) {
  const all = simulators();
  const version = (d) => Number(/iOS-(\d+)-(\d+)/.exec(d.runtime)?.slice(1).join('.') ?? 0);
  const iphones = all.filter((d) => /SimRuntime\.iOS/.test(d.runtime) && /^iPhone/.test(d.name)).sort((a, b) => version(b) - version(a) || a.name.localeCompare(b.name));
  let dev = pick ? all.find((d) => d.udid === pick || d.name === pick) : null;
  if (pick && !dev) throw new Error(`no simulator ${pick} (xcrun simctl list devices available)`);
  dev ??= iphones.find((d) => d.state === 'Booted') ?? iphones.find((d) => /^iPhone \d+ Pro$/.test(d.name)) ?? iphones[0];
  if (!dev) throw new Error('no iPhone simulator; add one in Xcode');
  if (dev.state !== 'Booted') {
    const b = read('xcrun', ['simctl', 'boot', dev.udid]);
    if (b.status !== 0 && !/current state: Booted/.test(b.stderr)) throw new Error('simctl boot: ' + b.stderr);
  }
  const s = read('xcrun', ['simctl', 'bootstatus', dev.udid, '-b']);
  if (s.status !== 0) throw new Error('simctl bootstatus: ' + s.stderr);
  return dev;
}

/** Install the assembled bundle on the simulator. */
export function install(dev, bundle = appBundle) {
  if (!existsSync(bundle)) throw new Error(bundle === appBundle ? 'run node host/apple/build.mjs --ios first' : 'run node host/apple/build.mjs --ios --host first');
  const r = read('xcrun', ['simctl', 'install', dev.udid, bundle]);
  if (r.status !== 0) throw new Error('simctl install: ' + r.stderr);
}

// ---------------------------------------------------------------- a phone: devicectl, a profile, an identity

/** Every phone this Mac knows (devicectl): { id, udid, name, model, os, reachable }. */
export function phones() {
  const out = resolve(mkdtempSync(resolve(tmpdir(), 'exact-devices-')), 'devices.json');
  const r = read('xcrun', ['devicectl', 'list', 'devices', '--json-output', out]);
  if (r.status !== 0) throw new Error('xcrun devicectl list devices: ' + r.stderr);
  const list = JSON.parse(readFileSync(out, 'utf8')).result.devices.map((d) => ({
    id: d.identifier, udid: d.hardwareProperties?.udid, name: d.deviceProperties?.name, model: d.hardwareProperties?.marketingName,
    os: d.deviceProperties?.osVersionNumber, reachable: d.connectionProperties?.tunnelState !== 'unavailable', paired: d.connectionProperties?.pairingState === 'paired',
  }));
  rmSync(resolve(out, '..'), { recursive: true, force: true });
  return list;
}

/** The phone to use: `pick` (a udid or a name; EXACT_PHONE by default), else a reachable phone, else the one phone this Mac knows — the bundle is built and signed for it either way; installing needs it connected (`reachable`). */
export function phone(pick = process.env.EXACT_PHONE) {
  const all = phones();
  const dev = pick ? all.find((d) => d.udid === pick || d.id === pick || d.name === pick) : all.find((d) => d.reachable) ?? (all.length === 1 ? all[0] : null);
  if (!dev) throw new Error(pick ? `no phone ${pick} (xcrun devicectl list devices)` : `no phone is known to this Mac (${all.length ? all.map((d) => `${d.name}, not connected`).join('; ') : 'xcrun devicectl list devices shows none'}): plug one in, unlock it, and trust this Mac`);
  return dev;
}

/**
 * A phone cannot read a plan or asset directory on this Mac. When the caller
 * names the dev server with EXACT_DEV_PLAN, carry that URL into the launched
 * process; the envelope supplies its own complete asset URLs.
 */
export function deviceLaunchArgs(device, id, environment = process.env) {
  const args = ['devicectl', 'device', 'process', 'launch', '--terminate-existing', '--device', device];
  const locator = environment.EXACT_DEV_PLAN;
  if (locator) {
    let url;
    try { url = new URL(locator); }
    catch { throw new Error(`--device cannot open EXACT_DEV_PLAN=${locator} on this Mac; name the dev server's http(s) URL`); }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') throw new Error(`--device requires EXACT_DEV_PLAN to be an http(s) dev-server URL, not ${locator}`);
    args.push('--environment-variables', JSON.stringify({ EXACT_DEV_PLAN: url.href }));
  }
  args.push(id);
  return args;
}

/** A development profile on this Mac covering the phone and the bundle id (the team's wildcard or the id itself), unexpired; EXACT_PROFILE names one. */
export function profile(udid, bundle) {
  if (process.env.EXACT_PROFILE) return decodeProfile(process.env.EXACT_PROFILE);
  const dirs = ['Library/Developer/Xcode/UserData/Provisioning Profiles', 'Library/MobileDevice/Provisioning Profiles'].map((d) => resolve(homedir(), d)).filter(existsSync);
  const found = dirs.flatMap((d) => readdirSync(d).filter((f) => f.endsWith('.mobileprovision')).map((f) => decodeProfile(resolve(d, f))))
    .filter((p) => p.dev && p.expires > new Date() && p.devices.includes(udid) && (p.appId === `${p.team}.*` || p.appId === `${p.team}.${bundle}`))
    .sort((a, b) => b.expires - a.expires);
  if (!found.length) throw new Error(`no development provisioning profile on this Mac covers ${bundle} on this phone (${udid}); run any app on it from Xcode once with team signing, or name one with EXACT_PROFILE`);
  return found[0];
}

/** The fields of a `.mobileprovision` this script reads (it is a CMS-signed XML plist). */
function decodeProfile(path) {
  const xml = read('security', ['cms', '-D', '-i', path]).stdout ?? '';
  const str = (key) => new RegExp(`<key>${key}</key>\\s*<string>([^<]*)</string>`).exec(xml)?.[1];
  const team = /<key>TeamIdentifier<\/key>\s*<array>\s*<string>([^<]*)<\/string>/.exec(xml)?.[1];
  const devices = [...(/<key>ProvisionedDevices<\/key>\s*<array>([\s\S]*?)<\/array>/.exec(xml)?.[1] ?? '').matchAll(/<string>([^<]*)<\/string>/g)].map((m) => m[1]);
  const expires = /<key>ExpirationDate<\/key>\s*<date>([^<]*)<\/date>/.exec(xml)?.[1];
  return { path, name: str('Name'), team, appId: str('application-identifier'), devices, dev: /<key>get-task-allow<\/key>\s*<true\/>/.test(xml), expires: new Date(expires ?? 0) };
}

/** The Apple Development identity (its SHA-1) for a team, from the keychain; EXACT_IDENTITY names one. */
export function identity(team) {
  if (process.env.EXACT_IDENTITY) return process.env.EXACT_IDENTITY;
  const valid = [...(read('security', ['find-identity', '-v', '-p', 'codesigning']).stdout ?? '').matchAll(/\d+\) ([0-9A-F]{40}) "(Apple Development: [^"]+)"/g)].map((m) => ({ sha1: m[1], name: m[2] }));
  const pems = (read('security', ['find-certificate', '-a', '-c', 'Apple Development', '-p']).stdout ?? '').split('-----END CERTIFICATE-----').filter((c) => c.includes('BEGIN CERTIFICATE'));
  for (const pem of pems) {
    const x = read('openssl', ['x509', '-noout', '-subject', '-fingerprint', '-sha1'], { input: pem + '-----END CERTIFICATE-----\n' }).stdout ?? '';
    const sha1 = /Fingerprint=([0-9A-F:]+)/i.exec(x)?.[1].replace(/:/g, '');
    const ou = /OU\s*=\s*([A-Z0-9]+)/.exec(x)?.[1];
    const id = valid.find((v) => v.sha1 === sha1);
    if (id && ou === team) return id.sha1;
  }
  throw new Error(`no valid "Apple Development" identity for team ${team} in the keychain (security find-identity -v -p codesigning); name one with EXACT_IDENTITY=<sha1>`);
}

/** The first valid "Apple Development" identity in the keychain (EXACT_IDENTITY names one), or null: the macOS binary is then ad-hoc signed. */
function macIdentity() {
  if (process.env.EXACT_IDENTITY) return process.env.EXACT_IDENTITY;
  const m = /\d+\) ([0-9A-F]{40}) "Apple Development: /.exec(read('security', ['find-identity', '-v', '-p', 'codesigning']).stdout ?? '');
  return m ? m[1] : null;
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

/** The entitlements a device build signs with (LLP 1030 D1's host-metadata row): the identity the profile grants, and what the manifest's `host.ios` claims — associated domains for the app's origin when it says so. Generated, never committed. */
export const entitlements = (app, team) => {
  const ios = app.manifest.host?.ios ?? {};
  const dict = {
    'application-identifier': `${team}.${app.id}`,
    'com.apple.developer.team-identifier': team,
    'get-task-allow': true,
  };
  if (ios.associatedDomains && app.origin) dict['com.apple.developer.associated-domains'] = [`applinks:${new URL(app.origin).host}`];
  return plistFile(dict);
};

/** The iOS `Info.plist` from the manifest (LLP 1030 D2: one declaration; `build.mjs` consumes what it generates). The dev client's local-networking permission is `host.ios.localNetworking` (a string: the prompt); the store-required version numbers are counters bake owns, not authored. */
export const infoPlist = (app, device = false, { executable = 'ExactIOS', id = app.id, name = app.displayName } = {}) => {
  const ios = app.manifest.host?.ios ?? {};
  const families = (ios.deviceFamily ?? ['iphone', 'ipad']).map((f) => (f === 'ipad' ? 2 : 1));
  const dict = {
    CFBundleExecutable: executable,
    CFBundleIdentifier: id,
    CFBundleName: name,
    CFBundleDisplayName: name,
    CFBundlePackageType: 'APPL',
    CFBundleVersion: '1',
    CFBundleShortVersionString: '0.1.0',
    CFBundleSupportedPlatforms: [device ? 'iPhoneOS' : 'iPhoneSimulator'],
    DTPlatformName: device ? 'iphoneos' : 'iphonesimulator',
    MinimumOSVersion: ios.minimumOS ?? '17.0',
    UIDeviceFamily: families,
    UILaunchScreen: {},
    UIApplicationSceneManifest: { UIApplicationSupportsMultipleScenes: false },
    CADisableMinimumFrameDurationOnPhone: true,
  };
  if (ios.localNetworking) {
    dict.NSAppTransportSecurity = { NSAllowsLocalNetworking: true };
    dict.NSLocalNetworkUsageDescription = typeof ios.localNetworking === 'string' ? ios.localNetworking : 'Connects to your dev server on the local network.';
  }
  if (ios.backgroundModes?.length) dict.UIBackgroundModes = ios.backgroundModes;
  if (ios.urlSchemes?.length) dict.CFBundleURLTypes = [{ CFBundleURLName: app.id, CFBundleURLSchemes: ios.urlSchemes }];
  for (const [key, text] of Object.entries(ios.permissions ?? {})) dict[key] = text;
  return plistFile(dict);
};

/** The macOS `Info.plist` for a bundled build, from the same manifest. */
export const macInfoPlist = (app) => plistFile({
  CFBundleExecutable: 'ExactMac',
  CFBundleIdentifier: app.id,
  CFBundleName: app.displayName,
  CFBundleDisplayName: app.displayName,
  CFBundlePackageType: 'APPL',
  CFBundleVersion: '1',
  CFBundleShortVersionString: '0.1.0',
  LSMinimumSystemVersion: app.manifest.host?.macos?.minimumOS ?? '14.0',
  NSHighResolutionCapable: true,
  ...(app.manifest.host?.macos?.urlSchemes?.length ? { CFBundleURLTypes: [{ CFBundleURLName: app.id, CFBundleURLSchemes: app.manifest.host.macos.urlSchemes }] } : {}),
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

// ---------------------------------------------------------------- the build

function main(args) {
  const device = args.includes('--device');
  const ios = device || args.includes('--ios');
  const app = resolveApp(args.find((a, i) => !a.startsWith('--') && args[i - 1] !== '--sim' && args[i - 1] !== '--phone'));
  const crate = app.crate('apple');
  const gpuCrate = app.crate('gpu');
  const hasGpu = existsSync(resolve(app.dir, 'gpu', 'Cargo.toml'));
  const dylib = `lib${gpuCrate.replace(/-/g, '_')}.dylib`;
  // What the presenter dlopens is the same name whatever the app is: one
  // Swift binary serves every app, and two apps' modules would otherwise
  // sit side by side in the shared build directory with the presenter
  // loading whichever one it was compiled to name.
  const loadName = 'libexact_gpu.dylib';
  const webLoadName = 'libexact_web.dylib';
  const webBuildDir = mkdtempSync(resolve(tmpdir(), 'exact-webarm-'));
  const webBuilt = resolve(webBuildDir, webLoadName);
  const t0 = Date.now();
  const target = device ? 'aarch64-apple-ios' : iosTarget;
  const targetArgs = ios ? ['--target', target] : [];
  const libDir = resolve(app.target, ios ? target : '', 'release');
  const sdkName = ios ? (device ? 'iphoneos' : 'iphonesimulator') : 'macosx';
  const sdk = read('xcrun', ['--sdk', sdkName, '--show-sdk-path']).stdout.trim();
  const cargoEnv = {
    ...process.env,
    SDKROOT: sdk,
    ...(ios ? { IPHONEOS_DEPLOYMENT_TARGET: '17.0' } : { MACOSX_DEPLOYMENT_TARGET: '14.0' }),
  };
  runApple('cargo', ['build', '--release', '-p', crate, ...targetArgs], { cwd: app.workspace, env: cargoEnv });
  // The app's GPU module (LLP 1009 D2): a dylib beside the executable (in
  // the bundle's Frameworks on iOS), loaded on demand by the presenter.
  if (hasGpu) runApple('cargo', ['build', '--release', '-p', gpuCrate, ...targetArgs], { cwd: app.workspace, env: cargoEnv });
  const gpuNote = hasGpu ? dylib : 'no GPU crate';
  // --embed (LLP 1031 D10, the developer-facing promise): what a consumer
  // without a Rust toolchain links — the archive, the C header, the GPU
  // module, the shaders and assets, and the cohort's `compat.json` — under
  // `target/embed/<app>/<platform>/`, with `ExactKit` as the package at
  // `host/apple`. No Swift is built for it; the sample hosts are the proof
  // that the same pieces link.
  if (args.includes('--embed')) {
    const platform = ios ? (device ? 'ios' : 'ios-simulator') : 'macos';
    const embed = resolve(app.target, 'embed', app.name, platform);
    rmSync(embed, { recursive: true, force: true });
    mkdirSync(resolve(embed, 'include'), { recursive: true });
    const archive = `lib${crate.replace(/-/g, '_')}.a`;
    copyFileSync(resolve(libDir, archive), resolve(embed, archive));
    copyFileSync(resolve(pkg, 'include/exact.h'), resolve(embed, 'include/exact.h'));
    if (hasGpu) copyFileSync(resolve(libDir, dylib), resolve(embed, loadName));
    copyAppleStaticTrees(app.dir, embed, [['assets', 'assets'], ['deck', 'deck'], ['gpu/shaders', 'shaders']]);
    const compat = read(resolve(root, 'target/release/contract'), ['compat', app.dir, '--platform', ios ? 'ios' : 'macos', '--target', ios ? target : (process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin'), '--json']);
    if (compat.status === 0) writeFileSync(resolve(embed, 'compat.json'), compat.stdout);
    writeFileSync(resolve(embed, 'receipt.json'), receipt(app, { platform, target: ios ? target : (process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin'), sdk, archive, gpu: hasGpu ? loadName : null, package: pkg }));
    const bytes = statSync(resolve(embed, archive)).size;
    console.log(`host/apple: ${embed.replace(root + '/', '')} — ${archive} ${(bytes / 1048576).toFixed(2)} MB, include/exact.h${hasGpu ? `, ${loadName}` : ''}, shaders, assets, compat.json; link it with the ExactKit package at ${pkg.replace(root + '/', '')}`);
    if (!args.includes('--run') && !args.includes('--host')) return;
  }
  const t1 = Date.now();
  // SwiftPM compiles Package.swift itself for macOS before applying the iOS
  // product triple; an iPhone SDKROOT in its environment breaks that host
  // manifest compile. The target SDK stays in the explicit Swift arguments.
  const env = {
    ...process.env,
    ...(ios ? { IPHONEOS_DEPLOYMENT_TARGET: '17.0' } : { MACOSX_DEPLOYMENT_TARGET: '14.0' }),
    EXACT_LIB_DIR: libDir,
    EXACT_LIB: crate.replace(/-/g, '_'),
  };
  // The products: the standalone app, and with --host the sample host too
  // (LLP 1031 D10 — the fixture the smoke drives).
  const products = [ios ? 'ExactIOS' : 'ExactMac', ...(args.includes('--host') ? [ios ? 'ExactHostIOS' : 'ExactHostMac'] : [])];
  const triple = ios ? (device ? 'arm64-apple-ios17.0' : iosTriple) : macTriple;
  const product = products[0];
  // swift build does not see the Rust archive change; drop the executables so
  // they relink against the archive cargo just built (a relink is ~0.4 s).
  for (const p of products) rmSync(productPath(p, triple), { force: true });
  // One `swift build` per product: given two `--product` flags SwiftPM
  // builds only the last; the second build is incremental and quick.
  const swiftArgs = ['build', '-c', 'release'];
  if (ios) {
    swiftArgs.push(
      '--triple', device ? 'arm64-apple-ios17.0' : iosTriple,
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
  }
  for (const p of products) runApple('swift', [...swiftArgs, '--product', p], { cwd: pkg, env });
  const binDir = resolve(productPath(product, triple), '..');
  // The iframe arm (@ref LLP 1020 D3): the only artifact that links WebKit.
  // It is built beside the presenter but never linked into it; WebModule.swift
  // dlopens this file at the first iframe create commit.
  // `--sdk` and not a bare `xcrun`: xcrun exports SDKROOT for the tool it runs,
  // and the default is macosx — the same MacOSX-sysroot-for-an-iPhone-target the
  // presenter's link step hits above.
  const webArgs = ['--sdk', sdkName, 'swiftc', '-module-cache-path', resolve(webBuildDir, 'module-cache'), '-parse-as-library', '-emit-library', '-O', '-module-name', 'ExactWebArm', resolve(root, 'host/apple/webarm/WebArm.swift'), '-o', webBuilt, '-framework', 'WebKit'];
  if (ios) {
    webArgs.push('-target', device ? 'arm64-apple-ios17.0' : iosTriple, '-sdk', sdk);
  } else {
    webArgs.push('-target', `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macos14.0`);
  }
  runApple('xcrun', webArgs);
  const t2 = Date.now();
  const bin = resolve(binDir, product);

  if (!ios) {
    // Replace the dylib, never overwrite it in place: a running app may still
    // have the old one mapped, and rewriting a mapped, ad-hoc-signed file poisons
    // the kernel's cached signature for that inode — every later dlopen dies with
    // SIGKILL (Code Signature Invalid). A new file is a new inode.
    const gpuDest = resolve(binDir, loadName);
    rmSync(gpuDest, { force: true });
    if (hasGpu) copyFileSync(resolve(libDir, dylib), gpuDest);
    const webDest = resolve(binDir, webLoadName);
    rmSync(webDest, { force: true });
    copyFileSync(webBuilt, webDest);
    // The app's kept secrets live in the login keychain, whose ACL trusts the
    // creating app by its code signature (LLP 1018 D7): signed with the team's
    // identity a rebuild keeps them; ad-hoc, every rebuild is a new app and
    // the keychain asks again — before the first frame.
    const sha1 = macIdentity();
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
    writeFileSync(resolve(binDir, `${products[0]}-Info.plist`), macInfoPlist(app));
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', webDest], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', app.id, bin], { stdio: 'ignore' });
    for (const p of products.slice(1)) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', `${app.id}.${p.toLowerCase()}`, resolve(binDir, p)], { stdio: 'ignore' });
    // The receipt beside it (LLP 1030 D2).
    writeFileSync(resolve(binDir, 'receipt.json'), receipt(app, { platform: 'macos', target: process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin', sdk, identity: sha1 ?? 'ad-hoc', profile: null, entitlements: null, gpu: hasGpu ? dylib : null }));
    rmSync(webBuildDir, { recursive: true, force: true });
    console.log(`host/apple: ${bin.replace(root + '/', '')} (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s; ${sha1 ? 'signed ' + sha1.slice(0, 8) : 'ad-hoc signed'}); GPU: ${gpuNote}; web arm: ${webLoadName}`);
    // --run: the app, with the dev loop's plan watched when host/web/dev.mjs is
    // running (it writes host/web/dist/app.plan on every save).
    if (args.includes('--run')) spawnSync(bin, [], { stdio: 'inherit', env: { ...env, EXACT_DEV_PLAN: env.EXACT_DEV_PLAN ?? resolve(root, 'host/web/dist/app.plan'), EXACT_ASSETS: app.dir } });
    return;
  }

  // The bundle, assembled from scratch (new inodes, see above), with the
  // app's assets (a phone reads no other machine's paths); ad-hoc signed
  // for a simulator, signed with the team's identity, profile, and
  // entitlements for a phone.
  const bundle = device ? deviceBundle : appBundle;
  rmSync(bundle, { recursive: true, force: true });
  mkdirSync(resolve(bundle, 'Frameworks'), { recursive: true });
  copyFileSync(bin, resolve(bundle, product));
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device));
  // The GPU crate's shaders (LLP 1030 D8): files the presenter registers
  // with the module before a surface is created, never strings in the dylib.
  copyAppleStaticTrees(app.dir, bundle, [['assets', 'assets'], ['deck', 'deck'], ['gpu/shaders', 'shaders']]);
  if (hasGpu) copyFileSync(resolve(libDir, dylib), resolve(bundle, 'Frameworks', loadName));
  copyFileSync(webBuilt, resolve(bundle, 'Frameworks', webLoadName));
  if (device) {
    let ph, prof, sha1;
    try {
      ph = phone(args.includes('--phone') ? args[args.indexOf('--phone') + 1] : undefined);
      prof = profile(ph.udid, app.id);
      sha1 = identity(prof.team);
    } catch (e) { console.error(e.message); process.exit(1); }
    copyFileSync(prof.path, resolve(bundle, 'embedded.mobileprovision'));
    const ent = resolve(pkg, '.build/entitlements.plist');
    writeFileSync(ent, entitlements(app, prof.team));
    if (hasGpu) run('codesign', ['--force', '--sign', sha1, '--timestamp=none', resolve(bundle, 'Frameworks', loadName)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1, '--timestamp=none', resolve(bundle, 'Frameworks', webLoadName)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1, '--timestamp=none', '--entitlements', ent, bundle], { stdio: 'ignore' });
    writeFileSync(resolve(bundle, '..', 'receipt.json'), receipt(app, { platform: 'ios', target, sdk, identity: sha1, profile: { name: prof.name, team: prof.team, expires: prof.expires }, entitlements: readFileSync(ent, 'utf8'), gpu: hasGpu ? dylib : null }));
    rmSync(webBuildDir, { recursive: true, force: true });
    console.log(`host/apple: ${bundle.replace(root + '/', '')} for ${ph.name} (${ph.model}, iOS ${ph.os}) — signed as ${prof.name} (${prof.team}) (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s); GPU: ${gpuNote}; web arm: ${webLoadName}`);
    if (!ph.reachable) { console.error(`${ph.name} is not connected: plug it in (or have it on this network), unlock it, and trust this Mac; then run this again`); process.exit(1); }
    const i = read('xcrun', ['devicectl', 'device', 'install', 'app', '--device', ph.udid, bundle]);
    if (i.status !== 0) { console.error(i.stderr || i.stdout); process.exit(i.status ?? 1); }
    console.log(`installed on ${ph.name} in ${((Date.now() - t2) / 1000).toFixed(1)} s`);
    if (args.includes('--run')) {
      let launch;
      try { launch = deviceLaunchArgs(ph.udid, app.id); }
      catch (e) { console.error(e.message); process.exit(1); }
      const l = read('xcrun', launch);
      if (l.status !== 0) { console.error(l.stderr || l.stdout); process.exit(l.status ?? 1); }
      console.log(`launched ${app.id} on ${ph.name}`);
    }
    return;
  }
  if (hasGpu) run('codesign', ['--force', '--sign', '-', resolve(appBundle, 'Frameworks', loadName)], { stdio: 'ignore' });
  run('codesign', ['--force', '--sign', '-', resolve(appBundle, 'Frameworks', webLoadName)], { stdio: 'ignore' });
  run('codesign', ['--force', '--sign', '-', appBundle], { stdio: 'ignore' });
  writeFileSync(resolve(appBundle, '..', 'receipt.json'), receipt(app, { platform: 'ios-simulator', target, sdk, identity: 'ad-hoc', profile: null, entitlements: null, gpu: hasGpu ? dylib : null }));
  rmSync(webBuildDir, { recursive: true, force: true });
  const dev = simulator(args.includes('--sim') ? args[args.indexOf('--sim') + 1] : undefined);
  install(dev);
  // The sample host (LLP 1031 D10): its own bundle beside the app's, the
  // same assets and frameworks, bundle id `<app id>.host`, ad-hoc signed and
  // installed on the same simulator; `scripts/smoke.mjs host-ios` drives it.
  if (args.includes('--host')) {
    rmSync(hostBundle, { recursive: true, force: true });
    mkdirSync(resolve(hostBundle, 'Frameworks'), { recursive: true });
    copyFileSync(productPath('ExactHostIOS', triple), resolve(hostBundle, 'ExactHostIOS'));
    writeFileSync(resolve(hostBundle, 'Info.plist'), infoPlist(app, false, { executable: 'ExactHostIOS', id: `${app.id}.host`, name: 'Host (not Exact)' }));
    copyAppleStaticTrees(appBundle, hostBundle);
    for (const f of readdirSync(resolve(appBundle, 'Frameworks'))) { copyFileSync(resolve(appBundle, 'Frameworks', f), resolve(hostBundle, 'Frameworks', f)); run('codesign', ['--force', '--sign', '-', resolve(hostBundle, 'Frameworks', f)], { stdio: 'ignore' }); }
    run('codesign', ['--force', '--sign', '-', hostBundle], { stdio: 'ignore' });
    install(dev, hostBundle);
  }
  const t3 = Date.now();
  console.log(`host/apple: ${appBundle.replace(root + '/', '')} on ${dev.name} (${dev.runtime.replace(/.*SimRuntime\./, '')}, ${dev.udid}) (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s, install ${((t3 - t2) / 1000).toFixed(1)} s); GPU: ${gpuNote}; web arm: ${webLoadName}`);
  if (args.includes('--run')) {
    // Simulator.app showing this device, then the app — with the dev loop's
    // plan watched when host/web/dev.mjs is running. simctl passes the
    // environment through as SIMCTL_CHILD_*.
    spawnSync('open', ['-a', 'Simulator', '--args', '-CurrentDeviceUDID', dev.udid], { stdio: 'ignore' });
    const launched = read('xcrun', ['simctl', 'launch', '--terminate-running-process', dev.udid, app.id], {
      env: { ...process.env, SIMCTL_CHILD_EXACT_DEV_PLAN: process.env.EXACT_DEV_PLAN ?? resolve(root, 'host/web/dist/app.plan'), SIMCTL_CHILD_EXACT_ASSETS: app.dir },
    });
    if (launched.status !== 0) { console.error(launched.stderr); process.exit(launched.status ?? 1); }
    console.log(`launched ${launched.stdout.trim()} on ${dev.name}`);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) main(process.argv.slice(2));
