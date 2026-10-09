// Rendered previews (lane r4-surfaces; MIT reference, see LICENSE-T3: DelimitedTablePreview.tsx and
// packages/shared/src/delimitedPreview.ts): CSV/TSV as rows, and a sent attachment's Markdown as
// ChatDocument blocks (gaps as macos/src/markdown.rs block_gap; r5-panels-attach.ts). A Markdown file
// in Files is parsed by the chat renderer itself (markdown-links-and-files-preview: app.contract
// filesMarkdown over macos/src/markdown.rs, r4-surfaces-files.ts renderedMarkdown).
import { fileIconToken } from './timeline-files';

export type Run = { id: string; text: string; weight: number; slant: string; mono: boolean; href: string; kind: string; icon: string };
export type Block = { id: string; kind: string; depth: number; marker: string; text: string; href: string; header: boolean; runs: Run[]; cells: { id: string; runs: Run[] }[]; gap: number; flow: boolean;
  // r4-timeline: markdown.contract ChatBlock's table fields (a `table` block's rows and sizing columns, its Markdown and CSV); empty here.
  rows: { id: string; header: boolean; cells: { id: string; runs: Run[] }[] }[]; columns: { id: string; sizer: Run[]; capped: boolean; header: boolean; grow: number; align: string }[]; markdown: string; csv: string; closedFence: boolean;
  // markdown.contract ChatBlock's GFM task ("open", "done" or "") and its offset (-1: the disabled checkbox of a read-only document).
  task: string; taskOffset: number };
export type Document = { id: string; blocks: Block[] };

const run = (id: number, text: string, extra: Partial<Run> = {}): Run => ({ id: String(id), text, weight: 400, slant: 'normal', mono: false, href: '', kind: '', icon: '', ...extra });
/** Inline spans: `code`, **bold**, *italic*, [links](url); prose splits into words so FlowRuns can wrap it. */
export function inlineRuns(source: string, words: boolean): Run[] {
  const runs: Run[] = [];
  const pattern = /`([^`]+)`|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*]+)\*|_([^_]+)_|\[([^\]]+)\]\(([^)\s]+)\)/g;
  let last = 0, match: RegExpExecArray | null;
  const push = (text: string, extra: Partial<Run> = {}) => {
    if (!text) return;
    if (words && !extra.mono) for (const word of text.match(/\S+\s*|\s+/g) ?? []) runs.push(run(runs.length, word, extra));
    else runs.push(run(runs.length, text, extra));
  };
  while ((match = pattern.exec(source))) {
    push(source.slice(last, match.index));
    if (match[1] !== undefined) push(match[1], { mono: true });
    else if (match[2] !== undefined || match[3] !== undefined) push(match[2] ?? match[3]!, { weight: 600 });
    else if (match[4] !== undefined || match[5] !== undefined) push(match[4] ?? match[5]!, { slant: 'italic' });
    else if (match[6] !== undefined) {
      const href = match[7]!, local = !/^[a-z][a-z0-9+.-]*:/i.test(href);
      push(match[6], local ? { kind: 'file', icon: fileIconToken(href) } : { href });
    }
    last = pattern.lastIndex;
  }
  push(source.slice(last));
  return runs;
}
const gapFor = (previous: Block | undefined, kind: string, depth: number) =>
  !previous ? 0 : kind === 'heading' ? 20 : previous.kind === 'item' && kind === 'item' && previous.depth === depth ? 4 : previous.kind === 'table-row' && kind === 'table-row' ? 0 : 10.4;

export function markdownDocument(id: string, text: string): Document {
  const blocks: Block[] = [];
  const add = (kind: string, fields: Partial<Block>) => {
    const previous = blocks[blocks.length - 1], depth = fields.depth ?? 0;
    blocks.push({ id: String(blocks.length), kind, depth, marker: '', text: '', href: '', header: false, runs: [], cells: [], flow: false, rows: [], columns: [], markdown: '', csv: '', closedFence: false, task: '', taskOffset: -1, ...fields, gap: gapFor(previous, kind, depth) });
  };
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  let paragraph: string[] = [];
  const flush = () => {
    if (!paragraph.length) return;
    const source = paragraph.join(' ').trim(); paragraph = [];
    const flow = /`/.test(source);
    add('paragraph', { runs: inlineRuns(source, flow), flow });
  };
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index]!;
    const fence = /^\s*(```|~~~)\s*([\w+-]*)/.exec(line);
    if (fence) {
      flush();
      const body: string[] = [];
      for (index++; index < lines.length && !lines[index]!.trim().startsWith(fence[1]!); index++) body.push(lines[index]!);
      add('code', { text: body.join('\n'), href: fence[2] ?? '', closedFence: index < lines.length });
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) { flush(); add('heading', { depth: heading[1]!.length, runs: inlineRuns(heading[2]!.replace(/\s+#+\s*$/, ''), false) }); continue; }
    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) { flush(); add('rule', {}); continue; }
    const item = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/.exec(line);
    if (item) {
      flush();
      const task = /^\[([ xX])\]\s+\S/.exec(item[3]!), content = task ? item[3]!.slice(3).trimStart() : item[3]!;
      const flow = /`/.test(content);
      add('item', { depth: Math.floor(item[1]!.replace(/\t/g, '  ').length / 2), marker: /\d/.test(item[2]!) ? item[2]!.replace(')', '.') : '•', runs: inlineRuns(content, flow), flow,
        task: task ? (task[1] === ' ' ? 'open' : 'done') : '' });
      continue;
    }
    const quote = /^\s*>\s?(.*)$/.exec(line);
    if (quote) { flush(); add('quote', { runs: inlineRuns(quote[1]!, false) }); continue; }
    if (/^\s*\|.*\|\s*$/.test(line) && /^\s*\|?\s*:?-{3,}/.test(lines[index + 1] ?? '')) {
      flush();
      const cells = (row: string) => row.trim().replace(/^\||\|$/g, '').split('|').map((cell, at) => ({ id: String(at), runs: inlineRuns(cell.trim(), false) }));
      add('table-row', { header: true, cells: cells(line) });
      for (index += 2; index < lines.length && /^\s*\|.*\|\s*$/.test(lines[index]!); index++) add('table-row', { cells: cells(lines[index]!) });
      index--;
      continue;
    }
    if (!line.trim()) { flush(); continue; }
    paragraph.push(line.trim());
  }
  flush();
  return { id, blocks };
}

/** parseDelimitedPreview: quoted delimiters, escaped quotes and multiline cells, 100 rows × 30 columns. */
export function parseDelimited(text: string, delimiter: ',' | '\t'): { rows: string[][]; truncated: boolean } {
  const rows: string[][] = [];
  let row: string[] = [], cell = '', quoted = false, truncated = false;
  let rowStart = text.charCodeAt(0) === 0xfeff ? 1 : 0;
  const endCell = () => { if (row.length < 30) row.push(cell); else truncated = true; cell = ''; };
  for (let index = rowStart; index < text.length; index++) {
    const char = text[index];
    if (char === '"') {
      if (quoted && text[index + 1] === '"') { if (cell.length < 2000) cell += '"'; else truncated = true; index++; continue; }
      if (quoted || cell === '') { quoted = !quoted; continue; }
    }
    if (!quoted && (char === delimiter || char === '\n' || char === '\r')) {
      endCell();
      if (char !== delimiter) {
        rows.push(row); row = [];
        if (char === '\r' && text[index + 1] === '\n') index++;
        rowStart = index + 1;
        if (rows.length === 100) return { rows, truncated: truncated || index < text.length - 1 };
      }
    } else if (cell.length < 2000) cell += char;
    else truncated = true;
  }
  if (rowStart < text.length) { endCell(); rows.push(row); }
  return { rows, truncated: truncated || quoted };
}
export function tableRows(path: string, text: string) {
  const { rows, truncated } = parseDelimited(text, /\.tsv$/i.test(path) ? '\t' : ',');
  return { truncated, rows: rows.map((cells, index) => ({ id: String(index), header: index === 0, cells: cells.map((cell, at) => ({ id: String(at), text: cell, syntax: '' })) })) };
}
