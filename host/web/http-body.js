// Native ordered HTTP defaults to 64 MiB; independent requests may lower it.
export async function boundedHttpBody(response, limit) {
  if (limit == null) limit = 64 * 1024 * 1024;
  if (!Number.isInteger(limit) || limit < 1 || limit > 64 * 1024 * 1024) throw Error("invalid HTTP response limit");
  if (!response.body) return new Uint8Array();
  const reader=response.body.getReader(), chunks=[]; let size=0;
  try { for (;;) { const {done,value}=await reader.read(); if(done) break; size+=value.length; if(size>limit) throw Error("HTTP response exceeds limit"); if(value.length) chunks.push(value); } }
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

// A WebSocket that only listens (LLP 1016.000 D3; LLP 1069.004 slice 3),
// owned here, never by the module realm. Each text message is a message;
// messages that arrive before the last was handed over coalesce to the
// newest (D4). A binary message, or one over the ceiling, ends it refused;
// the far side's close ends it as natively (`the socket closed (1000)`); an
// abort (the runner let the ticket go) closes it. Nothing is ever sent.
export function readSocket(url, limit, message, controller) {
  const encoder = new TextEncoder();
  const failed = (kind, text) => ({ kind, status: 0, headers: '', body: encoder.encode(text) });
  return new Promise((resolve) => {
    let socket, opened = false, ended = false, newest = null, coalesced = 0, timer = null;
    const flush = () => {
      clearTimeout(timer); timer = null;
      if (newest === null || ended) return;
      const m = { event: '', id: '', data: newest, coalesced };
      newest = null; coalesced = 0; message(m);
    };
    const end = (outcome) => {
      if (ended) return;
      ended = true; clearTimeout(timer);
      controller.signal.removeEventListener('abort', abort);
      try { socket?.close(1000); } catch {}
      resolve(outcome);
    };
    const abort = () => end(failed(4, 'request aborted'));
    if (controller.signal.aborted) return abort();
    try { socket = new WebSocket(url); } catch (error) { return end(failed(1, String(error?.message ?? error))); }
    socket.binaryType = 'arraybuffer';
    controller.signal.addEventListener('abort', abort);
    socket.onopen = () => { opened = true; };
    socket.onmessage = (e) => {
      if (ended) return;
      if (typeof e.data !== 'string') return end(failed(2, "a binary message: a socket's messages are text"));
      if (e.data.length > limit || encoder.encode(e.data).length > limit) return end(failed(2, 'a message exceeds the response ceiling'));
      if (newest !== null) coalesced++;
      newest = e.data;
      timer ??= setTimeout(flush, 0);
    };
    socket.onclose = (e) => {
      flush();
      end(failed(1, !opened ? 'the socket did not open' : e.reason ? `the socket closed (${e.code}: ${e.reason})` : `the socket closed (${e.code})`));
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

import { admitsNetwork, grantError, scopedGrantSet } from './grant-admission.js';

// Network and page-module requests share admission and the byte ceiling.
// Called after the enclosing batch, so even an immediate refusal cannot re-enter it.
export async function request(op, { grantSet, loadPageNative, moduleLoader, localAssetURL, controllers, controller = new AbortController(), active = () => true, message = () => {} }) {
  const encoder = new TextEncoder();
  const failed = (kind, message) => ({ kind, status: 0, headers: '', body: encoder.encode(String(message?.message ?? message)) });
  const { method, url, headers, body, cache } = op;
  if (!active()) return failed(4, 'request source unloaded');
  const native = url === 'exact-native:';
  const effective = scopedGrantSet(grantSet, op.scope), invalid = grantError(effective);
  const asset = method === 'GET' && !body && Object.keys(headers).length === 0 && /^\/assets\/(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9_-]+\.[A-Za-z0-9]+$/.test(url);
  const socket = op.stream && /^wss?:/i.test(url), capability = socket ? 'net.websocket' : 'net.fetch';
  if (invalid) return failed(2, invalid);
  if (!native && !asset) try { new URL(url); } catch (error) { return failed(1, error); }
  if (!native && !asset && !admitsNetwork(effective, url, socket ? 'websocket' : 'fetch')) return failed(2, `outside the app's grants (${capability})`);
  if (op.nativeHttp === 'independent' && (!Number.isInteger(op.maxResponseBytes) || op.maxResponseBytes < 1 || op.maxResponseBytes > 64 * 1024 * 1024)) return failed(2, 'invalid independent HTTP response limit');
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
  // A socket is a stream whose URL is `ws:` or `wss:` (its grant was
  // `net.websocket`, above); its method, headers and body are not sent.
  if (socket) return readSocket(url, op.maxResponseBytes ?? 1024 * 1024, message, controller);
  let decodedBody;
  try { if (body) decodedBody = Uint8Array.from(atob(body), c => c.charCodeAt(0)); }
  catch (error) { return failed(4, `invalid request body: ${error}`); }
  controllers.add(controller);
  const init = { method, headers, redirect: 'follow', cache: cache === 'reload' ? 'reload' : 'default', signal: controller.signal };
  if (decodedBody) init.body = decodedBody;
  if (op.stream && !headers.some(([k]) => k.toLowerCase() === 'accept')) init.headers = [...headers, ['accept', 'text/event-stream']];
  try {
    const response = await (!asset && moduleLoader?.claim?.(url, init) || fetch(asset ? localAssetURL(url) : url, init));
    if (response.url && (asset
      ? new URL(response.url).origin !== location.origin
      : !admitsNetwork(effective, response.url, 'fetch'))) return failed(2, 'outside the app\'s grants (net.fetch)');
    // A stream reads its body as events; anything else is its one answer.
    if (op.stream && response.ok && response.body && /^text\/event-stream\s*(;|$)/i.test(response.headers.get('content-type') ?? ''))
      return await readEvents(response, op.maxResponseBytes ?? 1024 * 1024, message, controller);
    return { kind: 0, status: response.status, headers: [...response.headers].map(([k, v]) => `${k}: ${v}`).join('\n'), body: await boundedHttpBody(response, op.maxResponseBytes) };
  } catch (error) { return failed(controller.signal.aborted ? 4 : 1, error); }
  finally { controllers.delete(controller); }
}

// A stream on the JS target (host/web-js: rust-data.js, ts-data.js): `request`
// over a source's request, as the wasm host runs it, so a stream is admitted,
// read, coalesced and ended the same on both web targets (LLP 1016.000).
export function streamed(req, grantSet, message, controller) {
  let body = '';
  for (const b of req.raw ?? []) body += String.fromCharCode(b);
  return request({ method: req.method ?? 'GET', url: req.url, headers: req.headers ?? [], body: body && btoa(body), stream: true, maxResponseBytes: req.maxResponseBytes, scope: req.scope },
    { grantSet, controllers: new Set(), controller, message, localAssetURL: url => new URL(url, location.href).href,
      loadPageNative: () => Promise.reject(new Error('a stream is not a page-module request')) });
}

if (globalThis.exact) globalThis.exact.httpHelpers = { boundedHttpBody, waitForInflight, request };
