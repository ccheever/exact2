// Issue #110: an aria-keyshortcuts chord whose key types no Latin character
// (Korean 2-Set's ㅠ on KeyB, Russian, Greek) is its physical key's (`code`),
// as web apps match; a Latin character is the key wherever it is (AZERTY,
// Dvorak, German's ö), so one chord fires, never two.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const source = readFileSync(new URL('./input-glue.js', import.meta.url), 'utf8').replace('export function', 'function');
function page(chords) {
  const clicks = [], listeners = [];
  const buttons = chords.map(chord => ({ isConnected: true, disabled: false, getClientRects: () => [{}],
    getAttribute: () => chord, click() { clicks.push(chord); } }));
  const context = {
    document: { addEventListener(kind, fn, capture) { if (kind === 'keydown' && capture) listeners.push(fn); }, activeElement: { closest: () => null } },
    getComputedStyle: () => ({ visibility: 'visible' }),
    root: { querySelectorAll: s => (s.includes('aria-modal') ? [] : buttons), addEventListener() {}, contains: () => true },
  };
  runInNewContext(`${source}\ncreateInputHandlers({ root, views: new Map(), retiredViews: new WeakSet(), ready: () => true, inertAncestor: () => false, dispatch() {} });`, context);
  const key = (key, code, extra = {}) => {
    const e = { key, code, metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, composedPath: () => [],
      preventDefault() { this.prevented = true; }, stopImmediatePropagation() {}, ...extra };
    for (const fn of listeners) fn(e);
    return e.prevented === true;
  };
  return { clicks, key };
}

test('a non-Latin key matches the chord its physical key names', () => {
  const p = page(['Meta+B', 'Meta+A', 'Meta+1', 'Meta+/', 'Meta+Shift+B', 'Meta+Shift+#']);
  expect([p.key('ㅠ', 'KeyB'), p.key('ф', 'KeyA'), p.key('ㅂ', 'Digit1'), p.key('ㅠ', 'Slash'),
    p.key('ㅠ', 'KeyB', { shiftKey: true }), p.key('№', 'Digit3', { shiftKey: true })]).toEqual([true, true, true, true, true, true]);
  expect(p.clicks).toEqual(['Meta+B', 'Meta+A', 'Meta+1', 'Meta+/', 'Meta+Shift+B', 'Meta+Shift+#']);
});

test('a Latin key is its own chord, never its physical key\'s', () => {
  const p = page(['Meta+A', 'Meta+Q', 'Meta+B', 'Meta+;', 'Meta+2', 'Alt+c']);
  // AZERTY's q on KeyA, Dvorak's b on KeyN, German's ö on Semicolon, AZERTY's é on Digit2, Option's ç.
  expect([p.key('q', 'KeyA'), p.key('b', 'KeyN'), p.key('x', 'KeyB'), p.key('ö', 'Semicolon'), p.key('é', 'Digit2'),
    p.key('ç', 'KeyC', { metaKey: false, altKey: true })]).toEqual([true, true, false, false, false, false]);
  expect(p.clicks).toEqual(['Meta+Q', 'Meta+B']);
});

test('the fallback keeps the chord\'s modifiers and refuses keys it cannot name', () => {
  const p = page(['Meta+B', 'Control+B']);
  expect([p.key('ㅠ', 'KeyB', { shiftKey: true }), p.key('ㅠ', 'KeyB', { altKey: true }), p.key('ㅠ', 'KeyB', { metaKey: false }),
    p.key('ㅠ', 'IntlRo'), p.key('ㅠ', undefined), p.key(undefined, 'KeyB'), p.key('Process', 'KeyB')]).toEqual([false, false, false, false, false, false, false]);
  expect(p.key('ㅠ', 'KeyB', { metaKey: false, ctrlKey: true })).toBe(true);
  expect(p.clicks).toEqual(['Control+B']);
});
