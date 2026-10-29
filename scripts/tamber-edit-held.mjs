// `edit-held` (tamber/1.17, LLP 1122; theme2's ruling 2026-10-24): one value of a held proposal changed, the rest as it
// is held. A `{client, proposal, index, when?, value, items?}` request sends no ops: the server keeps the proposal held but busy,
// sets the op's value (its own, or its `when`th branch's), keeps `items`, and holds that again in its place on the
// files as they are, as `accept`'s recompute does: the original client's still (its owner, who may accept or drop
// it), carrying `editedBy`, the last editor's client (the owner's own name when it edits its own), its review
// renumbered and marked `kept` (its overrides were ticked) and `from` the old one. Answered as a recompute (`{recomputed, replaces, ...}`);
// the old one's end is `proposal-dropped {reason: 'edited', replacedBy, editedBy}`. A server re-import is held as an
// edit: the value given, not marked again; its recorded choice (tamber-held-choice.mjs) carried, renumbered to what it keeps; a held revert is not edited (it is planned from its commit). A plan that
// fails leaves the old one held as it was.

import { carried, staleChoice } from './tamber-held-choice.mjs';

/** The edit `request` makes of held proposal `p` (`{ops, review}`, the ops kept and the review for them), or a
 * refusal (`{status, error}`). */
export function edited(request, p) {
  const bad = message => ({ status: 400, error: { message } });
  const { index, when, value, items } = request;
  if (!Number.isInteger(index) || index < 0 || index >= p.ops.length) return bad(`\`index\` is one of the proposal's ${p.ops.length} operations`);
  const op = p.ops[index];
  if (op?.op !== 'add-token' && op?.op !== 'set-token') return bad('`index` names an `add-token` or a `set-token`: only these carry a value');
  // (A `set-token` writes its value alone: only an `add-token` carries `when` branches the server reads.)
  if (when !== undefined && (op.op !== 'add-token' || !Number.isInteger(when) || when < 1 || !Array.isArray(op.whens) || when > op.whens.length)) return bad(`\`when\` is one of an \`add-token\`'s ${op.op === 'add-token' ? op.whens?.length ?? 0 : 0} \`when\` branches, from 1`);
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
    // Its brand offers (*1.18*) as its offers, but one whose accent this edits: its ink was chosen for the accent's
    // old value, so it goes, and the review says so (an import again offers it with an ink for the new one).
    ...(old.brands === undefined ? {} : { brands: old.brands.filter(b => b.token !== op.name).map(b => ({ ...b, items: (b.items ?? []).map(i => kept.indexOf(i)).filter(i => i >= 0) })).filter(b => b.items.length) }),
    ...(old.brands?.some(b => b.token === op.name) ? { notes: [...(old.notes ?? []), `\`${op.name}\` changed in this edit, so the brand fills set to it aren't offered: import again to have them, with inks for its new value`] } : {}),
    ...(old.tokens === undefined ? {} : { tokens: kept.filter(i => i < old.tokens).length }),
  };
  if (review) { delete review.dials; delete review.pins; }
  return { ops, review, choice: carried(p.shown, kept) };
}

/** `edit-held` by a client at `scope`, over the server's held proposals. The proposal stays held, and counted in
 * the held caps, while its edit is planned, marked `busy` (an `accept`, `drop`, `edit-held` or expiry meanwhile
 * passes it by); `hold` puts the new one in its place in one turn (`replaces`), and a plan that fails leaves it as it
 * was. `announce(event)` sends an event. */
export async function editHeld(request, scope, { expire, proposals, hold, announce, stale = () => null }) {
  expire();
  const id = request.proposal, p = proposals.get(id);
  if (!p || p.busy) return (!p && stale(id)) || { status: 404, error: { message: `no held proposal \`${id}\` (accepted, dropped, being accepted or edited, or expired)` } };
  // A revert is planned from its commit, never from operations a client changed: dropped and reverted again instead.
  if (p.shown.review?.revert !== undefined) return { status: 400, error: { message: 'a held revert is planned from its commit: drop it and revert again to change it' } };
  // (A `choiceAt` named, 1.18: the choice it carries the one the client saw, checked in the turn it is taken.)
  const made = staleChoice(request, p, 'edit') ?? edited(request, p);
  if (made.error) return made;
  // Its guards stay its guards (not those it inferred); planned on the files as they are, as a re-hold is.
  const own = Object.fromEntries(Object.entries(p.guards).filter(([path]) => !(p.inferred ?? []).includes(path)));
  p.busy = true;
  let fresh;
  try { fresh = await hold(p.client, made.ops, undefined, id, made.review, own, scope, undefined, p.restoring, request.client, made.choice); } finally { p.busy = false; }
  if (!fresh.ok) return fresh;
  announce({ event: 'proposal-dropped', proposal: id, client: request.client, reason: 'edited', replacedBy: fresh.ok.proposal, editedBy: request.client });
  return { ok: { recomputed: true, replaces: id, ...fresh.ok } };
}
