// A development page's storage is its app's own (LLP 1116.004 D8): every app
// the dev server serves shares its origin, and bake-off contacts read an
// earlier round's contacts.json, written under the same default app id. A
// development build writes the app directory's digest into the host files
// that name a store (scripts/agent-launch.mjs `developmentStore`): its files,
// databases and kept keys (storage-environment.js `storageKey`), its persisted
// state (persist.js) and its durable store (rt.js `Store`); a release ships
// them as written, keyed by the id alone. rt.js runs beside stand-ins for the
// modules it imports, as js-link-press.test.mjs runs it.
import { test, expect } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { appDigest, developmentStore } from '../../../scripts/agent-launch.mjs';
import { standIns } from './stand-ins.mjs';

const webJs = new URL('../../web-js/', import.meta.url).pathname, web = new URL('../', import.meta.url).pathname;
const REAL = ['rt.js', 'font-family.js', 'observers.js', 'grid.js', 'overlay.js', 'roster.js', 'router.js', 'schedule.js', 'budget.js', 'persist.js'];
const root = mkdtempSync(resolve(tmpdir(), 'exact-js-storage-'));
/** The runtime as a build lays it out for the app at `appDir`, or as written (a release) when it is null. */
function runtime(name, appDir) {
  const dir = resolve(root, name), ship = code => appDir ? developmentStore(code, appDir) : code;
  mkdirSync(dir);
  for (const [file, source] of standIns(REAL)) writeFileSync(resolve(dir, file), source);
  for (const f of REAL) writeFileSync(resolve(dir, f), ship(readFileSync(resolve(webJs, f), 'utf8')));
  writeFileSync(resolve(dir, 'storage-environment.js'), ship(readFileSync(resolve(web, 'storage-environment.js'), 'utf8')));
  return dir;
}
// One origin's `localStorage`, which every app the dev server serves shares.
const kept = new Map();
globalThis.localStorage = {
  get length() { return kept.size; }, key: i => [...kept.keys()][i] ?? null,
  getItem: k => kept.has(k) ? kept.get(k) : null, setItem: (k, v) => { kept.set(k, String(v)); }, removeItem: k => { kept.delete(k); },
};
const one = '/rounds/r5/apps/contacts', two = '/rounds/r6/apps/contacts';

test('a development build keys its files, databases and kept keys by its app directory; a release by the id alone', async () => {
  const dev = await import(resolve(runtime('key-one', one), 'storage-environment.js'));
  const other = await import(resolve(runtime('key-two', two), 'storage-environment.js'));
  const release = await import(resolve(runtime('key-release', null), 'storage-environment.js'));
  const id = 'com.example.contacts-exact2', d = appDigest(one);
  expect(dev.storageKey(id, 'http://127.0.0.1:8765/')).toBe(`${id}@${d}`);
  expect(dev.storageKey(id, 'http://127.0.0.1:1/?agent=1&storage=s1')).toBe(`${id}@${d}/agent/s1`);
  expect(dev.storageKey(id, 'http://127.0.0.1:1/?agent=1')).toBeNull();
  // Two apps of one id, in two directories, open two stores; a release opens the id's.
  expect(other.storageKey(id, 'http://127.0.0.1:8765/')).not.toBe(dev.storageKey(id, 'http://127.0.0.1:8765/'));
  expect(release.storageKey(id, 'http://127.0.0.1:8765/')).toBe(id);
  expect(release.storageKey(id, 'http://127.0.0.1:1/?agent=1&storage=s1')).toBe(`${id}/agent/s1`);
});

test('persisted state and the durable store are the app\'s own on a shared origin', async () => {
  kept.clear();
  const apps = [];
  for (const [name, dir] of [['one', one], ['two', two], ['release', null]]) {
    const at = runtime(name, dir);
    const rt = await import(resolve(at, 'rt.js')), persist = await import(resolve(at, 'persist.js'));
    apps.push({ rt, persist, prefix: dir ? `@${appDigest(dir)}/` : '' });
  }
  const [a, b, release] = apps;
  expect(a.rt.Kept('state')).toBe(`exact.state.@${appDigest(one)}/`);
  expect(release.rt.Kept('state')).toBe('exact.state.');
  // The first app keeps a goal of 2500 and a token (bake-off water-tracker: an earlier app's goal read as its own).
  const signal = v => ({ n: { v } });
  expect(a.persist.stored('goal', 2000, 'n')).toBe(2000);
  a.persist.keep(signal(2500), 'goal', 'n');
  for (const f of a.rt.After) f();
  a.rt.Store.set('token', 'abc'); a.rt.Store.persist();
  expect(kept.get(`exact.state.${a.prefix}goal`)).toBe('2500');
  expect(kept.get(`exact.secret.${a.prefix}token`)).toBe('abc');
  // The second app, another directory on the same origin, reads neither; the first reads its own again.
  expect(b.persist.stored('goal', 2000, 'n')).toBe(2000);
  b.rt.Store.load();
  expect(b.rt.Store.get('token')).toBeUndefined();
  expect(a.persist.stored('goal', 2000, 'n')).toBe(2500);
  a.rt.Store.map.clear(); a.rt.Store.load();
  expect(a.rt.Store.get('token')).toBe('abc');
  // A release's keys are the runner's: "exact.state.<name>".
  kept.set('exact.state.goal', '1500');
  expect(release.persist.stored('goal', 2000, 'n')).toBe(1500);
});
