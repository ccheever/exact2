// A code block's syntax colours, as Discord colours a block whose fence names its language: a
// small tokenizer (comments, strings, numbers, keywords, types, calls) per family of languages, not a
// parser. A language it does not know stays one plain run.
export interface Token { kind: 'plain' | 'keyword' | 'string' | 'comment' | 'number' | 'type' | 'call'; text: string }

// Comments a language has: `//` and `/* */`, or `#`, each to the line's end.
interface Grammar { comment: 'c' | 'hash' | 'none'; keywords: Set<string> }
const words = (s: string) => new Set(s.split(/\s+/));
const C_LIKE = 'c' as const, HASH = 'hash' as const;
const GRAMMARS: Record<string, Grammar> = {
  js: { comment: C_LIKE, keywords: words('async await break case catch class const continue debugger default delete do else export extends false finally for from function if import in instanceof let new null of return static super switch this throw true try typeof undefined var void while with yield interface type enum implements private public protected readonly as satisfies keyof') },
  rust: { comment: C_LIKE, keywords: words('as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while Some None Ok Err') },
  py: { comment: HASH, keywords: words('and as assert async await break class continue def del elif else except False finally for from global if import in is lambda None nonlocal not or pass raise return True try while with yield self') },
  sh: { comment: HASH, keywords: words('if then else elif fi for while do done case esac in function return export local echo exit set unset') },
  c: { comment: C_LIKE, keywords: words('auto break case char const continue default do double else enum extern float for goto if inline int long register return short signed sizeof static struct switch typedef union unsigned void volatile while bool true false nullptr class public private protected virtual template typename namespace using new delete this') },
  go: { comment: C_LIKE, keywords: words('break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var nil true false') },
  swift: { comment: C_LIKE, keywords: words('let var func if else guard return class struct enum protocol extension import self Self init deinit true false nil for in while switch case default break continue throw throws try catch async await some any private public internal fileprivate static override mutating') },
  json: { comment: 'none', keywords: words('true false null') },
};
const ALIASES: Record<string, string> = {
  js: 'js', javascript: 'js', jsx: 'js', ts: 'js', typescript: 'js', tsx: 'js', mjs: 'js',
  rs: 'rust', rust: 'rust', py: 'py', python: 'py', sh: 'sh', bash: 'sh', shell: 'sh', zsh: 'sh', console: 'sh',
  c: 'c', h: 'c', cpp: 'c', 'c++': 'c', cc: 'c', java: 'c', kotlin: 'c', kt: 'c', cs: 'c', csharp: 'c',
  go: 'go', golang: 'go', swift: 'swift', json: 'json',
};

const NUMBER = /\b(?:0x[0-9a-fA-F_]+|\d[\d_]*(?:\.\d+)?(?:e[+-]?\d+)?)\b/y;
const WORD = /[A-Za-z_$][\w$]*/y;
const at = (re: RegExp, text: string, i: number): string | null => { re.lastIndex = i; return re.exec(text)?.[0] ?? null; };
const lineEnd = (text: string, i: number): number => { const n = text.indexOf('\n', i); return n < 0 ? text.length : n; };

/** The comment at `i`, to its close, or to the end of the text (a block) or line when it has none. */
function comment(g: Grammar, code: string, i: number): string | null {
  if (g.comment === 'hash' ? code[i] !== '#' : g.comment !== 'c' || code[i] !== '/') return null;
  if (g.comment === 'hash' || code[i + 1] === '/') return code.slice(i, lineEnd(code, i));
  if (code[i + 1] !== '*') return null;
  const close = code.indexOf('*/', i + 2);
  return code.slice(i, close < 0 ? code.length : close + 2);
}

/** The string at `i`: to its closing quote, past escapes, or to the line's end when it has none (a
 * backtick string may span lines, to the text's end). Each character is looked at once. */
function string(code: string, i: number): string | null {
  const quote = code[i];
  if (quote !== '"' && quote !== "'" && quote !== '`') return null;
  let j = i + 1;
  while (j < code.length && code[j] !== quote && (quote === '`' || code[j] !== '\n')) j += code[j] === '\\' ? 2 : 1;
  return code.slice(i, Math.min(code.length, code[j] === quote ? j + 1 : j));
}

/** The tokens of a block in `lang`, or null when the language is not one this knows. Adjacent tokens of
 * one kind are merged, so a block is a few dozen runs, not one a character. */
export function highlight(code: string, lang: string): Token[] | null {
  const g = GRAMMARS[ALIASES[lang.toLowerCase()] ?? ''];
  if (!g || code.length > 20000) return null;
  const out: Token[] = [];
  const push = (kind: Token['kind'], text: string) => {
    const last = out[out.length - 1];
    if (last?.kind === kind) last.text += text; else out.push({ kind, text });
  };
  let i = 0;
  while (i < code.length) {
    let m: string | null;
    if ((m = comment(g, code, i))) push('comment', m);
    else if ((m = string(code, i))) push('string', m);
    else if ((m = at(NUMBER, code, i)) && !/[\w$]/.test(code[i - 1] ?? '')) push('number', m);
    else if ((m = at(WORD, code, i))) {
      const next = code.slice(i + m.length).match(/^\s*(.)/)?.[1];
      push(g.keywords.has(m) ? 'keyword' : /^[A-Z][a-z]/.test(m) ? 'type' : next === '(' ? 'call' : 'plain', m);
    } else { push('plain', code[i]); i++; continue; }
    i += m.length;
  }
  return out;
}
