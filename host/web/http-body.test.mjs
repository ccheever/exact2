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
  const pending = new Set();
  const complete = Promise.resolve().then(() => pending.delete(complete));
  pending.add(complete);
  assert.equal(await waitForInflight(pending, performance.now() + 1000), true);
  pending.add(new Promise(() => {}));
  assert.equal(await waitForInflight(pending, performance.now()), false);
});
