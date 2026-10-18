// @ref LLP 1126.001 WP9 — ActivityKit stays outside the app's load commands.
import { resolve } from 'node:path';
import { copyFileSync, existsSync, readdirSync, readFileSync } from 'node:fs';
import { wrapFramework } from './assets.mjs';
export function packageActivityArm({ app, bundle, directory, triple, sdk, device, runApple }) {
  const binary = resolve(directory, 'libexact_activity.dylib');
  // The system identifies the attributes by their module and type name. Compile the
  // existing envelope in ExactSnapshot, without linking its driver or ExactCore.
  runApple('xcrun', ['--sdk', device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-parse-as-library', '-emit-library', '-O', '-module-name', 'ExactSnapshot', '-target', triple, '-sdk', sdk,
    '-module-cache-path', resolve(directory, 'cache'),
    resolve('host/apple/Sources/ExactSnapshot/ActivityEnvelope.swift'), resolve('host/apple/activityarm/ActivityArm.swift'), '-o', binary]);
  const frameworks = resolve(bundle, 'Frameworks');
  copyFileSync(binary, resolve(frameworks, 'libexact_activity.dylib'));
  wrapFramework(frameworks, 'libexact_activity.dylib', 'ExactActivity', app, device ? 'iPhoneOS' : 'iPhoneSimulator');
}

/** The update-token ruling admits neither push-to-start nor ordinary remote notifications.
 * Run on archives and the async lane's built iOS app and extension. */
export function assertActivityRefusals(bundle, runApple) {
  const forbidden = ['pushToStartTokenUpdates', 'registerForRemoteNotifications'];
  const executables = [];
  const visit = directory => {
    const plist = resolve(directory, 'Info.plist');
    if (existsSync(plist)) {
      const info = JSON.parse(runApple('plutil', ['-convert', 'json', '-o', '-', plist], { cargoMessages: true }).stdout);
      if (info.UIBackgroundModes?.includes('remote-notification')) throw new Error(`${directory}: remote-notification background mode is refused (LLP 1126.001 WP9)`);
      if (info.CFBundleExecutable) executables.push(resolve(directory, info.CFBundleExecutable));
    }
    for (const tree of ['PlugIns', 'Frameworks']) {
      const path = resolve(directory, tree);
      if (existsSync(path)) for (const name of readdirSync(path)) {
        if (/\.(appex|framework)$/.test(name)) visit(resolve(path, name));
        else if (name.endsWith('.dylib')) executables.push(resolve(path, name));
      }
    }
  };
  visit(bundle);
  for (const binary of executables) {
    const bytes = readFileSync(binary);
    const symbols = runApple('xcrun', ['nm', '-u', binary], { cargoMessages: true }).stdout;
    const found = forbidden.filter(name => bytes.includes(Buffer.from(name)) || symbols.includes(name));
    if (found.length) throw new Error(`${binary}: ${found.join(', ')} refused (LLP 1126.001 WP9)`);
  }
  return executables.length;
}
