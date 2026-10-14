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
  modes([['dark-style', ['black', 'dim']], ['ink', ['tint', 'plain']]], { c1: { 0: 0 }, c3: { 0: -1 } }, ['color:var(--x-text-primary)'], {});
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
  const S = (e, prop, unit, f) => { const v = f(); if (v == null) e.style.removeProperty(prop); else e.style.setProperty(prop, v); };
  Sx(S, page, 'color', '', () => '#123456', 0);
  expect(dim.style.map.get('color')).toBeUndefined();
  // One that reads a token (the theme map says so), its text written again.
  Sx(S, page, 'color', '', () => ((globalThis.exact.$th = 1), 'var(--x-text-secondary)'), 0);
  expect(dim.style.map.get('color')).toBe('var(--x-text-secondary)');
  // The window's modes, set by `setMode`, and a refusal that changes nothing.
  globalThis.exact.modes.command(['dark-style', 'dim']);
  expect(document.documentElement.getAttribute('data-x-modes')).toBe('dark-style=dim,ink=tint');
  expect(black.getAttribute('data-x-modes')).toBe('dark-style=black,ink=tint');
  expect(globalThis.exact.modes.set([['dark-style', 'black'], ['dark-style', 'dim']])).toContain('an axis twice');
  expect(globalThis.exact.modes.read()).toEqual({ 'dark-style': 'dim', ink: 'tint' });
  globalThis.exact.modes.command(['dark-style', 'grey']);
  expect(journal.at(-1)).toBe('setMode("dark-style", "grey") refused: an axis of the app\'s design and one of its values');
});
