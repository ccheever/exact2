// A TypeScript source's stream on the JS target (LLP 1016.000; ts-fetch.js
// records it, ts-data.js loads this on the first): the web host's reader
// (http-body.js) opens it under the module's grants, as the wasm host runs
// one, and the source's `exactStream` maps each message, and the end, to the
// answer now, with the event Hermes gives it (js/src/prelude.js
// `__exact_message`).
import { streamed } from './http-body.js';

const KINDS = ['Response', 'Network', 'Refused', 'Unsupported', 'Aborted'], said = new TextDecoder();
// Hermes's checks and words on `init` (js/src/prelude.js `fetch`): a
// refusal ends the stream with that message, which fails the answer.
function request(input, init) {
  if (typeof init.exactStream !== 'function') throw new TypeError('exactStream maps each event to the answer: (event) => value');
  const independent = init.exactIndependentHttp?.maxResponseBytes;
  if (independent !== undefined && (!Number.isInteger(independent) || independent <= 0 || independent > 67108864))
    throw new TypeError('exactIndependentHttp.maxResponseBytes must be an integer from 1 to 67108864');
  const raw = init.body == null ? undefined : new TextEncoder().encode(String(init.body));
  return { method: String(init.method ?? 'GET').toUpperCase(), url: String(input), headers: [...new Headers(init.headers ?? [])], raw, maxResponseBytes: independent ?? 1048576 };
}
export function open({ input, init }, conv, grantSet, deliver, controller) {
  let req;
  try { req = request(input, init); } catch (e) { return Promise.resolve({ error: String(e?.message ?? e) }); }
  const map = init.exactStream;
  const value = event => {
    try {
      const v = map(event);
      if (v && typeof v.then === 'function') throw new Error('exactStream answers each event now; it cannot await');
      return { v: conv(v) };
    } catch (e) { return { error: String(e?.message ?? e) }; }
  };
  return streamed(req, grantSet, m => deliver({ ...value({ type: m.event || 'message', data: m.data, lastEventId: m.id, coalesced: m.coalesced }), coalesced: m.coalesced }), controller)
    .then(o => value(o.kind
      ? { type: 'error', data: '', lastEventId: '', coalesced: 0, kind: KINDS[o.kind], message: said.decode(o.body), status: 0 }
      : { type: 'error', data: said.decode(o.body), lastEventId: '', coalesced: 0, kind: 'Response', message: `HTTP ${o.status}`, status: o.status }));
}
