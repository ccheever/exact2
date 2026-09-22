#!/usr/bin/env bun
// Build the macOS app — or, with --ios, the iOS app: the app's static
// library (cargo, release), then the presenter (swift build) linked against
// it. Usage:
//   bun host/apple/build.mjs [crate=caltrain-apple] [--run]                 macOS
//   bun host/apple/build.mjs [crate] --test                                  the Swift host tests
//   bun host/apple/build.mjs --ios [crate] [--run] [--sim <udid|name>]        iOS, on a simulator
//   bun host/apple/build.mjs --device [crate] [--run] [--phone <udid|name>]   iOS, on a phone
// Add --url <http(s) app URL> with --run to connect any of these clients
// to the same address as the browser (LLP 1030.000 §7).
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
// These developer builds explicitly allow unsigned updates. Set
// EXACT_UPDATE_TRUST=production for a signed-update-only artifact.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { homedir, tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { appleCargoClaims, cargoLibraryTarget, claimBuildOutput, appSourceKey, bakeOutput, buildBake, bakeTarget, developmentBuildEnv, developmentURLScheme, resolveApp, verifyBakeFiles } from '../../scripts/app.mjs';
import { copyStaticTreeIfPresent, listAssets } from '../web/serve.mjs';

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
export const macTriple = `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macosx`;

/** App-owned Apple paths, shared by builder and launchers. @ref LLP 1036.000 §2 */
export function appleArtifacts(app, { destination = 'macos', composition, trust = process.env.EXACT_UPDATE_TRUST ?? 'development', host = false } = {}) {
  if (!['macos', 'ios-simulator', 'ios'].includes(destination)) throw new Error(`unknown Apple destination ${destination}`);
  const platform = destination === 'macos' ? 'macos' : 'ios';
  composition ??= app.manifest.deploy?.store?.[platform] === '0' ? 'embedded' : 'updating';
  if (!['embedded', 'updating'].includes(composition) || !['development', 'production'].includes(trust)) throw new Error('invalid Apple composition or trust policy');
  const target = destination === 'macos' ? bakeTarget('macos') : destination === 'ios' ? 'aarch64-apple-ios' : iosTarget;
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
/** Explicit Swift scratch directory; no shared publication path. */
// SwiftPM's native build system nests products under the triple; the classic
// one (Xcode's toolchain) writes them straight into `release/`. Both candidates
// are removed before every build and exactly one must exist after it, so a
// toolchain switch can never hand over a stale executable from the other layout.
export const productCandidates = (product, triple, buildRoot) => [
  resolve(buildRoot, triple.replace(/-ios[\d.]+/, '-ios'), 'release', product),
  resolve(buildRoot, 'release', product),
];
export const builtProduct = (product, triple, buildRoot) => {
  const built = productCandidates(product, triple, buildRoot).filter((path) => existsSync(path));
  if (built.length !== 1) throw new Error(`swift build left ${built.length} copies of ${product} (${productCandidates(product, triple, buildRoot).join(', ')}); expected exactly one`);
  return built[0];
};

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
export function install(dev, bundle, app, host = false) {
  if (!bundle || !existsSync(bundle)) throw new Error('build the selected app with --ios first');
  assertAppleIdentity(app, resolve(bundle, host ? 'ExactHostIOS' : 'ExactIOS'));
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

/** The explicit app URL is a launch input, never a baked origin. Validate it
 * before invoking any build or signing tools. @ref LLP 1030.000 §7 */
export function developmentLaunchEnvironment(args, environment = process.env) {
  const index = args.indexOf('--url');
  if (index < 0) return { ...environment };
  if (args.lastIndexOf('--url') !== index) throw new Error('--url may be specified only once');
  if (!args.includes('--run')) throw new Error('--url requires --run');
  let url;
  try { url = new URL(args[index + 1]); } catch { /* diagnosed below */ }
  if (!url || !['http:', 'https:'].includes(url.protocol) || !url.hostname) {
    throw new Error('--url requires an absolute http(s) app URL');
  }
  return { ...environment, EXACT_DEV_PLAN: url.href };
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

function openingLinks(app, platform, development) {
  const scheme = development ? developmentURLScheme(app.id) : null;
  const schemes = [...new Set([...(app.manifest.host?.[platform]?.urlSchemes ?? []), ...(scheme ? [scheme] : [])])];
  return { ...(schemes.length ? { CFBundleURLTypes: [{ CFBundleURLName: app.id, CFBundleURLSchemes: schemes }] } : {}),
    ...(scheme ? { ExactDevelopmentURLScheme: scheme } : {}) };
}

/** The entitlements a device build signs with (LLP 1030 D1's host-metadata row): the identity the profile grants, and what the manifest's `host.ios` claims — associated domains for the app's origin when it says so. Generated, never committed. */
export const entitlements = (app, team) => {
  const ios = app.manifest.host?.ios ?? {};
  const dict = {
    'application-identifier': `${team}.${app.id}`,
    'com.apple.developer.team-identifier': team,
    'get-task-allow': true,
  };
  // @ref LLP 1038 D8 — explicit applinks entries, or the declared origin.
  const domains = Array.isArray(ios.associatedDomains) ? ios.associatedDomains : ios.associatedDomains && app.origin ? [`applinks:${new URL(app.origin).host}`] : [];
  if (domains.length) dict['com.apple.developer.associated-domains'] = domains;
  return plistFile(dict);
};

/** The iOS `Info.plist` from the manifest (LLP 1030 D2: one declaration; `build.mjs` consumes what it generates). The dev client's local-networking permission is `host.ios.localNetworking` (a string: the prompt); the store-required version numbers are counters bake owns, not authored. */
export const infoPlist = (app, device = false, { executable = 'ExactIOS', id = app.id, name = app.displayName, development = false } = {}) => {
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
  Object.assign(dict, openingLinks(app, 'ios', development));
  for (const [key, text] of Object.entries(ios.permissions ?? {})) dict[key] = text;
  return plistFile(dict);
};

// The UTI each MIME type names on Apple platforms. `inode/directory` is the
// one deviation from IANA's registry — freedesktop's spelling for a folder,
// because the web has no MIME type for one and an app that opens a directory
// (the LLP reader) must be able to say so. An unmapped type fails the bake
// rather than guessing `public.data` (LLP 0382: fail closed, loudly).
const UTIS = {
  'text/markdown': 'net.daringfireball.markdown',
  'text/plain': 'public.plain-text',
  'text/html': 'public.html',
  'application/json': 'public.json',
  'inode/directory': 'public.folder',
};

/** `CFBundleDocumentTypes` from the manifest's `file_handlers` (LLP 1033 D1):
 *  one entry per handler, `Viewer` and `Alternate` so declaring a type never
 *  takes it away from whatever already owns it. */
export function documentTypes(app) {
  return (app.manifest.file_handlers ?? []).map((handler) => {
    const types = Object.keys(handler.accept).map((mime) => {
      const uti = UTIS[mime];
      if (!uti) throw new Error(`host/apple: ${app.name}'s file_handlers accepts ${mime}, which names no Apple type; add it to UTIS in host/apple/build.mjs`);
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

/** The macOS `Info.plist` for a bundled build, from the same manifest. */
export const macInfoPlist = (app, { development = false } = {}) => plistFile({
  CFBundleExecutable: 'ExactMac',
  CFBundleIdentifier: app.id,
  CFBundleName: app.displayName,
  CFBundleDisplayName: app.displayName,
  CFBundlePackageType: 'APPL',
  CFBundleVersion: '1',
  CFBundleShortVersionString: '0.1.0',
  LSMinimumSystemVersion: app.manifest.host?.macos?.minimumOS ?? '14.0',
  NSHighResolutionCapable: true,
  ...(app.manifest.host?.macos?.window ? { ExactWindow: app.manifest.host.macos.window } : {}),
  ...(documentTypes(app).length ? { CFBundleDocumentTypes: documentTypes(app) } : {}),
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

// ---------------------------------------------------------------- the build

function main(args) {
  let launchEnv;
  try { launchEnv = developmentLaunchEnvironment(args); }
  catch (e) { console.error(e.message); process.exitCode = 1; return; }
  const device = args.includes('--device');
  const ios = device || args.includes('--ios');
  const app = resolveApp(args.find((a, i) => !a.startsWith('--') && !['--sim', '--phone', '--url'].includes(args[i - 1])));
  const release = appleBuildLock(app);
  const cleanup = [];
  try {
  const crate = app.crate('apple');
  const gpuCrate = app.crate('gpu');
  const hasGpu = app.hasGpu;
  let ph, prof;
  const sha1 = device ? (() => {
    ph = phone(args.includes('--phone') ? args[args.indexOf('--phone') + 1] : undefined);
    prof = profile(ph.udid, app.id);
    return identity(prof.team);
  })() : ios ? '-' : macIdentity();
  const dylib = `lib${gpuCrate.replace(/-/g, '_')}.dylib`;
  // What the presenter dlopens is the same name whatever the app is: one
  // Swift binary serves every app, and two apps' modules would otherwise
  // are captured and packaged together under the selected app's owner.
  const loadName = 'libexact_gpu.dylib';
  const webLoadName = 'libexact_web.dylib';
  const webBuildDir = mkdtempSync(resolve(tmpdir(), 'exact-webarm-'));
  cleanup.push(webBuildDir);
  const webBuilt = resolve(webBuildDir, webLoadName);
  const t0 = Date.now();
  const target = ios ? (device ? 'aarch64-apple-ios' : iosTarget) : bakeTarget('macos');
  const cargoLibDir = resolve(app.target, target, 'release');
  const sdkName = ios ? (device ? 'iphoneos' : 'iphonesimulator') : 'macosx';
  const sdk = read('xcrun', ['--sdk', sdkName, '--show-sdk-path']).stdout.trim();
  const cargoEnv = {
    ...developmentBuildEnv(),
    SDKROOT: sdk,
    MACOSX_DEPLOYMENT_TARGET: '14.0',
    ...(ios ? {
      IPHONEOS_DEPLOYMENT_TARGET: '17.0',
      // The bake's host dependencies compile Objective-C++ too. cc-rs
      // inherits SDKROOT; target the Mac SDK explicitly for those units.
      HOST_CXXFLAGS: `${process.env.HOST_CXXFLAGS ?? ''} -isysroot ${read('xcrun', ['--sdk', 'macosx', '--show-sdk-path']).stdout.trim()}`,
    } : {}),
  };
  // Named Cargo products can alias in external workspaces or two checkouts
  // sharing a target. Claim those names through bake-and-capture only.
  let bakedPlan, paths;
  const development = cargoEnv.EXACT_UPDATE_TRUST === 'development';
  cargoEnv.EXACT_BAKE_OUTPUT = bakeOutput(app, cargoEnv);
  const buildReceipt = buildBake(app, ios ? 'ios' : 'macos', target, { env: cargoEnv, prepareGpu(product) {
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', product], {stdio:'ignore'});
  }, capture(buildReceipt) {
    const composition = buildReceipt.compat.inputs?.store?.L === '0' ? 'embedded' : 'updating';
    paths = appleArtifacts(app, { destination: ios ? (device ? 'ios' : 'ios-simulator') : 'macos', composition, trust: cargoEnv.EXACT_UPDATE_TRUST });
    mkdirSync(paths.namespace, { recursive: true });
    const capture = mkdtempSync(resolve(paths.namespace, '.capture-'));
    cleanup.push(capture);
    for (const file of [`lib${crate.replace(/-/g, '_')}.a`, ...(hasGpu ? [dylib] : [])]) captureAppleProduct(buildReceipt, resolve(cargoLibDir, file), resolve(capture, file));
    bakedPlan = readFileSync(resolve(cargoEnv.EXACT_BAKE_OUTPUT, `${ios ? 'ios' : 'macos'}-${target}.plan`));
    copyAppleStaticTrees(app.dir, capture, [['assets', 'assets'], ['deck', 'deck'], ...(app.manifest.game ? [] : [['gpu/shaders', 'shaders']])]);
    if (buildReceipt.rust) copyStaticTreeIfPresent(buildReceipt.rust, resolve(capture, 'rust'));
    verifyBakeFiles(buildReceipt.compat, bakedPlan, listAssets(capture, true));
    placeAppleArtifact(capture, paths.capture);
  } });
  const libDir = paths.capture;
  const bakedCompat = buildReceipt.compat;
  const level = bakedCompat.inputs?.store?.L;
  if (!['0', 'A'].includes(level)) throw new Error(`host/apple: unsupported baked store level ${level}`);
  const composition = level === '0' ? 'embedded' : 'updating';
  console.log(`host/apple: baked L=${level}; Swift ${composition} composition`);
  // The app's GPU module (LLP 1009 D2): a dylib beside the executable (in
  // the bundle's Frameworks on iOS), loaded on demand by the presenter.
  const gpuNote = hasGpu ? dylib : 'no GPU crate';
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
    copyAppleStaticTrees(paths.capture, embed);
    verifyBakeFiles(bakedCompat, bakedPlan, listAssets(embed, true));
    writeFileSync(resolve(embed, 'compat.json'), JSON.stringify(bakedCompat, null, 2) + '\n');
    writeFileSync(resolve(embed, 'receipt.json'), receipt(app, { platform, target: ios ? target : (process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin'), sdk, archive, gpu: hasGpu ? loadName : null, package: pkg, composition, compatibilityId:bakedCompat.id, build:buildReceipt }));
    const bytes = statSync(resolve(embed, archive)).size;
    placeAppleArtifact(embed, paths.embed);
    console.log(`host/apple: ${paths.embed.replace(root + '/', '')} — ${archive} ${(bytes / 1048576).toFixed(2)} MB, include/exact.h${hasGpu ? `, ${loadName}` : ''}, shaders, assets, compat.json; link it with ExactKit${composition === 'updating' ? ' + ExactUpdates' : ''} from the package at ${pkg.replace(root + '/', '')}`);
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
    EXACT_APP_COMPOSITION: composition,
  };
  // The products: the standalone app, and with --host the sample host too
  // (LLP 1031 D10 — the fixture the smoke drives).
  const products = [ios ? 'ExactIOS' : 'ExactMac', ...(args.includes('--host') ? [ios ? 'ExactHostIOS' : 'ExactHostMac'] : [])];
  const triple = ios ? (device ? 'arm64-apple-ios17.0' : iosTriple) : macTriple;
  const product = products[0];
  // swift build does not see the Rust archive change; drop the executables so
  // they relink against the archive cargo just built (a relink is ~0.4 s).
  const swiftBuildRoot = paths.scratch;
  const binDir = mkdtempSync(resolve(paths.namespace, '.products-'));
  cleanup.push(binDir);
  for (const p of products) for (const path of productCandidates(p, triple, swiftBuildRoot)) rmSync(path, { force: true });
  // One `swift build` per product: given two `--product` flags SwiftPM
  // builds only the last; the second build is incremental and quick.
  const swiftArgs = ['build', '-c', 'release', '--scratch-path', swiftBuildRoot];
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
  for (const p of products) {
    runApple('swift', [...swiftArgs, '--product', p], { cwd: pkg, env });
    const executable = resolve(binDir, p);
    copyFileSync(builtProduct(p, triple, swiftBuildRoot), executable);
    assertAppleIdentity(app, executable, bakedCompat.id);
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
    webArgs.push('-target', device ? 'arm64-apple-ios17.0' : iosTriple, '-sdk', sdk);
  } else {
    webArgs.push('-target', `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macos14.0`);
  }
  runApple('xcrun', webArgs);
  const t2 = Date.now();
  const bin = resolve(binDir, product);
  const hostPaths = appleArtifacts(app, { destination: ios ? (device ? 'ios' : 'ios-simulator') : 'macos', composition, trust: cargoEnv.EXACT_UPDATE_TRUST, host: true });
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
    const webDest = resolve(binDir, webLoadName);
    rmSync(webDest, { force: true });
    copyFileSync(webBuilt, webDest);
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
    writeFileSync(resolve(binDir, `${products[0]}-Info.plist`), macInfoPlist(app, { development }));
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', webDest], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', app.id, bin], { stdio: 'ignore' });
    for (const p of products.slice(1)) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', `${app.id}.${p.toLowerCase()}`, resolve(binDir, p)], { stdio: 'ignore' });
    // The receipt beside it (LLP 1030 D2).
    writeFileSync(resolve(binDir, 'receipt.json'), receipt(app, { compatibilityId:bakedCompat.id, build:buildReceipt, composition, platform: 'macos', target: process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin', sdk, identity: sha1 ?? 'ad-hoc', profile: null, entitlements: null, gpu: hasGpu ? dylib : null }));
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
      for (const file of ['ExactMac', webLoadName, ...(hasGpu ? [loadName] : [])]) copyFileSync(resolve(binDir, file), resolve(executables, file));
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development }));
      copyAppleStaticTrees(paths.capture, resources);
      verifyBakeFiles(bakedCompat, bakedPlan, listAssets(resources, true));
      copyFileSync(resolve(binDir, 'receipt.json'), resolve(resources, 'receipt.json'));
      for (const file of [webLoadName]) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(executables, file)], { stdio: 'ignore' });
      run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', bundle], { stdio: 'ignore' });
      const placed = bundleDestination;
      assertAppleIdentity(app, resolve(executables, 'ExactMac'), bakedCompat.id);
      placeAppleArtifact(bundle, placed);
      rmSync(stage, { recursive: true, force: true });
      bundlePath = placed;
      console.log(`local client: ${placed}\n  Open this app once to register its native opening link.`);
    }
    publishProducts();
    release();
    rmSync(webBuildDir, { recursive: true, force: true });
    console.log(`host/apple: ${resolve(paths.products, product).replace(root + '/', '')} (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s; ${sha1 ? 'signed ' + sha1.slice(0, 8) : 'ad-hoc signed'}); GPU: ${gpuNote}; web arm: ${webLoadName}`);
    // A live source is explicit (--url or EXACT_DEV_PLAN). The shared web
    // output may belong to another app, and TypeScript edits publish complete
    // URL generations rather than rewriting its initial app.plan.
    if (args.includes('--run')) spawnSync(bundlePath ? resolve(bundlePath, 'Contents/MacOS/ExactMac') : resolve(paths.products, product), [], { stdio: 'inherit', env: { ...env, ...launchEnv, EXACT_ASSETS: app.dir } });
    return;
  }

  // The bundle, assembled from scratch (new inodes, see above), with the
  // app's assets (a phone reads no other machine's paths); ad-hoc signed
  // for a simulator, signed with the team's identity, profile, and
  // entitlements for a phone.
  const bundle = resolve(binDir, 'ExactIOS.app');
  mkdirSync(resolve(bundle, 'Frameworks'), { recursive: true });
  copyFileSync(bin, resolve(bundle, product));
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, { development }));
  // The GPU crate's shaders (LLP 1030 D8): files the presenter registers
  // with the module before a surface is created, never strings in the dylib.
  copyAppleStaticTrees(paths.capture, bundle);
  verifyBakeFiles(bakedCompat, bakedPlan, listAssets(bundle, true));
  if (hasGpu) copyFileSync(resolve(libDir, dylib), resolve(bundle, 'Frameworks', loadName));
  copyFileSync(webBuilt, resolve(bundle, 'Frameworks', webLoadName));

  const bundles = [[bundle, false]];
  if (args.includes('--host')) {
    const hostBundle = resolve(binDir, 'ExactHostIOS.app');
    mkdirSync(resolve(hostBundle, 'Frameworks'), { recursive: true });
    copyFileSync(resolve(binDir, 'ExactHostIOS'), resolve(hostBundle, 'ExactHostIOS'));
    writeFileSync(resolve(hostBundle, 'Info.plist'), infoPlist(app, device, { executable: 'ExactHostIOS', id: `${app.id}.host`, name: 'Host (not Exact)', development }));
    copyAppleStaticTrees(paths.capture, hostBundle);
    for (const f of readdirSync(resolve(bundle, 'Frameworks'))) copyFileSync(resolve(bundle, 'Frameworks', f), resolve(hostBundle, 'Frameworks', f));
    bundles.push([hostBundle, true]);
  }
  for (const [assembled, host] of bundles) {
    const id = host ? `${app.id}.host` : app.id;
    const signingProfile = device ? (host ? profile(ph.udid, id) : prof) : null;
    const signingIdentity = device ? identity(signingProfile.team) : sha1;
    const ent = resolve(binDir, host ? 'host-entitlements.plist' : 'entitlements.plist');
    if (device) {
      copyFileSync(signingProfile.path, resolve(assembled, 'embedded.mobileprovision'));
      writeFileSync(ent, entitlements({ ...app, id }, signingProfile.team));
    }
    verifyBakeFiles(bakedCompat, bakedPlan, listAssets(assembled, true));
    assertAppleIdentity(app, resolve(assembled, host ? 'ExactHostIOS' : 'ExactIOS'), bakedCompat.id);
    writeFileSync(resolve(assembled, 'receipt.json'), receipt(app, { compatibilityId: bakedCompat.id, build: buildReceipt, composition,
      platform: device ? 'ios' : 'ios-simulator', target, sdk, identity: signingIdentity,
      profile: signingProfile ? { name: signingProfile.name, team: signingProfile.team, expires: signingProfile.expires } : null,
      entitlements: device ? readFileSync(ent, 'utf8') : null, gpu: hasGpu ? dylib : null }));
    for (const f of readdirSync(resolve(assembled, 'Frameworks')).filter(f => f !== loadName)) run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', resolve(assembled, 'Frameworks', f)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', ...(device ? ['--entitlements', ent] : []), assembled], { stdio: 'ignore' });
  }
  publishProducts();
  release();
  const dev = device ? ph : simulator(args.includes('--sim') ? args[args.indexOf('--sim') + 1] : undefined);
  for (const [, host] of bundles) {
    const placed = host ? hostPaths.bundle : paths.bundle;
    if (device) {
      if (!ph.reachable) throw new Error(`${ph.name} is not connected; signed bundle retained at ${placed}`);
      run('xcrun', ['devicectl', 'device', 'install', 'app', '--device', ph.udid, placed]);
    } else install(dev, placed, app, host);
  }
  console.log(`host/apple: ${paths.bundle} on ${dev.name} (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s); GPU: ${gpuNote}; web arm: ${webLoadName}`);
  if (args.includes('--run')) {
    if (device) run('xcrun', deviceLaunchArgs(ph.udid, app.id, launchEnv));
    else {
      spawnSync('open', ['-a', 'Simulator', '--args', '-CurrentDeviceUDID', dev.udid], { stdio: 'ignore' });
      run('xcrun', ['simctl', 'launch', '--terminate-running-process', dev.udid, app.id], {
        env: { ...process.env, ...(launchEnv.EXACT_DEV_PLAN ? { SIMCTL_CHILD_EXACT_DEV_PLAN: launchEnv.EXACT_DEV_PLAN } : {}), SIMCTL_CHILD_EXACT_ASSETS: app.dir },
      });
    }
  }
  } finally { for (const path of cleanup.reverse()) rmSync(path, { recursive: true, force: true }); release(); }

}

/** `--test`: the Swift host tests (LLP 1033 D4a). They link `ExactKit`,
 *  which links an app's archive, so cargo builds one first — Caltrain's by
 *  default, any app's by name. Not one of the five checks (`rules/RULES.md`
 *  caps those at five); run it when the host's own behaviour changes. */
function test(args) {
  const app = resolveApp(args.find((a) => !a.startsWith('--')));
  app.prepare?.();
  const crate = app.crate('apple');
  const release = appleBuildLock(app);
  const paths = appleArtifacts(app, { composition: 'embedded' });
  let cargoRelease;
  try {
    const cargoEnv = { ...developmentBuildEnv(), CARGO_TARGET_DIR: app.target };
    const metadata = read('cargo', ['metadata', '--no-deps', '--format-version', '1'], { cwd: app.workspace, env: cargoEnv });
    if (metadata.status !== 0) throw new Error(`cargo metadata: ${metadata.stderr}`);
    const package_ = JSON.parse(metadata.stdout).packages.find(p => p.name === crate);
    const unit = package_ && cargoLibraryTarget(package_);
    if (!unit) throw new Error(`Cargo has no library target for ${crate}`);
    cargoRelease = claimBuildOutput(app, appleCargoClaims(app, 'host', [unit])[0]);
    run('cargo', ['build', '--release', '-p', crate, '--lib'], { cwd: app.workspace, env: cargoEnv });
    const libDir = resolve(app.target, 'release');
    runApple('swift', ['test', '--scratch-path', resolve(paths.namespace, 'tests')], {
      cwd: pkg, stdio: 'inherit',
      env: { ...process.env, MACOSX_DEPLOYMENT_TARGET: '14.0', EXACT_TESTS: '1', EXACT_LIB_DIR: libDir, EXACT_LIB: unit.name.replace(/-/g, '_'), EXACT_APP_COMPOSITION: 'embedded' },
    });
  } finally { cargoRelease?.(); release(); }

}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  const args = process.argv.slice(2);
  try { if (args.includes('--test')) test(args); else main(args); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
