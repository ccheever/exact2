// The Inspector's row values as the panel shows them (stage 15 D4 batch 3 #2, theme2's ruling): a number is the
// shortest decimal that is still the plan's 32-bit float, a length at most 3 decimals, and the text an edit sends
// stays the row's own.
import { expect, test } from 'bun:test';
import { selectionView } from '../apps/design-tool/inspector.ts';

const view = (rows) => selectionView({ e: { build: 'b', node: 1, source: { file: 'empty-state.contract', line: 48, component: 'EmptyState' }, rows }, at: '', axes: '', scheme: 'light', stale: '' }, '', [], [{ id: 'light', label: 'Light', dark: false, values: [] }], true);

test('a row shows its float as the plan wrote it, a length at most 3 decimals, a unitless row every decimal it needs', () => {
  const rows = view([
    { name: 'font_size', value: 19.700000762939453, source: 'authored', editable: { rung: 'token', token: 'cards.empty-title-size', text: '1.23125rem' } },
    { name: 'padding_top', value: 16.006250381469727, source: 'authored', editable: { rung: 'template', file: 'app.contract', at: 'x', sha256: 's', row: 'padding-top', instances: 1, text: '1.000390625rem' } },
    { name: 'opacity', value: 0.3333333432674408, source: 'authored' },
    { name: 'width', value: 12, source: 'authored' },
    { name: 'transform', value: [{ scale: 1.100000023841858 }, 0.5], source: 'authored' },
    { name: 'color', value: 'label', source: 'authored' },
    // Round 1: every unitless row keeps its decimals, a true double is left as it is, and any magnitude shortens.
    { name: 'fill_opacity', value: Math.fround(1 / 3), source: 'authored' },
    { name: 'zoom', value: Math.fround(1.0004), source: 'authored' },
    { name: 'stroke_miterlimit', value: Math.fround(Math.SQRT2), source: 'authored' },
    { name: 'press_scale', value: 0.123456789123, source: 'authored' },
    { name: 'letter_spacing', value: Math.fround(1e-10), source: 'authored' },
    { name: 'flex_grow', value: Math.fround(1e30), source: 'authored' },
    // Round 2: a numeric line-height multiplies; translate and an SVG radius are lengths; an int row is as written;
    // the shortest decimal on the far side of a rounding boundary.
    { name: 'line_height', value: Math.fround(1.234567), source: 'authored' },
    { name: 'translate', value: [Math.fround(16.00625), 0], source: 'authored' },
    { name: 'r', value: Math.fround(16.00625), source: 'authored' },
    { name: 'z_index', value: 100000008, source: 'authored' },
    { name: 'scale', value: 2 ** -96, source: 'authored' },
    { name: 'stroke_opacity', value: Math.fround(1e-10), source: 'authored' },
    { name: 'translate_percent', value: [Math.fround(33.33333), 0], source: 'authored' },
  ]).rows;
  const shown = Object.fromEntries(rows.map(r => [r.name, r.value]));
  expect(shown['font-size']).toBe('19.7');
  expect(shown['padding-top']).toBe('16.006');
  expect(shown.opacity).toBe('0.33333334');
  expect(shown.width).toBe('12');
  expect(shown.transform).toBe('[{"scale":1.1},0.5]');
  expect(shown.color).toBe('label');
  expect(shown['fill-opacity']).toBe('0.33333334');
  expect(shown.zoom).toBe('1.0004');
  expect(shown['stroke-miterlimit']).toBe('1.4142135');
  expect(shown['-exact-press-scale']).toBe('0.123456789123');
  expect(shown['letter-spacing']).toBe('0');
  expect(shown['flex-grow']).toBe('1e+30');
  expect(shown['line-height']).toBe('1.234567');
  expect(shown.translate).toBe('[16.006,0]');
  expect(shown.r).toBe('16.006');
  expect(shown['z-index']).toBe('100000008');
  expect(shown.scale).toBe('1.2621775e-29');
  expect(shown['stroke-opacity']).toBe('1e-10');
  expect(shown['translate-percent']).toBe('[33.33333,0]');
  // What an edit starts from and sends is the source's text, not the rounded number shown.
  expect(rows.find(r => r.name === 'padding-top').editText).toBe('1.000390625rem');
});
