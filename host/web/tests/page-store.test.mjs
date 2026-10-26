// The page's store writes (navigation.js `storeInPage`): kept answers share
// `localStorage`'s quota with the app's secrets and persisted states, and
// are the cache among them (LLP 1027 D4).
import { test, expect } from 'bun:test';
import { storeInPage, pageStore } from '../navigation.js';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { glueDeclarations } from './glue-fixture.mjs';

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

// Run the actual batch consumer; unrelated presentation work has no nodes here.
function host(storage, warned, enabled = true) {
  const noop = () => {}, scope = {
    agentKeepsStore: enabled, root: {}, views: new Map(),
    presence: {}, listSelection: null, followedScrolls: new Map(), pendingScrolls: new Map(),
    prepareContexts: noop, navigation: { project: noop }, refreshSymbols: noop,
    collections: { restyled: noop }, syncLists: noop, focusAutofocus: noop,
    inputHandlers: null, positionContexts: noop, markScrollDocument: noop, log: noop,
    console: { warn: (...line) => warned.push(line), error: (...line) => { throw new Error(line.join(' ')); } },
  };
  Object.defineProperty(scope, 'localStorage', { get: storage });
  scope.pendingStore = pageStore(storage, scope.console.warn);
  vm.createContext(scope);
  vm.runInContext(glueDeclarations(readFileSync(new URL('../glue.js', import.meta.url), 'utf8'), ['apply']), scope);
  return (...ops) => scope.apply({ ops });
}
const write = value => ({ op: 'store', name: 'token', value });

test('a batch with no store ops retries failures, once per name until success or replacement', () => {
  const storage = page(1000), warned = [];
  let blocked = true;
  const set = storage.setItem;
  storage.setItem = (...args) => { if (blocked) throw new Error('unavailable'); set(...args); };
  const apply = host(() => storage, warned);
  apply(write('first')); apply(); apply();
  expect(warned).toHaveLength(1);
  blocked = false; apply();
  expect(storage.getItem('exact.secret.token')).toBe('first');
  blocked = true; apply(write('second')); apply();
  expect(warned).toHaveLength(2);
  apply(write('third')); apply();
  expect(warned).toHaveLength(3);
  blocked = false; apply();
  expect(storage.getItem('exact.secret.token')).toBe('third');
  blocked = true; apply(write('obsolete'));
  apply(write(null)); blocked = false; apply();
  expect(storage.getItem('exact.secret.token')).toBeNull();
});

test('an inaccessible localStorage retains writes and removals; a nameless drive never opens it', () => {
  const storage = page(1000, { 'exact.secret.token': 'old' }), warned = [];
  let blocked = true;
  const get = () => { if (blocked) throw new Error('disabled storage'); return storage; };
  const apply = host(get, warned);
  apply(write('new')); apply(write(null)); apply();
  expect(warned).toHaveLength(2);
  blocked = false; apply();
  expect(storage.getItem('exact.secret.token')).toBeNull();
  blocked = true; apply(write(null)); apply(write('latest'));
  blocked = false; apply();
  expect(storage.getItem('exact.secret.token')).toBe('latest');
  let reads = 0;
  const memoryWarnings = [];
  const memory = host(() => { reads++; throw new Error('must not open storage'); }, memoryWarnings, false);
  memory(write('private')); memory();
  expect(reads).toBe(0);
  expect(memoryWarnings).toEqual([]);
});

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

test('a kept answer the quota refuses leaves the older one and evicts nothing durable', () => {
  const storage = page(200, {
    'exact.secret.exact.kept.library': 'old',
    'exact.secret.token': 't'.repeat(100),
  });
  const warned = [];
  storeInPage(storage, 'exact.kept.library', 'k'.repeat(150), (...line) => warned.push(line));
  expect(warned.map(l => l[1])).toEqual(['exact.kept.library']);
  expect(storage.getItem('exact.secret.exact.kept.library')).toBe('old');
  expect(storage.getItem('exact.secret.token')).toBe('t'.repeat(100));
});

test('a durable write that still does not fit is said, and the kept answers are put back', () => {
  const storage = page(100, { 'exact.state.theme': 'd'.repeat(30), 'exact.secret.exact.kept.library': 'k'.repeat(20) });
  const warned = [];
  storeInPage(storage, 'token', 't'.repeat(80), (...line) => warned.push(line));
  expect(warned.map(l => l[1])).toEqual(['token']);
  expect(storage.getItem('exact.secret.token')).toBeNull();
  expect(storage.getItem('exact.state.theme')).toBe('d'.repeat(30));
  expect(storage.getItem('exact.secret.exact.kept.library')).toBe('k'.repeat(20));
  // Forgetting writes nothing, so it never meets the quota.
  storeInPage(storage, 'exact.state.theme', null);
  expect(storage.getItem('exact.state.theme')).toBeNull();
});
