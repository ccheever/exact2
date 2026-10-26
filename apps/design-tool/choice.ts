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
  const tokens = Number(r?.tokens ?? 0), recomputed = startsChosen(h, r), chose = heldChoice.base(proposal), base = chose && new Set(chose);
  const begins = (i: number) => (base ? base.has(i) : i < tokens || recomputed);
  const on = new Set(ops.map((_, i) => i).filter(begins));
  // Each flip the page made over this base (not one made obsolete by another client's choice since): a token's turns
  // it over; an override's, its items together, turns the whole override over from how it began (all of it in, or not).
  for (const f of picked.split(';').filter(x => x.startsWith(`${proposal}:`) && !heldChoice.stale(x))) {
    const items = f.slice(proposal.length + 1).split(',').map(Number), was = items.every(begins);
    for (const i of items) if (was) on.delete(i); else on.add(i);
  }
  return [...on].sort((a, b) => a - b);
}

/** The card's choice (`was`, the items it had chosen) as the ids to flip on `fresh`, the import held again whole and
 * beginning fresh: each of its first `tokens` items the card had left out, and each role override it had ticked. */
export function carriedChoice(fresh: string, tokens: number, offers: { items?: number[] }[], was: Set<number>): string[] {
  return [
    ...Array.from({ length: tokens }, (_, i) => i).filter(i => !was.has(i)).map(i => `${fresh}:${i}`),
    ...offers.filter(o => (o.items ?? []).length > 0 && o.items!.every(i => was.has(i))).map(o => `${fresh}:${o.items!.join(',')}`),
  ];
}

// The choice on the server (tamber/1.18 `choose`, #6's follow-up): what the card has chosen is sent as it is ticked,
// so a reload or another client starts from it. The card shows a base, the server's choice as this page took it (or
// the card's default when there was none), with the page's flips over it. The page's own `choose` answers leave the
// base be (its flips already say so). Another client's choice since replaces the base, and the page's flips on that
// card become obsolete at once: never sent, never accepted, then dropped from `picked` (the last to choose wins, as on
// the server). A newer record that comes while the page's own `choose` is on its way is kept, and settled when it lands.
type Base = { at: number; choice: number[] | null };
type Record = { proposal: unknown; choice?: number[]; choiceAt?: number };
const bases = new Map<string, Base>(), latest = new Map<string, Base>(), mine = new Map<string, number>(), sent = new Map<string, string>();
const pending = new Map<string, Promise<boolean>>(), touched = new Set<string>(), rebased = new Set<string>(), obsolete = new Set<string>();
let answers = 0, epoch = '', run = 0;

/** `id`'s base replaced by `b`, another client's: the page's flips on it obsolete from now. */
function rebase(id: string, b: Base) { bases.set(id, b); sent.delete(id); touched.delete(id); rebased.add(id); }
/** After the page's own `choose` of `id` landed (or failed): a newer record kept meanwhile, not its echo, rebases. */
function settle(id: string) {
  const b = bases.get(id), l = latest.get(id);
  if (b && l && l.at > b.at && l.at > (mine.get(id) ?? 0)) rebase(id, l);
}

export const heldChoice = {
  /** The held list as read on server run `on`: each proposal's base taken at first sight, or again when another client
   * chose since (kept, while the page's own `choose` of it is on its way, until that lands). A new run knows nothing. */
  saw(records: Record[], on: string) {
    if (on !== epoch) { this.forget(); epoch = on; }
    const now = new Set<string>();
    for (const p of records) {
      const id = String(p.proposal), seen = { at: p.choiceAt ?? 0, choice: p.choice ?? null }, b = bases.get(id);
      now.add(id); latest.set(id, seen);
      if (!b) bases.set(id, seen);
      else if (!pending.has(id)) settle(id);
    }
    for (const id of [...bases.keys()]) if (!now.has(id)) { bases.delete(id); latest.delete(id); mine.delete(id); sent.delete(id); touched.delete(id); rebased.delete(id); }
  },
  /** The card's base for `id`: the server's choice, or null for the card's default. */
  base: (id: string): number[] | null => bases.get(id)?.choice ?? null,
  /** The `choiceAt` an accept of `id` names: the newest this page knows, its own `choose`s' included. */
  at: (id: string): number => Math.max(mine.get(id) ?? 0, bases.get(id)?.at ?? 0),
  /** Whether the page's flip `f` (`<proposal>:<i[,j]>`) no longer counts: made over a base since replaced. */
  stale: (f: string): boolean => obsolete.has(f) || rebased.has(f.slice(0, f.indexOf(':'))),
  /** Whether a card was rebased since `drop` last ran (its flips still in `picked`). */
  pendingDrop: (): boolean => rebased.size > 0,
  /** `picked` as it is now: the flips on cards rebased since made obsolete (they count no more, from now) and returned
   * to drop from `picked`; obsolete ones `picked` no longer has forgotten. */
  drop(picked: string[]): string[] {
    for (const f of [...obsolete]) if (!picked.includes(f)) obsolete.delete(f);
    const out = picked.filter(f => rebased.has(f.slice(0, f.indexOf(':'))));
    for (const f of out) obsolete.add(f);
    rebased.clear();
    return out;
  },
  /** Whether `id`'s card has been ticked on this page since its base (its choice is the page's to send, flips or none). */
  touchedFor: (id: string): boolean => touched.has(id),
  /** `items` sent as `id`'s choice (`ask`, the request), after any sent before it and only when it differs from the
   * last sent; latest wins, one at a time a proposal. True once the server has it (or already had it). */
  send(id: string, items: number[], ask: (items: number[]) => Promise<{ choiceAt?: number } | null>): Promise<boolean> {
    const key = JSON.stringify(items), at = run;
    touched.add(id);
    const go = (pending.get(id) ?? Promise.resolve(true)).then(async () => {
      if (sent.get(id) === key) return true;
      const ok = await ask(items);
      if (at !== run) return false; // (a new server run meanwhile)
      if (ok?.choiceAt === undefined) return false;
      mine.set(id, Math.max(mine.get(id) ?? 0, ok.choiceAt)); sent.set(id, key);
      return true;
    });
    const tail = go.finally(() => { if (pending.get(id) === tail) { pending.delete(id); settle(id); } });
    pending.set(id, tail);
    return tail;
  },
  /** Every `choose` of `id` on its way, landed: true when the last of them reached the server. */
  flush: (id: string): Promise<boolean> => pending.get(id) ?? Promise.resolve(true),
  /** A number for each answer of the tool's `sendChoices`. */
  answer: (): number => ++answers,
  /** Nothing known (a new server run); answers still on their way are ignored. */
  forget() { bases.clear(); latest.clear(); mine.clear(); sent.clear(); pending.clear(); touched.clear(); rebased.clear(); obsolete.clear(); run += 1; },
};
