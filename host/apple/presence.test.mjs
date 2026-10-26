import { test, expect } from 'bun:test';
import { presenceLines, macPresence, macPresenceProfile } from './presence.mjs';

// LLP 1133.010 D7: what a macOS build says of its presence lines, and the release build it refuses.
const app = { id: 'com.example.lock' };
const ceiling = ['net.fetch https://x', 'secret.keep token user-presence optional', 'secret.keep key biometry-any apple.orWatch', 'secret.keep plain'].join('\n');
const none = () => null;

test('reads the lines that keep a name locked, and which are optional', () => {
  expect(presenceLines(ceiling)).toEqual([{ name: 'token', optional: true }, { name: 'key', optional: false }]);
  expect(presenceLines('secret.keep plain\nsecret.keep other optional')).toEqual([]);
});

test('a development build without a profile says it keeps no locked secret', () => {
  const said = [];
  macPresence(app, ceiling, { find: none, log: (l) => said.push(l) });
  expect(said).toHaveLength(1);
  expect(said[0]).toContain('keeps none');
  expect(said[0]).toContain('presenceSecrets is false, a set of secret.keep key throws NotSupportedError, an optional name stays plain');
});

test('a release build with a name that must be locked is refused without a profile', () => {
  expect(() => macPresence(app, ceiling, { release: true, find: none, log: () => {} })).toThrow(/^grant-needs-profile: secret.keep key must be kept locked/);
  // Only optional names: it ships, keeping them plain.
  expect(() => macPresence(app, 'secret.keep token user-presence optional', { release: true, find: none, log: () => {} })).not.toThrow();
});

test('an app with no presence line says nothing', () => {
  const said = [];
  macPresence(app, 'secret.keep plain', { find: none, log: (l) => said.push(l) });
  expect(said).toEqual([]);
});

test('finds a macOS profile for the id that grants its keychain group', () => {
  const at = (over) => ({ name: 'p', team: 'T', prefix: 'T', appId: 'T.com.example.lock', platforms: ['OSX'], dev: true,
    expires: new Date(Date.now() + 864e5), keychain: ['T.com.example.lock'], ...over });
  const named = null;
  expect(macPresenceProfile(app, { installed: [at({})], named })?.name).toBe('p');
  expect(macPresenceProfile(app, { installed: [at({ platforms: ['iOS'] })], named })).toBeNull();
  expect(macPresenceProfile(app, { installed: [at({ keychain: [] })], named })).toBeNull();
  expect(macPresenceProfile(app, { installed: [at({ keychain: ['T.*'], appId: 'T.*' })], named })?.name).toBe('p');
  expect(macPresenceProfile(app, { installed: [at({})], distribution: true, named })).toBeNull();
});

test('a Mac build with a passkey line says it has none, and a release build is refused', async () => {
  const { macPasskeys, passkeyLines } = await import('./presence.mjs');
  const grants = 'auth.passkey app.example\nauth.passkey localhost';
  expect(passkeyLines(grants)).toEqual(['app.example']);
  const said = [];
  macPasskeys(app, grants, { log: (l) => said.push(l) });
  expect(said[0]).toContain('available is false');
  expect(() => macPasskeys(app, grants, { release: true })).toThrow(/^grant-needs-profile: auth.passkey app.example: /);
  expect(() => macPasskeys(app, 'auth.passkey localhost', { release: true })).not.toThrow();
});

test('a development build asks for a passkey domain in developer mode, a release build never', async () => {
  const { associatedDomains } = await import('./presence.mjs');
  const auth = { passkeys: ['app.example'], associatedDomains: ['webcredentials:app.example', 'webcredentials:auth.example'] };
  expect(associatedDomains(auth, true)).toEqual(['webcredentials:app.example?mode=developer', 'webcredentials:auth.example']);
  expect(associatedDomains(auth, false)).toEqual(auth.associatedDomains);
  expect(associatedDomains(undefined, true)).toEqual([]);
});

test('App Attest and the age range: a Mac build signs neither, a release is refused; the environment is by build (LLP 1133.028)', async () => {
  const { macProfileRows, entitlementValue } = await import('./presence.mjs');
  const said = [];
  macProfileRows(app, 'device.attestation\ndevice.age-range\ndevice.tracking purpose.ads', { log: (l) => said.push(l) });
  expect(said.join('\n')).toMatch(/^device\.attestation, device\.age-range: this build signs neither entitlement/);
  expect(() => macProfileRows(app, 'device.age-range', { release: true })).toThrow(/^grant-needs-profile: device.age-range: /);
  expect(() => macProfileRows(app, 'device.tracking purpose.ads', { release: true })).not.toThrow();
  const env = 'com.apple.developer.devicecheck.appattest-environment';
  expect([entitlementValue(env, true), entitlementValue(env, false), entitlementValue('com.apple.developer.declared-age-range', true)]).toEqual(['development', 'production', true]);
});
