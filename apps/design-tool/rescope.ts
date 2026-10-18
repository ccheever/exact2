// Act first, re-scope after (LLP 1122 D6, T3): an applied edit of a param
// moved to the token it aliases directly, or an edit of a token one param
// aliases moved to that param, as one transaction: the edit's inverse, then
// the same value at the other rung, on the same branch (a `when` is never
// moved to an unconditional value). Undone as one write. A param whose value
// is an expression aliases no token, so it has no re-scope.
type When = { axis: string; value: string };
type Op = Record<string, unknown>;

/** A param as the panel's design read has it: its line's value (null with
 * none), its default as written, its short name, and the token it aliases
 * directly (its value, the line's or the default, when that is one token). */
export type ParamInfo = { set: string | null; default: string; short: string; alias: string | null };
// (`short` is the full name where two components share the short one.)

/** Each param's info, by its full name, from `describe`'s IR. */
export function paramInfoOf(ir: { tokens: { name: string }[]; components?: { params: { name: string; short: string | null; default: string; set: { value: string } | null }[] }[] }): Map<string, ParamInfo> {
  const tokens = new Set(ir.tokens.map(t => t.name));
  const out = new Map<string, ParamInfo>();
  for (const c of ir.components ?? []) for (const p of c.params) {
    const written = String(p.set?.value ?? p.default ?? '').trim();
    out.set(p.name, { set: p.set?.value ?? null, default: p.default ?? '', short: p.short ?? p.name, alias: tokens.has(written) ? written : null });
  }
  return out;
}

/** A re-scope of the last edit: what it says, the inverse of that edit and
 * the write at the other rung, and (once written) the files' digests the
 * edit left, which the move is checked against. */
export type Rescope = { text: string; ops: Op[]; files?: Record<string, string> };

const sameWhen = (a: When[], b: When[]) => JSON.stringify(a.map(c => `${c.axis}=${c.value}`).sort()) === JSON.stringify(b.map(c => `${c.axis}=${c.value}`).sort());

/** The re-scope of `set-param`/`set-token` `name` on branch `when` to
 * `value`, from the design as it was before the edit (`params`, and
 * `explain`'s branches of the name), or null where there is none. */
export async function rescopeOf(op: { op: string; name: string; when: When[]; value: string }, params: Map<string, ParamInfo>, explain: (name: string) => Promise<{ value: string; whens: { when: When[]; value: string }[] } | null>): Promise<Rescope | null> {
  // The edited branch as it was: whether it was there, and its value.
  const before = async (name: string, base: boolean) => {
    const x = await explain(name);
    if (!x) return null;
    if (!op.when.length) return { there: base, value: x.value };
    const w = x.whens.find(b => sameWhen(b.when, op.when));
    return { there: !!w, value: w?.value ?? '' };
  };
  if (op.op === 'set-param') {
    // (By the name its line is written with, short or full.)
    const p = params.get(op.name) ?? [...params.values()].find(q => q.short === op.name);
    if (!p?.alias) return null;
    const was = await before(op.name, p.set !== null);
    if (!was) return null;
    const undo = was.there ? { op: 'set-param', name: p.short, when: op.when, value: was.value } : { op: 'unset-param', name: p.short, when: op.when };
    return { text: `Apply to the token ${p.alias} instead`, ops: [undo, { op: 'set-token', name: p.alias, when: op.when, value: op.value }] };
  }
  if (op.op === 'set-token') {
    const aliasing = [...params].filter(([, p]) => p.alias === op.name);
    if (aliasing.length !== 1) return null;
    const [, p] = aliasing[0];
    const was = await before(op.name, true);
    if (!was) return null;
    const undo = was.there ? { op: 'set-token', name: op.name, when: op.when, value: was.value } : { op: 'unset-token-when', name: op.name, when: op.when };
    // A param's `when` line needs its own line first: the value it had.
    const own = op.when.length && p.set === null ? [{ op: 'set-param', name: p.short, when: [], value: p.default }] : [];
    return { text: `Apply to ${p.short} instead`, ops: [undo, ...own, { op: 'set-param', name: p.short, when: op.when, value: op.value }] };
  }
  return null;
}
