import { mobileMarkdownBlocks, type MarkdownBlock } from './markdown-blocks';
import { fileMarkdownPlan, type FileMarkdownPart, type FileTableRow } from './shared/file-markdown-parser';

type Alignment = 'left' | 'center' | 'right';
type TablePart = Extract<FileMarkdownPart, { kind: 'table' }>;
export type ThreadMarkdownBlock = (MarkdownBlock & { columns: 0; alignments: []; rows: [] })
  | { id: string; kind: 'table'; text: string; language: ''; tokens: []; columns: number; alignments: Alignment[]; rows: FileTableRow[] };

function sourceCells(line: string): string[] {
  const cells: string[] = [];
  let start = 0, escaped = false;
  for (let index = 0; index < line.length; index++) {
    const character = line[index];
    if (character === '|' && !escaped) { cells.push(line.slice(start, index).trim()); start = index + 1; }
    escaped = character === '\\' && !escaped;
  }
  cells.push(line.slice(start).trim());
  if (cells.length > 1 && !cells[0]) cells.shift();
  if (cells.length > 1 && !cells.at(-1)) cells.pop();
  return cells;
}

function tableSlice(block: MarkdownBlock, part: TablePart) {
  const match = /^:table:(\d+)$/.exec(part.idSuffix);
  if (!match) return null;
  const start = Number(match[1]);
  let end = start;
  for (let line = 0; line <= part.rows.length; line++) {
    const newline = block.text.indexOf('\n', end);
    if (newline < 0) {
      if (line !== part.rows.length) return null;
      end = block.text.length;
    } else end = newline + 1;
  }
  const text = block.text.slice(start, end), lines = text.trimEnd().split('\n'), delimiter = lines[1]?.trim();
  if (!delimiter) return null;
  if (lines.length !== part.rows.length + 1 || lines.some(line => sourceCells(line).length !== part.columns)
    || part.rows[0]!.cells.every(cell => cell.runs.every(run => !run.text.trim()))) return null;
  const cells = sourceCells(delimiter);
  if (cells.some(cell => !/^:?-+:?$/.test(cell))) return null;
  const alignments = cells.map<Alignment>(cell => cell.endsWith(':') ? cell.startsWith(':') ? 'center' : 'right' : 'left');
  return { start, end, block: { id: block.id + part.idSuffix, kind: 'table' as const, text, language: '' as const,
    tokens: [] as [], columns: part.columns, alignments, rows: part.rows } };
}

export function mobileThreadMarkdownBlocks(body: string, dark: boolean): ThreadMarkdownBlock[] {
  const source = body.replace(/\r\n?/g, '\n'), blocks = mobileMarkdownBlocks(source, dark);
  const plain = (block: MarkdownBlock): ThreadMarkdownBlock => ({ ...block, columns: 0, alignments: [], rows: [] });
  if (!source.includes('|') && !/^ {0,3}:?-+:?[ \t]*$/m.test(source)) return blocks.map(plain);
  const tables = new Map<number, TablePart[]>();
  for (const part of fileMarkdownPlan(source, blocks)) {
    if (part.kind !== 'table') continue;
    const group = tables.get(part.blockIndex) ?? [];
    group.push(part); tables.set(part.blockIndex, group);
  }
  return blocks.flatMap<ThreadMarkdownBlock>((block, index) => {
    const parts = tables.get(index);
    if (!parts) return [plain(block)];
    const result: ThreadMarkdownBlock[] = [];
    let start = 0;
    const prose = (end: number) => {
      const text = block.text.slice(start, end);
      if (text.trim()) result.push(plain({ ...block, id: start === 0 ? block.id : `${block.id}:text:${start}`, text }));
    };
    for (const part of parts) {
      const table = tableSlice(block, part);
      if (!table || table.start < start || table.end > block.text.length) return [plain(block)];
      prose(table.start); result.push(table.block); start = table.end;
    }
    prose(block.text.length);
    return result;
  });
}
