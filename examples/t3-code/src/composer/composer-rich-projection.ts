// ComposerPromptEditorTiptap's source projection at T3 Code 9c161462c9.
import { composerMarkdown, type ComposerBlockOrigin, type ComposerRange } from './composer-markdown';
import { codeLanguageEntry, codeLanguageLabel, languageOfInfoString } from './composer-code-languages';
import { composerChips, composerChipsInBlocks, type ComposerChip } from './composer-source-chips';
import type { ComposerMode } from './composer-source-session';

export type ComposerCodeHeader = ComposerRange & { infoStart: number; infoEnd: number; info: string; language: string; label: string };
export type ComposerSourceBlock = ComposerRange & { kind: string; contentStart: number; bodyStart: number; bodyEnd: number };
export type ComposerRichProjection = { owner: string; value: string; mode: ComposerMode; headers: ComposerCodeHeader[]; chips: ComposerChip[]; blocks: ComposerSourceBlock[] };
export type ComposerRichInput = {
  owner: string; value: string; styling: boolean; literal: boolean;
  blockOrigins?: readonly ComposerBlockOrigin[];
  chips?: readonly ComposerChip[];
};

/** One source signature owns the header, block and reference ranges. A late
 *  answer carries that signature back to Contract instead of mutating a newer
 *  draft. The Markdown field paints the source itself (LLP 1045). */
export function composerRichProjection(input: ComposerRichInput): ComposerRichProjection {
  const chips = input.literal ? [] : [...input.chips ?? composerChips(input.value)];
  const parsed = composerMarkdown(input.value, { styling: input.styling && !input.literal, blockOrigins: input.blockOrigins });
  const headers: ComposerCodeHeader[] = [];
  for (const block of parsed.blocks) {
    if (block.kind !== 'code') continue;
    const language = languageOfInfoString(block.info);
    headers.push({ start: block.open.start, end: block.open.end, infoStart: block.infoRange.start, infoEnd: block.infoRange.end,
      info: block.info, language: codeLanguageEntry(language)?.id ?? language, label: codeLanguageLabel(language) });
  }
  const blocks = parsed.blocks.map(block => ({ kind: block.kind, start: block.start, end: block.end,
    contentStart: 'contentStart' in block ? block.contentStart : block.start,
    bodyStart: block.kind === 'code' ? block.body.start : block.start,
    bodyEnd: block.kind === 'code' ? block.body.end : block.end }));
  return { owner: input.owner, value: input.value, mode: input.literal ? 'literal' : input.styling ? 'rich' : 'plain', headers, blocks, chips: composerChipsInBlocks(input.value, parsed.blocks, chips) };
}
