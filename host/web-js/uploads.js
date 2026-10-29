// The uploads a resource has asked (LLP 1016.000 D10), as the runner keeps
// them (runner/src/runner/uploads.rs): for each, the arguments it was asked
// at and how it stands, `t` its ticket while on its way, `v` the answer it
// ended with, or `failed`. Nothing asks an upload again at arguments it has
// a mark for. The marks are kept in the page's storage for the next launch,
// once the commit that changed them stands; one still on its way then comes
// back failed, `interrupted`, and is not sent again.
import { equal } from './shape.js';

export const INTERRUPTED = ['the upload was cut off before its reply; it is not sent again at these arguments', 'error', 'interrupted'];
// The most one resource's kept marks take, as JSON text: the oldest are forgotten first.
const KEPT_BYTES = 64 * 1024;
const dirty = new Set();

/** The marks resource `r` kept under `key` at the last launch. */
export function loadUploads(r, key) {
  r.marksKey = key; r.marks = [];
  try { r.marks = (JSON.parse(localStorage.getItem(key) ?? '[]')).map(m => m.sent ? { args: m.args, failed: INTERRUPTED } : m); } catch {}
}
/** The upload `r` asked at arguments `a`, if it did. */
export const uploadAt = (r, a) => r.marks.findLast(m => equal(m.args, a));
/** `r` asked an upload at `a`, on its way as ticket `t`. */
export function uploadSent(r, a, t) { r.marks = [...r.marks.filter(m => !equal(m.args, a)), { args: a, t }]; dirty.add(r); }
/** Ticket `t`'s upload ended (`{ v }`, or `{ failed: [error, code, reason] }`) once it reached the network (`t.ctl.sent`: its start
 * or its first bytes sent); one that never did is forgotten, an ordinary answer that `refresh` asks again. */
export function uploadEnded(r, t, how) {
  r.marks = t.ctl?.sent ? r.marks.map(m => m.t === t ? { args: m.args, ...how } : m) : r.marks.filter(m => m.t !== t);
  dirty.add(r);
}
/** A ticket let go on its way (its commit stood): its upload is cut off, `interrupted`. */
export function cutUpload(t) {
  if (t.upload && t.r?.marks?.some(m => m.t === t)) { uploadEnded(t.r, t, { failed: INTERRUPTED }); keepUploads(); }
}
/** What `r` answers at arguments `a` its upload `mark` was asked at, in place of asking its source: its ticket goes on for
 * them (the same request, LLP 1054.000.000 D3), or it answers as it ended, while that still fits, or with its failure. */
export function uploadAnswer(r, mark, a, { take, fails, release }) {
  if (mark.t && mark.t === r.ticket) { r.ticket.args = a; return r.value; }
  if (mark.v === undefined) return fails(a, ...(mark.failed ?? INTERRUPTED));
  if (!equal(a, r.settled) || r.ticket) {
    try { take(mark.v, a, r.origin); } catch { return fails(a, "the upload's answer no longer fits the resource's shape; it is not sent again at these arguments", 'shape', 'interrupted'); }
    release();
  }
  return r.value;
}
/** Keep the marks changed since the last commit stood, each resource's newest within its bound. */
export function keepUploads() {
  for (const r of dirty) {
    let marks = r.marks.map(m => m.t ? { args: m.args, sent: true } : m), text = JSON.stringify(marks);
    while (text.length > KEPT_BYTES && marks.length) text = JSON.stringify(marks = marks.slice(1));
    try { marks.length ? localStorage.setItem(r.marksKey, text) : localStorage.removeItem(r.marksKey); } catch {}
  }
  dirty.clear();
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
