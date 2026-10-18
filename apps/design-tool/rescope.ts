// Act first, re-scope after (LLP 1122 D6, T3): an applied edit of a param
// moved to the token it aliases directly on that branch, or an edit of a
// token's branch that exactly one param aliases there moved to that param,
// as one transaction: the edit's inverse, then the same value at the other
// rung, on the same branch (a `when` is never moved to an unconditional
// value). Undone as one write. A param whose value there is an expression
// aliases no token, so it has no re-scope.
type When = { axis: string; value: string };
type Op = Record<string, unknown>;
type Explained = { value: string; whens: { when: When[]; value: string }[]; files?: Record<string, string> };

/** A param as the panel's design read has it: its line's value (null with
 * none) and how many `when` lines it has, its default as written, and its
 * short name (the full name where two components share the short one). */
export type ParamInfo = { set: string | null; whens: number; default: string; short: string };

/** Each param's info, by its full name, from `describe`'s IR. */
export function paramInfoOf(ir: { components?: { params: { name: string; short: string | null; default: string; set: { value: string; whens?: number } | null }[] }[] }): Map<string, ParamInfo> {
  const out = new Map<string, ParamInfo>();
  for (const c of ir.components ?? []) for (const p of c.params) {
    out.set(p.name, { set: p.set?.value ?? null, whens: p.set?.whens ?? 0, default: p.default ?? '', short: p.short ?? p.name });
  }
  return out;
}

/** The declared token a value names alone, or null: a token by its name, a
 * role override by any spelling of its role (`label`, `-Exact-Label`), as
 * the checker reads them. */
export function tokenOfIn(ir: { tokens: { name: string; role?: string | null }[] }): (value: string) => string | null {
  const names = new Set(ir.tokens.map(t => t.name));
  const key = (n: string) => n.toLowerCase().replace(/^-exact-/, '');
  const roles = new Map(ir.tokens.filter(t => t.role).map(t => [key(t.role!), t.name]));
  return value => { const v = String(value ?? '').trim(); return names.has(v) ? v : roles.get(key(v)) ?? null; };
}

/** A re-scope of the last edit: what it says, the inverse of that edit and
 * the write at the other rung, and (once written) the files' digests the
 * edit left, which the move is checked against. */
export type Rescope = { text: string; ops: Op[]; files?: Record<string, string> };

const sameWhen = (a: When[], b: When[]) => JSON.stringify(a.map(c => `${c.axis}=${c.value}`).sort()) === JSON.stringify(b.map(c => `${c.axis}=${c.value}`).sort());

/** The branch `when` of an explained name as it was: there or not, and the
 * value it has there (the unconditional one where it has no such `when`). */
const branchOf = (x: Explained, when: When[], base: boolean) => {
  if (!when.length) return { there: base, value: x.value };
  const w = x.whens.find(b => sameWhen(b.when, when));
  return { there: !!w, value: w?.value ?? x.value };
};

/** The re-scope of `set-param`/`set-token` `name` on branch `when` to
 * `value`, from the design as it was before the edit, all of one revision
 * (`files`, the reviewed proposal's digests as one key: `explain`'s answers
 * must be of it), or null where there is none. */
export async function rescopeOf(op: { op: string; name: string; when: When[]; value: string }, params: Map<string, ParamInfo>, tokenOf: (v: string) => string | null, explain: (name: string) => Promise<Explained | null>, files: string): Promise<Rescope | null> {
  const ofIt = (x: Explained | null) => (x && JSON.stringify(Object.keys(x.files ?? {}).sort().map(f => [f, x.files![f]])) === files ? x : null);
  if (op.op === 'set-param') {
    // (By the name its line is written with, short or full.)
    const entry = [...params].find(([name, p]) => name === op.name || p.short === op.name);
    if (!entry) return null;
    const [name, p] = entry;
    const x = ofIt(await explain(name));
    if (!x) return null;
    const was = branchOf(x, op.when, p.set !== null);
    // The token it aliased on that branch, which the new value does not name.
    const alias = tokenOf(was.value);
    if (!alias || tokenOf(op.value) === alias) return null;
    const undo = was.there ? { op: 'set-param', name: p.short, when: op.when, value: was.value } : { op: 'unset-param', name: p.short, when: op.when };
    return { text: `Apply to the token ${alias} instead`, ops: [undo, { op: 'set-token', name: alias, when: op.when, value: op.value }] };
  }
  if (op.op === 'set-token') {
    const t = ofIt(await explain(op.name));
    if (!t) return null;
    // Every param whose value on that branch names this token alone.
    const aliasing: ParamInfo[] = [];
    for (const [name, p] of params) {
      if (!p.whens) { if (tokenOf(p.set ?? p.default) === op.name) aliasing.push(p); continue; }
      const x = ofIt(await explain(name));
      if (!x) return null;
      if (tokenOf(branchOf(x, op.when, true).value) === op.name) aliasing.push(p);
    }
    if (aliasing.length !== 1 || tokenOf(op.value) === op.name) return null;
    const p = aliasing[0];
    const was = branchOf(t, op.when, true);
    const undo = was.there ? { op: 'set-token', name: op.name, when: op.when, value: was.value } : { op: 'unset-token-when', name: op.name, when: op.when };
    // A param's `when` line needs its own line first: the value it had.
    const own = op.when.length && p.set === null ? [{ op: 'set-param', name: p.short, when: [], value: p.default }] : [];
    return { text: `Apply to ${p.short} instead`, ops: [undo, ...own, { op: 'set-param', name: p.short, when: op.when, value: op.value }] };
  }
  return null;
}
