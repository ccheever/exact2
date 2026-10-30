// LLP 1133.036 D6 on the web: a tool's, a wake's or the device's fetch to a
// space less public than the page's is refused unless that space's
// permission is granted, and its redirects are not followed while a less
// public space's permission is answered and not granted.
import { expect, test } from 'bun:test';
import { reachRefusal, redirectFor, setSpaceState, spaceOf } from './address-space.js';

const page = host => { globalThis.location = { hostname: host }; };
const setSpaceStates = state => { setSpaceState('local', state); setSpaceState('loopback', state); };

test('spaces are read as the URL writes the host', () => {
  expect(spaceOf('192.168.1.40')).toBe('local');
  expect(spaceOf('wled-a1b2c3.local')).toBe('local');
  expect(spaceOf('[fd00::1]')).toBe('local');
  expect(spaceOf('127.0.0.1')).toBe('loopback');
  expect(spaceOf('localhost')).toBe('loopback');
  expect(spaceOf('example.com')).toBe('public');
});

test('a tool from a public page is refused by the space\'s state; the app never', () => {
  page('lights.example');
  setSpaceStates('prompt');
  expect(reachRefusal('http://192.168.1.40/json/si', 'tool')).toBe('person-required');
  expect(reachRefusal('http://127.0.0.1:4790/', 'wake')).toBe('person-required');
  expect(reachRefusal('http://192.168.1.40/', 'app')).toBe(null);
  expect(reachRefusal('https://example.com/', 'tool')).toBe(null);
  setSpaceStates('denied');
  expect(reachRefusal('http://192.168.1.40/', 'device')).toBe('local network access is denied');
  setSpaceStates('granted');
  expect(reachRefusal('http://192.168.1.40/', 'tool')).toBe(null);
  setSpaceStates('unknown');
  expect(reachRefusal('http://192.168.1.40/', 'tool')).toBe('person-required');
});

test('from the loopback development origin nothing is less public', () => {
  page('127.0.0.1');
  setSpaceStates('prompt');
  expect(reachRefusal('http://192.168.1.40/', 'tool')).toBe(null);
  expect(redirectFor('tool', 'follow')).toBe('follow');
});

test('the redirect gate holds a tool\'s redirects while a permission is answered and not granted', () => {
  page('lights.example');
  setSpaceStates('prompt');
  expect(redirectFor('tool', 'follow')).toBe('error');
  expect(redirectFor('tool', 'manual')).toBe('manual');
  expect(redirectFor('app', 'follow')).toBe('follow');
  setSpaceStates('unknown');
  expect(redirectFor('tool', 'follow')).toBe('follow');
  setSpaceStates('granted');
  expect(redirectFor('wake', 'follow')).toBe('follow');
});

test('each space reads its own permission', () => {
  page('lights.example');
  setSpaceState('local', 'granted');
  setSpaceState('loopback', 'prompt');
  expect(reachRefusal('http://192.168.1.40/', 'tool')).toBe(null);
  expect(reachRefusal('http://127.0.0.1:4790/', 'tool')).toBe('person-required');
  expect(reachRefusal('ws://127.0.0.1:4790/', 'tool')).toBe('person-required');
  setSpaceState('loopback', 'granted');
  expect(redirectFor('tool', 'follow')).toBe('follow');
});

test('the substitute\'s state, where the runner sends one, stands for the browser\'s', () => {
  page('lights.example');
  setSpaceStates('granted');
  expect(reachRefusal('http://192.168.1.40/', 'tool', 'prompt')).toBe('person-required');
  expect(redirectFor('tool', 'follow', 'denied')).toBe('error');
  expect(reachRefusal('http://192.168.1.40/', 'app', 'denied')).toBe(null);
});
