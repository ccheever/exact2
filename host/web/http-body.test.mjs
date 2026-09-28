import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {boundedHttpBody as readBody, waitForInflight} from './http-body.js';

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
    grants: ['net.fetch https://example.test'], granted: () => true, controllers: new Set(),
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
