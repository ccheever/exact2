// Apple bundle assembly. The build owns compilation and artifact publication.
// @ref LLP 1126 D2/D8 — an extension is packaged inside its app's cohort.
import { packageWidgets, widgetId } from './widgets.mjs';
import { spawnSync } from 'node:child_process';
import { resolve, basename, dirname } from 'node:path';
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { verifyBakeFiles } from '../../scripts/app.mjs';
import { listAssets } from '../web/serve.mjs';
import { appIcon, buildInfo, iosAssets, appleAssets, copyMacResources, signingOrder, helperEntitlements, wrapFramework, assertArchiveFrameworks } from './assets.mjs';
import { allows, profile, identity } from './devices.mjs';
const root = resolve(new URL('../..', import.meta.url).pathname);
const run = (cmd, args, opts = {}) => {
  const r = spawnSync(cmd, args, { cwd: root, ...opts, stdio: opts.stdio === 'ignore' ? ['ignore', 'ignore', 'pipe'] : opts.stdio ?? 'inherit' });
  if (r.status !== 0) throw new Error(`${cmd} failed (${r.status ?? r.error?.message})${r.stderr?.length ? ': ' + String(r.stderr).trim() : ''}`);
  return r;
};
export function assembleMac({args, app, paths, binDir, libDir, bin, loadName, hasGpu, dylib, moduleDylibs, looseArms, hasSvg, svgLoadName, modulesLoadName, modulesBuilt, svgBuilt, canvasGpuLoadName, canvasGpuBuilt, sha1, linkRoot, swiftc, buildReceipt, development, bakedCompat, rosterKeys, production, distribution, stampSdk, products, composition, sdk, bakedPlan, cleanup}, {appleArtifacts, macInfoPlist, copyAppleStaticTrees, writeUsageStrings, receipt, shippedReceipt, assertAppleIdentity, placeAppleArtifact, infoPlist, distributionKeys, copyAppleGpuProducts, entitlements}) {
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
    const loaded = [...looseArms.filter(([has]) => has).map(([, , name]) => name), ...(hasSvg ? [svgLoadName] : [])];
    for (const [has, built, name] of looseArms) { rmSync(resolve(binDir, name), { force: true }); if (has) copyFileSync(built, resolve(binDir, name)); }
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
    const assetKeys = appleAssets(app, binDir, { platform: 'macos', kept: { dir: resolve(linkRoot, 'assets'), stamp: swiftc }, expected: buildReceipt.binary.metadata.appleAssetCatalogs ?? [] });
    writeFileSync(resolve(binDir, `${paths.executable}-Info.plist`), macInfoPlist(app, { development, reach: bakedCompat.reach, icon: { ...rosterKeys, ...assetKeys, ...buildInfo(app, { root, production: production || distribution, ...stampSdk }) } }));
    if (hasWeb) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, webLoadName)], { stdio: 'ignore' });
    if (modulesBuilt) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, modulesLoadName)], { stdio: 'ignore' });
    if (hasSvg) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, svgLoadName)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', app.id, bin], { stdio: 'ignore' });
    for (const p of products.slice(1)) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', `${app.id}.${p.toLowerCase()}`, resolve(binDir, p)], { stdio: 'ignore' });
    // The receipt beside it (LLP 1030 D2).
    writeFileSync(resolve(binDir, 'receipt.json'), receipt(app, { compatibilityId:bakedCompat.id, build:buildReceipt, composition, platform: 'macos', target: process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin', sdk, identity: sha1 ?? 'ad-hoc', profile: null, entitlements: null, gpu: hasGpu ? dylib : null, development }));
    // A local .app gives Launch Services an owner for development links, and is what `--run` launches (a bare executable has no Info.plist).
    if (args.includes('--bundle') || args.includes('--run') || buildReceipt.binary.metadata.appleAssetCatalogs?.length) {
      const bundleDestination = appleArtifacts(app).bundle;
      const output = resolve(bundleDestination, '..');
      mkdirSync(output, { recursive: true });
      // Assemble beside the stable app path, then replace atomically (LLP 1033 D2).
      const stage = mkdtempSync(resolve(output, '.build-'));
      cleanup.push(stage);
      const bundle = resolve(stage, basename(bundleDestination)), contents = resolve(bundle, 'Contents');
      const executables = resolve(contents, 'MacOS'), resources = resolve(contents, 'Resources');
      mkdirSync(executables, { recursive: true });
      mkdirSync(resources);
      for (const file of [paths.executable, ...loaded, ...(canvasGpuBuilt ? [canvasGpuLoadName] : []), ...(modulesBuilt ? [modulesLoadName] : []), ...(hasGpu ? [loadName] : []), ...moduleDylibs.map(m => m.load)]) copyFileSync(resolve(binDir, file), resolve(executables, file));
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development, reach: bakedCompat.reach, icon: { ...rosterKeys, ...assetKeys, ...buildInfo(app, { root, production: production || distribution, ...stampSdk }) } }));
      copyAppleStaticTrees(paths.capture, resources);
      for (const file of ['Assets.car', ...(assetKeys.CFBundleIconFile ? [`${assetKeys.CFBundleIconFile}.icns`] : [])]) if (existsSync(resolve(binDir, file))) copyFileSync(resolve(binDir, file), resolve(resources, file));
      verifyBakeFiles(bakedCompat, bakedPlan, listAssets(resources, true));
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development, reach: bakedCompat.reach, icon: { ...rosterKeys, ...appIcon(app, resources, 'macos'), ...assetKeys, ...buildInfo(app, { root, production: production || distribution, ...stampSdk }) } }));
      writeUsageStrings(bakedCompat.reach, resources);
      const whole = readFileSync(resolve(binDir, 'receipt.json'), 'utf8');
      writeFileSync(resolve(resources, 'receipt.json'), distribution ? shippedReceipt(whole) : whole);
      // Preserve the GPU artifact signatures that the baked receipt names.
      for (const file of [...loaded, ...(canvasGpuBuilt ? [canvasGpuLoadName] : []), ...(modulesBuilt ? [modulesLoadName] : [])]) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(executables, file)], { stdio: 'ignore' });
      copyMacResources(app, contents, buildReceipt.binary.metadata.nativeResources ?? []);
      const nativeRoots = (app.manifest.host?.macos?.resources ?? []).map(({to}) => resolve(contents, to)), order = signingOrder(bundle, bakedCompat);
      // A runtime exception's target is checked here, not first at release (LLP 1069.008 D9); a development build signs without the hardened runtime, so with no entitlements.
      helperEntitlements(bundle, app.manifest, bakedCompat, order);
      for (const file of order.filter(file => nativeRoots.some(root => file === root || file.startsWith(root + '/')))) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', file], { stdio: 'ignore' });
      run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', bundle], { stdio: 'ignore' });
      const placed = bundleDestination;
      assertAppleIdentity(app, resolve(executables, paths.executable), bakedCompat.id);
      placeAppleArtifact(bundle, placed);
      rmSync(stage, { recursive: true, force: true });
      bundlePath = placed;
      console.log(`local client: ${placed}\n  ${development ? `Open this app once to register its opening link for ${development.origins[0]}.` : 'No development opening link: build with --url <dev server URL> to register one.'}`);
    }
    return bundlePath;
}
export function assembleIOS({shared, triple, app, paths, binDir, bin, device, development, appleReach, tv, bakedCompat, bakedPlan, ipa, linkRoot, swiftc, buildReceipt, rosterKeys, sdkName, production, distribution, stampSdk, libDir, gpuArtifacts, looseArms, modulesBuilt, modulesLoadName, hasSvg, svgBuilt, svgLoadName, svgFilterBuilt, svgFilterLibraryName, canvasGpuBuilt, canvasGpuLoadName, args, unsigned, prof, ph, sha1, composition, destination, target, sdk, hasGpu, dylib}, {appleArtifacts, macInfoPlist, copyAppleStaticTrees, writeUsageStrings, receipt, shippedReceipt, assertAppleIdentity, placeAppleArtifact, infoPlist, distributionKeys, copyAppleGpuProducts, entitlements}) {
  // The bundle, assembled from scratch (new inodes, see above), with the app's assets (a phone reads no other
  // machine's paths); ad-hoc signed for a simulator, signed with the team's identity, profile, and entitlements for a phone.
  const bundle = resolve(binDir, basename(paths.bundle));
  mkdirSync(resolve(bundle, 'Frameworks'), { recursive: true });
  if (shared) cpSync(shared.framework, resolve(bundle, 'Frameworks/ExactCore.framework'), { recursive: true });
  copyFileSync(bin, resolve(bundle, paths.executable));
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, { development, reach: appleReach, tv }));
  // The GPU crate's shaders (LLP 1030 D8): files the presenter registers
  // with the module before a surface is created, never strings in the dylib.
  copyAppleStaticTrees(paths.capture, bundle);
  verifyBakeFiles(bakedCompat, bakedPlan, listAssets(bundle, true));
  const assetKeys = iosAssets(app, bundle, device, { tv, catalog: !!ipa, kept: { dir: resolve(linkRoot, 'assets'), stamp: swiftc }, expected: buildReceipt.binary.metadata.appleAssetCatalogs ?? [], ...(tv ? {} : { expectedLaunchImage: buildReceipt.binary.metadata.launchImage ?? null }), launch: buildReceipt.graph.launchBackground });
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, { development, reach: appleReach, icon: assetKeys, distribution: { ...rosterKeys, ...(ipa ? distributionKeys(sdkName) : {}), ...buildInfo(app, { root, archive: !!ipa, production: production || distribution, ...stampSdk }) }, tv }));
  writeUsageStrings(appleReach, bundle);
  copyAppleGpuProducts(libDir, resolve(bundle, 'Frameworks'), gpuArtifacts, !!ipa);
  for (const [has, built, name] of looseArms) if (has) copyFileSync(built, resolve(bundle, 'Frameworks', name));
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
    writeFileSync(resolve(hostBundle, 'Info.plist'), infoPlist(app, device, { executable: 'ExactHostIOS', id: `${app.id}.host`, name: 'Host (not Exact)', reach: appleReach, icon: { ...rosterKeys, ...(assetKeys.NSAccentColorName ? { NSAccentColorName: assetKeys.NSAccentColorName } : {}) }, distribution: buildInfo(app, { root, production: production || distribution, ...stampSdk }) }));
    writeUsageStrings(appleReach, hostBundle);
    copyAppleStaticTrees(paths.capture, hostBundle);
    if (existsSync(resolve(bundle, 'Assets.car'))) copyFileSync(resolve(bundle, 'Assets.car'), resolve(hostBundle, 'Assets.car'));
    for (const f of readdirSync(resolve(bundle, 'Frameworks'))) cpSync(resolve(bundle, 'Frameworks', f), resolve(hostBundle, 'Frameworks', f), { recursive: true });
    if (svgFilterBuilt) copyFileSync(svgFilterBuilt, resolve(hostBundle, svgFilterLibraryName));
    bundles.push([hostBundle, true]);
  }
  for (const [assembled, host] of bundles) {
    const id = host ? `${app.id}.host` : app.id;
    // A grant's signing entitlement needs a profile that allows it, which a
    // team wildcard never does for HealthKit (LLP 1069.008.000 D4).
    const required = appleReach?.entitlements ?? [];
    const signingProfile = device && !unsigned ? (host || !allows(prof, required) ? profile(ph?.udid, id, required) : prof) : null;
    const signingIdentity = signingProfile ? identity(signingProfile.team) : sha1;
    const ent = resolve(binDir, host ? 'host-entitlements.plist' : 'entitlements.plist');
    if (signingProfile) copyFileSync(signingProfile.path, resolve(assembled, 'embedded.mobileprovision'));
    // Unsigned, the re-signer's profile decides; ask for no debugger, as a distribution profile grants none.
    writeFileSync(ent, entitlements({ ...app, id }, signingProfile?.team, signingProfile?.dev ?? !unsigned, appleReach, { prefix: signingProfile?.prefix }));
    verifyBakeFiles(bakedCompat, bakedPlan, listAssets(assembled, true));
    assertAppleIdentity(app, resolve(assembled, host ? 'ExactHostIOS' : paths.executable), bakedCompat.id);
    const whole = receipt(app, { compatibilityId: bakedCompat.id, build: buildReceipt, composition,
      platform: destination, target, sdk, identity: signingIdentity,
      profile: signingProfile ? { name: signingProfile.name, team: signingProfile.team, expires: signingProfile.expires } : null,
      entitlements: readFileSync(ent, 'utf8'), gpu: hasGpu ? dylib : null, development: host ? null : development });
    writeFileSync(resolve(assembled, 'receipt.json'), ipa ? shippedReceipt(whole) : whole);
    if (ipa) { mkdirSync(dirname(ipa), { recursive: true }); writeFileSync(`${ipa.replace(/\.ipa$/, '')}.receipt.json`, whole); }
    if (ipa) for (const [loose, name] of [...looseArms.map(([, , load, framework]) => [load, framework]), [modulesLoadName, 'ExactModules'], [svgLoadName, 'ExactSvg'], [canvasGpuLoadName, 'ExactCanvasGpu']]) wrapFramework(resolve(assembled, 'Frameworks'), loose, name, app, tv ? 'AppleTVOS' : 'iPhoneOS');
    if (shared && !host) packageWidgets({ app, bundle: assembled, shared, directory: linkRoot, sdk, triple, device,
      minimumOS: app.manifest.host?.ios?.minimumOS ?? '17.0',
      entitlements: entitlements({ ...app, id: widgetId(app), widgetAppId: app.id }, signingProfile?.team, signingProfile?.dev ?? !unsigned, null, { simulator: !device, prefix: signingProfile?.prefix }),
      sign: (extension, ent) => run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', ...(device ? ['--entitlements', ent] : []), extension], { stdio: 'ignore' }) });
    for (const f of readdirSync(resolve(assembled, 'Frameworks')).filter(f => !gpuArtifacts.some(m => f === (ipa ? `${m.framework}.framework` : m.load)))) run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', resolve(assembled, 'Frameworks', f)], { stdio: 'ignore' });
    if (ipa) assertArchiveFrameworks(resolve(assembled, 'Frameworks'));
    run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', ...(device ? ['--entitlements', ent] : []), assembled], { stdio: 'ignore' });
  }
  return bundles;
}
