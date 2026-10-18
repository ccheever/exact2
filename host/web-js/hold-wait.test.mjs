// A hold sent while the page's input glue loads (LLP 1122 D8, LLP 1121 stage 8 E): the dev page waits for the
// glue's own `exact-hold` signal, not a fixed 2 s, so a page reloading after a rebuild on a loaded machine still
// holds the replayed state; it gives up after its bound, or at once when a newer frame comes. (host/web/dev.js
// keeps the same wait; host/web/tests/hold.test.mjs checks the glue says `exact-hold`.)
import { afterEach, expect, test } from 'bun:test';
import { glueHold } from './inspect-dev.js';

afterEach(() => { delete globalThis.exact; });

test('a hold waits for the input glue to say it has loaded, however long it takes within the bound', async () => {
  delete globalThis.exact;
  const at = Date.now();
  const waiting = glueHold(10_000);
  // The glue loads 3 s on: longer than the 2 s the page used to wait.
  setTimeout(() => { (globalThis.exact ??= {}).hold = () => ({ held: 'active' }); globalThis.dispatchEvent(new Event('exact-hold')); }, 3000);
  const hold = await waiting;
  expect(typeof hold).toBe('function');
  expect(Date.now() - at).toBeGreaterThanOrEqual(2900);
  expect(Date.now() - at).toBeLessThan(8000);
});

test('the glue\'s signal ends the wait as it comes, not at the next look', async () => {
  delete globalThis.exact;
  let said = 0;
  const waiting = glueHold(10_000).then((hold) => ({ hold, late: Date.now() - said }));
  // 100 ms on, between two of the wait's own looks (every 250 ms).
  setTimeout(() => { (globalThis.exact ??= {}).hold = () => ({}); said = Date.now(); globalThis.dispatchEvent(new Event('exact-hold')); }, 100);
  const { hold, late } = await waiting;
  expect(typeof hold).toBe('function');
  expect(late).toBeLessThan(100);
});

test('a glue loaded already answers at once; one that never loads gives up at the bound; a newer frame ends the wait', async () => {
  globalThis.exact = { hold: () => ({}) };
  expect(typeof (await glueHold(10_000))).toBe('function');
  delete globalThis.exact;
  let at = Date.now();
  expect(await glueHold(600)).toBe(null);
  expect(Date.now() - at).toBeLessThan(5000);
  let newer = false;
  setTimeout(() => { newer = true; }, 300);
  at = Date.now();
  expect(await glueHold(10_000, () => newer)).toBe(null);
  expect(Date.now() - at).toBeLessThan(5000);
});
