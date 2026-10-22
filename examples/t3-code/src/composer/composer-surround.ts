// Typing a bracket or quote over a selection wraps it, from ComposerPromptEditorTiptap.tsx handleTextInput at
// T3 Code 74541bab17 (SURROUND_CLOSE). A composition never wraps: the Contract sends no composing input here.
import { codeAt, type ComposerBlock, type ComposerRange } from './composer-markdown';
import type { ComposerEdit } from './composer-markdown-edit';
import type { ComposerChip } from './composer-source-chips';

export const SURROUND_CLOSE: Readonly<Record<string, string>> = {
  '(': ')', '[': ']', '{': '}', "'": "'", '"': '"', '“': '”', '`': '`', '<': '>', '«': '»', '*': '*', _: '_',
};

/** The wrapping edit, its selection still the wrapped text; null when the typed text replaces the selection as usual:
 *  a reference (an atom chip) or styled text in the selection, or the space beside a mention, is never wrapped.
 *  `blocks` are the rich composer's Markdown blocks; code holds no styling. */
export function composerSurround(value: string, selection: ComposerRange, text: string, chips: readonly ComposerChip[], blocks: readonly ComposerBlock[]): ComposerEdit | null {
  const closer = SURROUND_CLOSE[text], { start, end } = selection;
  if (!closer || start >= end || end > value.length) return null;
  if (chips.some(chip => chip.start < end && chip.end > start)) return null;
  // selectionTouchesMentionBoundary: the whitespace that delimits a mention.
  const inside = (at: number) => at >= start && at < end;
  if (chips.some(chip => chip.kind === 'mention' && (inside(chip.start - 1) && /\s/.test(value[chip.start - 1] ?? '') || inside(chip.end) && /\s/.test(value[chip.end] ?? '')))) return null;
  if (blocks.length && !codeAt(blocks, selection) && /[*_~`]|\]\(/.test(value.slice(start, end))) return null;
  return { start, end, text: text + value.slice(start, end) + closer, selection: { start: start + text.length, end: end + text.length } };
}
