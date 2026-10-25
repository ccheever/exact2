// What Markdown's code leaves alone, for the rewrites a message's text goes through on its way to and
// from the composer (`compose.ts`, `render.ts`).

/** Applies `plain` to the parts of Markdown outside code: a span opens with a run of backticks and
 * closes at the next run of the same length (a fence is such a run of three or more), as Discord's
 * Markdown has it; an unclosed run is text. */
export function outsideCode(text: string, plain: (part: string) => string): string {
  let out = '', from = 0;
  for (let i = 0; i < text.length;) {
    if (text[i] !== '`') { i++; continue; }
    let run = i;
    while (text[run] === '`') run++;
    const ticks = text.slice(i, run);
    let close = run;
    for (;;) {
      close = text.indexOf(ticks, close);
      if (close < 0 || text[close + ticks.length] !== '`') break;
      while (text[close] === '`') close++;
    }
    if (close < 0) { i = run; continue; }
    out += plain(text.slice(from, i)) + text.slice(i, close + ticks.length);
    i = from = close + ticks.length;
  }
  return out + plain(text.slice(from));
}
