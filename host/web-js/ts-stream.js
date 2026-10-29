// A TypeScript resource's stream on the JS target (LLP 1016.000 D7): the
// shared web reader owns admission and lifetime; the mapper produces a
// snapshot for every protocol frame. Loaded only for a streaming source.
import { streamed } from './http-body.js';
import { answering, SOCKET_AND_FETCH } from './ts-fetch.js';
import { rt } from './rt-link.js'; // (rt.js's own, as a lazy chunk takes them: rt-link.js)
const { clock, journal } = rt;
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
  // A BufferSource body goes as its bytes, copied now, as Hermes sends one (an upload's); anything else as its text.
  const body = init.body, raw = body == null ? undefined : body instanceof ArrayBuffer ? new Uint8Array(body.slice(0))
    : ArrayBuffer.isView(body) ? new Uint8Array(body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength)) : new TextEncoder().encode(String(body));
  const request = typeof Request === 'function' && input instanceof Request ? input : null;
  const method = methodOf(input, init), url = request ? request.url : String(input);
  // (a Request's own body comes along when `init` names none: a long poll has none)
  if (init.exactLongPoll !== undefined && request?.body != null && init.body == null)
    throw new TypeError('exactLongPoll: a long poll is a GET with no body');
  const upload = uploadOf(init, method, url, redirect), timeoutMs = upload ? init.exactTimeout : undefined;
  if (timeoutMs !== undefined && (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 3600000))
    throw new TypeError('exactTimeout must be an integer number of milliseconds from 1 to 3600000');
  return { method, url, headers: [...new Headers(init.headers ?? request?.headers ?? [])], raw, bodyFrom, redirect, maxResponseBytes: independent ?? 1048576, longPoll: longPollOf(init.exactLongPoll, method, url), upload, timeoutMs };
}
// `exactUploadProgress` (LLP 1016.000 D10), as js/src/binary.js `uploadOf` checks it.
function uploadOf(init, method, url, redirect) {
  const upload = init.exactUploadProgress;
  if (upload === undefined || upload === false) return undefined;
  if (upload !== true) throw new TypeError('exactUploadProgress is true or false');
  if (/^wss?:/i.test(url)) throw new TypeError('exactUploadProgress: a WebSocket sends no body');
  if (/^[a-z][a-z0-9+.-]*:/i.test(url) && !/^https?:/i.test(url)) throw new TypeError('exactUploadProgress: an upload is an http: or https: request');
  if (init.exactLongPoll !== undefined) throw new TypeError('exactUploadProgress: a long poll sends no body');
  if (method === 'GET' || method === 'HEAD') throw new TypeError('exactUploadProgress: a GET or HEAD request sends no body');
  if (redirect !== undefined && redirect !== 'follow') throw new TypeError('exactUploadProgress: an upload follows its redirects');
  return true;
}
// `exactLongPoll` (LLP 1016.000 D8), as js/src/prelude.js `longPollOf` checks it.
function longPollOf(poll, method, url) {
  if (poll === undefined) return undefined;
  if (/^wss?:/i.test(url)) throw new TypeError('exactLongPoll: a long poll is HTTP, not a WebSocket');
  if (method !== 'GET') throw new TypeError('exactLongPoll: a long poll asks with GET');
  if (poll === null || typeof poll !== 'object') throw new TypeError('exactLongPoll is { cursor, from }');
  // (characters as the runner counts them, code points; `from` a member, or two joined by one `.`)
  const name = k => typeof poll[k] === 'string' && [...poll[k]].length > 0 && [...poll[k]].length <= 256;
  if (!name('cursor')) throw new TypeError('exactLongPoll.cursor is the query parameter the cursor goes in, a name');
  if (!name('from') || poll.from.split('.').length > 2 || poll.from.split('.').some(p => !p))
    throw new TypeError("exactLongPoll.from is the reply's field the cursor comes from, a name, or two joined by one `.`");
  if (poll.sameCursor !== undefined && poll.sameCursor !== 'heartbeat' && poll.sameCursor !== 'event')
    throw new TypeError('exactLongPoll.sameCursor is "heartbeat" (the default) or "event"');
  return { cursor: poll.cursor, from: poll.from, sameCursorEvent: poll.sameCursor === 'event' };
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
  controller.upload = !!req.upload; // in flight until its reply, for every clock (rt.js)
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
  // A long poll's reply that is no event (`exact:reply`, LLP 1016.000 D8) reaches no mapper, nor an upload's start
  // (`exact:sending`), which, as any bytes sent, says it reached the network (rt.js); its bytes sent are a `progress`
  // event, as XMLHttpRequest's.
  const event = m => req.upload && m.event === 'exact:progress' ? { type: 'progress', ...JSON.parse(m.data), lengthComputable: true, data: '', lastEventId: '', coalesced: m.coalesced } : { type: m.event || 'message', data: m.data, lastEventId: m.id, coalesced: m.coalesced };
  const message = m => (controller.sent ||= !!req.upload, req.longPoll && m.event === 'exact:reply' || req.upload && m.event === 'exact:sending')
    ? undefined : deliver({ ...value(event(m), true), coalesced: m.coalesced });
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
      : req.upload ? { type: 'load', status: o.status, data: said.decode(o.body), lastEventId: '', coalesced: 0 }
      : { type: 'error', data: said.decode(o.body), lastEventId: '', coalesced: 0, kind: 'Response', message: `HTTP ${o.status}`, status: o.status });
    });
}
