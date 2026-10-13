// A TypeScript resource's stream on the JS target (LLP 1016.000 D7): the
// shared web reader owns admission and lifetime; the mapper produces a
// snapshot for every protocol frame. Loaded only for a streaming source.
import { streamed } from './http-body.js';
import { answering } from './ts-fetch.js';
import { clock, journal } from './rt.js';
import { failureCode } from './shape.js';
import { bodyFromRefusal, methodOf, readBodyFile } from './ts-body-from.js';

const KINDS = ['Response', 'Network', 'Refused', 'Unsupported', 'Aborted', , , , , , 'Timeout'], said = new TextDecoder();
// Hermes's checks and words on `init` (js/src/prelude.js `fetch`): a
// refusal ends the stream with that message, which fails the answer.
function request(input, init, redirect) {
  if (typeof init.exactStream !== 'function') throw new TypeError('exactStream maps each event to the answer: (event) => value');
  const independent = init.exactIndependentHttp?.maxResponseBytes;
  if (independent !== undefined && (!Number.isInteger(independent) || independent <= 0 || independent > 67108864))
    throw new TypeError('exactIndependentHttp.maxResponseBytes must be an integer from 1 to 67108864');
  // `exactBodyFrom` (LLP 1108 D6 R2): read as the stream opens (http-body.js).
  const bodyFrom = init.exactBodyFrom === undefined ? undefined : init.exactBodyFrom, refusal = bodyFrom === undefined ? null : bodyFromRefusal(input, init);
  if (refusal) throw new TypeError(refusal);
  const raw = init.body == null ? undefined : new TextEncoder().encode(String(init.body));
  const request = typeof Request === 'function' && input instanceof Request ? input : null;
  return { method: methodOf(input, init), url: request ? request.url : String(input), headers: [...new Headers(init.headers ?? request?.headers ?? [])], raw, bodyFrom, redirect, maxResponseBytes: independent ?? 1048576 };
}
// The module never sees the browser socket. Operations belong to the current
// mutation's answer, and rt.js publishes them only after its commit stands.
function handle(transport, owner) {
  const encoder = new TextEncoder();
  const stage = (op, data, reason, size = 0) => {
    const call = answering.call;
    if (!call?.mutation) throw new Error(`socket.${op} is only available in a mutation's synchronous source`);
    if (owner.signal.aborted || owner.active?.() === false) throw new DOMException('the socket is not open', 'InvalidStateError');
    const operations = call.socketOps ??= [];
    if (operations.length >= 256) throw new DOMException('socket mutation exceeds 256 operations', 'QuotaExceededError');
    if (operations.some(x => x.transport === transport && x.op === 'close')) throw new DOMException('the socket is not open', 'InvalidStateError');
    const bytes = operations.reduce((n, x) => n + x.size, 0) + size;
    transport.check(bytes);
    if (!operations.length) (call.effects ??= []).push(() => {
      // Recheck the whole batch before sending any frame: a previous mutation
      // in the same commit may have filled the browser's buffer or closed it.
      const totals = new Map();
      for (const x of operations) {
        if (x.owner.signal.aborted || x.owner.active?.() === false) throw new DOMException('the socket is not open', 'InvalidStateError');
        totals.set(x.transport, (totals.get(x.transport) ?? 0) + x.size);
      }
      for (const [socket, count] of totals) socket.check(count);
      for (const x of operations) {
        x.transport[x.op](x.data, x.reason);
        journal.push(`t=${clock.now} socket ${x.op}: ${x.size} bytes (${x.owner.streamName})`);
      }
    });
    operations.push({ transport, owner, op, data, reason, size });
  };
  return Object.freeze({
    get readyState() { return owner.signal.aborted ? 3 : transport.readyState; },
    get bufferedAmount() { return transport.bufferedAmount; },
    send(data) {
      let copy, size;
      if (typeof data === 'string') { copy = data; size = encoder.encode(data).length; }
      else if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
        const bytes = data instanceof ArrayBuffer ? new Uint8Array(data) : new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        copy = bytes.slice(); size = copy.byteLength;
      } else throw new TypeError('socket.send expects a string, ArrayBuffer or ArrayBufferView');
      stage('send', copy, undefined, size);
    },
    close(code = 1000, reason = '') {
      if (code !== 1000 && (!Number.isInteger(code) || code < 3000 || code > 4999)) throw new DOMException('invalid socket close code', 'InvalidAccessError');
      if (typeof reason !== 'string' || encoder.encode(reason).length > 123) throw new SyntaxError('socket close reason exceeds 123 UTF-8 bytes');
      stage('close', code, reason);
    },
  });
}
export function open({ input, init, redirect }, conv, grantSet, deliver, controller) {
  let req;
  try { req = request(input, init, redirect); } catch (e) { return Promise.resolve({ error: String(e?.message ?? e), code: failureCode(e) }); }
  // The caller's signal, as Fetch picks it (`init.signal` over the Request's),
  // ends the stream too, before a body is read from a file or after.
  const own = init.signal !== undefined ? init.signal : typeof Request === 'function' && input instanceof Request ? input.signal : null;
  if (own?.aborted) controller.abort(own.reason);
  else own?.addEventListener?.('abort', () => controller.abort(own.reason), { once: true });
  const map = init.exactStream;
  let socket;
  const protocol = /^wss?:/i.test(req.url) ? { socket: transport => { socket = handle(transport, controller); controller.socket = transport; } } : undefined;
  const value = event => {
    if (controller.active?.() === false) return { error: 'the stream was let go', code: 'error' };
    try {
      const v = map(event, socket);
      if (v && typeof v.then === 'function') throw new Error('exactStream answers each event now; it cannot await');
      return { v: conv(v) };
    } catch (e) { return { error: String(e?.message ?? e), code: failureCode(e) }; }
  };
  return streamed(req, grantSet, m => deliver({ ...value({ type: m.event || 'message', data: m.data, lastEventId: m.id, coalesced: m.coalesced }), coalesced: m.coalesced }), controller, readBodyFile, protocol)
    .then(o => {
      if (o.close) journal.push(`t=${clock.now} socket close: ${o.close.code} ${o.close.reason} (${controller.streamName})`);
      return value(o.close ? { type: 'close', data: '', lastEventId: '', coalesced: 0, ...o.close } : o.kind
      ? { type: 'error', data: '', lastEventId: '', coalesced: 0, kind: KINDS[o.kind], message: said.decode(o.body), status: 0 }
      : { type: 'error', data: said.decode(o.body), lastEventId: '', coalesced: 0, kind: 'Response', message: `HTTP ${o.status}`, status: o.status });
    });
}
