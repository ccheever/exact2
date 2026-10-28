// splitForMiddleTruncate (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: apps/web/src/components/ui/middle-truncate.tsx):
// where a label keeps its tail when it is cut. A path or branch keeps its last segment (up to 16 characters, else the
// last 10), started on a word where one is near. The row draws the head with an ellipsis and the tail whole; Contract's
// `text-overflow` cuts only at the end, so the ellipsis is the head's, not the tail's.
const DEFAULT_TAIL = 10;
const MAX_SEGMENT_TAIL = 16;
const MIN_WORD_TAIL = 4;
const WORD_BREAK = /[-_./]/;

export function splitForMiddleTruncate(value: string, tail?: number): { head: string; tail: string } | null {
  // Split by code point to avoid cutting surrogate pairs into broken glyphs.
  const chars = Array.from(value);
  let keep = tail ?? DEFAULT_TAIL;
  if (tail === undefined) {
    const slash = chars.lastIndexOf('/');
    if (slash > 0 && slash < chars.length - 1) {
      const segment = chars.length - slash - 1;
      keep = segment <= MAX_SEGMENT_TAIL ? segment : DEFAULT_TAIL;
    }
  }
  if (keep <= 0 || chars.length <= keep + 4) return null;
  let cut = chars.length - keep;
  // Start an automatic tail on a word ("machine", not "er-machine"). A shorter tail is preferred so the head stays
  // readable; a longer one is the fallback.
  if (tail === undefined && !WORD_BREAK.test(chars[cut - 1]!)) {
    const startsWord = (index: number) => WORD_BREAK.test(chars[index - 1]!);
    let snapped = -1;
    for (let index = cut + 1; index <= chars.length - MIN_WORD_TAIL; index++) {
      if (startsWord(index)) { snapped = index; break; }
    }
    for (let index = cut - 1; snapped < 0 && index >= chars.length - MAX_SEGMENT_TAIL && index > 4; index--) {
      if (startsWord(index)) snapped = index;
    }
    if (snapped > 0) cut = snapped;
  }
  return { head: chars.slice(0, cut).join(''), tail: chars.slice(cut).join('') };
}
