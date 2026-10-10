// diff-gutter-selection-followups: the diff gutter's drag selects no text (GS-1), and the Files surface's selected lines
// take the reference's blue (GS-2), as T3 Code (1e2ecbd975) draws them over @pierre/diffs (computed styles read over CDP,
// light and dark). #413's drags and #416's colours run in realinput-1010f-followups.test.ts and diff-gutter-visuals.test.ts.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const source = (file: string) => readFileSync(join(import.meta.dir, file), 'utf8');
const component = (text: string, name: string) => { const start = text.indexOf(`component ${name}\n`); const next = text.indexOf('\ncomponent ', start + 1); return text.slice(start, next < 0 ? undefined : next); };
const rows = source('diff-rows.contract');
const cell = component(rows, 'DiffCell');
const number = component(rows, 'DiffNumber');
const row = component(rows, 'DiffRow');
/** One `when item.kind == "<kind>"` branch of DiffRow. */
const branch = (kind: string) => { const start = row.indexOf(`when item.kind == "${kind}"\n`); const next = row.indexOf('\n      when ', start + 1); return row.slice(start, next < 0 ? undefined : next); };

/** CDP (1e2ecbd975), light then dark. */
const REFERENCE = {
  // The Diff panel's [data-column-number] and its number content; the code row ([data-line]) selects.
  diff: { numberUserSelect: 'none', lineUserSelect: 'auto' },
  // FilePreviewPanel's File: the selected number cell and number; no row tint, no bar.
  files: { cell: ['#d5e6ff', '#254566'], number: ['#20649e', '#72b6ff'] },
};

describe('GS-1: a drag that starts on the gutter selects no text', () => {
  test('the number is DiffNumber\'s: no pointer (the press is the gutter\'s), no text selection, as [data-column-number]', () => {
    expect(number).toContain('text number position="absolute" left=left top=0');
    expect(number).toContain(`pointer-events="none" user-select="${REFERENCE.diff.numberUserSelect}" aria-hidden=true`);
    // The cell keeps one paragraph, the code (its runs are inline), which a drag still selects: the cell authors no
    // `user-select`, so it is the reference row's `auto`.
    expect(cell.match(/\n {6}text /g)?.length).toBe(1);
    expect(cell).toContain('\n          text segment.text color=synColor(segment.syntax)');
    expect(cell).toContain('text flex=1 min-width=0');
    expect(/user-select="([a-z]+)"/.exec(cell)?.[1] ?? 'auto').toBe(REFERENCE.diff.lineUserSelect);
    expect(cell).not.toContain('text number');
  });
  test('the number sits where it did: the gutter cell\'s width and inset, its colours unchanged', () => {
    const box = /box (width=\(\(gutter - 2\) \* size \/ 13 \+ 2\)) flex-shrink=0 position="relative" (padding-right=\(7\.83 \* size \/ 13 \+ 2\)) box-sizing="border-box" min-height="20px" pointerdown=down\(false\)/.exec(cell);
    expect(box).not.toBeNull();
    expect(number).toContain(`${box![1]} ${box![2]} box-sizing="border-box"`);
    expect(number).toContain('font-size=size letter-spacing=(-0.21 * size / 13) line-height="20px" text-align="right" font-variant-numeric="tabular-nums"');
    expect(number).toContain('color=(selected ? diffSel("number") : tone == "deletion" ? diffInk(scheme, "del") : tone == "addition" ? diffInk(scheme, "add") : "light-dark(#565656, #9d9d9d)")');
  });
  test('every row draws its numbers after all of its code, so a copy across the list\'s rows reads the code where the host placed it', () => {
    const line = branch('line'), split = branch('split');
    expect(line).toContain('row width="100%" align-items="stretch" position="relative"');
    expect(line.indexOf('DiffNumber(number=item.number, tone=item.tone, selected=item.selected,')).toBeGreaterThan(line.lastIndexOf('DiffCell('));
    expect(line).toContain('left="0%")');
    expect(split).toContain('row width="100%" align-items="stretch" position="relative"');
    const firstNumber = split.indexOf('DiffNumber(');
    expect(firstNumber).toBeGreaterThan(split.lastIndexOf('DiffCell('));
    expect(split).toContain('DiffNumber(number=item.leftNumber, tone=item.leftTone, selected=item.leftSelected, gutter=item.gutter, size=size, scheme=scheme, left="0%")');
    expect(split).toContain('DiffNumber(number=item.rightNumber, tone=item.rightTone, selected=item.rightSelected, gutter=item.gutter, size=size, scheme=scheme, left="50%")');
    // The two cells split the row evenly, so the additions side's gutter starts at 50%.
    expect(cell).toContain('row id=`dl:${side}:${line}:${path}` flex=1 min-width=0');
    expect(row).not.toContain('number=item.leftNumber, segments');
  });
  test('#413\'s press, drag and "+" are unchanged', () => {
    expect(cell).toContain('pointerdown=down(false) pointermove=move pointerup=up testId=`diff-line-${path}-${side}-${line}` aria-label=(line > 0 ? `Select line ${line}` : "")');
    expect(cell).toContain('button press=comment pointerdown=down(true) pointermove=move pointerup=up aria-label=`Comment on line ${line}`');
    expect(cell).toContain('let hit = match elementFromPoint(e.clientX, e.clientY) { case some(id) => id, case none => "" }');
  });
});

describe('GS-2: the Files surface\'s selected lines in the reference\'s blue', () => {
  const files = source('r4-surfaces-files.contract');
  const code = component(files, 'R4CodeRow');
  test('the number\'s cell and the number take the CDP colours; the number\'s is the diff\'s', () => {
    expect(files).toContain('use diffSel from "./diff-rows.contract"');
    expect(code).toContain(`background-color=(line.selected ? "light-dark(${REFERENCE.files.cell.join(', ')})" : "#00000000") color=(line.selected ? diffSel("number") : "light-dark(#565656, #9d9d9d)")`);
    expect(rows).toContain(`role == "number" ? "light-dark(${REFERENCE.files.number.join(', ')})"`);
    // The cell spans its line's height, as the grid's number cell does under a wrapped line.
    expect(code).toContain('padding-right=9.83 box-sizing="border-box" align-self="stretch"');
  });
  test('no row tint and no bar: the code and the comment card under a selected line stay the surface\'s', () => {
    expect(code).toContain('row width="100%" align-items="flex-start" hover=hover position="relative"\n');
    expect(code).not.toContain('line.selected ? "light-dark(#fef3c7');
    expect(code).not.toContain('diffSel("bar")');
    expect(files).toContain('DiffDraftCard(rangeLabel=line.label, text=draftText, selected=false,');
  });
  test('no amber is left where a selected line is painted', () => {
    // The sites that painted a selected line amber before #416 and this task: the diff's row, gutter and number
    // (diffSel, DiffCell, DiffNumber) and the Files surface's row (R4CodeRow). Amber elsewhere (a warning) is not this.
    const sites = { diffSel: /\nfn diffSel\(.*/.exec(rows)?.[0] ?? '', DiffCell: cell, DiffNumber: number, R4CodeRow: code };
    for (const [site, text] of Object.entries(sites)) {
      expect(text.length).toBeGreaterThan(0);
      expect([site, /#fef3c7|#fde68a|#3a3112|#4a3d14/.test(text)]).toEqual([site, false]);
    }
  });
});
