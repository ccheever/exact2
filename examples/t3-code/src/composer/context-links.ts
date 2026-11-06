// The composer's context links, `[label](t3-context://v1/<kind>/<id>)`, an image's with `!` before it:
// collectComposerContextReferences' CONTEXT_LINK and parseComposerContextHref at T3 Code 7c3b3b3605ef
// (packages/shared/src/composerContextReferences.ts; MIT, see LICENSE-T3), the expression's matches found in one pass.

const CONTEXT_HREF = 't3-context://v1/';
const CONTEXT_ID = /^[a-z0-9_-]{1,128}$/i;
/** A link of a prompt, from `start` to `end`, with its label and its href. */
export type ContextLink = { start: number; end: number; image: boolean; label: string; href: string };

/**
 * The matches of CONTEXT_LINK, `(!?)\[([^\]\n]{0,512})\]\((t3-context:\/\/v1\/[^\s)]{1,200})\)`, leftmost first and
 * none overlapping, found in one pass: the expression reads up to 512 characters after every `[`, so a long line of `![`
 * held a turn for half a second. Each `[` reads where its label would end from `stop`, and each `]` its link once.
 */
export function contextLinks(text: string): ContextLink[] {
  const links: ContextLink[] = [];
  if (!text.includes('](t3-context:')) return links;
  const n = text.length, stop = new Int32Array(n + 1); // the next `]` or line break at or after each index
  stop[n] = n;
  for (let at = n - 1; at >= 0; at--) stop[at] = text[at] === ']' || text[at] === '\n' ? at : stop[at + 1]!;
  const ends = new Map<number, number>(); // by a `]`: the end of the link it closes, or -1
  const linkEnd = (close: number): number => {
    let end = ends.get(close);
    if (end === undefined) {
      end = -1;
      if (text.startsWith(`](${CONTEXT_HREF}`, close)) {
        const from = close + 2 + CONTEXT_HREF.length;
        let at = from;
        while (at < n && at - from < 200 && text[at] !== ')' && !/\s/.test(text[at]!)) at++;
        if (at > from && text[at] === ')') end = at + 1;
      }
      ends.set(close, end);
    }
    return end;
  };
  for (let at = 0; at < n;) {
    const image = text[at] === '!' && text[at + 1] === '[', open = image ? at + 1 : at;
    if (text[open] === '[') {
      const close = stop[open + 1]!, end = text[close] === ']' && close - open - 1 <= 512 ? linkEnd(close) : -1;
      if (end > 0) {
        links.push({ start: at, end, image, label: text.slice(open + 1, close), href: text.slice(close + 2, end - 1) });
        at = end;
        continue;
      }
    }
    at++;
  }
  return links;
}

/** parseComposerContextHref: a link's kind and context id, or null where its href names none. */
export function contextTarget(href: string): { kind: string; id: string } | null {
  const parts = href.slice(CONTEXT_HREF.length).split('/');
  return parts.length === 2 && /^[a-z][a-z0-9-]{0,39}$/.test(parts[0]!) && CONTEXT_ID.test(parts[1]!) ? { kind: parts[0]!, id: parts[1]! } : null;
}

/** `text` with each of `links` (ascending, apart) replaced by what `replace` makes of it; null keeps the link as it is. */
export function replaceLinks(text: string, links: readonly ContextLink[], replace: (link: ContextLink) => string | null): string {
  let out = '', cursor = 0;
  for (const link of links) {
    const replacement = replace(link);
    if (replacement === null) continue;
    out += text.slice(cursor, link.start) + replacement;
    cursor = link.end;
  }
  return cursor === 0 ? text : out + text.slice(cursor);
}
