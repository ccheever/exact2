// realinput-1010f RF-3: the gutter's drags (diff-line-drag.ts) against @pierre/diffs' InteractionManager, as T3 Code
// 1e2ecbd975 drives it (over CDP in the reference: a drag on the numbers from line 3 to line 5 paints 3–5 and, in the Code
// tab, opens the draft; the same from the "+"; a click on a number opens the draft on that line).
import { describe, expect, test } from 'bun:test';
import { dragTo, gutterClick, lineCellId, parseLineCellId, pressGutter, pressLine, releaseDrag, utilityPlacement, type FileSelection, type LinePoint, type RowIndex } from './diff-line-drag';

// A stacked file: context 2, 3, 4, deleted 5, added 5, context 6 (rows 0..5), as docs/usage.md in #168.
const rows: LinePoint[] = [
  { line: 2, side: 'additions' }, { line: 3, side: 'additions' }, { line: 4, side: 'additions' },
  { line: 5, side: 'deletions' }, { line: 5, side: 'additions' }, { line: 6, side: 'additions' },
];
const index: RowIndex = point => { const at = rows.findIndex(row => row.line === point.line && row.side === point.side); return at < 0 ? null : at; };
const add = (line: number): LinePoint => ({ line, side: 'additions' });
const del = (line: number): LinePoint => ({ line, side: 'deletions' });
const path = 'docs/usage.md';

describe('the line number drag (startLineSelectionFromPointerDown, handleDocumentPointerMove/Up)', () => {
  test('a press selects its line, each line the pointer reaches moves the end, the release ends on the range', () => {
    let step = pressLine(null, path, add(3), false, index);
    expect(step.selection).toEqual({ path, range: { start: 3, side: 'additions', end: 3, endSide: 'additions' } });
    step = dragTo(step.drag, step.selection, path, add(4));
    step = dragTo(step.drag, step.selection, path, del(5));
    step = dragTo(step.drag, step.selection, path, add(5));
    expect(step.selection).toEqual({ path, range: { start: 3, side: 'additions', end: 5, endSide: 'additions' } });
    expect(releaseDrag(step.drag, step.selection)).toEqual({ selection: step.selection, gutter: null, ended: { start: 3, side: 'additions', end: 5, endSide: 'additions' } });
  });
  test('a click (no move) ends on its one line', () => {
    const step = pressLine(null, path, add(3), false, index);
    expect(releaseDrag(step.drag, step.selection).ended).toEqual({ start: 3, side: 'additions', end: 3, endSide: 'additions' });
  });
  test('a line of another file changes nothing; the pointer back on the same line is no step', () => {
    const step = pressLine(null, path, add(3), false, index);
    expect(dragTo(step.drag, step.selection, 'docs/catalog.md', add(5))).toEqual(step);
    expect(dragTo(step.drag, step.selection, path, add(3))).toEqual(step);
  });
  test('a press on the one selected line clears it on release, unless the pointer moves first', () => {
    const one: FileSelection = { path, range: { start: 4, side: 'additions', end: 4, endSide: 'additions' } };
    const step = pressLine(one, path, add(4), false, index);
    expect(step.drag?.mode).toBe('pending-unselect');
    expect(step.selection).toBe(one);
    expect(releaseDrag(step.drag, step.selection)).toEqual({ selection: null, gutter: null, ended: null });
    const moved = dragTo(step.drag, step.selection, path, add(6));
    expect([moved.drag?.mode, moved.selection?.range]).toEqual(['selecting', { start: 4, side: 'additions', end: 6, endSide: 'additions' }]);
  });
  test('Shift extends the selection from the end the press is beyond', () => {
    const range: FileSelection = { path, range: { start: 3, side: 'additions', end: 4, endSide: 'additions' } };
    expect(pressLine(range, path, add(6), true, index).selection?.range).toEqual({ start: 3, side: 'additions', end: 6, endSide: 'additions' });
    expect(pressLine(range, path, add(2), true, index).selection?.range).toEqual({ start: 4, side: 'additions', end: 2, endSide: 'additions' });
    // Another file's selection: a plain press there.
    expect(pressLine(range, 'docs/catalog.md', add(2), true, index).selection).toEqual({ path: 'docs/catalog.md', range: { start: 2, side: 'additions', end: 2, endSide: 'additions' } });
  });
});

describe('the "+" drag (startGutterSelectionFromPointerDown)', () => {
  test('from its own line to the line the pointer leaves it on: the range the "+" comments on', () => {
    let step = pressGutter(null, path, add(3), index);
    step = dragTo(step.drag, step.selection, path, add(5));
    expect(step.selection?.range).toEqual({ start: 3, side: 'additions', end: 5, endSide: 'additions' });
    const end = releaseDrag(step.drag, step.selection);
    expect([end.gutter, end.ended]).toEqual([{ start: 3, side: 'additions', end: 5, endSide: 'additions' }, { start: 3, side: 'additions', end: 5, endSide: 'additions' }]);
  });
  test('with a selection in the file it starts at the selection\'s top and ends at its bottom (a click comments on it)', () => {
    const upward: FileSelection = { path, range: { start: 6, side: 'additions', end: 3, endSide: 'additions' } };
    const step = pressGutter(upward, path, add(4), index);
    expect(releaseDrag(step.drag, step.selection).gutter).toEqual({ start: 3, side: 'additions', end: 6, endSide: 'additions' });
  });
});

describe('where the "+" sits (placeUtility, placeUtilityFromSelection)', () => {
  test('no selection: it follows the hover; a selection pins it to its bottom line, which moves as a drag moves the end', () => {
    expect(utilityPlacement(null, index).pinned).toBe(false);
    let step = pressLine(null, path, add(3), false, index);
    let placed = utilityPlacement(step.selection!.range, index);
    expect([placed.pinned, rows.filter(row => placed.holds(row.side, row.line, false)).map(row => `${row.side}:${row.line}`)]).toEqual([true, ['additions:3']]);
    step = dragTo(step.drag, step.selection, path, add(5));
    placed = utilityPlacement(step.selection!.range, index);
    expect(rows.filter(row => placed.holds(row.side, row.line, false))).toEqual([add(5)]);
    // Dragged upward from 5 to 3, the bottom is still 5.
    placed = utilityPlacement({ start: 5, side: 'additions', end: 3, endSide: 'additions' }, index);
    expect(rows.filter(row => placed.holds(row.side, row.line, false))).toEqual([add(5)]);
  });
  test('stacked rows match by row, split rows by row and side; a bottom the file does not draw hides it', () => {
    // A context line's row is the same from either side: stacked, its one cell holds the "+" whichever side the range ends on.
    const shared: RowIndex = point => (point.line === 4 && point.side === 'deletions') || (point.line === 4 && point.side === 'additions') ? 2 : index(point);
    const placed = utilityPlacement({ start: 3, side: 'additions', end: 4, endSide: 'deletions' }, shared);
    expect([placed.holds('additions', 4, false), placed.holds('additions', 4, true), placed.holds('deletions', 4, true)]).toEqual([true, false, true]);
    const gone = utilityPlacement({ start: 3, side: 'additions', end: 40, endSide: 'additions' }, index);
    expect([gone.pinned, rows.some(row => gone.holds(row.side, row.line, false))]).toEqual([true, false]);
  });
  test('a "+" press no gesture carried comments on what its press and release would: the selection, else its own line', () => {
    expect(gutterClick(null, path, add(4), index)).toEqual({ start: 4, side: 'additions', end: 4, endSide: 'additions' });
    const upward: FileSelection = { path, range: { start: 5, side: 'additions', end: 3, endSide: 'additions' } };
    expect(gutterClick(upward, path, add(6), index)).toEqual({ start: 3, side: 'additions', end: 5, endSide: 'additions' });
    expect(gutterClick(upward, 'docs/catalog.md', add(6), index)).toEqual({ start: 6, side: 'additions', end: 6, endSide: 'additions' });
  });
});

test('cell ids name the side, the line and the path; other ids name nothing', () => {
  expect(lineCellId('deletions', 5, 'docs/a:b.md')).toBe('dl:deletions:5:docs/a:b.md');
  expect(parseLineCellId('dl:deletions:5:docs/a:b.md')).toEqual({ path: 'docs/a:b.md', at: { line: 5, side: 'deletions' } });
  expect(parseLineCellId('dl:additions:0:docs/usage.md')).toBeNull();
  expect(parseLineCellId('pull-request-code-file-docs/usage.md')).toBeNull();
});
