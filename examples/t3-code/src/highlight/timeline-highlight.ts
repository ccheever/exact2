// The shared highlighter's entry points: chat code blocks, the diff panel, the Files panel, the attachment code
// preview, content-search lines and command rows paint their code with these, in @pierre/highlights' colour classes
// (highlights.ts; syntax-ink.contract paints them).

import { languageIconToken } from '../shared/timeline-files';
import { highlightDeferrals, highlightShown, highlightTokens, type Token } from './highlights';

export type { Token } from './highlights';

/** Tokens for one code block or diff line; plain when the language has no lexer. `language` is a fence word or a path
 *  (highlights.ts lexerOf); with `maxLineLength`, longer lines paint plain (TOKENIZE_MAX_LINE_LENGTH). */
export function highlight(text: string, language: string, maxLineLength = 0): Token[] {
  if (!text) return [];
  return highlightTokens(text, language, maxLineLength) ?? [{ text, cls: '' }];
}

/** Every fenced code block of a message with its tokens, matched in Contract by its text. */
type Highlighted = { id: string; code: string; icon: string; tokens: CodeToken[] };
/** A token as markdown.contract CodeToken draws it: with `at`, where it starts in its code (UTF-16 units). */
export type CodeToken = { id: string; text: string; cls: string; at: number };

/** `tokens` with their ids and offsets. */
export function codeTokens(tokens: readonly Token[]): CodeToken[] {
  let at = 0;
  return tokens.map((token, index) => {
    const placed = { id: String(index), text: token.text, cls: token.cls, at };
    at += token.text.length;
    return placed;
  });
}
/** A message's blocks once all have their tokens, with each block's code and language: the answer paints them again. */
const cache = new Map<string, { blocks: Highlighted[]; painted: [string, string][] }>();
export function messageCodeBlocks(markdownText: string): Highlighted[] {
  if (!markdownText.includes('```') && !markdownText.includes('~~~')) return [];
  const known = cache.get(markdownText);
  if (known) {
    for (const [code, language] of known.painted) highlightShown(code, language);
    return known.blocks;
  }
  const blocks: Highlighted[] = [], painted: [string, string][] = [], deferrals = highlightDeferrals;
  const fence = /^( {0,3})(`{3,}|~{3,})([^\n`]*)\n([\s\S]*?)\n? {0,3}\2[`~]*[ \t]*(?:\n|$)/gm;
  for (const match of markdownText.matchAll(fence)) {
    const indent = match[1]!.length, language = match[3]!.trim().split(/\s+/)[0] ?? '';
    const code = (indent ? match[4]!.split('\n').map(line => line.replace(new RegExp(`^ {0,${indent}}`), '')).join('\n') : match[4]!).replace(/\n+$/, '');
    if (blocks.some(block => block.code === code)) continue;
    const tokens = highlight(code, language);
    painted.push([code, language]);
    blocks.push({ id: String(blocks.length), code, icon: languageIconToken(language), tokens: codeTokens(tokens) });
  }
  // Blocks still waiting for their tokens are asked again next time.
  if (highlightDeferrals !== deferrals) return blocks;
  if (cache.size > 200) cache.delete(cache.keys().next().value!);
  cache.set(markdownText, { blocks, painted });
  return blocks;
}
