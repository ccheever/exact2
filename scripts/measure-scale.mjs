#!/usr/bin/env bun
// The base scale's measured cells (LLP 1121 stage 11 B; docs/scale-measured.md): builds and runs one probe and
// prints each `measure <id> <value>` line. Opt-in, never part of a test run.
//
//   bun scripts/measure-scale.mjs mac
//   bun scripts/measure-scale.mjs ios --udid <simulator>   (booted for the run, shut down after if it was off)
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const here = resolve(import.meta.dir, 'measure-scale');
const [what, ...rest] = process.argv.slice(2);
const at = name => { const i = rest.indexOf(name); return i < 0 ? undefined : rest[i + 1]; };
const run = (cmd, args, options = {}) => {
  const r = spawnSync(cmd, args, { encoding: 'utf8', ...options });
  if (r.status !== 0 && !options.allowFail) { process.stderr.write(r.stdout + r.stderr); process.exit(r.status ?? 1); }
  return r.stdout ?? '';
};
const work = mkdtempSync(join(tmpdir(), 'exact-measure-scale-'));
try {
  if (what === 'mac') {
    run('swiftc', ['-O', join(here, 'mac.swift'), '-o', join(work, 'measure-mac')]);
    process.stdout.write(run(join(work, 'measure-mac'), []));
  } else if (what === 'ios') {
    const udid = at('--udid');
    if (!udid) { console.error('usage: bun scripts/measure-scale.mjs ios --udid <simulator>'); process.exit(2); }
    const app = join(work, 'Measure.app');
    mkdirSync(app);
    writeFileSync(join(app, 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.exact.measure-scale</string><key>CFBundleExecutable</key><string>Measure</string>
<key>CFBundleName</key><string>Measure</string><key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string><key>CFBundleShortVersionString</key><string>1.0</string>
<key>MinimumOSVersion</key><string>17.0</string><key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
<key>UILaunchScreen</key><dict/><key>UIApplicationSceneManifest</key><dict><key>UIApplicationSupportsMultipleScenes</key><false/></dict>
</dict></plist>
`);
    run('xcrun', ['-sdk', 'iphonesimulator', 'swiftc', '-parse-as-library', '-target', 'arm64-apple-ios17.0-simulator', '-O', join(here, 'ios.swift'), '-o', join(app, 'Measure')]);
    run('codesign', ['-s', '-', '--force', app]);
    const wasOff = run('xcrun', ['simctl', 'list', 'devices']).split('\n').some(l => l.includes(udid) && l.includes('(Shutdown)'));
    if (wasOff) { run('xcrun', ['simctl', 'boot', udid]); run('xcrun', ['simctl', 'bootstatus', udid, '-b']); }
    try {
      run('xcrun', ['simctl', 'install', udid, app]);
      const out = run('xcrun', ['simctl', 'launch', '--console-pty', '--terminate-running-process', udid, 'com.exact.measure-scale'], { allowFail: true });
      process.stdout.write(out.split('\n').filter(l => /^(os|measure|done)\b/.test(l)).join('\n') + '\n');
      run('xcrun', ['simctl', 'uninstall', udid, 'com.exact.measure-scale'], { allowFail: true });
    } finally {
      if (wasOff) run('xcrun', ['simctl', 'shutdown', udid], { allowFail: true });
    }
  } else {
    console.error('usage: bun scripts/measure-scale.mjs mac | ios --udid <simulator>');
    process.exit(2);
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}
