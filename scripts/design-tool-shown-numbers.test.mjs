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
  ]).rows;
  const shown = Object.fromEntries(rows.map(r => [r.name, r.value]));
  expect(shown['font-size']).toBe('19.7');
  expect(shown['padding-top']).toBe('16.006');
  expect(shown.opacity).toBe('0.33333334');
  expect(shown.width).toBe('12');
  expect(shown.transform).toBe('[{"scale":1.1},0.5]');
  expect(shown.color).toBe('label');
  // What an edit starts from and sends is the source's text, not the rounded number shown.
  expect(rows.find(r => r.name === 'padding-top').editText).toBe('1.000390625rem');
});
