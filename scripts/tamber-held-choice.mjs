// A held proposal's choice (tamber/1.18, theme2's 23:24 ruling on #6's follow-up): which of its operations the
// reviewer has chosen, kept on the proposal itself so a reload, a second client or another device sees the same ticks.
// One choice a proposal, the last to choose setting it: `choice` (the sorted indices), `choiceBy` (the client that
// chose) and `choiceAt` (how many times it has been chosen), on its record and every `proposal` event. `choose` sets
// it and only records it: it writes and plans nothing. An `accept` with no `items` takes the choice (a recompute when
// it is part, the whole when it is all); one that names the `choiceAt` it saw is refused 409 `{stale, choice,
// choiceBy, choiceAt}` when another choice came since, so no client accepts a choice it hasn't seen. `edit-held`
// may name it too, refused alike, and carries the choice to the proposal it holds (tamber-edit-held.mjs); a
// recompute's new proposal has none, all of it being chosen.

/** `items`, sorted with no duplicates, if it lists indices of `count` operations; else null. */
const indices = (items, count) =>
  Array.isArray(items) && items.length <= count && items.every(i => Number.isInteger(i) && i >= 0 && i < count) ? [...new Set(items)].sort((a, b) => a - b) : null;

/** `choose` `{client, proposal, items}` over the server's held proposals: the choice recorded and announced. */
export function choose(request, { expire, proposals, stale, bump, announce }) {
  expire();
  const id = request.proposal, p = proposals.get(id);
  if (!p || p.busy) return (!p && stale(id)) || { status: 404, error: { message: `no held proposal \`${id}\` (accepted, dropped, being accepted or edited, or expired)` } };
  const choice = indices(request.items, p.ops.length);
  if (!choice) return { status: 400, error: { message: `\`items\` lists indices of the proposal's ${p.ops.length} operations` } };
  p.shown = { ...p.shown, choice, choiceBy: request.client, choiceAt: (p.shown.choiceAt ?? 0) + 1 };
  bump();
  announce(p.shown);
  return { ok: { proposal: id, choice, choiceBy: request.client, choiceAt: p.shown.choiceAt } };
}

/** A refusal when `request` names a `choiceAt` that isn't held proposal `p`'s (another client's choice since: 409
 * stale, so it can `verb` again once it has seen it), or one that isn't a whole number; else null. */
export function staleChoice(request, p, verb) {
  const { choice, choiceBy = null, choiceAt = 0 } = p.shown;
  if (request.choiceAt === undefined) return null;
  if (!Number.isSafeInteger(request.choiceAt) || request.choiceAt < 0) return { status: 400, error: { message: '`choiceAt` is the proposal\'s choice as last seen, a whole number' } };
  if (request.choiceAt === choiceAt) return null;
  return { status: 409, error: { message: `held proposal \`${request.proposal}\`'s choice was changed${choiceBy ? ` by ${choiceBy}` : ''} since: review it, then ${verb}`, stale: true, choice: choice ?? null, choiceBy, choiceAt } };
}

/** What an `accept` of held proposal `p` takes: `{items}` (undefined for the whole), or a refusal. `choiceAt`, when
 * the request names one, must be the record's: another client's choice since is 409 stale. */
export function accepted(request, p) {
  const { choice } = p.shown, refused = staleChoice(request, p, 'accept');
  if (refused) return refused;
  if (request.items !== undefined || !choice) return { items: request.items };
  if (!choice.length) return { status: 400, error: { message: `nothing is chosen in held proposal \`${request.proposal}\`: choose some of it, or drop it` } };
  return { items: choice.length < p.ops.length ? choice : undefined };
}

/** A held proposal's choice carried to the one an edit holds of operations `kept` (indices into it, in order): the
 * chosen ones it keeps, renumbered, as the record's fields; nothing when it had no choice. */
export function carried(shown, kept) {
  if (!shown.choice) return undefined;
  return { choice: shown.choice.filter(i => kept.includes(i)).map(i => kept.indexOf(i)), choiceBy: shown.choiceBy ?? null, choiceAt: 1 };
}
