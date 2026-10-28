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
  test('the direction is the table\'s alone (MarkdownTable\'s `dir` on the `table`): its scroller and footer, Copy on the right, stay left to right', () => {
    expect([fns.blockDirection!('table', 'rtl'), fns.blockDirection!('paragraph', 'rtl'), fns.blockDirection!('item', 'ltr')]).toEqual(['ltr', 'rtl', 'ltr']);
    const table = appText('markdown.contract').split('component MarkdownTable')[1]!.split('\ncomponent ')[0]!;
    expect(table).toContain('role="table" direction=(block.dir == "rtl" ? "rtl" : "ltr")');
    expect(appText('markdown.contract').match(/direction=blockDirection\(block\.kind, block\.dir\)/g)?.length).toBe(3);
  });
});

// A long thread's rows kept re-rendering at their resize: each resize wrote the lane's width again, an equal write a
// commit all the same, which re-laid the row out and resized it again. The reference's container query settles at once.
describe('a row and a table cell settle after one measure', () => {
  const counting = (initial: Record<string, unknown>) => {
    const writes: string[] = [];
    return { writes, state: new Proxy({ ...initial }, { set(target, key, value) { writes.push(String(key)); (target as Record<string, unknown>)[String(key)] = value; return true; } }) };
  };
  test('a row writes its lane width once; a height change, or the same width again, writes nothing', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const row = contractComponent('timeline.contract', 'TimelineRow');
    const { writes, state } = counting(row.state());
    row.call('laneSized', state, {}, [720, 400]);
    row.call('laneSized', state, {}, [720, 412]);
    row.call('laneSized', state, {}, [720, 400]);
    expect(writes).toEqual(['laneWidth']);
    row.call('laneSized', state, {}, [640, 400]);
    expect(writes).toEqual(['laneWidth', 'laneWidth']);
  });
  test('a table cell reports its clipping once per width', async () => {
    const { contractComponent } = await import('../support/contract-action-fixture');
    const tableCell = contractComponent('markdown.contract', 'TableCell');
    const { writes, state } = counting(tableCell.state());
    const reports: unknown[][] = [];
    const scope = { header: false, nodePrefix: '0-0-0', clipped: (...args: unknown[]) => reports.push(args), cellClipped: (natural: number, width: number) => natural > width };
    tableCell.call('naturalSized', state, scope, [300, 20]);
    tableCell.call('cellSized', state, scope, [200, 20]);
    tableCell.call('cellSized', state, scope, [200, 46]);
    tableCell.call('naturalSized', state, scope, [300, 22]);
    expect(writes).toEqual(['naturalWidth', 'cellWidth']);
    expect(reports).toEqual([['b:0-0-0', true]]);
  });
});
