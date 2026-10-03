#!/usr/bin/env bun
// Approved diagnostic, not a sixth check: UIKit touch reduction (LLP 1012).
// bun host/apple/touch.mjs [--device] [--phone <id>] [--presenter] [--case 0..6] [--early-session] [--run]
// Separate bundle; never overwrites Caltrain or adds dependencies to ExactKit.
import {spawnSync} from 'node:child_process';
import {copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {resolve} from 'node:path';
import {entitlements, infoPlist, install} from './build.mjs';
import {phone, profile, identity, simulator} from './devices.mjs';

const args = process.argv.slice(2);
function option(flag) {
  const at = args.indexOf(flag);
  if (at < 0) return undefined;
  if (!args[at + 1] || args[at + 1].startsWith('--')) throw new Error(`${flag} needs a value`);
  return args[at + 1];
}
const selected = option('--case') ?? '0';
const presenter = args.includes('--presenter');
const earlySession = args.includes('--early-session');
if (earlySession && (!presenter || selected !== '6')) throw new Error('--early-session requires --presenter --case 6');
if (!(presenter ? /^[0-6]$/ : /^[0-4]$/).test(selected)) throw new Error(`--case must be 0..${presenter ? 6 : 4}`);
const device = args.includes('--device');
const app = {id: 'com.exact.touch-repro', displayName: 'Exact Touch Repro', manifest: {}};
const root = fileURLToPath(new URL('../..', import.meta.url));
const output = resolve(root, 'target/touch-repro');
mkdirSync(output, {recursive: true});
const stage = mkdtempSync(resolve(output, 'build-'));
const bundle = resolve(stage, 'TouchRepro.app');
mkdirSync(bundle);
function run(command, parameters, capture = false) {
  const compiler = parameters.includes('swiftc');
  const result = spawnSync(command, parameters, {encoding: 'utf8', stdio: capture || compiler ? 'pipe' : 'inherit'});
  if (compiler) {
    process.stdout.write(result.stdout ?? '');
    process.stderr.write(result.stderr ?? '');
  }
  if (result.status !== 0) throw new Error(`${command} failed (${result.status}): ${result.stderr ?? ''}`);
  if (compiler && /object file .* was built for newer|using sysroot for|incompatible.*sysroot/i.test(`${result.stdout}\n${result.stderr}`)) {
    throw new Error('refused mixed Apple deployment targets');
  }
  return result.stdout?.trim();
}
const sdk = run('xcrun', ['--sdk', device ? 'iphoneos' : 'iphonesimulator', '--show-sdk-path'], true);
const triple = device ? 'arm64-apple-ios17.0' : `${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-apple-ios17.0-simulator`;
const sources = [resolve(stage, 'main.swift')];
copyFileSync(fileURLToPath(new URL('./ios/touch.swift', import.meta.url)), sources[0]);
const linked = [];
if (presenter) {
  const rustTarget = device ? 'aarch64-apple-ios' : process.arch === 'arm64' ? 'aarch64-apple-ios-sim' : 'x86_64-apple-ios';
  const archive = resolve(option('--archive') ?? resolve(root, 'target', rustTarget, 'release/libcaltrain_ts_apple.a'));
  if (!existsSync(archive)) throw new Error(`Build the native client first, or pass --archive: ${archive}`);
  const kit = resolve(root, 'host/apple/Sources/ExactKit');
  sources.push(...readdirSync(kit, {recursive: true}).filter(p => p.endsWith('.swift')).sort().map(p => resolve(kit, p)));
  linked.push('-D', 'EXACT_PRESENTER', '-I', resolve(root, 'host/apple/Sources/CExact'), archive, '-lc++');
  run(resolve(root, 'target/debug/contract'), ['build', resolve(root, 'contract/corpus/scroll.contract'), '-o', resolve(bundle, 'scroll.plan')]);
}
run('xcrun', ['--sdk', device ? 'iphoneos' : 'iphonesimulator', 'swiftc', '-O', '-sdk', sdk, '-target', triple, '-framework', 'UIKit',
  ...sources, ...linked, '-o', resolve(bundle, 'TouchRepro')]);
writeFileSync(resolve(bundle, 'Info.plist'), infoPlist(app, device, {executable: 'TouchRepro'}));
// Search/Home launches use the same starting case, not transient launch environment.
run('plutil', ['-insert', 'ExactTouchStartCase', '-integer', selected, resolve(bundle, 'Info.plist')]);
run('plutil', ['-insert', 'ExactTouchEarlySession', '-bool', String(earlySession), resolve(bundle, 'Info.plist')]);
if (device) {
  const dev = phone(option('--phone')), prof = profile(dev.udid, app.id);
  const ent = resolve(stage, 'entitlements.plist');
  writeFileSync(ent, entitlements(app, prof.team));
  copyFileSync(prof.path, resolve(bundle, 'embedded.mobileprovision'));
  run('codesign', ['--force', '--sign', identity(prof.team), '--timestamp=none', '--entitlements', ent, bundle]);
  run('codesign', ['--verify', '--strict', bundle]);
  run('xcrun', ['devicectl', 'device', 'install', 'app', '--device', dev.id, '--timeout', '45', bundle]);
  console.log(`Installed Exact Touch Repro: ${bundle}`);
  if (args.includes('--run')) run('xcrun', ['devicectl', 'device', 'process', 'launch', '--device', dev.id,
    '--terminate-existing', '--environment-variables', JSON.stringify({TOUCH_CASE: selected}), '--console', app.id]);
} else {
  run('codesign', ['--force', '--sign', '-', bundle]);
  const dev = simulator(option('--sim'));
  install(dev, bundle);
  console.log(`Installed Exact Touch Repro: ${bundle}`);
  if (args.includes('--run')) {
    const result = spawnSync('xcrun', ['simctl', 'launch', '--terminate-running-process', '--console', dev.udid, app.id],
      {stdio: 'inherit', env: {...process.env, SIMCTL_CHILD_TOUCH_CASE: selected}});
    if (result.status !== 0) process.exit(result.status ?? 1);
  }
}
