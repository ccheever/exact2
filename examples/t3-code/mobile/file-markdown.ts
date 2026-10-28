import { mobileMarkdownBlocks, type MarkdownBlock } from './markdown-blocks';

type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;
export type FileMarkdownBlock = (MarkdownBlock & { level: 0 })
  | { id: string; kind: 'heading'; text: string; level: HeadingLevel; language: ''; tokens: [] };
const levels: readonly HeadingLevel[] = [1, 2, 3, 4, 5, 6];

function joinedPlainText(text: string): string | undefined {
  for (const line of text.split('\n')) {
    if (/^[ \t]*$/.test(line)) continue;
    if (/[\t\u2028\u2029]| {2,}$/.test(line) || /`{2}/.test(line)) return;
    const plain = line.replace(/`[^`]+`/g, 'code');
    if (!/^[\p{L}\p{N}]/u.test(plain) || /^\d+[.)](?:[ \t]|$)/.test(plain)
      || /[`\\[\]<>*_~|&#$@]/.test(plain) || /\b(?:[a-z][a-z\d+.-]*:\/\/|www\.)/i.test(plain)) return;
  }
  return text.replace(/(\S) ?\n(?=\S)/g, '$1 ');
}

export function mobileFileMarkdownBlocks(body: string, dark: boolean): FileMarkdownBlock[] {
  const result: FileMarkdownBlock[] = [];
  for (const block of mobileMarkdownBlocks(body, dark)) {
    if (block.kind === 'code' || /^[ \t]*\[|^ {0,3}(?:`{3,}|~{3,}|[-+*][ \t]|\d+[.)][ \t]|>)|^[ \t]+\S|</m.test(block.text)) {
      result.push({ ...block, level: 0 });
      continue;
    }
    const fragments: FileMarkdownBlock[] = [];
    const lines = block.text.split('\n');
    let offset = 0, start = 0, split = false;
    const prose = (end: number) => {
      const text = block.text.slice(start, end);
      if (text.trim()) fragments.push({ ...block, id: `${block.id}:text:${start}`, text, level: 0 });
    };
    for (let index = 0; index < lines.length; index++) {
      const line = lines[index]!, next = Math.min(block.text.length, offset + line.length + 1);
      const heading = /^(#{1,6})[ \t]+(.+)$/.exec(line), level = heading ? levels[heading[1]!.length - 1] : undefined;
      const text = heading?.[2]?.trim() ?? '';
      if (level !== undefined && text && !/[\\`*_~\[\]<>&#]/.test(text) && !/https?:\/\//i.test(text)
        && (index === 0 || !lines[index - 1]!.trim()) && (index + 1 === lines.length || !lines[index + 1]!.trim())) {
        prose(offset);
        fragments.push({ id: `${block.id}:heading:${offset}`, kind: 'heading', text, level, language: '', tokens: [] });
        start = next; split = true;
      }
      offset = next;
    }
    if (split) prose(block.text.length);
    else fragments.push({ ...block, level: 0 });
    const folded = fragments.map(fragment => {
      if (fragment.kind !== 'markdown') return fragment;
      const text = joinedPlainText(fragment.text);
      return text === undefined ? undefined : { ...fragment, text };
    });
    result.push(...(folded.every(fragment => fragment !== undefined) ? folded : fragments));
  }
  return result;
}
