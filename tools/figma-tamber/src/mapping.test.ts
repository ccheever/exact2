// The Figma plugin's mapping (tools/figma-tamber/src/mapping.ts), on the
// theme fixture's export (`contract tamber export dtcg apps/theme-fixture`).
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { conflicts, pull, push, split, tokensOf, type FigmaValue, type Plan } from './mapping';

const TEXT = readFileSync(new URL('../test/theme-fixture.export.json', import.meta.url), 'utf8');
const DOC = JSON.parse(TEXT);

/** The plan's values as Figma would hold them, edited by `f`. */
const held = (plan: Plan, f: (c: string, v: string, m: string, value: FigmaValue) => FigmaValue = (_c, _v, _m, x) => x) =>
  Object.fromEntries(plan.collections.map(c => [c.name, Object.fromEntries(c.variables.map(v => [v.name, Object.fromEntries(c.modes.map(m => [m, f(c.name, v.name, m, v.values[m])]))]))]));

test('one collection, a mode per context, when the file allows as many', () => {
  const plan = pull(DOC, 10);
  expect(plan.collections.map(c => [c.name, c.modes])).toEqual([['Tamber', ['dark-style=black, dark', 'dark-style=black, light', 'dark-style=dim, dark', 'dark-style=dim, light']]]);
  const fill = plan.collections[0].variables.find(v => v.token === 'accent.fill')!;
  expect([fill.name, fill.type]).toEqual(['accent/fill', 'COLOR']);
  expect(fill.values['dark-style=black, light']).toEqual({ r: 0x1d / 255, g: 0x9b / 255, b: 0xf0 / 255, a: 1 });
});

test('one collection per app tuple with Light and Dark when it allows fewer (theme2 Q8); two modes at least', () => {
  const plan = pull(DOC, 2);
  expect(plan.collections.map(c => [c.name, c.modes])).toEqual([['Tamber · dark-style=black', ['Dark', 'Light']], ['Tamber · dark-style=dim', ['Dark', 'Light']]]);
  expect(() => pull(DOC, 1)).toThrow(/at least Light and Dark/);
});

test('an unchanged push is the export pulled, value for value', () => {
  const plan = pull(DOC, 10);
  expect(JSON.parse(push(TEXT, plan, held(plan)))).toEqual(DOC);
});

test('a changed colour is written as the export writes one, only where it changed', () => {
  const plan = pull(DOC, 10);
  const red = { r: 1, g: 0, b: 0, a: 1 };
  const text = push(TEXT, plan, held(plan, (_c, v, m, x) => (v === 'accent/fill' && m === 'dark-style=dim, dark' ? red : x)));
  const out = tokensOf(JSON.parse(text)), was = tokensOf(DOC);
  expect(out['dark-style=dim, dark']['accent.fill'].$value).toEqual({ colorSpace: 'srgb', components: [1, 0, 0], hex: '#ff0000' });
  for (const c of Object.keys(was)) {
    for (const t of Object.keys(was[c])) if (!(c === 'dark-style=dim, dark' && t === 'accent.fill')) expect([c, t, out[c][t]]).toEqual([c, t, was[c][t]]);
  }
});

test('split collections push back to their contexts', () => {
  const plan = pull(DOC, 2);
  const red = { r: 1, g: 0, b: 0, a: 1 };
  const text = push(TEXT, plan, held(plan, (c, v, m, x) => (c === 'Tamber · dark-style=dim' && v === 'accent/fill' && m === 'Light' ? red : x)));
  expect(tokensOf(JSON.parse(text))['dark-style=dim, light']['accent.fill'].$value.hex).toBe('#ff0000');
  expect(tokensOf(JSON.parse(text))['dark-style=black, light']['accent.fill'].$value.hex).toBe('#1d9bf0');
});

test('a value both Figma and the design moved since the pull is a conflict; one only, none', () => {
  const plan = pull(DOC, 10);
  const moved = JSON.parse(TEXT);
  tokensOf(moved)['dark-style=black, light']['accent.fill'].$value = { colorSpace: 'srgb', components: [0, 0, 1], hex: '#0000ff' };
  const fresh = pull(moved, 10);
  const red = { r: 1, g: 0, b: 0, a: 1 };
  const figma = held(plan, (_c, v, m, x) => (v === 'accent/fill' && (m === 'dark-style=black, light' || m === 'dark-style=dim, light') ? red : x));
  expect(conflicts(plan, fresh, figma)).toEqual([{ collection: 'Tamber', variable: 'accent/fill', mode: 'dark-style=black, light' }]);
});

test('contexts split into tuple and scheme', () => {
  expect(split('dark-style=dim, density=compact, dark')).toEqual({ tuple: 'dark-style=dim, density=compact', dark: true });
  expect(split('light')).toEqual({ tuple: '', dark: false });
});
