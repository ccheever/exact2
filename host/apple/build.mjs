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
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolveApp } from '../../scripts/app.mjs';

const root = resolve(new URL('../..', import.meta.url).pathname);
const run = (cmd, args, opts = {}) => { const r = spawnSync(cmd, args, { cwd: root, stdio: 'inherit', ...opts }); if (r.status !== 0) process.exit(r.status ?? 1); return r; };
const read = (cmd, args, opts = {}) => spawnSync(cmd, args, { cwd: root, encoding: 'utf8', ...opts });

// ---------------------------------------------------------------- iOS: the bundle and the simulator

/** The app's bundle identifier, from its crate (`caltrain-apple` → com.exact.caltrain). */
export const bundleId = (crate = 'caltrain-apple') => `com.exact.${crate.replace(/-apple$/, '')}`;
/** Where `--ios` leaves the assembled app (the simulator's), and `--device` the phone's. */
export const appBundle = resolve(root, 'host/apple/ios/.build/ExactIOS.app');
export const deviceBundle = resolve(root, 'host/apple/ios/.build/device/ExactIOS.app');
/** The simulator's Rust target and Swift triple on this machine. */
export const iosTarget = process.arch === 'arm64' ? 'aarch64-apple-ios-sim' : 'x86_64-apple-ios';
export const iosTriple = `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-ios17.0-simulator`;

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
export function install(dev) {
  if (!existsSync(appBundle)) throw new Error('run node host/apple/build.mjs --ios first');
  const r = read('xcrun', ['simctl', 'install', dev.udid, appBundle]);
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

const entitlements = (team, bundle) => `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>application-identifier</key><string>${team}.${bundle}</string>
  <key>com.apple.developer.team-identifier</key><string>${team}</string>
  <key>get-task-allow</key><true/>
</dict></plist>
`;

const infoPlist = (crate, device = false) => `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>ExactIOS</string>
  <key>CFBundleIdentifier</key><string>${bundleId(crate)}</string>
  <key>CFBundleName</key><string>Exact</string>
  <key>CFBundleDisplayName</key><string>Exact</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleSupportedPlatforms</key><array><string>${device ? 'iPhoneOS' : 'iPhoneSimulator'}</string></array>
  <key>DTPlatformName</key><string>${device ? 'iphoneos' : 'iphonesimulator'}</string>
  <key>MinimumOSVersion</key><string>17.0</string>
  <key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
  <key>UILaunchScreen</key><dict/>
  <key>UIApplicationSceneManifest</key><dict><key>UIApplicationSupportsMultipleScenes</key><false/></dict>
  <key>CADisableMinimumFrameDurationOnPhone</key><true/>
</dict></plist>
`;

// ---------------------------------------------------------------- the build

function main(args) {
  const device = args.includes('--device');
  const ios = device || args.includes('--ios');
  const app = resolveApp(args.find((a, i) => !a.startsWith('--') && args[i - 1] !== '--sim' && args[i - 1] !== '--phone'));
  const crate = app.crate('apple');
  const gpuCrate = app.crate('gpu');
  const hasGpu = existsSync(resolve(app.dir, 'gpu', 'Cargo.toml'));
  const dylib = `lib${gpuCrate.replace(/-/g, '_')}.dylib`;
  const t0 = Date.now();
  const target = device ? 'aarch64-apple-ios' : iosTarget;
  const targetArgs = ios ? ['--target', target] : [];
  const libDir = resolve(app.target, ios ? target : '', 'release');
  run('cargo', ['build', '--release', '-p', crate, ...targetArgs], { cwd: app.workspace });
  // The app's GPU module (LLP 1009 D2): a dylib beside the executable (in
  // the bundle's Frameworks on iOS), loaded on demand by the presenter.
  if (hasGpu) run('cargo', ['build', '--release', '-p', gpuCrate, ...targetArgs], { cwd: app.workspace });
  const gpuNote = hasGpu ? dylib : 'no GPU crate';
  const t1 = Date.now();
  const env = { ...process.env, EXACT_LIB_DIR: libDir, EXACT_LIB: crate.replace(/-/g, '_') };
  const pkg = resolve(root, 'host/apple', ios ? 'ios' : 'macos');
  const product = ios ? 'ExactIOS' : 'ExactMac';
  // swift build does not see the Rust archive change; drop the executable so
  // it relinks against the archive cargo just built (a relink is ~0.4 s).
  rmSync(resolve(pkg, '.build/release', product), { force: true });
  const swiftArgs = ['build', '-c', 'release'];
  if (ios) {
    const sdk = read('xcrun', ['--sdk', device ? 'iphoneos' : 'iphonesimulator', '--show-sdk-path']).stdout.trim();
    swiftArgs.push('--triple', device ? 'arm64-apple-ios17.0' : iosTriple, '--sdk', sdk);
  }
  run('swift', swiftArgs, { cwd: pkg, env });
  const t2 = Date.now();
  const bin = resolve(pkg, '.build/release', product);

  if (!ios) {
    // Replace the dylib, never overwrite it in place: a running app may still
    // have the old one mapped, and rewriting a mapped, ad-hoc-signed file poisons
    // the kernel's cached signature for that inode — every later dlopen dies with
    // SIGKILL (Code Signature Invalid). A new file is a new inode.
    if (hasGpu) {
      const dest = resolve(pkg, '.build/release', dylib);
      rmSync(dest, { force: true });
      copyFileSync(resolve(libDir, dylib), dest);
    }
    console.log(`host/apple: ${bin.replace(root + '/', '')} (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s); GPU: ${gpuNote}`);
    // --run: the app, with the dev loop's plan watched when host/web/dev.mjs is
    // running (it writes host/web/dist/app.plan on every save).
    if (args.includes('--run')) spawnSync(bin, [], { stdio: 'inherit', env: { ...env, EXACT_DEV_PLAN: resolve(root, 'host/web/dist/app.plan'), EXACT_ASSETS: app.dir } });
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
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(crate, device));
  if (existsSync(resolve(app.dir, 'assets'))) cpSync(resolve(app.dir, 'assets'), resolve(bundle, 'assets'), { recursive: true });
  if (hasGpu) copyFileSync(resolve(libDir, dylib), resolve(bundle, 'Frameworks', dylib));
  if (device) {
    let ph, prof, sha1;
    try {
      ph = phone(args.includes('--phone') ? args[args.indexOf('--phone') + 1] : undefined);
      prof = profile(ph.udid, bundleId(crate));
      sha1 = identity(prof.team);
    } catch (e) { console.error(e.message); process.exit(1); }
    copyFileSync(prof.path, resolve(bundle, 'embedded.mobileprovision'));
    const ent = resolve(root, 'host/apple/ios/.build/entitlements.plist');
    writeFileSync(ent, entitlements(prof.team, bundleId(crate)));
    if (hasGpu) run('codesign', ['--force', '--sign', sha1, '--timestamp=none', resolve(bundle, 'Frameworks', dylib)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1, '--timestamp=none', '--entitlements', ent, bundle], { stdio: 'ignore' });
    console.log(`host/apple: ${bundle.replace(root + '/', '')} for ${ph.name} (${ph.model}, iOS ${ph.os}) — signed as ${prof.name} (${prof.team}) (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s); GPU: ${gpuNote}`);
    if (!ph.reachable) { console.error(`${ph.name} is not connected: plug it in (or have it on this network), unlock it, and trust this Mac; then run this again`); process.exit(1); }
    const i = read('xcrun', ['devicectl', 'device', 'install', 'app', '--device', ph.udid, bundle]);
    if (i.status !== 0) { console.error(i.stderr || i.stdout); process.exit(i.status ?? 1); }
    console.log(`installed on ${ph.name} in ${((Date.now() - t2) / 1000).toFixed(1)} s`);
    if (args.includes('--run')) {
      const l = read('xcrun', ['devicectl', 'device', 'process', 'launch', '--terminate-existing', '--device', ph.udid, bundleId(crate)]);
      if (l.status !== 0) { console.error(l.stderr || l.stdout); process.exit(l.status ?? 1); }
      console.log(`launched ${bundleId(crate)} on ${ph.name}`);
    }
    return;
  }
  if (hasGpu) run('codesign', ['--force', '--sign', '-', resolve(appBundle, 'Frameworks', dylib)], { stdio: 'ignore' });
  run('codesign', ['--force', '--sign', '-', appBundle], { stdio: 'ignore' });
  const dev = simulator(args.includes('--sim') ? args[args.indexOf('--sim') + 1] : undefined);
  install(dev);
  const t3 = Date.now();
  console.log(`host/apple: ${appBundle.replace(root + '/', '')} on ${dev.name} (${dev.runtime.replace(/.*SimRuntime\./, '')}, ${dev.udid}) (cargo ${((t1 - t0) / 1000).toFixed(1)} s, swift ${((t2 - t1) / 1000).toFixed(1)} s, install ${((t3 - t2) / 1000).toFixed(1)} s); GPU: ${gpuNote}`);
  if (args.includes('--run')) {
    // Simulator.app showing this device, then the app — with the dev loop's
    // plan watched when host/web/dev.mjs is running. simctl passes the
    // environment through as SIMCTL_CHILD_*.
    spawnSync('open', ['-a', 'Simulator', '--args', '-CurrentDeviceUDID', dev.udid], { stdio: 'ignore' });
    const launched = read('xcrun', ['simctl', 'launch', '--terminate-running-process', dev.udid, bundleId(crate)], {
      env: { ...process.env, SIMCTL_CHILD_EXACT_DEV_PLAN: resolve(root, 'host/web/dist/app.plan'), SIMCTL_CHILD_EXACT_ASSETS: app.dir },
    });
    if (launched.status !== 0) { console.error(launched.stderr); process.exit(launched.status ?? 1); }
    console.log(`launched ${launched.stdout.trim()} on ${dev.name}`);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) main(process.argv.slice(2));
