import { mobileMarkdownBlocks, type MarkdownBlock } from './markdown-blocks';

type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;
export type FileMarkdownBlock = (MarkdownBlock & { level: 0 })
  | { id: string; kind: 'heading'; text: string; level: HeadingLevel; language: ''; tokens: [] };
const levels: readonly HeadingLevel[] = [1, 2, 3, 4, 5, 6];

export function mobileFileMarkdownBlocks(body: string, dark: boolean): FileMarkdownBlock[] {
  const result: FileMarkdownBlock[] = [];
  for (const block of mobileMarkdownBlocks(body, dark)) {
    if (block.kind === 'code' || /^[ \t]*\[|^ {0,3}(?:`{3,}|~{3,}|[-+*][ \t]|\d+[.)][ \t]|>)|^[ \t]+\S|</m.test(block.text)) {
      result.push({ ...block, level: 0 });
      continue;
    }
    const lines = block.text.split('\n');
    let offset = 0, start = 0, split = false;
    const prose = (end: number) => {
      const text = block.text.slice(start, end);
      if (text.trim()) result.push({ ...block, id: `${block.id}:text:${start}`, text, level: 0 });
    };
    for (let index = 0; index < lines.length; index++) {
      const line = lines[index]!, next = Math.min(block.text.length, offset + line.length + 1);
      const heading = /^(#{1,6})[ \t]+(.+)$/.exec(line), level = heading ? levels[heading[1]!.length - 1] : undefined;
      const text = heading?.[2]?.trim() ?? '';
      if (level !== undefined && text && !/[\\`*_~\[\]<>&#]/.test(text) && !/https?:\/\//i.test(text)
        && (index === 0 || !lines[index - 1]!.trim()) && (index + 1 === lines.length || !lines[index + 1]!.trim())) {
        prose(offset);
        result.push({ id: `${block.id}:heading:${offset}`, kind: 'heading', text, level, language: '', tokens: [] });
        start = next; split = true;
      }
      offset = next;
    }
    if (split) prose(block.text.length);
    else result.push({ ...block, level: 0 });
  }
  return result;
}
