// Source ranges for composer-rich-text.ts at T3 Code 9c161462c9.
export type ComposerRange = { start: number; end: number };
export type ComposerMark = 'bold' | 'italic' | 'strike' | 'code';
export type ComposerSpan = ComposerRange & { name: string };
export type ComposerHidden = ComposerRange & { reveal: ComposerRange | null };
type Parsed = { spans: ComposerSpan[]; hidden: ComposerHidden[] };
type Frame = Parsed & { delimiter: string; mark: ComposerMark; open: number };

/** Offsets remain in the original UTF-16 string, including escaped markers. */
export function inlineDecorations(value: string, range: ComposerRange, literal: readonly ComposerRange[] = []): Parsed {
  const output: Parsed = { spans: [], hidden: [] }, stack: Frame[] = [];
  const current = () => stack.at(-1) ?? output;
  const overlaps = (start: number, end: number) => literal.some(item => item.start < end && item.end > start);
  const unit = (at: number) => at >= range.start && at < range.end ? value[at] ?? '' : '';
  let index = range.start;
  while (index < range.end) {
    const excluded = literal.find(item => item.start <= index && item.end > index);
    if (excluded) { index = excluded.end; continue; }
    const char = unit(index);
    if (char === '\\') { index += 2; continue; }
    if (char === '`') {
      let end = index + 1;
      while (unit(end) === '`') end++;
      const close = value.indexOf('`', end);
      if (end === index + 1 && close > end && close < range.end && unit(close + 1) !== '`' && !overlaps(index, close + 1)) {
        const reveal = { start: index, end: close + 1 }, target = current();
        target.spans.push({ name: 'cmp-code-inline', start: end, end: close });
        target.hidden.push({ start: index, end, reveal }, { start: close, end: close + 1, reveal });
        index = close + 1;
      } else index = end;
      continue;
    }
    if (char !== '*' && char !== '_' && char !== '~') { index++; continue; }
    let end = index + 1;
    while (unit(end) === char) end++;
    const before = unit(index - 1), after = unit(end);
    const canClose = before !== '' && !/\s/.test(before) && (char !== '_' || !/\w/.test(after));
    const canOpen = after !== '' && !/\s/.test(after) && (char !== '_' || !/\w/.test(before));
    while (index < end) {
      const top = stack.at(-1);
      const nestedBold = top?.mark === 'italic' && end - index === 2 && canOpen && char !== '~' && !stack.some(frame => frame.mark === 'bold');
      if (!nestedBold && canClose && top && value.startsWith(top.delimiter, index) && index + top.delimiter.length <= end) {
        stack.pop();
        const closeEnd = index + top.delimiter.length, target = current();
        const reveal = { start: top.open, end: closeEnd };
        target.spans.push({ name: `cmp-${top.mark}`, start: top.open + top.delimiter.length, end: index }, ...top.spans);
        target.hidden.push({ start: top.open, end: top.open + top.delimiter.length, reveal }, { start: index, end: closeEnd, reveal }, ...top.hidden);
        index = closeEnd;
      } else if (canOpen && (char !== '~' || end - index >= 2)) {
        const length = char === '~' || end - index >= 2 ? 2 : 1;
        const mark: ComposerMark = char === '~' ? 'strike' : length === 2 ? 'bold' : 'italic';
        if (stack.some(frame => frame.mark === mark)) { index = end; continue; }
        stack.push({ delimiter: char.repeat(length), mark, open: index, spans: [], hidden: [] });
        index += length;
      } else index = end;
    }
  }
  while (stack.length) {
    const frame = stack.pop();
    if (frame) { current().spans.push(...frame.spans); current().hidden.push(...frame.hidden); }
  }
  return output;
}
