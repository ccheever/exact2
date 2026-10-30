import { mobileMarkdownBlocks, type MarkdownBlock } from './markdown-blocks';
import { fileMarkdownPlan, type FileTableRow } from './shared/file-markdown-parser';

type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;
type ContentBlock = (MarkdownBlock & { level: 0 })
  | { id: string; kind: 'heading'; text: string; level: HeadingLevel; language: ''; tokens: [] };
type TableBlock = { id: string; kind: 'table'; text: ''; level: 0; language: ''; tokens: []; columns: number; rows: FileTableRow[] };
export type FileMarkdownBlock = (ContentBlock & { columns: 0; rows: [] }) | TableBlock;
export function mobileFileMarkdownBlocks(body: string, dark: boolean): FileMarkdownBlock[] {
  const source = body.replace(/\r\n?/g, '\n'), blocks = mobileMarkdownBlocks(source, dark);
  return fileMarkdownPlan(source, blocks).map<FileMarkdownBlock>(part => {
    const block = blocks[part.blockIndex]!;
    switch (part.kind) {
      case 'code': return { ...block, level: 0, columns: 0, rows: [] };
      case 'markdown': return { ...block, id: block.id + part.idSuffix, text: part.text, level: 0, columns: 0, rows: [] };
      case 'heading': return { id: block.id + part.idSuffix, kind: 'heading', text: part.text, level: part.level,
        language: '', tokens: [], columns: 0, rows: [] };
      case 'table': return { id: block.id + part.idSuffix, kind: 'table', text: '', level: 0,
        language: '', tokens: [], columns: part.columns, rows: part.rows };
      default: { const exhaustive: never = part; return exhaustive; }
    }
  });
}
