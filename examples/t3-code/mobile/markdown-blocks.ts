import { mobileCodeTokens, type ThreadCodeToken } from './thread-highlight';

export interface MarkdownBlock { id: string; kind: 'markdown' | 'code'; text: string; language: string; tokens: ThreadCodeToken[] }

export function mobileMarkdownBlocks(body: string, dark: boolean): MarkdownBlock[] {
  body = body.replace(/\r\n?/g, '\n');
  const blocks: MarkdownBlock[] = [];
  const fence = /^( {0,3})(`{3,}|~{3,})([^\n]*)\n/gm;
  let position = 0;
  const prose = (text: string) => { if (text.trim()) blocks.push({ id: String(blocks.length), kind: 'markdown', text, language: '', tokens: [] }); };
  for (const match of body.matchAll(fence)) {
    if (match.index < position || match[2]![0] === '`' && match[3]!.includes('`')) continue;
    const closing = new RegExp(`^ {0,3}[${match[2]![0]}]{${match[2]!.length},}[ \\t]*(?:\\n|$)`, 'gm');
    const start = match.index + match[0].length;
    closing.lastIndex = start;
    const end = closing.exec(body);
    if (!end) break;
    prose(body.slice(position, match.index));
    const indent = match[1]!.length, language = match[3]!.trim().split(/\s+/)[0] || '';
    const content = body.slice(start, end.index);
    const text = (indent ? content.split('\n').map(line => line.replace(new RegExp(`^ {0,${indent}}`), '')).join('\n') : content).replace(/\n$/, '');
    blocks.push({ id: String(blocks.length), kind: 'code', text, language, tokens: mobileCodeTokens(text, language, dark) });
    position = end.index + end[0].length;
  }
  prose(body.slice(position)); return blocks;
}
