// A client's operations in one request (tamber/1.17, `tamber-ops-size`): at
// most OPS_LIMIT, refused at the design server's door before any is read
// (the checker sweep: each operation is a step over the whole design).
// Operations the server applies itself (accepting a held proposal, a
// revert's, an import it plans) are no client's request.

import { allows } from './tamber-pairing.mjs';

/** The most operations a client's request carries. */
export const OPS_LIMIT = 1024;

/** Whether `request` carries more operations than a client may: past
 * OPS_LIMIT, unless it re-holds a held proposal (`reholds`). */
export function overOps(request, scope, proposals) {
  return Array.isArray(request.ops) && request.ops.length > OPS_LIMIT && !reholds(request, scope, proposals);
}

/** The refusal's error. */
export function opsRefusal(request) {
  return { id: 'tamber-ops-size', message: `a transaction is at most ${OPS_LIMIT} operations; this one has ${request.ops.length}` };
}

/** Whether `request` re-holds a held proposal (theme2's ruling): a
 * `propose` with `hold` naming the proposal it derives from (`review.from`),
 * with at most that proposal's operations, every one on a target (its kind
 * and its token's or param's name) the proposal has, by a client that may
 * replace it (its own, or one with write scope). Its size is one the server
 * planned; every value is still checked as any write's is. */
export function reholds(request, scope, proposals) {
  const from = request.review?.from, p = typeof from === 'string' ? proposals.get(from) : undefined;
  if (request.method !== 'propose' || request.hold !== true || !p || request.ops.length > p.ops.length) return false;
  if (p.client !== request.client && !allows(scope, 'write')) return false;
  const targets = new Set(p.ops.map(target));
  return request.ops.every(op => targets.has(target(op)));
}

// The operations whose `value` (and a new token's `when` lines' values) a
// re-hold may change: every other field, and every field of any other
// operation, is its target.
const VALUED = new Set(['set-token', 'add-token', 'set-dial', 'set-param']);

/** An operation's target: the whole operation but the values a re-hold may
 * edit (Astra's round 1: by its kind and name alone, an axis value's or
 * another file's op could stand in for a held one). */
export function target(op) {
  if (!op || typeof op !== 'object' || !VALUED.has(op.op)) return JSON.stringify(op ?? null);
  const { value, ...rest } = op;
  if (Array.isArray(rest.whens)) rest.whens = rest.whens.map(w => (Array.isArray(w) ? w.slice(0, 1) : w && typeof w === 'object' ? { ...w, value: undefined } : w));
  return JSON.stringify(rest);
}
