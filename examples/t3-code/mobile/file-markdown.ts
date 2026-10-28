import { mobileMarkdownBlocks, type MarkdownBlock } from './markdown-blocks';
import { fileMarkdownPlan, type FileMarkdownPlanSegment, type FileTableRow } from './shared/file-markdown-parser';

type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;
type ContentBlock = (MarkdownBlock & { level: 0 })
  | { id: string; kind: 'heading'; text: string; level: HeadingLevel; language: ''; tokens: [] };
type TableBlock = { id: string; kind: 'table'; text: ''; level: 0; language: ''; tokens: []; columns: number; rows: FileTableRow[] };
export type FileMarkdownBlock = (ContentBlock & { columns: 0; rows: [] }) | TableBlock;
type Fragment = { start: number; end: number; block: ContentBlock };
type ProsePlan = Extract<FileMarkdownPlanSegment, { kind: 'prose' }>;
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

function proseFragments(block: MarkdownBlock): Fragment[] {
  if (/^[ \t]*\[|^ {0,3}(?:`{3,}|~{3,}|[-+*][ \t]|\d+[.)][ \t]|>)|^[ \t]+\S|</m.test(block.text)) {
    return [{ start: 0, end: block.text.length, block: { ...block, level: 0 } }];
  }
  const fragments: Fragment[] = [], lines = block.text.split('\n');
  let offset = 0, start = 0, split = false;
  const prose = (end: number) => {
    const text = block.text.slice(start, end);
    if (text.trim()) fragments.push({ start, end, block: { ...block, id: `${block.id}:text:${start}`, text, level: 0 } });
  };
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index]!, next = Math.min(block.text.length, offset + line.length + 1);
    const heading = /^(#{1,6})[ \t]+(.+)$/.exec(line), level = heading ? levels[heading[1]!.length - 1] : undefined;
    const text = heading?.[2]?.trim() ?? '';
    if (level !== undefined && text && !/[\\`*_~\[\]<>&#]/.test(text) && !/https?:\/\//i.test(text)
      && (index === 0 || !lines[index - 1]!.trim()) && (index + 1 === lines.length || !lines[index + 1]!.trim())) {
      prose(offset);
      fragments.push({ start: offset, end: next,
        block: { id: `${block.id}:heading:${offset}`, kind: 'heading', text, level, language: '', tokens: [] } });
      start = next; split = true;
    }
    offset = next;
  }
  if (split) prose(block.text.length);
  else fragments.push({ start: 0, end: block.text.length, block: { ...block, level: 0 } });
  const folded = fragments.map(fragment => {
    if (fragment.block.kind !== 'markdown') return fragment;
    const text = joinedPlainText(fragment.block.text);
    return text === undefined ? undefined : { ...fragment, block: { ...fragment.block, text } };
  });
  return folded.every(fragment => fragment !== undefined) ? folded : fragments;
}

function tablePlans(source: string, blocks: readonly MarkdownBlock[]): Map<string, ProsePlan> {
  const plan = fileMarkdownPlan(source), matched = new Map<string, ProsePlan>();
  if (!plan || plan.length !== blocks.length) return matched;
  let end = 0;
  for (const [index, segment] of plan.entries()) {
    const block = blocks[index];
    if (!block || segment.start < end || segment.end < segment.start || segment.end > source.length
      || source.slice(end, segment.start).trim()) return new Map();
    if (segment.kind === 'code') {
      if (block.kind !== 'code' || block.text !== segment.value || block.language !== segment.language) return new Map();
    } else {
      if (block.kind !== 'markdown' || block.text !== source.slice(segment.start, segment.end)) return new Map();
      matched.set(block.id, segment);
    }
    end = segment.end;
  }
  return source.slice(end).trim() ? new Map() : matched;
}

function insertTables(block: MarkdownBlock, fragments: Fragment[], plan: ProsePlan): (ContentBlock | TableBlock)[] {
  const tables = plan.tables.map(table => ({ ...table, start: table.start - plan.start, end: table.end - plan.start }));
  let previous = 0;
  for (const table of tables) {
    if (table.start < previous || table.end <= table.start || table.end > block.text.length
      || !fragments.some(fragment => fragment.block.kind === 'markdown' && table.start >= fragment.start && table.end <= fragment.end)) {
      return fragments.map(fragment => fragment.block);
    }
    previous = table.end;
  }
  const result: (ContentBlock | TableBlock)[] = [];
  for (const fragment of fragments) {
    const contained = tables.filter(table => table.start >= fragment.start && table.end <= fragment.end);
    if (!contained.length) { result.push(fragment.block); continue; }
    let start = fragment.start;
    const prose = (end: number) => {
      const text = block.text.slice(start, end);
      if (text.trim()) result.push({ ...block, id: start === fragment.start ? fragment.block.id : `${block.id}:text:${start}`, text, level: 0 });
    };
    for (const table of contained) {
      prose(table.start);
      result.push({ id: `${block.id}:table:${table.start}`, kind: 'table', text: '', language: '', tokens: [], level: 0,
        columns: table.columns, rows: table.rows });
      start = table.end;
    }
    prose(fragment.end);
  }
  return result;
}

export function mobileFileMarkdownBlocks(body: string, dark: boolean): FileMarkdownBlock[] {
  const source = body.replace(/\r\n?/g, '\n'), blocks = mobileMarkdownBlocks(source, dark), plans = tablePlans(source, blocks);
  const result: (ContentBlock | TableBlock)[] = [];
  for (const block of blocks) {
    if (block.kind === 'code') { result.push({ ...block, level: 0 }); continue; }
    const fragments = proseFragments(block), plan = plans.get(block.id);
    result.push(...(plan?.tables.length ? insertTables(block, fragments, plan) : fragments.map(fragment => fragment.block)));
  }
  return result.map<FileMarkdownBlock>(block => block.kind === 'table' ? block : { ...block, columns: 0, rows: [] });
}
