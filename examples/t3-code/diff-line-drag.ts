// The diff gutter's drags (realinput-1010f RF-3), as @pierre/diffs 1.3.0-beta.10's InteractionManager (MIT) runs them for
// T3 Code 1e2ecbd975 (see LICENSE-T3): a press on a line number starts a selection that follows the pointer over the file's
// lines (a Shift-press extends the current selection; a press on the one line already selected clears it on release unless
// the pointer moves first), and a press on the gutter's "+" starts a range from the current selection's top (or its own line)
// whose end follows the pointer. The release ends the gesture: the Code tab opens its draft on what it ended on
// (PullRequestCodeTab's onLineSelectionEnd and onGutterUtilityClick), the thread's Diff panel only on the "+"'s range
// (AnnotatableCodeView's onGutterUtilityClick). DiffCell (diff-rows.contract) reads the line under the pointer with
// `elementFromPoint` over the cells' ids (`lineCellId`); the two panels keep the selection and the drag.
import type { SelectedLineRange, SelectionSide } from './diff-comments';

export type LinePoint = { line: number; side: SelectionSide };
/** startLineSelectionFromPointerDown's `selecting` and `pendingSingleLineUnselect`, startGutterSelectionFromPointerDown's `gutterSelecting`. */
export type LineDrag = { path: string; mode: 'selecting' | 'pending-unselect' | 'gutter'; anchor: LinePoint; current: LinePoint };
export type FileSelection = { path: string; range: SelectedLineRange } | null;
/** A point's row in its file's diff (Pierre's line index), or null where the file draws no such line. */
export type RowIndex = (point: LinePoint) => number | null;
export type DragStep = { drag: LineDrag | null; selection: FileSelection };

const rangeOf = (anchor: LinePoint, current: LinePoint): SelectedLineRange => ({ start: anchor.line, side: anchor.side, end: current.line, endSide: current.side });
const samePoint = (a: LinePoint, b: LinePoint) => a.line === b.line && a.side === b.side;
const sideOf = (value: string): SelectionSide => (value === 'deletions' ? 'deletions' : 'additions');

/** The id DiffCell gives its row, so `elementFromPoint` names the line under the pointer: `dl:<side>:<line>:<path>`. */
export const lineCellId = (side: string, line: number, path: string): string => `dl:${sideOf(side)}:${line}:${path}`;
/** The file and line a cell id names, or null for any other id (a file header, a comment card, the list). */
export function parseLineCellId(id: string): { path: string; at: LinePoint } | null {
  const match = /^dl:(additions|deletions):(\d+):(.+)$/.exec(id);
  if (!match) return null;
  const line = Number(match[2]);
  return line > 0 ? { path: match[3]!, at: { line, side: sideOf(match[1]!) } } : null;
}

/** The selection's top and bottom by row (selectionEnds). */
function ends(range: SelectedLineRange, index: RowIndex): { top: LinePoint; bottom: LinePoint } | null {
  const start = { line: range.start, side: range.side }, end = { line: range.end, side: range.endSide ?? range.side };
  const from = index(start), to = index(end);
  if (from === null || to === null) return null;
  return from > to ? { top: end, bottom: start } : { top: start, bottom: end };
}

/** A primary press on a line number. */
export function pressLine(selection: FileSelection, path: string, at: LinePoint, shift: boolean, index: RowIndex): DragStep {
  const current = selection?.path === path ? selection.range : null;
  if (shift && current) {
    // The end of the selection the press is beyond stays put: below its start it keeps the start, above it the end.
    const start = index({ line: current.start, side: current.side }), end = index({ line: current.end, side: current.endSide ?? current.side }), row = index(at);
    if (start === null || end === null || row === null) return { drag: null, selection };
    const useStart = start <= end ? row >= start : row <= end;
    const anchor = useStart ? { line: current.start, side: current.side } : { line: current.end, side: current.endSide ?? current.side };
    return { drag: { path, mode: 'selecting', anchor, current: at }, selection: { path, range: rangeOf(anchor, at) } };
  }
  if (current && current.start === at.line && current.end === at.line) return { drag: { path, mode: 'pending-unselect', anchor: at, current: at }, selection };
  return { drag: { path, mode: 'selecting', anchor: at, current: at }, selection: { path, range: rangeOf(at, at) } };
}

/** A primary press on the gutter's "+": from the selection's top to its bottom when the file has one, else its own line. */
export function pressGutter(selection: FileSelection, path: string, at: LinePoint, index: RowIndex): DragStep {
  const both = selection?.path === path ? ends(selection.range, index) : null;
  const anchor = both?.top ?? at, current = both?.bottom ?? at;
  return { drag: { path, mode: 'gutter', anchor, current }, selection: { path, range: rangeOf(anchor, current) } };
}

/** The pointer over a line while the button is down; a line of another file changes nothing. */
export function dragTo(drag: LineDrag | null, selection: FileSelection, path: string, at: LinePoint): DragStep {
  if (!drag || path !== drag.path || samePoint(drag.current, at)) return { drag, selection };
  const mode = drag.mode === 'pending-unselect' ? 'selecting' : drag.mode;
  return { drag: { ...drag, mode, current: at }, selection: { path, range: rangeOf(drag.anchor, at) } };
}

/**
 * The release: what stays selected, the "+"'s range (onGutterUtilityClick) and the selection the gesture ended on
 * (onLineSelectionEnd, also after a "+" drag). A press on the one selected line that never moved clears it.
 */
export function releaseDrag(drag: LineDrag | null, selection: FileSelection): { selection: FileSelection; gutter: SelectedLineRange | null; ended: SelectedLineRange | null } {
  if (!drag) return { selection, gutter: null, ended: null };
  if (drag.mode === 'pending-unselect') return { selection: null, gutter: null, ended: null };
  const ended = selection?.path === drag.path ? selection.range : null;
  return { selection, gutter: drag.mode === 'gutter' ? rangeOf(drag.anchor, drag.current) : null, ended };
}
