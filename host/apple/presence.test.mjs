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
