// Held proposals replaced since a client last saw them (tamber/1.17, the third hands-on pass's #2): an `accept`,
// `drop` or `edit-held` of one is refused 409 `{stale: true, replacedBy, by}`, so a client never acts on a proposal
// someone else changed without being told who. A proposal is replaced by an `edit-held` (its editor) or by a recompute
// at an `accept` (its accepter); each says so as `proposal-dropped` with `replacedBy`, and that event is what this
// keeps. `replacedBy` is the latest of a chain (A replaced it, then C replaced A's), and `by` the client that made the
// latest: one client's name, unlike a transaction's file-to-who `by`. At most 256 replaced ids are kept, oldest first;
// one past that, as one never held or held and since dropped, accepted or expired, answers 404.
const LIMIT = 256;

export function replacedProposals() {
  const replaced = new Map(); // id → { replacedBy, by }
  return {
    /** An event the server sends: a replacement's `proposal-dropped` is kept. */
    record(event) {
      if (event?.event !== 'proposal-dropped' || typeof event.replacedBy !== 'string' || (event.reason !== 'replaced' && event.reason !== 'edited')) return;
      replaced.delete(event.proposal);
      replaced.set(event.proposal, { replacedBy: event.replacedBy, by: event.editedBy ?? event.client ?? null });
      while (replaced.size > LIMIT) replaced.delete(replaced.keys().next().value);
    },
    /** The 409 for `id` if it was replaced, its chain followed to the latest; else null. */
    stale(id) {
      let at = replaced.get(id);
      if (!at) return null;
      for (let hops = 0, next; hops < LIMIT && (next = replaced.get(at.replacedBy)); hops++) at = next;
      return { status: 409, error: { message: `held proposal \`${id}\` was replaced by ${at.replacedBy}${at.by ? `, by ${at.by}` : ''}: act on that one`, stale: true, replacedBy: at.replacedBy, by: at.by } };
    },
  };
}
