// no-tells and the root `theme` block (LLP 1095 D4): its entries' literals are the
// app's palette, and only a declaration at the top level is one.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { fixedSizesIn, tellsIn } from './no-tells.mjs';

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

test('a channel may read alpha, and an origin may be an expression over tokens', () => {
  const tokens = new Set(['brand', 'surface.card']);
  const source = 'component A\n  view\n    text "x" color="rgb(from brand calc(alpha * 255) g b)" background-color="rgb(from color-mix(in srgb, brand, surface.card) r g b / 0.5)"\n    text "y" color="rgb(from color-mix(in srgb, brand, white) r g b)" background-color="rgb(from rgb(0 0 0) r g b)"\n';
  assert.deepEqual(tellsIn(source, tokens).map(t => [t.line, t.property]), [[4, 'color'], [4, 'background-color']]);
});

test("an inline theme's entries are tokens as a design's are", () => {
  const source = 'theme\n  brand="#c8102e"\n\ncomponent A\n  view\n    text "x" color="rgb(from brand r g b / 0.5)" background-color="rgb(from ink r g b)"\n';
  assert.deepEqual(tellsIn(source).map(t => [t.line, t.property]), [[6, 'background-color']]);
});

// LLP 1116.020 D4: what verify names as a size Dynamic Type does not move.
test('a fixed font size is a bare number, px or pt, whole or a branch; rem, a role, a control and a kept line are not', () => {
  const source = [
    'component A',
    '  view',
    '    column',
    '      text "a" font-size=28 testId="big"',
    '      text "b" font-size="2rem" font-variant-numeric="tabular-nums"',
    '      text "c" font-size=(wide ? 48 : 28) font-variant-numeric="tabular-nums"',
    '      text "d" font-size=(wide ? "3rem" : "2rem")',
    '      text "e" font-size="-exact-title1"',
    '      text "f" font-size="20px"',
    '        testId=`ff`',
    '      input value=v font-size=17',
    '      text "g" font-size=72 // no-tells: a poster',
    '      text "h" font-size=(items.length > 3 ? "2rem" : "1rem")',
    'style Big',
    '  font-size="30pt"',
  ].join('\n');
  assert.deepEqual(fixedSizesIn(source).map((f) => [f.line, f.tag, f.size, f.testId]), [[4, 'text', 28, 'big'], [6, 'text', 48, null], [9, 'text', 20, 'ff'], [15, 'style', 30, null]]);
});
