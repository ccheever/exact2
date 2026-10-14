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

// LLP 1121: an app's design names its tokens, and an expression over tokens and
// roles is folded by the build, not a literal.
test('a design token and an expression over tokens are no tells', () => {
  const tokens = new Set(['surface.card', 'accent.fill']);
  const source = 'component A\n  view\n    text "x" color="surface.card" background-color="color-mix(in oklab, accent.fill 40%, surface.card)"\n    text "y" border-color="rgb(from surface.card r g b / 0.5)" outline-color="oklch(from CanvasText l c h / 0.4)"\n    text "z" color="rgb(from #fff r g b)" background-color="color-mix(in srgb, accent.fill, white)" border-color="rgb(0 0 0 / 0.5)"\n';
  assert.deepEqual(tellsIn(source, tokens).map(t => [t.line, t.property]), [[5, 'color'], [5, 'background-color'], [5, 'border-color']]);
  // Without the design, a relative colour from an unknown name is one.
  assert.deepEqual(tellsIn('component A\n  view\n    text "y" border-color="rgb(from surface.card r g b / 0.5)"\n').map(t => t.property), ['border-color']);
});

test("an app's design is read for its token names", async () => {
  const { designTokens } = await import('./no-tells.mjs');
  const names = designTokens(new URL('../apps/theme-fixture/app.contract', import.meta.url).pathname);
  for (const name of ['surface.page', 'text.primary', 'accent.soft', 'AccentColor']) assert.ok(names.has(name), name);
  assert.ok(!names.has('dark-style'));
});

test('a relative colour keeps its channels to be an expression, and roles match without case', () => {
  const tokens = new Set(['surface.card']);
  const source = 'component A\n  view\n    text "x" color="oklch(from canvastext l c h / 0.4)" background-color="oklch(from surface.card calc(l * 0.9) c h)" border-color="color(from surface.card srgb r g b / 0.5)"\n    text "y" color="rgb(from surface.card 255 0 0)" background-color="oklch(from CanvasText 0.7 0.2 30)"\n';
  assert.deepEqual(tellsIn(source, tokens).map(t => [t.line, t.property]), [[4, 'color'], [4, 'background-color']]);
});

test("a design's entries are read at the block's own indent, with or without a kind", async () => {
  const { mkdtempSync, writeFileSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const { join } = await import('node:path');
  const { designTokens } = await import('./no-tells.mjs');
  const dir = mkdtempSync(join(tmpdir(), 'no-tells-design-'));
  writeFileSync(join(dir, 'app.json'), '{}');
  writeFileSync(join(dir, 'app.tamber'), 'tamber t\n\ntheme\n    surface.card: color = #ffffff\n      when x=y: #000000\n    brand = #c8102e\n');
  assert.deepEqual([...designTokens(join(dir, 'app.contract'))], ['surface.card', 'brand']);
});
