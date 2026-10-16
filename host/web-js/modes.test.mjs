// modes.js under the render DOM (dom.js), where a page rendered ahead
// resolves its boundaries as it is built: each boundary's tuple, its pins
// over the one around it, and the inherited token rows written again.
import { test, expect, mock } from 'bun:test';
import { createDocument } from './dom.js';

// rt.js's pieces modes.js reads; the runtime itself needs a browser's.
const After = [], journal = [];
mock.module('./rt.js', () => ({ After, PropHooks: {}, inflight: { n: 0 }, say: line => journal.push(line) }));

test('a boundary takes its tuple and the inherited token text, in document order', async () => {
  const document = createDocument();
  Object.assign(globalThis, { document, __exactRender: true });
  const { modes, Sx } = await import('./modes.js');
  const el = (parent, cls, attrs = {}) => { const e = document.createElement('div'); if (cls) e.setAttribute('class', cls); for (const k in attrs) e.setAttribute(k, attrs[k]); parent.append(e); return e; };
  const root = document.getElementById('exact-root');
  const page = el(root, 'c1');
  const dim = el(page, 'c2', { 'data-x-b': '0', 'data-x-pin': 'dark-style=dim' });
  const black = el(dim, 'c2', { 'data-x-b': '0', 'data-x-pin': 'dark-style=black' });
  const own = el(page, 'c3', { 'data-x-b': '1', 'data-x-pin': 'ink=plain' });
  const bad = el(page, 'c2', { 'data-x-b': '0', 'data-x-pin': 'dark-style:dim' });
  const bound = el(page, 'c2', { 'data-x-b': '0', 'data-x-pin': 'unset' });
  modes([['dark-style', ['black', 'dim']], ['ink', ['tint', 'plain']]], { c1: { 0: 0 }, c3: { 0: -1 } }, ['color:var(--x-text-primary)'], {}, [], { '--x-text-primary': 'text.primary', '--x-text-secondary': 'text.secondary' });
  for (const f of After) f();
  expect(document.documentElement.getAttribute('data-x-modes')).toBe('dark-style=black,ink=tint');
  expect(dim.getAttribute('data-x-modes')).toBe('dark-style=dim,ink=tint');
  expect(black.getAttribute('data-x-modes')).toBe('dark-style=black,ink=tint');
  expect(dim.style.map.get('color')).toBe('var(--x-text-primary)');
  expect(black.style.map.get('color')).toBe('var(--x-text-primary)');
  // Its own colour stands; a colon pin is refused and keeps the tuple around; `unset` is no pin.
  expect(own.style.map.get('color')).toBeUndefined();
  expect(own.getAttribute('data-x-modes')).toBe('dark-style=black,ink=plain');
  expect(bad.getAttribute('data-x-modes')).toBe('dark-style=black,ink=tint');
  expect(bound.getAttribute('data-x-modes')).toBe('dark-style=black,ink=tint');
  expect(journal).toEqual(['invalid -exact-mode value "dark-style:dim": an axis of the app\'s design and one of its values, each axis once; unset']);
  // A bound colour on the page that reads no token: nothing below is written again.
  // rt.js `S`: the same value again writes nothing.
  const S = (e, prop, unit, f) => { const v = f(), last = e.$css ??= {}; if (last[prop] === v) return; last[prop] = v; if (v == null) e.style.removeProperty(prop); else e.style.setProperty(prop, v); };
  const flush = () => { for (const f of After) f(); };
  Sx(S, page, 'color', '', () => '#123456', 0);
  flush();
  expect(dim.style.map.get('color')).toBeUndefined();
  // One that reads a token (the theme map says so), its text written again.
  Sx(S, page, 'color', '', () => ((globalThis.exact.$th = 1), 'var(--x-text-secondary)'), 0);
  flush();
  expect(dim.style.map.get('color')).toBe('var(--x-text-secondary)');
  // A pin's own bound colour, then cleared: the row written again after the
  // binding's write, and kept through another clear.
  let mine = '#b00020';
  const pinned = el(page, '', { 'data-x-b': '0', 'data-x-pin': 'dark-style=dim' }), write = () => Sx(S, pinned, 'color', '', () => mine, 0);
  write(); flush();
  expect(pinned.style.map.get('color')).toBe('#b00020');
  mine = 'unset'; write(); flush();
  expect(pinned.style.map.get('color')).toBe('var(--x-text-secondary)');
  mine = 'inherit'; write(); flush();
  expect(pinned.style.map.get('color')).toBe('var(--x-text-secondary)');
  // layout's token: a cleared binding's row, written again, is implied; one
  // the binding sets is not, and a token folded inline is no token here.
  const T = globalThis.exact.modes.token;
  expect(T(pinned, 'color', 'var(--x-text-secondary)')).toEqual({ names: ['text.secondary'], text: 'var(--x-text-secondary)', modes: 'dark-style=dim,ink=tint', implied: true });
  expect(T(page, 'color', 'var(--x-text-secondary)').implied).toBe(false);
  expect(T(page, 'color', '#123456')).toBeUndefined();
  // Off a boundary a clear is written as itself.
  const plainBox = el(page, '');
  Sx(S, plainBox, 'color', '', () => 'inherit', 0);
  expect(plainBox.style.map.get('color')).toBe('inherit');
  // A stroke's shorthand is written again by its colour alone.
  Sx(S, page, '-webkit-text-stroke', '', () => ((globalThis.exact.$th = 1), '2px var(--x-ink)'), 6);
  flush();
  expect(pinned.style.map.get('-webkit-text-stroke-color')).toBe('var(--x-ink)');
  expect(pinned.style.map.get('-webkit-text-stroke')).toBeUndefined();
  // The window's modes, set by `setMode`, and a refusal that changes nothing.
  globalThis.exact.modes.command(['dark-style', 'dim']);
  expect(document.documentElement.getAttribute('data-x-modes')).toBe('dark-style=dim,ink=tint');
  expect(black.getAttribute('data-x-modes')).toBe('dark-style=black,ink=tint');
  expect(globalThis.exact.modes.set([['dark-style', 'black'], ['dark-style', 'dim']])).toContain('an axis twice');
  expect(globalThis.exact.modes.read()).toEqual({ 'dark-style': 'dim', ink: 'tint' });
  globalThis.exact.modes.command(['dark-style', 'grey']);
  expect(journal.at(-1)).toBe('setMode("dark-style", "grey") refused: an axis of the app\'s design and one of its values');
});

// LLP 1121 stage 4: a development build's live batch sets each changed
// token's property in every tuple's rule, on its base only.
test('a live batch sets each tuple rule on its base, and a stale one asks for the snapshot', async () => {
  const document = createDocument();
  const rule = (selectorText) => ({ selectorText, style: { map: new Map(), setProperty(p, v) { this.map.set(p, v); } } });
  const black = rule(':root,[data-x-modes="dark-style=black"]'), dim = rule('[data-x-modes="dark-style=dim"]'), accent = rule('[data-x-modes="dark-style=dim"][data-x-accent="--x-ink"]');
  document.styleSheets = [{ cssRules: [black, dim, accent] }];
  Object.assign(globalThis, { document, __exactRender: true });
  const { modes } = await import('./modes.js');
  modes([['dark-style', ['black', 'dim']]], {}, [], {}, ['--x-ink'], { '--x-ink': 'ink' }, 'd0', 's0');
  const design = globalThis.exact.design;
  expect(design({ kind: 'tamber-batch', structure: 's0', base: 'other', design: 'd1', tokens: [] }).live).toBe('stale');
  // A notice of the design it runs applies nothing; another structure's is refused.
  expect(design({ kind: 'tamber-batch', structure: 's0', base: 'd0', design: 'd0', tokens: [] }).live).toBe('applied');
  expect(design({ kind: 'tamber-snapshot', structure: 's1', base: null, design: 'd9', tokens: [{ name: 'ink', prop: '--x-ink', css: ['#000', '#000'] }] }).live).toBe('structure');
  const batch = { kind: 'tamber-batch', structure: 's0', base: 'd0', design: 'd1', tokens: [{ name: 'ink', value: '#ff0000ff', slices: [], prop: '--x-ink', css: ['#ff0000ff', '#00ff00ff'] }] };
  expect(design(batch).live).toBe('applied');
  expect(black.style.map.get('--x-ink')).toBe('#ff0000ff');
  expect(dim.style.map.get('--x-ink')).toBe('#00ff00ff');
  // Its accent companion follows, where the token is a colour of its own.
  expect(accent.style.map.get('--exact-accent')).toBe('#00ff00ff');
  // Its base is past now; a snapshot applies whatever this page ran.
  expect(design(batch).live).toBe('stale');
  expect(design({ ...batch, kind: 'tamber-snapshot', base: null, design: 'd2', tokens: [{ ...batch.tokens[0], css: ['#000000ff', '#111111ff'] }] }).live).toBe('applied');
  expect(dim.style.map.get('--x-ink')).toBe('#111111ff');
});

// A development build with no axes gives its boundaries the empty tuple,
// whose rule declares every token there (a token naming the accent then
// resolves in the boundary's own accent).
test('a development build with no axes puts its boundaries in the empty tuple', async () => {
  const document = createDocument();
  Object.assign(globalThis, { document, __exactRender: true });
  const { modes } = await import('./modes.js');
  const root = document.getElementById('exact-root');
  const b = document.createElement('div');
  b.setAttribute('data-x-b', '127');
  root.append(b);
  modes([], {}, [], {}, ['--x-ink'], { '--x-ink': 'ink' }, 'd0', 's0');
  for (const f of After) f();
  expect(document.documentElement.getAttribute('data-x-modes')).toBe('');
  expect(b.getAttribute('data-x-modes')).toBe('');
});

// A release build copies modes.js without the live table (build.mjs
// `withoutDev`): no `exact.design`, whatever the call passes, and the
// modes still work.
test('a release copy of modes.js has no live entry point', async () => {
  const { withoutDev } = await import('./module.mjs');
  const { mkdtempSync, readFileSync, writeFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const release = withoutDev(readFileSync(new URL('./modes.js', import.meta.url), 'utf8'));
  for (const live of ['function design(', 'tupleRules', 'tupleKey', 'designDigest', 'dev-only']) expect(release).not.toContain(live);
  const dir = mkdtempSync(join((await import('node:os')).tmpdir(), 'exact-modes-release-'));
  writeFileSync(join(dir, 'rt.js'), 'export const After = [], PropHooks = {}, inflight = { n: 0 }, say = () => {};\n');
  writeFileSync(join(dir, 'modes.js'), release);
  const document = createDocument();
  Object.assign(globalThis, { document, __exactRender: true, exact: {} });
  const { modes } = await import(join(dir, 'modes.js'));
  modes([['dark-style', ['black', 'dim']]], {}, [], {}, [], {}, 'd0', 's0');
  expect(globalThis.exact.design).toBeUndefined();
  expect(globalThis.exact.modes).toBeDefined();
});
