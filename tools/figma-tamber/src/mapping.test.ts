// The Figma plugin's mapping (tools/figma-tamber/src/mapping.ts), on the
// theme fixture's export (`contract tamber export dtcg apps/theme-fixture`).
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { conflicts, contextsMoved, doorOf, families, familiesOf, lightClashes, newEnough, plainWhy, pull, push, same, split, tokensOf, type FigmaValue, type Plan } from './mapping';

// The fixture's export as read (its dark-style axis under scheme=dark: one light mode), and the
// same with that axis an ordinary one (a mode for each context), which most tests use.
const REAL = readFileSync(new URL('../test/theme-fixture.export.json', import.meta.url), 'utf8');
const TEXT = (d => { for (const a of d.sets.exact.sources[0].$extensions.exact.axes) delete a.under; return JSON.stringify(d); })(JSON.parse(REAL));
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

test('light contexts that differ only by an axis under scheme=dark are one light mode; its value goes to each on push', () => {
  const real = JSON.parse(REAL);
  const plan = pull(real, 10);
  expect(plan.collections.map(c => [c.modes, c.also])).toEqual([[['light', 'dark-style=black, dark', 'dark-style=dim, dark'], { light: ['dark-style=dim, light'] }]]);
  expect(plan.collections[0].contexts.light).toBe('dark-style=black, light');
  const red = { r: 1, g: 0, b: 0, a: 1 };
  const text = push(REAL, plan, held(plan, (_c, v, m, x) => (v === 'accent/fill' && m === 'light' ? red : x)));
  const out = tokensOf(JSON.parse(text));
  expect([out['dark-style=black, light']['accent.fill'].$value.hex, out['dark-style=dim, light']['accent.fill'].$value.hex]).toEqual(['#ff0000', '#ff0000']);
  expect(out['dark-style=dim, dark']['accent.fill']).toEqual(tokensOf(real)['dark-style=dim, dark']['accent.fill']);
  // Contexts the light mode stands for are no context the design dropped.
  expect(contextsMoved(plan, real)).toEqual({ added: [], removed: [] });
});

test('quick re-check: a light mode\'s edit is checked against each light context it stands for', () => {
  const real = JSON.parse(REAL);
  const plan = pull(real, 10);
  const red = { r: 1, g: 0, b: 0, a: 1 };
  const figma = held(plan, (_c, v, m, x) => (v === 'accent/fill' && m === 'light' ? red : x));
  // The design moves only the twin (dim's light): the light mode's edit conflicts there.
  const moved = JSON.parse(REAL);
  tokensOf(moved)['dark-style=dim, light']['accent.fill'].$value = { colorSpace: 'srgb', components: [0, 0, 1], hex: '#0000ff' };
  expect(conflicts(plan, moved, figma).map(c => [c.mode, c.context, c.figmaOnly ?? false])).toEqual([['light', 'dark-style=black, light', true], ['light', 'dark-style=dim, light', false]]);
});

test('the plugin builds (its main thread and its UI, as Figma loads them)', async () => {
  for (const entry of ['code.ts', 'ui.ts']) {
    const built = await Bun.build({ entrypoints: [new URL(`./${entry}`, import.meta.url).pathname], target: 'browser', format: 'iife' });
    expect([entry, built.success, built.logs.map(String)]).toEqual([entry, true, []]);
  }
});

// What a browser has and Figma's main-thread sandbox does not (Charlie's
// live push: structuredClone). The UI runs in an iframe, a browser's.
// `globalThis` and `self` too, so no listed name is reached through them.
const UNSANDBOXED = ['globalThis', 'self', 'structuredClone', 'URL', 'URLSearchParams', 'TextEncoder', 'TextDecoder', 'atob', 'btoa', 'crypto', 'queueMicrotask', 'Blob', 'XMLHttpRequest', 'localStorage', 'window', 'document', 'navigator', 'performance', 'setImmediate', 'process', 'Buffer', 'require'];

test('the main thread reads none of the listed globals Figma\'s sandbox lacks', async () => {
  // The bundler's `define` swaps a free name only (never a property, a key,
  // a string or a local), so a marker left in the output is a global read.
  const build = async (entry: string) => {
    const built = await Bun.build({ entrypoints: [new URL(`./${entry}`, import.meta.url).pathname], target: 'browser', format: 'iife', define: Object.fromEntries(UNSANDBOXED.map(g => [g, `__figma_lacks_${g}`])) });
    return [...new Set([...(await built.outputs[0].text()).matchAll(/__figma_lacks_(\w+)/g)].map(m => m[1]))];
  };
  expect(await build('code.ts')).toEqual([]);
  // And it sees one: the UI, a browser's, reads `URL` and `document`.
  expect(await build('ui.ts')).toEqual(expect.arrayContaining(['URL', 'document']));
});

test('one choice a token and light side: two light sides of one token are asked and written apart', () => {
  const ctx = (d: string, s: string, scheme: string) => `density=${d}, dark-style=${s}, ${scheme}`;
  const contexts: Record<string, any> = {}, info: Record<string, any> = {};
  const hex = (h: string) => ({ c: { $type: 'color', $value: { colorSpace: 'srgb', components: [0, 0, 0], hex: h } } });
  for (const d of ['a', 'b']) for (const s of ['black', 'dim']) for (const scheme of ['light', 'dark']) {
    contexts[ctx(d, s, scheme)] = [hex('#000000')];
    info[ctx(d, s, scheme)] = { modes: { density: d, 'dark-style': s }, scheme };
  }
  const doc = { modifiers: { mode: { contexts, default: ctx('a', 'black', 'light') } }, sets: { exact: { sources: [{ $extensions: { exact: { axes: [{ name: 'density', values: ['a', 'b'] }, { name: 'dark-style', values: ['black', 'dim'], under: 'dark' }], contexts: info } } }] } } };
  const plan = pull(doc, 2);
  expect(plan.collections.length).toBe(4);
  const red = { r: 1, g: 0, b: 0, a: 1 }, blue = { r: 0, g: 0, b: 1, a: 1 };
  const now = held(plan, (c, _v, m, x) => (m === 'Light' && c.endsWith('density=a, dark-style=black') ? red : m === 'Light' && c.endsWith('density=b, dark-style=dim') ? blue : x));
  const clashes = lightClashes(doc, plan, now);
  expect(clashes.map(c => c.key)).toEqual(['c@density=a, dark-style=black, light', 'c@density=b, dark-style=black, light']);
  const out = tokensOf(JSON.parse(push(JSON.stringify(doc), plan, now, { [clashes[0].key]: 'Tamber · density=a, dark-style=black', [clashes[1].key]: 'Tamber · density=b, dark-style=dim' })));
  expect([ctx('a', 'black', 'light'), ctx('a', 'dim', 'light'), ctx('b', 'black', 'light'), ctx('b', 'dim', 'light')].map(k => out[k].c.$value.hex)).toEqual(['#ff0000', '#ff0000', '#0000ff', '#0000ff']);
});

test('a value not proposed for a dark style\'s Light value says which differs from the one kept and what to do, never the rule\'s id', () => {
  const why = "the design would refuse it (tamber-under-light: the light side `#1df032ff` is not the default's `#1d9bf0ff`: an axis `under scheme=dark` varies the dark side only)";
  expect(plainWhy(why)).toBe('its Light value here, #1df032ff, is not the one the design keeps for every dark style, #1d9bf0ff: only the Dark side varies by dark style, so put the Light value back to #1d9bf0ff, or change it for every dark style at once (the same in every Light mode), then push');
  expect(plainWhy('no token of that name')).toBe('no token of that name');
});
