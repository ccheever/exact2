// Syntax colour for the changes panel: @pierre/diffs highlights each side of a
// hunk with the same Shiki themes as chat code (pierre-light / pierre-dark), so
// a line keeps the context of the lines around it. Word marks and syntax
// classes overlay into one run list per line.
import { highlight, languageOf, type Token } from './timeline-highlight';
import { shikiLanguage, SHIKI_MAX_CHARS } from './r12-render-highlight';

/** Token runs per line for one side of a hunk, highlighted as one text (up to the highlighter's limit, shiki-residuals). */
export function lineTokens(lines: readonly string[], path: string): Token[][] {
  const language = languageOf(path) || shikiLanguage(path);
  const total = lines.reduce((sum, line) => sum + line.length + 1, 0);
  if (!language || !lines.length || total > SHIKI_MAX_CHARS) return lines.map(line => line ? [{ text: line, cls: '' }] : []);
  const out: Token[][] = [[]];
  // lane r12-render: the path itself picks the grammar, as @pierre/diffs picks it from the file name.
  for (const token of highlight(lines.join('\n'), path)) {
    token.text.split('\n').forEach((part, index) => {
      if (index > 0) out.push([]);
      if (part) out[out.length - 1]!.push({ text: part, cls: token.cls });
    });
  }
  while (out.length < lines.length) out.push([]);
  return out.slice(0, lines.length);
}

export interface Segment { id: string; text: string; mark: boolean; syntax: string }
/** Splits word-mark segments at syntax token boundaries (and vice versa). */
export function overlay(segments: readonly Segment[], tokens: readonly Token[]): Segment[] {
  if (!tokens.length) return [...segments];
  const out: Segment[] = [];
  let tokenIndex = 0, tokenOffset = 0;
  for (const segment of segments) {
    let rest = segment.text;
    while (rest.length) {
      const token = tokens[tokenIndex];
      if (!token) { out.push({ id: String(out.length), text: rest, mark: segment.mark, syntax: segment.syntax }); break; }
      const take = Math.min(rest.length, token.text.length - tokenOffset);
      out.push({ id: String(out.length), text: rest.slice(0, take), mark: segment.mark, syntax: token.cls || segment.syntax });
      rest = rest.slice(take); tokenOffset += take;
      if (tokenOffset >= token.text.length) { tokenIndex++; tokenOffset = 0; }
    }
  }
  // Adjacent runs with the same mark and class merge back into one.
  const merged: Segment[] = [];
  for (const segment of out) {
    const last = merged[merged.length - 1];
    if (last && last.mark === segment.mark && last.syntax === segment.syntax) last.text += segment.text;
    else merged.push({ ...segment, id: String(merged.length) });
  }
  return merged;
}
