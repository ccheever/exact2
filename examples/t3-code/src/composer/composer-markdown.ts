// Source-backed blocks from composer-rich-text-doc.ts at T3 Code 9c161462c9.
import { inlineDecorations, type ComposerHidden, type ComposerRange, type ComposerSpan } from './composer-markdown-inline';

export type ComposerLine = ComposerRange & { text: string; after: number };
type TextBlock = ComposerRange & { contentStart: number; prefix: string };
export type ComposerBlock =
  | (TextBlock & { kind: 'paragraph' | 'quote' })
  | (TextBlock & { kind: 'heading'; level: number })
  | (TextBlock & { kind: 'list'; list: 'bullet' | 'ordered' | 'task'; indent: string; marker: string; checked: boolean })
  | (ComposerRange & { kind: 'rule' })
  | (ComposerRange & { kind: 'code'; fence: string; info: string; infoRange: ComposerRange; open: ComposerRange; body: ComposerRange; close: ComposerRange | null });
export type ComposerDecorationBlock = ComposerSpan & { marker: string };
export type ComposerAtom = ComposerSpan & { label: string };
export type ComposerDecorations = { value: string; spans: ComposerSpan[]; blocks: ComposerDecorationBlock[]; hidden: ComposerHidden[]; atoms: ComposerAtom[] };
export type ComposerMarkdown = { decorations: ComposerDecorations; blocks: ComposerBlock[] };

export function composerLines(source: string): ComposerLine[] {
  const lines: ComposerLine[] = [];
  let start = 0;
  for (;;) {
    const next = source.indexOf('\n', start), end = next < 0 ? source.length : next;
    lines.push({ start, end, after: next < 0 ? end : end + 1, text: source.slice(start, end) });
    if (next < 0) return lines;
    start = next + 1;
  }
}

export function parseOpeningFence(line: string): { fence: string; info: string } | null {
  const match = /^(`{3,}|~{3,})([^\n]*)$/.exec(line);
  const fence = match?.[1], info = match?.[2];
  if (!fence || info === undefined || fence[0] === '`' && info.includes('`')) return null;
  return { fence, info };
}
export function isClosingFence(line: string, fence: string): boolean {
  const run = /^(`{3,}|~{3,})[ \t]*\r?$/.exec(line)?.[1];
  return run !== undefined && run[0] === fence[0] && run.length >= fence.length;
}

function textBlock(line: ComposerLine): Exclude<ComposerBlock, { kind: 'code' | 'rule' }> {
  const heading = /^(#{1,6})([ \t]+)([^\n]*)$/.exec(line.text);
  if (heading?.[1] && heading[2]) {
    const prefix = heading[1] + heading[2];
    return { kind: 'heading', level: heading[1].length, ...line, prefix, contentStart: line.start + prefix.length };
  }
  const quote = /^(>[ \t]*)([^\n]*)$/.exec(line.text);
  if (quote?.[1]) return { kind: 'quote', ...line, prefix: quote[1], contentStart: line.start + quote[1].length };
  const task = /^([ \t]*)-([ \t]+)\[([ xX])\]([ \t]*)([^\n]*)$/.exec(line.text);
  if (task && (task[4] || !task[5])) {
    const indent = task[1] ?? '', prefix = `${indent}-${task[2]}[${task[3]}]${task[4]}`;
    return { kind: 'list', ...line, list: 'task', indent, marker: '-', checked: task[3]?.toLowerCase() === 'x', prefix, contentStart: line.start + prefix.length };
  }
  const list = /^([ \t]*)(\d+[.)]|[-*+])([ \t]+|$)/.exec(line.text);
  if (list?.[2]) {
    const indent = list[1] ?? '', marker = list[2], prefix = indent + marker + (list[3] ?? '');
    return { kind: 'list', ...line, list: /^\d/.test(marker) ? 'ordered' : 'bullet', indent, marker, checked: false, prefix, contentStart: line.start + prefix.length };
  }
  return { kind: 'paragraph', ...line, prefix: '', contentStart: line.start };
}

/** Every range points into `source`; parsing never rewrites the stored draft. */
export function composerMarkdown(source: string, options: { styling?: boolean; atoms?: readonly ComposerAtom[]; literal?: readonly ComposerRange[] } = {}): ComposerMarkdown {
  const decorations: ComposerDecorations = { value: source, spans: [], blocks: [], hidden: [], atoms: [] }, blocks: ComposerBlock[] = [];
  const lines = composerLines(source), atoms = options.atoms ?? [], literal = [...options.literal ?? [], ...atoms];
  const conceal = (start: number, end: number) => { if (end > start) decorations.hidden.push({ start, end, reveal: null }); };
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index];
    if (!line) continue;
    const opening = options.styling !== false ? parseOpeningFence(line.text) : null;
    if (opening && line.after > line.end) {
      let last = index + 1;
      while (last < lines.length && !isClosingFence(lines[last]?.text ?? '', opening.fence)) last++;
      const close = lines[last] ?? null, end = close?.end ?? source.length;
      const bodyEnd = close ? Math.max(line.after, close.start - 1) : source.length;
      const block: ComposerBlock = { kind: 'code', start: line.start, end, fence: opening.fence, info: opening.info,
        infoRange: { start: line.start + opening.fence.length, end: line.end }, open: { start: line.start, end: line.end },
        body: { start: line.after, end: bodyEnd }, close: close ? { start: close.start, end: close.end } : null };
      blocks.push(block);
      decorations.blocks.push({ name: 'cmp-code-block', start: line.start, end, marker: '' });
      conceal(line.start, line.end);
      if (close) conceal(close.start, close.end);
      index = close ? last : lines.length;
      continue;
    }
    if (options.styling !== false && /^([-*_])(?:[ \t]*\1){2,}[ \t]*\r?$/.test(line.text)) {
      blocks.push({ kind: 'rule', start: line.start, end: line.end });
      decorations.blocks.push({ name: 'cmp-rule', start: line.start, end: line.end, marker: 'rule' });
      conceal(line.start, line.end);
      continue;
    }
    const block: Exclude<ComposerBlock, { kind: 'code' | 'rule' }> = options.styling === false ? { kind: 'paragraph', ...line, prefix: '', contentStart: line.start } : textBlock(line);
    blocks.push(block);
    if (block.kind === 'heading' || block.kind === 'quote' || block.kind === 'list') {
      const depth = block.kind === 'list' ? Math.min(8, block.indent.replace(/\t/g, '  ').length / 2 | 0) : 0;
      const name = block.kind === 'heading' ? `cmp-h${block.level}` : block.kind === 'quote' ? 'cmp-quote' : `cmp-list-depth${depth}`;
      const marker = block.kind !== 'list' ? '' : block.list === 'task' ? block.checked ? 'checkbox:on' : 'checkbox:off' : block.list === 'ordered' ? 'source' : 'bullet';
      if (block.end > block.start) decorations.blocks.push({ name, start: block.start, end: block.end, marker });
      conceal(block.start, block.contentStart);
    }
    if (options.styling !== false) {
      const inline = inlineDecorations(source, { start: block.contentStart, end: block.end }, literal);
      decorations.spans.push(...inline.spans);
      decorations.hidden.push(...inline.hidden);
    }
    decorations.atoms.push(...atoms.filter(atom => atom.start >= block.contentStart && atom.end <= block.end));
  }
  return { decorations, blocks };
}

export function codeAt(blocks: readonly ComposerBlock[], selection: ComposerRange): Extract<ComposerBlock, { kind: 'code' }> | null {
  return blocks.find((block): block is Extract<ComposerBlock, { kind: 'code' }> => block.kind === 'code' && selection.start >= block.body.start && selection.end <= block.body.end) ?? null;
}
