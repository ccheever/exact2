import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {existsSync,mkdtempSync,rmSync,writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {boundedHttpBody as readBody, waitForInflight} from './http-body.js';

const ROOT=resolve(dirname(fileURLToPath(import.meta.url)),'../..'),sets=new Map();
function normalized(spec){
  if(sets.has(spec))return sets.get(spec);
  const scratch=mkdtempSync(resolve(tmpdir(),'exact-grants-')),input=resolve(scratch,'grants.txt');writeFileSync(input,spec);
  const target=resolve(process.env.CARGO_TARGET_DIR||resolve(ROOT,'target'),'debug/exact-web-js');
  const result=existsSync(target)?spawnSync(target,['normalize-grants',input],{cwd:ROOT,encoding:'utf8'}):spawnSync('cargo',['run','-q','-p','exact-web-js','--','normalize-grants',input],{cwd:ROOT,encoding:'utf8'});
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

// A local WebSocket peer (LLP 1069.004 slice 3): `/three` sends `a`, then
// `b c d` in one write, then a close; `/binary` a binary frame; `/hold`
// one message, then waits for the client to go.
async function socketPeer() {
  const { createServer } = await import('node:http');
  const { createHash } = await import('node:crypto');
  const frame = (op, payload) => Buffer.concat([Buffer.from([0x80 | op, payload.length]), payload]);
  const gone = [];
  const server = createServer((_, res) => res.end());
  server.on('upgrade', (req, socket) => {
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    socket.on('close', () => gone.push(req.url));
    socket.on('error', () => {});
    // The client's close: answer it and hang up (a server's half of the handshake).
    socket.on('data', (d) => { if ((d[0] & 0x0f) === 8) socket.end(frame(8, Buffer.from([0x03, 0xe8]))); });
    if (req.url === '/binary') return socket.write(frame(2, Buffer.from([1, 2, 3])));
    if (req.url === '/hold') return socket.write(frame(1, Buffer.from('held')));
    socket.write(frame(1, Buffer.from('a')));
    setTimeout(() => {
      socket.write(Buffer.concat(['b', 'c', 'd'].map((s) => frame(1, Buffer.from(s)))));
      setTimeout(() => socket.write(frame(8, Buffer.from([0x03, 0xe8]))), 30);
    }, 30);
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  return { url: `ws://127.0.0.1:${server.address().port}`, gone, close: () => server.close() };
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
    const sent = [];
    const end = await open('/three', sent);
    assert.deepEqual(sent[0], { event: '', id: '', data: 'a', coalesced: 0 });
    assert.equal(sent.at(-1).data, 'd', 'the newest of a burst');
    assert.equal(sent.reduce((n, m) => n + 1 + m.coalesced, 0), 4, 'every message delivered or counted');
    assert.deepEqual([end.kind, text(end)], [1, 'the socket closed (1000)']);
    const binary = await open('/binary', []);
    assert.deepEqual([binary.kind, text(binary)], [2, "a binary message: a socket's messages are text"]);
    // Letting the ticket go aborts it, and the far side sees the socket close.
    const controller = new AbortController(), held = [];
    const done = open('/hold', held, controller);
    while (!held.length) await new Promise((r) => setTimeout(r, 5));
    controller.abort();
    assert.equal((await done).kind, 4);
    for (let i = 0; i < 200 && !peer.gone.includes('/hold'); i++) await new Promise((r) => setTimeout(r, 5));
    assert.ok(peer.gone.includes('/hold'), 'the peer saw the socket close');
  } finally { peer.close(); }
});
