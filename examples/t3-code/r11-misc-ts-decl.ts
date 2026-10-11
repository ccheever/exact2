// Lane r11-misc: statement state for the shared JavaScript / TypeScript tokenizer (tsLike in
// timeline-highlight.ts), as Shiki 4.2's typescript and javascript grammars scope it (MIT
// reference, see LICENSE-T3: the reference colours code with Shiki and the pierre themes).
//   - var-expr: every declarator of a `const` / `let` / `var` list is a definition: after the
//     keyword, after a top-level `,` (`const a = 1, m = 2`), and the binding names of a
//     destructuring pattern (`const {r, s: t} = o`, `const [p, q]`). A `const` name is
//     variable.other.constant whatever follows it (`const d: number`, `for (const x of xs)`).
//     The list ends at `;`, at a bracket closing below it, or at a line's end not after a `,`.
//   - object literals: a key's `:` is punctuation.separator.key-value, so the value after it is
//     an expression, not a type annotation; so is a `case x:` label's colon. A `{` in a type
//     (an annotation, a `type` alias, generic arguments) is a type literal, whose keys type.
//   - import-declaration: to the line's end (or `;`) every name is an alias (variable), `type`
//     a keyword, `*` constant.language.import-export-all, and other operator characters plain.

export type Bracket = '(' | '[' | 'obj' | 'block';
const OBJECT_BEFORE = /(?:[=(,:[?!&|+\-*%<>~^]|\breturn|\byield|\bawait|\bcase|\bdefault|\bin|\bof|\bnew|\btypeof|\bvoid)$/;

export class TsStatements {
  readonly brackets: Bracket[] = [];
  varKind = ''; varNest = -1; destructure = -1; importing = false; caseOpen = false;
  typeAlias = false; angle = 0; // a `type X = …` statement; open generic `<` brackets

  /** A `{` / `[` / `(` opens: `before` is the text of the last significant token (trimmed). */
  open(character: string, before: string, typeContext: boolean, declaring: string): void {
    const binding = (declaring === 'const' || declaring === 'let' || declaring === 'var') && this.varKind !== '';
    if (binding && (character === '{' || character === '[')) this.destructure = this.brackets.length;
    const kind: Bracket = character === '(' ? '(' : character === '[' ? '['
      : binding || this.inPattern() || !typeContext && !this.typeAlias && this.angle === 0 && !before.endsWith('=>') && OBJECT_BEFORE.test(before) ? 'obj' : 'block';
    this.brackets.push(kind);
  }
  close(): void {
    this.brackets.pop();
    if (this.destructure >= 0 && this.brackets.length <= this.destructure) this.destructure = -1;
    if (this.varNest >= 0 && this.brackets.length < this.varNest) this.endVar();
  }
  /** Inside a destructuring pattern of the current declaration (any depth). */
  inPattern(): boolean { return this.destructure >= 0 && this.brackets.length > this.destructure; }
  /** The innermost open bracket is an object literal or a pattern: its `:` separates a key. */
  keyColon(): boolean { return this.brackets[this.brackets.length - 1] === 'obj' || this.caseOpen; }

  beginVar(kind: string): void { this.varKind = kind; this.varNest = this.brackets.length; this.destructure = -1; }
  endVar(): void { this.varKind = ''; this.varNest = -1; this.destructure = -1; }
  /** A `,` at the declaration's own level starts the next declarator: the kind to declare, or ''. */
  comma(): string { return this.varKind && this.brackets.length === this.varNest && this.angle === 0 ? this.varKind : ''; }
  semicolon(): void {
    if (this.varKind && this.brackets.length <= this.varNest) this.endVar();
    this.importing = false; this.caseOpen = false;
    if (this.brackets.length === 0) { this.typeAlias = false; this.angle = 0; }
  }
  /** A line ends: a declaration list continues only after a trailing `,`; an import ends. */
  newline(before: string): void {
    if (this.varKind && this.brackets.length === this.varNest && !before.endsWith(',')) this.endVar();
    if (!/(?:^|[^.$\w])import$/.test(before)) this.importing = false;
    if (this.typeAlias && this.brackets.length === 0 && this.angle === 0 && !/[=|&,<(]$/.test(before)) this.typeAlias = false;
  }
  /** A destructured binding name (`before` the last significant text, `next` the character after). */
  bindingName(before: string, next: string): string {
    if (!this.inPattern() || next === ':' || next === '(') return '';
    return /(?:[{[,:]|\.\.\.)$/.test(before) ? (this.varKind === 'const' ? 'const' : 'var') : '';
  }
}

/** The class an import declaration gives a word, or '' when the declaration does not apply. */
export function importWord(word: string, keywords: Set<string>): 'kw' | 'var' {
  return keywords.has(word) || word === 'type' || word === 'typeof' ? 'kw' : 'var';
}

// ── Where an HTML page's script runs on (r9-device-html-end.ts) ─────────────────────────────
/** The javascript grammar's if-statement rule (`if (…)` not directly followed by `{`: `\s*(?!\{)`
 *  backtracks, so `if (a) {` with a space qualifies) begins at `index`, matched on its own line. */
const IF_STATEMENT = /if\s*(\(([^()]|(\(([^()]|\([^()]*\))*\)))*\))\s*(?!\{)/y;
export function ifStatementBegins(text: string, index: number): boolean {
  if (text[index - 1] === '.' || /[$\w]/.test(text[index - 1] ?? '')) return false;
  const end = text.indexOf('\n', index), line = (end < 0 ? text.slice(index) : text.slice(index, end)) + '\n';
  IF_STATEMENT.lastIndex = 0;
  return IF_STATEMENT.test(line);
}
/** Whether `index` sits on an import declaration's line (its rule runs to the line's end or a `;`),
 *  where a `<` is plain text, never a JSX element. */
export function inImportLine(text: string, index: number): boolean {
  const start = text.lastIndexOf('\n', index - 1) + 1, line = text.slice(start, index);
  const found = /(?:^|[^.$\w])import\b(?!\s*[(.])/.exec(line);
  return !!found && !line.slice(found.index).includes(';');
}
