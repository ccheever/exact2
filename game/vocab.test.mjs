import {test, expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {lookup} from './vocab.mjs';
// Generate once before this integration test: bun game/vocab.mjs --json.
const entries = JSON.parse(readFileSync(new URL('./target/vocab/game-vocab/cards.json', import.meta.url), 'utf8'));

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
