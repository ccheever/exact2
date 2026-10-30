// timelineMinimapItems.ts's markdown preview text at T3 Code 74541bab17 (MIT, see LICENSE-T3).
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
        if (!runRange) return null;
        if (run.type === 'strong' || run.type === 'emphasis') {
          if (!run.children.length) return null;
          let childEnd = runRange.start, text = '';
          for (const child of run.children) {
            const childRange = fileNodeRange(child, childEnd, runRange.end);
            if (child.type !== 'text' || !childRange || !child.value) return null;
            text += child.value;
            childEnd = childRange.end;
          }
          runs.push({ id: `${id}:${runIndex}`, kind: run.type, text });
        } else {
          if (run.type !== 'text' && run.type !== 'inlineCode') return null;
          runs.push({ id: `${id}:${runIndex}`, kind: run.type === 'inlineCode' ? 'code' : 'text', text: run.value });
        }
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

function fileSegments(source) {
  if (source.includes('\r')) return null;
  const document = markdownPreviewParser.parse(source);
  const segments = [];
  let position = 0, proseNodes = [];
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
    segments.push({ kind: 'prose', start: position, end, tables, nodes: proseNodes });
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
  return segments;
}

function matchingProsePlans(source, blocks) {
  const plan = fileSegments(source), matched = new Map();
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
      matched.set(index, segment);
    }
    end = segment.end;
  }
  return source.slice(end).trim() ? new Map() : matched;
}

function admitsPlainFold(text) {
  for (const line of text.split('\n')) {
    if (/^[ \t]*$/.test(line)) continue;
    if (/[\t\u2028\u2029]| {2,}$/.test(line) || /`{2}/.test(line)) return false;
    const plain = line.replace(/`[^`]+`/g, 'code');
    if (!/^[\p{L}\p{N}]/u.test(plain) || /^\d+[.)](?:[ \t]|$)/.test(plain)
      || /[`\\[\]<>*_~|&#$@]/.test(plain) || /\b(?:[a-z][a-z\d+.-]*:\/\/|www\.)/i.test(plain)) return false;
  }
  return true;
}

function proseFragments(text) {
  if (/^[ \t]*\[|^ {0,3}(?:`{3,}|~{3,}|[-+*][ \t]|\d+[.)][ \t]|>)|^[ \t]+\S|</m.test(text)) {
    return [{ kind: 'markdown', start: 0, end: text.length, idSuffix: '', plainFold: false }];
  }
  const fragments = [], lines = text.split('\n');
  let offset = 0, start = 0, split = false;
  const prose = end => {
    if (text.slice(start, end).trim()) fragments.push({ kind: 'markdown', start, end, idSuffix: `:text:${start}`, plainFold: false });
  };
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index], next = Math.min(text.length, offset + line.length + 1);
    const heading = /^(#{1,6})[ \t]+(.+)$/.exec(line), level = heading?.[1].length;
    const content = heading?.[2]?.trim() ?? '';
    if (level !== undefined && content && !/[\\`*_~\[\]<>&#]/.test(content) && !/https?:\/\//i.test(content)
      && (index === 0 || !lines[index - 1].trim()) && (index + 1 === lines.length || !lines[index + 1].trim())) {
      prose(offset);
      fragments.push({ kind: 'heading', start: offset, end: next, idSuffix: `:heading:${offset}`, text: content, level });
      start = next; split = true;
    }
    offset = next;
  }
  if (split) prose(text.length);
  else fragments.push({ kind: 'markdown', start: 0, end: text.length, idSuffix: '', plainFold: false });
  if (fragments.every(fragment => fragment.kind !== 'markdown' || admitsPlainFold(text.slice(fragment.start, fragment.end)))) {
    for (const fragment of fragments) if (fragment.kind === 'markdown') fragment.plainFold = true;
  }
  return fragments;
}

function insertTables(text, fragments, plan) {
  const tables = plan.tables.map(table => ({ ...table, start: table.start - plan.start, end: table.end - plan.start }));
  let previous = 0;
  for (const table of tables) {
    if (table.start < previous || table.end <= table.start || table.end > text.length
      || !fragments.some(fragment => fragment.kind === 'markdown' && table.start >= fragment.start && table.end <= fragment.end)) return fragments;
    previous = table.end;
  }
  const result = [];
  for (const fragment of fragments) {
    const contained = tables.filter(table => table.start >= fragment.start && table.end <= fragment.end);
    if (!contained.length) { result.push(fragment); continue; }
    let start = fragment.start;
    const prose = end => {
      if (text.slice(start, end).trim()) result.push({ kind: 'markdown', start, end,
        idSuffix: start === fragment.start ? fragment.idSuffix : `:text:${start}`, plainFold: false });
    };
    for (const table of contained) {
      prose(table.start);
      result.push({ ...table, kind: 'table', idSuffix: `:table:${table.start}` });
      start = table.end;
    }
    prose(fragment.end);
  }
  return result;
}

// Positions are UTF-16 offsets in the original document, including Markdown delimiters.
function paragraphBreaks(source, node, range) {
  const breaks = [];
  const children = (parent, start, end) => {
    let cursor = start;
    for (const child of parent.children) {
      const span = fileNodeRange(child, cursor, end);
      if (!span || span.start === span.end) return false;
      const gap = source.slice(cursor, span.start), raw = source.slice(span.start, span.end);
      // Remark omits one trailing space before a soft LF that starts a text leaf.
      if (gap && !(gap === ' ' && source[span.start] === '\n')) return false;
      if (child.type === 'text') {
        if (/[`\\[\]<>*_~|&#$@]/.test(raw) || /\b(?:[a-z][a-z\d+.-]*:\/\/|www\.)/i.test(raw)
          || raw.replace(/ (?=\n)/g, '') !== child.value) return false;
        for (let index = span.start; index < span.end; index++) {
          if (source[index] !== '\n') continue;
          const from = source[index - 1] === ' ' ? index - 1 : index;
          if (from < start || /\s/.test(source[from - 1] ?? '') || !source[index + 1] || /\s/.test(source[index + 1])) return false;
          breaks.push({ start: from, end: index + 1 });
        }
      } else if (child.type === 'strong' || child.type === 'emphasis') {
        const width = child.type === 'strong' ? 2 : 1, delimiter = raw.slice(0, width);
        if (!['*'.repeat(width), '_'.repeat(width)].includes(delimiter) || !raw.endsWith(delimiter)
          || !children(child, span.start + width, span.end - width)) return false;
      } else if (child.type === 'inlineCode') {
        if (!/^`[^`\n]+`$/.test(raw)) return false;
      } else if (child.type === 'break') {
        if (!/^(?: {2,}|\\)\n$/.test(raw)) return false;
      } else return false;
      cursor = span.end;
    }
    return /^ *$/.test(source.slice(cursor, end));
  };
  const raw = source.slice(range.start, range.end);
  if (/^\d+[.)](?:[ \t]|$)/m.test(raw) || !children(node, range.start, range.end)) return null;
  return breaks;
}

function certifiedFolds(source, text, fragments, plan) {
  if (/[\t\u2028\u2029]|^[ \t]+\S/m.test(text) || plan.nodes.some(hasFileTableContext)) return null;
  const folds = new Map();
  for (const fragment of fragments) {
    if (fragment.kind === 'markdown') folds.set(fragment, { coveredUntil: fragment.start, breaks: [] });
  }
  let cursor = plan.start, fragmentIndex = 0, breakEnd = 0;
  for (const node of plan.nodes) {
    const range = fileNodeRange(node, cursor, plan.end);
    if (!range || source.slice(cursor, range.start).trim()) return null;
    const start = range.start - plan.start, end = range.end - plan.start;
    while (fragmentIndex < fragments.length && fragments[fragmentIndex].end <= start) fragmentIndex++;
    const fragment = fragments[fragmentIndex];
    if (!fragment || start < fragment.start || end > fragment.end) return null;
    if (node.type === 'paragraph') {
      if (fragment.kind !== 'markdown') return null;
      const fold = folds.get(fragment);
      if (text.slice(fold.coveredUntil, start).trim()) return null;
      const admitted = paragraphBreaks(source, node, range);
      if (!admitted) return null;
      for (const span of admitted) {
        const local = { start: span.start - plan.start, end: span.end - plan.start };
        if (local.start < breakEnd || local.start < fragment.start || local.end > fragment.end) return null;
        fold.breaks.push(local);
        breakEnd = local.end;
      }
      fold.coveredUntil = end;
    } else if (fragment.kind !== node.type || fragment.start !== start) return null;
    cursor = range.end;
  }
  if (source.slice(cursor, plan.end).trim()) return null;
  for (const [fragment, fold] of folds) {
    if (text.slice(fold.coveredUntil, fragment.end).trim()) return null;
  }
  return folds;
}

function foldedSlice(text, fragment, breaks) {
  if (breaks === null) {
    const original = text.slice(fragment.start, fragment.end);
    return fragment.plainFold ? original.replace(/(\S) ?\n(?=\S)/g, '$1 ') : original;
  }
  let result = '', cursor = fragment.start;
  for (const span of breaks) {
    result += text.slice(cursor, span.start) + ' ';
    cursor = span.end;
  }
  return result + text.slice(cursor, fragment.end);
}

/** Final file-preview instructions; all cuts use the unchanged source partition. */
export function fileMarkdownPlan(source, blocks) {
  const plans = matchingProsePlans(source, blocks), result = [];
  for (const [blockIndex, block] of blocks.entries()) {
    if (block.kind === 'code') { result.push({ kind: 'code', blockIndex }); continue; }
    let fragments = proseFragments(block.text);
    const plan = plans.get(blockIndex);
    if (plan?.tables.length) fragments = insertTables(block.text, fragments, plan);
    const folds = plan ? certifiedFolds(source, block.text, fragments, plan) : null;
    for (const fragment of fragments) {
      const { kind, idSuffix } = fragment;
      if (kind === 'table') result.push({ kind, blockIndex, idSuffix, columns: fragment.columns, rows: fragment.rows });
      else if (kind === 'heading') result.push({ kind, blockIndex, idSuffix, text: fragment.text, level: fragment.level });
      else {
        result.push({ kind, blockIndex, idSuffix, text: foldedSlice(block.text, fragment, folds === null ? null : folds.get(fragment).breaks) });
      }
    }
  }
  return result;
}
