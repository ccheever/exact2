// no-tells and the root `theme` block (LLP 1095 D4): its entries' literals are the
// app's palette, and only a declaration at the top level is one.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { tellsIn } from './no-tells.mjs';

test("a theme's literals are its palette and a row naming an entry is no tell", () => {
  const source = 'theme\n  brand="#c8102e"\n  background = "light-dark(#fff, #000)"\n\ncomponent A\n  view\n    text "x" color="brand" background-color="#fff"\n';
  assert.deepEqual(tellsIn(source).map(t => [t.line, t.property]), [[7, 'background-color']]);
});

test('a `theme` word continuing an expression starts no theme', () => {
  const source = 'component A\n  view\n    text "x" color=(true\ntheme\n  ? "#111" : "b")\n      text "y" background-color="#fff"\n';
  assert.deepEqual(tellsIn(source).map(t => [t.line, t.property]), [[3, 'color'], [6, 'background-color']]);
  const list = 'component A\n  view\n    text [\ntheme\n      ].join("") color="#fff"\n';
  assert.deepEqual(tellsIn(list).map(t => [t.line, t.property]), [[5, 'color']]);
});
