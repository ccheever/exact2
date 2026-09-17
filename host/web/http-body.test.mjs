import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const source=readFileSync(new URL('./glue.js',import.meta.url),'utf8');
const actual=source.match(/async function boundedHttpBody\(response, limit\) \{[\s\S]*?\n\}/)[0];
const readBody=Function(`${actual}; return boundedHttpBody;`)();

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
