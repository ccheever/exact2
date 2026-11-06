// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r10-device-html-regex.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r10-device: regular expression literals inside an HTML page's scripts, as Shiki 4.2's
// javascript grammar colours them (MIT reference, see LICENSE-T3: the reference colours source
// with Shiki's html grammar, which embeds source.js; scopes read from Shiki's own tokens with the
// pierre-light / pierre-dark themes). A `/` starts a regular expression where the grammar's
// `regexp` rule begins: not after an identifier, `)`, `]`, `}`, `++`, `--` or `*/` (or after
// `return` / `case`), and only when the rest of the line holds a whole literal: its body, the
// closing `/`, flags, and no identifier right after. Inside it:
//   the slashes str · flags kw · `^` `$` `\b` `\B` kw · backreferences `\1` `\k<` … `>` kw (the
//   name var) · `.` (in a class too) and `\d` `\w` `\s` `\D` `\W` `\S` `\n` `\t` `\r` `\f` `\v` tag · `\x41`
//   `\u00e9` `\cJ` (a `\u{…}` is an escape) and quantifiers (`*` `+` `?` `{n,m}`, lazy `?`) flag · other escapes esc ·
//   groups, `|` and class brackets punct (a named group's name var) · a class's members and
//   ranges flag (`\d` tag, escapes esc) · any other character the `regex` class.
import type { Token } from './timeline-highlight';

type Script = (text: string) => Token[];

const LOOK = /(?:[^\/\\\[\(\)\n]|\\.|\[(?:[^\]\\\n]|\\.)+\]|\((?:[^\)\\\n]|\\.)+\))+\/(?:[dgimsuvy]+|(?![\/*])|(?=\/\*))(?!\s*[a-zA-Z0-9_$])/y;

/** Whether a regular expression literal may begin with the `/` at `index` (`text` starts a line or an expression). */
export function regexMayStart(text: string, index: number): boolean {
  if (text[index] !== '/' || text[index + 1] === '/' || text[index + 1] === '*') return false;
  const before = text.slice(Math.max(0, text.lastIndexOf('\n', index - 1) + 1), index);
  const trimmed = before.replace(/\s+$/, '');
  const keyword = /(?:^|[^._$\w])(?:return|case)$/.test(trimmed);
  const direct = before.endsWith('++') || before.endsWith('--') || before.endsWith('*/') || /[_$\w)\]}]$/.test(before);
  const expression = /(?:[=(:,[?+!]|=>|&&|\|\||\*\/)$/.test(trimmed) && !/(?:\+\+|--|\})$/.test(trimmed);
  if (direct && !keyword && !expression) return false;
  LOOK.lastIndex = index + 1;
  return LOOK.test(text);
}

/** The literal from its opening `/`: its tokens and the index after its flags. */
export function regexLiteral(text: string, index: number): { tokens: Token[]; end: number } {
  const tokens: Token[] = [];
  const add = (value: string, cls: Token['cls']) => {
    if (!value) return;
    const last = tokens[tokens.length - 1];
    if (last && last.cls === cls) last.text += value; else tokens.push({ text: value, cls });
  };
  const sticky = (re: RegExp, at: number) => { re.lastIndex = at; const found = re.exec(text); return found ? found[0] : ''; };
  add('/', 'str');
  let at = index + 1;
  while (at < text.length && text[at] !== '\n') {
    const character = text[at]!;
    if (character === '/') {
      add('/', 'str'); at++;
      const flags = sticky(/[dgimsuvy]*/y, at);
      add(flags, 'kw');
      return { tokens, end: at + flags.length };
    }
    if (character === '[') {
      const open = text[at + 1] === '^' ? '[^' : '[';
      add(open, 'punct'); at += open.length;
      while (at < text.length && text[at] !== ']' && text[at] !== '\n') {
        const range = sticky(/(?:[^\n]|\\(?:[0-7]{3}|x[\da-fA-F]{2}|u[\da-fA-F]{4})|\\c[A-Z]|\\.)-(?:[^\]\\\n]|\\(?:[0-7]{3}|x[\da-fA-F]{2}|u[\da-fA-F]{4})|\\c[A-Z]|\\.)/y, at);
        if (range) { add(range, 'flag'); at += range.length; continue; }
        const escape = sticky(/\\(?:[dDwWsSnrtfv]|[0-7]{3}|x[\da-fA-F]{2}|u[\da-fA-F]{4}|c[A-Z]|[^\n])/y, at);
        if (escape) { add(escape, /^\\[dDwWsSnrtfv]$/.test(escape) ? 'tag' : /^\\(?:[0-7]{3}|x|u|c)/.test(escape) ? 'flag' : 'esc'); at += escape.length; continue; }
        add(text[at]!, text[at] === '.' ? 'tag' : 'flag'); at++;
      }
      if (text[at] === ']') { add(']', 'punct'); at++; }
      continue;
    }
    if (character === '\\') {
      const backref = sticky(/\\(?:[1-9]\d*|k<)/y, at);
      if (backref) {
        add(backref, 'kw'); at += backref.length;
        if (backref === '\\k<') { const name = sticky(/[\w$]+/y, at); add(name, 'var'); at += name.length; if (text[at] === '>') { add('>', 'kw'); at++; } }
        continue;
      }
      const escape = sticky(/\\(?:[bB]|[dDwWsSnrtfv]|x[\da-fA-F]{2}|u[\da-fA-F]{4}|c[A-Z]|[^\n])/y, at);
      add(escape, /^\\[bB]$/.test(escape) ? 'kw' : /^\\[dDwWsSnrtfv]$/.test(escape) ? 'tag' : /^\\(?:x|u[\da-fA-F]|c[A-Z])/.test(escape) ? 'flag' : 'esc');
      at += escape.length || 1;
      continue;
    }
    const group = sticky(/\((?:\?(?::|=|!|<=|<!))?/y, at);
    if (group) {
      const named = sticky(/\(\?<(?![=!])/y, at);
      if (named) { add(named, 'punct'); at += named.length; const name = sticky(/[\w$]+/y, at); add(name, 'var'); at += name.length; if (text[at] === '>') { add('>', 'punct'); at++; } continue; }
      add(group, 'punct'); at += group.length; continue;
    }
    if (character === ')' || character === '|') { add(character, 'punct'); at++; continue; }
    if (character === '^' || character === '$') { add(character, 'kw'); at++; continue; }
    if (character === '.') { add('.', 'tag'); at++; continue; }
    const quantifier = sticky(/(?:[*+?]|\{\d+(?:,\d*)?\})\??/y, at);
    if (quantifier) { add(quantifier, 'flag'); at += quantifier.length; continue; }
    add(character, 'regex'); at++;
  }
  return { tokens, end: at };
}

export type Island = { start: number; end: number; tokens: Token[] };

/** The regular expression literals in a script (outside its strings, templates, comments and the
 *  given islands). */
export function regexIslands(text: string, skip: readonly Island[] = []): Island[] {
  const islands: Island[] = [];
  let at = 0;
  while (at < text.length) {
    const inside = skip.find(island => island.start <= at && at < island.end);
    if (inside) { at = inside.end; continue; }
    const character = text[at]!;
    if (character === '/' && text[at + 1] === '/') { const end = text.indexOf('\n', at); at = end < 0 ? text.length : end; continue; }
    if (character === '/' && text[at + 1] === '*') { const end = text.indexOf('*/', at + 2); at = end < 0 ? text.length : end + 2; continue; }
    if (character === '"' || character === "'" || character === '`') {
      let end = at + 1;
      while (end < text.length && text[end] !== character && (character === '`' || text[end] !== '\n')) end += text[end] === '\\' ? 2 : 1;
      at = end + 1; continue;
    }
    if (character === '/' && regexMayStart(text, at)) {
      const literal = regexLiteral(text, at);
      islands.push({ start: at, end: literal.end, tokens: literal.tokens });
      at = literal.end; continue;
    }
    at++;
  }
  return islands;
}

/** The script's tokens with each island's own: the script is tokenized once over the whole text with
 *  every island masked as a string of the same length (line breaks kept), so what surrounds an island
 *  keeps its context (an open ternary's `:`, a declaration), then the islands' tokens replace it. */
export function spliced(text: string, islands: readonly Island[], script: Script): Token[] {
  if (!islands.length) return script(text);
  const sorted = [...islands].sort((a, b) => a.start - b.start);
  let masked = '', from = 0;
  for (const island of sorted) {
    const body = text.slice(island.start, island.end), quote = body.includes('\n') ? '`' : '"';
    const inner = body.slice(1, -1).replace(/[^\n]/g, 'x');
    masked += text.slice(from, island.start) + (body.length >= 2 ? quote + inner + quote : body.replace(/./g, 'x'));
    from = island.end;
  }
  masked += text.slice(from);
  const tokens: Token[] = [];
  const push = (value: string, cls: Token['cls']) => {
    if (!value) return;
    const last = tokens[tokens.length - 1];
    if (last && last.cls === cls) last.text += value; else tokens.push({ text: value, cls });
  };
  let offset = 0, next = 0;
  for (const token of script(masked)) {
    let start = offset;
    const end = offset + token.text.length;
    while (start < end) {
      const island = sorted[next];
      if (island && island.start <= start) {
        if (start === island.start) for (const part of island.tokens) push(part.text, part.cls);
        start = Math.min(end, island.end);
        if (start >= island.end) next++;
        continue;
      }
      const stop = island ? Math.min(end, island.start) : end;
      push(text.slice(start, stop), token.cls);
      start = stop;
    }
    offset = end;
  }
  return tokens;
}

/** A script's tokens with its regular expression literals coloured; the rest is `script`'s. */
export function withRegexLiterals(text: string, script: Script): Token[] {
  return spliced(text, regexIslands(text), script);
}
