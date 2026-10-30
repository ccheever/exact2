// A client's operations in one request (tamber/1.17, `tamber-ops-size`): at
// most OPS_LIMIT, refused at the design server's door before any is read
// (the checker sweep: each operation is a step over the whole design).
// Operations the server applies itself (accepting a held proposal, a
// revert's, an import it plans) are no client's request; a value edited in a
// held proposal is `edit-held`'s, which sends none.

/** The most operations a client's request carries. */
export const OPS_LIMIT = 1024;

/** Whether `request` carries more operations than a client may. */
export function overOps(request) {
  return Array.isArray(request.ops) && request.ops.length > OPS_LIMIT;
}

/** The refusal's error. */
export function opsRefusal(request) {
  return { id: 'tamber-ops-size', message: `a transaction is at most ${OPS_LIMIT} operations; this one has ${request.ops.length}` };
}
