// Adapted from T3 Code 7c3b3b3605ef (MIT); see LICENSE-T3.
// composer-text-direction.ts: each composer block reads in the direction of most of its words (textDirection), drawn
// here as the Markdown field's base direction for each of its source lines (`paragraphDirs`, LLP 1045 D11). The
// reference's blocks are its editor's top-level nodes (buildTiptapContent); here they are composerMarkdown's lines
// grouped as that function groups them: a run of quote lines with one marker is one blockquote, a run of list lines
// one list for as long as its root list keeps its kind, and a fence through its closing line one code block.
import { textDirection, type TextDirection } from '../shared/text-direction';
import { composerLines, type ComposerBlock, type ComposerRange } from './composer-markdown';

type ListBlock = Extract<ComposerBlock, { kind: 'list' }>;
type Group = { code: boolean; lines: number[]; texts: string[] };

/** The text whose words count, as proseText reads a node: inline code and chips are spaces. */
function proseText(value: string, range: ComposerRange, chips: readonly ComposerRange[]): string {
  let text = '', at = range.start;
  for (const chip of chips) {
    if (chip.end <= at || chip.start >= range.end) continue;
    text += `${value.slice(at, Math.max(at, chip.start))} `;
    at = Math.max(at, chip.end);
  }
  return withoutInlineCode(text + value.slice(at, range.end));
}

/** parseInlineMarkdown's code spans: one backtick to the next lone one on the line, outside a backslash escape. */
function withoutInlineCode(line: string): string {
  let text = '';
  for (let index = 0; index < line.length;) {
    const char = line[index]!;
    if (char === '\\') { text += line.slice(index, index + 2); index += 2; continue; }
    if (char !== '`') { text += char; index += 1; continue; }
    const run = /^`+/.exec(line.slice(index))![0], close = line.indexOf(run, index + run.length);
    if (run.length === 1 && close > index + 1 && line[close + 1] !== '`') { text += ' '; index = close + 1; continue; }
    text += run;
    index += run.length;
  }
  return text;
}

/** listKey: a root list ends where its kind does (bullet by marker, ordered by its delimiter, task). */
function listKey(block: ListBlock): string {
  return block.list === 'task' ? 'task' : block.list === 'ordered' ? `ordered:${block.marker.slice(-1)}` : `bullet:${block.marker}`;
}

/**
 * Lines inside a list or quote follow the block around them, which keeps the bullets and the quote rule on the same
 * side as the text. A line without letters yet continues the one above, so a new line after Arabic starts on the
 * right. Code blocks stay left to right. One direction per source line, null where no line above has letters.
 */
export function composerTextDirections(value: string, blocks: readonly ComposerBlock[], chips: readonly ComposerRange[] = []): Array<TextDirection | null> {
  const lines = composerLines(value), groups: Group[] = [];
  let line = 0, group: Group | null = null, quote: string | null = null;
  // The open root list's nesting, as buildTiptapContent's stack places each item: a line starts a new root list when
  // the stack is empty (any other block empties it) or its root frame changes kind.
  let stack: Array<{ indent: string; key: string; root: boolean }> = [];
  const open = (code = false): Group => { const next = { code, lines: [], texts: [] }; groups.push(next); return next; };
  for (const block of blocks) {
    // A block covers the lines that start within it: one, or a fence's through its closing line.
    const first = line;
    while (line < lines.length && lines[line]!.start <= block.end) line += 1;
    const covered = Array.from({ length: line - first }, (_, index) => first + index);
    if (block.kind !== 'list') stack = [];
    if (block.kind !== 'quote') quote = null;
    if (block.kind === 'code') { open(true).lines.push(...covered); group = null; continue; }
    if (block.kind === 'quote') {
      if (quote !== block.prefix || !group) group = open();
      quote = block.prefix;
    } else if (block.kind === 'list') {
      const key = listKey(block);
      let root = false;
      for (;;) {
        const top = stack.at(-1);
        if (!top) { stack.push({ indent: block.indent, key, root: true }); root = true; continue; }
        if (top.indent === block.indent) { if (top.key !== key) { top.key = key; root ||= top.root; } break; }
        if (top.indent !== '' && !block.indent.startsWith(top.indent)) {
          if (stack.length > 1) { stack.pop(); continue; }
          top.indent = block.indent;
          if (top.key !== key) { top.key = key; root = true; }
          break;
        }
        stack.push({ indent: block.indent, key, root: false });
        break;
      }
      if (root) group = open();
    } else group = open();
    group!.lines.push(...covered);
    if (block.kind !== 'rule') group!.texts.push(proseText(value, { start: block.contentStart, end: block.end }, chips));
    if (block.kind !== 'quote' && block.kind !== 'list') group = null;
  }
  const directions: Array<TextDirection | null> = lines.map(() => null);
  let previous: TextDirection | null = null;
  for (const each of groups) {
    const direction: TextDirection | null = each.code ? 'ltr' : textDirection(each.texts.join(' ')) ?? previous;
    if (!each.code) previous = direction;
    for (const index of each.lines) directions[index] = direction;
  }
  return directions;
}

/** The field's `paragraphDirs`: one word per source line; a line no direction reaches takes the page's, left to right. */
export function composerParagraphDirs(value: string, blocks: readonly ComposerBlock[], chips: readonly ComposerRange[] = []): string {
  return composerTextDirections(value, blocks, chips).map(direction => direction ?? 'ltr').join(' ');
}
