// A small syntax highlighter for chat code blocks and the diff panel. Classes
// map to the built T3 reference's Shiki themes, pierre-light and pierre-dark
// (timeline/markdown and diff contracts: synColor), measured from the scopes
// those themes colour: keywords, declarations and types, function names,
// variables and constants, parameters, strings and escapes, literals,
// operators, punctuation, comments, JSON keys, shell flags and headings.

import { languageIconToken } from './timeline-files';
import { htmlTokens } from './r7-polish-html-syntax';
import { TsStatements, importWord } from './r11-misc-ts-decl';
import { shikiLanguage, shikiTokens } from './r12-render-highlight';

export type Cls = '' | 'kw' | 'decl' | 'type' | 'fn' | 'var' | 'const' | 'param' | 'pyparam' | 'str' | 'esc' | 'num' | 'nul'
  | 'op' | 'punct' | 'com' | 'key' | 'flag' | 'builtin' | 'interp' | 'heading' | 'bold' | 'tag' | 'attr'
  | 'regex' // lane r10-device: a regular expression's plain characters (r10-device-html-regex.ts)
  | 'deco'; // lane r12-render: a decorator (meta.decorator), the one pierre colour pair no other class had
export interface Token { text: string; cls: Cls }

const words = (text: string) => new Set(text.split(' '));
const TS_KEYWORDS = words('import from export default async await return if else for while do switch case break continue new throw try catch finally of in as yield delete typeof instanceof void with satisfies keyof readonly declare abstract public private protected static implements extends super');
const TS_DECL = words('function const let var class interface type enum namespace module get set');
const TS_TYPES = words('string number boolean void any unknown never object symbol bigint undefined');
const TS_LITERALS = words('true false null undefined NaN Infinity');
const PY_KEYWORDS = words('import from as return if elif else for while in not and or is with try except finally raise pass break continue yield await async global nonlocal del assert');
const PY_DECL = words('def class lambda');
const PY_BUILTINS = words('str int float bool list dict tuple set bytes object type print len range open super isinstance');
const GENERIC_KEYWORDS = words('if else for while do switch case break continue return new throw try catch finally import package use mod pub crate impl trait where match loop in as mut ref move unsafe extern static const let var val fn func def struct enum class interface type public private protected internal override final abstract virtual namespace using include defer go select chan map range nil self Self super this extends implements fun when object guard throws init deinit');
const GENERIC_DECL = words('fn func def struct enum class interface type trait impl let var val const fun');
const GENERIC_LITERALS = words('true false null nil None True False undefined');
const SHELL_WORDS = words('if then else elif fi for while do done case esac in function return export local');

const LANGS: Record<string, string> = {
  ts: 'ts', tsx: 'ts', typescript: 'ts', js: 'ts', jsx: 'ts', javascript: 'ts', mjs: 'ts', cjs: 'ts', mts: 'ts', cts: 'ts',
  json: 'json', jsonc: 'json', json5: 'json', bash: 'sh', sh: 'sh', shell: 'sh', zsh: 'sh', console: 'sh', shellscript: 'sh',
  py: 'py', python: 'py', md: 'md', markdown: 'md', mdx: 'md', yaml: 'yaml', yml: 'yaml', toml: 'yaml',
  rs: 'c', rust: 'c', go: 'c', swift: 'c', java: 'c', kt: 'c', kotlin: 'c', c: 'c', h: 'c', cpp: 'c', cc: 'c', hpp: 'c',
  cs: 'c', csharp: 'c', php: 'c', rb: 'c', ruby: 'c', scala: 'c', dart: 'c', zig: 'c', css: 'css', scss: 'css', less: 'css',
  html: 'html', xml: 'html', svg: 'html', vue: 'html', svelte: 'html',
};
/** The grammar a fence language or file name selects, or '' for plain text. */
export function languageOf(nameOrLang: string): string {
  const value = nameOrLang.trim().toLowerCase();
  if (LANGS[value]) return LANGS[value]!;
  const base = value.split('/').pop() ?? value;
  if (base === 'dockerfile' || base === 'makefile') return 'sh';
  const extension = base.includes('.') ? base.split('.').pop()! : '';
  return LANGS[extension] ?? '';
}

function push(tokens: Token[], text: string, cls: Cls) {
  if (!text) return;
  const last = tokens[tokens.length - 1];
  if (last && last.cls === cls) last.text += text; else tokens.push({ text, cls });
}
const IDENT = /[A-Za-z_$][\w$]*/y;
const NUMBER = /(?:0[xX][\da-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?)n?/y;
function at(re: RegExp, text: string, index: number): string | null {
  re.lastIndex = index;
  const match = re.exec(text);
  return match ? match[0] : null;
}
function nextNonSpace(text: string, index: number): string {
  while (index < text.length && (text[index] === ' ' || text[index] === '\t')) index++;
  return text[index] ?? '';
}
function previousSignificant(tokens: Token[]): Token | undefined {
  for (let index = tokens.length - 1; index >= 0; index--) if (tokens[index]!.text.trim()) return tokens[index];
  return undefined;
}
/** A quoted string with escapes marked separately; returns the end index. */
function quoted(tokens: Token[], text: string, start: number, quote: string, escapes = true): number {
  let index = start + 1;
  push(tokens, quote, 'str');
  let run = '';
  while (index < text.length) {
    const character = text[index]!;
    if (escapes && character === '\\' && index + 1 < text.length) {
      push(tokens, run, 'str'); run = '';
      push(tokens, text.slice(index, index + 2), 'esc'); index += 2; continue;
    }
    if (character === quote) { run += character; index++; break; }
    if (character === '\n' && quote !== '`' && quote !== '"""') { break; }
    run += character; index++;
  }
  push(tokens, run, 'str');
  return index;
}

function tsLike(text: string, lang: 'ts' | 'c' | 'css'): Token[] {
  const tokens: Token[] = [];
  const keywords = lang === 'ts' ? TS_KEYWORDS : GENERIC_KEYWORDS, decl = lang === 'ts' ? TS_DECL : GENERIC_DECL;
  const literals = lang === 'ts' ? TS_LITERALS : GENERIC_LITERALS;
  let typeContext = false, paramDepth = -1, depth = 0, declaring = '';
  // lane r10-device: an open ternary's bracket nesting; its `:` is keyword.operator.ternary (kw), as Shiki scopes it.
  let nest = 0; const ternaries: number[] = [];
  // lane r11-misc: declaration lists, object-literal keys and import lines (r11-misc-ts-decl.ts).
  const st = lang === 'ts' ? new TsStatements() : null;
  const lastText = () => { for (let at = tokens.length - 1; at >= 0; at--) { const token = tokens[at]!; if (token.text.trim() && token.cls !== 'com') return token.text.trimEnd(); } return ''; };
  let index = 0;
  while (index < text.length) {
    const rest = text[index]!;
    if (text.startsWith('//', index) || lang !== 'ts' && lang !== 'css' && rest === '#' && (index === 0 || text[index - 1] === '\n') && /^#(?:include|define|if|endif|pragma)/.test(text.slice(index))) {
      const end = text.indexOf('\n', index); const stop = end < 0 ? text.length : end;
      push(tokens, text.slice(index, stop), 'com'); index = stop; continue;
    }
    if (text.startsWith('/*', index)) {
      const end = text.indexOf('*/', index + 2); const stop = end < 0 ? text.length : end + 2;
      push(tokens, text.slice(index, stop), 'com'); index = stop; continue;
    }
    if (rest === '`' && lang === 'ts') { index = template(tokens, text, index); continue; } // lane r7-polish
    if (rest === '"' || rest === "'" || rest === '`') { index = quoted(tokens, text, index, rest); continue; }
    if (rest === '\n' && st) st.newline(lastText());
    if (/\s/.test(rest)) { push(tokens, rest, previousSignificant(tokens)?.cls === 'com' ? '' : tokens[tokens.length - 1]?.cls === 'punct' ? 'punct' : ''); index++; continue; }
    const number = /\d/.test(rest) ? at(NUMBER, text, index) : null;
    if (number) { push(tokens, number, 'num'); index += number.length; continue; }
    const word = at(IDENT, text, index);
    if (word) {
      const before = previousSignificant(tokens), next = nextNonSpace(text, index + word.length);
      let cls: Cls;
      let binding = '';
      if (lang === 'css') cls = next === ':' ? 'key' : before?.text.endsWith('.') || before?.text.endsWith('#') ? 'attr' : 'var';
      else if (st?.importing) cls = importWord(word, keywords);
      else if (literals.has(word)) cls = 'num';
      else if (st && (binding = st.bindingName(lastText(), next))) cls = binding as Cls;
      else if (st && before?.cls === 'punct' && /(?:^|[^.])\.$/.test(before.text) && (keywords.has(word) || decl.has(word))) cls = next === '(' ? 'fn' : 'var'; // a member named like a keyword
      else if (word === 'this' || word === 'self' || lang === 'ts' && word === 'super') cls = 'const';
      // lane r7-polish (Shiki's JS/TS scopes): a constructor is storage.type with parameters, an
      // inherited class entity.other.inherited-class, an arrow function's lone parameter a parameter.
      else if (lang === 'ts' && word === 'constructor' && next === '(') { cls = 'decl'; declaring = 'params'; }
      else if (lang === 'ts' && /^(?:extends|implements)$/.test(before?.text.trim() ?? '') && !keywords.has(word)) cls = 'type';
      else if (lang === 'ts' && next === '=' && /^\s*=>/.test(text.slice(index + word.length))) cls = 'param';
      else if (decl.has(word)) { cls = 'decl'; declaring = word; }
      else if (keywords.has(word)) cls = 'kw';
      else if (declaring === 'function' || declaring === 'fn' || declaring === 'func' || declaring === 'def' || declaring === 'fun' || next === '(' && !typeContext) cls = 'fn';
      else if (declaring === 'class' || declaring === 'interface' || declaring === 'type' || declaring === 'enum' || declaring === 'struct' || declaring === 'trait') cls = 'type';
      else if (lang === 'ts' && (declaring === 'const' || declaring === 'let' || declaring === 'var') && ARROW_VALUE.test(text.slice(index + word.length))) cls = 'fn'; // lane r7-polish
      else if (declaring === 'const' && (next === '=' || st)) cls = 'const';
      else if (typeContext || TS_TYPES.has(word) && lang === 'ts') cls = 'type';
      else if (paramDepth >= 0 && depth === paramDepth && (next === ':' || next === ',' || next === ')' || next === '=') && (!st || st.brackets[st.brackets.length - 1] === '(')) cls = 'param';
      else if (/^[A-Z][A-Z0-9_]+$/.test(word)) cls = 'const';
      else if (/^[A-Z]/.test(word) && lang !== 'ts') cls = 'type';
      else cls = 'var';
      if (cls !== 'decl' && declaring && word !== 'async') declaring = declaring === 'function' && next === '(' ? 'params' : '';
      if (st && cls === 'decl' && (word === 'const' || word === 'let' || word === 'var')) st.beginVar(word);
      if (st && cls === 'decl' && word === 'type' && /[A-Za-z_$]/.test(next)) st.typeAlias = true;
      if (st && word === 'import' && cls === 'kw' && next !== '(' && next !== '.' && text[index - 1] !== '.') st.importing = true;
      if (st && cls === 'kw' && (word === 'case' || word === 'default' && next === ':')) st.caseOpen = true;
      push(tokens, word, cls); index += word.length; continue;
    }
    // Punctuation and operators.
    if (text.startsWith('=>', index)) { push(tokens, '=>', 'decl'); typeContext = false; index += 2; continue; }
    if (rest === '(') {
      if (declaring === 'params' || before(tokens) === 'fn' && previousFunction(tokens) || lang === 'ts' && ARROW_PARAMS.test(text.slice(index))) { paramDepth = depth + 1; declaring = ''; }
      st?.open('(', lastText(), typeContext, declaring);
      depth++; nest++; push(tokens, rest, 'punct'); typeContext = false; index++; continue;
    }
    if (rest === ')') { st?.close(); if (paramDepth === depth) paramDepth = -1; depth = Math.max(0, depth - 1); nest = Math.max(0, nest - 1); while (ternaries.length && ternaries[ternaries.length - 1]! > nest) ternaries.pop(); push(tokens, rest, 'punct'); typeContext = false; index++; continue; }
    if (rest === ':' && lang === 'ts' && ternaries.length && ternaries[ternaries.length - 1] === nest) { ternaries.pop(); push(tokens, rest, 'kw'); typeContext = false; index++; continue; }
    if (rest === ':' && st?.keyColon()) { st.caseOpen = false; push(tokens, rest, 'punct'); typeContext = false; index++; continue; }
    if (rest === ':' && lang === 'ts') {
      const previous = previousSignificant(tokens);
      typeContext = !!previous && (previous.cls === 'param' || previous.cls === 'var' || previous.cls === 'const' || previous.text.endsWith(')'));
      push(tokens, rest, 'punct'); index++; continue;
    }
    if ((rest === '<' || rest === '>') && st?.importing) { push(tokens, rest, ''); index++; continue; }
    if (rest === '<' || rest === '>') {
      const previous = previousSignificant(tokens);
      const generic = lang === 'ts' && previous?.cls === 'type';
      if (generic && rest === '<') typeContext = true;
      const angle = generic || typeContext || rest === '>' && !!st && st.angle > 0; // lane r11-misc: generic brackets
      if (st && angle) st.angle = Math.max(0, st.angle + (rest === '<' ? 1 : -1));
      push(tokens, rest, angle ? 'punct' : 'op'); index++; continue;
    }
    if ('{}[],;.'.includes(rest)) {
      if (st && (rest === '{' || rest === '[')) { st.open(rest, lastText(), typeContext, declaring); if (st.destructure === st.brackets.length - 1) declaring = ''; }
      if (st && (rest === '}' || rest === ']')) st.close();
      if (rest !== '.') typeContext = rest === ',' && typeContext && depth > 0 && paramDepth < 0 ? typeContext : false;
      if (rest === ',' && st) { const next = st.comma(); if (next) declaring = next; }
      if (rest === ';') st?.semicolon();
      if (rest === '{' && declaring && declaring !== 'params') declaring = '';
      if (rest === '{' || rest === '[') nest++;
      if (rest === '}' || rest === ']') { nest = Math.max(0, nest - 1); while (ternaries.length && ternaries[ternaries.length - 1]! > nest) ternaries.pop(); }
      if (rest === ';') ternaries.length = 0;
      push(tokens, rest, 'punct'); index++; continue;
    }
    if ('=+-*/%!&|^~?'.includes(rest)) {
      const operator = /[=+\-*/%!&|^~?]+/y; operator.lastIndex = index;
      const value = operator.exec(text)![0];
      if (st?.importing) { push(tokens, value, value === '*' ? 'num' : ''); index += value.length; continue; }
      if (value === '=' || value.length > 1 || rest !== '?') typeContext = false;
      if (value === '?' && lang === 'ts') ternaries.push(nest);
      push(tokens, value, value === '?' || value === '?.' ? 'kw' : 'op'); index += value.length; continue;
    }
    push(tokens, rest, rest === '@' ? 'fn' : ''); index++;
  }
  return tokens;
}
const ARROW_VALUE = /^\s*=\s*(?:async\s+)?(?:function\b|(?:\((?:[^()]|\([^()]*\))*\)|[A-Za-z_$][\w$]*)\s*(?::[^=;{}()]*)?=>)/;
const ARROW_PARAMS = /^\((?:[^()]|\([^()]*\))*\)\s*(?::[^=;{}()]*)?=>/;
/** lane r7-polish: a template literal as Shiki scopes it: backticks punctuation, `${…}` keyword-coloured around its expression. */
function template(tokens: Token[], text: string, start: number): number {
  push(tokens, '`', 'punct');
  let index = start + 1, run = '';
  while (index < text.length) {
    const character = text[index]!;
    if (character === '\\' && index + 1 < text.length) { push(tokens, run, 'str'); run = ''; push(tokens, text.slice(index, index + 2), 'esc'); index += 2; continue; }
    if (character === '`') { push(tokens, run, 'str'); push(tokens, '`', 'punct'); return index + 1; }
    if (character === '$' && text[index + 1] === '{') {
      push(tokens, run, 'str'); run = '';
      let depth = 1, end = index + 2;
      while (end < text.length) { if (text[end] === '{') depth++; else if (text[end] === '}' && --depth === 0) break; end++; }
      push(tokens, '${', 'kw');
      for (const token of tsLike(text.slice(index + 2, end), 'ts')) push(tokens, token.text, token.cls);
      if (end < text.length) push(tokens, '}', 'kw');
      index = end + 1; continue;
    }
    run += character; index++;
  }
  push(tokens, run, 'str');
  return index;
}
function before(tokens: Token[]): Cls { return previousSignificant(tokens)?.cls ?? ''; }
function previousFunction(tokens: Token[]): boolean {
  const significant = tokens.filter(token => token.text.trim());
  return significant.length >= 2 && significant[significant.length - 2]!.cls === 'decl';
}

function json(text: string): Token[] {
  const tokens: Token[] = [];
  let index = 0;
  while (index < text.length) {
    const character = text[index]!;
    if (character === '"') {
      const end = quoted([], text, index, '"');
      const value = text.slice(index, end);
      push(tokens, value, nextNonSpace(text, end) === ':' ? 'key' : 'str'); index = end; continue;
    }
    if (text.startsWith('//', index)) { const end = text.indexOf('\n', index); const stop = end < 0 ? text.length : end; push(tokens, text.slice(index, stop), 'com'); index = stop; continue; }
    const word = at(/-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null/y, text, index);
    if (word) { push(tokens, word, word === 'null' ? 'nul' : 'num'); index += word.length; continue; }
    push(tokens, character, /\s/.test(character) ? (tokens[tokens.length - 1]?.cls === 'punct' ? 'punct' : '') : 'punct'); index++;
  }
  return tokens;
}

function shell(text: string): Token[] {
  const tokens: Token[] = [];
  let index = 0, commandStart = true;
  while (index < text.length) {
    const character = text[index]!;
    if (character === '#' && (index === 0 || /\s/.test(text[index - 1]!))) {
      const end = text.indexOf('\n', index); const stop = end < 0 ? text.length : end;
      push(tokens, text.slice(index, stop), 'com'); index = stop; continue;
    }
    if (character === '\n' || character === ';') { push(tokens, character, character === ';' ? 'punct' : ''); commandStart = true; index++; continue; }
    if (/\s/.test(character)) { push(tokens, character, tokens[tokens.length - 1]?.cls === 'punct' ? 'punct' : tokens[tokens.length - 1]?.cls ?? ''); index++; continue; }
    const operator = at(/&&|\|\||\||>>|>|<|&/y, text, index);
    if (operator) { push(tokens, operator, 'punct'); commandStart = operator !== '>' && operator !== '>>' && operator !== '<'; index += operator.length; continue; }
    if (character === '$') {
      const variable = at(/\$\{[^}]*\}|\$\(|\$[A-Za-z_][\w]*|\$[0-9@#?*!$-]/y, text, index);
      if (variable) { push(tokens, variable, variable === '$(' ? 'punct' : 'var'); index += variable.length; if (variable === '$(') commandStart = true; continue; }
    }
    if (character === '"') {
      push(tokens, '"', 'str'); index++;
      let run = '';
      while (index < text.length && text[index] !== '"') {
        if (text[index] === '\\' && index + 1 < text.length) { run += text.slice(index, index + 2); index += 2; continue; }
        const variable = text[index] === '$' ? at(/\$\{[^}]*\}|\$[A-Za-z_][\w]*/y, text, index) : null;
        if (variable) { push(tokens, run, 'str'); run = ''; push(tokens, variable, 'var'); index += variable.length; continue; }
        run += text[index]; index++;
      }
      push(tokens, run + (text[index] === '"' ? '"' : ''), 'str'); if (text[index] === '"') index++;
      commandStart = false; continue;
    }
    if (character === "'") { index = quoted(tokens, text, index, "'", false); commandStart = false; continue; }
    const word = at(/[^\s;&|<>"'$#]+/y, text, index);
    if (word) {
      const assignment = /^[A-Za-z_]\w*=/.test(word);
      push(tokens, word, assignment ? 'var' : SHELL_WORDS.has(word) && commandStart ? 'kw' : commandStart ? 'fn' : word.startsWith('-') ? 'flag' : 'str');
      if (!assignment && !(SHELL_WORDS.has(word) && commandStart)) commandStart = false;
      index += word.length; continue;
    }
    push(tokens, character, ''); index++;
  }
  return tokens;
}

function python(text: string): Token[] {
  const tokens: Token[] = [];
  let index = 0, inParams = false, depth = 0, afterDef = false;
  while (index < text.length) {
    const character = text[index]!;
    if (character === '#') { const end = text.indexOf('\n', index); const stop = end < 0 ? text.length : end; push(tokens, text.slice(index, stop), 'com'); index = stop; continue; }
    const prefix = at(/[fFrRbBuU]{1,2}(?=['"])/y, text, index);
    if (prefix || character === '"' || character === "'") {
      const formatted = !!prefix && /f/i.test(prefix);
      if (prefix) { push(tokens, prefix, 'decl'); index += prefix.length; }
      const quote = text.startsWith('"""', index) ? '"""' : text.startsWith("'''", index) ? "'''" : text[index]!;
      const end = text.indexOf(quote, index + quote.length);
      const stop = end < 0 ? text.length : end + quote.length;
      const body = text.slice(index, stop);
      if (!formatted) push(tokens, body, 'str');
      else {
        let cursor = 0;
        for (const match of body.matchAll(/\{([^{}]*)\}/g)) {
          push(tokens, body.slice(cursor, match.index), 'str');
          push(tokens, '{', 'interp'); push(tokens, match[1]!, ''); push(tokens, '}', 'interp');
          cursor = match.index! + match[0].length;
        }
        push(tokens, body.slice(cursor), 'str');
      }
      index = stop; continue;
    }
    if (/\s/.test(character)) { push(tokens, character, tokens[tokens.length - 1]?.cls === 'punct' ? 'punct' : ''); index++; continue; }
    const number = /\d/.test(character) ? at(NUMBER, text, index) : null;
    if (number) { push(tokens, number, 'num'); index += number.length; continue; }
    const word = at(IDENT, text, index);
    if (word) {
      const next = nextNonSpace(text, index + word.length);
      let cls: Cls;
      if (word === 'None' || word === 'True' || word === 'False') cls = 'num';
      else if (word === 'self' || word === 'cls') cls = 'const';
      else if (PY_DECL.has(word)) { cls = 'decl'; afterDef = word === 'def'; }
      else if (PY_KEYWORDS.has(word)) cls = 'kw';
      else if (afterDef) { cls = 'fn'; afterDef = false; }
      else if (inParams && depth === 1 && /[(,]$/.test(previousSignificant(tokens)?.text ?? '') && (next === ':' || next === ',' || next === ')' || next === '=')) cls = 'pyparam';
      else if (PY_BUILTINS.has(word)) cls = next === '(' ? 'fn' : 'builtin';
      else if (next === '(') cls = 'fn';
      else cls = '';
      push(tokens, word, cls); index += word.length; continue;
    }
    if (character === '(') { depth++; if (tokens.length && previousSignificant(tokens)?.cls === 'fn' && tokens.filter(token => token.text.trim()).slice(-2)[0]?.text === 'def') inParams = true; }
    if (character === ')') { depth--; if (depth <= 0) { inParams = false; depth = 0; } }
    if (text.startsWith('->', index)) { push(tokens, '->', 'punct'); index += 2; continue; }
    push(tokens, character, '()[]{},:.;'.includes(character) ? 'punct' : '=+-*/%<>!&|^~@'.includes(character) ? 'op' : ''); index++;
  }
  return tokens;
}

function lines(text: string, each: (line: string) => Token[]): Token[] {
  const tokens: Token[] = [];
  text.split('\n').forEach((line, index) => { if (index) push(tokens, '\n', ''); for (const token of each(line)) push(tokens, token.text, token.cls); });
  return tokens;
}
function markdown(text: string): Token[] {
  let fenced = false;
  return lines(text, line => {
    if (/^\s*(```|~~~)/.test(line)) { fenced = !fenced; return [{ text: line, cls: 'str' }]; }
    if (fenced) return [{ text: line, cls: '' }];
    if (/^#{1,6}\s/.test(line)) return [{ text: line, cls: 'heading' }];
    const tokens: Token[] = [];
    let cursor = 0;
    // r6-polish: a list item's marker (punctuation.definition.list.begin) takes the heading ink in the pierre themes.
    const marker = /^(\s*)([-*+]|\d+[.)])(?=\s)/.exec(line);
    if (marker) { push(tokens, marker[1]!, ''); push(tokens, marker[2]!, 'heading'); cursor = marker[0].length; }
    const start = cursor;
    for (const match of line.slice(start).matchAll(/(\*\*[^*]+\*\*|__[^_]+__)|(`[^`]+`)/g)) {
      push(tokens, line.slice(cursor, start + match.index!), '');
      push(tokens, match[0], match[1] ? 'bold' : 'str');
      cursor = start + match.index! + match[0].length;
    }
    push(tokens, line.slice(cursor), '');
    return tokens;
  });
}
function yaml(text: string): Token[] {
  return lines(text, line => {
    const comment = /(^|\s)#.*$/.exec(line);
    const body = comment ? line.slice(0, comment.index + comment[1]!.length) : line;
    const tokens: Token[] = [];
    const key = /^(\s*-?\s*)([\w.\-"' ]+?)(\s*[:=])(\s|$)/.exec(body);
    if (key) {
      push(tokens, key[1]!, 'punct'); push(tokens, key[2]!, 'tag'); push(tokens, key[3]!, 'punct');
      const value = body.slice(key[0].length - key[4]!.length);
      push(tokens, value, /^\s*(true|false|null|~|-?\d[\d.]*)\s*$/.test(value) ? 'num' : value.trim() ? 'str' : '');
    } else if (/^\s*\[.*\]\s*$/.test(body)) push(tokens, body, 'tag');
    else push(tokens, body, body.trim().startsWith('-') ? 'str' : '');
    if (comment) push(tokens, line.slice(body.length), 'com');
    return tokens;
  });
}
/** Tokens for one code block or diff line; unknown languages stay one plain token. */
export function highlight(text: string, language: string): Token[] {
  const grammar = languageOf(language);
  if (!text) return [];
  // lane r12-render: the reference's own grammars and themes where they are here (r12-render-highlight.ts).
  const shiki = shikiTokens(text, language);
  if (shiki) return shiki;
  switch (grammar) {
    case 'ts': return tsLike(text, 'ts');
    case 'c': return tsLike(text, 'c');
    case 'css': return tsLike(text, 'css');
    case 'json': return json(text);
    case 'sh': return shell(text);
    case 'py': return python(text);
    case 'md': return markdown(text);
    case 'yaml': return yaml(text);
    case 'html': return htmlTokens(text, script => tsLike(script, 'ts')); // lane r7-polish: Shiki's html grammar
    default: return [{ text, cls: '' }];
  }
}

/** Every fenced code block of a message with its tokens, matched in Contract by its text. */
type Highlighted = { id: string; code: string; icon: string; tokens: { id: string; text: string; cls: string }[] };
const cache = new Map<string, Highlighted[]>();
export function messageCodeBlocks(markdownText: string): Highlighted[] {
  if (!markdownText.includes('```') && !markdownText.includes('~~~')) return [];
  const known = cache.get(markdownText);
  if (known) return known;
  const blocks: Highlighted[] = [];
  const fence = /^( {0,3})(`{3,}|~{3,})([^\n`]*)\n([\s\S]*?)\n? {0,3}\2[`~]*[ \t]*(?:\n|$)/gm;
  for (const match of markdownText.matchAll(fence)) {
    const indent = match[1]!.length, language = match[3]!.trim().split(/\s+/)[0] ?? '';
    const code = (indent ? match[4]!.split('\n').map(line => line.replace(new RegExp(`^ {0,${indent}}`), '')).join('\n') : match[4]!).replace(/\n+$/, '');
    if (blocks.some(block => block.code === code)) continue;
    const tokens = languageOf(language) || shikiLanguage(language) ? highlight(code, language) : [{ text: code, cls: '' as Cls }];
    blocks.push({ id: String(blocks.length), code, icon: languageIconToken(language),
      tokens: tokens.map((token, index) => ({ id: String(index), text: token.text, cls: token.cls })) });
  }
  if (cache.size > 200) cache.delete(cache.keys().next().value!);
  cache.set(markdownText, blocks);
  return blocks;
}
