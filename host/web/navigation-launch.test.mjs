import { expect, test } from 'bun:test';
import { historyURL, launchLocation } from './navigation.js';

// A drive's launch facts (scripts/agent-launch.mjs) are the carrier's, not the app's: none reaches the launch
// route, and every History entry keeps them, so a reload relaunches with them. `formatLocale` (#952) among them.
// One module serves both web hosts: the wasm glue and the JS target (rt.js) navigate through it.
const DRIVE = 'agent=1&seed=3&locale=en-US&formatLocale=en-KR&timeZone=UTC';

test('a drive\'s formatLocale stays out of the launch route', () => {
  expect(launchLocation({ location: { pathname: '/detail/3', search: `?${DRIVE}&q=a` } })).toBe('/detail/3?q=a');
  // An ordinary visit's own query is the route's.
  expect(launchLocation({ location: { pathname: '/', search: '?formatLocale=x' } })).toBe('/?formatLocale=x');
});

test('the launch keeps its fragment, after the query a drive leaves out (LLP 1153 D5)', () => {
  expect(launchLocation({ location: { pathname: '/item/7', search: '', hash: '#moves' } })).toBe('/item/7#moves');
  expect(launchLocation({ location: { pathname: '/item/7', search: `?${DRIVE}&q=a`, hash: '#moves' } })).toBe('/item/7?q=a#moves');
  // A history entry's URL carries the fragment after the drive's facts, as the browser writes it.
  const saved = Object.getOwnPropertyDescriptor(globalThis, 'location');
  Object.defineProperty(globalThis, 'location', { configurable: true, value: { origin: 'http://127.0.0.1:9', href: `http://127.0.0.1:9/?${DRIVE}` } });
  try {
    const url = new URL(historyURL('/item/7#moves'));
    expect([url.pathname, url.hash, url.searchParams.get('agent')]).toEqual(['/item/7', '#moves', '1']);
  } finally { if (saved) Object.defineProperty(globalThis, 'location', saved); else delete globalThis.location; }
});

test('every History entry keeps a drive\'s formatLocale, so navigation and reload keep it', () => {
  const saved = Object.getOwnPropertyDescriptor(globalThis, 'location');
  Object.defineProperty(globalThis, 'location', { configurable: true, value: { origin: 'http://127.0.0.1:9', href: `http://127.0.0.1:9/?${DRIVE}` } });
  try {
    const next = new URL(historyURL('/next?page=2'));
    expect(next.pathname).toBe('/next');
    expect(Object.fromEntries(next.searchParams)).toEqual({ page: '2', agent: '1', seed: '3', locale: 'en-US', formatLocale: 'en-KR', timeZone: 'UTC' });
    // Reloaded there, the launch route is the app's alone again.
    expect(launchLocation({ location: { pathname: next.pathname, search: next.search } })).toBe('/next?page=2');
  } finally { if (saved) Object.defineProperty(globalThis, 'location', saved); else delete globalThis.location; }
});
