// A held import's brand offers (*1.18*, theme2's brand-fill ruling) through a partial accept, an edit or a conflict's
// recompute: each is held again only whole, with the accent it was measured on, so a brand never lands with an ink
// chosen for another accent. Its ink was measured on the accent's value as the import proposed it, which reads the
// operations that write the accent and the tokens its value names; one of those left out, edited, or (the design
// having moved) read from the design rather than the proposal, and the brand goes, its operations with it.

const named = op => (op?.op === 'add-token' || op?.op === 'set-token' ? op.name : null);
const valuesOf = op => [op?.value ?? '', ...(op?.whens ?? []).map(w => w.value ?? '')].join(' ');
// A token's name in a value, whole (`shadcn.brand`, not `shadcn.brand-2`).
const escape = s => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const mentions = (text, name) => new RegExp(`(^|[^\\w.-])${escape(name)}($|[^\\w.-])`).test(text);

/** The operations (indices into `ops`) brand `b`'s ink was measured through: its accent's, and those of the tokens
 * that value names, transitively; and whether the value names a dotted token no operation writes (the design's). */
function measuredOn(b, ops) {
  const at = new Map(ops.map((op, i) => [named(op), i]).filter(([n]) => n));
  const need = new Set();
  let outside = false;
  const walk = name => {
    const i = at.get(name);
    if (i === undefined) { outside = true; return; }
    if (need.has(i)) return;
    need.add(i);
    const text = valuesOf(ops[i]);
    for (const n of at.keys()) if (mentions(text, n)) walk(n);
    // (a dotted name no operation writes: a token of the design's)
    if ((text.match(/[A-Za-z_][\w-]*(?:\.[\w-]+)+/g) ?? []).some(n => !at.has(n))) outside = true;
  };
  walk(b.token);
  return { need, outside };
}

/** The brand offers of `review` left out when the operations `kept` (indices into `ops`) are held again: one kept in
 * part, or without an operation its ink was measured through, or whose accent (or a token it names) the edit of
 * `edited` changes, or, with the design moved since (`moved`), measured on a value the design holds. `{drop, gone}`:
 * the items to leave out, and the brand fills (`sets`) that go. */
export function brandsLeftOut(review, ops, kept, { edited = null, moved = false } = {}) {
  const keep = new Set(kept), drop = new Set(), gone = [];
  for (const b of review?.brands ?? []) {
    const items = b.items ?? [];
    if (!items.some(i => keep.has(i))) continue;
    const { need, outside } = measuredOn(b, ops);
    const out = !items.every(i => keep.has(i))
      || [...need].some(i => !keep.has(i))
      || (edited !== null && [...need].some(i => named(ops[i]) === edited))
      || (moved && outside);
    if (out) { for (const i of items) drop.add(i); gone.push(b.sets); }
  }
  return { drop, gone };
}

/** The review's note for the brand fills `gone`, as a held proposal's `notes` line. */
export const goneNote = gone => `${gone.join(' and ')} ${gone.length > 1 ? 'aren\'t' : 'isn\'t'} offered here: its ink was measured on the accent as the import proposed it, which this one doesn't hold whole; import again to have ${gone.length > 1 ? 'them' : 'it'}`;
