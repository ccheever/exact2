// Source-backed blocks from composer-rich-text-doc.ts at T3 Code 9c161462c9.
export type ComposerRange = { start: number; end: number };

export type ComposerLine = ComposerRange & { text: string; after: number };
type TextBlock = ComposerRange & { contentStart: number; prefix: string };
export type ComposerBlock =
  | (TextBlock & { kind: 'paragraph' | 'quote' })
  | (TextBlock & { kind: 'heading'; level: number })
  | (TextBlock & { kind: 'list'; list: 'bullet' | 'ordered' | 'task'; indent: string; marker: string; checked: boolean })
  | (ComposerRange & { kind: 'rule' })
  | (ComposerRange & { kind: 'code'; fence: string; info: string; infoRange: ComposerRange; open: ComposerRange; body: ComposerRange | null; close: ComposerRange | null });
export type ComposerCodeBlock = Extract<ComposerBlock, { kind: 'code' }>;
export type ComposerMarkdown = { blocks: ComposerBlock[] };
// Live gestures can keep a restored Markdown prefix literal, or promote a
// bullet's checkbox body without changing the shared field's source.
export type ComposerBlockOrigin = { start: number; kind: 'paragraph' | 'bullet' | 'task' };

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

function textBlock(line: ComposerLine, origin?: ComposerBlockOrigin['kind']): Exclude<ComposerBlock, { kind: 'code' | 'rule' }> {
  if (origin === 'paragraph') return { kind: 'paragraph', ...line, prefix: '', contentStart: line.start };
  const heading = /^(#{1,6})([ \t]+)([^\n]*)$/.exec(line.text);
  if (heading?.[1] && heading[2]) {
    const prefix = heading[1] + heading[2];
    return { kind: 'heading', level: heading[1].length, ...line, prefix, contentStart: line.start + prefix.length };
  }
  const quote = /^(>[ \t]*)([^\n]*)$/.exec(line.text);
  if (quote?.[1]) return { kind: 'quote', ...line, prefix: quote[1], contentStart: line.start + quote[1].length };
  const task = origin === 'bullet' ? null : (origin === 'task'
    ? /^([ \t]*)[-*+]([ \t]+)\[([ xX])\]([ \t]*)([^\n]*)$/
    : /^([ \t]*)-([ \t]+)\[([ xX])\]([ \t]*)([^\n]*)$/).exec(line.text);
  if (task && (task[4] || !task[5])) {
    const indent = task[1] ?? '', marker = line.text.charAt(indent.length), prefix = `${indent}${marker}${task[2]}[${task[3]}]${task[4]}`;
    return { kind: 'list', ...line, list: 'task', indent, marker, checked: task[3]?.toLowerCase() === 'x', prefix, contentStart: line.start + prefix.length };
  }
  const list = /^([ \t]*)(\d+[.)]|[-*+])([ \t]+|$)/.exec(line.text);
  if (list?.[2]) {
    const indent = list[1] ?? '', marker = list[2], prefix = indent + marker + (list[3] ?? '');
    return { kind: 'list', ...line, list: /^\d/.test(marker) ? 'ordered' : 'bullet', indent, marker, checked: false, prefix, contentStart: line.start + prefix.length };
  }
  return { kind: 'paragraph', ...line, prefix: '', contentStart: line.start };
}

/** Every range points into `source`; parsing never rewrites the stored draft.
 *  The Markdown field paints the source; these blocks drive the app's key
 *  handling and the controls outside the field. */
export function composerMarkdown(source: string, options: { styling?: boolean; blockOrigins?: readonly ComposerBlockOrigin[] } = {}): ComposerMarkdown {
  const blocks: ComposerBlock[] = [];
  const lines = composerLines(source);
  const origins = new Map(options.blockOrigins?.map(origin => [origin.start, origin.kind]));
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index];
    if (!line) continue;
    const origin = origins.get(line.start);
    const opening = options.styling !== false && origin !== 'paragraph' ? parseOpeningFence(line.text) : null;
    if (opening && line.after > line.end) {
      let last = index + 1;
      while (last < lines.length && !isClosingFence(lines[last]?.text ?? '', opening.fence)) last++;
      const close = lines[last] ?? null, end = close?.end ?? source.length;
      // A fence closed on the next line has no body range, so its closing line is not code (emptyFenceAt).
      const body = close?.start === line.after ? null : { start: line.after, end: close ? close.start - 1 : source.length };
      const block: ComposerBlock = { kind: 'code', start: line.start, end, fence: opening.fence, info: opening.info,
        infoRange: { start: line.start + opening.fence.length, end: line.end }, open: { start: line.start, end: line.end },
        body, close: close ? { start: close.start, end: close.end } : null };
      blocks.push(block);
      index = close ? last : lines.length;
      continue;
    }
    if (options.styling !== false && origin === undefined && /^([-*_])(?:[ \t]*\1){2,}[ \t]*\r?$/.test(line.text)) {
      blocks.push({ kind: 'rule', start: line.start, end: line.end });
      continue;
    }
    const block: Exclude<ComposerBlock, { kind: 'code' | 'rule' }> = options.styling === false ? { kind: 'paragraph', ...line, prefix: '', contentStart: line.start } : textBlock(line, origin);
    blocks.push(block);
  }
  return { blocks };
}

export function codeAt(blocks: readonly ComposerBlock[], selection: ComposerRange): (ComposerCodeBlock & { body: ComposerRange }) | null {
  return blocks.find((block): block is ComposerCodeBlock & { body: ComposerRange } => block.kind === 'code' && block.body !== null
    && selection.start >= block.body.start && selection.end <= block.body.end) ?? null;
}

/** The fence without a body line whose closing line starts at the collapsed
 *  caret: where its body would be. Text and keys there open the body line
 *  rather than joining the closing fence. */
export function emptyFenceAt(blocks: readonly ComposerBlock[], selection: ComposerRange): ComposerCodeBlock | null {
  return blocks.find((block): block is ComposerCodeBlock => block.kind === 'code' && block.body === null
    && selection.start === selection.end && selection.start === block.close?.start) ?? null;
}
