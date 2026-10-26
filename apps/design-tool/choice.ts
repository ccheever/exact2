// A held import's card's choice (LLP 1122 D11): which of its tokens and role overrides are in, as the card begins and
// as a value edit carries it to the import held again (third pass #6).

/** Whether a held import's card begins with everything it holds chosen (a recomputed import holds only what was
 * chosen) or as a fresh import's does, its tokens in and its role overrides off until ticked. A value edit holds the
 * import whole again, so it begins fresh, the card's choice carried to it: an edit-held one replaces the proposal its
 * review came `from`, and names who edited it; one held by the older propose path replaces none and is not marked
 * kept. */
export const startsChosen = (h: any, r: any): boolean => !!r && (!!h?.replaces || !!r.kept) && !(h?.editedBy && h.replaces && r.from === h.replaces);

/** The card's choice (`was`, the items it had chosen) as the ids to flip on `fresh`, the import held again whole and
 * beginning fresh: each of its first `tokens` items the card had left out, and each role override it had ticked. */
export function carriedChoice(fresh: string, tokens: number, offers: { items?: number[] }[], was: Set<number>): string[] {
  return [
    ...Array.from({ length: tokens }, (_, i) => i).filter(i => !was.has(i)).map(i => `${fresh}:${i}`),
    ...offers.filter(o => (o.items ?? []).length > 0 && o.items!.every(i => was.has(i))).map(o => `${fresh}:${o.items!.join(',')}`),
  ];
}
