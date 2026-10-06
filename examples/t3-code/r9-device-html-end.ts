// Lane r9-device: where Shiki's html grammar ends an embedded script or style sheet (MIT reference,
// see LICENSE-T3: the reference colours source with Shiki 4.2's html grammar, tags-valid). The
// grammar's `source.js` region ends at `(?=</(?i:script))` and its `source.css` region at
// `(?=</(?i:style))`, but TextMate checks a region's end only while no inner rule is open, so a
// closing tag inside a string, a template, a regular expression, an unclosed bracket, a `var` /
// `let` / `const` or `import` / `export` declaration still on its line, or an open ternary is
// JavaScript text, and the script runs on. Its own `//` and `/*` comments end early only at a
// lower-case `</script`. The end tag itself must be `</script>` exactly (any case, no space): after
// a `</script >` or `</scriptx>` the `<` is punctuation and the rest stays plain text until the next
// `/script>`. A style sheet closes at `</style\s*>` (any case) outside comments, strings and blocks.
import type { Token } from './timeline-highlight';
import { regexIslands, regexLiteral, regexMayStart, spliced, withRegexLiterals, type Island } from './r10-device-html-regex'; // lane r10-device: regular expression literals
import { ifStatementBegins, inImportLine } from './r11-misc-ts-decl'; // lane r11-misc: if statements and import lines

const SCRIPT_TAG = /<\/script/iy;
const REGEX_BEFORE = /(?:[=(:,[?+!&|;{}~^%*>-]|=>|\breturn|\bcase|\btypeof|\bvoid|\bdelete|\bin|\bof|\bnew|^)\s*$/;

/** The index where a script body that starts at `start` ends (its `</script`), or the text's end. */
export function scriptBodyEnd(text: string, start: number): number {
  const stack: string[] = []; // ( [ { and "${" (a template expression)
  let declaration = false; // a var / let / const / import / export declaration open on this line
  let ternary = 0;
  let ifOpen = false; // lane r11-misc: an if-statement rule at the top level, open to `;`, `}` or the line's end
  let index = start;
  const lineEnd = (at: number) => { const n = text.indexOf('\n', at); return n < 0 ? text.length : n; };
  while (index < text.length) {
    const character = text[index]!;
    if (ifOpen && stack.length === 0 && (character === '\n' || character === ';' || character === '}')) ifOpen = false;
    if (character === '\n') { declaration = false; index++; continue; }
    if (character === '<' && stack.length === 0 && !declaration && ternary === 0 && !ifOpen) {
      SCRIPT_TAG.lastIndex = index;
      if (SCRIPT_TAG.test(text)) return index;
    }
    if (character === '/' && text[index + 1] === '/') {
      // The html grammar's line comment: to the line's end or a lower-case `</script`.
      const end = lineEnd(index), close = text.slice(index, end).indexOf('</script');
      // lane r11-misc: inside an open rule the javascript grammar's own comment runs to the line's end, over the tag.
      if (close >= 0 && stack.length === 0 && !declaration && ternary === 0 && !ifOpen) return index + close;
      index = end; continue;
    }
    if (character === '/' && text[index + 1] === '*') {
      const end = text.indexOf('*/', index + 2), close = text.indexOf('</script', index + 2);
      if (close >= 0 && (end < 0 || close < end) && stack.length === 0 && !declaration && ternary === 0 && !ifOpen) return close;
      index = end < 0 ? text.length : end + 2; continue;
    }
    if (character === '"' || character === "'") {
      let at = index + 1;
      while (at < text.length && text[at] !== character && text[at] !== '\n') at += text[at] === '\\' ? 2 : 1;
      index = at < text.length && text[at] === character ? at + 1 : at; continue;
    }
    if (character === '`') { stack.push('`'); index++; index = template(text, index, stack); continue; }
    if (character === '/' && REGEX_BEFORE.test(text.slice(Math.max(start, text.lastIndexOf('\n', index - 1) + 1), index))) {
      const end = regexEnd(text, index);
      if (end > 0) { index = end; continue; }
    }
    if (character === '(' || character === '[' || character === '{') { stack.push(character); index++; continue; }
    if (character === ')' || character === ']' || character === '}') {
      const top = stack[stack.length - 1];
      if (top === (character === ')' ? '(' : character === ']' ? '[' : '{')) stack.pop();
      else if (top === '${' && character === '}') { stack.pop(); index++; index = template(text, index, stack); continue; }
      if (character === '}' && stack.length === 0) declaration = false;
      index++; continue;
    }
    if (character === ';' && stack.length === 0) { declaration = false; index++; continue; }
    if (character === '?' && text[index + 1] !== '?' && text[index + 1] !== '.' && text[index - 1] !== '?') { ternary++; index++; continue; }
    if (character === ':' && ternary > 0) { ternary--; index++; continue; }
    const word = /[A-Za-z_$][\w$]*/y;
    word.lastIndex = index;
    const found = word.exec(text);
    if (found) {
      if (/^(?:var|let|const|import|export)$/.test(found[0]) && text[index - 1] !== '.') declaration = true;
      if (found[0] === 'if' && stack.length === 0 && ifStatementBegins(text, index)) ifOpen = true;
      index += found[0].length; continue;
    }
    index++;
  }
  return text.length;
}

/** Inside a template literal (its opening backtick is on the stack): to its closing backtick or a `${`. */
function template(text: string, index: number, stack: string[]): number {
  while (index < text.length) {
    if (text[index] === '\\') { index += 2; continue; }
    if (text[index] === '`') { stack.pop(); return index + 1; }
    if (text.startsWith('${', index)) { stack.push('${'); return index + 2; }
    index++;
  }
  return index;
}

/** A regular expression literal's end on its own line, or -1 when the slash does not start one. */
function regexEnd(text: string, index: number): number {
  if (text[index + 1] === '/' || text[index + 1] === '*') return -1;
  let at = index + 1, inClass = false;
  while (at < text.length && text[at] !== '\n') {
    const character = text[at]!;
    if (character === '\\') { at += 2; continue; }
    if (character === '[') inClass = true;
    else if (character === ']') inClass = false;
    else if (character === '/' && !inClass) {
      at++;
      while (at < text.length && /[dgimsuvy]/.test(text[at]!)) at++;
      return at;
    }
    at++;
  }
  return -1;
}

/** The index where a style sheet that starts at `start` ends (its `</style`), or the text's end. */
export function styleBodyEnd(text: string, start: number): number {
  let depth = 0, index = start;
  const tag = /<\/style/iy;
  while (index < text.length) {
    const character = text[index]!;
    if (character === '<' && depth === 0) { tag.lastIndex = index; if (tag.test(text)) return index; }
    if (text.startsWith('/*', index)) { const end = text.indexOf('*/', index + 2); index = end < 0 ? text.length : end + 2; continue; }
    if (character === '"' || character === "'") {
      let at = index + 1;
      while (at < text.length && text[at] !== character && text[at] !== '\n') at += text[at] === '\\' ? 2 : 1;
      index = at < text.length && text[at] === character ? at + 1 : at; continue;
    }
    if (character === '{') depth++;
    if (character === '}') depth = Math.max(0, depth - 1);
    index++;
  }
  return text.length;
}

/** The closing tag at `index` (a `</script` or `</style` the body ended at), as Shiki paints it,
 *  and the index after it. A malformed close leaves the rest plain until the grammar's end match. */
export function closingTag(text: string, index: number, element: 'script' | 'style'): { tokens: Token[]; end: number } {
  const exact = element === 'script' ? /<\/(script)>/iy : /<\/(style)(\s*)>/iy;
  exact.lastIndex = index;
  const found = exact.exec(text);
  if (found) {
    return { tokens: [{ text: '</', cls: 'punct' }, { text: found[1]!, cls: 'tag' }, { text: (found[2] ?? '') + '>', cls: 'punct' }], end: index + found[0].length };
  }
  const resume = (element === 'script' ? /\/(script)>/ig : /<\/(style)\s*>/ig);
  resume.lastIndex = index + 1;
  const next = resume.exec(text);
  if (!next) return { tokens: [{ text: '<', cls: 'punct' }, { text: text.slice(index + 1), cls: '' }], end: text.length };
  const tail = element === 'script'
    ? [{ text: '/', cls: 'punct' as const }, { text: next[1]!, cls: 'tag' as const }, { text: '>', cls: 'punct' as const }]
    : [{ text: '</', cls: 'punct' as const }, { text: next[1]!, cls: 'tag' as const }, { text: next[0].slice(2 + next[1]!.length), cls: 'punct' as const }];
  return { tokens: [{ text: '<', cls: 'punct' }, { text: text.slice(index + 1, next.index), cls: '' }, ...tail], end: next.index + next[0].length };
}

// --- JSX: Shiki's javascript grammar reads `<name …>` in expression position as a JSX element
// (a closing tag mistaken for script text often runs into one). Brackets and child text are
// punctuation, tag names tags (capitalised or dotted names types), attribute names attributes,
// `=` an operator, `{` / `}` keywords around JavaScript.
const JSX_BEFORE = /(?:[({[,?=>:*]|&&|\|\||\*\/|(?:^|[^._$\w])(?:await|return|default|yield))\s*$|^\s*$/;
const JSX_NAME = /<\s*([_$A-Za-z][-_$\w.]*(?::[_$A-Za-z][-_$\w.]*)?)(?=\s|\/?>|<)/y;
const JSX_CLOSE = /<\/\s*([_$A-Za-z][-_$\w.:]*)?\s*>/y;

type Script = (text: string) => Token[];
const jsxName = (name: string): Token[] => name.split(/(:)/).map(part => ({ text: part, cls: part === ':' ? 'punct' as const : /^[A-Z]|\./.test(part) ? 'type' as const : 'tag' as const }));

/** Whether a JSX element starts at `index` (a `<`) in this script text. */
export function jsxStarts(text: string, index: number): boolean {
  if (text[index] !== '<') return false;
  JSX_NAME.lastIndex = index;
  if (!JSX_NAME.test(text)) return false;
  const lineStart = text.lastIndexOf('\n', index - 1) + 1;
  return JSX_BEFORE.test(text.slice(lineStart, index)) && !inImportLine(text, index);
}

/** A JavaScript `{…}` expression from its `{`: the index after its matching `}` (or the end). */
function braceEnd(text: string, index: number): number {
  let depth = 0;
  for (let at = index; at < text.length; at++) {
    const character = text[at]!;
    if (character === '"' || character === "'" || character === '`') {
      let end = at + 1;
      while (end < text.length && text[end] !== character) end += text[end] === '\\' ? 2 : 1;
      at = end; continue;
    }
    if (character === '{') depth++;
    if (character === '}' && --depth === 0) return at + 1;
  }
  return text.length;
}

/** One JSX element from its `<`: its tokens and the index after it. */
export function jsxElement(text: string, index: number, script: Script): { tokens: Token[]; end: number } {
  const tokens: Token[] = [];
  const add = (value: string, cls: Token['cls']) => { if (value) tokens.push({ text: value, cls }); };
  const expression = (from: number) => { const end = braceEnd(text, from); add('{', 'kw'); const inner = text.slice(from + 1, Math.max(from + 1, end - 1)); tokens.push(...script(inner)); if (text[end - 1] === '}' && end - 1 > from) add('}', 'kw'); return end; };
  JSX_NAME.lastIndex = index;
  const open = JSX_NAME.exec(text)!;
  add(open[0].slice(0, open[0].length - open[1]!.length), 'punct'); tokens.push(...jsxName(open[1]!));
  let at = index + open[0].length;
  // Attributes up to `>` or `/>`.
  for (;;) {
    if (at >= text.length) return { tokens, end: at };
    if (text.startsWith('/>', at)) { add('/>', 'punct'); return { tokens, end: at + 2 }; }
    if (text[at] === '>') { add('>', 'punct'); at++; break; }
    const space = /\s+/y; space.lastIndex = at;
    const gap = space.exec(text);
    if (gap) { add(gap[0], 'punct'); at += gap[0].length; continue; }
    if (text[at] === '{') { at = expression(at); continue; }
    if (text[at] === '"' || text[at] === "'") { const close = text.indexOf(text[at]!, at + 1), end = close < 0 ? text.length : close + 1; add(text.slice(at, end), 'str'); at = end; continue; }
    if (text[at] === '=') { add('=', 'op'); at++; continue; }
    const attr = /[_$A-Za-z][-_$\w:]*/y; attr.lastIndex = at;
    const name = attr.exec(text);
    if (name) { add(name[0], 'attr'); at += name[0].length; continue; }
    add(text[at]!, 'punct'); at++;
  }
  // Children up to any closing tag.
  while (at < text.length) {
    JSX_CLOSE.lastIndex = at;
    const close = JSX_CLOSE.exec(text);
    if (close) {
      const name = close[1] ?? '', head = close[0].indexOf(name || '>');
      add(close[0].slice(0, name ? head : close[0].length - 1), 'punct');
      if (name) { tokens.push(...jsxName(name)); add(close[0].slice(head + name.length), 'punct'); } else add('>', 'punct');
      return { tokens, end: at + close[0].length };
    }
    if (text[at] === '{') { at = expression(at); continue; }
    JSX_NAME.lastIndex = at;
    if (text[at] === '<' && JSX_NAME.test(text)) { const child = jsxElement(text, at, script); tokens.push(...child.tokens); at = child.end; continue; }
    const next = text.slice(at + 1).search(/[<{]/), end = next < 0 ? text.length : at + 1 + next;
    add(text.slice(at, end), 'punct'); at = end;
  }
  return { tokens, end: at };
}

/** A script body's tokens: JavaScript, with JSX elements where the grammar reads them. */
export function scriptTokens(body: string, plain: Script): Token[] {
  // lane r10-device: JSX elements and regular expression literals are islands in one pass over the
  // script (r10-device-html-regex.ts spliced), so the text around them keeps its context.
  const script: Script = text => withRegexLiterals(text, plain);
  const islands: Island[] = [];
  let at = 0;
  while (at < body.length) {
    const lt = body.indexOf('<', at);
    if (lt < 0) break;
    if (jsxStarts(body, lt) && !insideLiteral(body, islands.length ? islands[islands.length - 1]!.end : 0, lt)) {
      const element = jsxElement(body, lt, script);
      islands.push({ start: lt, end: element.end, tokens: element.tokens });
      at = element.end;
      continue;
    }
    at = lt + 1;
  }
  return spliced(body, [...islands, ...regexIslands(body, islands)], plain);
}

/** Whether `index` falls inside a string, template or comment that starts after `from`. */
function insideLiteral(text: string, from: number, index: number): boolean {
  let at = from;
  while (at < index) {
    const character = text[at]!;
    if (character === '/' && text[at + 1] === '/') { const end = text.indexOf('\n', at); if (end < 0 || end >= index) return true; at = end; continue; }
    if (character === '/' && text[at + 1] === '*') { const end = text.indexOf('*/', at + 2); if (end < 0 || end + 2 > index) return true; at = end + 2; continue; }
    if (character === '"' || character === "'" || character === '`') {
      let end = at + 1;
      while (end < text.length && text[end] !== character && (character === '`' || text[end] !== '\n')) end += text[end] === '\\' ? 2 : 1;
      if (end >= index) return true;
      at = end + 1; continue;
    }
    if (character === '/' && regexMayStart(text, at)) { const end = regexLiteral(text, at).end; if (end > index) return true; at = end; continue; }
    at++;
  }
  return false;
}
