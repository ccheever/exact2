// `GET /__tamber/events?after=<cursor>&epoch=<e>&client=<c>` (tamber/1.17, LLP 1122 stage 14 D): the change
// stream's long poll for a client that re-asks by cursor (LLP 1016.000 D8's `exactLongPoll`), beside the POST
// form. Its first ask names no `after` and is answered at once (a host waits on a stream's first reply): from
// `open=<cursor>` (one a POST read's answer gave) if given, which no later ask reads, the host setting `after`.
// The cursor is opaque to a client, which only compares it: here `<seq>.<held>`, the events' sequence and the held
// proposals' revision, since the list moves without events.

import { TIMING } from './tamber-timing.mjs';

/** How long a client stays present after its GET long poll's answer (LLP 1122 D4), 90 s (a test's may be shorter:
 * tamber-timing.mjs): the long poll re-asks after a backoff of up to 30 s (the server sends no longer `Retry-After`
 * here), and a POST client's 35 s would lose its pins in that wait. */
export const LONG_POLL_PRESENT_MS = TIMING.longPresent;

/** The cursor of an answer at `seq` with held revision `held`. */
export const cursorOf = (seq, held) => `${seq}.${held}`;

const CURSOR = /^(0|[1-9][0-9]{0,15})(?:\.(0|[1-9][0-9]{0,15}))?$/;

/** A GET's query as the POST form's request (`{after, held?, epoch?, client?, now?}`, `now` with no `after`: answered
 * at once), or `{error}`: each bounded as the body's fields are. */
export function eventsQuery(url) {
  const q = url.searchParams, request = {};
  for (const key of q.keys()) {
    if (!['after', 'open', 'epoch', 'client'].includes(key)) return { error: `\`${key}\` is no parameter of the events long poll (after, open, epoch, client)` };
    if (q.getAll(key).length > 1) return { error: `\`${key}\` is given once` };
  }
  if (!q.has('after')) request.now = true;
  // (`open` read only with no `after`: beside one, it is not looked at, as a host's re-asks keep it.)
  for (const key of q.has('after') ? ['after'] : ['open']) {
    if (!q.has(key)) continue;
    const m = CURSOR.exec(q.get(key));
    const after = m && Number(m[1]), held = m?.[2] === undefined ? undefined : Number(m[2]);
    if (!m || !Number.isSafeInteger(after) || (held !== undefined && !Number.isSafeInteger(held))) return { error: `\`${key}\` is a cursor this server gave` };
    request.after = after;
    if (held !== undefined) request.held = held;
  }
  for (const key of ['epoch', 'client']) {
    if (!q.has(key)) continue;
    const v = q.get(key);
    if (!v || v.length > 256) return { error: `\`${key}\` is 1 to 256 characters` };
    request[key] = v;
  }
  return { request };
}
