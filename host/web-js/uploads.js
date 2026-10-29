// The uploads a resource has asked (LLP 1016.000 D10), as the runner keeps
// them (runner/src/runner/uploads.rs): for each, the identifying arguments
// it was asked at (not its `with` context) and how it stands, `t` its
// ticket while on its way, `v` the answer it ended with, `failed`, or
// `gone`, an older one that went out and whose answer is no longer kept.
// Nothing asks an upload again at arguments it has a mark for. The marks
// last as long as the page.
import { equal } from './shape.js';

export const INTERRUPTED = ['the upload was cut off before its reply; it is not sent again at these arguments', 'error', 'interrupted'];
// How many of a resource's uploads keep the answer each ended with, the newest (the runner's `ANSWERS_KEPT`).
const ANSWERS_KEPT = 32;
const ALREADY_SENT = [`already sent: the upload went out at these arguments, and of a resource's uploads only the newest ${ANSWERS_KEPT} keep their answers; give new arguments to send it again`, 'error', 'already-sent'];
const identity = (r, a) => (r.context ? a.slice(0, a.length - r.context) : a);

/** The upload `r` asked at arguments `a`, if it did. */
export const uploadAt = (r, a) => r.marks.findLast(m => equal(m.args, identity(r, a)));
/** `r` asked an upload at `a`, on its way as ticket `t`. */
export function uploadSent(r, a, t) {
  const args = identity(r, a);
  r.marks = [...r.marks.filter(m => !equal(m.args, args)), { args, t }];
}
/** Ticket `t`'s upload ended (`{ v }`, or `{ failed: [error, code, reason] }`) once it reached the network (`t.ctl.sent`: its
 * start or its first bytes sent); one that never did is forgotten, an ordinary answer that `refresh` asks again. Of the
 * resource's ended uploads only the newest `ANSWERS_KEPT` keep how they ended. */
export function uploadEnded(r, t, how) {
  if (!t.ctl?.sent) { r.marks = r.marks.filter(m => m.t !== t); return; }
  const marks = r.marks.map(m => m.t === t ? { args: m.args, ...how } : m);
  let over = marks.filter(m => m.v !== undefined || m.failed).length - ANSWERS_KEPT;
  r.marks = marks.map(m => over > 0 && (m.v !== undefined || m.failed) && over-- ? { args: m.args, gone: true } : m);
}
/** A ticket let go on its way (its commit stood): its upload is cut off, `interrupted`. */
export function cutUpload(t) {
  if (t.upload && t.r?.marks.some(m => m.t === t)) uploadEnded(t.r, t, { failed: INTERRUPTED });
}
/** What `r` answers at arguments `a` its upload `mark` was asked at, in place of asking its source: its ticket goes on for
 * them (the same request, LLP 1054.000.000 D3), or it answers as it ended, while that still fits, or with its failure, or
 * that it was sent already. */
export function uploadAnswer(r, mark, a, { take, fails, release }) {
  if (mark.t && mark.t === r.ticket) { r.ticket.args = a; return r.value; }
  if (mark.v === undefined) return fails(a, ...(mark.gone ? ALREADY_SENT : mark.failed ?? INTERRUPTED));
  if (!equal(a, r.settled) || r.ticket) {
    try { take(mark.v, a, r.origin); } catch { return fails(a, "the upload's answer no longer fits the resource's shape; it is not sent again at these arguments", 'shape', 'interrupted'); }
    release();
  }
  return r.value;
}
/** The identity of an upload's request (`exactUploadProgress`) among `fetch`'s arguments, which the same request for newer
 * arguments shares (D3): its URL, method, headers, body (by length and FNV-1a), file, deadline and redirect mode. */
export function requestKey({ input, init, redirect }) {
  if (init?.exactUploadProgress !== true) return undefined;
  const request = typeof Request === 'function' && input instanceof Request ? input : null, body = init.body;
  const bytes = body == null ? null : body instanceof ArrayBuffer ? new Uint8Array(body) : ArrayBuffer.isView(body) ? new Uint8Array(body.buffer, body.byteOffset, body.byteLength) : new TextEncoder().encode(String(body));
  let hash = 0x811c9dc5;
  for (const b of bytes ?? []) hash = Math.imul(hash ^ b, 16777619) >>> 0;
  return JSON.stringify([request ? request.url : String(input), init.method ?? request?.method ?? 'GET', [...new Headers(init.headers ?? request?.headers ?? [])].sort(),
    bytes ? [bytes.length, hash] : null, init.exactBodyFrom ?? null, init.exactTimeout ?? null, redirect ?? null]);
}
