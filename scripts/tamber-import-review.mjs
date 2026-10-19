// An importer's proposal as the wire holds it (LLP 1121 D14, stage 10b):
// its operations, each role override offered on its own, and a re-import's
// dial changes and marked pins, each naming the operations it is.

/** An importer's proposal (`contract tamber import`'s JSON) as one held
 * proposal's operations: the tokens, then each role override as its own
 * items, for the review to leave off until ticked (accepting some items
 * recomputes); and the review the tool reads. */
export function importReview(imported) {
  const ops = [...imported.ops];
  const offers = imported.offers.map(o => {
    const items = o.ops.map((op, k) => ops.length + k);
    ops.push(...o.ops);
    return { role: o.role, token: o.token, items, note: o.note };
  });
  return { ops, review: { importer: imported.importer, counted: imported.counted, unmapped: imported.unmapped, offers, tokens: imported.ops.length, ...marked(ops, imported.dials, imported.pins) } };
}

/** A re-import's dial changes and marked pins (stage 10b), when it has any,
 * each naming the operations it is (`items`), as an offer does. */
export function marked(ops, dials = [], pins = []) {
  const same = (a, b) => JSON.stringify(a ?? []) === JSON.stringify(b ?? []), at = test => ops.flatMap((op, i) => (test(op) ? [i] : []));
  dials = dials.map(d => ({ ...d, items: at(op => op.op === 'set-dial' && op.name === d.dial) }));
  pins = pins.map(p => ({ ...p, items: at(op => op.op === 'set-token' && op.name === p.token && same(op.when, p.when)) }));
  return { ...(dials.length ? { dials } : {}), ...(pins.length ? { pins } : {}) };
}
