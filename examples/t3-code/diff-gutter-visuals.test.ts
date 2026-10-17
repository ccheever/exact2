// The gutter utility placement expression against the reference offsets measured over CDP.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const source = (file: string) => readFileSync(join(import.meta.dir, file), 'utf8');
const component = (text: string, name: string) => { const start = text.indexOf(`component ${name}\n`); const next = text.indexOf('\ncomponent ', start + 1); return text.slice(start, next < 0 ? undefined : next); };
const rows = source('diff-rows.contract');
const cell = component(rows, 'DiffCell');

describe('the "+" sits at the number\'s right, as Pierre\'s gutter utility', () => {
  const plus = cell.slice(cell.indexOf('button press=comment'));

  test('its left edge is the number\'s right edge: the reference\'s offsets at the default size', () => {
    const left = /left=\(([^)]*\)[^)]*)\) top=0 width=20/.exec(plus)?.[1];
    expect(left).toBe('(gutter - 2) * size / 13 - 7.83 * size / 13');
    const at = (gutter: number, size: number) => Function('gutter', 'size', `return ${left}`)(gutter, size) as number;
    // CDP: the Code tab's two-digit gutter (41.1 wide) puts the "+" 31.3 from the cell's left; the Diff panel's one-digit
    // gutter (33.3) 23.5. The number cell (diff-rows.contract) is ((gutter - 2) * size / 13 + 2) wide with a 1ch + 2pt inset.
    expect(Math.round(at(41.1, 13) * 10) / 10).toBe(31.3);
    expect(Math.round(at(33.3, 13) * 10) / 10).toBe(23.5);
    expect(cell).toContain('box width=((gutter - 2) * size / 13 + 2) flex-shrink=0 position="relative" padding-right=(7.83 * size / 13 + 2)');
  });
});
