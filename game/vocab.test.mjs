import {test, expect} from 'bun:test';
import {spawnSync} from 'node:child_process';
import {lookup} from './vocab.mjs';
const cli = (...args) => spawnSync(process.execPath, ['game/vocab.mjs', ...args], {cwd:new URL('..', import.meta.url), encoding:'utf8'});
const generated = cli('--json');
if (generated.status !== 0) throw new Error(generated.stderr || String(generated.error));
const entries = JSON.parse(generated.stdout);

test('rustdoc vocabulary evaluates defaults and keeps public World and handle verbs', () => {
  const find = name => entries.find(e => e.name === name);
  expect(find('Follow').default.lag).toBe(0);
  expect(lookup(entries, 'Follow.lag')[0].fields.map(f => f.name)).toEqual(['lag']);
  expect(lookup(entries, 'World::nearest_xz_mut.radius')[0].arguments[0].type).toBe('f32');
  expect(find('Material').default.color).toEqual([1, 1, 1, 1]);
  expect(find('Transform').default.scale).toEqual([1, 1, 1]);
  expect(find('Follow').fields.map(f => f.name)).toContain('target');
  expect(find('Follow').recipes).toContain('game/recipes/follow-camera.md');
  expect(find('World::nearest_xz_mut').arguments.map(a => a.name)).toEqual(['self', 'origin', 'radius', 'predicate']);
  expect(find('World::save').docs).toContain('save');
  expect(find('RefMut::glow_to').arguments.map(a => a.name)).toEqual(['self', 'intensity', 'seconds']);
  expect(find('Offset').presentation).toBe(true);
  expect(entries.filter(e => e.kind === 'Component').length).toBeGreaterThan(40);
  expect(entries.filter(e => e.kind === 'World verb').length).toBeGreaterThan(90);
  for (const entry of entries.filter(e => e.kind === 'Component' || e.kind === 'Resource')) {
    expect(entry.docs.length).toBeGreaterThan(0);
    if (entry.defaults) expect(entry.default).toBeDefined();
  }
});

test('CLI exposes free functions, trait verbs and null defaults from current sources', () => {
  for (const name of ['animation::step', 'audio::step', 'scene::follow', 'Game::tick']) {
    expect(entries.find(e => e.name === name)?.arguments.length).toBeGreaterThan(0);
  }
  const result = cli('Environment.background');
  expect(result.status, result.stderr || String(result.error)).toBe(0);
  expect(result.stdout).toContain('Default: null');
}, 180000);
