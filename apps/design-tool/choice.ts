// A held import's card's choice (LLP 1122 D11): which of its tokens and role overrides are in, as the card begins and
// as a value edit carries it to the import held again (third pass #6).

/** Whether a held import's card begins with everything it holds chosen (a recomputed import holds only what was
 * chosen) or as a fresh import's does, its tokens in and its role overrides off until ticked. A value edit holds the
 * import whole again, so it begins fresh, the card's choice carried to it: an edit-held one replaces the proposal its
 * review came `from`, and names who edited it; one held by the older propose path replaces none and is not marked
 * kept. */
export const startsChosen = (h: any, r: any): boolean => !!r && (!!h?.replaces || !!r.kept) && !(h?.editedBy && h.replaces && r.from === h.replaces);

/** An import's items its card has chosen: the server's choice (`heldChoice.base`) when there is one, else the mapped
 * tokens unless unticked and the role overrides only if ticked (LLP 1121 D1), or, on a recomputed import
 * (`startsChosen`), all unless unticked; `picked`'s flips (`<proposal>:<i[,j]>`, joined by `;`) over it. */
export function chosenOf(proposal: string, picked: string, r: any, ops: unknown[], h: any): number[] {
  const flipped = new Set(picked.split(';').filter(x => x.startsWith(`${proposal}:`)).flatMap(x => x.slice(proposal.length + 1).split(',').map(Number)));
  const tokens = Number(r?.tokens ?? 0), recomputed = startsChosen(h, r), chose = heldChoice.base(proposal), base = chose && new Set(chose);
  return ops.map((_, i) => i).filter(i => (base ? base.has(i) !== flipped.has(i) : i < tokens || recomputed ? !flipped.has(i) : flipped.has(i)));
}

/** The card's choice (`was`, the items it had chosen) as the ids to flip on `fresh`, the import held again whole and
 * beginning fresh: each of its first `tokens` items the card had left out, and each role override it had ticked. */
export function carriedChoice(fresh: string, tokens: number, offers: { items?: number[] }[], was: Set<number>): string[] {
  return [
    ...Array.from({ length: tokens }, (_, i) => i).filter(i => !was.has(i)).map(i => `${fresh}:${i}`),
    ...offers.filter(o => (o.items ?? []).length > 0 && o.items!.every(i => was.has(i))).map(o => `${fresh}:${o.items!.join(',')}`),
  ];
}

// The choice on the server (tamber/1.17 `choose`, #6's follow-up): what the card has chosen is sent as it is ticked,
// so a reload or another client starts from it. The card shows a base, the server's choice as this page first saw it
// (or the card's default when there was none), with the page's flips over it. The page's own `choose` answers leave
// the base be (its flips already say so); another client's choice since replaces the base, and the page's flips on
// that proposal are dropped (the last to choose wins, as on the server).
type Base = { at: number; choice: number[] | null };
const bases = new Map<string, Base>(), mine = new Map<string, number>(), sent = new Map<string, string>(), pending = new Map<string, Promise<void>>();
let rebased = new Set<string>(), answers = 0;

export const heldChoice = {
  /** The held list as read: each proposal's base taken at first sight, or again when another client chose since. */
  saw(records: { proposal: unknown; choice?: number[]; choiceAt?: number }[]) {
    const now = new Set<string>();
    for (const p of records) {
      const id = String(p.proposal), at = p.choiceAt ?? 0, b = bases.get(id);
      now.add(id);
      if (!b) bases.set(id, { at, choice: p.choice ?? null });
      else if (at > b.at && at > (mine.get(id) ?? 0) && !pending.has(id)) { bases.set(id, { at, choice: p.choice ?? null }); sent.delete(id); rebased.add(id); }
    }
    for (const id of [...bases.keys()]) if (!now.has(id)) { bases.delete(id); mine.delete(id); sent.delete(id); rebased.delete(id); }
  },
  /** The card's base for `id`: the server's choice, or null for the card's default. */
  base: (id: string): number[] | null => bases.get(id)?.choice ?? null,
  /** The `choiceAt` an accept of `id` names: the newest this page knows, its own `choose`s' included. */
  at: (id: string): number => Math.max(mine.get(id) ?? 0, bases.get(id)?.at ?? 0),
  /** The page's flips (`picked`) on proposals another client chose on since: to drop. Each is said once. */
  drop(picked: string[]): string[] {
    const out = picked.filter(p => rebased.has(p.slice(0, p.indexOf(':'))));
    rebased = new Set();
    return out;
  },
  /** `items` sent as `id`'s choice (`ask`, the request), after any sent before it and only when it differs from the
   * last sent; latest wins, one at a time a proposal. */
  send(id: string, items: number[], ask: (items: number[]) => Promise<{ choiceAt?: number } | null>): Promise<void> {
    const key = JSON.stringify(items);
    const run = (pending.get(id) ?? Promise.resolve()).then(async () => {
      if (sent.get(id) === key) return;
      const ok = await ask(items);
      if (ok?.choiceAt !== undefined) { mine.set(id, Math.max(mine.get(id) ?? 0, ok.choiceAt)); sent.set(id, key); }
    });
    const tail = run.finally(() => { if (pending.get(id) === tail) pending.delete(id); });
    pending.set(id, tail);
    return tail;
  },
  /** Every `choose` of `id` on its way, landed. */
  flush: (id: string): Promise<void> => pending.get(id) ?? Promise.resolve(),
  /** Whether a choice of `id` was sent (a card whose flips are undone sends its base again). */
  sentFor: (id: string): boolean => sent.has(id),
  /** A number for each answer of the tool's `sendChoices`. */
  answer: (): number => ++answers,
  /** A new server run: nothing known. */
  forget() { bases.clear(); mine.clear(); sent.clear(); pending.clear(); rebased = new Set(); },
};
