// Native ordered HTTP defaults to 64 MiB; independent requests may lower it.
// A body over it is the host refusing the response (kind `Refused`, `failure(x)`'s
// `refused`, LLP 1109 D3), on every host and executor, as a stream's event over its ceiling is.
export async function boundedHttpBody(response, limit) {
  if (limit == null) limit = 64 * 1024 * 1024;
  if (!Number.isInteger(limit) || limit < 1 || limit > 64 * 1024 * 1024) throw Error("invalid HTTP response limit");
  if (!response.body) return new Uint8Array();
  const reader=response.body.getReader(), chunks=[]; let size=0;
  try { for (;;) { const {done,value}=await reader.read(); if(done) break; size+=value.length; if(size>limit) throw Object.assign(Error("HTTP response exceeds limit"), {kind:'Refused'}); if(value.length) chunks.push(value); } }
  catch(error) { await reader.cancel().catch(()=>{}); throw error; } finally { reader.releaseLock(); }
  const bytes=new Uint8Array(size); let at=0; for(const chunk of chunks) { bytes.set(chunk,at); at+=chunk.length; } return bytes;
}

// A `text/event-stream` parser (HTML's "Parsing an event stream"), the same
// rules as the native executor's: lines end at CR, LF or CRLF even across
// chunks; `data` lines join with LF; `id` persists as the cursor; comments
// and `retry` are dropped (the host never reconnects, LLP 1069.004). One
// event or line over `limit` bytes is refused, never truncated.
export function eventStream(limit) {
  const decoder = new TextDecoder();
  let line = '', afterCr = false, started = false, event = '', data = '', hasData = false, id = '';
  const done = (out) => {
    if (!started) { started = true; if (line.startsWith('\ufeff')) line = line.slice(1); }
    const text = line; line = '';
    if (text === '') {
      const type = event; event = '';
      if (!hasData) { data = ''; return; }
      hasData = false;
      out.push({ event: type, id, data: data.endsWith('\n') ? data.slice(0, -1) : data });
      data = '';
      return;
    }
    if (text.startsWith(':')) return;
    const colon = text.indexOf(':');
    const field = colon < 0 ? text : text.slice(0, colon);
    let value = colon < 0 ? '' : text.slice(colon + 1);
    if (value.startsWith(' ')) value = value.slice(1);
    if (field === 'data') {
      if (data.length + value.length + 1 > limit) throw Error('an event exceeds the response ceiling');
      data += value + '\n'; hasData = true;
    } else if (field === 'event') event = value;
    else if (field === 'id' && !value.includes('\0')) id = value;
  };
  return (bytes, end = false) => {
    const text = decoder.decode(bytes, { stream: !end }), out = [];
    for (const c of text) {
      if (afterCr) { afterCr = false; if (c === '\n') continue; }
      if (c === '\r' || c === '\n') { afterCr = c === '\r'; done(out); }
      else { if (line.length >= limit + 64) throw Error('an event exceeds the response ceiling'); line += c; }
    }
    return out;
  };
}

// Read an event stream to its end (LLP 1016.000): each read's events are
// delivered as one message, the newest, carrying how many it replaced — the
// runner has taken nothing in between, so display data coalesces (D4) and
// a log sees the gap. What ends the body is the stream's last outcome.
async function readEvents(response, limit, message, controller) {
  const parse = eventStream(limit), reader = response.body.getReader(), encoder = new TextEncoder();
  const failed = (kind, text) => ({ kind, status: 0, headers: '', body: encoder.encode(text) });
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) return failed(1, 'the event stream ended');
      const events = parse(value);
      if (events.length) message({ ...events[events.length - 1], coalesced: events.length - 1 });
    }
  } catch (error) {
    await reader.cancel().catch(() => {});
    if (controller.signal.aborted) return failed(4, 'request aborted');
    return failed(/ceiling/.test(error?.message) ? 2 : 1, String(error?.message ?? error));
  } finally { reader.releaseLock?.(); }
}

// Read a byte stream to its end (LLP 1016.000, the amendment of
// 2026-10-23): a 2xx body that is not `text/event-stream`, for a TypeScript
// stream. Reads join until the page's next task delivers them as one
// message, an ArrayBuffer, so the mapper's turns follow its pace and no byte
// is dropped or reordered. What waits is bounded by the ceiling, at most
// 1 MiB: a read that would pass it delivers what waits
// and ends the stream refused. `received` is the oldest read's time, as a socket
// frame's is its arrival's, for the development page's stream timing
// (ts-stream.js).
async function readBytes(response, limit, message, controller) {
  const reader = response.body.getReader(), encoder = new TextEncoder(), bound = Math.min(limit, 1048576);
  const failed = (kind, text) => ({ kind, status: 0, headers: '', body: encoder.encode(text) });
  let parts = [], size = 0, received = 0, timer = null;
  const flush = () => {
    clearTimeout(timer); timer = null;
    if (!size) return;
    const data = new Uint8Array(size);
    let at = 0;
    for (const p of parts) { data.set(p, at); at += p.length; }
    const m = { event: '', id: '', data: data.buffer, coalesced: 0, received };
    parts = []; size = 0;
    message(m);
  };
  try {
    for (;;) {
      let { done, value } = await reader.read();
      if (done) { flush(); return failed(1, 'the byte stream ended'); }
      if (!value.length) continue;
      // A read longer than the bound alone, which no native read is, goes
      // to the mapper now in pieces of the bound, after what waits; the rest
      // waits as a read of its own.
      if (value.length > bound) {
        const now = performance.now();
        flush();
        for (; value.length > bound; value = value.subarray(bound)) {
          parts = [value.subarray(0, bound)]; size = bound; received = now;
          flush();
        }
      }
      if (size + value.length > bound) {
        flush();
        return failed(2, bound === 1048576 ? 'byte stream receive queue exceeds 1 MiB' : 'byte stream receive queue exceeds the response ceiling');
      }
      if (!size) received = performance.now();
      parts.push(value); size += value.length;
      timer ??= setTimeout(flush, 0);
    }
  } catch (error) {
    clearTimeout(timer);
    await reader.cancel().catch(() => {});
    if (controller.signal.aborted) return failed(4, 'request aborted');
    return failed(1, String(error?.message ?? error));
  } finally { reader.releaseLock?.(); }
}

// An upload by XMLHttpRequest (LLP 1016.000 D10):
// each `upload` progress event is a message of the bytes sent, and the reply,
// any status, read whole within `limit`, is the outcome, as `request` gives
// one. A reply whose last hop the grants do not admit is refused.
function upload(url, method, headers, body, limit, message, signal, admits) {
  const encoder = new TextEncoder(), failed = (kind, text) => ({ kind, status: 0, headers: '', body: encoder.encode(String(text)) });
  return new Promise(resolve => {
    const xhr = new globalThis.XMLHttpRequest(), abort = () => xhr.abort();
    const end = o => { signal.removeEventListener('abort', abort); xhr.onabort = xhr.onerror = xhr.onload = xhr.onprogress = xhr.upload.onprogress = null; resolve(o); };
    try {
      xhr.open(method, url);
      xhr.responseType = 'arraybuffer';
      for (const [name, value] of headers) xhr.setRequestHeader(name, value);
    } catch (error) { return resolve(failed(2, error?.message ?? error)); }
    xhr.upload.onprogress = e => { if (e.lengthComputable) message({ event: 'exact:progress', id: '', data: JSON.stringify({ loaded: e.loaded, total: e.total }), coalesced: 0, received: performance.now() }); };
    xhr.onprogress = e => { if (e.loaded > limit) { end(failed(2, 'HTTP response exceeds limit')); xhr.abort(); } };
    xhr.onload = () => {
      if (xhr.responseURL && !admits(xhr.responseURL)) return end(failed(2, `outside the app's grants (net.fetch): redirected to ${new URL(xhr.responseURL).origin}`));
      const bytes = new Uint8Array(xhr.response ?? new ArrayBuffer(0));
      end(bytes.length > limit ? failed(2, 'HTTP response exceeds limit') : { kind: 0, status: xhr.status, headers: xhr.getAllResponseHeaders().trim().split(/\r?\n/).filter(Boolean).join('\n'), body: bytes });
    };
    xhr.onerror = () => end(failed(1, 'TypeError: Failed to fetch'));
    xhr.onabort = () => end(failed(4, 'request aborted'));
    if (signal.aborted) return end(failed(4, 'request aborted'));
    signal.addEventListener('abort', abort, { once: true });
    xhr.send(body);
  });
}

// A long poll (LLP 1016.000 D8), as the native executor's executor_longpoll.rs:
// ask, deliver each 2xx JSON reply as one message whose `id` is the cursor at
// `from`, and ask again with it in the `cursor` query parameter, one request
// at a time. A 204, a 304, an empty body or (unless `sameCursorEvent`) the
// cursor asked with is a heartbeat. The network, a timeout, a 408, 429, 502,
// 503 or 504 asks again after `Retry-After` (at most 60 s) or a backoff of 1,
// 2, 4, 8, 16, then 30 s within a fifth either way, shown on the controller
// as `retrying` for `state.streams`; anything else ends the stream. A reply
// asks again no sooner than `heartbeatMs` after it was asked (a test's
// shorter), so a server that answers at once is not hammered. Each reply of
// any kind calls `controller.replied` (the page's held count, D9); one that is
// no event is also a message of event `exact:reply`, which the runner counts
// and no mapper sees (`LONG_POLL_REPLY`).
export const longPollTiming = { heartbeatMs: 1000 };
export const LONG_POLL_REPLY = 'exact:reply';
const BUSY = [408, 429, 502, 503, 504];
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
// `Retry-After` in ms: seconds, or an IMF-fixdate (the native parser's one form); none for anything else.
export function retryAfter(value, now = Date.now()) {
  const v = value?.trim();
  if (v == null) return null;
  if (/^\d+$/.test(v)) return Math.min(Number(v) * 1000, 60000);
  const m = /^[A-Z][a-z]{2}, (\d{2}) ([A-Z][a-z]{2}) (\d{4}) (\d{2}):(\d{2}):(\d{2}) GMT$/.exec(v);
  if (!m || !MONTHS.includes(m[2]) || +m[1] < 1 || +m[1] > 31 || +m[4] > 23 || +m[5] > 59 || +m[6] > 60) return null;
  return Math.min(Math.max(Date.UTC(+m[3], MONTHS.indexOf(m[2]), +m[1], +m[4], +m[5], +m[6]) - now, 0), 60000);
}
// Why a long poll is refused before anything is sent (the runner's `long_poll_refusal`), if it is.
export function longPollRefusal(op, method, body) {
  const { cursor, from, sameCursorEvent } = op.longPoll ?? {};
  if (!op.stream) return "exactLongPoll asks again for an answer that keeps coming: it is a stream's";
  const named = v => typeof v === 'string' && [...v].length >= 1 && [...v].length <= 256;
  if (!/^https?:\/\//i.test(op.url)) return 'exactLongPoll: a long poll is HTTP, not a WebSocket';
  if (!/^GET$/i.test(method) || op.bodyFrom != null || body) return 'exactLongPoll: a long poll is a GET with no body';
  if (!named(cursor)) return 'exactLongPoll.cursor is the query parameter the cursor goes in, a name of 1 to 256 characters';
  if (!named(from) || from.split('.').length > 2 || from.split('.').some(p => !p)) return "exactLongPoll.from is the reply's field the cursor comes from: a name, or two joined by one `.`, of 1 to 256 characters";
  if (typeof sameCursorEvent !== 'boolean') return 'exactLongPoll.sameCursor is "heartbeat" or "event"';
  return null;
}
export async function readLongPoll(op, init, limit, message, controller, admits) {
  const encoder = new TextEncoder(), { cursor, from, sameCursorEvent } = op.longPoll;
  const failed = (kind, text) => ({ kind, status: 0, headers: '', body: encoder.encode(text) });
  const aborted = () => failed(4, 'request aborted');
  const replied = () => controller.replied?.();
  const noEvent = () => { replied(); message({ event: LONG_POLL_REPLY, id: '', data: '', coalesced: 0, received: performance.now() }); };
  const pause = (ms) => new Promise(resolve => {
    if (controller.signal.aborted) return resolve();
    const done = () => { clearTimeout(timer); controller.signal.removeEventListener('abort', done); resolve(); };
    const timer = setTimeout(done, ms);
    controller.signal.addEventListener('abort', done, { once: true });
  });
  // The cursor at `from`: a string, or a whole number within 2^53; `undefined` for none, an Error for one no host carries.
  const cursorOf = (text) => {
    let at; try { at = JSON.parse(text); } catch { return undefined; }
    for (const part of from.split('.', 2)) { if (at == null || typeof at !== 'object' || Array.isArray(at) || !Object.hasOwn(at, part)) return undefined; at = at[part]; }
    // (a lone surrogate would not survive the URL or a native host's JSON: no host could carry it)
    if (typeof at === 'string') return encoder.encode(at).length > 4096 || /[\u0000-\u001f\u007f-\u009f]|[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/.test(at) ? Error("a long poll's cursor is at most 4096 bytes, with no control characters") : at;
    if (typeof at === 'number') return Number.isSafeInteger(at) ? String(at) : Error("a long poll's numeric cursor is a whole number within 2^53: send a larger one as a string");
    return undefined;
  };
  let url = new URL(op.url), failures = 0;
  for (;;) {
    if (controller.signal.aborted) return aborted();
    if (!admits(url.href)) { replied(); return failed(2, "outside the app's grants (net.fetch)"); }
    const asked = url.searchParams.get(cursor), askedAt = performance.now();
    const floor = () => Math.max(0, Math.ceil(longPollTiming.heartbeatMs - (performance.now() - askedAt)));
    const deadline = op.timeoutMs === undefined ? null : AbortSignal.timeout(op.timeoutMs);
    const signal = deadline ? AbortSignal.any([controller.signal, deadline]) : controller.signal;
    let wait, error = null;
    try {
      const response = await fetch(url.href, { ...init, signal });
      const drop = () => response.body?.cancel().catch(() => {});
      if (response.url && !admits(response.url)) { drop(); replied(); return failed(2, `outside the app's grants (net.fetch): redirected to ${new URL(response.url).origin}`); }
      const status = response.status, type = response.headers.get('content-type') ?? '';
      // Judged by status and headers first: a reply that asks again, or an event stream, is never read.
      if (status === 204 || status === 304) { drop(); failures = 0; noEvent(); wait = floor(); }
      else if (BUSY.includes(status)) {
        drop(); noEvent();
        const after = retryAfter(response.headers.get('retry-after'));
        if (after != null) failures++;
        else { const base = Math.min(1000 * 2 ** Math.min(failures, 5), 30000); failures++; wait = Math.round(base * (0.8 + 0.4 * Math.random())); }
        wait = Math.max(after ?? wait, floor()); error = `HTTP ${status}`;
      } else if (status >= 200 && status <= 299 && /^text\/event-stream\s*(;|$)/i.test(type)) {
        drop(); replied(); return failed(2, "an event stream under exactLongPoll: a long poll's replies are JSON");
      } else {
        const bytes = await boundedHttpBody(response, limit);
        const headers = [...response.headers].map(([k, v]) => `${k}: ${v}`).join('\n');
        if (status < 200 || status > 299) { replied(); return { kind: 0, status, headers, body: bytes }; }
        failures = 0;
        const text = new TextDecoder().decode(bytes);
        if (/^[ \t\n\r]*$/.test(text)) { noEvent(); wait = floor(); }
        else {
          const next = cursorOf(text);
          if (next instanceof Error) { replied(); return failed(2, next.message); }
          if (next === undefined) { replied(); return { kind: 0, status, headers, body: bytes }; }
          if (!sameCursorEvent && asked === next) { noEvent(); wait = floor(); }
          else {
            if (controller.signal.aborted) return aborted();
            replied();
            message({ event: '', id: next, data: text, coalesced: 0, received: performance.now() });
            url.searchParams.set(cursor, next);
            continue;
          }
        }
      }
    } catch (e) {
      if (controller.signal.aborted) return aborted();
      if (e?.kind === 'Refused') { replied(); return failed(2, String(e.message)); }
      error = deadline?.aborted ? `the request timed out after ${op.timeoutMs} ms` : String(e?.message ?? e) || 'a network error';
      const base = Math.min(1000 * 2 ** Math.min(failures, 5), 30000);
      failures++;
      wait = Math.max(Math.round(base * (0.8 + 0.4 * Math.random())), floor());
    }
    if (error) controller.retrying = { attempt: failures, at: performance.now() + wait, error };
    await pause(wait);
    delete controller.retrying;
  }
}

// The transport belongs to the resource's abort controller (LLP 1016.000).
// `protocol` opts the JS TypeScript adapter into D7: ordered frames and a
// private transport handle; the other adapters still take snapshot delivery.
export function readSocket(url, limit, message, controller, protocol) {
  const encoder = new TextEncoder(), MAX_BYTES = 1048576, MAX_FRAMES = 256;
  const failed = (kind, text) => ({ kind, status: 0, headers: '', body: encoder.encode(text) });
  return new Promise((resolve) => {
    let socket, opened = false, ended = false, newest = null, coalesced = 0, timer = null, queue = [], stamps = [], bytes = 0, sent = 0;
    const flush = () => {
      clearTimeout(timer); timer = null;
      if (ended) return;
      if (protocol) {
        const frames = queue, received = stamps; queue = []; stamps = []; bytes = 0;
        frames.forEach((data, i) => { if (!ended) message({ event: '', id: '', data, coalesced: 0, received: received[i] }); });
      } else if (newest !== null) {
        const m = { event: '', id: '', data: newest, coalesced };
        newest = null; coalesced = 0; message(m);
      }
    };
    const end = (outcome) => {
      if (ended) return;
      ended = true; clearTimeout(timer); queue = []; newest = null; bytes = 0;
      controller.signal.removeEventListener('abort', abort);
      try { socket?.close(1000); } catch {}
      resolve(outcome);
    };
    const abort = () => end(failed(4, 'request aborted'));
    if (controller.signal.aborted) return abort();
    try { socket = new WebSocket(url); } catch (error) { return end(failed(1, String(error?.message ?? error))); }
    socket.binaryType = 'arraybuffer';
    controller.signal.addEventListener('abort', abort);
    const check = size => {
      if (ended || socket.readyState !== 1) throw new DOMException('the socket is not open', 'InvalidStateError');
      if (!protocol.admit()) throw new Error("outside the app's grants (net.websocket)");
      if (socket.bufferedAmount + size > MAX_BYTES) throw new DOMException('socket buffer exceeds 1 MiB', 'QuotaExceededError');
    };
    protocol?.socket(Object.freeze({
      get readyState() { return ended ? 3 : socket.readyState; },
      get bufferedAmount() { return ended ? 0 : socket.bufferedAmount; },
      get sent() { return sent; },
      check,
      send(data) {
        const size = typeof data === 'string' ? encoder.encode(data).length : data.byteLength;
        check(size); socket.send(data); sent++;
      },
      close(code = 1000, reason = '') {
        check(0);
        if (code !== 1000 && (!Number.isInteger(code) || code < 3000 || code > 4999)) throw new DOMException('invalid socket close code', 'InvalidAccessError');
        if (typeof reason !== 'string' || encoder.encode(reason).length > 123) throw new SyntaxError('socket close reason exceeds 123 UTF-8 bytes');
        socket.close(code, reason);
      },
    }));
    socket.onopen = () => {
      if (ended) return;
      opened = true;
      if (protocol) message({ event: 'open', id: '', data: '', coalesced: 0 });
    };
    socket.onmessage = (e) => {
      if (ended) return;
      if (!protocol && typeof e.data !== 'string') return end(failed(2, "a binary message: a socket's messages are text"));
      const size = typeof e.data === 'string' ? encoder.encode(e.data).length : e.data.byteLength;
      if (size > limit) {
        if (protocol) flush();
        return end(failed(2, 'a message exceeds the response ceiling'));
      }
      if (protocol) {
        if (queue.length >= MAX_FRAMES || bytes + size > MAX_BYTES + limit) {
          flush();
          return end(failed(2, 'socket receive queue exceeds 256 frames or 1 MiB'));
        }
        queue.push(e.data); stamps.push(performance.now()); bytes += size;
      } else {
        if (newest !== null) coalesced++;
        newest = e.data;
      }
      timer ??= setTimeout(flush, 0);
    };
    socket.onclose = (e) => {
      flush();
      const outcome = failed(1, !opened ? 'the socket did not open' : e.reason ? `the socket closed (${e.code}: ${e.reason})` : `the socket closed (${e.code})`);
      if (protocol && opened) outcome.close = { code: e.code, reason: e.reason, wasClean: e.wasClean };
      end(outcome);
    };
  });
}

// `waiting()` is the work still counted; a promise it drops (a ticket the
// runner let go of) stops holding the wait at the next commit or settle.
export async function waitForInflight(waiting, deadline) {
  for (let now = waiting(); now.length; now = waiting()) {
    const remaining = deadline - performance.now();
    if (remaining <= 0) return false;
    let timer;
    const completed = await Promise.race([
      Promise.race(now).then(() => true),
      new Promise((resolve) => { timer = setTimeout(() => resolve(false), remaining); }),
    ]);
    clearTimeout(timer);
    if (!completed) return false;
  }
  return true;
}

import { admitsNetwork, admittingGrant, coversPath, grantError, scopedGrantSet } from './grant-admission.js';
import { faultMessage, takeFault } from './faults.js';
import { reachRefusal, redirectFor } from './address-space.js';

// Network and page-module requests share admission and the byte ceiling.
// Called after the enclosing batch, so even an immediate refusal cannot re-enter it.
export async function request(op, { grantSet, loadPageNative, moduleLoader, localAssetURL, controllers, controller = new AbortController(), active = () => true, message = () => {}, bodyFile, bodyTarget, protocol }) {
  const encoder = new TextEncoder();
  const failed = (kind, message) => ({ kind, status: 0, headers: '', body: encoder.encode(String(message?.message ?? message)) });
  const { method, url, headers, body, cache } = op;
  if (!active()) return failed(4, 'request source unloaded');
  // A long poll no host may run (LLP 1016.000 D8) is refused before anything else is tried: a native call, a socket.
  const pollRefusal = op.longPoll && longPollRefusal(op, method, body);
  if (pollRefusal) return failed(2, pollRefusal);
  const native = url === 'exact-native:';
  const effective = scopedGrantSet(grantSet, op.scope), invalid = grantError(effective);
  const asset = method === 'GET' && !body && Object.keys(headers).length === 0 && /^\/assets\/(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9_-]+\.[A-Za-z0-9]+$/.test(url);
  const socket = op.stream && /^wss?:/i.test(url), capability = socket ? 'net.websocket' : 'net.fetch';
  if (invalid) return failed(2, invalid);
  if (!native && !asset) try { new URL(url); } catch (error) { return failed(1, error); }
  const grant = native || asset ? null : admittingGrant(effective, url, socket ? 'websocket' : 'fetch');
  if (!native && !asset && !grant) return failed(2, `outside the app's grants (${capability})`);
  // LLP 1133.036 D6: a tool's, a wake's or the device's reach to a less public address space waits for the person.
  const unreached = native || asset ? null : reachRefusal(url, op.origin, op.reachState);
  if (unreached) return failed(2, `NotAllowedError: ${unreached}`);
  // The line that admitted a stream, for `state.streams`.
  if (op.stream && grant) controller.grant = grant;
  // A body written to a file (`exactBodyTo`) may name up to 1 GiB: it is not held in memory.
  if (op.nativeHttp === 'independent' && (!Number.isInteger(op.maxResponseBytes) || op.maxResponseBytes < 1 || op.maxResponseBytes > (op.bodyTo == null ? 64 : 1024) * 1024 * 1024)) return failed(2, 'invalid independent HTTP response limit');
  if (native) {
    let response;
    try {
      const module = await loadPageNative();
      if (!active()) return failed(4, 'request source unloaded');
      response = new Response(await module.later(body));
    }
    catch (error) { response = new Response(String(error?.message ?? error), { status: 500 }); }
    try { return { kind: 0, status: response.status, headers: '', body: await boundedHttpBody(response, op.maxResponseBytes) }; }
    catch (error) { return failed(2, error); }
  }
  // A long poll asks again and again on its own (D8): an HTTP GET, no body, JSON replies; checked before a socket opens.
  if (op.longPoll) {
    const accept = headers.some(([k]) => k.toLowerCase() === 'accept') ? headers : [...headers, ['accept', 'application/json']];
    controllers.add(controller);
    try {
      return await readLongPoll(op, { method: 'GET', headers: accept, redirect: redirectFor(op.origin, op.redirect ?? 'follow', op.reachState), cache: 'no-store' },
        op.maxResponseBytes ?? 1024 * 1024, message, controller, (u) => active() && admitsNetwork(scopedGrantSet(grantSet, op.scope), u, 'fetch'));
    } finally { controllers.delete(controller); delete controller.retrying; }
  }
  // A socket is a stream whose URL is `ws:` or `wss:` (its grant was
  // `net.websocket`, above); its method, headers and body are not sent.
  if (socket && op.bodyFrom != null) return failed(2, 'exactBodyFrom: a WebSocket sends no body');
  if (socket) return readSocket(url, op.maxResponseBytes ?? 1024 * 1024, message, controller, protocol && { ...protocol, admit: () => active() && admitsNetwork(scopedGrantSet(grantSet, op.scope), url, 'websocket') });
  let decodedBody;
  try { if (body) decodedBody = Uint8Array.from(atob(body), c => c.charCodeAt(0)); }
  catch (error) { return failed(4, `invalid request body: ${error}`); }
  // A deadline for the whole exchange (Request::timeout_ms): kind 10 when it
  // passes (9 is an auth session's delivery, glue.js). A body read from a file
  // counts against it.
  if (op.timeoutMs !== undefined && (!Number.isInteger(op.timeoutMs) || op.timeoutMs < 1 || op.timeoutMs > 3600000)) return failed(2, 'a request timeout must be 1 to 3600000 ms');
  const deadline = op.timeoutMs === undefined ? null : AbortSignal.timeout(op.timeoutMs);
  // The same deadline as an instant, checked by the clock just before the request goes out:
  // a body read or grant work that held the event loop past it fires no timer first.
  const expires = op.timeoutMs === undefined ? Infinity : performance.now() + op.timeoutMs;
  const expired = () => deadline && !controller.signal.aborted && (deadline.aborted || performance.now() >= expires);
  controllers.add(controller);
  const signal = deadline ? AbortSignal.any([controller.signal, deadline]) : controller.signal;
  // `exactBodyFrom` (LLP 1108 D6 R2): the app file, read now under the
  // request's `fs.read` grant into a Blob body (a picked file is its own
  // File, never copied into a string), or refused before anything is sent.
  let fileBody;
  if (op.bodyFrom != null) {
    if (!bodyFile) return failed(3, `exactBodyFrom ${op.bodyFrom}: this host has no app files (no storage here)`);
    if (body) return failed(2, 'fetch: a request has one body: body or exactBodyFrom');
    if (/^(GET|HEAD)$/i.test(method)) return failed(2, 'fetch: a GET or HEAD request cannot have a body');
    if (typeof op.bodyFrom !== 'string' || !op.bodyFrom.startsWith('app:/')) return failed(2, 'exactBodyFrom must be an app:/ path');
    if (new TextEncoder().encode(op.bodyFrom).length > 4096 || /(^|\/)\.\.?(\/|$)|\0/.test(op.bodyFrom.slice(5))) return failed(2, `exactBodyFrom ${op.bodyFrom.slice(0, 256)}: an app:/ path has no . or .. segment, and is at most 4096 bytes`);
    // Refused before the storage adapters load, which an app without file grants does not ship.
    if (!coversPath(effective, 'fs.read', op.bodyFrom))
      return failed(2, `exactBodyFrom ${op.bodyFrom}: denied: fs.read ${op.bodyFrom}: no grant covers it; grant \`fs.read ${op.bodyFrom}\``);
    // The read yields to the deadline and to letting go: nothing is sent once either has ended it.
    if (expired()) return failed(10, `the request timed out after ${op.timeoutMs} ms`);
    if (signal.aborted) return failed(4, 'request aborted');
    const stopped = new Promise(resolve => signal.addEventListener('abort', resolve, { once: true }));
    const reading = bodyFile(op.bodyFrom, effective);
    reading.catch(() => {}); // a read that loses the race is nobody's
    try { fileBody = await Promise.race([reading, stopped]); }
    catch (error) { return failed(error?.code === 'agent' ? 3 : 2, error); }
    if (expired()) return failed(10, `the request timed out after ${op.timeoutMs} ms`);
    if (signal.aborted) return failed(4, 'request aborted');
    if (!active()) return failed(4, 'request source unloaded');
  }
  // `exactBodyTo` (LLP 1108 D6 R2, 2026-10-26): a 2xx body goes to the app
  // file (storage-fs.js `bodyTarget`), admitted under `fs.write` with its
  // parent checked now, or refused before anything is sent.
  let target;
  if (op.bodyTo != null) {
    if (!bodyTarget) return failed(3, `exactBodyTo ${op.bodyTo}: this host has no app files (no storage here)`);
    if (typeof op.bodyTo !== 'string' || !op.bodyTo.startsWith('app:/') || new TextEncoder().encode(op.bodyTo).length > 4096 || /(^|\/)\.\.?(\/|$)|\0/.test(op.bodyTo.slice(5)))
      return failed(2, `exactBodyTo ${String(op.bodyTo).slice(0, 256)}: an app:/ path has no . or .. segment, and is at most 4096 bytes`);
    if (!coversPath(effective, 'fs.write', op.bodyTo))
      return failed(2, `exactBodyTo ${op.bodyTo}: denied: fs.write ${op.bodyTo}: no grant covers it; grant \`fs.write ${op.bodyTo}\``);
    const stopped = new Promise(resolve => signal.addEventListener('abort', () => resolve(), { once: true }));
    const opening = bodyTarget(op.bodyTo, effective);
    opening.catch(() => {}); // an opening that loses the race is nobody's
    try { target = await Promise.race([opening, stopped]); }
    catch (error) { return failed(error?.code === 'agent' ? 3 : 2, error); }
    if (!target) opening.then(t => t.close(), () => {});
    const ended = expired() ? failed(10, `the request timed out after ${op.timeoutMs} ms`) : signal.aborted ? failed(4, 'request aborted') : !active() ? failed(4, 'request source unloaded') : null;
    if (ended) { target?.close(); return ended; }
  }
  // An upload's progress (LLP 1016.000 D10): Fetch tells no bytes sent, so it goes by
  // XMLHttpRequest, which follows every redirect; where a hop might ask the person (D6) it is not sent.
  if (op.stream && op.upload) {
    try {
      if (redirectFor(op.origin, 'follow', op.reachState) !== 'follow') return failed(2, 'NotAllowedError: an upload follows its redirects, and one could reach a less public address space before the person answers');
      const o = await upload(url, method, headers, decodedBody ?? fileBody ?? null, op.maxResponseBytes ?? 1024 * 1024, message, signal, u => admitsNetwork(effective, u, 'fetch'));
      return o.kind === 4 && deadline?.aborted && !controller.signal.aborted ? failed(10, `the request timed out after ${op.timeoutMs} ms`) : o;
    } finally { controllers.delete(controller); }
  }
  // A browser answers a `manual` redirect with an opaque-redirect response
  // (Fetch §2.2.6: status 0, no Location)
  const init = { method, headers, redirect: redirectFor(op.origin, op.redirect ?? 'follow', op.reachState), cache: cache === 'reload' ? 'reload' : 'default', signal };
  if (decodedBody) init.body = decodedBody;
  else if (fileBody) init.body = fileBody;
  if (op.stream && !headers.some(([k]) => k.toLowerCase() === 'accept')) init.headers = [...headers, ['accept', 'text/event-stream']];
  try {
    const early = !asset && moduleLoader?.claim?.(url, init);
    // @ref LLP 1103 D1, D2 — a driver fault is a refused connection, never sent; a GET
    // `fetchEarly` already sent (before the fault was armed) is not one it decides.
    if (!early && !asset && !op.stream && takeFault(url)) return failed(1, faultMessage(url));
    if (!early && expired()) return failed(10, `the request timed out after ${op.timeoutMs} ms`);
    const response = await (early || fetch(asset ? localAssetURL(url) : url, init));
    // A redirect that left the grants names where it led (podcast F5): the
    // browser followed it, and the response's URL is the last hop's.
    if (response.url && (asset
      ? new URL(response.url).origin !== location.origin
      : !admitsNetwork(effective, response.url, 'fetch'))) return failed(2, `outside the app's grants (net.fetch): redirected to ${new URL(response.url).origin}`);
    // A stream reads its body as events, or a TypeScript stream's as bytes;
    // anything else is its one answer.
    if (op.stream && response.ok && response.body && /^text\/event-stream\s*(;|$)/i.test(response.headers.get('content-type') ?? ''))
      return await readEvents(response, op.maxResponseBytes ?? 1024 * 1024, message, controller);
    if (op.stream && protocol?.bytes && response.ok && response.body)
      return await readBytes(response, op.maxResponseBytes ?? 1024 * 1024, message, controller);
    const headerText = [...response.headers].map(([k, v]) => `${k}: ${v}`).join('\n');
    // Written: its size first, as a pseudo-header (host.rs `outcome_from`); any other reply is read as usual, at most 1 MiB.
    if (target && response.ok) return { kind: 0, status: response.status, headers: `:exact-body-to: ${await target.put(response, op.maxResponseBytes ?? 1024 * 1024 * 1024)}\n${headerText}`, body: new Uint8Array() };
    return { kind: 0, status: response.status, headers: headerText, body: await boundedHttpBody(response, target ? Math.min(op.maxResponseBytes ?? 1024 * 1024, 1024 * 1024) : op.maxResponseBytes) };
  } catch (error) {
    // Whichever ended it first: the combined signal keeps the first reason.
    if (deadline && signal.aborted && signal.reason === deadline.reason) return failed(10, `the request timed out after ${op.timeoutMs} ms`);
    // A store that would not take a written body (`Unavailable`) refuses it, as natively.
    return failed(controller.signal.aborted ? 4 : error?.kind === 'Refused' || error?.kind === 'Unavailable' ? 2 : 1, error);
  }
  finally { controllers.delete(controller); target?.close(); }
}

// A stream on the JS target (host/web-js: rust-data.js, ts-data.js): `request`
// over a source's request with the wasm host's admission and ownership.
// Only the TypeScript adapter opts into ordered protocol frames (D7) and
// byte streams.
export function streamed(req, grantSet, message, controller, bodyFile, protocol) {
  let body = '';
  for (const b of req.raw ?? []) body += String.fromCharCode(b);
  return request({ method: req.method ?? 'GET', url: req.url, headers: req.headers ?? [], body: body && btoa(body), bodyFrom: req.bodyFrom, redirect: req.redirect, longPoll: req.longPoll, upload: req.upload, timeoutMs: req.timeoutMs, stream: true, maxResponseBytes: req.maxResponseBytes, scope: req.scope },
    { grantSet, controllers: new Set(), controller, message, bodyFile, protocol, localAssetURL: url => new URL(url, location.href).href,
      loadPageNative: () => Promise.reject(new Error('a stream is not a page-module request')) });
}

if (globalThis.exact) globalThis.exact.httpHelpers = { boundedHttpBody, waitForInflight, request };
