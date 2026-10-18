// ComposerPromptEditorTiptap's source projection at T3 Code 9c161462c9.
import { highlight } from '../highlight/timeline-highlight';
import { composerMarkdown, type ComposerDecorations } from './composer-markdown';
import { codeLanguageEntry, codeLanguageLabel, languageOfInfoString } from './composer-code-languages';
import { chipDecorations, composerChips, type ComposerChip } from './composer-source-chips';
import type { ComposerRange } from './composer-markdown-inline';
import type { ComposerMode } from './composer-source-session';

export const MAX_HIGHLIGHTED_CODE_BLOCK_LENGTH = 20_000;
export type ComposerCodeHeader = ComposerRange & { infoStart: number; infoEnd: number; info: string; language: string; label: string };
export type ComposerSourceBlock = ComposerRange & { kind: string; contentStart: number; bodyStart: number; bodyEnd: number };
export type ComposerRichProjection = { owner: string; value: string; mode: ComposerMode; decorations: ComposerDecorations; headers: ComposerCodeHeader[]; chips: ComposerChip[]; blocks: ComposerSourceBlock[] };
export type ComposerRichInput = {
  owner: string; value: string; styling: boolean; literal: boolean;
  skills: Readonly<Record<string, string>>;
  literalRanges: readonly ComposerRange[];
  chips?: readonly ComposerChip[];
};

/** One source signature owns both body tokens and header ranges. A late answer
 *  carries that signature back to Contract instead of mutating a newer draft. */
export function composerRichProjection(input: ComposerRichInput): ComposerRichProjection {
  const chips = input.literal ? [] : [...input.chips ?? composerChips(input.value)];
  const parsed = composerMarkdown(input.value, { styling: input.styling && !input.literal, atoms: chipDecorations(chips, input.skills), literal: input.literalRanges });
  const headers: ComposerCodeHeader[] = [];
  for (const block of parsed.blocks) {
    if (block.kind !== 'code') continue;
    const language = languageOfInfoString(block.info);
    headers.push({ start: block.open.start, end: block.open.end, infoStart: block.infoRange.start, infoEnd: block.infoRange.end,
      info: block.info, language: codeLanguageEntry(language)?.id ?? language, label: codeLanguageLabel(language) });
    const body = input.value.slice(block.body.start, block.body.end);
    if (body.length > MAX_HIGHLIGHTED_CODE_BLOCK_LENGTH) continue;
    let offset = block.body.start;
    for (const token of highlight(body, language)) {
      const end = offset + token.text.length;
      if (token.cls) parsed.decorations.spans.push({ name: token.cls === '+i' ? 'cmp-syn-italic' : `cmp-syn-${token.cls.replace('+', '-')}`, start: offset, end });
      offset = end;
    }
  }
  const blocks = parsed.blocks.map(block => ({ kind: block.kind, start: block.start, end: block.end,
    contentStart: 'contentStart' in block ? block.contentStart : block.start,
    bodyStart: block.kind === 'code' ? block.body.start : block.start,
    bodyEnd: block.kind === 'code' ? block.body.end : block.end }));
  return { owner: input.owner, value: input.value, mode: input.literal ? 'literal' : input.styling ? 'rich' : 'plain', decorations: parsed.decorations, headers, blocks, chips: chips.filter(chip => parsed.decorations.atoms.some(atom => atom.start === chip.start && atom.end === chip.end)) };
}
