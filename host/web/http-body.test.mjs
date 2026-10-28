import {expect,test} from 'bun:test';
import assert from 'node:assert/strict';
import {mkdtempSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {boundedHttpBody as readBody, longPollTiming, request, waitForInflight} from './http-body.js';
import {admittingGrant, scopedGrantSet} from './grant-admission.js';
import {createRequestExecutor} from '../web-js/admission.js';

const ROOT=resolve(dirname(fileURLToPath(import.meta.url)),'../..'),sets=new Map();
const target=resolve(process.env.CARGO_TARGET_DIR||resolve(ROOT,'target'),`debug/exact-web-js${process.platform==='win32'?'.exe':''}`);
const built=spawnSync('cargo',['build','-q','-p','exact-web-js'],{cwd:ROOT,encoding:'utf8'});
if(built.status!==0)throw new Error(built.stderr||'grant normalizer build failed');
function normalized(spec){
  if(sets.has(spec))return sets.get(spec);
  const scratch=mkdtempSync(resolve(tmpdir(),'exact-grants-')),input=resolve(scratch,'grants.txt');writeFileSync(input,spec);
  const result=spawnSync(target,['normalize-grants',input],{cwd:ROOT,encoding:'utf8'});
  rmSync(scratch,{recursive:true});if(result.status!==0)throw new Error(result.stderr||'grant normalizer failed');
  const set=JSON.parse(result.stdout);sets.set(spec,set);return set;
}

test('independent HTTP collects only a bounded complete response',async()=>{
  assert.deepEqual(await readBody(new Response('four'),4),new TextEncoder().encode('four'));
  assert.deepEqual(await readBody(new Response(null),4),new Uint8Array());
  for(const limit of [0,-1,1.5,64*1024*1024+1]) await assert.rejects(readBody(new Response('data'),limit),/invalid/);
});
test('an over-limit stream is cancelled before its remainder is collected',async()=>{
  let cancelled=false;
  const stream=new ReadableStream({pull(c){c.enqueue(new Uint8Array(5));},cancel(){cancelled=true;}});
  await assert.rejects(readBody(new Response(stream),4),/exceeds limit/);
  assert.equal(cancelled,true);
});

test('settlement returns after completions and refuses a never-settling request at its deadline', async () => {
  const pending = new Set(), all = () => [...pending];
  const complete = Promise.resolve().then(() => pending.delete(complete));
  pending.add(complete);
  assert.equal(await waitForInflight(all, performance.now() + 1000), true);
  pending.add(new Promise(() => {}));
  assert.equal(await waitForInflight(all, performance.now()), false);
});

test('work the runner let go of stops holding the wait (LLP 1016 D5)', async () => {
  // A hung read, and an answer whose commit forgets it: the hung one is still in flight, no longer counted.
  const inflight = new Set(), counted = new Set(), hung = new Promise(() => {});
  const answer = Promise.resolve().then(() => { counted.delete(hung); inflight.delete(answer); });
  for (const p of [hung, answer]) { inflight.add(p); counted.add(p); }
  assert.equal(await waitForInflight(() => [...inflight].filter(p => counted.has(p)), performance.now() + 1000), true);
  assert.equal(inflight.has(hung), true);
});

test('ordered HTTP defaults to the native ceiling and cancels oversized bodies', async () => {
  assert.deepEqual(await readBody(new Response('four')), new TextEncoder().encode('four'));
  let cancelled = false, pulls = 0;
  const chunk = new Uint8Array(1024 * 1024);
  const stream = new ReadableStream({
    pull(controller) { pulls++; controller.enqueue(chunk); },
    cancel() { cancelled = true; },
  }, { highWaterMark: 0 });
  await assert.rejects(readBody(new Response(stream)), /exceeds limit/);
  assert.equal(cancelled, true);
  assert.equal(pulls, 65);
});

test('the event-stream parser reads lines, fields and cursors as HTML does (LLP 1016.000)', async () => {
  const { eventStream } = await import('./http-body.js');
  const parse = eventStream(1024), enc = (s) => new TextEncoder().encode(s);
  const got = [
    ...parse(enc('﻿: a comment\r')),
    ...parse(enc('\nretry: 10\nevent: progress\nid: 7\ndata: {"a":\r\n')),
    ...parse(enc('data: 1}\r\r')),
    ...parse(enc('data\n\nid\ndata: x\n\n')),
    ...parse(enc('data: no end yet')),
  ];
  assert.deepEqual(got, [
    { event: 'progress', id: '7', data: '{"a":\n1}' },
    { event: '', id: '7', data: '' },
    { event: '', id: '', data: 'x' },
  ]);
  assert.throws(() => eventStream(4)(enc('data: 12345\n')), /ceiling/);
  // A multi-byte character split across reads is one character.
  const split = eventStream(64), bytes = enc('data: é\n\n');
  assert.deepEqual([...split(bytes.slice(0, 7)), ...split(bytes.slice(7))], [{ event: '', id: '', data: 'é' }]);
});

test('a stream delivers each read as its newest event, then its end', async () => {
  const { request } = await import('./http-body.js');
  let push;
  const body = new ReadableStream({ start(c) { push = c; } });
  const sent = [];
  const op = { method: 'GET', url: 'https://example.test/events', headers: [], body: '', stream: true, maxResponseBytes: 64 };
  const done = request(op, {
    grantSet: normalized('net.fetch https://example.test'), controllers: new Set(),
    moduleLoader: { claim: (url, init) => { sent.push(init.headers); return Promise.resolve(new Response(body, { headers: { 'content-type': 'text/event-stream' } })); } },
    message: (m) => sent.push(m),
  });
  const enc = (s) => new TextEncoder().encode(s);
  const tick = () => new Promise((r) => setTimeout(r, 5));
  await tick();
  push.enqueue(enc('id: 1\ndata: a\n\n'));
  await tick();
  push.enqueue(enc('id: 2\ndata: b\n\nid: 3\ndata: c\n\nid: 4\ndata: d\n\n'));
  await tick();
  push.close();
  const end = await done;
  assert.deepEqual(sent, [
    [['accept', 'text/event-stream']],
    { event: '', id: '1', data: 'a', coalesced: 0 },
    { event: '', id: '4', data: 'd', coalesced: 2 },
  ]);
  assert.equal(end.kind, 1);
  assert.equal(new TextDecoder().decode(end.body), 'the event stream ended');
});

// LLP 1016.000's byte stream: reads that land before the page's next task
// join into one ArrayBuffer; nothing is dropped; the end follows; a read past
// the bound delivers what waits and refuses. Without `bytes` (a Rust
// source) the body is the one answer.
test('a TypeScript stream reads a body that is not events as joined bytes, then its end', async () => {
  const { request } = await import('./http-body.js');
  const open = (maxResponseBytes, protocol, sent) => {
    let push;
    const body = new ReadableStream({ start(c) { push = c; } });
    const done = request({ method: 'GET', url: 'https://example.test/screen.avcc', headers: [], body: '', stream: true, maxResponseBytes }, {
      grantSet: normalized('net.fetch https://example.test'), controllers: new Set(), protocol,
      moduleLoader: { claim: () => Promise.resolve(new Response(body, { headers: { 'content-type': 'application/octet-stream' } })) },
      message: (m) => sent.push([...new Uint8Array(m.data)]),
    });
    return { done, push: (bytes) => push.enqueue(new Uint8Array(bytes)), close: () => push.close() };
  };
  const tick = () => new Promise((r) => setTimeout(r, 5));
  const sent = [], s = open(64, { bytes: true }, sent);
  await tick();
  s.push([0, 1]); s.push([2]); s.push([3, 4, 5]);
  await tick();
  s.push([6, 7]);
  await tick();
  s.close();
  const end = await s.done;
  assert.deepEqual(sent, [[0, 1, 2, 3, 4, 5], [6, 7]]);
  assert.equal(new TextDecoder().decode(end.body), 'the byte stream ended');

  const over = [], o = open(8, { bytes: true }, over);
  await tick();
  o.push([1, 1, 1, 1, 1]); o.push([2, 2, 2, 2, 2]);
  const refused = await o.done;
  assert.deepEqual(over, [[1, 1, 1, 1, 1]]);
  assert.equal(refused.kind, 2);
  assert.equal(new TextDecoder().decode(refused.body), 'byte stream receive queue exceeds the response ceiling');

  // One browser read longer than the bound goes in pieces of it, at once.
  const big = [], b = open(8, { bytes: true }, big);
  await tick();
  b.push([0]); b.push([...Array(20).keys()].map(n => n + 1));
  await tick();
  b.close();
  const after = await b.done;
  assert.deepEqual(big, [[0], [1, 2, 3, 4, 5, 6, 7, 8], [9, 10, 11, 12, 13, 14, 15, 16], [17, 18, 19, 20]]);
  assert.equal(new TextDecoder().decode(after.body), 'the byte stream ended');

  const whole = [], w = open(64, undefined, whole);
  await tick();
  w.push([9]); w.close();
  const answer = await w.done;
  assert.deepEqual([whole, answer.status, [...answer.body]], [[], 200, [9]]);
});

// A local WebSocket peer (LLP 1069.004 slice 3): `/three` sends `a`, then at
// each `next('/three')` `b c d` in one write, then a close; `/binary` a binary
// frame; `/hold` one message, then waits for the client to go.
async function socketPeer() {
  const { createServer } = await import('node:http');
  const { createHash } = await import('node:crypto');
  const frame = (op, payload) => Buffer.concat([Buffer.from([0x80 | op, payload.length]), payload]);
  const gone = [], closing = new Map(), steps = new Map();
  const server = createServer((_, res) => res.end());
  server.on('upgrade', (req, socket) => {
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    socket.on('close', () => { gone.push(req.url); closing.get(req.url)?.(); });
    socket.on('error', () => {});
    // The client's close: answer it and hang up (a server's half of the handshake).
    socket.on('data', (d) => { if ((d[0] & 0x0f) === 8) socket.end(frame(8, Buffer.from([0x03, 0xe8]))); });
    if (req.url === '/binary') return socket.write(frame(2, Buffer.from([1, 2, 3])));
    if (req.url === '/hold') return socket.write(frame(1, Buffer.from('held')));
    socket.write(frame(1, Buffer.from('a')));
    steps.set(req.url, [() => socket.write(Buffer.concat(['b', 'c', 'd'].map((s) => frame(1, Buffer.from(s))))), () => socket.write(frame(8, Buffer.from([0x03, 0xe8])))]);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  // `closed(path)`: once the socket of the request for `path` has closed
  const closed = path => gone.includes(path) ? Promise.resolve() : new Promise((ok) => closing.set(path, ok));
  return { url: `ws://127.0.0.1:${server.address().port}`, closed, next: path => steps.get(path).shift()(), close: () => server.close() };
}

test('a socket delivers text messages, coalescing a burst, then its close (LLP 1069.004)', async () => {
  const { request } = await import('./http-body.js');
  const peer = await socketPeer();
  const open = (path, sent, controller = new AbortController()) => request(
    { method: 'GET', url: `${peer.url}${path}`, headers: [], body: '', stream: true, maxResponseBytes: 64 },
    { grantSet: normalized(`net.websocket ${peer.url}`), controllers: new Set(), controller, message: (m) => sent.push(m) },
  );
  const text = (r) => new TextDecoder().decode(r.body);
  try {
    // The burst once `a` is delivered, and the close once `d` is, so that neither arrives with what came before
    const sent = [];
    const end = await open('/three', { push: (m) => { sent.push(m); if (m.data === 'a' || m.data === 'd') peer.next('/three'); } });
    assert.deepEqual(sent[0], { event: '', id: '', data: 'a', coalesced: 0 });
    assert.equal(sent.at(-1).data, 'd', 'the newest of a burst');
    assert.equal(sent.reduce((n, m) => n + 1 + m.coalesced, 0), 4, 'every message delivered or counted');
    assert.deepEqual([end.kind, text(end)], [1, 'the socket closed (1000)']);
    const binary = await open('/binary', []);
    assert.deepEqual([binary.kind, text(binary)], [2, "a binary message: a socket's messages are text"]);
    // Letting the ticket go aborts it, and the far side sees the socket close.
    const controller = new AbortController();
    const { promise: heard, resolve: hear } = Promise.withResolvers();
    const done = open('/hold', { push: hear }, controller);
    await heard;
    controller.abort();
    assert.equal((await done).kind, 4);
    await peer.closed('/hold');
  } finally { peer.close(); }
}, 30_000); // hang bound

// The JS target's stream (`streamed`, LLP 1016.000): a source's request run
// as the wasm host runs it, so a socket is admitted by `net.websocket` alone
// and a `net.fetch` grant does not open one (the web module's fetch
// refused a socket as `net.fetch`, x2apps dash diary).
test('the JS target opens a socket under net.websocket, never net.fetch', async () => {
  const { streamed } = await import('./http-body.js');
  const peer = await socketPeer();
  const text = (r) => new TextDecoder().decode(r.body);
  try {
    const sent = [];
    const end = await streamed({ method: 'GET', url: `${peer.url}/three`, headers: [], maxResponseBytes: 64 }, normalized(`net.websocket ${peer.url}`), (m) => { sent.push(m); if (m.data === 'a' || m.data === 'd') peer.next('/three'); }, new AbortController());
    assert.deepEqual(sent[0], { event: '', id: '', data: 'a', coalesced: 0 });
    assert.deepEqual([end.kind, text(end)], [1, 'the socket closed (1000)']);
    const refused = await streamed({ url: `${peer.url}/three`, maxResponseBytes: 64 }, normalized(`net.fetch ${peer.url.replace('ws:', 'http:')}`), () => {}, new AbortController());
    assert.deepEqual([refused.kind, text(refused)], [2, "outside the app's grants (net.websocket)"]);
  } finally { peer.close(); }
});

test('a source-scoped POST can return through an app-granted middle origin', async () => {
  const hops = [], finals = [];
  const final = Bun.serve({ port: 0, async fetch(req) { finals.push([req.method, await req.text()]); return new Response('accepted'); } });
  const middle = Bun.serve({ port: 0, async fetch(req) { hops.push([req.method, await req.text()]); return new Response(null, { status: 307, headers: { location: `${final.url}finish` } }); } });
  const origin = Bun.serve({ port: 0, fetch() { return new Response(null, { status: 307, headers: { location: `${middle.url}middle` } }); } });
  const app = normalized(`net.fetch ${origin.url.origin}\nnet.fetch ${middle.url.origin}\nnet.fetch ${final.url.origin}`);
  const scope = `net.fetch ${origin.url.origin}\nnet.fetch ${final.url.origin}`;
  const js = createRequestExecutor('test.app', app, response => response.arrayBuffer());
  try {
    for (const run of [
      () => request({ method: 'POST', url: `${origin.url}start`, headers: [], body: btoa('payload'), scope }, { grantSet: app, controllers: new Set(), moduleLoader: null }),
      () => js({ method: 'POST', url: `${origin.url}start`, headers: [], raw: 'payload', scope }),
    ]) {
      const result = await run();
      expect([result.failed ?? result.kind ?? 0, new TextDecoder().decode(result.body)]).toEqual([0, 'accepted']);
    }
    expect([hops, finals]).toEqual([[['POST', 'payload'], ['POST', 'payload']], [['POST', 'payload'], ['POST', 'payload']]]);
  } finally { origin.stop(true); middle.stop(true); final.stop(true); }
});

test('the wasm early GET follows a return chain and is claimed at the granted final URL', async () => {
  globalThis.exact ??= {};
  const { claim, fetchEarly } = await import('./module-glue.js');
  let middleHits = 0, finalHits = 0;
  const final = Bun.serve({ port: 0, fetch() { finalHits++; return new Response('accepted'); } });
  const middle = Bun.serve({ port: 0, fetch() { middleHits++; return new Response(null, { status: 307, headers: { location: `${final.url}finish` } }); } });
  const origin = Bun.serve({ port: 0, fetch() { return new Response(null, { status: 307, headers: { location: `${middle.url}middle` } }); } });
  const app = normalized(`net.fetch ${origin.url.origin}\nnet.fetch ${middle.url.origin}\nnet.fetch ${final.url.origin}`);
  const scope = `net.fetch ${origin.url.origin}\nnet.fetch ${final.url.origin}`;
  try {
    const op = { method: 'GET', url: `${origin.url}start`, headers: [], scope };
    const source = scopedGrantSet(app, scope);
    expect(fetchEarly(op, source)).toBeFunction();
    const response = await request(op, { grantSet: app, controllers: new Set(), moduleLoader: { claim } });
    expect([response.kind, new TextDecoder().decode(response.body), middleHits, finalHits]).toEqual([0, 'accepted', 1, 1]);
  } finally { origin.stop(true); middle.stop(true); final.stop(true); }
});

test('a tool\'s request to the local network from a public page waits for the person, sent by neither path (LLP 1133.036 D6)', async () => {
  globalThis.exact ??= {};
  const { fetchEarly } = await import('./module-glue.js');
  const was = globalThis.location, sent = [], real = globalThis.fetch;
  let hits = 0;
  const light = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch() { hits++; return new Response('{}'); } });
  globalThis.location = { hostname: 'lights.example', origin: 'https://lights.example', href: 'https://lights.example/' };
  globalThis.fetch = (url, init) => { sent.push(String(url)); return real(url, init); };
  try {
    const set = normalized('device.local-network lanPurpose\nnet.fetch local-network development');
    const host = { grantSet: set, controllers: new Set(), moduleLoader: null };
    const lan = { method: 'GET', url: 'http://192.168.1.40/json/si', headers: [], origin: 'tool', reachState: 'prompt' };
    expect(fetchEarly(lan, set)).toBe(null);
    const refused = await request(lan, host);
    expect([refused.kind, new TextDecoder().decode(refused.body)]).toEqual([2, 'NotAllowedError: person-required']);
    const denied = await request({ ...lan, url: `${light.url}json/si`, reachState: 'denied' }, host);
    expect([denied.kind, new TextDecoder().decode(denied.body)]).toEqual([2, 'NotAllowedError: local network access is denied']);
    const socket = await request({ ...lan, url: `ws://127.0.0.1:${light.port}/`, stream: true }, { ...host, grantSet: normalized('device.local-network lanPurpose\nnet.websocket local-network development') });
    expect(new TextDecoder().decode(socket.body)).toBe('NotAllowedError: person-required');
    expect([sent, hits]).toEqual([[], 0]);
    // The app's own reach is the ask, and goes.
    const own = await request({ method: 'GET', url: `${light.url}json/si`, headers: [] }, host);
    expect([own.kind, own.status, hits]).toEqual([0, 200, 1]);
  } finally { light.stop(true); globalThis.fetch = real; globalThis.location = was; }
});

test('a scheme line admits every origin of its scheme, and a request names the line that admitted it', async () => {
  const set = normalized('net.fetch https://api.example\nnet.fetch http:\nnet.websocket wss://feed.example\nnet.websocket ws:');
  expect([
    admittingGrant(set, 'http://127.0.0.1:49152/x', 'fetch'), admittingGrant(set, 'https://api.example/x', 'fetch'),
    admittingGrant(set, 'ws://laptop.tailnet.ts.net:3773/ws', 'websocket'), admittingGrant(set, 'wss://feed.example/s', 'websocket'),
    admittingGrant(set, 'https://other.example/x', 'fetch'), admittingGrant(set, 'wss://other.example/s', 'websocket'),
  ]).toEqual(['net.fetch http:', 'net.fetch https://api.example', 'net.websocket ws:', 'net.websocket wss://feed.example', null, null]);
  const controller = new AbortController();
  const refused = await request({ method: 'GET', url: 'wss://other.example/s', headers: [], stream: true }, { grantSet: set, controllers: new Set(), controller });
  expect([refused.kind, new TextDecoder().decode(refused.body), controller.grant]).toEqual([2, "outside the app's grants (net.websocket)", undefined]);
  const admitted = new AbortController();
  const unreachable = await request({ method: 'GET', url: 'http://127.0.0.1:1/events', headers: [], stream: true }, { grantSet: set, controllers: new Set(), controller: admitted });
  expect([unreachable.kind, admitted.grant]).toEqual([1, 'net.fetch http:']);
});

// LLP 1016.000 D8: a long poll against a stand-in. Each request takes the
// next answer in the queue; `hold` keeps it open until the client lets go.
function standIn(answers) {
  const asked = [], held = [];
  let open = 0, most = 0;
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    asked.push(new URL(req.url).search); open++; most = Math.max(most, open);
    try {
      const a = answers.shift() ?? 'hold';
      if (a === 'hold') return await new Promise((resolve) => { held.push(req); req.signal.addEventListener('abort', () => resolve(new Response('', { status: 204 }))); });
      return new Response(a.body ?? null, { status: a.status ?? 200, headers: a.headers ?? { 'content-type': 'application/json' } });
    } finally { open--; }
  } });
  return { server, asked, held, most: () => most, origin: `http://127.0.0.1:${server.port}` };
}
function longPoll(s, controller, sent, poll = {}, extra = {}) {
  const op = { method: 'GET', url: `${s.origin}/changes?since=0&wait=25`, headers: [], body: '', stream: true, maxResponseBytes: 64, longPoll: { cursor: 'since', from: 'seq', sameCursorEvent: false, ...poll }, ...extra };
  // Each reply is counted (`replied`); one that is no event (`exact:reply`) is no message here.
  controller.replies = 0; controller.replied = () => controller.replies++;
  return request(op, { grantSet: normalized(`net.fetch ${s.origin}`), controllers: new Set(), controller, message: (m) => { if (m.event === 'exact:reply') controller.markers = (controller.markers ?? 0) + 1; else sent.push(m); } });
}
const until = async (ok) => { const end = performance.now() + 5000; while (!ok()) { if (performance.now() > end) throw new Error('timed out'); await new Promise((r) => setTimeout(r, 5)); } };

test('a long poll delivers each reply with its cursor, asks again with it, and heartbeats deliver nothing', async () => {
  const s = standIn([
    { body: '{"seq":1,"c":["a"]}' }, { status: 204 }, { body: ' ' }, { body: '{"seq":1,"c":[]}' }, { status: 304 },
    { body: '{"seq":2}' }, { body: '{"seq":"x3"}' },
  ]);
  const controller = new AbortController(), sent = [], started = performance.now();
  longPollTiming.heartbeatMs = 40;
  const done = longPoll(s, controller, sent);
  await until(() => s.held.length === 1);
  // Four heartbeats answered at once, each asked again no sooner than the floor.
  assert.ok(performance.now() - started >= 4 * 40);
  longPollTiming.heartbeatMs = 1000;
  assert.deepEqual(sent.map((m) => m.id), ['1', '2', 'x3']);
  assert.equal(sent[0].data, '{"seq":1,"c":["a"]}');
  assert.deepEqual(s.asked, ['?since=0&wait=25', ...Array(5).fill('?since=1&wait=25'), '?since=2&wait=25', '?since=x3&wait=25']);
  assert.equal(s.most(), 1, 'one request at a time');
  assert.equal(controller.markers, 4, 'each heartbeat a reply that is no event');
  assert.equal(controller.replies, 7, 'every reply counted');
  controller.abort();
  const end = await done;
  assert.equal(end.kind, 4);
  await until(() => s.held[0].signal.aborted);
  s.server.stop(true);
});

test('under sameCursor "event" a reply with the cursor asked with is delivered', async () => {
  const s = standIn([{ body: '{"seq":1,"n":1}' }, { body: '{"seq":1,"n":2}' }]);
  const controller = new AbortController(), sent = [];
  const done = longPoll(s, controller, sent, { sameCursorEvent: true });
  await until(() => s.held.length === 1);
  assert.deepEqual(sent.map((m) => m.data), ['{"seq":1,"n":1}', '{"seq":1,"n":2}']);
  controller.abort(); await done; s.server.stop(true);
});

test('a long poll waits out Retry-After, shows retrying, and a drop ends the wait', async () => {
  const s = standIn([{ status: 503, headers: { 'retry-after': '0' } }, { body: '{"seq":5}' }, { status: 429, headers: { 'retry-after': '30' } }]);
  const controller = new AbortController(), sent = [];
  const done = longPoll(s, controller, sent);
  await until(() => controller.retrying?.error === 'HTTP 429');
  assert.deepEqual(sent.map((m) => m.id), ['5']);
  assert.equal(controller.retrying.attempt, 1, 'the 2xx reset the count');
  assert.equal(controller.retrying.error, 'HTTP 429');
  assert.ok(controller.retrying.at - performance.now() > 25000);
  const dropped = performance.now();
  controller.abort();
  assert.equal((await done).kind, 4);
  assert.ok(performance.now() - dropped < 1000, 'the wait ended with the drop');
  assert.equal(controller.retrying, undefined);
  assert.equal(s.asked.length, 3, 'not asked again');
  s.server.stop(true);
});

test('what is not a poll reply ends a long poll, and only a bodiless GET is one', async () => {
  const cases = [
    [{ status: 404, body: 'gone', headers: {} }, (e) => e.kind === 0 && e.status === 404],
    [{ body: 'not json', headers: {} }, (e) => e.kind === 0 && e.status === 200],
    [{ body: '{"other":1}' }, (e) => e.kind === 0 && e.status === 200],
    [{ body: '[1,2]' }, (e) => e.kind === 0 && e.status === 200],
    [{ body: 'data: 1\n\n', headers: { 'content-type': 'text/event-stream' } }, (e) => e.kind === 2 && /event stream/.test(new TextDecoder().decode(e.body))],
    [{ body: `{"seq":1,"pad":"${'.'.repeat(80)}"}` }, (e) => e.kind === 2],
  ];
  for (const [answer, ok] of cases) {
    const s = standIn([answer]);
    const end = await longPoll(s, new AbortController(), []);
    assert.ok(ok(end), JSON.stringify({ answer, end: { ...end, body: new TextDecoder().decode(end.body) } }));
    s.server.stop(true);
  }
  const s = standIn([]);
  const post = await longPoll(s, new AbortController(), [], {}, { method: 'POST', body: btoa('x') });
  assert.equal(post.kind, 2);
  assert.match(new TextDecoder().decode(post.body), /a GET with no body/);
  assert.equal(s.asked.length, 0);
  s.server.stop(true);
});

test('a retry asks no sooner than the floor, a busy body is never read, and a cursor no host carries ends the poll', async () => {
  longPollTiming.heartbeatMs = 40;
  try {
    const s = standIn([{ status: 503, headers: { 'retry-after': '0' }, body: 'x'.repeat(100000) }, { status: 429, headers: { 'retry-after': 'Sun, 06 Nov 1994 08:49:37 GMT' } }, { body: '{"seq":9}' }]);
    const controller = new AbortController(), sent = [], started = performance.now();
    const done = longPoll(s, controller, sent);
    await until(() => sent.length === 1);
    assert.ok(performance.now() - started >= 80);
    assert.equal(controller.markers, 2);
    controller.abort(); await done; s.server.stop(true);
    for (const body of ['{"seq":9007199254740993}', '{"seq":"a\\nb"}', '{"seq":1.5}', '{"seq":"\\ud800"}']) {
      const t = standIn([{ body }]);
      const end = await longPoll(t, new AbortController(), []);
      assert.equal(end.kind, 2, body);
      t.server.stop(true);
    }
    // A long poll refused before anything is sent: a socket's URL, a bad `from`.
    const r = standIn([]);
    const ws = await request({ method: 'GET', url: r.origin.replace('http', 'ws'), headers: [], body: '', stream: true, longPoll: { cursor: 'since', from: 'seq', sameCursorEvent: false } }, { grantSet: normalized(`net.fetch ${r.origin}\nnet.websocket ${r.origin.replace('http', 'ws')}`), controllers: new Set(), controller: new AbortController() });
    assert.match(new TextDecoder().decode(ws.body), /HTTP, not a WebSocket/);
    // (an http URL with no `//` is refused, as natively)
    const { longPollRefusal } = await import('./http-body.js');
    assert.match(longPollRefusal({ url: 'https:example.test/changes', stream: true, longPoll: { cursor: 'since', from: 'seq', sameCursorEvent: false } }, 'GET', ''), /HTTP, not a WebSocket/);
    const plain = await longPoll(r, new AbortController(), [], {}, { stream: false });
    assert.match(new TextDecoder().decode(plain.body), /a stream's/);
    const deep = await longPoll(r, new AbortController(), [], { from: 'a.b.c' });
    assert.match(new TextDecoder().decode(deep.body), /exactLongPoll.from/);
    assert.equal(r.asked.length, 0);
    r.server.stop(true);
  } finally { longPollTiming.heartbeatMs = 1000; }
});
