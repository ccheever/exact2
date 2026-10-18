import { CODE_BLOCK_LANGUAGES, withInfoStringLanguage } from './composer-code-languages';
import { composerMarkdown } from './composer-markdown';
import type { ComposerRange } from './composer-markdown-inline';

export type ComposerReplacement = ComposerRange & { text: string };

/** Header actions carry the observed source and opening-line range. A delayed
 *  picker result must never change another occurrence or a newer draft. */
export function composerLanguageEdit(input: {
  value: string; current: string; header: ComposerRange; language: string;
}): ComposerReplacement | null {
  if (input.value !== input.current || !CODE_BLOCK_LANGUAGES.some(item => item.id === input.language)) return null;
  const block = composerMarkdown(input.current).blocks.find(block => block.kind === 'code'
    && block.open.start === input.header.start && block.open.end === input.header.end);
  if (block?.kind !== 'code') return null;
  const text = withInfoStringLanguage(block.info, input.language);
  return text === block.info ? null : { ...block.infoRange, text };
}

/** The host's marker range identifies a whole task line. Only the checked
 *  source slot changes; Contract applies this with preserve selection + undo. */
export function composerTaskEdit(input: {
  value: string; current: string; block: ComposerRange; checked: boolean;
}): ComposerReplacement | null {
  if (input.value !== input.current) return null;
  const block = composerMarkdown(input.current).blocks.find(block => block.start === input.block.start && block.end === input.block.end);
  if (block?.kind !== 'list' || block.list !== 'task' || block.checked === input.checked) return null;
  const start = block.start + block.prefix.indexOf('[') + 1;
  return { start, end: start + 1, text: input.checked ? 'x' : ' ' };
}
