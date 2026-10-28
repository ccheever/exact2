// A right-to-left table (T3 Code 7c3b3b3605ef, #22777): index.css pads a row's first and last cells by their inline
// start and end, and `text-align` left and right are the page's sides, so a right-to-left row swaps both.
import { describe, expect, test } from 'bun:test';
import { contractFns } from '../support/contract-fn-fixture';
import { value } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const fns = contractFns('src/timeline/markdown.contract') as Record<string, (...args: unknown[]) => unknown>;
const cell = appText('markdown.contract').split('component TableCell')[1]!;
const padding = (side: 'left' | 'right', first: boolean, last: boolean, rtl: boolean) =>
  value(new RegExp(`padding-${side}=\\((.+?)\\) padding-`).exec(cell)![1]!, { first, last, rtl });

describe('a right-to-left table', () => {
  test('its first cell, on the right, sits flush on its right; its last on its left', () => {
    expect([padding('left', true, false, false), padding('right', true, false, false)]).toEqual(['0px', '0.75rem']);
    expect([padding('left', true, false, true), padding('right', true, false, true)]).toEqual(['0.75rem', '0px']);
    expect([padding('left', false, true, true), padding('right', false, true, true)]).toEqual(['0px', '0.75rem']);
  });
  test('a `-:` column keeps its boxes on the right and a `:-` one on the left; an unaligned one follows the row', () => {
    expect(['right', 'left', 'center', 'start'].map(align => fns.cellJustify!(align, false))).toEqual(['flex-end', 'flex-start', 'center', 'flex-start']);
    expect(['right', 'left', 'center', 'start'].map(align => fns.cellJustify!(align, true))).toEqual(['flex-start', 'flex-end', 'center', 'flex-start']);
    expect(cell).toContain('justify=cellJustify(column.align, rtl)');
    expect(appText('markdown.contract')).toContain('rtl=(block.dir == "rtl")');
  });
});
