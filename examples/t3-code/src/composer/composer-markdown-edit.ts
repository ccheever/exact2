// Source edits from composer-code-block.ts at T3 Code 9c161462c9, and list continuation and the
// quote exit (composer-rich-text-doc.ts needsBlankLineAfterQuote) at T3 Code 74541bab17.
import { listOutdentForTab, nextOrderedMarkerText, renumberFollowingItems } from './composer-list-continuation';
import { codeAt, composerLines, composerMarkdown, emptyFenceAt, isClosingFence, parseOpeningFence, type ComposerBlockOrigin, type ComposerCodeBlock } from './composer-markdown';
import type { ComposerRange } from './composer-markdown';

export type ComposerEdit = ComposerRange & { text: string; selection: ComposerRange };
export type ComposerKeyResult = { kind: 'edit'; edit: ComposerEdit } | { kind: 'send' } | { kind: 'default' } | { kind: 'handled' };
export type ComposerKey = { name: string; shift: boolean; command: boolean };
const DEFAULT: ComposerKeyResult = { kind: 'default' };
const HANDLED: ComposerKeyResult = { kind: 'handled' };
export const CODE_BLOCK_INDENT = '  ';

export function applyComposerEdit(source: string, edit: ComposerEdit): string {
  return source.slice(0, edit.start) + edit.text + source.slice(edit.end);
}
function edit(start: number, end: number, text: string, selection: ComposerRange = { start: start + text.length, end: start + text.length }): ComposerKeyResult {
  return { kind: 'edit', edit: { start, end, text, selection } };
}
export { nextOrderedMarkerText };
export function outdentLine(line: string): string {
  return line.startsWith(CODE_BLOCK_INDENT) ? line.slice(2) : /^[\t ]/.test(line) ? line.slice(1) : line;
}
const leadingWhitespace = (line: string) => /^[ \t]*/.exec(line)?.[0] ?? '';

function serializedCode(fence: string, info: string, body: string, closing: string | null): string {
  let longest = 0;
  for (const line of body.split('\n')) {
    if (isClosingFence(line, fence)) longest = Math.max(longest, /^[`~]+/.exec(line)?.[0].length ?? 0);
  }
  if (longest > 0) {
    fence = fence.charAt(0).repeat(longest + 1);
    if (closing !== null) closing = fence;
  }
  // A closed fence keeps a body line, empty or not, as Enter opens one.
  return `${fence}${info}${body || closing !== null ? '\n' + body : ''}${closing === null ? '' : '\n' + closing}`;
}

/** Text at an empty fence's closing line becomes its first body line, with
 *  the closing line's own newline in the same edit. */
export function composerCodeInput(source: string, selection: ComposerRange, text: string, blocks = composerMarkdown(source).blocks): ComposerKeyResult {
  const block = codeAt(blocks, selection), empty = block ? null : emptyFenceAt(blocks, selection), code = block ?? empty;
  if (code) {
    const offset = block ? selection.start - block.body.start : 0;
    const body = block ? source.slice(block.body.start, selection.start) + text + source.slice(selection.end, block.body.end) : text;
    let longest = 0;
    for (const line of body.split('\n')) {
      if (isClosingFence(line, code.fence)) longest = Math.max(longest, /^[`~]+/.exec(line)?.[0].length ?? 0);
    }
    if (longest > 0) {
      const fence = code.fence.charAt(0).repeat(longest + 1);
      const next = `${fence}${code.info}\n${body}${code.close ? '\n' + fence : ''}`;
      const caret = code.start + fence.length + code.info.length + 1 + offset + text.length;
      return edit(code.start, code.end, next, { start: caret, end: caret });
    }
  }
  if (empty && text) return edit(selection.start, selection.end, text + '\n', { start: selection.start + text.length, end: selection.start + text.length });
  return edit(selection.start, selection.end, text);
}

function indentCode(source: string, selection: ComposerRange, block: ComposerCodeBlock & { body: ComposerRange }, out: boolean): ComposerKeyResult {
  if (selection.start === selection.end && !out) return composerCodeInput(source, selection, CODE_BLOCK_INDENT, [block]);
  const content = source.slice(block.body.start, block.body.end);
  const startOffset = selection.start - block.body.start, endOffset = selection.end - block.body.start;
  const lineStart = content.lastIndexOf('\n', Math.max(0, startOffset - 1)) + 1;
  const newline = content.indexOf('\n', Math.max(startOffset, endOffset - 1)), lineEnd = newline < 0 ? content.length : newline;
  const old = content.slice(lineStart, lineEnd).split('\n');
  const lines = old.map(line => out ? outdentLine(line) : line ? CODE_BLOCK_INDENT + line : line);
  const text = lines.join('\n'), previous = old.join('\n');
  if (text === previous) return HANDLED;
  const start = block.body.start + lineStart, end = block.body.start + lineEnd;
  const firstDelta = (lines[0]?.length ?? 0) - (old[0]?.length ?? 0), totalDelta = text.length - previous.length;
  const nextStart = Math.max(start, selection.start + firstDelta);
  return edit(start, end, text, { start: nextStart, end: Math.max(nextStart, selection.end + totalDelta) });
}

/** Contract cancels an admitted key synchronously, then applies this guarded edit. */
export function composerMarkdownKey(source: string, selection: ComposerRange, key: ComposerKey, logicalFence?: string, blockOrigins?: readonly ComposerBlockOrigin[], chips: readonly ComposerRange[] = []): ComposerKeyResult {
  if (!Number.isInteger(selection.start) || !Number.isInteger(selection.end) || selection.start < 0 || selection.end < selection.start || selection.end > source.length) return DEFAULT;
  const parsed = composerMarkdown(source, { blockOrigins }), block = codeAt(parsed.blocks, selection), collapsed = selection.start === selection.end;
  if (key.name === 'Enter' && key.command) return { kind: 'send' };
  const empty = block ? null : emptyFenceAt(parsed.blocks, selection);
  if (empty) {
    // A caret at an empty fence's closing line stands for its empty code: Tab
    // and Enter open the body line, and Backspace unwraps the fence as at any
    // body start, leaving its one empty line.
    if (key.name === 'Tab') return key.shift ? HANDLED : composerCodeInput(source, selection, CODE_BLOCK_INDENT, parsed.blocks);
    if (key.name === 'Enter') return composerCodeInput(source, selection, '\n', parsed.blocks);
    return key.name === 'Backspace' ? edit(empty.start, empty.end, '', { start: empty.start, end: empty.start }) : DEFAULT;
  }
  if (block) {
    if (key.name === 'Tab') return indentCode(source, selection, block, key.shift);
    if (key.name === 'Backspace' && collapsed && selection.start === block.body.start) {
      return edit(block.start, block.end, source.slice(block.body.start, block.body.end), { start: block.start, end: block.start });
    }
    if (key.name !== 'Enter') return DEFAULT;
    const content = source.slice(block.body.start, block.body.end);
    const offset = selection.start - block.body.start, lineStart = content.lastIndexOf('\n', offset - 1) + 1;
    const newline = content.indexOf('\n', offset), lineEnd = newline < 0 ? content.length : newline;
    const line = content.slice(lineStart, lineEnd), fence = logicalFence ?? block.fence;
    if (collapsed && offset === lineEnd && isClosingFence(line, fence)) {
      const body = content.slice(0, Math.max(0, lineStart - 1));
      const rest = newline < 0 ? '' : content.slice(newline + 1);
      const closed = serializedCode(fence, block.info, body, line), text = closed + '\n' + (rest ? '\n' + rest : '');
      const caret = block.start + closed.length + 1;
      return edit(block.start, block.end, text, { start: caret, end: caret });
    }
    const trailing = collapsed && selection.start === block.body.end ? /\n[ \t]*\n[ \t]*$/.exec(content) : null;
    if (trailing) {
      const body = content.slice(0, -trailing[0].length), closing = block.close ? source.slice(block.close.start, block.close.end) : block.fence;
      return edit(block.start, block.end, serializedCode(fence, block.info, body, closing) + '\n');
    }
    const before = source.slice(block.body.start, selection.start), current = before.slice(before.lastIndexOf('\n') + 1);
    return composerCodeInput(source, selection, '\n' + leadingWhitespace(current), parsed.blocks);
  }
  const line = composerLines(source).find(line => selection.start >= line.start && selection.start <= line.end);
  if (!line) return DEFAULT;
  if (key.name === 'Enter') {
    const closing = parsed.blocks.find(block => block.kind === 'code' && block.close?.end === selection.start);
    if (collapsed && closing) return edit(selection.start, selection.end, '\n');
    const opening = collapsed && selection.start === line.end ? parseOpeningFence(line.text) : null;
    // The body line is empty, not absent, so text typed there is the field's own and stays inside the fence.
    if (opening) {
      const text = `${opening.fence}${opening.info}\n\n${opening.fence}`, caret = line.start + opening.fence.length + opening.info.length + 1;
      return edit(line.start, line.end, text, { start: caret, end: caret });
    }
    if (!key.shift) return { kind: 'send' };
  }
  const current = parsed.blocks.find(block => block.start === line.start);
  if (current?.kind === 'list') {
    if (key.name === 'Tab' && !key.shift && collapsed) return edit(line.start, line.start, CODE_BLOCK_INDENT, { start: selection.start + 2, end: selection.end + 2 });
    // ⇧Tab moves a nested item out (listOutdentForTab); on a top-level item it toggles plan mode or moves the focus back.
    if (key.name === 'Tab' && key.shift) {
      const outdent = listOutdentForTab(source, selection.start, selection.end, chips);
      return outdent ? { kind: 'edit', edit: outdent } : DEFAULT;
    }
    if (key.name === 'Enter' && key.shift && selection.start >= current.contentStart) {
      // A marker typed without its space ("2." alone) is still text: Enter adds a line.
      if (!/[ \t]$/.test(current.prefix)) return edit(selection.start, selection.end, '\n');
      if (!source.slice(current.contentStart, current.end).trim()) return edit(current.start, Math.max(selection.end, current.contentStart), '');
      const marker = current.list === 'task' ? '- [ ]' : current.list === 'ordered' ? nextOrderedMarkerText(current.marker) : current.marker;
      const insertion = `\n${current.indent}${marker} `, ordered = /^(\d+)([.)])$/.exec(current.marker);
      // The rich reference numbers an ordered list's items by position, so the items below a new one follow it.
      const renumbered = ordered && selection.end <= current.end
        ? renumberFollowingItems(source, current.end, { kind: 'ordered', indent: current.indent, numberText: ordered[1] ?? '', delimiter: ordered[2] === ')' ? ')' : '.', space: ' ' }, chips) : null;
      if (renumbered) {
        const caret = selection.start + insertion.length;
        return edit(selection.start, renumbered.end, insertion + source.slice(selection.end, current.end) + renumbered.text, { start: caret, end: caret });
      }
      return edit(selection.start, selection.end, insertion);
    }
  }
  if (current?.kind === 'quote' && key.name === 'Enter' && key.shift && selection.start >= current.contentStart) {
    if (source.slice(current.contentStart, current.end).trim()) return edit(selection.start, selection.end, '\n' + current.prefix);
    // Leaving a quote keeps a blank line after it: a line right below a quote is a lazy continuation that joins it.
    const afterQuote = parsed.blocks.some(block => block.kind === 'quote' && block.end === current.start - 1);
    return edit(current.start, Math.max(selection.end, current.contentStart), afterQuote ? '\n' : '');
  }
  return key.name === 'Enter' && key.shift ? edit(selection.start, selection.end, '\n') : DEFAULT;
}
