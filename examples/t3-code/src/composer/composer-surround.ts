// Typing a bracket or quote over a selection wraps it, from ComposerPromptEditorTiptap.tsx handleTextInput at
// T3 Code 74541bab17 (SURROUND_CLOSE). A composition never wraps: the Contract sends no composing input here.
import { codeAt, type ComposerBlock, type ComposerRange } from './composer-markdown';
import type { ComposerEdit } from './composer-markdown-edit';
import type { ComposerChip } from './composer-source-chips';

export const SURROUND_CLOSE: Readonly<Record<string, string>> = {
  '(': ')', '[': ']', '{': '}', "'": "'", '"': '"', '“': '”', '`': '`', '<': '>', '«': '»', '*': '*', _: '_',
};

const punctuation = (character: string | undefined) => character !== undefined && /[\p{P}\p{S}]/u.test(character);
const space = (character: string | undefined) => character === undefined || /\s/u.test(character);

/** The styled runs of one line, each from its opening marker to the end of its closing one: code spans, links, and
 *  `*`, `_` and `~~` emphasis paired by CommonMark's flanking rules, so `node_modules` and a lone `*` are plain text. */
export function composerStyledRuns(line: string): Array<{ start: number; end: number }> {
  const runs: Array<{ start: number; end: number }> = [], code: Array<{ start: number; end: number }> = [];
  for (let at = 0; at < line.length;) {
    if (line[at] !== '`') { at++; continue; }
    let open = 0;
    while (line[at + open] === '`') open++;
    let close = -1;
    for (let next = line.indexOf('`', at + open); next >= 0; next = line.indexOf('`', next)) {
      let length = 0;
      while (line[next + length] === '`') length++;
      if (length === open) { close = next + length; break; }
      next += length;
    }
    if (close < 0) { at += open; continue; }
    code.push({ start: at, end: close });
    at = close;
  }
  runs.push(...code);
  const inCode = (at: number) => code.some(run => at >= run.start && at < run.end);
  for (const link of line.matchAll(/\[[^\]\n]*\]\([^)\s]*\)/g)) if (!inCode(link.index)) runs.push({ start: link.index, end: link.index + link[0].length });
  const openers: Array<{ character: string; start: number; length: number }> = [];
  for (let at = 0; at < line.length;) {
    const character = line[at]!;
    if (!'*_~'.includes(character) || inCode(at)) { at++; continue; }
    let length = 0;
    while (line[at + length] === character) length++;
    const before = line[at - 1], after = line[at + length];
    const left = !space(after) && (!punctuation(after) || space(before) || punctuation(before));
    const right = !space(before) && (!punctuation(before) || space(after) || punctuation(after));
    const canOpen = character === '_' ? left && (!right || punctuation(before)) : left;
    const canClose = character === '_' ? right && (!left || punctuation(after)) : right;
    if (character !== '~' || length === 2) {
      const opener = canClose ? openers.findLastIndex(entry => entry.character === character && entry.length === length) : -1;
      if (opener >= 0) {
        runs.push({ start: openers[opener]!.start, end: at + length });
        openers.length = opener;
      } else if (canOpen) openers.push({ character, start: at, length });
    }
    at += length;
  }
  return runs;
}

/** The wrapping edit, its selection still the wrapped text; null when the typed text replaces the selection as usual:
 *  a reference (an atom chip) or any styled text in the selection, or the space beside a mention, is never wrapped.
 *  `blocks` are the rich composer's Markdown blocks; code holds no styling. */
export function composerSurround(value: string, selection: ComposerRange, text: string, chips: readonly ComposerChip[], blocks: readonly ComposerBlock[]): ComposerEdit | null {
  const closer = SURROUND_CLOSE[text], { start, end } = selection;
  if (!closer || start >= end || end > value.length) return null;
  if (chips.some(chip => chip.start < end && chip.end > start)) return null;
  // selectionTouchesMentionBoundary: the whitespace that delimits a mention.
  const inside = (at: number) => at >= start && at < end;
  if (chips.some(chip => chip.kind === 'mention' && (inside(chip.start - 1) && /\s/.test(value[chip.start - 1] ?? '') || inside(chip.end) && /\s/.test(value[chip.end] ?? '')))) return null;
  if (blocks.length && !codeAt(blocks, selection)) {
    // The lines the selection touches; a styled run never crosses a line in the composer's source.
    for (let lineStart = value.lastIndexOf('\n', start - 1) + 1; lineStart <= end && lineStart <= value.length;) {
      const newline = value.indexOf('\n', lineStart), lineEnd = newline < 0 ? value.length : newline;
      if (composerStyledRuns(value.slice(lineStart, lineEnd)).some(run => lineStart + run.start < end && lineStart + run.end > start)) return null;
      if (newline < 0) break;
      lineStart = newline + 1;
    }
  }
  return { start, end, text: text + value.slice(start, end) + closer, selection: { start: start + text.length, end: end + text.length } };
}
