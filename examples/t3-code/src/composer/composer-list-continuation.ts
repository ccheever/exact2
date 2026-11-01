// composer-list-continuation.ts at T3 Code v0.1.6 (MIT; LICENSE-T3): Enter, Tab and ⇧Tab in a
// Markdown list typed as text. The plain composer uses it whole; the rich composer's own list
// gesture (composer-markdown-edit.ts) renumbers with it.
import type { ComposerRange } from './composer-markdown';
import type { ComposerEdit } from './composer-markdown-edit';

type ListMarker =
  | { kind: 'ordered'; indent: string; numberText: string; delimiter: '.' | ')'; space: string }
  | { kind: 'task'; indent: string; space: string }
  | { kind: 'bullet'; indent: string; bullet: string; space: string };
export type OrderedMarker = Extract<ListMarker, { kind: 'ordered' }>;

/** A list line's marker, where it ends, and the column a nested item must reach to belong to it. */
type ParsedListMarker = { marker: ListMarker; markerEnd: number; contentColumn: number };

export function parseListMarker(line: string): ParsedListMarker | null {
  const indent = /^[ \t]*/.exec(line)?.[0] ?? '';
  const rest = line.slice(indent.length);
  const ordered = /^(\d+)([.)])((?:[ \t]+)|\s*$)/.exec(rest);
  if (ordered?.[1] !== undefined && ordered[2] !== undefined) {
    const space = ordered[3] ?? '';
    return { marker: { kind: 'ordered', indent, numberText: ordered[1], delimiter: ordered[2] === ')' ? ')' : '.', space },
      markerEnd: indent.length + ordered[0].length, contentColumn: contentColumn(indent, ordered[1].length + 1, space) };
  }
  // The checkbox is item content, so a task's content starts after "- ".
  const task = /^-(\s)\[[ xX]\]((?:[ \t]+)|\s*$)/.exec(rest);
  if (task?.[1] !== undefined) return { marker: { kind: 'task', indent, space: task[2] ?? '' }, markerEnd: indent.length + task[0].length, contentColumn: contentColumn(indent, 1, task[1]) };
  const bullet = /^([-*+])((?:[ \t]+)|\s*$)/.exec(rest);
  if (bullet?.[1] !== undefined) {
    const space = bullet[2] ?? '';
    return { marker: { kind: 'bullet', indent, bullet: bullet[1], space }, markerEnd: indent.length + bullet[0].length, contentColumn: contentColumn(indent, 1, space) };
  }
  return null;
}

// CommonMark content follows 1-4 columns of padding; wider padding is code, so the content
// column sits one column past the marker.
function contentColumn(indent: string, markerWidth: number, padding: string): number {
  const markerEnd = indentWidth(indent) + markerWidth;
  const space = indentWidth(padding, markerEnd);
  return markerEnd + (space >= 1 && space <= 4 ? space : 1);
}

export function nextOrderedMarkerText(marker: string): string {
  const number = marker.slice(0, -1), value = Number.parseInt(number, 10);
  return `${Number.isSafeInteger(value) ? String(value + 1).padStart(number.length, '0') : number}${marker.slice(-1)}`;
}

function nextMarkerText(marker: ListMarker): string {
  if (marker.kind === 'ordered') return `${marker.indent}${nextOrderedMarkerText(`${marker.numberText}${marker.delimiter}`)} `;
  return marker.kind === 'task' ? `${marker.indent}- [ ] ` : `${marker.indent}${marker.bullet} `;
}

// Markdown tabs advance to the next four-column stop.
function indentWidth(line: string, fromColumn = 0): number {
  let column = fromColumn;
  for (const char of line) {
    if (char === ' ') column += 1;
    else if (char === '\t') column += 4 - (column % 4);
    else break;
  }
  return column - fromColumn;
}

/** The following siblings of an ordered item renumbered after a new item is inserted below it:
 *  the replacement of `afterLineEnd..end`, or null when every number already follows. A sibling
 *  is an item at the same indent with the same delimiter; a blank line, plain text, another
 *  delimiter or a shallower line ends the run, and deeper lines are skipped. */
export function renumberFollowingItems(value: string, afterLineEnd: number, marker: OrderedMarker, chips: readonly ComposerRange[]): { end: number; text: string } | null {
  const baseWidth = indentWidth(marker.indent), parts: string[] = [];
  let expected = Number.parseInt(marker.numberText, 10) + 2, position = afterLineEnd, copiedUntil = afterLineEnd;
  while (position < value.length && Number.isSafeInteger(expected)) {
    const lineStart = position + 1, nextBreak = value.indexOf('\n', lineStart), lineEnd = nextBreak === -1 ? value.length : nextBreak;
    position = lineEnd;
    // A newline inside a chip does not start a list item.
    if (chips.some(chip => chip.start < lineStart && chip.end > lineStart)) continue;
    const line = value.slice(lineStart, lineEnd), width = indentWidth(line);
    if (line.trim() === '' || width < baseWidth) break;
    if (width > baseWidth) continue;
    const parsed = parseListMarker(line);
    if (parsed?.marker.kind !== 'ordered' || parsed.marker.delimiter !== marker.delimiter) break;
    const current = parsed.marker, numberStart = lineStart + current.indent.length, numberEnd = numberStart + current.numberText.length;
    const replacement = current.numberText.startsWith('0') ? String(expected).padStart(current.numberText.length, '0') : String(expected);
    if (replacement !== current.numberText) {
      parts.push(value.slice(copiedUntil, numberStart), replacement);
      copiedUntil = numberEnd;
    }
    expected += 1;
  }
  return parts.length === 0 ? null : { end: copiedUntil, text: parts.join('') };
}

function currentLine(value: string, cursor: number): ComposerRange & { text: string } {
  const start = value.lastIndexOf('\n', cursor - 1) + 1, next = value.indexOf('\n', cursor), end = next === -1 ? value.length : next;
  return { start, end, text: value.slice(start, end) };
}
/** The list items directly above a line, nearest first, until a line that is not one. */
function* listMarkersAbove(value: string, lineStart: number, chips: readonly ComposerRange[]): Generator<ParsedListMarker> {
  const sorted = [...chips].sort((a, b) => a.start - b.start);
  let chipIndex = sorted.length - 1;
  while (lineStart > 0) {
    const lineEnd = lineStart - 1;
    lineStart = value.lastIndexOf('\n', lineEnd - 1) + 1;
    // A newline inside a chip does not start a list item.
    for (let chip = sorted[chipIndex]; chip; chip = sorted[chipIndex]) {
      if (chip.start >= lineStart) chipIndex -= 1;
      else if (chip.end > lineStart) lineStart = value.lastIndexOf('\n', chip.start - 1) + 1;
      else break;
    }
    const parsed = parseListMarker(value.slice(lineStart, lineEnd));
    if (!parsed) return;
    yield parsed;
  }
}

function parentMarker(value: string, lineStart: number, marker: ListMarker, chips: readonly ComposerRange[]): ListMarker | null {
  const width = indentWidth(marker.indent);
  for (const above of listMarkersAbove(value, lineStart, chips)) if (indentWidth(above.marker.indent) < width) return above.marker;
  return null;
}

function nestedNumber(marker: ListMarker, previousChild: ListMarker | undefined): { replacedLength: number; text: string } {
  if (marker.kind !== 'ordered') return { replacedLength: 0, text: '' };
  const continues = previousChild?.kind === 'ordered' && previousChild.delimiter === marker.delimiter;
  return { replacedLength: marker.numberText.length,
    text: continues ? nextOrderedMarkerText(`${previousChild.numberText}${marker.delimiter}`).slice(0, -1) : '1' };
}

const insideChip = (chips: readonly ComposerRange[], cursor: number) => chips.some(chip => cursor > chip.start && cursor < chip.end);
const listEdit = (start: number, end: number, text: string, caret: number): ComposerEdit => ({ start, end, text, selection: { start: caret, end: caret } });

/** listContinuationForEnter: the next item, the empty item lifted to its parent's next item (or removed at the top
 *  level), or null for an ordinary newline. A marker typed without its space ("2." alone) is text, so Enter keeps it. */
export function listContinuationForEnter(value: string, cursor: number, chips: readonly ComposerRange[]): ComposerEdit | null {
  if (!Number.isInteger(cursor) || cursor < 0 || cursor > value.length) return null;
  const line = currentLine(value, cursor), parsed = parseListMarker(line.text);
  if (!parsed || parsed.marker.space === '') return null;
  const markerEnd = line.start + parsed.markerEnd;
  if (cursor < markerEnd || insideChip(chips, cursor)) return null;
  if (value.slice(markerEnd, line.end).trim() === '') {
    const parent = parentMarker(value, line.start, parsed.marker, chips);
    const replacement = parent ? nextMarkerText(parent) : '', caret = line.start + replacement.length, end = Math.max(cursor, markerEnd);
    const renumbered = parent?.kind === 'ordered' ? renumberFollowingItems(value, line.end, parent, chips) : null;
    if (renumbered) return listEdit(line.start, renumbered.end, replacement + value.slice(end, line.end) + renumbered.text, caret);
    return listEdit(line.start, end, replacement, caret);
  }
  const insertion = `\n${nextMarkerText(parsed.marker)}`, caret = cursor + insertion.length;
  if (parsed.marker.kind === 'ordered') {
    const renumbered = renumberFollowingItems(value, line.end, parsed.marker, chips);
    if (renumbered) return listEdit(cursor, renumbered.end, insertion + value.slice(cursor, line.end) + renumbered.text, caret);
  }
  return listEdit(cursor, cursor, insertion, caret);
}

/** listIndentForTab: a list line nests under the item above it, at that item's content column (or under its last
 *  child), and an ordered item restarts at 1 or continues that child's numbering; with no item above, two spaces.
 *  The caret stays on the line's text. */
export function listIndentForTab(value: string, start: number, end: number, chips: readonly ComposerRange[]): ComposerEdit | null {
  if (!Number.isInteger(start) || start !== end || start < 0 || start > value.length) return null;
  const line = currentLine(value, start), parsed = parseListMarker(line.text);
  if (!parsed || insideChip(chips, start)) return null;
  const { marker } = parsed, width = indentWidth(marker.indent);
  let previousChild: ListMarker | undefined;
  for (const above of listMarkersAbove(value, line.start, chips)) {
    const aboveWidth = indentWidth(above.marker.indent);
    if (aboveWidth < width) break;
    if (aboveWidth > width) {
      if (!previousChild || aboveWidth < indentWidth(previousChild.indent)) previousChild = above.marker;
      continue;
    }
    const indent = previousChild?.indent ?? above.marker.indent + ' '.repeat(above.contentColumn - aboveWidth);
    const number = nestedNumber(marker, previousChild);
    const replacedEnd = line.start + marker.indent.length + number.replacedLength, replacement = indent + number.text;
    return listEdit(line.start, replacedEnd, replacement, start < replacedEnd ? line.start + indent.length : start - replacedEnd + line.start + replacement.length);
  }
  return listEdit(line.start, line.start, '  ', start + 2);
}

/** listOutdentForTab: a nested list line moves out to its parent item's indent, or loses one step of Tab's two-space
 *  fallback with no parent above; an ordered item under an ordered parent of its delimiter takes the parent's next
 *  number, and the items after it follow. Null for a top-level line, which leaves ⇧Tab to plan mode or the focus. */
export function listOutdentForTab(value: string, start: number, end: number, chips: readonly ComposerRange[]): ComposerEdit | null {
  if (!Number.isInteger(start) || start !== end || start < 0 || start > value.length) return null;
  const line = currentLine(value, start), parsed = parseListMarker(line.text);
  if (!parsed?.marker.indent || insideChip(chips, start)) return null;
  const { marker } = parsed, parent = parentMarker(value, line.start, marker, chips);
  const indent = parent?.indent ?? marker.indent.replace(/^(?:\t| {1,2})/, '');
  const continues = parent?.kind === 'ordered' && marker.kind === 'ordered' && parent.delimiter === marker.delimiter;
  const number = continues ? nestedNumber(marker, parent) : { replacedLength: 0, text: '' };
  const replacedEnd = line.start + marker.indent.length + number.replacedLength, replacement = indent + number.text;
  const caret = start < replacedEnd ? line.start + indent.length : start - replacedEnd + line.start + replacement.length;
  const renumbered = continues ? renumberFollowingItems(value, line.end, parent, chips) : null;
  if (renumbered) return listEdit(line.start, renumbered.end, replacement + value.slice(replacedEnd, line.end) + renumbered.text, caret);
  return listEdit(line.start, replacedEnd, replacement, caret);
}

/** The list lines of a plain composer, for the Contract's Tab admission (a projected `list` block). */
export function plainListLines(value: string): ComposerRange[] {
  const lines: ComposerRange[] = [];
  let start = 0;
  for (const text of value.split('\n')) {
    if (parseListMarker(text)) lines.push({ start, end: start + text.length });
    start += text.length + 1;
  }
  return lines;
}
