// Copied from T3 Code 2359643cff (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts).
// This part holds link detection over the viewport rows. The reference keeps all of surface.ts
// in one file.
// Changes from the reference:
// - split out of surface.ts for the 1,500-line cap.
// - the import path of terminal-links follows this tree.
// - `isSameTerminalLink` is exported for surface.ts.
import { collectWrappedTerminalLinkLine, extractTerminalLinks } from "../../../src/terminal/terminal-links";
import { GHOSTTY_CELL_WIDE, type GhosttyCell, type GhosttySnapshot } from "./core";
import type { GhosttyCellRange } from "./renderer";

function terminalCellText(cell: GhosttyCell): string {
  if (cell.wide === GHOSTTY_CELL_WIDE.spacerTail || cell.wide === GHOSTTY_CELL_WIDE.spacerHead) {
    return "";
  }
  return cell.text || " ";
}

function terminalRowText(row: GhosttySnapshot["rowData"][number], trimRight: boolean): string {
  const text = row.cells.map(terminalCellText).join("");
  return trimRight ? text.trimEnd() : text;
}

function terminalColumnOffset(row: GhosttySnapshot["rowData"][number], column: number): number {
  let offset = 0;
  for (let cellIndex = 0; cellIndex < column; cellIndex += 1) {
    const cell = row.cells[cellIndex];
    offset += cell ? terminalCellText(cell).length : 1;
  }
  return offset;
}

export interface TerminalLinkWithRange {
  readonly text: string;
  readonly range: GhosttyCellRange;
}

export function isSameTerminalLink(
  left: TerminalLinkWithRange,
  right: TerminalLinkWithRange | null,
): boolean {
  return (
    right?.text === left.text &&
    right.range.start.x === left.range.start.x &&
    right.range.start.y === left.range.start.y &&
    right.range.end.x === left.range.end.x &&
    right.range.end.y === left.range.end.y
  );
}

function terminalColumnAtOffset(row: GhosttySnapshot["rowData"][number], offset: number): number {
  for (let column = 0; column < row.cells.length; column += 1) {
    const nextOffset = terminalColumnOffset(row, column + 1);
    if (offset < nextOffset) return column;
  }
  return Math.max(0, row.cells.length - 1);
}

export function terminalLinkAtPositionWithRange(
  rows: GhosttySnapshot["rowData"],
  rowIndex: number,
  column: number,
): TerminalLinkWithRange | null {
  const wrappedLine = collectWrappedTerminalLinkLine(rowIndex + 1, (index) => {
    const row = rows[index];
    if (!row) return null;
    return {
      isWrapped: row.isWrapContinuation,
      translateToString: (trimRight = false) => terminalRowText(row, trimRight),
    };
  });
  if (!wrappedLine) return null;
  // Only viewport rows are available: a wrapped line whose head scrolled above
  // the viewport would resolve a truncated match into a wrong link.
  const firstSegment = wrappedLine.segments[0];
  if (firstSegment && rows[firstSegment.bufferLineNumber - 1]?.isWrapContinuation) {
    return null;
  }
  const segment = wrappedLine.segments.find((value) => value.bufferLineNumber === rowIndex + 1);
  const row = rows[rowIndex];
  if (!segment || !row) return null;
  const lastSegment = wrappedLine.segments.at(-1);
  const lastRow = lastSegment ? rows[lastSegment.bufferLineNumber - 1] : undefined;
  // Ghostty's soft-wrap flag is authoritative: when the last collected row
  // still wraps onward, its continuation is outside the viewport.
  const continuesBelowViewport = lastRow !== undefined && lastRow.wrapsToNext;
  const hitColumn = row.cells[column]?.wide === GHOSTTY_CELL_WIDE.spacerTail ? column - 1 : column;
  const offset = segment.startIndex + terminalColumnOffset(row, hitColumn);
  for (const match of extractTerminalLinks(wrappedLine.text)) {
    if (offset >= match.start && offset < match.end) {
      // A truncated tail must not activate as a complete link.
      if (match.end === wrappedLine.text.length && continuesBelowViewport) return null;
      const startSegment = wrappedLine.segments.find(
        (value) => match.start >= value.startIndex && match.start < value.endIndex,
      );
      const endSegment = wrappedLine.segments.find(
        (value) => match.end - 1 >= value.startIndex && match.end - 1 < value.endIndex,
      );
      const startRow = startSegment ? rows[startSegment.bufferLineNumber - 1] : undefined;
      const endRow = endSegment ? rows[endSegment.bufferLineNumber - 1] : undefined;
      if (!startSegment || !endSegment || !startRow || !endRow) return null;
      const endColumn = terminalColumnAtOffset(endRow, match.end - 1 - endSegment.startIndex);
      return {
        text: match.text,
        range: {
          start: {
            x: terminalColumnAtOffset(startRow, match.start - startSegment.startIndex),
            y: startSegment.bufferLineNumber - 1,
          },
          end: {
            x:
              endRow.cells[endColumn + 1]?.wide === GHOSTTY_CELL_WIDE.spacerTail
                ? endColumn + 1
                : endColumn,
            y: endSegment.bufferLineNumber - 1,
          },
        },
      };
    }
  }
  return null;
}
