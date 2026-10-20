import { test, expect } from 'bun:test';
import { HOST_MACOS, withHostDeployment } from './filesystem.mjs';
import { cargoEnvironment } from './app.mjs';

// One macOS deployment target for every cargo call's host side: two calls sharing a target directory
// that differ in it rebuild every build script recording it (ring's), each build (the lead's 20:40Z).
test('every cargo call on a Mac gives its host side one deployment target, and a named one stands', () => {
  const darwin = process.platform === 'darwin';
  expect(withHostDeployment({}).MACOSX_DEPLOYMENT_TARGET).toBe(darwin ? HOST_MACOS : undefined);
  expect(cargoEnvironment({ PATH: '/bin' }).MACOSX_DEPLOYMENT_TARGET).toBe(darwin ? HOST_MACOS : undefined);
  expect(withHostDeployment({ MACOSX_DEPLOYMENT_TARGET: '15.0' }).MACOSX_DEPLOYMENT_TARGET).toBe('15.0');
  expect(cargoEnvironment({ MACOSX_DEPLOYMENT_TARGET: '15.0' }).MACOSX_DEPLOYMENT_TARGET).toBe('15.0');
});
