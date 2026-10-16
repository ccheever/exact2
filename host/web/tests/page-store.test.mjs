// The page's store writes (navigation.js `storeInPage`): kept answers share
// `localStorage`'s quota with the app's secrets and persisted states, and
// are the cache among them (LLP 1027 D4).
import { test, expect } from 'bun:test';
import { storeInPage } from '../navigation.js';

/** A `localStorage` of `quota` characters, keys and values together, refusing as a browser does. */
function page(quota, entries = {}) {
  const kept = new Map(Object.entries(entries));
  const used = () => [...kept].reduce((n, [k, v]) => n + k.length + v.length, 0);
  return {
    kept,
    get length() { return kept.size; },
    key: i => [...kept.keys()][i] ?? null,
    getItem: k => kept.get(k) ?? null,
    setItem(k, v) {
      const before = kept.get(k);
      kept.set(k, String(v));
      if (used() > quota) {
        before == null ? kept.delete(k) : kept.set(k, before);
        throw Object.assign(new Error('quota'), { name: 'QuotaExceededError' });
      }
    },
    removeItem: k => { kept.delete(k); },
  };
}

test('a durable write the quota refuses forgets the kept answers and is written', () => {
  const storage = page(200, {
    'exact.secret.exact.kept.library': 'k'.repeat(120),
    'exact.secret.token': 'old',
    'exact.state.theme': 'dark',
  });
  const warned = [];
  storeInPage(storage, 'token', 't'.repeat(80), (...line) => warned.push(line));
  expect(warned).toEqual([]);
  expect(storage.getItem('exact.secret.token')).toBe('t'.repeat(80));
  expect(storage.getItem('exact.secret.exact.kept.library')).toBeNull();
  expect(storage.getItem('exact.state.theme')).toBe('dark');
  // A persisted state too, under its own name.
  storage.setItem('exact.secret.exact.kept.library', 'k'.repeat(40));
  storeInPage(storage, 'exact.state.theme', 'l'.repeat(60), (...line) => warned.push(line));
  expect(storage.getItem('exact.state.theme')).toBe('l'.repeat(60));
  expect(storage.getItem('exact.secret.exact.kept.library')).toBeNull();
});

test('a kept answer the quota refuses forgets the older one and evicts nothing durable', () => {
  const storage = page(200, {
    'exact.secret.exact.kept.library': 'old',
    'exact.secret.token': 't'.repeat(100),
  });
  const warned = [];
  storeInPage(storage, 'exact.kept.library', 'k'.repeat(150), (...line) => warned.push(line));
  expect(warned.length).toBe(1);
  expect(storage.getItem('exact.secret.exact.kept.library')).toBeNull();
  expect(storage.getItem('exact.secret.token')).toBe('t'.repeat(100));
});

test('a durable write that still does not fit is said, and what else is durable stays', () => {
  const storage = page(100, { 'exact.state.theme': 'd'.repeat(60) });
  const warned = [];
  storeInPage(storage, 'token', 't'.repeat(80), (...line) => warned.push(line));
  expect(warned.map(l => l[1])).toEqual(['token']);
  expect(storage.getItem('exact.secret.token')).toBeNull();
  expect(storage.getItem('exact.state.theme')).toBe('d'.repeat(60));
  // Forgetting writes nothing, so it never meets the quota.
  storeInPage(storage, 'exact.state.theme', null);
  expect(storage.getItem('exact.state.theme')).toBeNull();
});
