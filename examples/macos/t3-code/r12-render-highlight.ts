// Lane r12-render: the shared highlighter's Shiki path. For a language the reference's Shiki 4.2
// grammars cover here (r12-render-grammar.ts: typescript, tsx, javascript, jsx, json, jsonc,
// shellscript, python, css, html, markdown, yaml, toml, rust, go, swift) the tokens are what the
// reference paints: r12-render-textmate.ts runs the grammar, and each run's pierre-light /
// pierre-dark colour pair names the synColor class that paints it (markdown.contract). Chat code
// blocks, the diff panel, the Files panel, the attachment code preview and content-search lines
// all reach this through timeline-highlight.ts's highlight().
//
// Cost: Hermes runs the tokenizer at roughly 50k characters a second, so a text over
// SHIKI_MAX_CHARS keeps the heuristic tokenizer; a text that extends a recently tokenized one
// (a streaming code block, the same file again) resumes from the last unchanged line's state.

import { colorOf, grammarId, tokenizeLine, type LineState, type Piece } from './r12-render-textmate';
import type { Cls, Token } from './timeline-highlight';

export const SHIKI_MAX_CHARS = 40_000;

/** Each pierre-light / pierre-dark foreground pair the themes define, as its synColor class. */
const CLASS_OF_PAIR: Record<string, Cls> = {
  '#d32a61/#ff678d': 'kw', '#a631be/#d568ea': 'decl', '#693acf/#9d6afb': 'fn', '#d47628/#ffa359': 'var',
  '#d5901c/#ffab16': 'const', '#636363/#a3a3a3': 'param', '#d5a910/#ffd452': 'flag', '#199f43/#5ecc71': 'str',
  '#16a994/#61d5c0': 'esc', '#1ca1c7/#68cdf2': 'num', '#08c0ef/#08c0ef': 'op', '#636363/#636363': 'punct',
  '#737373/#737373': 'com', '#d5512f/#ff855e': 'tag', '#18a46c/#60d199': 'attr', '#17a5af/#64d1db': 'regex',
  '#1a85d4/#69b1ff': 'deco', '#0a0a0a/#fafafa': '',
};
const classCache = new Map<string, Cls>();
function classOf(scopes: string[]): Cls {
  const key = scopes.join(' ');
  let cls = classCache.get(key);
  if (cls === undefined) {
    cls = CLASS_OF_PAIR[`${colorOf(scopes, 'light')}/${colorOf(scopes, 'dark')}`] ?? '';
    if (classCache.size > 4000) classCache.clear();
    classCache.set(key, cls);
  }
  return cls;
}

// @pierre/diffs getFiletypeFromFileName, for the extensions whose grammar is here.
const EXTENSIONS: Record<string, string> = {
  sh: 'zsh', bash: 'zsh', zsh: 'zsh', css: 'css', go: 'go', html: 'html', htm: 'html', js: 'javascript', mjs: 'javascript',
  cjs: 'javascript', json: 'json', jsonc: 'jsonc', jsx: 'jsx', md: 'markdown', markdown: 'markdown', py: 'python', pyw: 'python',
  pyi: 'python', rs: 'rust', swift: 'swift', toml: 'toml', ts: 'typescript', mts: 'typescript', cts: 'typescript', tsx: 'tsx',
  yaml: 'yaml', yml: 'yml',
};
/** The grammar for a fence language (Shiki's id or alias) or a file path (its extension), or ''. */
export function shikiLanguage(nameOrPath: string): string {
  const raw = nameOrPath.trim();
  if (!raw) return '';
  if (!raw.includes('/') && !raw.includes('.')) return grammarId(raw) || grammarId(raw.toLowerCase());
  const compound = /\.([^/\\]+\.[^/\\]+)$/.exec(raw)?.[1];
  if (compound && EXTENSIONS[compound]) return grammarId(EXTENSIONS[compound]!);
  const simple = /\.([^.]+)$/.exec(raw)?.[1] ?? '';
  return EXTENSIONS[simple] ? grammarId(EXTENSIONS[simple]!) : '';
}

interface Document { lang: string; lines: string[]; states: LineState[]; pieces: Piece[][] }
const documents: Document[] = [];
const results = new Map<string, Token[]>();

function tokenizeDocument(text: string, lang: string): Piece[][] {
  const lines = text.split(/\r\n|\r|\n/);
  // Resume after the longest run of unchanged leading lines among the recent documents.
  let base: Document | null = null, shared = 0;
  for (const doc of documents) {
    if (doc.lang !== lang) continue;
    let same = 0;
    while (same < lines.length && same < doc.lines.length && doc.lines[same] === lines[same]) same++;
    if (same > shared) { shared = same; base = doc; }
  }
  const states: LineState[] = base ? base.states.slice(0, shared) : [];
  const pieces: Piece[][] = base ? base.pieces.slice(0, shared) : [];
  let state: LineState = shared ? states[shared - 1]! : null;
  for (let index = shared; index < lines.length; index++) {
    const result = tokenizeLine(lines[index]!, lang, state);
    state = result.state;
    states.push(state);
    pieces.push(result.pieces);
  }
  documents.unshift({ lang, lines, states, pieces });
  if (documents.length > 6) documents.pop();
  return pieces;
}

/** The reference's tokens for `text`, or null when no grammar here covers the language or the text is too long. */
export function shikiTokens(text: string, nameOrPath: string): Token[] | null {
  if (text.length > SHIKI_MAX_CHARS) return null;
  const lang = shikiLanguage(nameOrPath);
  if (!lang) return null;
  const key = `${lang}\u0000${text}`;
  const known = results.get(key);
  if (known) return known.map(token => ({ ...token }));
  const tokens: Token[] = [];
  const push = (value: string, cls: Cls) => {
    const last = tokens[tokens.length - 1];
    if (last && last.cls === cls) last.text += value; else tokens.push({ text: value, cls });
  };
  tokenizeDocument(text, lang).forEach((line, index) => {
    if (index) push('\n', '');
    for (const piece of line) push(piece.text, classOf(piece.scopes));
  });
  if (results.size > 64) results.delete(results.keys().next().value!);
  results.set(key, tokens);
  return tokens.map(token => ({ ...token }));
}
