import { CODE_BLOCK_LANGUAGES, withInfoStringLanguage } from './composer-code-languages';
import { composerMarkdown, type ComposerBlockOrigin } from './composer-markdown';
import type { ComposerRange } from './composer-markdown';

export type ComposerReplacement = ComposerRange & { text: string };

/** Header actions carry the observed source and opening-line range. A delayed
 *  picker result must never change another occurrence or a newer draft. */
export function composerLanguageEdit(input: {
  value: string; current: string; header: ComposerRange; language: string; blockOrigins?: readonly ComposerBlockOrigin[];
}): ComposerReplacement | null {
  if (input.value !== input.current || !CODE_BLOCK_LANGUAGES.some(item => item.id === input.language)) return null;
  const block = composerMarkdown(input.current, { blockOrigins: input.blockOrigins }).blocks.find(block => block.kind === 'code'
    && block.open.start === input.header.start && block.open.end === input.header.end);
  if (block?.kind !== 'code') return null;
  const text = withInfoStringLanguage(block.info, input.language);
  return text === block.info ? null : { ...block.infoRange, text };
}
