// What Markdown's code leaves alone, for the rewrites a message's text goes through on its way to and
// from the composer (`compose.ts`, `render.ts`).

// The parser's own rules (`markdown.ts`): a fence at a line's start closes at the next three backticks;
// inline code is a run of backticks closed by a run as long, which may be shorter than the run that
// opened it.
const FENCE = /^```(?:[\w+.#-]+[ \t]*\n)?[\s\S]*?```/;
const INLINE = /^(`+)([\s\S]*?[^`])\1(?!`)/;

/** Applies `plain` to the parts of Markdown outside code, as the parser finds code; a backslash
 * escapes a backtick, and a run nothing closes is text. */
export function outsideCode(text: string, plain: (part: string) => string): string {
  let out = '', from = 0;
  for (let i = 0; i < text.length;) {
    if (text[i] === '\\') { i += 2; continue; }
    if (text[i] !== '`') { i++; continue; }
    const rest = text.slice(i);
    const code = (i === 0 || text[i - 1] === '\n') && rest.startsWith('```') ? FENCE.exec(rest) ?? INLINE.exec(rest) : INLINE.exec(rest);
    if (!code) { while (text[i] === '`') i++; continue; }
    out += plain(text.slice(from, i)) + code[0];
    i = from = i + code[0].length;
  }
  return out + plain(text.slice(from));
}
