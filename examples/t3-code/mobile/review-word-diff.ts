// T3 Code 365aa87982 reviewWordDiffs.ts/nativeReviewDiffAdapter.ts (MIT; see LICENSE-T3).
// Mobile admission is separate from Settings' unrestricted word-alt preview.
import type { DiffFileModel } from './shared/diff';
import { wordAltMarks, type Segment } from './shared/word-diff';
import type { ThreadCodeToken } from './thread-highlight';

export interface ReviewWordRange { start: number; end: number }
export interface ReviewToken extends ThreadCodeToken { mark: boolean }

function markedRanges(segments: Segment[]): ReviewWordRange[] {
  const ranges: ReviewWordRange[] = [];
  let offset = 0;
  for (const segment of segments) {
    const end = offset + segment.text.length;
    if (segment.mark && end > offset) {
      const previous = ranges.at(-1);
      if (previous && offset - previous.end <= 1) previous.end = end;
      else ranges.push({ start: offset, end });
    }
    offset = end;
  }
  return ranges;
}

function admittedRanges(content: string, ranges: ReviewWordRange[]): ReviewWordRange[] {
  const trimmed = ranges.flatMap(range => {
    let start = Math.max(0, range.start), end = Math.min(content.length, range.end);
    while (start < end && /\s/.test(content[start] ?? '')) start++;
    while (end > start && /\s/.test(content[end - 1] ?? '')) end--;
    return end > start ? [{ start, end }] : [];
  });
  if (!trimmed.length || trimmed.length > 4) return [];
  const meaningful = content.replace(/\s/g, '').length;
  const highlighted = trimmed.reduce((total, range) => total + content.slice(range.start, range.end).replace(/\s/g, '').length, 0);
  return meaningful > 0 && highlighted / meaningful <= .45 ? trimmed : [];
}

/** Accepted native ranges in source UTF-16 offsets, before display tab expansion. */
export function reviewWordRanges(before: string, after: string): [ReviewWordRange[], ReviewWordRange[]] {
  const deletion = before.endsWith('\n') ? before.slice(0, -1) : before;
  const addition = after.endsWith('\n') ? after.slice(0, -1) : after;
  if (!before || !after || deletion.length > 1000 || addition.length > 1000) return [[], []];
  const [old, next] = wordAltMarks(deletion, addition);
  return [admittedRanges(before, markedRanges(old)), admittedRanges(after, markedRanges(next))];
}

/** Pair source runs within one hunk, before Review inserts comment rows. */
export function reviewHunkWordRanges(lines: DiffFileModel['hunks'][number]['lines']): ReviewWordRange[][] {
  const ranges = lines.map((): ReviewWordRange[] => []);
  let index = 0;
  while (index < lines.length) {
    const deleted: number[] = [], added: number[] = [];
    while (lines[index]?.kind === 'deletion') deleted.push(index++);
    while (lines[index]?.kind === 'addition') added.push(index++);
    for (let pair = 0; pair < Math.min(deleted.length, added.length); pair++) {
      const old = deleted[pair]!, next = added[pair]!;
      [ranges[old], ranges[next]] = reviewWordRanges(lines[old]!.text, lines[next]!.text);
    }
    if (!deleted.length && !added.length) index++;
  }
  return ranges;
}

/** Split raw syntax runs first; display expansion must not move source range offsets. */
export function reviewMarkedTokens(tokens: ThreadCodeToken[], ranges: ReviewWordRange[]): ReviewToken[] {
  const result: ReviewToken[] = [];
  let offset = 0, rangeIndex = 0;
  for (const token of tokens) {
    const end = offset + token.text.length;
    let start = offset;
    while (start < end) {
      while (rangeIndex < ranges.length && ranges[rangeIndex]!.end <= start) rangeIndex++;
      const range = ranges[rangeIndex], mark = !!range && range.start <= start;
      const stop = Math.min(end, range ? mark ? range.end : range.start : end);
      result.push({ ...token, id: start === offset && stop === end ? token.id : `${token.id}@${start - offset}`,
        text: token.text.slice(start - offset, stop - offset).replace(/\t/g, '    '), mark });
      start = stop;
    }
    offset = end;
  }
  return result;
}
