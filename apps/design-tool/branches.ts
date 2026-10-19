// A design read's columns and a cell's branch (LLP 1122 D6), and what a
// value names: pure functions of `describe`'s axes and `explain`'s whens.
export type Axis = { name: string; values: string[]; under: string | null };
export type Column = { id: string; label: string; tuple: number; dark: boolean; values: string[] };
type When = { axis: string; value: string };
const whenText = (w: When[]) => w.map(c => `${c.axis}=${c.value}`).sort().join(' ');

/** The grid's columns: every app-mode tuple, light and dark. A light column
 * for an `under scheme=dark` axis's other values repeats the default's (the
 * axis varies the dark side only), so it is left out. */
export function columnsOf(axes: Axis[]): Column[] {
  const tuples: string[][] = [[]];
  for (const a of axes) tuples.splice(0, tuples.length, ...tuples.flatMap(t => a.values.map(v => [...t, v])));
  const out: Column[] = [];
  for (const dark of [false, true]) {
    tuples.forEach((values, tuple) => {
      if (!dark && axes.some((a, i) => a.under === 'dark' && values[i] !== a.values[0])) return;
      // Each mode off its default, by axis, so two axes' like-named values
      // never make one label.
      const modes = values.flatMap((v, i) => (!(axes[i].under === 'dark' && !dark) && v !== axes[i].values[0] ? [`${axes[i].name}=${v}`] : []));
      out.push({ id: `${tuple}-${dark ? 'dark' : 'light'}`, label: [dark ? 'Dark' : 'Light', ...modes].join(' · '), tuple, dark, values });
    });
  }
  return out;
}

/** The branch a cell shows (LLP 1122 D6's target branch), as the checker
 * picks it: of the `when`s whose every condition holds at the column's
 * tuple, the one naming the most axes; on a light column a `when` naming an
 * `under scheme=dark` axis is passed over (it sets the dark side only). Two
 * that tie are ambiguous, which the checker refuses, so a checked design
 * has none; said, if one comes. And "only this mode": a `when` naming every
 * axis at this tuple, which holds here alone; on a light column with an
 * under-dark axis such a `when` would set the dark side only, so there is
 * none to offer ("" ). A token of another kind (LLP 1121 stage 6) has no
 * sides and may not vary on an under-dark axis, so its own `when` names the
 * other axes only (none, with no other axis); a duration or an easing
 * varies on no axis, so it has none. */
export function branchOf(axes: Axis[], col: Column, whens: { when: When[]; value: string }[], kind = 'color'): { when: string; index: number; tie: string; own: string } {
  const at = (axis: string) => col.values[axes.findIndex(a => a.name === axis)];
  const under = new Set(axes.filter(a => a.under === 'dark').map(a => a.name));
  const holding = whens.map((w, index) => ({ ...w, index })).filter(w => (col.dark || !w.when.some(c => under.has(c.axis))) && w.when.every(c => at(c.axis) === c.value));
  const most = Math.max(0, ...holding.map(w => w.when.length));
  const top = holding.filter(w => w.when.length === most && most > 0);
  const others = axes.map((a, i) => ({ axis: a.name, value: col.values[i] })).filter(c => !under.has(c.axis));
  const own = kind === 'duration' || kind === 'easing' ? ''
    : kind !== 'color' ? (others.length ? whenText(others) : '')
    : !col.dark && under.size > 0 ? '' : whenText(axes.map((a, i) => ({ axis: a.name, value: col.values[i] })));
  return { when: top[0] ? whenText(top[0].when) : '', index: top[0]?.index ?? -1, tie: top.length > 1 ? top.map(w => `when ${whenText(w.when)}`).join(' and ') : '', own };
}

// CSS's system colours, which a value may name as a platform colour.
const SYSTEM = new Set(['canvas', 'canvastext', 'linktext', 'visitedtext', 'activetext', 'buttonface', 'buttontext', 'buttonborder', 'field', 'fieldtext', 'highlight', 'highlighttext', 'selecteditem', 'selecteditemtext', 'mark', 'marktext', 'graytext', 'accentcolor', 'accentcolortext']);
/** What a value names, as the checker reads a name: a token by its name, a
 * role override by its row's spelling or its bare name in any case
 * (`label` for `-exact-label`), a dial by its name; and a platform role or
 * system colour no token overrides (`-exact-secondary-label`, `Canvas`),
 * which also makes the value an expression. A word is a whole word of the
 * value: never part of a hex colour or a number. */
export function inputsOf(value: string, names: string[], dials: string[]): { tokens: string[]; dials: string[]; roles: string[] } {
  // (A param's full name, `src/card.contract#Card.tone`, is one word: a division has spaces.)
  const words = value.split(/[\s,()*+]+/).filter(w => /^[A-Za-z_.-][\w./#-]*$/.test(w));
  const set = new Set(words), lower = new Set(words.map(w => w.toLowerCase()));
  const named = (n: string) => set.has(n) || lower.has(n.toLowerCase()) || (n.toLowerCase().startsWith('-exact-') && lower.has(n.toLowerCase().slice(7)));
  const tokens = names.filter(named), found = dials.filter(d => set.has(d));
  const taken = new Set([...tokens, ...found].map(n => n.toLowerCase()));
  const roles = words.filter(w => (w.toLowerCase().startsWith('-exact-') || SYSTEM.has(w.toLowerCase())) && !taken.has(w.toLowerCase()) && !tokens.some(n => n.toLowerCase() === `-exact-${w.toLowerCase()}`));
  return { tokens, dials: found, roles: [...new Set(roles)] };
}

