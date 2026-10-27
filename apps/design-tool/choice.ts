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
  // An override partly chosen (another client's choice can be) is off, as its box shows it: none of it is taken.
  for (const o of (r?.offers ?? []) as { items?: number[] }[]) if ((o.items ?? []).some(i => !on.has(i))) for (const i of o.items ?? []) on.delete(i);
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
const pending = new Map<string, Promise<boolean>>(), touched = new Set<string>(), rebased = new Set<string>(), gens = new Map<string, number>(), shown = new Map<string, number>();
let answers = 0, epoch = '', run = 0, dirty = false;

/** `id`'s base replaced by `b`, another client's: the page's flips on it obsolete from now, its cards to be shown again. */
function rebase(id: string, b: Base) { bases.set(id, b); sent.delete(id); touched.delete(id); rebased.add(id); gens.set(id, (gens.get(id) ?? 0) + 1); dirty = true; }
/** Whether a record newer than `id`'s base and than the page's own choose of it, so another client's, is kept. */
const foreign = (id: string) => { const b = bases.get(id), l = latest.get(id); return !!b && !!l && l.at > b.at && l.at > (mine.get(id) ?? 0); };
/** After the page's own `choose` of `id` landed (or failed): a newer record kept meanwhile, not its echo, rebases. */
function settle(id: string) { if (foreign(id)) rebase(id, latest.get(id)!); }

export const heldChoice = {
  /** The held list as read on server run `on`: each proposal's base taken at first sight, or again when another client
   * chose since (kept, while the page's own `choose` of it is on its way, until that lands). A proposal missing from a
   * list (out being accepted, and back if that fails) keeps its base. A new run knows nothing. */
  saw(records: Record[], on: string) {
    if (on !== epoch) { this.forget(); epoch = on; }
    for (const p of records) {
      const id = String(p.proposal), seen = { at: p.choiceAt ?? 0, choice: p.choice ?? null };
      latest.set(id, seen);
      if (!bases.has(id)) bases.set(id, seen);
      else if (!pending.has(id)) settle(id);
    }
  },
  /** The card's base for `id`: the server's choice, or null for the card's default. */
  base: (id: string): number[] | null => bases.get(id)?.choice ?? null,
  /** How many times another client's choice has replaced `id`'s base on this page: an accept or an edit that saw one
   * number and finds another acts on nothing it was shown. */
  gen: (id: string): number => gens.get(id) ?? 0,
  /** The `choiceAt` an accept of `id` names: the newest this page knows, its own `choose`s' included. */
  at: (id: string): number => Math.max(mine.get(id) ?? 0, bases.get(id)?.at ?? 0),
  /** Whether the page's flip `f` (`<proposal>:<i[,j]>`) no longer counts: made over a base since replaced. */
  stale: (f: string): boolean => rebased.has(f.slice(0, f.indexOf(':'))),
  /** Whether a base moved since the cards were last built from the list: they are built again (`takeShown`). */
  shownStale: (): boolean => dirty,
  /** The cards were built from the bases as they are now. */
  takeShown() { dirty = false; for (const id of bases.keys()) shown.set(id, gens.get(id) ?? 0); },
  /** Whether `id`'s card, as shown with `picked`, is what the page would accept or carry: built from its base as it is
   * now, with none of its flips made obsolete by another client's choice still in `picked`. An accept or an edit asks
   * for another look when not. */
  inStep: (id: string, picked: string): boolean => (shown.get(id) ?? 0) === (gens.get(id) ?? 0) && !(rebased.has(id) && picked.split(';').some(f => f.startsWith(`${id}:`))),
  /** The flips in `picked` that no longer count, to drop from it; said again each time until `picked` has none of a
   * rebased card's, so a dropped answer loses nothing. */
  drop(picked: string[]): string[] {
    const out = picked.filter(f => rebased.has(f.slice(0, f.indexOf(':'))));
    for (const id of [...rebased]) if (!picked.some(f => f.startsWith(`${id}:`))) rebased.delete(id);
    return out;
  },
  /** Whether `id`'s card has been ticked on this page since its base (its choice is the page's to send, flips or none). */
  touchedFor: (id: string): boolean => touched.has(id),
  /** `items` sent as `id`'s choice (`ask`, the request), after any sent before it and only when it differs from the
   * last sent; latest wins, one at a time a proposal. True once the server has it as its newest; false when it
   * didn't (no answer, a new run) or when another client's choice came since (left to replace the base, not overwritten). */
  send(id: string, items: number[], ask: (items: number[]) => Promise<{ choiceAt?: number } | null>): Promise<boolean> {
    const key = JSON.stringify(items), at = run;
    touched.add(id);
    const go = (pending.get(id) ?? Promise.resolve(true)).then(async () => {
      if (foreign(id)) return false;
      if (sent.get(id) === key) return true;
      sent.delete(id); // (an answer that never comes leaves the server's choice unknown: the next ask sends again)
      const ok = await ask(items);
      if (at !== run || ok?.choiceAt === undefined) return false;
      mine.set(id, Math.max(mine.get(id) ?? 0, ok.choiceAt));
      if (foreign(id)) return false;
      sent.set(id, key);
      return true;
    });
    const tail = go.finally(() => { if (pending.get(id) === tail) { pending.delete(id); settle(id); } });
    pending.set(id, tail);
    return tail;
  },
  /** Every `choose` of `id` on its way, landed: true when the last of them reached the server as its newest. */
  flush: (id: string): Promise<boolean> => pending.get(id) ?? Promise.resolve(true),
  /** A number for each answer of the tool's `sendChoices`. */
  answer: (): number => ++answers,
  /** Nothing known (a new server run); answers still on their way are ignored. */
  forget() { bases.clear(); latest.clear(); mine.clear(); sent.clear(); pending.clear(); touched.clear(); rebased.clear(); gens.clear(); shown.clear(); dirty = false; run += 1; },
};
