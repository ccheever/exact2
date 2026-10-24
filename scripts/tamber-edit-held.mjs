// `edit-held` (tamber/1.17, LLP 1122; theme2's ruling 2026-10-24): one value of a held proposal changed, the rest as it
// is held. A `{client, proposal, index, when?, value, items?}` request sends no ops: the server takes the proposal out,
// sets the op's value (its own, or its `when`th branch's), keeps `items`, and holds that again in its place on the
// files as they are, as `accept`'s recompute does: the original client's still (its owner, who may accept or drop
// it), carrying `editedBy`, the last editor's client (the owner's own name when it edits its own), its review
// renumbered and marked `kept` (its overrides were ticked) and `from` the old one. Answered as a recompute (`{recomputed, replaces, ...}`);
// the old one's end is `proposal-dropped {reason: 'edited', replacedBy, editedBy}`. A server re-import is held as an
// edit: the value given, not marked again. A plan that fails puts the old one back as it was.

/** The edit `request` makes of held proposal `p` (`{ops, review}`, the ops kept and the review for them), or a
 * refusal (`{status, error}`). */
export function edited(request, p) {
  const bad = message => ({ status: 400, error: { message } });
  const { index, when, value, items } = request;
  if (!Number.isInteger(index) || index < 0 || index >= p.ops.length) return bad(`\`index\` is one of the proposal's ${p.ops.length} operations`);
  const op = p.ops[index];
  if (op?.op !== 'add-token' && op?.op !== 'set-token') return bad('`index` names an `add-token` or a `set-token`: only these carry a value');
  if (when !== undefined && (!Number.isInteger(when) || when < 1 || !Array.isArray(op.whens) || when > op.whens.length)) return bad(`\`when\` is one of the operation's ${op.whens?.length ?? 0} \`when\` branches, from 1`);
  if (typeof value !== 'string' || !value.trim()) return bad('`value` is the new value, a string');
  if (items !== undefined && (!Array.isArray(items) || items.some(i => !Number.isInteger(i) || i < 0 || i >= p.ops.length))) return bad(`\`items\` lists indices of the proposal's ${p.ops.length} operations`);
  const kept = items === undefined ? p.ops.map((_, i) => i) : [...new Set(items)].sort((a, b) => a - b);
  if (!kept.includes(index)) return bad('`items` keeps the operation it edits');
  const copy = { ...op, ...(op.whens ? { whens: op.whens.map(w => ({ ...w })) } : {}) };
  if (when === undefined) copy.value = value.trim(); else copy.whens[when - 1].value = value.trim();
  const ops = kept.map(i => (i === index ? copy : p.ops[i]));
  // The review as a recompute renumbers it, with the client's marks (dials, pins) not carried past its operations.
  const old = p.shown.review;
  const review = old && {
    ...old, kept: true, from: p.shown.proposal,
    offers: (old.offers ?? []).map(o => ({ ...o, items: (o.items ?? []).map(i => kept.indexOf(i)).filter(i => i >= 0) })),
    ...(old.tokens === undefined ? {} : { tokens: kept.filter(i => i < old.tokens).length }),
  };
  if (review) { delete review.dials; delete review.pins; }
  return { ops, review };
}

/** `edit-held`, over the server's held proposals: `take(id)` takes one out (a concurrent accept, drop or edit then
 * finds none), `restore(id, p)` puts it back, `hold(...)` holds as `tamber-server.mjs` `hold` does, `announce(event)`
 * sends an event. */
export async function editHeld(request, { expire, proposals, take, restore, hold, announce }) {
  expire();
  const id = request.proposal, p = proposals.get(id);
  if (!p) return { status: 404, error: { message: `no held proposal \`${id}\` (accepted, dropped, being accepted or expired)` } };
  const made = edited(request, p);
  if (made.error) return made;
  take(id);
  // Its guards stay its guards (not those it inferred); planned on the files as they are, as a re-hold is.
  const own = Object.fromEntries(Object.entries(p.guards).filter(([path]) => !(p.inferred ?? []).includes(path)));
  let fresh;
  try { fresh = await hold(p.client, made.ops, undefined, id, made.review, own, 'write', undefined, p.restoring, request.client); } catch (e) { restore(id, p); throw e; }
  if (!fresh.ok) { restore(id, p); return fresh; }
  announce({ event: 'proposal-dropped', proposal: id, client: request.client, reason: 'edited', replacedBy: fresh.ok.proposal, editedBy: request.client });
  return { ok: { recomputed: true, replaces: id, ...fresh.ok } };
}
