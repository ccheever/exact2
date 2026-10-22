// A TypeScript resource's stream on the JS target (LLP 1016.000 D7): the
// shared web reader owns admission and lifetime; the mapper produces a
// snapshot for every protocol frame. Loaded only for a streaming source.
import { streamed } from './http-body.js';
import { answering, SOCKET_AND_FETCH } from './ts-fetch.js';
import { clock, journal } from './rt.js';
import { failureCode, failureReason } from './shape.js';
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
// A turn's operations, sent once its commit stands. The whole batch is
// checked again before any frame: an earlier batch in the same commit may
// have filled the browser's buffer or closed the socket.
const publish = operations => () => {
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
};
// The module never sees the browser socket. Operations belong to the current
// mutation's answer, to the socket's own mapper while it answers `open` or a
// `message` (`answering.mapper`), or to a resource's answer while its source
// runs, the one turn of it this page can tie to it (LLP 1016.000 D7, the
// amendment of 2026-10-18). rt.js publishes them only after the commit that
// settles that turn, or that ran that source, stands.
function handle(transport, owner) {
  const encoder = new TextEncoder();
  answering.sockets = true;
  const turn = () => answering.mapper?.owner === owner ? answering.mapper : answering.call;
  const stage = (op, data, reason, size = 0) => {
    const call = turn(), mapper = !!call && call === answering.mapper, answer = !!call && !mapper && !call.mutation;
    if (!mapper && !call?.mutation && !answer) throw new Error(`socket.${op} is only available in a mutation's or a resource's synchronous source, or its socket's own mapper`);
    if (owner.signal.aborted || owner.active?.() === false || answer && !owner.resource) throw new DOMException('the socket is not open', 'InvalidStateError');
    if (!mapper && !answer && call.fetched) { call.socketConflict = true; throw new Error(SOCKET_AND_FETCH); }
    // Overload fails a mapper's snapshot, caught or not.
    const over = e => { if (mapper && e.name === 'QuotaExceededError') call.overload = e; return e; };
    const kind = mapper ? 'mapper call' : answer ? 'answer' : 'mutation';
    const operations = call.socketOps ??= [];
    if (operations.length >= 256) throw over(new DOMException(`socket ${kind} exceeds 256 operations`, 'QuotaExceededError'));
    if (operations.some(x => x.transport === transport && x.op === 'close')) throw new DOMException('the socket is not open', 'InvalidStateError');
    // The turn's payload is bounded as a whole; each socket's buffer with
    // what the turn stages on it.
    if (operations.reduce((n, x) => n + x.size, 0) + size > 1048576) throw over(new DOMException(`socket ${kind} exceeds 1 MiB`, 'QuotaExceededError'));
    try { transport.check(operations.reduce((n, x) => x.transport === transport ? n + x.size : n, 0) + size); } catch (e) { throw over(e); }
    if (!mapper && !operations.length) (call.effects ??= []).push(publish(operations));
    operations.push({ transport, owner, op, data, reason, size });
  };
  return Object.freeze({
    get readyState() { return owner.signal.aborted ? 3 : transport.readyState; },
    // The browser's count (WHATWG: as of the task's start, plus its sends)
    // and what this turn has staged on the socket.
    get bufferedAmount() {
      const staged = turn()?.socketOps ?? [];
      return transport.bufferedAmount + staged.reduce((n, x) => x.transport === transport ? n + x.size : n, 0);
    },
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
// LLP 1133.003 §9's gate, under the agent only: agent.js installs
// `exact.streamGate`, which times each message from its receipt
// (http-body.js `received`) through the mapper to the commit, and the stub
// decode the mapper calls. A page with no agent has none of it.
const gate = () => globalThis.exact?.streamGate;
export function open({ input, init, redirect }, conv, grantSet, deliver, controller) {
  let req;
  try { req = request(input, init, redirect); } catch (e) { return Promise.resolve({ error: String(e?.message ?? e), code: failureCode(e), reason: failureReason(e) }); }
  // The caller's signal, as Fetch picks it (`init.signal` over the Request's),
  // ends the stream too, before a body is read from a file or after.
  const own = init.signal !== undefined ? init.signal : typeof Request === 'function' && input instanceof Request ? input.signal : null;
  if (own?.aborted) controller.abort(own.reason);
  else own?.addEventListener?.('abort', () => controller.abort(own.reason), { once: true });
  const map = init.exactStream;
  let socket;
  // An HTTP body that is not events is a byte stream (http-body.js `readBytes`).
  const protocol = /^wss?:/i.test(req.url) ? { socket: transport => { socket = handle(transport, controller); controller.socket = transport; } } : { bytes: true };
  // A mapper call's replies are refused with the frame: the journal says so.
  const refused = why => journal.push(`t=${clock.now} mapper ${controller.streamName} failed: ${why}; socket frames were refused`);
  // `frame`: the socket's `open` or a `message`, which its mapper may answer
  // with replies on it (D7). They ride the frame's answer as effects
  // (rt.js `reply`), sent once its commit stands.
  const value = (event, frame) => {
    if (controller.active?.() === false) return { error: 'the stream was let go', code: 'error' };
    const turn = socket && frame ? answering.mapper = { owner: controller, socketOps: null, overload: null } : null;
    try {
      const v = map(event, socket);
      if (v && typeof v.then === 'function') throw new Error('exactStream answers each event now; it cannot await');
      if (turn?.overload) throw turn.overload;
      // A socket no resource owns (a mutation's stream) takes none, as natively.
      if (turn?.socketOps?.length && !controller.resource) throw new DOMException('the socket is not open', 'InvalidStateError');
      const ops = turn?.socketOps;
      return ops?.length ? { v: conv(v), effects: [Object.assign(() => { try { publish(ops)(); } catch (e) { refused(e?.message ?? e); } }, { refused })] } : { v: conv(v) };
    } catch (e) {
      if (turn?.socketOps?.length) refused(e?.message ?? e);
      return { error: String(e?.message ?? e), code: failureCode(e), reason: failureReason(e) };
    } finally { if (turn) answering.mapper = null; }
  };
  const message = m => deliver({ ...value({ type: m.event || 'message', data: m.data, lastEventId: m.id, coalesced: m.coalesced }, true), coalesced: m.coalesced });
  const timed = m => {
    const g = gate();
    if (!g) return message(m);
    g.enter(controller.streamName, m.received);
    try { message(m); } finally { g.leave(); }
  };
  return streamed(req, grantSet, timed, controller, readBodyFile, protocol)
    .then(o => {
      gate()?.ended(controller.streamName);
      if (o.close) journal.push(`t=${clock.now} socket close: ${o.close.code} ${o.close.reason} (${controller.streamName})`);
      return value(o.close ? { type: 'close', data: '', lastEventId: '', coalesced: 0, ...o.close } : o.kind
      ? { type: 'error', data: '', lastEventId: '', coalesced: 0, kind: KINDS[o.kind], message: said.decode(o.body), status: 0 }
      : { type: 'error', data: said.decode(o.body), lastEventId: '', coalesced: 0, kind: 'Response', message: `HTTP ${o.status}`, status: o.status });
    });
}
