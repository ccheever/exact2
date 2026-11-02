import { test, expect } from 'bun:test';
import { HOST_MACOS, withHostDeployment } from './filesystem.mjs';
import { cargoEnvironment } from './app.mjs';
import { bakeTool } from './linked-plan.mjs';
import { warmGameEnvironment } from '../host/web-js/module.mjs';

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
