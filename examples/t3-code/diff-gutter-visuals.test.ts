// diff-gutter-visuals: the diff gutter's selection colour and the "+"'s place, as T3 Code (1e2ecbd975) paints them in the
// Code tab and the Diff panel (StyledDiffCodeView over @pierre/diffs; computed styles read over CDP, light and dark,
// stacked and split). The projections' marks for what sits under a selected line run in diff-review.test.ts and
// pages-pr-code.test.ts; #413's drags in diff-line-drag.test.ts and realinput-1010f-followups.test.ts.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const source = (file: string) => readFileSync(join(import.meta.dir, file), 'utf8');
const component = (text: string, name: string) => { const start = text.indexOf(`component ${name}\n`); const next = text.indexOf('\ncomponent ', start + 1); return text.slice(start, next < 0 ? undefined : next); };
const rows = source('diff-rows.contract');
const cell = component(rows, 'DiffCell');

/** What CDP read in the reference: light, dark. */
const REFERENCE = { row: ['#eff7fd', '#0f151a'], gutter: ['#f0f7fd', '#10151a'], number: ['#20649e', '#72b6ff'], bar: '#009fff', plusGlyph: ['#fcfcfc', '#0a0a0a'] };

describe('a selected line takes the reference\'s blue, not amber', () => {
  test('the selection\'s colours are the ones CDP read', () => {
    expect(rows).toContain(`fn diffSel(role: string): string = role == "row" ? "light-dark(${REFERENCE.row.join(', ')})" : role == "gutter" ? "light-dark(${REFERENCE.gutter.join(', ')})" : role == "number" ? "light-dark(${REFERENCE.number.join(', ')})" : "${REFERENCE.bar}"`);
    expect(rows).not.toMatch(/#fef3c7|#fde68a|#3a3112|#4a3d14/);
  });
  test('the row, the number\'s cell and the number take them; the selection\'s bar replaces the change bar', () => {
    expect(cell).toContain('position="relative" background-color=(selected ? diffSel("row") : tone == "deletion"');
    expect(cell).toContain('testId=`diff-line-${path}-${side}-${line}` aria-label=(line > 0 ? `Select line ${line}` : "") background-color=(selected ? diffSel("gutter") : tone == "deletion"');
    expect(cell).toContain('font-variant-numeric="tabular-nums" color=(selected ? diffSel("number") : tone == "deletion"');
    expect(cell).toContain('when selected\n          box position="absolute" left=0 top=0 bottom=0 width="0.25rem" background-color=diffSel("bar")');
    expect(cell).toContain('when tone == "addition" and not selected\n');
    expect(cell).toContain('when tone == "deletion" and not selected\n');
  });
  test('a comment under a selected line takes the row\'s colour and the bar, in both panels', () => {
    const row = component(rows, 'DiffRow');
    expect(row).toContain('column width="100%" position="relative" background-color=(item.selected ? diffSel("row") : "#00000000")');
    expect(row).toContain('DiffDraftCard(rangeLabel=item.label, text=draftText, selected=item.selected,');
    expect(row).toContain('when item.selected\n            box position="absolute" left=0 top=0 bottom=0 width="0.25rem" background-color=diffSel("bar")');
    const code = source('pages-pr-code.contract');
    expect(code).toContain('when item.kind == "pr-thread" or item.kind == "pr-pending" or item.kind == "pr-draft"\n        // What sits under a selected line');
    expect(code).toContain('PrdCodeDraft(rangeLabel=item.label, text=draftText, selected=item.selected,');
    for (const [file, name] of [['diff-comments.contract', 'DiffDraftCard'], ['pages-pr-threads.contract', 'PrdCodeDraft']]) {
      expect(component(source(file!), name!)).toContain(`background-color=(selected ? "light-dark(${REFERENCE.row.join(', ')})" : "light-dark(#fcfcfc, #0a0a0a)")`);
    }
  });
});

describe('the "+" sits at the number\'s right, as Pierre\'s gutter utility', () => {
  const plus = cell.slice(cell.indexOf('button press=comment'));
  test('a 20pt square of the modified blue, its 16pt plus in the code surface\'s colour', () => {
    expect(plus).toContain('top=0 width=20 height=20 padding=0 border-radius=4 border-width=0');
    expect(plus).toContain('background-color=diffSel("bar")');
    expect(plus).toContain(`svg width=16 height=16 viewBox="0 0 16 16" color="light-dark(${REFERENCE.plusGlyph.join(', ')})"`);
    expect(plus).toContain('path d="M8 3a.75.75 0 0 1 .75.75v3.5h3.5a.75.75 0 0 1 0 1.5h-3.5v3.5a.75.75 0 0 1-1.5 0v-3.5h-3.5a.75.75 0 0 1 0-1.5h3.5v-3.5A.75.75 0 0 1 8 3" fill="currentColor"');
  });
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
  test('#413\'s handlers and placement stay as they were', () => {
    expect(plus).toContain('button press=comment pointerdown=down(true) pointermove=move pointerup=up aria-label=`Comment on line ${line}`');
    expect(plus).toContain('opacity=(pin or (hovering and not pinned) ? 1 : 0)');
    expect(cell).toContain('when line > 0 and not open and (pin or not pinned or dragging)\n        button press=comment');
  });
});
