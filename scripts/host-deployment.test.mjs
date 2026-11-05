import { test, expect } from 'bun:test';
import { HOST_MACOS, withHostDeployment } from './filesystem.mjs';
import { cargoEnvironment } from './app.mjs';
import { bakeTool } from './linked-plan.mjs';
import { warmGameEnvironment } from '../host/web-js/module.mjs';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import process from 'node:process';
import { appleCargo } from './cargo-check.mjs';
import { keptCrates } from '../host/apple/crates.mjs';

// One macOS deployment target for every cargo call's host side: two calls sharing a target directory
// that differ in it rebuild every build script recording it (ring's), each build (the lead's 20:40Z).
test('every cargo call on a Mac gives its host side one deployment target, and a named one stands', () => {
  const darwin = process.platform === 'darwin';
  expect(withHostDeployment({}).MACOSX_DEPLOYMENT_TARGET).toBe(darwin ? HOST_MACOS : undefined);
  expect(cargoEnvironment({ PATH: '/bin' }).MACOSX_DEPLOYMENT_TARGET).toBe(darwin ? HOST_MACOS : undefined);
  expect(withHostDeployment({ MACOSX_DEPLOYMENT_TARGET: '15.0' }).MACOSX_DEPLOYMENT_TARGET).toBe('15.0');
  expect(cargoEnvironment({ MACOSX_DEPLOYMENT_TARGET: '15.0' }).MACOSX_DEPLOYMENT_TARGET).toBe('15.0');
});

// The rustc wrapper links proc macros without chained fixups: with them, ld-27037.1 can write one that
// dyld on macOS 27.0.1 refuses ("mis-aligned LINKEDIT string pool", or rustc's "can't find crate for
// `zerofrom_derive`"; scripts/filesystem.mjs).
// It is a name on PATH; a build script's own cargo call inherits it, and PATH does not grow.
test('every cargo call on a Mac runs rustc through the wrapper, which unchains proc macros alone', () => {
  if (process.platform !== 'darwin') return;
  const once = withHostDeployment({ PATH: '/bin' });
  expect(once.RUSTC_WRAPPER).toBe('exact-rustc');
  expect(once.PATH).toMatch(/\/scripts\/rustc:\/bin$/);
  expect(withHostDeployment(once).PATH).toBe(once.PATH);
  expect(withHostDeployment({ PATH: '/bin', RUSTC_WRAPPER: 'sccache' })).toMatchObject({ PATH: '/bin', RUSTC_WRAPPER: 'sccache' });
  const wrapper = Bun.which('exact-rustc', { PATH: once.PATH });
  const run = (...args) => Bun.spawnSync([wrapper, 'echo', ...args]).stdout.toString().trim();
  expect(run('--crate-type', 'proc-macro', 'lib.rs')).toBe('--crate-type proc-macro lib.rs -C link-arg=-Wl,-no_fixup_chains');
  expect(run('--crate-type', 'cdylib', 'lib.rs')).toBe('--crate-type cdylib lib.rs');
});

// A warm web build's receipt records the bake's environment, so reuse reads the same; the bake tool takes
// the build's own target when the build asks one (an app's later macOS minimum), else the host's.
test('a receipt, its reuse and the bake tool read the same deployment target', () => {
  const darwin = process.platform === 'darwin';
  expect(warmGameEnvironment({}).MACOSX_DEPLOYMENT_TARGET).toBe(darwin ? HOST_MACOS : undefined);
  expect(bakeTool.options({ MACOSX_DEPLOYMENT_TARGET: '15.0' }).env.MACOSX_DEPLOYMENT_TARGET).toBe('15.0');
  expect(bakeTool.options().env.MACOSX_DEPLOYMENT_TARGET).toBe(process.env.MACOSX_DEPLOYMENT_TARGET || (darwin ? HOST_MACOS : undefined));
});

// Cargo does not fingerprint Apple's deployment environment. Exercise real kept registry crates and
// an existing target directory: both used to retain iOS 26 objects when asked to build for iOS 17.
test.skipIf(process.platform !== 'darwin')('Apple Cargo units follow the minimum across shared and local caches', () => {
  const fixture = appleFixture();
  try {
    const donor = resolve(fixture.directory, 'donor'), recipient = resolve(fixture.directory, 'recipient');
    const keeper = keptCrates({ root: fixture.directory, env: { ...fixture.environment, HOME: resolve(fixture.directory, 'home') }, profile: 'debug' });
    const build = (target, minimum, label) => {
      const artifacts = fixture.build(target, { IPHONEOS_DEPLOYMENT_TARGET: minimum });
      const objects = resolve(fixture.directory, label);
      mkdirSync(objects);
      fixture.run('xcrun', ['ar', '-x', resolve(target, fixture.triple, 'debug/libdeployment_probe.a')], { cwd: objects });
      const files = readdirSync(objects).filter(file => /^(foldhash|deployment_probe).*\.o$/.test(file));
      expect(files.some(file => file.startsWith('foldhash'))).toBe(true);
      expect(files.some(file => file.startsWith('deployment_probe'))).toBe(true);
      for (const file of files) expect(fixture.run('xcrun', ['vtool', '-show-build', resolve(objects, file)])).toContain(`minos ${minimum}`);
      return artifacts;
    };
    build(donor, '26.0', 'first');
    keeper.keep(donor, 'app', resolve(fixture.directory, 'Cargo.lock'));
    keeper.take(recipient, 'app');
    expect(readdirSync(resolve(recipient, fixture.triple, 'debug/.fingerprint')).some(name => name.startsWith('foldhash-'))).toBe(true);
    expect(build(recipient, '17.0', 'shared').every(artifact => !artifact.fresh)).toBe(true);
    expect(build(donor, '17.0', 'local').every(artifact => !artifact.fresh)).toBe(true);
    expect(build(donor, '26.0', 'return').every(artifact => artifact.fresh)).toBe(true);
  } finally { fixture.dispose(); }
}, 120_000);

test.skipIf(process.platform !== 'darwin')('Apple deployment hashing preserves the flags Cargo selects', () => {
  const fixture = appleFixture();
  const modes = [
    { configuration: '[build]\nrustflags = ["--cfg=from_build"]' },
    { configuration: '[build]\nrustflags = "--cfg=from_build"' },
    { configuration: `[build]\nrustflags = ["--cfg=ignored"]\n[target.${fixture.triple}]\nrustflags = ["--cfg=from_target"]` },
    { configuration: `[target.${fixture.triple}]\nrustflags = "--cfg=from_target_string"` },
    { configuration: '[build]\nrustflags = ["--cfg=ignored"]\n[target.\'cfg(target_os = "ios")\']\nrustflags = ["--cfg=from_cfg"]' },
    { environment: { RUSTFLAGS: '--cfg=from_environment' } },
    { environment: { CARGO_ENCODED_RUSTFLAGS: '--cfg\x1ffrom_encoded="a b"', RUSTFLAGS: '--cfg=ignored' } },
    { environment: { CARGO_TARGET_AARCH64_APPLE_IOS_SIM_RUSTFLAGS: '--cfg=from_target_environment' } },
    { environment: { CARGO_BUILD_RUSTFLAGS: '--cfg=from_build_environment' } },
    { configuration: '[env]\nRUSTFLAGS = { value = "--cfg=forced", force = true }', environment: { RUSTFLAGS: '--cfg=ignored' } },
    { configuration: '[env]\nCARGO_ENCODED_RUSTFLAGS = { value = "--cfg\\u001fforced_encoded", force = true }' },
    { configuration: '[env]\nCARGO_BUILD_RUSTFLAGS = "--cfg=configured_build"' },
    { configuration: '[env]\nCARGO_TARGET_AARCH64_APPLE_IOS_SIM_RUSTFLAGS = "--cfg=configured_target"' },
    { configuration: '[env]\nIPHONEOS_DEPLOYMENT_TARGET = { value = "17.0", force = true }', minimum: '17.0' },
    { environment: { MACOSX_DEPLOYMENT_TARGET: '15.0', TVOS_DEPLOYMENT_TARGET: '18.0', WATCHOS_DEPLOYMENT_TARGET: '11.0' } },
    { native: true, environment: { MACOSX_DEPLOYMENT_TARGET: '15.0' } },
  ];
  try {
    mkdirSync(resolve(fixture.directory, '.cargo'));
    for (const [index, mode] of modes.entries()) {
      writeFileSync(resolve(fixture.directory, '.cargo/config.toml'), mode.configuration ?? '');
      const target = resolve(fixture.directory, `flags-${index}`);
      const flags = (tracked) => {
        const artifact = fixture.build(target, mode.environment, tracked, mode.native).find(artifact => artifact.target.name === 'foldhash');
        const hash = artifact.filenames.find(file => file.endsWith('.rlib')).match(/libfoldhash-([^.]+)\.rlib$/)[1];
        return JSON.parse(readFileSync(resolve(target, mode.native ? '' : fixture.triple, `debug/.fingerprint/foldhash-${hash}/lib-foldhash.json`), 'utf8')).rustflags;
      };
      const before = flags(false), after = flags(true);
      expect(after.filter(flag => !flag.startsWith('--cfg=exact_apple_deployment_targets=')), `mode ${index}`).toEqual(before);
      const environment = { ...fixture.environment, IPHONEOS_DEPLOYMENT_TARGET: mode.minimum ?? '26.0', ...mode.environment };
      const values = ['MACOSX_DEPLOYMENT_TARGET', 'IPHONEOS_DEPLOYMENT_TARGET', 'TVOS_DEPLOYMENT_TARGET', 'WATCHOS_DEPLOYMENT_TARGET'].map(name => environment[name] ?? '');
      expect(after.filter(flag => flag.startsWith('--cfg=exact_apple_deployment_targets=')), `mode ${index}`).toContain(`--cfg=exact_apple_deployment_targets=${JSON.stringify(values.join(':'))}`);
    }
  } finally { fixture.dispose(); }
}, 120_000);

function appleFixture() {
  const directory = mkdtempSync(resolve(tmpdir(), 'exact-apple-deployment-'));
  const triple = 'aarch64-apple-ios-sim';
  const environment = withHostDeployment({ ...process.env, RUSTC_WRAPPER: '', RUSTUP_TOOLCHAIN: '1.97.0' });
  for (const key of Object.keys(environment)) {
    if (key.includes('RUSTFLAGS') || ['CARGO_TARGET_DIR', 'IPHONEOS_DEPLOYMENT_TARGET', 'TVOS_DEPLOYMENT_TARGET', 'WATCHOS_DEPLOYMENT_TARGET'].includes(key)) delete environment[key];
  }
  const run = (command, args, options = {}) => {
    const result = spawnSync(command, args, { cwd: directory, env: environment, encoding: 'utf8', ...options });
    expect(result.status, `${command}: ${result.stderr ?? result.error}`).toBe(0);
    return result.stdout;
  };
  mkdirSync(resolve(directory, 'src'));
  writeFileSync(resolve(directory, 'Cargo.toml'), '[package]\nname = "deployment-probe"\nversion = "0.1.0"\nedition = "2021"\n[lib]\ncrate-type = ["staticlib"]\n[dependencies]\nfoldhash = "=0.2.0"\n[profile.dev]\ndebug = false\n');
  writeFileSync(resolve(directory, 'src/lib.rs'), 'use std::hash::BuildHasher;\n#[no_mangle]\npub extern "C" fn probe() -> u64 { foldhash::fast::FixedState::default().hash_one(7_u64) }\n');
  return { directory, triple, environment, run, dispose: () => rmSync(directory, { recursive: true, force: true }),
    build(target, extra = {}, tracked = true, native = false) {
      const args = ['build', '--offline', ...(native ? [] : ['--target', triple]), '--target-dir', target, '--message-format=json'];
      const options = { cwd: directory, env: { ...environment, IPHONEOS_DEPLOYMENT_TARGET: '26.0', ...extra }, encoding: 'utf8' };
      const result = tracked ? spawnSync(...appleCargo('cargo', args, options)) : spawnSync('cargo', args, options);
      expect(result.status, result.stderr ?? String(result.error)).toBe(0);
      return result.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line)).filter(event => event.reason === 'compiler-artifact');
    } };
}
