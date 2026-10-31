#!/usr/bin/env bun
// Build the macOS app — or, with --ios, the iOS app: the app's static
// library (cargo, release), then the presenter (swift build) linked against
// it. Usage:
//   bun host/apple/build.mjs [crate=caltrain-apple] [--run]                 macOS (--run assembles and launches the .app)
//   bun host/apple/build.mjs [crate] --test                                  the Swift host tests
//   bun host/apple/build.mjs [crate] --test --ios [--sim <udid|name>]        all tests compiled for UIKit on a simulator
//   bun host/apple/build.mjs --ios [crate] [--run] [--sim <udid|name>]        iOS, on a simulator
//   bun host/apple/build.mjs --tvos [crate] [--run] [--sim <udid|name>]       tvOS, on an Apple TV simulator
//   bun host/apple/build.mjs --tvos --device [crate] [--run] [--archive <out.ipa>]  tvOS, on an Apple TV
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
// or EXACT_SIM names one; else a booted iPhone; else the iPhone Pro on the newest iOS;
// --no-install stops before that and installs on no simulator (an iOS module roster no Mac Catalyst slice can read is read on one), a compile check — and with --run shows it in Simulator.app. --device is the
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
// These developer builds explicitly allow unsigned updates. --release, --distribution, --archive (or EXACT_UPDATE_TRUST=production) build a signed-update-only artifact.
import { platformWidgetEntries, activityEntries, hasEntries, widgetGroup, prepareWidgetCore, sharedWidgetCore, entryProfiles, macWidgetSigning, alarmActivity } from './widgets.mjs';
import { assembleMac, assembleIOS, buildIntents, shortcutKeys } from './bundle.mjs';
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { tmpdir } from 'node:os';
import { basename, dirname, isAbsolute, relative, resolve } from 'node:path';
import { closeSync, copyFileSync, cpSync, existsSync, linkSync, mkdirSync, mkdtempSync, openSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { evictArms, keepRoster, keptRoster, snapshotSources } from './arms.mjs';
import { HOST_MACOS } from '../../scripts/filesystem.mjs';
import { documentTypes, exportedTypes, importedTypes, openingLinks, webPageTypes } from './documents.mjs';
import { executableName, HOST_DEV, checkModuleRoster, copyShaders, appleCargoClaims, awaitBuildOutput, cargoLibraryTarget, claimBuildOutput, appSourceKey, bakeOutput, buildBake, contractLast, bakeTarget, developmentBuildEnv, developmentURLScheme, gpuModules, hermesBundle, injectedProfiles, resolveApp, verifyBakeFiles } from '../../scripts/app.mjs';
import { copyStaticTreeIfPresent, listAssets } from '../web/serve.mjs';
import { startSweep } from '../../scripts/sweep.mjs';
import { guardFlags } from '../../scripts/help.mjs';
import { appleImplementations } from '../../js/bake/src/typescript.mjs';
import { composedModule, moduleSeam, systemFrameworks } from './compose.mjs';
import { associatedDomains, entitlementValue, macPresence, macPasskeys, macProfileRows, merchantEntitlement, signInEntitlement } from './presence.mjs';
import { writeDataKeys } from './data-keys.mjs';
import { backgroundKeys, localNetworking, nfcKeys, startLocation, transportSecurity, updateActivation } from './plist.mjs';
import { compileFixtures, iosTestRuns, macTests } from './xctest.mjs';
import { appIcon, buildInfo, iosAssets, appleAssets, appleAssetCatalogInventory, copyMacResources, signingOrder, helperEntitlements, plistFile, wrapFramework, signedGpuProduct, assertArchiveFrameworks } from './assets.mjs';
import { keptModules } from './modules.mjs';
import { appleLinkArgs, prepareLibrary, xcframeworkSlice } from '../../scripts/package-libraries.mjs';
import { quiet, quietFor } from './quiet.mjs';
import { keptCrates } from './crates.mjs';
import { linkedPlanArgs, placeLinkedPlan } from '../../scripts/linked-plan.mjs';
import { appleComposition, assertCapabilityFrameworks, assertLinkedSdk, linksByPlan } from './link.mjs';
export { appIcon, iosAssets, documentTypes, exportedTypes, importedTypes };
import { IPAD_ORIENTATIONS, orientationKeys, allows, deviceLaunchArgs, developmentLaunchEnvironment, identity, installedBuild, macIdentity, phone, replacedNote, profile, showSimulator, simulator, simulators, useXcode } from './devices.mjs';
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
  if (!cargoMessages) process.stdout.write(quiet(r.stdout ?? ''));
  process.stderr.write(quiet(r.stderr ?? ''));
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
 * says its own failure. A build that ends early stops it. `authored` restores paths in the log (`snapshotSources`). */
function startApple(cmd, args, log, { authored, ...opts } = {}) {
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
    if (authored) writeFileSync(log, authored(readFileSync(log, 'utf8')));
    if (repeated && r.status !== 0) return false;
    const output = readFileSync(log, 'utf8');
    process.stderr.write(quiet(output));
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
export const tvosTarget = 'aarch64-apple-tvos-sim', tvosDeviceTarget = 'aarch64-apple-tvos';
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
  const flag = { iphoneos: `-mios-version-min=${minimum}`, iphonesimulator: `-mios-simulator-version-min=${minimum}`, appletvos: `-mtvos-version-min=${minimum}`, appletvsimulator: `-mtvos-simulator-version-min=${minimum}`, macosx: `-mmacosx-version-min=${minimum}` }[sdkName];
  const air = out.replace(/\.metallib$/, '') + '.air';
  run('xcrun', ['-sdk', sdkName, 'metal', '-std=metal3.0', flag, '-c', resolve(root, 'host/apple/metal/SvgFilter.metal'), '-o', air], { stdio: 'pipe' });
  run('xcrun', ['-sdk', sdkName, 'metallib', air, '-o', out], { stdio: 'pipe' });
  rmSync(air, { force: true });
  return out;
}
export const svgFilterLibraryName = 'ExactSvgFilter.metallib';
/** The Swift triple for an app's iOS build. */
export const iosTripleFor = (app, device, tv = false) =>
  tv ? `arm64-apple-tvos${deploymentTargets(app).ios}${device ? '' : '-simulator'}` : device ? `arm64-apple-ios${deploymentTargets(app).ios}` : `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-ios${deploymentTargets(app).ios}-simulator`;
export const macTriple = `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macosx`;
export const moduleTripleFor = (app, ios, device = false, tv = false) =>
  ios ? iosTripleFor(app, device, tv) : `${macTriple}${deploymentTargets(app).macos}`;
/** App-owned Apple paths, shared by builder and launchers. @ref LLP 1036.000 §2 */
export function appleArtifacts(app, { destination = 'macos', composition, trust = process.env.EXACT_UPDATE_TRUST ?? 'development', host = false, optimize = 'speed' } = {}) {
  if (!['macos', 'ios-simulator', 'ios', 'tvos-simulator', 'tvos'].includes(destination)) throw new Error(`unknown Apple destination ${destination}`);
  const platform = destination === 'macos' ? 'macos' : 'ios';
  composition ??= app.manifest.deploy?.store?.[platform] === '0' ? 'embedded' : 'updating';
  if (!['embedded', 'updating'].includes(composition) || !['development', 'production'].includes(trust)) throw new Error('invalid Apple composition or trust policy');
  const target = destination === 'macos' ? bakeTarget('macos') : destination === 'ios' ? 'aarch64-apple-ios' : destination === 'tvos-simulator' ? tvosTarget : destination === 'tvos' ? tvosDeviceTarget : iosTarget;
  const owner = resolve(app.target, 'clients', appSourceKey(app), app.id);
  const namespace = resolve(owner, destination, target, composition, trust);
  // `product` is the Swift package's; `executable`, what it is linked to, is the app's name, as Xcode's (a sample host keeps its own).
  const product = destination === 'macos' ? (host ? 'ExactHostMac' : 'ExactMac') : (host ? 'ExactHostIOS' : 'ExactIOS');
  const executable = host ? product : executableName(app);
  const products = resolve(namespace, host ? 'host' : 'standalone');
  // The Swift host is the same code for every app, so its compile is shared:
  // one SwiftPM scratch per destination, deployment target and trust, whatever
  // the app or its composition (one per app recompiled ExactKit for each: 45–56 s
  // on an M4; one shared by trust, at each switch to or from the production
  // whole-module build: 92 s). Only the link is the app's, made under `swiftLock`
  // and copied out to its private stage before the lock is released (`scratch`).
  const swift = resolve(app.target, 'apple-swift', `${destination}-${deploymentTargets(app)[platform]}${optimize === 'size' ? '-size' : trust === 'production' ? '-whole' : ''}`);
  return { owner, namespace, target, product, executable, products, composition, scratch: resolve(namespace, 'link'), swift, swiftLock: `${swift}.lock`,
    lock: resolve(owner, '.apple-build.lock'), capture: resolve(namespace, 'capture'),
    binary: resolve(products, executable), bundle: destination === 'macos'
      ? resolve(owner, 'macos', `${host ? executableName(app) + ' Host' : executable}.app`)
      : resolve(products, `${executable}.app`),
    embed: resolve(namespace, 'embed') };
}
/** App drives use the shipped bundle's transport policy; the sample host is a bare build product. */
export function appleRunBinary(app, host = false) {
  const paths = appleArtifacts(app, { host });
  return host ? paths.binary : resolve(paths.bundle, 'Contents/MacOS', paths.executable);
}
/** Both bare product directories resolve @executable_path/../Frameworks here. */
export function publishMacFramework(framework, namespace, identity) {
  mkdirSync(namespace, { recursive: true });
  const stage = mkdtempSync(resolve(namespace, '.frameworks-'));
  try {
    const copy = resolve(stage, 'ExactCore.framework');
    cpSync(framework, copy, { recursive: true, verbatimSymlinks: true });
    run('codesign', ['--force', '--sign', identity ?? '-', '--timestamp=none', copy]);
    placeAppleArtifact(stage, resolve(namespace, 'Frameworks'));
  } finally { rmSync(stage, { recursive: true, force: true }); }
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
export function assertAppleIdentity(app, executable, compatibilityId, shared = null) {
  const linked = read('otool', ['-L', executable]).stdout ?? '';
  const runtime = shared ?? resolve(dirname(executable), existsSync(resolve(dirname(executable), 'Frameworks')) ? 'Frameworks/ExactCore.framework/ExactCore' : '../Frameworks/ExactCore.framework/ExactCore');
  const bytes = readFileSync(/@rpath\/ExactCore\.framework\/(?:Versions\/A\/)?ExactCore/.test(linked) ? runtime : executable, 'utf8');
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
export function copyAppleStaticTrees(source, target, trees = [['assets', 'assets'], ['deck', 'deck'], ['shaders', 'shaders'], ['rust', 'rust'], ['plans', 'plans'], ['typescript', 'typescript'], ['activities', 'activities']]) {
  for (const [from, to] of trees) {
    copyStaticTreeIfPresent(resolve(source, from), resolve(target, to));
  }
}

/** Install the assembled bundle on the simulator, saying a development build put over another kind (`replacedNote`) unless the caller kept that one (`said`). */
export function install(dev, bundle, app, host = false, said = false, executable = host ? 'ExactHostIOS' : executableName(app)) {
  if (!bundle || !existsSync(bundle)) throw new Error('build the selected app with --ios first');
  assertAppleIdentity(app, resolve(bundle, executable));
  const replaced = !host && !said && bundle.includes('/development/') ? installedBuild(dev.udid, app.id) : null, r = read('xcrun', ['simctl', 'install', dev.udid, bundle]);
  if (r.status !== 0) throw new Error('simctl install: ' + r.stderr);
  if (replaced && replaced.trust !== 'development') console.error(replacedNote(dev.udid, app, replaced, appleArtifacts(app, { destination: 'ios-simulator', trust: 'production' }).bundle)); }

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
 * build's token (its receipt keeps it). Only to a request the development
 * gate calls local (`access.local`): the token is this Mac's, as the install
 * token is, never a LAN peer's or a proxied client's (stage 10 C's canvas
 * gateway marks its requests forwarded). */
export function developmentLinks(app, page, access) {
  if (access?.local !== true) return [];
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
/** A simulator build's App ID prefix: ten characters, a Team ID's shape,
 * which HealthKit splits the identifier by (LLP 1069.008.000 D3). */
export const SIMULATOR_PREFIX = 'SIMULATORX';

/** The App Clip `host.ios.appClip` declares, or null: its bundle id (`<app id>.Clip`, as Xcode names one), its
 * name (the app's) and the executable's name. An App Clip is an app (LLP 1126 D12): the clip bundle runs the whole
 * app's executable under this second identity, so nothing here is a target of its own. */
export const appClip = (app) => {
  const clip = app.manifest.host?.ios?.appClip;
  if (!clip) return null;
  const id = clip.id ?? `${app.id}.Clip`, name = clip.name ?? app.displayName;
  return { id, name, executable: executableName({ id, displayName: name ?? '' }) };
};

/** Device identity comes from the profile. Simulators need an app identity too: Keychain's default access group
 * is the application-identifier. A simulator's is `SIMULATORX.<id>`, with the bare id it had before kept as a
 * second Keychain group so items stored then still read and update in place (D3, measured). The grants' signing
 * entitlements (`reach.entitlements`, D2) are each `true`. `allows`: the signing profile's entitlements. */
export const entitlements = (app, team = null, debuggable = true, reach = null, { simulator = false, prefix = team, allows = [] } = {}) => {
  const ios = app.manifest.host?.ios ?? {};
  const prefixed = `${SIMULATOR_PREFIX}.${app.id}`;
  const dict = {
    'application-identifier': team ? `${prefix}.${app.id}` : simulator ? prefixed : app.id,
    ...(team ? { 'com.apple.developer.team-identifier': team } : {}),
    ...(simulator && !team ? { 'keychain-access-groups': [prefixed, app.id] } : {}),
    // A distribution profile grants no debugger; its entitlements must not ask.
    'get-task-allow': debuggable,
  };
  // The clip (`clipParent`, the app's id) is its parent's App Clip: the parent's entitlements, the App Group and
  // Keychain group of the parent's included (how a clip shares data with its app), under the clip's own
  // application-identifier (Apple, "Creating an App Clip with Xcode", the App Clip entitlement).
  if (app.clipParent) dict['com.apple.developer.parent-application-identifiers'] = [`${prefix ?? SIMULATOR_PREFIX}.${app.clipParent}`];
  if (hasEntries(app) && !app.widgetTV && !app.widgetHost) {
    dict['com.apple.security.application-groups'] = [widgetGroup(app)];
    const owner = app.widgetAppId ?? app.clipParent ?? app.id;
    dict['keychain-access-groups'] = [`${prefix ?? SIMULATOR_PREFIX}.${owner}`, ...(simulator && !team ? [owner] : [])];
    // An activity a server advances (LLP 1126 D9): the app alone reads the update tokens, so the extension
    // (`widgetAppId`) signs none. A development profile grants the sandbox, a distribution one production.
    // The clip's notifications are ephemeral (`NSAppClip`), so it takes no push environment either.
    if (!app.widgetAppId && !app.clipParent && activityEntries(app).some(entry => entry.push === true)) dict['aps-environment'] = debuggable ? 'development' : 'production';
  }
  if (app.manifest.share_target && simulator && !app.widgetTV && !app.widgetHost && !app.clipParent) dict['com.apple.security.application-groups'] ??= [widgetGroup(app)]; // the share inbox (LLP 1133.024 D4)
  // @ref LLP 1038 D8 — explicit applinks entries, or the declared origin.
  const domains = Array.isArray(ios.associatedDomains) ? ios.associatedDomains : ios.associatedDomains && app.origin ? [`applinks:${new URL(app.origin).host}`] : [];
  // @ref LLP 1069.006 D2 — a claimed https auth callback needs `webcredentials:` (the bake's derivation from the `auth.callback` grants, LLP 1069.008).
  const linked = [...domains, ...associatedDomains(reach?.auth, debuggable)];
  // An App Clip is invoked from its `appclips:` domains, which its parent's entitlements list too (Apple,
  // "Associating your App Clip with your website"): each `applinks:` host, in the clip in place of the `applinks:`.
  const clips = appClip(app) && !app.widgetTV ? linked.filter((d) => d.startsWith('applinks:')).map((d) => `appclips:${d.slice('applinks:'.length)}`) : [];
  const all = [...new Set([...(app.clipParent ? linked.filter((d) => !d.startsWith('applinks:')) : linked), ...clips])];
  if (all.length) dict['com.apple.developer.associated-domains'] = all;
  for (const name of reach?.entitlements ?? []) dict[name] = reach.entitlementValues?.[name] ?? entitlementValue(name, debuggable);
  Object.assign(dict, merchantEntitlement(reach, { simulator, team, allows }));
  return plistFile({ ...dict, ...signInEntitlement(reach?.auth, simulator && !team) });
};

/** `reach` as a tvOS build uses it: tvOS bakes the iOS plan, so the keys
 * and entitlements of rows not on TV go (`reach.tvOmits`, LLP 1069.008.000 D5). */
export const tvReach = (reach) => {
  const omit = new Set(reach?.tvOmits ?? []);
  if (!reach || !omit.size) return reach;
  return { ...reach, usage: Object.fromEntries(Object.entries(reach.usage ?? {}).filter(([key]) => !omit.has(key))),
    entitlements: (reach.entitlements ?? []).filter((name) => !omit.has(name)) };
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
    ...Object.fromEntries((reach.flags ?? []).map((key) => [key, true])),
    ...reach.modules, ...(reach.bonjour?.length ? { NSBonjourServices: reach.bonjour } : {}),
    CFBundleDevelopmentRegion: reach.base,
    CFBundleLocalizations: reach.locales,
  };
};

/** `reach` with what the app's modules' implementations add to Info.plist
 * (`modules`): each declared dictionary, keyed by the strings keys the app
 * passes and valued by their base text (LLP 1133.002 D10). */
const modulePlist = (app, reach, target) => {
  const added = appleImplementations(app.dir, target).flatMap((p) => p.plist);
  if (!added.length) return reach;
  const base = reach?.base ?? 'en', strings = JSON.parse(readFileSync(resolve(app.dir, `strings/${base}.json`), 'utf8'));
  const modules = Object.fromEntries(added.map(({ key, keys }) => [key, Object.fromEntries(keys.map((k) => {
    if (typeof strings[k] !== 'string') throw new Error(`host/apple: ${key} names "${k}", which strings/${base}.json lacks`);
    return [k, strings[k]];
  }))]));
  return { ...reach, modules };
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

/** The iOS `Info.plist` from the manifest (LLP 1030 D2: one declaration; `build.mjs` consumes what it generates). The dev client's local-networking permission is `host.ios.localNetworking` (a string: the prompt); the store-required version numbers are counters bake owns, not authored. */
export const infoPlist = (app, device = false, { executable = executableName(app), id = app.id, name = app.displayName, development = null, icon = {}, distribution = null, distributed = false, reach = null, tv = false, clip = false } = {}) => {
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
    CFBundleSupportedPlatforms: [tv ? (device ? 'AppleTVOS' : 'AppleTVSimulator') : device ? 'iPhoneOS' : 'iPhoneSimulator'],
    DTPlatformName: tv ? (device ? 'appletvos' : 'appletvsimulator') : device ? 'iphoneos' : 'iphonesimulator',
    MinimumOSVersion: ios.minimumOS ?? '17.0',
    UIDeviceFamily: families,
    UILaunchScreen: {},
    UIApplicationSceneManifest: { UIApplicationSupportsMultipleScenes: false },
    ...(tv ? {} : { CADisableMinimumFrameDurationOnPhone: true }),
  };
  // An entry of either kind makes the app the group store's writer (`ExactWidgetKinds`, empty with activities
  // alone, is what WidgetContainer.configure reads); the activity keys are ActivityKit's (LLP 1126.001 §1).
  if (!tv && id === app.id && hasEntries(app)) Object.assign(dict, { ExactAppGroup: widgetGroup(app), ExactAppIdentifier: app.id, ExactWidgetKinds: platformWidgetEntries(app).map(e => e.kind),
    ...(activityEntries(app).length ? { NSSupportsLiveActivities: true, ...(activityEntries(app).some(e => e.frequent === true) ? { NSSupportsLiveActivitiesFrequentUpdates: true } : {}) } : {}) });
  Object.assign(dict, transportSecurity(ios, localNetworking(ios, reach), { web: !tv }));
  if (ios.localNetworking) dict.NSLocalNetworkUsageDescription = typeof ios.localNetworking === 'string' ? ios.localNetworking : 'Connects to your dev server on the local network.';
  Object.assign(dict, backgroundKeys(ios, reach, id, tv || clip), nfcKeys(ios, tv), startLocation(app));
  // The schemes `canOpenURL` may ask about; iOS answers false for any other (`UIApplication.canOpenURL(_:)`)
  if (ios.queriesSchemes?.length) dict.LSApplicationQueriesSchemes = ios.queriesSchemes;
  // @ref LLP 1096 D8 — the audio session's category, which ExactKit's one owner reads.
  if (app.manifest.audio_session) dict.ExactAudioSession = app.manifest.audio_session;
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
  // An App Clip is opened through its associated domains, never a URL scheme (Apple, "Choosing the right
  // functionality for your App Clip"), so the clip registers none: the full app owns the schemes and the dev link.
  if (clip) dict.NSAppClip = { NSAppClipRequestEphemeralUserNotification: false, NSAppClipRequestLocationConfirmation: false };
  else Object.assign(dict, openingLinks(app, 'ios', development, distributed), shortcutKeys(app, tv ? 'tvos' : 'ios'));
  Object.assign(dict, updateActivation(app));
  Object.assign(dict, usageKeys(reach));
  Object.assign(dict, icon);
  Object.assign(dict, distribution, orientationKeys(app.manifest.orientation));
  return plistFile(dict);
};

/** What App Store Connect reads from a distributed bundle and Xcode would
 * have written (`--archive`): the build's toolchain (DT* keys) and the store's
 * counters, EXACT_VERSION and EXACT_BUILD_NUMBER (the version and build
 * numbers App Store Connect requires to rise). */
export function distributionKeys(sdkName = 'iphoneos') {
  const xcode = read('xcodebuild', ['-version']).stdout ?? '';
  const [major, minor = '0', patch = '0'] = (/Xcode (\d+)(?:\.(\d+))?(?:\.(\d+))?/.exec(xcode) ?? []).slice(1);
  const sdk = (flag) => read('xcrun', ['--sdk', sdkName, flag]).stdout.trim();
  const sdkVersion = sdk('--show-sdk-version');
  const sdkBuild = sdk('--show-sdk-build-version');
  return {
    CFBundleVersion: process.env.EXACT_BUILD_NUMBER ?? '1',
    CFBundleShortVersionString: process.env.EXACT_VERSION ?? '0.1.0',
    DTCompiler: 'com.apple.compilers.llvm.clang.1_0',
    DTPlatformBuild: sdkBuild,
    DTPlatformVersion: sdkVersion,
    DTSDKBuild: sdkBuild,
    DTSDKName: `${sdkName}${sdkVersion}`,
    DTXcode: `${major.padStart(2, '0')}${minor}${patch}`,
    DTXcodeBuild: /Build version (\S+)/.exec(xcode)?.[1] ?? '',
    BuildMachineOSBuild: read('sw_vers', ['-buildVersion']).stdout.trim(),
    UIRequiredDeviceCapabilities: ['arm64'],
    // What iOS assumes when the key is absent, written out: App Store Connect
    // requires it (all four on iPad, for multitasking).
    UISupportedInterfaceOrientations: ['UIInterfaceOrientationPortrait', 'UIInterfaceOrientationLandscapeLeft', 'UIInterfaceOrientationLandscapeRight'],
    'UISupportedInterfaceOrientations~ipad': IPAD_ORIENTATIONS,
    // Export compliance is the developer's declaration, so it is written only
    // when EXACT_NON_EXEMPT_ENCRYPTION says it (`false` for HTTPS alone).
    ...(['true', 'false'].includes(process.env.EXACT_NON_EXEMPT_ENCRYPTION) ? { ITSAppUsesNonExemptEncryption: process.env.EXACT_NON_EXEMPT_ENCRYPTION === 'true' } : {}),
  };
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
export const macInfoPlist = (app, { development = null, icon = {}, reach = null, distributed = false } = {}) => plistFile({
  ...icon,
  CFBundleExecutable: executableName(app),
  CFBundleIdentifier: app.id,
  CFBundleName: app.displayName,
  CFBundleDisplayName: app.displayName,
  CFBundlePackageType: 'APPL',
  CFBundleVersion: '1',
  CFBundleShortVersionString: '0.1.0',
  LSMinimumSystemVersion: app.manifest.host?.macos?.minimumOS ?? '14.0',
  NSHighResolutionCapable: true,
  ...usageKeys(reach), // usage strings for the devices the app's grants name (LLP 1069.008)
  ...(app.manifest.host?.macos?.activationPolicy === 'accessory' ? { LSUIElement: true } : {}), // a menu-bar app (LLP 1133.025 D6)
  ...(app.manifest.host?.macos?.window ? { ExactWindow: app.manifest.host.macos.window } : {}),
  ...(app.manifest.display_override?.length ? { ExactDisplayOverride: app.manifest.display_override } : {}),
  ...transportSecurity(app.manifest.host?.macos, localNetworking({}, reach)),
  ...([...documentTypes(app), ...webPageTypes(app, distributed)].length ? { CFBundleDocumentTypes: [...documentTypes(app), ...webPageTypes(app, distributed)] } : {}),
  ...(exportedTypes(app).length ? { UTExportedTypeDeclarations: exportedTypes(app) } : {}),
  // Where a launch lands (LLP 1069.010 D4) is the manifest's `launch_handler`'s, with or
  // without `file_handlers`: File ▸ New Window needs no document type. Always written, so
  // absence, `auto` and an explicit `navigate-existing` are one plist.
  ExactLaunchMode: launchMode(app),
  ...startLocation(app),
  ...updateActivation(app),
  ...openingLinks(app, 'macos', development, distributed), ...shortcutKeys(app, 'macos'),
  ...(app.manifest.host?.macos?.oneTimeCodeAutofill === 'marked' ? { NSAutoFillRequiresTextContentTypeForOneTimeCodeOnMac: true } : {}),
});
/** The build receipt (LLP 1030 D2): what this binary was actually built from and with — the toolchain, the SDK, the identity and profile, the entitlements as signed — written beside it, never committed, so "what did this binary contain" is answered by a file. */
export function receipt(app, fields) {
  const version = (cmd, args) => (read(cmd, args).stdout ?? '').trim().split('\n')[0];
  return JSON.stringify({
    app: { id: app.id, name: app.displayName, origin: app.origin, declared: app.declared },
    built: new Date().toISOString(), started: new Date(Date.now() - process.uptime() * 1000).toISOString(), // started: before this build read a source
    toolchain: { rustc: version('rustc', ['--version']), swift: version('swift', ['--version']), xcode: version('xcodebuild', ['-version']) },
    ...fields,
  }, null, 2) + '\n';
}

/** The receipt a distributed bundle carries: the same, without the list of every file the binary was
 * compiled from (3,600 rows with this machine's paths, 1.4 of the receipt's 1.9 MB) or where its plan was
 * baked, which are the driver's evidence and nothing an installed app reads, and with each Cargo product
 * by its name alone. The whole receipt is written beside the artifact. */
export const shippedReceipt = (text) => {
  const whole = JSON.parse(text), { linkedPlan: _, ...build } = whole.build ?? {};
  return JSON.stringify({ ...whole, build: { ...build, binary: { sha256: build.binary?.sha256 },
    products: (build.products ?? []).map((product) => ({ ...product, path: basename(product.path) })) } }, null, 2) + '\n';
};

export function appleEmbedOnly(args) {
  return args.includes('--embed') && !args.includes('--archive') && !args.includes('--run') && !args.includes('--host');
}

export function copyAppleGpuProducts(capture, destination, artifacts, archive = false) {
  for (const m of artifacts) {
    if (archive) cpSync(resolve(capture, `${m.framework}.framework`), resolve(destination, `${m.framework}.framework`), { recursive: true });
    else copyFileSync(resolve(capture, m.built), resolve(destination, m.load));
  }
}

async function main(args, lockHeld = false) {
  let launchEnv;
  try { launchEnv = developmentLaunchEnvironment(args); }
  catch (e) { console.error(e.message); process.exitCode = 1; return; }
  const device = args.includes('--device');
  // tvOS runs the iOS simulator path with tvOS's target, SDK and plist.
  const tv = args.includes('--tvos');
  const ios = device || tv || args.includes('--ios');
  // Every Mac drive needs the bundle's identity and transport policy.
  if (!ios && !args.includes('--bundle')) args = [...args, '--bundle'];
  if (tv && (args.includes('--host') || args.includes('--embed'))) {
    console.error('--tvos takes neither --host nor --embed');
    process.exitCode = 1; return;
  }
  const tvTarget = device ? tvosDeviceTarget : tvosTarget;
  if (tv && !read('rustup', ['target', 'list', '--installed'], { cwd: root }).stdout?.split('\n').includes(tvTarget)) {
    console.error(`--tvos needs Rust's ${tvTarget} std, which this toolchain lacks. Install it with \`rustup target add ${tvTarget}\``);
    process.exitCode = 1; return;
  }
  const ipa = args.includes('--archive') ? resolve(process.cwd(), args[args.indexOf('--archive') + 1] ?? '') : null;
  const unsigned = args.includes('--unsigned');
  // What leaves this machine: an .ipa, or the Mac bundle `exact release` signs and notarises.
  const distribution = !!ipa || args.includes('--distribution');
  // What is stamped `release` or `archive` (buildinfo.mjs) bakes production trust, as --release does, so the stamp and
  // the trust every production policy reads (compat.json's `embedded.trust`) cannot disagree.
  if (distribution && process.env.EXACT_UPDATE_TRUST !== 'production') {
    if (process.env.EXACT_UPDATE_TRUST) console.error(`ignoring EXACT_UPDATE_TRUST=${process.env.EXACT_UPDATE_TRUST}: what is distributed bakes production trust`);
    process.env.EXACT_UPDATE_TRUST = 'production';
  }
  if ((unsigned && !ipa) || ipa && (!device || (!unsigned && (!process.env.EXACT_IDENTITY || !process.env.EXACT_PROFILE)) || args.includes('--run') || args.includes('--host'))) {
    console.error('--archive needs --device and EXACT_IDENTITY and EXACT_PROFILE (or --unsigned, which needs --archive), and takes neither --run nor --host');
    process.exitCode = 1; return;
  }
  const app = quietFor(resolveApp(args.find((a, i) => !a.startsWith('--') && !['--sim', '--phone', '--url', '--archive', '--optimize', '--link', '--watch-sim'].includes(args[i - 1]))), root);
  if (!ios && !args.includes('--watchos') && (args.includes('--painted-benchmark') || app.manifest.host?.macos?.presenter === 'painted')) {
    return (await import('../apple-painted/build.mjs')).buildPainted(app, args, { macInfoPlist });
  }
  if (args.includes('--watchos')) return (await import('./watch.mjs')).buildWatch(app, args, { runApple, appleArtifacts, appleBuildLock, receipt, buildParent: args => main(args, true) });
  // An app with an update store ships into a stream: the bake refuses a production build without its receipt. Said before Cargo.
  const store = `deploy.store.${ios ? 'ios' : 'macos'}`, streamed = app.manifest.deploy?.store?.[ios ? 'ios' : 'macos'] !== '0';
  if (distribution && streamed && !process.env.EXACT_UPDATE_RECEIPT && process.env.EXACT_UPDATE_GENESIS !== '1') { console.error(`${app.name}: what is distributed bakes production trust, and with an update store (${store} is not "0") it belongs to a stream: set EXACT_UPDATE_RECEIPT to its authenticated publisher receipt (scripts/deploy.mjs), or EXACT_UPDATE_GENESIS=1 to start a new stream`); process.exitCode = 1; return; }
  // A build of this app already running (a --prebuild's) is waited for, as test and agent wait for it, and a background
  // build's failure is said by the next build (scripts/agent-build.mjs `waitForBuild`, `reportPrebuild`).
  const running = !lockHeld && !args.includes('--prebuild') && existsSync(appleArtifacts(app).lock);
  const prebuilt = existsSync(resolve(app.dir, '.exact', `${tv ? 'tvos' : ios ? 'ios' : 'macos'}-prebuild.json`));
  const agentBuild = running || prebuilt || args.includes('--prebuild') ? await import('../../scripts/agent-build.mjs') : null;
  if (running) await agentBuild.waitForBuild(app);
  if (prebuilt) await agentBuild.reportPrebuild(app, args);
  if (args.includes('--prebuild')) { process.exitCode = await agentBuild.prebuild(app, args); return; } // LLP 1116.003 D1.2
  const gallery = process.env.EXACT_UPDATE_TRUST !== 'production' && !tv && (args.includes('--widget-gallery') || process.env.EXACT_WIDGET_GALLERY === '1');
  // An entry of either kind (a widget, a Live Activity) is the extension, the shared framework, the App Group and
  // the Keychain group (LLP 1126.001 WP13); tvOS has none of them.
  let widgets = !tv && (ios ? hasEntries(app) : platformWidgetEntries(app, 'macos').length > 0);
  // Another build may take the claim first after that wait: this one waits again rather than refusing as busy.
  const release = lockHeld ? () => {} : awaitBuildOutput(app, appleArtifacts(app).lock, () => console.error(`waiting for the Apple build of ${app.name} already running`));
  const cleanup = [], beside = []; // what to remove, and the steps started beside the build, when it ends
  try {
  const crate = app.crate(ios ? 'ios' : 'macos');
  const modules = app.modulesFor(ios ? 'ios' : 'macos');
  const gpuCrate = app.crate('gpu');
  const hasGpu = app.hasGpu;
  let ph, prof;
  let sha1 = unsigned ? '-' : device ? (() => {
    ph = ipa ? null : phone(args.includes('--phone') ? args[args.indexOf('--phone') + 1] : undefined, { tv });
    // With entries, the app's profile is one that grants the group (and the extension's exists), refused here,
    // before anything is built, with the edit; the bundle takes both again with the grants' entitlements.
    prof = widgets ? entryProfiles({ app, udid: ph?.udid }).app : profile(ph?.udid, app.id);
    return identity(prof.team);
  })() : ios ? '-' : (distribution ? process.env.EXACT_DEVELOPER_ID : null) ?? macIdentity();
  const macWidgets = !ios && widgets ? macWidgetSigning(app, sha1, { distribution }) : null;
  if (!ios && widgets && !macWidgets) sha1 = null;
  if (!ios) widgets = !!macWidgets;
  const dylib = `lib${gpuCrate.replace(/-/g, '_')}.dylib`;
  // What the presenter dlopens is the same name whatever the app is: one
  // Swift binary serves every app, and two apps' modules would otherwise
  // are captured and packaged together under the selected app's owner.
  const loadName = 'libexact_gpu.dylib';
  // Each declared GPU module's dylib beside it (LLP 1009 D6), under the name
  // GpuModule.loadName gives it; signed before the host bake binds its digest.
  const moduleDylibs = gpuModules(app.manifest).map(({ name }) => ({
    framework: `ExactGpu_${name.replace(/-/g, '_')}`, built: `lib${app.crate(`gpu-${name}`).replace(/-/g, '_')}.dylib`, load: `libexact_gpu_${name.replace(/-/g, '_')}.dylib` }));
  const gpuArtifacts = [...(hasGpu ? [{ built: dylib, load: loadName, framework: 'ExactGpu' }] : []), ...moduleDylibs];
  const preparedGpu = new Map();
  const webLoadName = 'libexact_web.dylib';
  const videoLoadName = 'libexact_video.dylib';
  const soundLoadName = 'libexact_sound.dylib', svgLoadName = 'libexact_svg.dylib';
  const webBuildDir = mkdtempSync(resolve(tmpdir(), 'exact-webarm-'));
  cleanup.push(webBuildDir);
  const webBuilt = resolve(webBuildDir, webLoadName);
  const t0 = Date.now();
  const target = ios ? (tv ? tvTarget : device ? 'aarch64-apple-ios' : iosTarget) : bakeTarget('macos');
  const library = await prepareLibrary(app.dir, target); // a package's prebuilt framework (LLP 1154 D3), fetched before Cargo
  const sdkName = ios ? (tv ? (device ? 'appletvos' : 'appletvsimulator') : device ? 'iphoneos' : 'iphonesimulator') : 'macosx', stampSdk = { sdk: sdkName, sdkVersion: designCompatible(app, ios ? 'ios' : 'macos') ? COMPATIBLE_SDK[ios ? 'ios' : 'macos'] : null }; // stampSdk: the SDK the link records
  const destination = ios ? (tv ? (device ? 'tvos' : 'tvos-simulator') : device ? 'ios' : 'ios-simulator') : 'macos';
  const sdk = read('xcrun', ['--sdk', sdkName, '--show-sdk-path']).stdout.trim();
  const targets = deploymentTargets(app);
  // The filter pictures' kernels (iOS; `SvgFilterMetal`), for the bundle,
  // when Xcode's Metal toolchain is installed.
  const svgFilterBuilt = ios ? svgFilterLibrary(sdkName, targets.ios, resolve(webBuildDir, svgFilterLibraryName)) : null;
  const cargoEnv = {
    ...developmentBuildEnv(),
    EXACT_WIDGET_GALLERY: gallery ? '1' : '0',
    SDKROOT: sdk,
    // An iOS build's Mac side is its build scripts and proc macros: the host's target, as every cargo call's.
    MACOSX_DEPLOYMENT_TARGET: ios ? process.env.MACOSX_DEPLOYMENT_TARGET || HOST_MACOS : targets.macos,
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
  // A production build optimizes for speed (`release`) or for size (`size`,
  // and Swift's -Osize; LLP 1047.001 D9): `--optimize`, else the manifest's
  // `host.<platform>.optimize`, else size for an embed and speed for an app.
  const optimize = (args.includes('--optimize') ? args[args.indexOf('--optimize') + 1] : undefined) ?? app.manifest.host?.[ios ? 'ios' : 'macos']?.optimize ?? (args.includes('--embed') ? 'size' : 'speed');
  if (!['speed', 'size'].includes(optimize)) throw new Error(`host/apple: --optimize is speed or size, not ${optimize}`);
  const production = cargoEnv.EXACT_UPDATE_TRUST === 'production';
  const cargoProfile = production ? (optimize === 'size' ? 'size' : 'release') : HOST_DEV;
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
  const hermes = ios && existsSync(resolve(app.dir, 'app.ts')) && cargoEnv.EXACT_JS_ENGINE !== 'stub' && hermesBundle(target, cargoEnv);
  if (hermes && !hermes.installed) throw new Error(`the pinned Hermes bundle for ${target} is not installed. Install it once with:\n  ${hermes.fix}`);
  // What the bake is expected to decide (the manifest's store level). The
  // shared Swift scratch is the same whatever it decides; the composition and
  // capture directory name the Swift host's environment before the bake ends.
  const expected = appleArtifacts(app, { destination, trust: cargoEnv.EXACT_UPDATE_TRUST, optimize: production ? optimize : 'speed' });
  const swiftBuildRoot = widgets ? resolve(expected.swift, 'widgets') : expected.swift;
  // A development build compiles the Swift host as it compiles its Rust
  // (`host-dev`): optimized, but file by file and incrementally, so an edited
  // Swift file is a 3-second build and not the whole module's 45 (an M4; LLP
  // 1036.000 §6). SwiftPM compiles that way only in its debug configuration,
  // so the optimization is asked for on top of it. A production bake and
  // what is distributed keep the whole-module build, the smaller and faster.
  const swiftWhole = production || distribution;
  // One `swift build` per product: given two `--product` flags SwiftPM
  // builds only the last; the second build is incremental and quick.
  const swiftArgs = ['build', '-c', swiftWhole ? 'release' : 'debug', '--scratch-path', swiftBuildRoot, ...(swiftWhole ? [] : ['-Xswiftc', '-O']),
    ...(production && optimize === 'size' ? ['-Xswiftc', '-Osize', '-Xswiftc', '-Xfrontend', '-Xswiftc', '-internalize-at-link'] : [])];
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
    // The same `--sysroot` on macOS: clang reads no SDK version from it, so the link recorded
    // the deployment target as the SDK (`sdk 14.0`), and AppKit, which keys its macOS 26 design
    // on the recorded SDK, drew every Exact app as on macOS 14; `-isysroot` records the SDK. The
    // triple carries `minimumOS`, as iOS's does: SwiftPM otherwise links at Package.swift's 14.
    swiftArgs.push('--triple', `${macTriple}${targets.macos}`, '-Xswiftc', '-Xclang-linker', '-Xswiftc', '-isysroot', '-Xswiftc', '-Xclang-linker', '-Xswiftc', sdk);
    // `designRequiresCompatibility`: the earlier design, by the one lever macOS
    // 27 keeps (it ignores UIDesignRequiresCompatibility) — the link records
    // the macOS 15 SDK, the last before the new design; this later
    // `-platform_version` wins over the driver's.
    if (designCompatible(app, 'macos')) swiftArgs.push('-Xlinker', '-platform_version', '-Xlinker', 'macos', '-Xlinker', targets.macos, '-Xlinker', COMPATIBLE_SDK.macos);
  }
  // SwiftPM compiles Package.swift itself for macOS before applying the iOS
  // product triple; an iPhone SDKROOT in its environment breaks that host
  // manifest compile. The target SDK stays in the explicit Swift arguments.
  const swiftEnv = (libDir, composition, link = 'all') => ({
    ...process.env,
    EXACT_WIDGET_GALLERY: gallery ? '1' : '0',
    EXACT_APPLE_LINK: link, EXACT_UPDATE_TRUST: cargoEnv.EXACT_UPDATE_TRUST ?? 'development', // LLP 1047.001 D4 (link.mjs); a production build compiles no inspect mode (Package.swift)
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
  const embedOnly = appleEmbedOnly(args);
  // The compiler, by its version: part of the name of what is kept from one build to the next.
  const swiftc = read('xcrun', ['--sdk', sdkName, 'swiftc', '--version']).stdout ?? '';
  // Where SwiftPM puts its products, once it has been asked (below).
  const binAnswer = resolve(swiftBuildRoot, `bin-path-${createHash('sha256').update(JSON.stringify([swiftc, swiftArgs])).digest('hex').slice(0, 16)}`);
  // The simulator's entitlements go in the executable's text section (below); the arguments name the file.
  const entitledArgs = (scratch, p) => ios && !device ? ['-Xlinker', '-sectcreate', '-Xlinker', '__TEXT', '-Xlinker', '__entitlements', '-Xlinker', resolve(scratch, `${p}-entitlements.plist`)] : [];
  // A development build whose last link in this scratch was this app's starts the product's build here, where
  // it only started ExactKit's compile: when the bake leaves the archive as it was, that build is the link,
  // and the half second SwiftPM takes to find nothing to do is spent beside Cargo, not after it. It holds the
  // link's claim from here; with another app linking, or anything else, the build is as it was. It links the
  // capabilities that link did (`<product>.linked`), since this build's are known only once the bake ends.
  const sole = args.includes('--host') ? null : ios ? 'ExactIOS' : 'ExactMac';
  const widgetCore = widgets ? prepareWidgetCore({ app, directory: resolve(pkg, '.build/widget-core'), swiftBuildRoot, swiftArgs, swiftc, triple: moduleTripleFor(app, ios, device, tv), device, platform: ios ? 'ios' : 'macos' }) : null;
  const coreFrameworkBuilt = widgetCore && existsSync(resolve(widgetCore.xcframework, 'Info.plist'));
  // A widget build links ExactCore as a framework (`sharedWidgetCore`), which an app's first build of a set
  // of ExactCore sources does not have yet. ExactKit is compiled against that framework only: an ExactCore
  // module built from source in this scratch would stay among SwiftPM's products and be found before the
  // framework, and ExactKit would be compiled against one ExactCore while the app links another. So with no
  // framework yet ExactKit is left to the product build. A module an older build left goes here, through
  // this compiler's and arguments' answer file, and again before the product builds, from the products'
  // directory itself, which other arguments share.
  const dropLeftoverCore = (bin) => { for (const name of ['ExactCore.swiftmodule', 'ExactCore.o']) rmSync(resolve(bin, name), { recursive: true, force: true }); };
  if (widgetCore && existsSync(binAnswer)) dropLeftoverCore(readFileSync(binAnswer, 'utf8'));
  const earlyEnv = (link = 'all') => ({ ...swiftEnv(expected.capture, expected.composition, link),
    ...(coreFrameworkBuilt ? { EXACT_CORE_FRAMEWORK: relative(pkg, widgetCore.xcframework) } : {}) });
  const early = (() => {
    if (widgets || embedOnly || cargoProfile !== HOST_DEV || !sole || !existsSync(binAnswer)) return null;
    const bin = readFileSync(binAnswer, 'utf8');
    let was = null;
    try { was = JSON.parse(readFileSync(resolve(bin, `${sole}.linked`), 'utf8')); } catch { return null; }
    if (was.intents || (was.library ?? null) !== (library?.sha256 ?? null) || was.libDir !== expected.capture || was.lib !== crate.replace(/-/g, '_') || typeof was.link !== 'string' || !existsSync(resolve(bin, sole)) || !existsSync(resolve(expected.capture, `lib${was.lib}.a`))) return null;
    if (ios && !device && !existsSync(resolve(expected.scratch, `${sole}-entitlements.plist`))) return null;
    if (was.plan && !existsSync(resolve(expected.scratch, 'plan/app.hbc'))) return null;
    let claim;
    try { claim = claimBuildOutput(app, expected.swiftLock); } catch { return null; }
    const step = startApple('swift', [...swiftArgs, ...entitledArgs(expected.scratch, sole), ...appleLinkArgs(library, ios), ...linkedPlanArgs(was.plan && resolve(expected.scratch, 'plan')), '--product', sole], resolve(webBuildDir, 'swift-compile.log'),
      { cwd: pkg, env: swiftEnv(expected.capture, expected.composition, was.link) });
    beside.push(step);
    return { claim, step };
  })();
  const hostCompile = embedOnly || early || (widgetCore && !coreFrameworkBuilt) ? null : startApple('swift', [...swiftArgs, '--target', 'ExactKit'], resolve(webBuildDir, 'swift-compile.log'),
    { cwd: pkg, env: earlyEnv() });
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
  const fixedPlan = production && expected.composition === 'embedded';
  cargoEnv.EXACT_APPLE_LINK = linksByPlan(production, args) ? 'plan' : 'all'; // what the archive links (link.mjs)
  const hostModules = embedOnly || fixedPlan ? null : buildModules(['exact-svg-raster', ...(metal ? ['exact-canvas-vello'] : []), ...(ios ? [] : ['exact-listen'])], 'host-modules.log'); // the listener: macOS's (LLP 1133.005 §8)
  const buildReceipt = contractLast(() => buildBake(app, ios ? 'ios' : 'macos', target, { env: cargoEnv, profile: cargoProfile, link: !args.includes('--embed'), prepareGpu(product) {
    const artifact = gpuArtifacts.find(m => m.built === basename(product));
    if (!artifact) throw new Error(`unknown GPU product: ${product}`);
    const signed = signedGpuProduct(product, { identity: sha1, app, framework: ipa ? artifact.framework : null, platform: tv ? 'AppleTVOS' : 'iPhoneOS', hardened: !ios && distribution });
    preparedGpu.set(artifact.built, signed);
    return signed;
  }, capture(buildReceipt) {
    const composition = buildReceipt.compat.inputs?.store?.L === '0' ? 'embedded' : 'updating';
    paths = appleArtifacts(app, { destination, composition, trust: cargoEnv.EXACT_UPDATE_TRUST, optimize: production ? optimize : 'speed' });
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
    for (const m of gpuArtifacts) {
      const source = preparedGpu.get(m.built);
      if (ipa) { cpSync(dirname(source), resolve(capture, `${m.framework}.framework`), { recursive: true }); rmSync(resolve(capture, `${m.framework}.framework`, m.framework)); }
      take(source, ipa ? `${m.framework}.framework/${m.framework}` : m.built);
    }
    bakedPlan = readFileSync(resolve(cargoEnv.EXACT_BAKE_OUTPUT, `${ios ? 'ios' : 'macos'}-${target}.plan`));
    copyAppleStaticTrees(app.dir, capture, [['assets', 'assets'], ['deck', 'deck']]);
    copyAppleStaticTrees(resolve(cargoEnv.EXACT_BAKE_OUTPUT, `${ios ? 'ios' : 'macos'}-${target}.entries`), capture, [['plans', 'plans'], ['typescript', 'typescript'], ['activities', 'activities']]);
    copyShaders(app, resolve(capture, 'shaders'));
    if (buildReceipt.rust) copyStaticTreeIfPresent(buildReceipt.rust, resolve(capture, 'rust'));
    verifyBakeFiles(buildReceipt.compat, bakedPlan, listAssets(capture, true, ['typescript']));
    placeAppleArtifact(capture, paths.capture);
    writeFileSync(heldAt, JSON.stringify(holds));
  } }));
  const libDir = paths.capture;
  const bakedCompat = buildReceipt.compat;
  // What this Apple destination derives from the grants: the bake's, less what tvOS has not.
  const appleReach = modulePlist(app, tv ? tvReach(bakedCompat.reach) : bakedCompat.reach, tv ? 'tvos' : ios ? 'ios' : 'macos');
  if (!ios) macPresence(app, bakedCompat.inputs?.grantCeiling, { release: production || distribution, distribution }); // LLP 1133.010 D7
  if (!ios) macPasskeys(app, bakedCompat.inputs?.grantCeiling, { release: production || distribution }); // LLP 1133.011 D6
  if (!ios) macProfileRows(app, bakedCompat.inputs?.grantCeiling, { release: production || distribution }); // LLP 1133.028 D17
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
  // `host/apple`. With --archive it also builds the IPA; --embed alone
  // builds no Swift. The sample hosts prove that the same pieces link.
  const { link: appleLink, products: swiftCapabilities } = appleComposition(buildReceipt.graph, linksByPlan(production, args), app.manifest.link, bakedCompat.reach?.auth?.sessions === true);
  if (args.includes('--embed')) {
    const platform = ios ? (device ? 'ios' : 'ios-simulator') : 'macos';
    const gpu = hasGpu ? (ipa ? 'ExactGpu.framework/ExactGpu' : loadName) : null;
    const embed = mkdtempSync(resolve(paths.namespace, '.embed-'));
    cleanup.push(embed);
    mkdirSync(resolve(embed, 'include'), { recursive: true });
    const archive = `lib${crate.replace(/-/g, '_')}.a`;
    copyFileSync(resolve(libDir, archive), resolve(embed, archive));
    copyFileSync(resolve(pkg, 'include/exact.h'), resolve(embed, 'include/exact.h'));
    copyAppleGpuProducts(libDir, embed, gpuArtifacts, !!ipa);
    // Beside the executable in the embedding app's bundle, as the standalone app has it.
    if (svgFilterBuilt) copyFileSync(svgFilterBuilt, resolve(embed, svgFilterLibraryName));
    copyAppleStaticTrees(paths.capture, embed);
    verifyBakeFiles(bakedCompat, bakedPlan, listAssets(embed, true, ['typescript']));
    writeFileSync(resolve(embed, 'compat.json'), JSON.stringify(bakedCompat, null, 2) + '\n');
    // What the consumer links beside the archive and installs before its first session (LLP 1047.001 D4).
    writeFileSync(resolve(embed, 'capabilities.json'), JSON.stringify({ products: ['ExactKit', ...swiftCapabilities], install: swiftCapabilities.map((product) => `${product}.install()`), optimize, swiftFlags: production && optimize === 'size' ? ['-Osize', '-Xfrontend', '-internalize-at-link'] : [] }, null, 2) + '\n');
    writeFileSync(resolve(embed, 'receipt.json'), receipt(app, { platform, target: ios ? target : (process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin'), sdk, archive, gpu, package: pkg, composition, compatibilityId:bakedCompat.id, build:buildReceipt }));
    const bytes = statSync(resolve(embed, archive)).size;
    placeAppleArtifact(embed, paths.embed);
    console.log(`host/apple: ${paths.embed.replace(root + '/', '')} — ${archive} ${(bytes / 1048576).toFixed(2)} MB, include/exact.h${gpu ? `, ${gpu}` : ''}${svgFilterBuilt ? `, ${svgFilterLibraryName} (the bundle's top level)` : ios ? ', no SVG filter kernels (no Metal toolchain)' : ''}, shaders, assets, compat.json; link it with ExactKit${composition === 'updating' ? ' + ExactUpdates' : ''} from the package at ${pkg.replace(root + '/', '')}`);
    if (embedOnly) return;
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
  // @ref LLP 1098 D8 — the lock screen and Control Center show only a non-mixable playback session's media, and the audio
  // stops at the lock without the background mode: an app that claims the media session states both.
  const iosModes = app.manifest.host?.ios?.backgroundModes ?? [];
  if (ios && !tv && buildReceipt.graph.mediaSession && !(app.manifest.audio_session === 'playback' && iosModes.includes('audio'))) throw new Error('host/apple: a media session needs `audio_session: "playback"` and `"audio"` in `host.ios.backgroundModes` in app.json on iOS: the lock screen shows only a playback session\'s media');
  const reaches = buildReceipt.graph.loads;
  const settled = production && level === '0' && Array.isArray(reaches);
  const carries = (module) => !settled || reaches.includes(module);
  const leftOut = settled ? ['canvas', 'svg', 'video', 'web', 'sound', 'midi', 'listen'].filter((module) => !reaches.includes(module)) : [];
  if (leftOut.length) console.log(`host/apple: the plan cannot change (production, store level 0) and reaches no ${leftOut.join(', ')} module: left out of this build`);
  const canvasGpu = metal && carries('canvas');
  const lateCrates = fixedPlan && !embedOnly ? [...(carries('svg') ? ['exact-svg-raster'] : []), ...(canvasGpu ? ['exact-canvas-vello'] : []), ...(!ios && carries('listen') ? ['exact-listen'] : [])] : [];
  const lateModules = lateCrates.length ? buildModules(lateCrates, 'late-modules.log') : null;
  const env = swiftEnv(libDir, composition, appleLink);
  // The products: the standalone app, and with --host the sample host too
  // (LLP 1031 D10 — the fixture the smoke drives).
  const products = [ios ? 'ExactIOS' : 'ExactMac', ...(args.includes('--host') ? [ios ? 'ExactHostIOS' : 'ExactHostMac'] : [])];
  const product = products[0];
  // The scratch is shared by every app of this destination (appleArtifacts);
  // `linkRoot` is this app's own, for what names it.
  const linkRoot = paths.scratch;
  mkdirSync(linkRoot, { recursive: true });
  const linkedPlan = buildReceipt.linkedPlan ? placeLinkedPlan(resolve(linkRoot, 'plan'), bakedPlan, readFileSync(resolve(cargoEnv.EXACT_BAKE_OUTPUT, `${ios ? 'ios' : 'macos'}-${target}.json`)), bakedCompat.id, readFileSync(resolve(buildReceipt.linkedPlan, 'app.hbc'))) : null;
  const shared = widgets ? sharedWidgetCore({ app, prepared: widgetCore, libDir, crate, env, runApple, linkedPlan }) : null;
  if (shared) env.EXACT_CORE_FRAMEWORK = relative(pkg, shared.xcframework);
  rmSync(resolve(paths.namespace, 'swift'), { recursive: true, force: true }); // the per-app scratch this replaced
  const binDir = mkdtempSync(resolve(paths.namespace, '.products-')); cleanup.push(binDir);
  const intents = tv ? null : buildIntents({ app, compat: bakedCompat, directory: resolve(linkRoot, 'intents'), sdkName, sdk, triple: moduleTripleFor(app, ios, device, tv), platform: ios ? 'ios' : 'macos', deployment: ios ? targets.ios : targets.macos, swiftc, runApple });
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
  const frameSources = ['FrameBrowser.swift', 'FrameWindows.swift', 'FrameCapture.swift', 'FramePDF.swift', 'FrameAutomation.swift', 'FrameFiles.swift'].map(f => resolve(root, 'host/apple/webarm', f)), ownArgs = (args) => args.filter(value => !frameSources.includes(value)), webArgs = ['--sdk', sdkName, 'swiftc', '-module-cache-path', resolve(swiftBuildRoot, 'webarm-module-cache'), '-parse-as-library', '-emit-library', '-O', '-module-name', 'ExactWebArm', resolve(root, 'host/apple/webarm/WebArm.swift'), ...frameSources, '-o', webBuilt, '-framework', 'WebKit', '-framework', 'PDFKit']; // its other sources, a partitioned frame's browser, every frame's windows, dialogs and downloads, its pictures, a local PDF's view and its automation (LLP 1020.002), a page's given files (LLP 1020.001 D7), the other arms' arguments drop
  if (ios) {
    webArgs.push('-target', iosTripleFor(app, device, tv), '-sdk', sdk);
  } else {
    webArgs.push('-target', `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-macos${targets.macos}`);
  }
  // An arm is compiled once per sources, arguments and compiler, and kept in the
  // scratch path (rebuilt every time, two cost 93 s of a build at load 120): the
  // host's are shared by every app's build, an app's module artifact (`own`)
  // stays in its own directory. Each is written under this process's name and
  // renamed, as another app's build may compile it at once; a cached one is
  // copied out at once and marked used, so another build's `evictArms` keeps it.
  const snapshot = snapshotSources(modules.apple, resolve(webBuildDir, 'module-sources')); if (modules.apple.length) console.log(`host/apple: the app's Swift modules compile as they are now (${modules.apple.length} copied); an edit from here on needs another build`);
  // Where a module's Swift packages compile (LLP 1133.001 D5), this build's own: an arm's key names it `<packages>`.
  const packageDir = resolve(webBuildDir, 'packages');
  const arm = (args, source, built, extra = '', own = false, optional = false) => { // optional: placing a failed one answers its log's path
    const key = createHash('sha256').update(JSON.stringify([args.map(a => a === built ? '<out>' : snapshot.key(a).replace(packageDir, '<packages>')), [].concat(source).map(f => readFileSync(f, 'utf8')), swiftc, extra])).digest('hex').slice(0, 16);
    const dir = resolve(own ? linkRoot : swiftBuildRoot, 'arms'), cached = resolve(dir, `${key}-${basename(built)}`);
    try { const now = new Date(); utimesSync(cached, now, now); copyFileSync(cached, built); return Object.assign(async () => true, { cached }); } catch {} // none, or evicted meanwhile
    mkdirSync(dir, { recursive: true }); if (args.includes(packageDir)) placePackages(); // the packages' objects only for an arm compiled anew
    const making = `${cached}.${process.pid}.tmp`;
    const log = resolve(webBuildDir, `arm-${beside.length}.log`), compile = startApple('xcrun', args.map(a => a === built ? making : a), log, { authored: snapshot.authored });
    beside.push(compile);
    return Object.assign(async () => {
      if (!await compile.done({ repeated: optional })) return log;
      // Place this build's result before publishing it to the shared eviction pool.
      copyFileSync(making, built);
      renameSync(making, cached);
      evictArms(dir, basename(built)); return true;
    }, { cached }); // its copy's path: a probe's roster is kept beside it
  };
  const arms = [];
  // tvOS has no WebKit, so no iframe arm there.
  const hasWeb = !tv && carries('web'), hasVideo = carries('video'), hasSvg = carries('svg'), hasListen = !ios && carries('listen');
  if (hasWeb) arms.push(arm([...webArgs, ...(!production && !distribution ? ['-D', 'EXACT_DEVELOPMENT'] : [])], [resolve(root, 'host/apple/webarm/WebArm.swift'), ...frameSources], webBuilt));
  const videoBuilt = resolve(webBuildDir, videoLoadName);
  // The video arm and its media session (LLP 1098 D7): both sources are its key and its inputs, with MediaPlayer.
  const video = ['VideoArm.swift', 'NowPlaying.swift'].map(f => resolve(root, 'host/apple/videoarm', f));
  const videoArgs = ownArgs(webArgs).flatMap(value => value === 'ExactWebArm' ? ['ExactVideoArm'] : value === resolve(root, 'host/apple/webarm/WebArm.swift') ? video : value === webBuilt ? [videoBuilt] : value === 'WebKit' ? ['AVKit', '-framework', 'MediaPlayer'] : [value]);
  if (hasVideo) arms.push(arm(videoArgs, video, videoBuilt));
  // The sound arm (LLP 1096 D8): Swift over a C mixer and AVAudioEngine's raising calls in Objective-C.
  const hasSound = carries('sound'), soundBuilt = resolve(webBuildDir, soundLoadName);
  if (hasSound) {
    const sound = (f) => resolve(root, 'host/apple/soundarm', f), triple = webArgs[webArgs.indexOf('-target') + 1], flags = ['-O2', '-fobjc-arc', '-fobjc-arc-exceptions'];
    const objects = ['sound_render.c', 'sound_engine.m'].map((file) => {
      const object = resolve(swiftBuildRoot, 'arms', `${file.replace('.', '-')}-${createHash('sha256').update(readFileSync(sound(file))).update(readFileSync(sound('sound_render.h'))).update(`${triple} ${sdk} ${flags.join(' ')}`).digest('hex').slice(0, 16)}.o`);
      if (existsSync(object)) return object;
      mkdirSync(dirname(object), { recursive: true });
      run('xcrun', ['--sdk', sdkName, 'clang', '-c', ...flags, '-target', triple, sound(file), '-o', `${object}.${process.pid}.tmp`]);
      renameSync(`${object}.${process.pid}.tmp`, object); return object;
    });
    const soundArgs = [...ownArgs(webArgs).map(value => value === 'ExactWebArm' ? 'ExactSoundArm' : value === resolve(root, 'host/apple/webarm/WebArm.swift') ? sound('SoundArm.swift') : value === webBuilt ? soundBuilt : value === 'WebKit' ? 'AVFoundation' : value), '-import-objc-header', sound('sound_render.h'), ...objects];
    arms.push(arm(soundArgs, [sound('SoundArm.swift'), sound('sound_render.c'), sound('sound_engine.m'), sound('sound_render.h')], soundBuilt));
  }
  // The MIDI arm (LLP 1125 D6): CoreMIDI on a thread of its own; tvOS has none.
  const hasMidi = !tv && carries('midi'), midiBuilt = resolve(webBuildDir, 'libexact_midi.dylib'), midiSource = resolve(root, 'host/apple/midiarm/MIDIArm.swift'), midiRing = resolve(root, 'host/apple/midiarm/midi_ring.h');
  if (hasMidi) arms.push(arm(ownArgs(webArgs).map(value => value === 'ExactWebArm' ? 'ExactMIDIArm' : value === resolve(root, 'host/apple/webarm/WebArm.swift') ? midiSource : value === webBuilt ? midiBuilt : value === 'WebKit' ? 'CoreMIDI' : value).concat(['-import-objc-header', midiRing]), [midiSource, midiRing], midiBuilt));
  // The loose arms, each where the build carries it: [carried, built, load name, its framework on iOS].
  const looseArms = [[hasWeb, webBuilt, webLoadName, 'ExactWeb'], [hasVideo, videoBuilt, videoLoadName, 'ExactVideo'], [hasSound, soundBuilt, soundLoadName, 'ExactSound'], [hasMidi, midiBuilt, 'libexact_midi.dylib', 'ExactMIDI'], ...(ios ? [] : [[hasListen, moduleLib['exact-listen'], 'libexact_listen.dylib', null]])];
  // @ref LLP 1024 D3/D8.4 — the app's one module artifact, only when the app
  // has modules (the GPU gate): the host's table glue and the app's own
  // `modules/apple/*.swift`, one dylib under one load name. A release build
  // whose roster names a tag the artifact lacks fails here, named.
  const modulesLoadName = 'libexact_modules.dylib';
  // @ref LLP 1075.003 Q2 — the app's `data-*` words as typed keys, written
  // from app.json `data` beside the glue (built, never committed).
  const dataKeys = resolve(linkRoot, `ExactDataKeys-${app.id}.swift`);
  // @ref LLP 1133.001 D4, D5 — the modules' Swift implementations the app names, each package compiled as a Swift
  // module of its own and composed into the app's one module by an adapter written here (never committed).
  const packages = appleImplementations(app.dir, tv ? 'tvos' : ios ? 'ios' : 'macos'), composing = modules.apple.length > 0 || packages.length > 0;
  // A package that asks for AlarmKit's countdown compiles the host's metadata type into its own module, on iOS
  // only, as the extension compiles it (LLP 1133.030 D8 item 1).
  for (const p of packages) if (p.activity === 'alarmkit' && ios && !tv) p.sources = [...p.sources, resolve(root, 'host/apple/widget/AlarmAttributes.swift')];
  // Each package compiles against `ExactPackage` (host/apple/modules/ExactPackage.swift), a Swift module of its own
  // compiled first and linked into the app's module with them.
  const seam = resolve(root, 'host/apple/modules/ExactPackage.swift');
  const composed = resolve(linkRoot, `ExactComposed-${app.id}.swift`), packageObjects = packages.length ? [resolve(packageDir, 'ExactPackage.o'), ...packages.map(p => resolve(packageDir, `${p.module}.o`))] : [];
  // Each package's Swift copied as it is when the build begins (an edit meanwhile lands in the next build, as the app's
  // own sources do): what it compiles from and what an arm linking it is keyed by. Each copy says where it came from
  // (`#sourceLocation`), so a diagnostic names the package's file and `#filePath` its place in the package.
  const packageCopies = packages.map(p => p.sources.map((f, i) => {
    const copy = resolve(packageDir, `src-${p.module}`, `${i}-${basename(f)}`);
    mkdirSync(dirname(copy), { recursive: true });
    writeFileSync(copy, `#sourceLocation(file: ${JSON.stringify(realpathSync(f))}, line: 1)\n${readFileSync(f, 'utf8')}`);
    return copy;
  })), packageSources = packages.length ? [seam, ...packageCopies.flat()] : [];
  if (composing) writeFileSync(composed, composedModule(packages, modules.apple.length > 0));
  if (composing) writeDataKeys(app, dataKeys, ios ? 'ios' : 'macos');
  // @ref LLP 1121 D12 — the token names `contract tamber gen swift` writes from the design (never committed).
  if (composing) (await import('../../scripts/tamber-names.mjs')).writeTamberNames(app, 'swift'); const tamberNames = resolve(app.dir, '.exact/tamber/tamber.swift');
  const moduleSources = composing ? [...moduleSeam(resolve, root), composed, ...(existsSync(tamberNames) ? [tamberNames] : []), dataKeys, ...snapshot.copies] : [];
  const modulesBuilt = moduleSources.length ? resolve(webBuildDir, modulesLoadName) : null;
  // The slice of each `modules/apple/*.xcframework` for this build, read
  // from the xcframework's own Info.plist (`AvailableLibraries`: platform,
  // variant, architectures), never guessed from a directory name; Mac
  // Catalyst is a variant of its own, matched only for the roster probe. A static library's
  // slice gives `-I` its headers, `-L` it and `-l` each archive in it; a
  // framework's slice gives `-F` it and `-framework` its name.
  const frameworkSlice = (fw, forIos, simulator, arch) => {
    const platform = forIos ? 'ios' : 'macos', variant = forIos && simulator ? (simulator === true ? 'simulator' : simulator) : undefined;
    let dir, lib; try { ({ dir, lib } = xcframeworkSlice(fw, { platform, variant, arch })); } catch (e) { throw new Error(`host/apple: ${e.message}`); }
    const framework = lib.LibraryPath.endsWith('.framework');
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
  // tvOS links only what a package declares for `tvos`: AlarmKit is iOS's alone.
  const linkArgs = (forIos, os = forIos ? 'ios' : 'macos') => [...(app.manifest.host?.[os]?.link ?? []), ...systemFrameworks(packages, forIos && tv ? 'tvos' : os, app.manifest.host?.[os]?.minimumOS)];
  // What the arm cache must see change: the flags, and each library's bytes.
  const frameworkStamp = (forIos, simulator, arch) => JSON.stringify([linkArgs(forIos), ...(modules.frameworks ?? []).flatMap(fw => frameworkSlice(fw, forIos, simulator, arch).stamped).map(f => { const st = statSync(f); return [f, st.size, st.mtimeMs]; })]);
  const macArch = process.arch === 'arm64' ? 'arm64' : 'x86_64';
  const iosArch = device ? 'arm64' : (iosTriple.startsWith('arm64') ? 'arm64' : 'x86_64');
  const moduleArgs = (sdkFor, targetArgs, out, forIos = false, simulator = false, arch = macArch) => ['--sdk', sdkFor, 'swiftc', '-module-cache-path', resolve(swiftBuildRoot, 'modules-module-cache'), '-parse-as-library', '-emit-library', '-O', '-swift-version', '5', '-module-name', 'ExactAppModules', ...moduleSources, ...(packages.length ? ['-I', packageDir, ...packageObjects] : []), ...frameworkArgs(forIos, simulator, arch), ...linkArgs(forIos), '-o', out, ...targetArgs];
  const macTarget = ['-target', moduleTripleFor(app, false)];
  // Each package's Swift module for this target, before the app's module links it: once, and only when an arm
  // linking it is compiled anew (its copied sources and `packageStamp` are that arm's key; its objects' paths this
  // build's own). A failed package stays unplaced, so the next arm that needs it compiles it again and says why.
  const swiftModuleArgs = (name, sources) => ['--sdk', sdkName, 'swiftc', '-module-cache-path', resolve(swiftBuildRoot, 'modules-module-cache'), '-parse-as-library', '-wmo', '-O', '-swift-version', '5', '-module-name', name, '-emit-module', '-emit-module-path', resolve(packageDir, `${name}.swiftmodule`), '-I', packageDir, '-c', ...sources, '-o', resolve(packageDir, `${name}.o`), ...(ios ? ['-target', moduleTripleFor(app, true, device, tv), '-sdk', sdk] : macTarget)];
  const packageArgs = (p, i) => [...swiftModuleArgs(p.module, packageCopies[i]), ...(!production && !distribution ? ['-D', 'EXACT_DEVELOPMENT'] : [])]; // a package's development-only paths (an injected device, LLP 1133.037 D19 item 3), as the web arm's
  // What an arm linking the packages adds to its key: each package, its sources by name, and how it compiles.
  const packageStamp = JSON.stringify(packages.map((p, i) => [p.module, p.sources, packageArgs(p, i).map(a => a.replaceAll(packageDir, '<packages>'))]));
  const seamArgs = () => swiftModuleArgs('ExactPackage', [seam]);
  let placed = false;
  const placePackages = () => {
    if (placed) return;
    if (packages.length) {
      const r = spawnSync('xcrun', seamArgs(), { encoding: 'utf8' });
      if (r.status !== 0) throw new Error(`host/apple: ExactPackage.swift did not compile:\n${r.stderr ?? ''}`);
    }
    packages.forEach((p, i) => {
      const r = spawnSync('xcrun', packageArgs(p, i), { encoding: 'utf8' });
      // Each copy's real path first (the temporary directory's `/private/var/…`), as `snapshot.authored` maps the app's.
      const said = packageCopies[i].reduce((text, copy, j) => text.replaceAll(realpathSync(copy), p.sources[j]).replaceAll(copy, p.sources[j]), r.stderr ?? '');
      if (r.status !== 0) throw new Error(`host/apple: ${p.extension}'s Swift (${p.sources.join(', ')}) did not compile:\n${said}`);
    });
    placed = true;
  };
  if (modulesBuilt) arms.push(arm(moduleArgs(sdkName, ios ? ['-target', moduleTripleFor(app, true, device, tv), '-sdk', sdk] : macTarget, modulesBuilt, ios, ios && !device, ios ? iosArch : macArch), [...moduleSources, ...packageSources], modulesBuilt, frameworkStamp(ios, ios && !device, ios ? iosArch : macArch) + packageStamp, true));
  // The roster the artifact serves (its tags, and which join the first frame) is read by running its table: a macOS build
  // dlopens its artifact; an iOS build links the same sources (their iOS code) with each xcframework's slice for a route, and
  // the first route that reads it stands: a Mac Catalyst executable run here, an iOS simulator one spawned on a booted
  // simulator (UIKit will not start without one: any booted, else one booted here and shut down after), a macOS dylib dlopened
  // here. A route with no slice, or whose probe does not compile, load or run, falls through; a roster read is final. tvOS has
  // the macOS route alone, and no first-frame tag. The first route compiles beside the build, a later one only when needed.
  let rosterKeys = {};
  const routes = !(modulesBuilt && modules.tags.length && typeof Bun !== 'undefined') ? [] : !ios ? [{ via: 'macOS', built: modulesBuilt, placed: async () => true }]
    : [...(tv ? [] : [['Mac Catalyst', 'maccatalyst'], ['iOS Simulator', true]]), ['macOS', false]].map(([via, variant]) => ({ via, variant,
      // The Swift packages compile for the build's own target, which only a simulator build's simulator route shares.
      lacking: (packages.length && !(variant === true && !device) ? `${packages[0].extension} (a Swift package, compiled for this build's target)` : null) ?? (modules.frameworks ?? []).find(fw => { try { frameworkSlice(fw, variant !== false, variant, macArch); return false; } catch { return true; } }),
      built: resolve(webBuildDir, variant === false ? 'probe-' + modulesLoadName : `probe-roster-${variant === true ? 'simulator' : variant}`) }));
  const compileProbe = ({ variant, built }, probeMain = resolve(linkRoot, 'ExactRosterProbe.swift')) => {
    if (variant === false) return arm(moduleArgs('macosx', macTarget, built), [...moduleSources, ...packageSources], built, frameworkStamp(false, false, macArch) + packageStamp, true, true);
    writeFileSync(probeMain, '@main enum ExactRosterProbe { static func main() { print(String(cString: exactNativeAbi().load(fromByteOffset: 8, as: UnsafePointer<CChar>.self))) } }\n');
    const on = variant === true ? 'iphonesimulator' : 'macosx', catalyst = variant !== true && resolve(read('xcrun', ['--sdk', 'macosx', '--show-sdk-path']).stdout.trim(), 'System/iOSSupport');
    const target = catalyst ? ['-target', `${macArch}-apple-ios${targets.ios}-macabi`, '-Fsystem', `${catalyst}/System/Library/Frameworks`, '-I', `${catalyst}/usr/lib/swift`, '-L', `${catalyst}/usr/lib/swift`] : ['-target', `${macArch}-apple-ios${targets.ios}-simulator`, '-sdk', read('xcrun', ['--sdk', on, '--show-sdk-path']).stdout.trim()];
    return arm(moduleArgs(on, [...target, probeMain], built, true, variant, macArch).filter(a => a !== '-emit-library'), [...moduleSources, ...packageSources, probeMain], built, frameworkStamp(true, variant, macArch) + packageStamp, true, true);
  };
  const firstRoute = routes.find(r => !r.lacking); if (firstRoute && !firstRoute.placed) try { firstRoute.placed = compileProbe(firstRoute); } catch (e) { firstRoute.placed = () => { throw e; }; }
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
    if (widgetCore) dropLeftoverCore(swiftBinDir);
    const archive = buildReceipt.products.find(made => made.path === resolve(cargoLibDir, `lib${crate.replace(/-/g, '_')}.a`))?.sha256;
    // The linked plan, compat.json and bytecode, as the bake verified them, where this app alone links from (and its next early build).
    for (const p of products) {
      const productArgs = [...swiftArgs, ...entitledArgs(linkRoot, p), ...(intents ? ['-Xlinker', intents.object] : []), ...(shared ? ['-Xlinker', '-rpath', '-Xlinker', ios ? '@executable_path/Frameworks' : '@executable_path/../Frameworks'] : []), ...appleLinkArgs(library, ios, !shared), ...linkedPlanArgs(shared ? null : linkedPlan?.dir)];
      let entitled = null;
      if (ios && !device) {
        // Simulator Security reads entitlements from the Mach-O text section.
        // Device-style entitlements in its ad-hoc signature can prevent launch.
        entitled = entitlements({...app, widgetTV: tv, widgetHost: p === 'ExactHostIOS', id: p === 'ExactHostIOS' ? `${app.id}.host` : app.id}, null, true, appleReach, { simulator: true });
        const ent = resolve(linkRoot, `${p}-entitlements.plist`);
        // Written only when it differs: the early build may be reading it.
        if (!existsSync(ent) || readFileSync(ent, 'utf8') !== entitled) writeFileSync(ent, entitled);
      }
      // swift build does not see the Rust archive (or that plist, or a linked plan) change, so the executable is
      // dropped to be linked again: 0.4 s, and it was every build's. A development build drops only one linked from
      // other bytes, which `<product>.linked` beside it says: the archive's hash, whose it is, the plist, the plan's.
      const linked = resolve(swiftBinDir, `${p}.linked`), from = JSON.stringify({ archive: archive ?? null, libDir, lib: env.EXACT_LIB ?? null, link: appleLink, entitled, shared: shared?.xcframework ?? null, plan: linkedPlan?.digest ?? null, intents: intents?.digest ?? null, library: library?.sha256 ?? null });
      const same = cargoProfile === HOST_DEV && archive && existsSync(resolve(swiftBinDir, p)) && existsSync(linked) && readFileSync(linked, 'utf8') === from;
      if (!same) for (const stale of [p, `${p}.linked`]) rmSync(resolve(swiftBinDir, stale), { force: true });
      // The early build was this one's when nothing it linked from has changed.
      if (!(same && linkedEarly && p === sole)) runApple('swift', [...productArgs, '--product', p], { cwd: pkg, env });
      writeFileSync(linked, from);
      const executable = resolve(binDir, p === product ? paths.executable : p);
      copyFileSync(resolve(swiftBinDir, p), executable);
      assertAppleIdentity(app, executable, bakedCompat.id, shared && resolve(shared.framework, 'ExactCore'));
      assertLinkedSdk(executable, designCompatible(app, ios ? 'ios' : 'macos') ? COMPATIBLE_SDK[ios ? 'ios' : 'macos'] : read('xcrun', ['--sdk', sdkName, '--show-sdk-version']).stdout.trim());
      // ExactNumerals uses SwiftUI only on iOS/tvOS; the Mac has no linked numerals presenter.
      const launchCapabilities = ios ? swiftCapabilities : swiftCapabilities.filter(name => name !== 'ExactNumerals');
      assertCapabilityFrameworks(executable, gallery ? [...launchCapabilities, 'ExactSnapshot'] : launchCapabilities, gallery ? ['SwiftUI', 'WidgetKit'] : []);
      if (ipa) stripped = stripForDistribution(executable, `${ipa.replace(/\.ipa$/, '')}.dSYM`);
    }
  } finally { releaseSwift(); }
  const tSwift = Date.now();
  for (const placed of arms) await placed();
  if (modules.tags.length) {
    // The manifest's roster stands when no route reads the artifact's; with no artifact, it serves nothing.
    let provided = modulesBuilt ? modules.tags : [], roster = null, noSimulator = false;
    if (modulesBuilt && typeof Bun === 'undefined') console.warn('host/apple: the module roster check needs Bun (bun:ffi); skipped');
    const causes = [], logs = [], failed = (did, text, log, lines = text.trim().split('\n')) => {
      writeFileSync(log, text); logs.push(log); const named = lines.filter(l => / error: |^ld: |[Uu]ndefined symbol/.test(l));
      return new Error(`does not ${did}:\n${(named.length ? named : lines.slice(-5)).slice(0, 5).map(l => `    ${l}`).join('\n')}`);
    };
    const ran = (r, log) => { if (r.status !== 0 || r.error) throw failed('run', `${r.stdout ?? ''}${r.stderr ?? ''}${r.error ? `${r.error.message}\n` : ''}exit ${r.status ?? r.signal}`, log); return r.stdout; }; // a minute is a hang: the table reads in milliseconds
    for (const route of routes) {
      if (route.lacking) { causes.push(`${route.via}: ${basename(route.lacking)} has no ${route.via} slice`); continue; }
      const log = resolve(linkRoot, `roster-probe-${route.via.replaceAll(' ', '-')}.log`);
      try {
        const placed = route.placed ?? compileProbe(route), compiled = await placed();
        if (compiled !== true) throw failed('compile', readFileSync(compiled, 'utf8'), log);
        let text = keptRoster(placed.cached); if (text === undefined && !route.variant) try { const { dlopen, read: at, CString } = import.meta.require('bun:ffi'); text = new CString(at.ptr(dlopen(route.built, { exact_native_abi: { args: [], returns: 'ptr' } }).symbols.exact_native_abi(), 8)).toString(); } catch (e) { throw failed('load', e.message, log); }
        else if (text === undefined && route.variant === 'maccatalyst') text = ran(read(route.built, [], { timeout: 60_000 }), log);
        else if (text === undefined) { // the iOS simulator named (--sim, EXACT_SIM; a name that is none is not this route's), else any booted
          const pick = args.includes('--sim') ? args[args.indexOf('--sim') + 1] : process.env.EXACT_SIM, ios = simulators().filter(d => /SimRuntime\.iOS/.test(d.runtime)), named = pick && ios.find(d => d.udid === pick || d.name === pick); let sim = named ? null : ios.find(d => d.state === 'Booted');
          if (!sim) try { sim = simulator(named?.udid ?? null); } catch (e) { noSimulator = /^no iPhone simulator/.test(e.message); throw noSimulator ? new Error('no iPhone simulator on an installed iOS runtime') : e; }
          try { text = ran(read('xcrun', ['simctl', 'spawn', sim.udid, route.built], { timeout: 60_000 }), log); } finally { if (sim.bootedHere) read('xcrun', ['simctl', 'shutdown', sim.udid]); }
        }
        try { roster = JSON.parse(text); } catch (e) { throw failed('load', `${e.message}\n${text}`, log); } keepRoster(placed.cached, text);
        if (route !== routes[0]) console.log(`host/apple: the module roster was read on ${route.via}`);
        break;
      } catch (e) { causes.push(`${route.via}: ${e.message}`); }
    }
    if (roster) {
      provided = Object.keys(roster).filter(key => key.includes('-')); // its tags: beside them, `hatches`, `extensions` and `facts`, which no tag can be
      // LLP 1024 D3's first-frame tags, which the host reads before it loads the module.
      const firstFrame = provided.filter(tag => roster[tag].firstFrame === true);
      if (firstFrame.length && !tv) rosterKeys = { ExactFirstFrameModules: firstFrame };
    } else if (routes.length) {
      const why = `host/apple: cannot qualify ${app.id}'s ${tv ? 'tvOS' : ios ? 'iOS' : 'macOS'} native-module roster.\n${causes.map(c => `  ${c}`).join('\n')}\n${noSimulator ? 'Install an iOS Simulator runtime (xcodebuild -downloadPlatform iOS), or add an iPhone simulator in Xcode' : 'Give each xcframework a slice for one route, or fix the failure a route names'}, and rebuild.${logs.length ? `\nLogs: ${logs.join(', ')}` : ''}`;
      if (production) throw new Error(`${why}\nA production build does not take the manifest's roster: a missing factory would pass, and no tag would join the first frame.`);
      console.warn(`${why}\nThe manifest's roster stands, and no tag joins the first frame (docs/recipes/native-views.md)`);
    }
    // A roster read that lacks a tag's factory fails here: another route is not tried.
    checkModuleRoster(app, provided, `host/apple ${ios ? 'iOS' : 'macOS'}`, production);
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
  const bin = resolve(binDir, paths.executable);
  const hostPaths = appleArtifacts(app, { destination, composition, trust: cargoEnv.EXACT_UPDATE_TRUST, host: true });
  const publishProducts = () => {
    if (!ios && shared) publishMacFramework(shared.framework, paths.namespace, sha1);
    if (args.includes('--host')) {
      const hostStage = mkdtempSync(resolve(paths.namespace, '.host-'));
      cleanup.push(hostStage);
      cpSync(binDir, hostStage, { recursive: true });
      for (const name of [paths.executable, `${paths.executable}.app`, `${paths.executable}-Info.plist`]) rmSync(resolve(hostStage, name), { recursive: true, force: true });
      for (const name of [hostPaths.product, `${hostPaths.product}.app`]) rmSync(resolve(binDir, name), { recursive: true, force: true });
      placeAppleArtifact(hostStage, hostPaths.products);
    }
    placeAppleArtifact(binDir, paths.products);
  };

  const bundleTools = {runApple,appleArtifacts, macInfoPlist, copyAppleStaticTrees, writeUsageStrings, receipt, shippedReceipt, assertAppleIdentity, placeAppleArtifact, infoPlist, distributionKeys, copyAppleGpuProducts, entitlements, appClip};
  if (!ios) {
    const bundlePath = assembleMac({intents, shared, library, macWidgets, triple: moduleTripleFor(app, ios, device, tv), appleReach, hasWeb, webLoadName, args, app, paths, binDir, libDir, bin, loadName, hasGpu, dylib, moduleDylibs, looseArms, hasSvg, svgLoadName, modulesLoadName, modulesBuilt, svgBuilt, canvasGpuLoadName, canvasGpuBuilt, sha1, linkRoot, swiftc, buildReceipt, development, bakedCompat, rosterKeys, production, distribution, stampSdk, products, composition, sdk, bakedPlan, cleanup}, bundleTools);
    publishProducts();
    release();
    rmSync(webBuildDir, { recursive: true, force: true });
    console.log(`host/apple: ${paths.binary.replace(root + '/', '')} (${timing()}; ${sha1 ? 'signed ' + sha1.slice(0, 8) : 'ad-hoc signed'}); GPU: ${gpuNote}; web arm: ${hasWeb ? webLoadName : 'none'}${modulesBuilt ? `; modules: ${modulesLoadName}` : ''}`);
    // A live source is explicit (--url or EXACT_DEV_PLAN). The shared web
    // output may belong to another app, and TypeScript edits publish complete
    // URL generations rather than rewriting its initial app.plan.
    if (args.includes('--run')) spawnSync(bundlePath ? resolve(bundlePath, 'Contents/MacOS', paths.executable) : paths.binary, [], { stdio: 'inherit', env: { ...env, ...launchEnv, EXACT_ASSETS: paths.capture } });
    return;
  }

  const bundles = assembleIOS({intents, shared, library, triple: iosTripleFor(app, device, tv), app, paths, binDir, bin, device, development, appleReach, tv, bakedCompat, bakedPlan, ipa, linkRoot, swiftc, buildReceipt, rosterKeys, sdkName, production, distribution, stampSdk, libDir, gpuArtifacts, looseArms, modulesBuilt, modulesLoadName, hasSvg, svgBuilt, svgLoadName, svgFilterBuilt, svgFilterLibraryName, canvasGpuBuilt, canvasGpuLoadName, args, unsigned, prof, ph, sha1, composition, destination, target, sdk, hasGpu, dylib, alarm: ios && !tv ? alarmActivity(packages) : null}, bundleTools);
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
  if (args.includes('--no-install')) { if (args.includes('--run')) throw new Error('--no-install takes no --run'); console.log(`host/apple: ${paths.bundle} (${timing()}); built and signed, installed on nothing (--no-install)`); return; }
  const dev = device ? ph : simulator(args.includes('--sim') ? args[args.indexOf('--sim') + 1] : undefined, { tv, app: app.id, dir: app.dir, hint: '--sim <udid|name> (or EXACT_SIM)' });
  // A phone installs the clip with the app that embeds it; a simulator registers it (`simctl listapps`, a launch
  // of its id) only when the nested bundle is installed itself (measured on iOS 26.5: the app's install leaves it out).
  for (const [, kind] of bundles) {
    const host = kind === 'host', clip = kind === 'clip' ? appClip(app) : null;
    const placed = clip ? resolve(paths.bundle, 'AppClips', `${clip.executable}.app`) : host ? hostPaths.bundle : paths.bundle;
    if (device) {
      if (clip) continue;
      if (!ph.reachable) throw new Error(`${ph.name} is not connected; signed bundle retained at ${placed}`);
      run('xcrun', ['devicectl', 'device', 'install', 'app', '--device', ph.udid, placed]);
    } else install(dev, placed, app, host, !!clip, clip?.executable);
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
 *  default, any app's by name. Not one of the six checks (`rules/RULES.md`
 *  caps those at six); run it when the host's own behaviour changes. With
 *  `--ios`, all tests compiled for UIKit through xcodebuild on a
 *  simulator: `--sim`/EXACT_SIM, else one that is not running (another
 *  session may be driving a booted one), shut down again only if booted here.
 *  Swift availability guards decide which shared methods run on iOS. */
async function test(args) {
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
    mkdirSync(paths.namespace, { recursive: true });
    env.EXACT_SVG_METALLIB = svgFilterLibrary(ios ? 'iphonesimulator' : 'macosx', ios ? '17.0' : '14.0', resolve(paths.namespace, svgFilterLibraryName), true);
    if (ios) env.TEST_RUNNER_EXACT_SVG_METALLIB = env.EXACT_SVG_METALLIB;
    // The optional modules tests drive, built for the tests' platform as a build embeds them and named to the runner: the
    // video and WebKit arms and the Canvas 2D GPU module. A test without one is a missing prerequisite (Coverage.swift).
    const armDir = resolve(paths.namespace, 'arms');
    const arm = (name, sources, frameworks, out = resolve(armDir, `lib${name}.dylib`)) => (runApple('xcrun', ['--sdk', ios ? 'iphonesimulator' : 'macosx', 'swiftc', '-parse-as-library', '-emit-library', '-O', '-module-name', name,
      '-module-cache-path', resolve(armDir, 'cache'), ...sources, ...frameworks.flatMap(f => ['-framework', f]), '-target', ios ? iosTriple : `${macTriple}14.0`, '-o', out]), out);
    const named = ios ? 'TEST_RUNNER_' : '';
    env[`${named}EXACT_WIDGET_RELOAD_TEST`] = arm('WidgetReloadTest', [resolve(pkg, 'tests/ExactKitTests/WidgetContainerTests.swift'), '-D', 'EXACT_TEST_WIDGET_RELOAD'], ['Foundation']);
    env[`${named}EXACT_VIDEO_DYLIB`] = arm('ExactVideoArm', ['VideoArm.swift', 'NowPlaying.swift'].map(f => resolve(root, 'host/apple/videoarm', f)), ['AVKit', 'MediaPlayer']);
    if (!ios) env.EXACT_MIDI_DYLIB = arm('ExactMIDIArm', [resolve(root, 'host/apple/midiarm/MIDIArm.swift'), '-import-objc-header', resolve(root, 'host/apple/midiarm/midi_ring.h')], ['CoreMIDI']); // MIDITests' loopback
    env[`${named}EXACT_TEST_WEB_MODULE`] = arm('ExactWebArm', ['WebArm.swift', 'FrameBrowser.swift', 'FrameWindows.swift', 'FrameCapture.swift', 'FramePDF.swift', 'FrameAutomation.swift', 'FrameFiles.swift'].map(f => resolve(root, 'host/apple/webarm', f)).concat(['-D', 'EXACT_DEVELOPMENT']), ['WebKit', 'PDFKit']);
    // The GPU module goes to its own tests alone: loaded, every canvas draws on the GPU (xctest.mjs).
    const modules = resolve(process.env.CARGO_TARGET_DIR ?? resolve(root, 'target'), 'apple-modules');
    run('cargo', ['build', '--profile', HOST_DEV, '-p', 'exact-canvas-vello', '--lib', ...(ios ? ['--target', iosTarget] : []), '--target-dir', modules, '--manifest-path', resolve(root, 'Cargo.toml')],
      { env: { ...cargoEnv, [`CARGO_PROFILE_${HOST_DEV.toUpperCase().replace(/-/g, '_')}_STRIP`]: 'false' } });
    const canvasGpu = resolve(modules, ...(ios ? [iosTarget] : []), HOST_DEV, 'libexact_canvas_vello.dylib');
    run('cargo', ['build', '-q', '-p', 'contract', '--bin', 'contract', '--manifest-path', resolve(root, 'Cargo.toml')]);
    env.EXACT_CONTRACT = resolve(process.env.CARGO_TARGET_DIR || resolve(root, 'target'), 'debug', 'contract');
    if (!ios) {
      compileFixtures(root, run, env, paths.namespace, 'EXACT', app.id);
      const swiftEnv = { ...env, MACOSX_DEPLOYMENT_TARGET: '14.0' }, scratch = resolve(paths.namespace, 'tests');
      runApple('swift', ['build', '--build-tests', '--scratch-path', scratch], { cwd: pkg, stdio: 'inherit', env: swiftEnv });
      return await macTests(pkg, scratch, swiftEnv, canvasGpu);
    }
    // The fixture's plan and hatch module (LLP 1075.003 §3.9), built for the
    // tests' simulator so they run its hatches over its routes: the glue, its
    // typed keys, its Swift.
    const fixture = resolveApp('native-fixture'), fixtureDir = resolve(paths.namespace, 'fixture-module');
    writeDataKeys(fixture, resolve(fixtureDir, 'ExactDataKeys.swift'), 'ios');
    // Its Contract under the identity of the app the tests link, which the
    // runner requires of a plan (an app's plan boots in no other app).
    const source = resolve(fixtureDir, 'app');
    mkdirSync(source, { recursive: true });
    cpSync(resolve(fixture.dir, 'app.contract'), resolve(source, 'app.contract'));
    writeFileSync(resolve(source, 'app.json'), JSON.stringify({ ...fixture.manifest, $schema: undefined, id: app.id, app: { ...fixture.manifest.app, id: app.id } }));
    env.TEST_RUNNER_EXACT_FIXTURE_PLAN = resolve(fixtureDir, 'app.plan');
    run(env.EXACT_CONTRACT, ['build', resolve(source, 'app.contract'), '-o', env.TEST_RUNNER_EXACT_FIXTURE_PLAN]);
    env.TEST_RUNNER_EXACT_FIXTURE_MODULE = resolve(fixtureDir, 'libexact_modules.dylib');
    // The module composes no packages' extensions (LLP 1133.001 D5): their table, empty.
    writeFileSync(resolve(fixtureDir, 'ExactComposed.swift'), composedModule([], fixture.modules.apple.length > 0));
    runApple('xcrun', ['--sdk', 'iphonesimulator', 'swiftc', '-parse-as-library', '-emit-library', '-O', '-swift-version', '5', '-module-name', 'ExactAppModules',
      '-module-cache-path', resolve(fixtureDir, 'cache'), ...moduleSeam(resolve, root), resolve(fixtureDir, 'ExactComposed.swift'), resolve(fixtureDir, 'ExactDataKeys.swift'),
      ...fixture.modules.apple, '-target', iosTriple, '-o', env.TEST_RUNNER_EXACT_FIXTURE_MODULE]);
    compileFixtures(root, run, env, fixtureDir, 'TEST_RUNNER_EXACT', app.id);
    if (process.env.EXACT_REQUIRED) env.TEST_RUNNER_EXACT_REQUIRED = process.env.EXACT_REQUIRED;
    const pick = args.includes('--sim') ? args[args.indexOf('--sim') + 1] : process.env.EXACT_SIM;
    const before = simulators();
    const idle = before.filter(d => /SimRuntime\.iOS/.test(d.runtime) && /^iPhone \d+ Pro$/.test(d.name) && d.state !== 'Booted');
    const newest = (d) => Number(/iOS-(\d+)-(\d+)/.exec(d.runtime)?.slice(1).join('.') ?? 0);
    const dev = simulator(pick ?? idle.sort((a, b) => newest(b) - newest(a))[0]?.udid, { hint: '--sim <udid|name> (or EXACT_SIM)' });
    const bootedHere = before.find(d => d.udid === dev.udid)?.state !== 'Booted';
    try {
      const destination = `platform=iOS Simulator,id=${dev.udid}`, derived = resolve(paths.namespace, 'ios-tests'), xcodeEnv = { ...env, IPHONEOS_DEPLOYMENT_TARGET: '17.0' };
      runApple('xcodebuild', ['build-for-testing', '-scheme', 'Exact-Package', '-destination', destination, '-derivedDataPath', derived], { cwd: pkg, stdio: 'inherit', env: xcodeEnv });
      iosTestRuns({ pkg, products: resolve(derived, 'Build/Products'), destination, udid: dev.udid, env: xcodeEnv, canvasGpu, hostSource: resolve(root, 'host/apple/testhost/TestHost.swift'), triple: iosTriple, runApple, run });
    } finally { if (bootedHere) read('xcrun', ['simctl', 'shutdown', dev.udid]); }
  } finally { cargoRelease?.(); release(); }
}
if (process.argv[1] && resolve(process.argv[1]) === new URL(import.meta.url).pathname) {
  const args = guardFlags(['--ios', '--device', '--tvos', '--watchos'].some((flag) => process.argv.includes(flag)) ? 'ios' : 'mac', process.argv.slice(2));
  if (args.includes('--phone') && !args.includes('--device')) throw new Error('--phone needs --device; for a simulator use --sim <udid|name> (or EXACT_SIM)');
  useXcode(); if (args.includes('--release')) process.env.EXACT_UPDATE_TRUST = 'production'; // the build that ships: Cargo's release profile, no development trust
  (args.includes('--test') ? test(args) : main(args)).catch((error) => { console.error(error.rebuild ? `${error.message}: ${rebuildArgs(args)}` : error.message); process.exitCode = 1; });
}

/** This build's command, to run it again: an `exact new` app's `exact.mjs` verb (EXACT_APP_DIR, which that
 * sets), else this script's. */
export function rebuildArgs(args, dir = process.env.EXACT_APP_DIR) {
  const quote = (a) => /^[\w@%+=:,./-]+$/.test(a) ? a : `'${a.replaceAll("'", "'\\''")}'`;
  if (!dir) return `bun host/apple/build.mjs ${args.map(quote).join(' ')}`;
  const ios = ['--ios', '--device', '--tvos'].some((flag) => args.includes(flag));
  const rest = args.filter((a) => a !== '--ios' && !(a.endsWith('-apple') && !a.startsWith('-')));
  return [`(cd ${quote(dir)} && bun exact.mjs ${ios ? 'ios' : 'mac'}`, ...rest.map(quote)].join(' ') + ')';
}
