// A code block's syntax colours, as Discord colours a block whose fence names its language: a
// small tokenizer (comments, strings, numbers, keywords, types, calls) per family of languages, not a
// parser. A language it does not know stays one plain run.
export interface Token { kind: 'plain' | 'keyword' | 'string' | 'comment' | 'number' | 'type' | 'call'; text: string }

interface Grammar { comment: RegExp; keywords: Set<string> }
const words = (s: string) => new Set(s.split(/\s+/));
const C_LIKE = /\/\/[^\n]*|\/\*[\s\S]*?\*\//y;
const HASH = /#[^\n]*/y;
const GRAMMARS: Record<string, Grammar> = {
  js: { comment: C_LIKE, keywords: words('async await break case catch class const continue debugger default delete do else export extends false finally for from function if import in instanceof let new null of return static super switch this throw true try typeof undefined var void while with yield interface type enum implements private public protected readonly as satisfies keyof') },
  rust: { comment: C_LIKE, keywords: words('as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while Some None Ok Err') },
  py: { comment: HASH, keywords: words('and as assert async await break class continue def del elif else except False finally for from global if import in is lambda None nonlocal not or pass raise return True try while with yield self') },
  sh: { comment: HASH, keywords: words('if then else elif fi for while do done case esac in function return export local echo exit set unset') },
  c: { comment: C_LIKE, keywords: words('auto break case char const continue default do double else enum extern float for goto if inline int long register return short signed sizeof static struct switch typedef union unsigned void volatile while bool true false nullptr class public private protected virtual template typename namespace using new delete this') },
  go: { comment: C_LIKE, keywords: words('break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var nil true false') },
  swift: { comment: C_LIKE, keywords: words('let var func if else guard return class struct enum protocol extension import self Self init deinit true false nil for in while switch case default break continue throw throws try catch async await some any private public internal fileprivate static override mutating') },
  json: { comment: /(?!)/y, keywords: words('true false null') },
};
const ALIASES: Record<string, string> = {
  js: 'js', javascript: 'js', jsx: 'js', ts: 'js', typescript: 'js', tsx: 'js', mjs: 'js',
  rs: 'rust', rust: 'rust', py: 'py', python: 'py', sh: 'sh', bash: 'sh', shell: 'sh', zsh: 'sh', console: 'sh',
  c: 'c', h: 'c', cpp: 'c', 'c++': 'c', cc: 'c', java: 'c', kotlin: 'c', kt: 'c', cs: 'c', csharp: 'c',
  go: 'go', golang: 'go', swift: 'swift', json: 'json',
};

const STRING = /"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`/y;
const NUMBER = /\b(?:0x[0-9a-fA-F_]+|\d[\d_]*(?:\.\d+)?(?:e[+-]?\d+)?)\b/y;
const WORD = /[A-Za-z_$][\w$]*/y;
const at = (re: RegExp, text: string, i: number): string | null => { re.lastIndex = i; return re.exec(text)?.[0] ?? null; };

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
    if ((m = at(g.comment, code, i))) push('comment', m);
    else if ((m = at(STRING, code, i))) push('string', m);
    else if ((m = at(NUMBER, code, i)) && !/[\w$]/.test(code[i - 1] ?? '')) push('number', m);
    else if ((m = at(WORD, code, i))) {
      const next = code.slice(i + m.length).match(/^\s*(.)/)?.[1];
      push(g.keywords.has(m) ? 'keyword' : /^[A-Z][a-z]/.test(m) ? 'type' : next === '(' ? 'call' : 'plain', m);
    } else { push('plain', code[i]); i++; continue; }
    i += m.length;
  }
  return out;
}
