// Shared TextMate tokenizer, with mobile's GitHub themes rather than desktop Pierre colors.
// Theme-trie helper bodies below retain shared/r12-render-textmate.ts at887b249, presentation only.
// @ref llp/1107.002-design-system-parity.spec.md#semantic-colors
import { tokenizeCode } from './shared/r12-render-textmate';
import { shikiLanguage } from './shared/r12-render-highlight';
import { MOBILE_CODE_THEMES } from './thread-highlight-themes';
export interface ThreadCodeToken { id: string; text: string; color: string; weight: number; slant: string }
interface TrieRule { depth: number; parents: string[] | null; fg: string; fs: number }
class TrieNode {
  children = new Map<string, TrieNode>();
  constructor(public main: TrieRule, public withParents: TrieRule[]) {}
  insert(depth: number, scope: string, parents: string[] | null, fg: string, fs: number) {
    if (scope === '') {
      if (parents === null) { this.main.depth = Math.max(this.main.depth, depth); if (fg) this.main.fg = fg; if (fs !== -1) this.main.fs = fs; return; }
      const same = this.withParents.find(rule => strArrCmp(rule.parents, parents) === 0);
      if (same) { same.depth = Math.max(same.depth, depth); if (fg) same.fg = fg; if (fs !== -1) same.fs = fs; return; }
      this.withParents.push({ depth, parents, fg: fg || this.main.fg, fs: fs !== -1 ? fs : this.main.fs });
      return;
    }
    const dot = scope.indexOf('.'), head = dot < 0 ? scope : scope.slice(0, dot), tail = dot < 0 ? '' : scope.slice(dot + 1);
    let child = this.children.get(head);
    if (!child) { child = new TrieNode({ ...this.main }, this.withParents.map(rule => ({ ...rule }))); this.children.set(head, child); }
    child.insert(depth + 1, tail, parents, fg, fs);
  }
  match(scope: string): TrieRule[] {
    if (scope !== '') {
      const dot = scope.indexOf('.'), head = dot < 0 ? scope : scope.slice(0, dot), tail = dot < 0 ? '' : scope.slice(dot + 1);
      const child = this.children.get(head);
      if (child) return child.match(tail);
    }
    return [...this.withParents, this.main].sort(bySpecificity);
  }
}
function strcmp(a: string, b: string) { return a < b ? -1 : a > b ? 1 : 0; }
function strArrCmp(a: string[] | null, b: string[] | null): number {
  if (a === null && b === null) return 0;
  if (!a) return -1;
  if (!b) return 1;
  if (a.length !== b.length) return a.length - b.length;
  for (let index = 0; index < a.length; index++) { const result = strcmp(a[index]!, b[index]!); if (result) return result; }
  return 0;
}
function bySpecificity(a: TrieRule, b: TrieRule): number {
  if (a.depth !== b.depth) return b.depth - a.depth;
  const pa = a.parents ?? [], pb = b.parents ?? [];
  let ia = 0, ib = 0;
  for (;;) {
    if (pa[ia] === '>') ia++;
    if (pb[ib] === '>') ib++;
    if (ia >= pa.length || ib >= pb.length) break;
    const diff = pb[ib]!.length - pa[ia]!.length;
    if (diff) return diff;
    ia++; ib++;
  }
  return pb.length - pa.length;
}
const matchesScope = (scope: string, pattern: string) => scope === pattern || scope.startsWith(pattern) && scope[pattern.length] === '.';
function parentsMatch(path: string[], parents: string[] | null): boolean {
  if (!parents || !parents.length) return true;
  let at = path.length - 2;
  for (let index = 0; index < parents.length; index++) {
    let pattern = parents[index]!, must = false;
    if (pattern === '>') { if (index === parents.length - 1) return false; pattern = parents[++index]!; must = true; }
    while (at >= 0) { if (matchesScope(path[at]!, pattern)) break; if (must) return false; at--; }
    if (at < 0) return false;
    at--;
  }
  return true;
}
/** vscode-textmate's parseTheme: a fontStyle string as bits ('' and 'normal' are 0). */
function fontStyleBits(style: string | undefined): number {
  if (style === undefined) return -1;
  let bits = 0;
  for (const word of style.split(' ')) bits |= word === 'italic' ? 1 : word === 'bold' ? 2 : word === 'underline' ? 4 : word === 'strikethrough' ? 8 : 0;
  return bits;
}

const themeCache = new Map<string, { root: TrieNode; fg: string }>();
function mobileTheme(scheme: 'light' | 'dark') {
  let theme = themeCache.get(scheme);
  if (theme) return theme;
  const source = MOBILE_CODE_THEMES[scheme];
  const parsed = source.rules.map(([selector, fg, style], index) => {
    const segments = selector!.split(' ').filter(Boolean);
    return { scope: segments.at(-1) ?? '', parents: segments.length > 1 ? segments.slice(0, -1).reverse() : null,
      fg: fg ?? '', fs: fontStyleBits(style ?? undefined), index };
  }).sort((a, b) => strcmp(a.scope, b.scope) || strArrCmp(a.parents, b.parents) || a.index - b.index);
  const root = new TrieNode({ depth: 0, parents: null, fg: '', fs: -1 }, []);
  for (const rule of parsed) root.insert(0, rule.scope, rule.parents, rule.fg, rule.fs);
  theme = { root, fg: source.fg }; themeCache.set(scheme, theme); return theme;
}
const cache = new Map<string, ThreadCodeToken[]>();
/** Actual Shiki grammar scopes, resolved against pinned mobile GitHub theme selectors. */
export function mobileCodeTokens(code: string, language: string, dark: boolean): ThreadCodeToken[] {
  const scheme = dark ? 'dark' : 'light', key = JSON.stringify([code, language, scheme]), cached = cache.get(key);
  if (cached) return cached;
  const theme = mobileTheme(scheme), lang = shikiLanguage(language);
  // Match upstream's plain-code fallback while a grammar is absent or a long line exceeds its limit.
  const pieces = lang && code.split('\n').every(line => line.length <= 1000) ? tokenizeCode(code, lang) : [{ text: code, scopes: [] }];
  const result = pieces.map((piece, index) => {
    let color = theme.fg, style = 0;
    for (let depth = 0; depth < piece.scopes.length; depth++) {
      const path = piece.scopes.slice(0, depth + 1);
      const rule = theme.root.match(piece.scopes[depth]!).find(candidate => parentsMatch(path, candidate.parents));
      if (rule?.fg) color = rule.fg;
      if (rule && rule.fs !== -1) style = rule.fs;
    }
    return { id: String(index), text: piece.text, color, weight: (style & 2) === 2 ? 700 : 400, slant: (style & 1) === 1 ? 'italic' : 'normal' };
  });
  if (cache.size >= 64) cache.delete(cache.keys().next().value!);
  cache.set(key, result); return result;
}
