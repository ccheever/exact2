// Apple bundle assembly. The build owns compilation and artifact publication.
// @ref LLP 1126 D2/D8 — an extension is packaged inside its app's cohort.
import { packageActivityArm, assertActivityRefusals } from './activities.mjs';
import { packageWidgets, packageAlarmExtension, widgetId, entryProfiles, activityEntries, platformWidgetEntries, macWidgetGroup, macWidgetEntitlements, widgetGroup, widgetSigningTeam } from './widgets.mjs';
import { packageShareExtension, shareInfo, shareId, macShareEntitlements } from './extensions.mjs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { resolve, basename, dirname } from 'node:path';
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { verifyBakeFiles } from '../../scripts/app.mjs';
import { webRefusal, browserEntitlements } from './documents.mjs';
import { listAssets } from '../web/serve.mjs';
import { appIcon, buildInfo, iosAssets, appleAssets, copyMacResources, signingOrder, helperEntitlements, wrapFramework, assertArchiveFrameworks } from './assets.mjs';
import { allows, profile, identity, embeddedProfile } from './devices.mjs';
// The App Intents module the bundle places, built before the executable links it (LLP 1133.023 D8).
export { buildIntents, shortcutKeys } from './intents.mjs';
const root = resolve(new URL('../..', import.meta.url).pathname);
const run = (cmd, args, opts = {}) => {
  const r = spawnSync(cmd, args, { cwd: root, ...opts, stdio: opts.stdio === 'ignore' ? ['ignore', 'ignore', 'pipe'] : opts.stdio ?? 'inherit' });
  if (r.status !== 0) throw new Error(`${cmd} failed (${r.status ?? r.error?.message})${r.stderr?.length ? ': ' + String(r.stderr).trim() : ''}`);
  return r;
};
export function assembleMac({intents, shared, macWidgets, triple, appleReach, hasWeb, webLoadName, args, app, paths, binDir, libDir, bin, loadName, hasGpu, dylib, moduleDylibs, looseArms, hasSvg, svgLoadName, modulesLoadName, modulesBuilt, svgBuilt, canvasGpuLoadName, canvasGpuBuilt, sha1, linkRoot, swiftc, buildReceipt, development, bakedCompat, rosterKeys, production, distribution, stampSdk, products, composition, sdk, bakedPlan, cleanup}, {runApple, appleArtifacts, macInfoPlist, copyAppleStaticTrees, writeUsageStrings, receipt, shippedReceipt, assertAppleIdentity, placeAppleArtifact, infoPlist, distributionKeys, copyAppleGpuProducts, entitlements}) {
    // `--bundle`'s assembled `.app`, when one was asked for: what `--run`
    // then launches, so the running process has the app's bundle identity —
    // its Info.plist, its document types, its Dock tile (LLP 1033 D2).
    let bundlePath = null;
    const { team: widgetTeam = null, profiles = null } = macWidgets ?? {};
    // The share extension's group is the team's, as the widgets' is (LLP 1133.024 D4): ad hoc, the app has no inbox.
    const team = widgetTeam ?? (app.manifest.share_target && sha1 && sha1 !== '-' ? (() => { try { return widgetSigningTeam(sha1); } catch (e) { console.log(`Mac share extension: ${e.message}`); return null; } })() : null);
    if (app.manifest.share_target && !team) console.log('Mac share extension: no Apple Development identity; building without it (EXACT_IDENTITY names one, LLP 1133.024 D4).');
    const shareGroup = app.manifest.share_target && team ? macWidgetGroup(app, team) : null;
    const widgetKeys = { ...shareInfo(app, shareGroup), ...(shared ? {
      ExactAppGroup: macWidgetGroup(app, team), ExactAppIdentifier: app.id,
      ExactWidgetKinds: platformWidgetEntries(app, 'macos').map(entry => entry.kind),
      ExactWidgetTypeScript: existsSync(resolve(app.dir, 'app.ts')),
      ExactWidgetRequiresSecrets: platformWidgetEntries(app, 'macos').filter(entry => entry.requiresSecrets).map(entry => entry.kind),
      ExactWidgetGeneration: createHash('sha256').update(JSON.stringify([bakedCompat.id, buildReceipt.graph.artifacts])).digest('hex'),
      ...(profiles ? { ExactKeychainAccessGroup: profiles.keychain } : {}),
    } : {}) };
    if (shared && !profiles) console.log('Mac widgets: no shared Keychain profiles; entries with requiresSecrets show the placeholder (LLP 1126.001 §5 Q5).');
    const ent = shared || shareGroup ? resolve(linkRoot, 'mac-entitlements.plist') : null;
    if (ent) writeFileSync(ent, macWidgetEntitlements({ app, team, profiles }));
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
    writeFileSync(resolve(binDir, `${paths.executable}-Info.plist`), macInfoPlist(app, { development, distributed: distribution, reach: appleReach, icon: { ...widgetKeys, ...rosterKeys, ...assetKeys, ...buildInfo(app, { root, production: production || distribution, ...stampSdk }) } }));
    if (hasWeb) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, webLoadName)], { stdio: 'ignore' });
    if (modulesBuilt) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, modulesLoadName)], { stdio: 'ignore' });
    if (hasSvg) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(binDir, svgLoadName)], { stdio: 'ignore' });
    run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', app.id, bin], { stdio: 'ignore' });
    for (const p of products.slice(1)) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', '--identifier', `${app.id}.${p.toLowerCase()}`, resolve(binDir, p)], { stdio: 'ignore' });
    // The receipt beside it (LLP 1030 D2).
    writeFileSync(resolve(binDir, 'receipt.json'), receipt(app, { compatibilityId:bakedCompat.id, build:buildReceipt, composition, platform: 'macos', target: process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin', sdk, identity: sha1 ?? 'ad-hoc', profile: profiles ? { name: profiles.app.name, team, expires: profiles.app.expires } : null, entitlements: ent ? readFileSync(ent, 'utf8') : null, gpu: hasGpu ? dylib : null, development }));
    // A local .app gives Launch Services an owner for development links, and is what `--run` launches (a bare executable has no Info.plist).
    if (shared || args.includes('--bundle') || args.includes('--run') || buildReceipt.binary.metadata.appleAssetCatalogs?.length) {
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
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development, distributed: distribution, reach: appleReach, icon: { ...widgetKeys, ...rosterKeys, ...assetKeys, ...buildInfo(app, { root, production: production || distribution, ...stampSdk }) } }));
      copyAppleStaticTrees(paths.capture, resources);
      for (const file of ['Assets.car', ...(assetKeys.CFBundleIconFile ? [`${assetKeys.CFBundleIconFile}.icns`] : [])]) if (existsSync(resolve(binDir, file))) copyFileSync(resolve(binDir, file), resolve(resources, file));
      verifyBakeFiles(bakedCompat, bakedPlan, listAssets(resources, true, ['typescript']));
      writeFileSync(resolve(contents, 'Info.plist'), macInfoPlist(app, { development, distributed: distribution, reach: appleReach, icon: { ...widgetKeys, ...rosterKeys, ...appIcon(app, resources, 'macos'), ...assetKeys, ...buildInfo(app, { root, production: production || distribution, ...stampSdk }) } }));
      writeUsageStrings(appleReach, resources);
      intents?.place(resources); // LLP 1133.023 D8: the faces' App Intents metadata
      const whole = readFileSync(resolve(binDir, 'receipt.json'), 'utf8');
      writeFileSync(resolve(resources, 'receipt.json'), distribution ? shippedReceipt(whole) : whole);
      // Preserve the GPU artifact signatures that the baked receipt names.
      for (const file of [...loaded, ...(canvasGpuBuilt ? [canvasGpuLoadName] : []), ...(modulesBuilt ? [modulesLoadName] : [])]) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', resolve(executables, file)], { stdio: 'ignore' });
      copyMacResources(app, contents, buildReceipt.binary.metadata.nativeResources ?? []);
      if (shared) {
        const frameworks = resolve(contents, 'Frameworks');
        mkdirSync(frameworks, { recursive: true });
        cpSync(shared.framework, resolve(frameworks, 'ExactCore.framework'), { recursive: true, verbatimSymlinks: true });
        if (profiles) copyFileSync(profiles.app.path, resolve(contents, 'embedded.provisionprofile'));
        packageWidgets({ app, bundle, shared, directory: linkRoot, sdk, triple, platform: 'macos',
          runApple, production: production || distribution, profile: profiles?.extension,
          entitlements: macWidgetEntitlements({ app, team, profiles, extension: true }),
          sign: (extension, path) => run('codesign', ['--force', '--sign', sha1, '--timestamp=none', '--entitlements', path, extension], { stdio: 'ignore' }) });
        for (const name of readdirSync(frameworks)) run('codesign', ['--force', '--sign', sha1, '--timestamp=none', resolve(frameworks, name)], { stdio: 'ignore' });
      }
      if (shareGroup) packageShareExtension({ app, bundle, directory: linkRoot, sdk, triple, platform: 'macos', group: shareGroup, runApple, entitlements: macShareEntitlements(shareGroup),
        sign: (extension, path) => run('codesign', ['--force', '--sign', sha1, '--timestamp=none', '--entitlements', path, extension], { stdio: 'ignore' }) });
      const nativeRoots = (app.manifest.host?.macos?.resources ?? []).map(({to}) => resolve(contents, to)), order = signingOrder(bundle, bakedCompat);
      // A runtime exception's target is checked here, not first at release (LLP 1069.008 D9); a development build signs without the hardened runtime, so with no entitlements.
      helperEntitlements(bundle, app.manifest, bakedCompat, order);
      for (const file of order.filter(file => nativeRoots.some(root => file === root || file.startsWith(root + '/')))) run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', file], { stdio: 'ignore' });
      run('codesign', ['--force', '--sign', sha1 ?? '-', '--timestamp=none', ...(ent ? ['--entitlements', ent] : []), bundle], { stdio: 'ignore' });
      const placed = bundleDestination;
      assertAppleIdentity(app, resolve(executables, paths.executable), bakedCompat.id, shared && resolve(contents, 'Frameworks/ExactCore.framework/ExactCore'));
      placeAppleArtifact(bundle, placed);
      rmSync(stage, { recursive: true, force: true });
      bundlePath = placed;
      console.log(`local client: ${placed}\n  ${development ? `Open this app once to register its opening link for ${development.origins[0]}.` : 'No development opening link: build with --url <dev server URL> to register one.'}`);
    }
    return bundlePath;
}
export function assembleIOS({intents, shared, triple, app, paths, binDir, bin, device, development, appleReach, tv, bakedCompat, bakedPlan, ipa, linkRoot, swiftc, buildReceipt, rosterKeys, sdkName, production, distribution, stampSdk, libDir, gpuArtifacts, looseArms, modulesBuilt, modulesLoadName, hasSvg, svgBuilt, svgLoadName, svgFilterBuilt, svgFilterLibraryName, canvasGpuBuilt, canvasGpuLoadName, args, unsigned, prof, ph, sha1, composition, destination, target, sdk, hasGpu, dylib, alarm = null}, {runApple, appleArtifacts, macInfoPlist, copyAppleStaticTrees, writeUsageStrings, receipt, shippedReceipt, assertAppleIdentity, placeAppleArtifact, infoPlist, distributionKeys, copyAppleGpuProducts, entitlements, appClip}) {
  // The bundle, assembled from scratch (new inodes, see above), with the app's assets (a phone reads no other
  // machine's paths); ad-hoc signed for a simulator, signed with the team's identity, profile, and entitlements for a phone.
  const bundle = resolve(binDir, basename(paths.bundle));
  mkdirSync(resolve(bundle, 'Frameworks'), { recursive: true });
  if (shared) cpSync(shared.framework, resolve(bundle, 'Frameworks/ExactCore.framework'), { recursive: true });
  copyFileSync(bin, resolve(bundle, paths.executable));
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, { development, distributed: distribution, reach: appleReach, tv }));
  const refused = webRefusal(app, distribution, tv);
  if (refused) console.log(refused);
  // The GPU crate's shaders (LLP 1030 D8): files the presenter registers
  // with the module before a surface is created, never strings in the dylib.
  copyAppleStaticTrees(paths.capture, bundle);
  verifyBakeFiles(bakedCompat, bakedPlan, listAssets(bundle, true, ['typescript']));
  const assetKeys = iosAssets(app, bundle, device, { tv, catalog: !!ipa, kept: { dir: resolve(linkRoot, 'assets'), stamp: swiftc }, expected: buildReceipt.binary.metadata.appleAssetCatalogs ?? [], ...(tv ? {} : { expectedLaunchImage: buildReceipt.binary.metadata.launchImage ?? null }), launch: buildReceipt.graph.launchBackground });
  const stamp = { ...rosterKeys, ...(ipa ? distributionKeys(sdkName) : {}), ...buildInfo(app, { root, archive: !!ipa, production: production || distribution, ...stampSdk }) };
  writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, { development, distributed: distribution, reach: appleReach, icon: assetKeys, distribution: stamp, tv }));
  if (shared) run('plutil', ['-insert', 'ExactWidgetGeneration', '-string', createHash('sha256').update(JSON.stringify([bakedCompat.id, buildReceipt.graph.artifacts])).digest('hex'), resolve(bundle, 'Info.plist')]);
  writeUsageStrings(appleReach, bundle);
  // AlarmKit's countdown is a Live Activity the extension draws (LLP 1133.030 D8 item 2).
  if (alarm && !tv) run('plutil', ['-replace', 'NSSupportsLiveActivities', '-bool', 'YES', resolve(bundle, 'Info.plist')]);
  intents?.place(bundle); // LLP 1133.023 D8: the faces' App Intents metadata
  copyAppleGpuProducts(libDir, resolve(bundle, 'Frameworks'), gpuArtifacts, !!ipa);
  for (const [has, built, name] of looseArms) if (has) copyFileSync(built, resolve(bundle, 'Frameworks', name));
  if (modulesBuilt) copyFileSync(modulesBuilt, resolve(bundle, 'Frameworks', modulesLoadName));
  if (hasSvg) copyFileSync(svgBuilt, resolve(bundle, 'Frameworks', svgLoadName));
  if (svgFilterBuilt) copyFileSync(svgFilterBuilt, resolve(bundle, svgFilterLibraryName));
  if (canvasGpuBuilt) copyFileSync(canvasGpuBuilt, resolve(bundle, 'Frameworks', canvasGpuLoadName));
  // A sibling bundle runs the app's own executable over the app's frameworks, assets and resources from an
  // Info.plist of its own: the sample host (`--host`) and the App Clip (LLP 1126 D12: an App Clip is an app, the
  // whole one, under a second identity). `bundles` is signed in order: the clip before the app that holds it.
  const sibling = (path, executable, plist) => {
    mkdirSync(resolve(path, 'Frameworks'), { recursive: true });
    copyFileSync(executable[0], resolve(path, executable[1]));
    writeFileSync(resolve(path, 'Info.plist'), plist);
    writeUsageStrings(appleReach, path);
    copyAppleStaticTrees(paths.capture, path);
    if (existsSync(resolve(bundle, 'Assets.car'))) copyFileSync(resolve(bundle, 'Assets.car'), resolve(path, 'Assets.car'));
    for (const f of readdirSync(resolve(bundle, 'Frameworks'))) cpSync(resolve(bundle, 'Frameworks', f), resolve(path, 'Frameworks', f), { recursive: true });
    if (svgFilterBuilt) copyFileSync(svgFilterBuilt, resolve(path, svgFilterLibraryName));
  };
  const clip = tv ? null : appClip(app);
  const bundles = [[bundle, 'app']];
  if (clip) {
    const clipBundle = resolve(bundle, 'AppClips', `${clip.executable}.app`);
    sibling(clipBundle, [bin, clip.executable], infoPlist(app, device, { distributed: distribution, executable: clip.executable, id: clip.id, name: clip.name, clip: true, reach: appleReach, icon: assetKeys, distribution: stamp }));
    bundles.unshift([clipBundle, 'clip']);
  }
  if (args.includes('--host')) {
    const hostBundle = resolve(binDir, 'ExactHostIOS.app');
    // The sample host takes no development link: it would share the scheme.
    sibling(hostBundle, [resolve(binDir, 'ExactHostIOS'), 'ExactHostIOS'], infoPlist(app, device, { distributed: distribution, executable: 'ExactHostIOS', id: `${app.id}.host`, name: 'Host (not Exact)', reach: appleReach, icon: { ...rosterKeys, ...(assetKeys.NSAccentColorName ? { NSAccentColorName: assetKeys.NSAccentColorName } : {}) }, distribution: buildInfo(app, { root, production: production || distribution, ...stampSdk }) }));
    bundles.push([hostBundle, 'host']);
  }
  // A grant's signing entitlement needs a profile that allows it, which a
  // team wildcard never does for HealthKit (LLP 1069.008.000 D4).
  const required = appleReach?.entitlements ?? [], appRequired = [...required, ...browserEntitlements({ ...app, widgetTV: tv }, distribution)];
  // With entries, the app's and the extension's profiles together, each granting the shared groups (LLP 1126.001 WP13).
  const provisioned = shared && device && !unsigned ? entryProfiles({ app, udid: ph?.udid, required: appRequired }) : null;
  const appProfile = device && !unsigned ? provisioned?.app ?? (allows(prof, appRequired) ? prof : profile(ph?.udid, app.id, appRequired)) : null;
  // The clip's profile is for its own App ID, of the app's profile's team, prefix and method, as the extension's is.
  const profiles = { host: () => profile(ph?.udid, `${app.id}.host`, required), clip: () => embeddedProfile(ph?.udid, clip.id, appProfile, [...required, 'com.apple.developer.parent-application-identifiers']) };
  for (const [assembled, kind] of bundles) {
    const id = kind === 'host' ? `${app.id}.host` : kind === 'clip' ? clip.id : app.id, executable = kind === 'host' ? 'ExactHostIOS' : kind === 'clip' ? clip.executable : paths.executable;
    const signingProfile = appProfile ? (kind === 'app' ? appProfile : profiles[kind]()) : null;
    const signingIdentity = signingProfile ? identity(signingProfile.team) : sha1;
    const ent = resolve(binDir, kind === 'app' ? 'entitlements.plist' : `${kind}-entitlements.plist`);
    if (signingProfile) copyFileSync(signingProfile.path, resolve(assembled, 'embedded.mobileprovision'));
    // Unsigned, the re-signer's profile decides; ask for no debugger, as a distribution profile grants none.
    writeFileSync(ent, entitlements({ ...app, id, widgetTV: tv, widgetHost: kind === 'host', ...(kind === 'clip' ? { clipParent: app.id } : {}) }, signingProfile?.team, signingProfile?.dev ?? !unsigned, appleReach, { simulator: !device, prefix: signingProfile?.prefix, distributed: distribution }));
    if (shared && kind === 'app') {
      const group = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', ent]).stdout)['keychain-access-groups'][0];
      runApple('plutil', ['-insert', 'ExactKeychainAccessGroup', '-string', group, resolve(assembled, 'Info.plist')]);
      runApple('plutil', ['-insert', 'ExactWidgetTypeScript', '-bool', String(existsSync(resolve(app.dir, 'app.ts'))), resolve(assembled, 'Info.plist')]);
    }
    verifyBakeFiles(bakedCompat, bakedPlan, listAssets(assembled, true, ['typescript']));
    assertAppleIdentity(app, resolve(assembled, executable), bakedCompat.id);
    const whole = receipt(app, { compatibilityId: bakedCompat.id, build: buildReceipt, composition,
      platform: destination, target, sdk, identity: signingIdentity,
      profile: signingProfile ? { name: signingProfile.name, team: signingProfile.team, expires: signingProfile.expires } : null,
      entitlements: readFileSync(ent, 'utf8'), gpu: hasGpu ? dylib : null, development: kind === 'app' ? development : null });
    writeFileSync(resolve(assembled, 'receipt.json'), ipa ? shippedReceipt(whole) : whole);
    if (ipa && kind === 'app') { mkdirSync(dirname(ipa), { recursive: true }); writeFileSync(`${ipa.replace(/\.ipa$/, '')}.receipt.json`, whole); }
    if (ipa) for (const [loose, name] of [...looseArms.map(([, , load, framework]) => [load, framework]), [modulesLoadName, 'ExactModules'], [svgLoadName, 'ExactSvg'], [canvasGpuLoadName, 'ExactCanvasGpu']]) wrapFramework(resolve(assembled, 'Frameworks'), loose, name, app, tv ? 'AppleTVOS' : 'iPhoneOS');
    if (shared && kind === 'app' && activityEntries(app).length) packageActivityArm({ app, bundle: assembled, shared, directory: linkRoot, sdk, triple, device, runApple });
    if (shared && kind === 'app') packageWidgets({ app, bundle: assembled, shared, directory: linkRoot, sdk, triple, device, alarm: tv ? null : alarm,
      runApple, production: production || distribution, profile: provisioned?.extension ?? null,
      // The extension's identifier and shared Keychain group under its own profile's prefix (entryProfiles refused a pair whose prefixes differ).
      entitlements: entitlements({ ...app, id: widgetId(app), widgetAppId: app.id }, signingProfile?.team, signingProfile?.dev ?? !unsigned, null, { simulator: !device, prefix: provisioned?.extension.prefix ?? signingProfile?.prefix }),
      sign: (extension, ent) => run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', ...(device ? ['--entitlements', ent] : []), extension], { stdio: 'ignore' }) });
    // The share extension (LLP 1133.024 D4): a simulator's group needs no account; a phone's waits for the App Group
    // and the `.share` App ID on the team, so a device build has no inbox yet and says so.
    if (kind === 'app' && tv && app.manifest.share_target) console.log('Share extension: not built for tvOS, which has no share sheet (LLP 1133.024 D4).');
    if (kind === 'app' && !tv && app.manifest.share_target) {
      const group = device ? null : widgetGroup(app);
      for (const [key, value] of Object.entries(shareInfo(app, group))) runApple('plutil', ['-replace', key, '-string', value, resolve(assembled, 'Info.plist')]);
      if (device) console.log(`Share extension: not built for a device until the team registers the App Group ${widgetGroup(app)} and the App ID ${shareId(app)} (LLP 1133.024 §19).`);
      else packageShareExtension({ app, bundle: assembled, directory: linkRoot, sdk, triple, group, runApple,
        entitlements: entitlements({ ...app, id: shareId(app), widgetAppId: app.id }, signingProfile?.team, signingProfile?.dev ?? !unsigned, null, { simulator: true, prefix: signingProfile?.prefix }),
        sign: (extension) => run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', extension], { stdio: 'ignore' }) });
    }
    if (!shared && alarm && !tv && kind === 'app') packageAlarmExtension({ app, bundle: assembled, directory: linkRoot, sdk, triple, device, runApple, alarm,
      profile: signingProfile ? profile(ph?.udid, widgetId(app)) : null,
      entitlements: entitlements({ ...app, id: widgetId(app), widgetAppId: app.id }, signingProfile?.team, signingProfile?.dev ?? !unsigned, null, { simulator: !device, prefix: signingProfile?.prefix }),
      sign: (extension, ent) => run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', ...(device ? ['--entitlements', ent] : []), extension], { stdio: 'ignore' }) });
    for (const f of readdirSync(resolve(assembled, 'Frameworks')).filter(f => !gpuArtifacts.some(m => f === (ipa ? `${m.framework}.framework` : m.load)))) run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', resolve(assembled, 'Frameworks', f)], { stdio: 'ignore' });
    if (ipa) assertArchiveFrameworks(resolve(assembled, 'Frameworks'));
    if (ipa) assertActivityRefusals(assembled, runApple);
    run('codesign', ['--force', '--sign', signingIdentity, '--timestamp=none', ...(device ? ['--entitlements', ent] : []), assembled], { stdio: 'ignore' });
  }
  return bundles;
}
