// Benchmark-only bundle. The shipping Apple presenter remains native. @ref LLP 1149 D12.
import { spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cargoReproducibilityFlags, executableName, injectedProfiles } from '../../scripts/app.mjs';

const root = resolve(import.meta.dir, '../..');
const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', ...options });
  if (result.status !== 0) throw new Error(`${command} failed (${result.status ?? result.error?.message})`);
};

export function buildPainted(app, args, { macInfoPlist }) {
  if (process.platform !== 'darwin') throw new Error('the painted benchmark carrier requires macOS');
  if (args.some(arg => ['--distribution', '--archive', '--host', '--embed', '--test', '--prebuild', '--url', '--link'].includes(arg))) {
    throw new Error('the painted benchmark carrier supports --release and --run; distribution requires the LLP 1149 accessibility gate');
  }
  const crate = app.crate('macos'), bin = `${crate}-painted`;
  const profile = args.includes('--release') ? 'release' : 'host-dev';
  const target = resolve(root, 'target');
  const out = resolve(target, 'apple-painted', app.name, profile);
  const bundle = resolve(out, `${executableName(app)} benchmark carrier.app`);
  const executable = resolve(bundle, 'Contents/MacOS', executableName(app));
  mkdirSync(resolve(bundle, 'Contents/MacOS'), { recursive: true });
  mkdirSync(resolve(bundle, 'Contents/Resources'), { recursive: true });
  const probe = process.env.EXACT_PAINTED_PROBE;
  const link = [];
  if (probe) {
    const library = resolve(bundle, 'Contents/Resources/benchmark-probe.dylib');
    run('xcrun', ['clang', '-dynamiclib', '-fobjc-arc', '-O2', '-mmacosx-version-min=14.0', '-framework', 'AppKit', '-framework', 'QuartzCore', '-framework', 'Metal', resolve(probe), '-o', library, '-install_name', '@executable_path/../Resources/benchmark-probe.dylib']);
    link.push('-C', `link-arg=${library}`);
  }
  console.error(`${app.displayName}: benchmark carrier; no IME, menus, dialogs or accessibility`);
  run('cargo', ['rustc', ...cargoReproducibilityFlags(app), ...injectedProfiles(app), '--manifest-path', resolve(app.workspace, 'Cargo.toml'), '--target-dir', target, '-p', crate, '--bin', bin, '--profile', profile, '--', ...link], {
    cwd: app.workspace, env: { ...process.env, MACOSX_DEPLOYMENT_TARGET: app.manifest.host?.macos?.minimumOS ?? '14.0' },
  });
  copyFileSync(resolve(target, profile, bin), executable);
  chmodSync(executable, 0o755);
  writeFileSync(resolve(bundle, 'Contents/Info.plist'), macInfoPlist(app));
  if (existsSync(resolve(app.dir, 'assets'))) cpSync(resolve(app.dir, 'assets'), resolve(bundle, 'Contents/Resources/assets'), { recursive: true });
  run('codesign', ['--force', '--deep', '--sign', '-', bundle]);
  console.log(executable);
  if (args.includes('--run')) run(executable, [], { env: { ...process.env, EXACT_ASSETS: app.dir } });
}
