// A held import's brand offers (*1.18*, theme2's brand-fill ruling) held again: by a partial accept, an edit or a
// recompute after the files moved. Each is held again only whole (its fill and its ink), and measured again on the
// design as it is with the operations it is held with (`contract tamber import-plan`'s `brands`, the importer's own
// measure): one whose fill or ink was edited, whose accent changed under it, or that lost an operation its ink was
// measured through goes, its operations with it, and the review's notes say so. One kept has its contrast as it is now.

/** The brand offers of `review` kept only in part when the operations `kept` (indices into the proposal's) are held
 * again: `{drop, gone}`, their items to leave out and the brand fills (`sets`) that go. */
export function brandsLeftOut(review, kept) {
  const keep = new Set(kept), drop = new Set(), gone = [];
  for (const b of review?.brands ?? []) {
    const items = b.items ?? [];
    if (items.some(i => keep.has(i)) && !items.every(i => keep.has(i))) { for (const i of items) drop.add(i); gone.push(b.sets); }
  }
  return { drop, gone };
}

/** The review's note for the brand fills `gone`, as a held proposal's `notes` line. */
export const goneNote = gone => `${gone.join(' and ')} ${gone.length > 1 ? 'aren\'t' : 'isn\'t'} offered here: its ink no longer fits what this one holds (the fill or the ink edited, the accent changed, or not kept whole); import again to have ${gone.length > 1 ? 'them' : 'it'}`;

/** `ops` without the operations `drop` (indices), the review's offers and brands and a recorded `choice`
 * renumbered to what is left. (A brand's operations come after the tokens', so `tokens` is unchanged.) */
export function withoutItems(ops, review, choice, drop) {
  if (!drop.size) return { ops, review, choice };
  const at = i => i - [...drop].filter(d => d < i).length;
  const renumber = list => list.map(o => ({ ...o, items: (o.items ?? []).filter(i => !drop.has(i)).map(at) }));
  return {
    ops: ops.filter((_, i) => !drop.has(i)),
    review: review && { ...review, offers: renumber(review.offers ?? []), ...(review.brands ? { brands: renumber(review.brands).filter(b => b.items.length) } : {}) },
    choice: choice && { ...choice, choice: (choice.choice ?? []).filter(i => !drop.has(i)).map(at) },
  };
}

/** The brand offers of `review` measured again with `ask` (the design server's), held with `ops`: those that no
 * longer fit dropped with their operations (and from `choice`), the rest with their contrast and note as they are
 * now. A measure that can't be had drops them all, since none is known to fit. `{ops, review, choice}`. */
export async function remeasured(ask, ops, review, choice) {
  const brands = review?.brands ?? [];
  if (!brands.length) return { ops, review, choice };
  const m = await ask({ method: 'import-plan', brands: brands.map(({ sets, token, items }) => ({ sets, token, items })), ops });
  const results = m?.ok?.brands ?? [];
  const drop = new Set(), gone = [];
  const kept = brands.flatMap((b, k) => {
    const r = results[k];
    if (r?.keep && r.sets === b.sets) { const { note: _was, ...rest } = b; return [{ ...rest, contrast: r.contrast, ...(r.note ? { note: r.note } : {}) }]; }
    for (const i of b.items ?? []) drop.add(i);
    gone.push(b.sets);
    return [];
  });
  return withoutItems(ops, { ...review, brands: kept, ...(gone.length ? { notes: [...(review.notes ?? []), goneNote(gone)] } : {}) }, choice, drop);
}
