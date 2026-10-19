// The Figma plugin's mapping (tools/figma-tamber/src/mapping.ts), on the
// theme fixture's export (`contract tamber export dtcg apps/theme-fixture`).
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { conflicts, contextsMoved, doorOf, families, familiesOf, newEnough, pull, push, same, split, tokensOf, type FigmaValue, type Plan } from './mapping';

const TEXT = readFileSync(new URL('../test/theme-fixture.export.json', import.meta.url), 'utf8');
const DOC = JSON.parse(TEXT);

/** The plan's values as Figma would hold them, edited by `f`. */
const held = (plan: Plan, f: (c: string, v: string, m: string, value: FigmaValue) => FigmaValue = (_c, _v, _m, x) => x) =>
  Object.fromEntries(plan.collections.map(c => [c.name, Object.fromEntries(c.variables.map(v => [v.name, Object.fromEntries(c.modes.map(m => [m, f(c.name, v.name, m, v.values[m])]))]))]));

test('one collection, a mode per context, the export\'s default first, when the file allows as many', () => {
  const plan = pull(DOC, 10);
  expect(plan.collections.map(c => [c.name, c.modes])).toEqual([['Tamber', ['dark-style=black, light', 'dark-style=black, dark', 'dark-style=dim, dark', 'dark-style=dim, light']]]);
  const fill = plan.collections[0].variables.find(v => v.token === 'accent.fill')!;
  expect([fill.name, fill.type]).toEqual(['accent/fill', 'COLOR']);
  expect(fill.values['dark-style=black, light']).toEqual({ r: 0x1d / 255, g: 0x9b / 255, b: 0xf0 / 255, a: 1 });
});

test('one collection per app tuple with Light and Dark when it allows fewer (theme2 Q8); two modes at least', () => {
  const plan = pull(DOC, 2);
  expect(plan.collections.map(c => [c.name, c.modes])).toEqual([['Tamber · dark-style=black', ['Light', 'Dark']], ['Tamber · dark-style=dim', ['Light', 'Dark']]]);
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

test('a value Figma changed is one a pull writes over: both changed, or Figma only; the design only, none', () => {
  const plan = pull(DOC, 10);
  const moved = JSON.parse(TEXT);
  tokensOf(moved)['dark-style=black, light']['accent.fill'].$value = { colorSpace: 'srgb', components: [0, 0, 1], hex: '#0000ff' };
  const red = { r: 1, g: 0, b: 0, a: 1 };
  const figma = held(plan, (_c, v, m, x) => (v === 'accent/fill' && (m === 'dark-style=black, light' || m === 'dark-style=dim, light') ? red : x));
  expect(conflicts(plan, moved, figma)).toEqual([
    { token: 'accent.fill', context: 'dark-style=black, light', collection: 'Tamber', variable: 'accent/fill', mode: 'dark-style=black, light', gone: false },
    { token: 'accent.fill', context: 'dark-style=dim, light', collection: 'Tamber', variable: 'accent/fill', mode: 'dark-style=dim, light', gone: false, figmaOnly: true },
  ]);
  // The design's own change, Figma's left alone: nothing written over.
  expect(conflicts(plan, moved, held(plan))).toEqual([]);
});

test('conflicts are by token and context, whatever the layout; a Figma edit the design dropped is one', () => {
  const plan = pull(DOC, 2); // split, as a small plan has it
  const moved = JSON.parse(TEXT);
  tokensOf(moved)['dark-style=dim, light']['accent.fill'].$value = { colorSpace: 'srgb', components: [0, 0, 1], hex: '#0000ff' };
  delete moved.modifiers.mode.contexts['dark-style=black, dark'];
  const red = { r: 1, g: 0, b: 0, a: 1 };
  const figma = held(plan, (_c, v, _m, x) => (v === 'accent/fill' ? red : x));
  expect(conflicts(plan, moved, figma).map(c => [c.collection, c.mode, c.context, c.gone, !!c.figmaOnly])).toEqual([
    ['Tamber · dark-style=black', 'Light', 'dark-style=black, light', false, true],
    ['Tamber · dark-style=black', 'Dark', 'dark-style=black, dark', true, false],
    ['Tamber · dark-style=dim', 'Light', 'dark-style=dim, light', false, false],
    ['Tamber · dark-style=dim', 'Dark', 'dark-style=dim, dark', false, true],
  ]);
  expect(contextsMoved(plan, moved)).toEqual({ added: [], removed: ['dark-style=black, dark'] });
});

test('a changed colour is written to the byte, its alpha only when not opaque', () => {
  const plan = pull(DOC, 10);
  const text = push(TEXT, plan, held(plan, (_c, v, m, x) => (v === 'accent/fill' && m === 'dark-style=dim, dark' ? { r: 0.3, g: 0.6, b: 0.9, a: 0.5 } : x)));
  expect(tokensOf(JSON.parse(text))['dark-style=dim, dark']['accent.fill'].$value).toEqual({ colorSpace: 'srgb', components: [77 / 255, 153 / 255, 230 / 255], alpha: 128 / 255, hex: '#4d99e6' });
});

test('families: a name with a comma or a quote survives the round trip', () => {
  const all = ['ACME, Sans', 'Say "hi"', 'serif'];
  expect(families(all)).toBe('"ACME, Sans", "Say \\"hi\\"", serif');
  expect(familiesOf(families(all))).toEqual(all);
  expect(familiesOf('Inter, system-ui ,  sans-serif')).toEqual(['Inter', 'system-ui', 'sans-serif']);
  expect(familiesOf("'ACME, Sans', serif")).toEqual(['ACME, Sans', 'serif']);
  expect(() => familiesOf('"ACME, serif')).toThrow(/not closed/);
  expect(familiesOf("Mike's Font, serif")).toEqual(["Mike's Font", 'serif']);
  expect(familiesOf(families(["Mike's Font", 'serif']))).toEqual(["Mike's Font", 'serif']);
});

test('contexts split into tuple and scheme', () => {
  expect(split('dark-style=dim, density=compact, dark')).toEqual({ tuple: 'dark-style=dim, density=compact', dark: true });
  expect(split('light')).toEqual({ tuple: '', dark: false });
});

test('an alpha that rounds to the opaque byte is opaque', () => {
  expect(same({ r: 1, g: 0, b: 0, a: 0.9999 }, { r: 1, g: 0, b: 0, a: 1 })).toBe(true);
  expect(same({ r: 1, g: 0, b: 0, a: 0.99 }, { r: 1, g: 0, b: 0, a: 1 })).toBe(false);
});

test('a code or a token goes only to an https server or to this machine over http; a server below tamber/1.14 is too old', () => {
  expect(doorOf('https://figma-a77a37.tuft.host/').origin).toBe('https://figma-a77a37.tuft.host');
  expect(doorOf('http://127.0.0.1:7791').origin).toBe('http://127.0.0.1:7791');
  expect(doorOf('http://localhost:7791').origin).toBe('http://localhost:7791');
  for (const bad of ['http://figma-a77a37.tuft.host', 'http://10.0.0.4:7791', 'https://me:pw@x.example', 'ftp://x', 'not a url'])
    expect([bad, doorOf(bad).error]).not.toEqual([bad, '']);
  expect([newEnough('tamber/1.14'), newEnough('tamber/1.15'), newEnough('tamber/2.0')]).toEqual([true, true, true]);
  expect([newEnough('tamber/1.13'), newEnough('tamber/1.9'), newEnough(undefined), newEnough('x')]).toEqual([false, false, false, false]);
});
