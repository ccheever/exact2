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
// A record of the page's own last `choose` not yet answered (its client, the items it asked last), whose answer may
// have been lost, is the page's own: client names are shared by a person's tabs, so the items tell it apart, and the
// same items from another tab under the name would be the same base. Only the last: an earlier ask's items, chosen
// again by another tab after the page moved on, are another client's choice. A rebase or an answer ends it. Only the flips `picked` held when a rebase was taken are obsolete: one
// made after it counts.
type Base = { at: number; choice: number[] | null };
type Record = { proposal: unknown; choice?: number[]; choiceBy?: string | null; choiceAt?: number };
const bases = new Map<string, Base>(), latest = new Map<string, Base>(), mine = new Map<string, number>(), sent = new Map<string, string>();
const pending = new Map<string, Promise<boolean>>(), touched = new Set<string>(), gens = new Map<string, number>(), asked = new Map<string, string>();
// (`rebased`: a card whose flips are not yet taken, all of them obsolete until the page's next answer takes them; `stale`, those taken)
const rebased = new Set<string>(), stale = new Map<string, Set<string>>();
let answers = 0, epoch = '', run = 0, dirty = false;

/** `id`'s base replaced by `b`, another client's: the page's flips on it obsolete from now, its cards to be shown again. */
function rebase(id: string, b: Base) { bases.set(id, b); sent.delete(id); touched.delete(id); rebased.add(id); stale.delete(id); asked.delete(id); gens.set(id, (gens.get(id) ?? 0) + 1); dirty = true; }
/** Whether a record newer than `id`'s base and than the page's own choose of it, so another client's, is kept. */
const foreign = (id: string) => { const b = bases.get(id), l = latest.get(id); return !!b && !!l && l.at > b.at && l.at > (mine.get(id) ?? 0); };
/** After the page's own `choose` of `id` landed (or failed): a newer record kept meanwhile, not its echo, rebases. */
function settle(id: string) { if (foreign(id)) rebase(id, latest.get(id)!); }
/** Whether the page's flip `f` (`<proposal>:<i[,j]>`) no longer counts: its card rebased and not yet taken, or `f` in
 * `picked` when the rebase was taken. */
const obsolete = (f: string) => { const id = f.slice(0, f.indexOf(':')); return rebased.has(id) || !!stale.get(id)?.has(f); };

export const heldChoice = {
  /** The held list as read on server run `on`: each proposal's base taken at first sight, or again when another client
   * chose since (kept, while the page's own `choose` of it is on its way, until that lands). A proposal missing from a
   * list (out being accepted, and back if that fails) keeps its base. A new run knows nothing. `client`: this page's,
   * whose record of the items it last asked, with no answer yet, is its own (its `choiceAt` taken as the page's, its answer lost or not). */
  saw(records: Record[], on: string, client = '') {
    if (on !== epoch) { this.forget(); epoch = on; }
    for (const p of records) {
      const id = String(p.proposal), seen = { at: p.choiceAt ?? 0, choice: p.choice ?? null };
      if (client && p.choiceBy === client && seen.choice && asked.get(id) === JSON.stringify(seen.choice)) mine.set(id, Math.max(mine.get(id) ?? 0, seen.at));
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
  stale: (f: string): boolean => obsolete(f),
  /** Whether a base moved since the cards were last built from the list: they are built again (`takeShown`). */
  shownStale: (): boolean => dirty,
  /** The cards were built from the bases as they are now (each card carries its `gen`, as `choiceGen`). */
  takeShown() { dirty = false; },
  /** Whether `id`'s card, as the page shows it (built at `shownGen`, its `choiceGen`) with `picked`, is what the page
   * would accept or carry: built from its base as it is now, with none of its flips made obsolete by another client's
   * choice still in `picked`. An accept or an edit asks for another look when not. (The page's own card's number, not
   * the module's build: a card built again is shown only once the page has it.) */
  inStep: (id: string, picked: string, shownGen: number): boolean => shownGen === (gens.get(id) ?? 0) && !picked.split(';').some(f => f.startsWith(`${id}:`) && obsolete(f)),
  /** The flips in `picked` that no longer count, to drop from it: a rebased card's as `picked` has them at its first
   * answer since (a flip made after that counts), said again each time until `picked` has none of them, so a dropped
   * answer loses nothing. */
  drop(picked: string[]): string[] {
    for (const id of rebased) stale.set(id, new Set(picked.filter(f => f.startsWith(`${id}:`))));
    rebased.clear();
    const out = picked.filter(obsolete);
    for (const [id, flips] of stale) { for (const f of flips) if (!picked.includes(f)) flips.delete(f); if (!flips.size) stale.delete(id); }
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
      asked.set(id, key);
      const ok = await ask(items);
      if (at !== run || ok?.choiceAt === undefined) return false;
      asked.delete(id); // (answered: every ask before it was taken first, one at a time a proposal)
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
  forget() { bases.clear(); latest.clear(); mine.clear(); sent.clear(); pending.clear(); touched.clear(); rebased.clear(); stale.clear(); asked.clear(); gens.clear(); dirty = false; run += 1; },
};
