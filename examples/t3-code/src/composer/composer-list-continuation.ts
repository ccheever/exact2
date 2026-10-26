// composer-list-continuation.ts at T3 Code 74541bab17 (MIT; LICENSE-T3): Enter and Tab in a
// Markdown list typed as text. The plain composer uses it whole; the rich composer's own list
// gesture (composer-markdown-edit.ts) renumbers with it.
import type { ComposerRange } from './composer-markdown';
import type { ComposerEdit } from './composer-markdown-edit';

type ListMarker =
  | { kind: 'ordered'; indent: string; numberText: string; delimiter: '.' | ')'; space: string }
  | { kind: 'task'; indent: string; space: string }
  | { kind: 'bullet'; indent: string; bullet: string; space: string };
export type OrderedMarker = Extract<ListMarker, { kind: 'ordered' }>;

export function parseListMarker(line: string): { marker: ListMarker; markerEnd: number } | null {
  const indent = /^[ \t]*/.exec(line)?.[0] ?? '';
  const rest = line.slice(indent.length);
  const ordered = /^(\d+)([.)])((?:[ \t]+)|\s*$)/.exec(rest);
  if (ordered?.[1] !== undefined && ordered[2] !== undefined) {
    return { marker: { kind: 'ordered', indent, numberText: ordered[1], delimiter: ordered[2] === ')' ? ')' : '.', space: ordered[3] ?? '' },
      markerEnd: indent.length + ordered[0].length };
  }
  const task = /^-\s\[[ xX]\]((?:[ \t]+)|\s*$)/.exec(rest);
  if (task) return { marker: { kind: 'task', indent, space: task[1] ?? '' }, markerEnd: indent.length + task[0].length };
  const bullet = /^([-*+])((?:[ \t]+)|\s*$)/.exec(rest);
  if (bullet?.[1] !== undefined) return { marker: { kind: 'bullet', indent, bullet: bullet[1], space: bullet[2] ?? '' }, markerEnd: indent.length + bullet[0].length };
  return null;
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
function indentWidth(line: string): number {
  let width = 0;
  for (const char of line) {
    if (char === ' ') width += 1;
    else if (char === '\t') width += 4 - (width % 4);
    else break;
  }
  return width;
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
const insideChip = (chips: readonly ComposerRange[], cursor: number) => chips.some(chip => cursor > chip.start && cursor < chip.end);
const listEdit = (start: number, end: number, text: string, caret: number): ComposerEdit => ({ start, end, text, selection: { start: caret, end: caret } });

/** listContinuationForEnter: the next item, the empty item removed, or null for an ordinary newline.
 *  A marker typed without its space ("2." alone) is text, so Enter keeps it. */
export function listContinuationForEnter(value: string, cursor: number, chips: readonly ComposerRange[]): ComposerEdit | null {
  if (!Number.isInteger(cursor) || cursor < 0 || cursor > value.length) return null;
  const line = currentLine(value, cursor), parsed = parseListMarker(line.text);
  if (!parsed || parsed.marker.space === '') return null;
  const markerEnd = line.start + parsed.markerEnd;
  if (cursor < markerEnd || insideChip(chips, cursor)) return null;
  if (value.slice(markerEnd, line.end).trim() === '') return listEdit(line.start, Math.max(cursor, markerEnd), '', line.start);
  const insertion = `\n${nextMarkerText(parsed.marker)}`, caret = cursor + insertion.length;
  if (parsed.marker.kind === 'ordered') {
    const renumbered = renumberFollowingItems(value, line.end, parsed.marker, chips);
    if (renumbered) return listEdit(cursor, renumbered.end, insertion + value.slice(cursor, line.end) + renumbered.text, caret);
  }
  return listEdit(cursor, cursor, insertion, caret);
}

/** listIndentForTab: two spaces before a list line's marker, the caret kept on its text. */
export function listIndentForTab(value: string, start: number, end: number, chips: readonly ComposerRange[]): ComposerEdit | null {
  if (!Number.isInteger(start) || start !== end || start < 0 || start > value.length) return null;
  const line = currentLine(value, start);
  if (!parseListMarker(line.text) || insideChip(chips, start)) return null;
  return listEdit(line.start, line.start, '  ', start + 2);
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
