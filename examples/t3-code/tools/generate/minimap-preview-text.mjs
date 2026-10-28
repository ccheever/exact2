// timelineMinimapItems.ts's markdown preview text at T3 Code 74541bab17 (MIT, see LICENSE-T3).
// Bundled for the app by gen-minimap-text.mjs.
import remarkGfm from 'remark-gfm';
import remarkParse from 'remark-parse';
import { unified } from 'unified';

const markdownPreviewParser = unified().use(remarkParse).use(remarkGfm).freeze();

const INLINE_PARENT_TYPES = new Set(['paragraph', 'heading', 'tableCell', 'emphasis', 'strong', 'delete', 'link', 'linkReference']);

function markdownNodeText(node) {
  if (node.type === 'html') {
    return node.value
      .replace(/<!--[\s\S]*?(?:-->|$)/g, '')
      .replace(/<\/?([a-z][\w-]*)(?:[^"'<>]|"[^"]*"|'[^']*')*>/gi, (_, tag) =>
        /^(?:address|article|aside|blockquote|br|dd|details|div|dl|dt|figcaption|figure|footer|h[1-6]|header|hr|li|main|nav|ol|p|pre|section|summary|table|tbody|td|th|thead|tr|ul)$/i.test(tag) ? ' ' : '');
  }
  if (node.type === 'break') return ' ';
  if ('value' in node) return node.value;
  if (node.type === 'image' || node.type === 'imageReference') return node.alt ?? '';
  if (!('children' in node)) return '';
  const separator = INLINE_PARENT_TYPES.has(node.type) ? '' : ' ';
  return node.children.map(markdownNodeText).join(separator);
}

/** resolveTimelineMinimapPreview's assistant text before compaction: the first 1,500 UTF-16 units as plain text. */
export function minimapPreviewText(markdown) {
  return markdownNodeText(markdownPreviewParser.parse(markdown.slice(0, 1_500)));
}

const FILE_TABLE_CONTEXT_TYPES = new Set(['definition', 'linkReference', 'imageReference', 'footnoteDefinition', 'footnoteReference']);

function hasFileTableContext(node) {
  return FILE_TABLE_CONTEXT_TYPES.has(node.type) || ('children' in node && node.children.some(hasFileTableContext));
}

function fileNodeRange(node, start, end) {
  const from = node.position?.start.offset, to = node.position?.end.offset;
  return Number.isInteger(from) && Number.isInteger(to) && from >= start && to >= from && to <= end
    ? { start: from, end: to } : null;
}

function fileTableSlice(node, start, end) {
  const range = fileNodeRange(node, start, end);
  if (!range || range.start === range.end || !node.children.length) return null;
  const columns = node.children[0].children.length;
  if (!columns) return null;
  const rows = [];
  let rowEnd = range.start;
  for (const [rowIndex, row] of node.children.entries()) {
    const rowRange = fileNodeRange(row, rowEnd, range.end);
    if (row.type !== 'tableRow' || !rowRange) return null;
    const cells = [];
    let cellEnd = rowRange.start;
    for (const [cellIndex, cell] of row.children.entries()) {
      const cellRange = fileNodeRange(cell, cellEnd, rowRange.end);
      if (cell.type !== 'tableCell' || !cellRange) return null;
      const id = `${rowIndex}:${cellIndex}`, runs = [];
      let runEnd = cellRange.start;
      for (const [runIndex, run] of cell.children.entries()) {
        const runRange = fileNodeRange(run, runEnd, cellRange.end);
        if (!runRange || (run.type !== 'text' && run.type !== 'inlineCode')) return null;
        runs.push({ id: `${id}:${runIndex}`, kind: run.type === 'inlineCode' ? 'code' : 'text', text: run.value });
        runEnd = runRange.end;
      }
      const text = runs.map(run => run.text).join('').trim();
      const numeric = /^[+-]?(?:(?:\d+|\d{1,3}(?:,\d{3})+)(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?(?:%|\s*(?:ns|[µμu]s|ms|s|min|h|B|[kKMGTPE]i?B|[kMG]?Hz|px|pt))?$/.test(text);
      cells.push({ id, numeric, runs });
      cellEnd = cellRange.end;
    }
    while (cells.length < columns) cells.push({ id: `${rowIndex}:${cells.length}`, numeric: false, runs: [] });
    rows.push({ id: String(rowIndex), header: rowIndex === 0, cells: cells.slice(0, columns) });
    rowEnd = rowRange.end;
  }
  return { ...range, columns, rows };
}

export function fileMarkdownPlan(source) {
  if (source.includes('\r')) return null;
  const document = markdownPreviewParser.parse(source);
  if (!document.children.some(node => node.type === 'table')) return null;
  const segments = [];
  let position = 0, proseNodes = [], usable = false;
  const prose = end => {
    if (!source.slice(position, end).trim()) return;
    const tables = [];
    if (!proseNodes.some(hasFileTableContext)) {
      let tableEnd = position;
      for (const node of proseNodes) {
        if (node.type !== 'table') continue;
        const table = fileTableSlice(node, tableEnd, end);
        if (!table) { tables.length = 0; break; }
        tables.push(table);
        tableEnd = table.end;
      }
    }
    if (tables.length) usable = true;
    segments.push({ kind: 'prose', start: position, end, tables });
  };
  for (const node of document.children) {
    if (node.type !== 'code') { proseNodes.push(node); continue; }
    const range = fileNodeRange(node, position, source.length);
    if (!range || range.start === range.end) return null;
    prose(range.start);
    position = range.end + (source[range.end] === '\n' ? 1 : 0);
    segments.push({ kind: 'code', start: range.start, end: position, value: node.value, language: node.lang ?? '' });
    proseNodes = [];
  }
  prose(source.length);
  return usable ? segments : null;
}
