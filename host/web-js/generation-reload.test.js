// A page whose build was reaped loads again into the build in place, once (generation-reload.js; theme2's ruling on
// the build-swap pair). A drive's page is never given the handler (rt.js `mount`).
import { expect, test } from 'bun:test';
import { reloadOnGone } from './generation-reload.js';

const page = (search = '') => {
  const listeners = {}, store = new Map();
  let reloads = 0;
  const win = { location: { search, reload: () => { reloads++; } }, addEventListener: (k, f) => { listeners[k] = f; },
    sessionStorage: { getItem: (k) => store.get(k) ?? null, setItem: (k, v) => store.set(k, v), removeItem: (k) => store.delete(k) }, setTimeout: () => 0 };
  return { win, listeners, store, reloads: () => reloads };
};

test('a chunk or script of a reaped generation reloads the page once; other failures do not', () => {
  const p = page();
  expect(reloadOnGone(p.win)).toBe(true);
  p.listeners.error({ target: { src: '/app.js' } });
  p.listeners.unhandledrejection({ reason: new TypeError('Failed to fetch dynamically imported module: http://x/lazy-1.js') });
  expect(p.reloads()).toBe(0);
  p.listeners.unhandledrejection({ reason: new TypeError('Failed to fetch dynamically imported module: http://x/.exact/gen/0123456789abcdef0123456789abcdef/lazy-1.js') });
  expect(p.reloads()).toBe(1);
  p.listeners.error({ target: { src: 'http://x/.exact/gen/0123456789abcdef0123456789abcdef/app.js' } });
  expect(p.reloads()).toBe(1); // once a session: a bad build cannot loop
});
