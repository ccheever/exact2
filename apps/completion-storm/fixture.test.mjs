import { test, expect } from 'bun:test';
import { createFixture } from './fixture.mjs';

test('API-only fixture needs no dist and refuses static requests', async () => {
  const f=await createFixture({port:0,controlPort:0,dist:null});
  try {
    expect((await fetch(`${f.control}/`)).status).toBe(404);
    expect((await fetch(`${f.control}/app.wasm`)).status).toBe(404);
    expect((await fetch(`${f.control}/api/stats`)).status).toBe(200);
    expect((await fetch(`${f.control}/api/open?count=1&errors=0`,{method:'POST'})).status).toBe(200);
  } finally { await f.close(); }
});

async function waitForHeld(f, count) {
  for (let attempt=0; attempt<100; attempt++) {
    if ((await (await fetch(`${f.control}/api/stats`)).json()).held===count) return;
    await Bun.sleep(5);
  }
  throw new Error('requests did not arrive');
}

test('browser preflight admits the real cross-origin request headers', async () => {
  const f=await createFixture({port:0,controlPort:0});
  try {
    const reply=await fetch(`${f.data}/api/hold`,{method:'OPTIONS',headers:{Origin:f.control,'Access-Control-Request-Method':'GET','Access-Control-Request-Headers':'cache-control'}});
    expect(reply.status).toBe(204);
    expect(reply.headers.get('access-control-allow-origin')).toBe('*');
    expect(reply.headers.get('access-control-allow-headers')).toContain('cache-control');
  } finally { await f.close(); }
});

test('held responses release together; release stays open for queued arrivals', async () => {
  const f = await createFixture({port:0, controlPort:0});
  try {
    const wave = await (await fetch(`${f.control}/api/open?count=3&errors=0`, {method:'POST'})).json();
    const first = fetch(`${f.data}/api/hold?wave=${wave.id}&lane=0`);
    await waitForHeld(f, 1);
    const release = await (await fetch(`${f.control}/api/release?wave=${wave.id}`, {method:'POST'})).json();
    expect(release.released).toBe(1);
    expect((await (await first).json()).lane).toBe(0);
    const late = await fetch(`${f.data}/api/hold?wave=${wave.id}&lane=1`);
    expect(late.status).toBe(200);
    expect((await late.json()).wave).toBe(wave.id);
  } finally { await f.close(); }
});

test('bounds reject oversized, duplicate and unknown work; mixed errors really cross HTTP', async () => {
  const f = await createFixture({port:0, controlPort:0, maxWaves:1});
  try {
    for (const count of ['129','-1','NaN','1.5']) {
      expect((await fetch(`${f.control}/api/open?count=${count}&errors=0`, {method:'POST'})).status).toBe(400);
    }
    const wave = await (await fetch(`${f.control}/api/open?count=4&errors=100`, {method:'POST'})).json();
    expect((await fetch(`${f.control}/api/open?count=1&errors=0`, {method:'POST'})).status).toBe(429);
    expect((await fetch(`${f.data}/api/hold?wave=999&lane=0`)).status).toBe(410);
    expect((await fetch(`${f.data}/api/hold?wave=${wave.id}&lane=4`)).status).toBe(400);
    await fetch(`${f.control}/api/release?wave=${wave.id}`, {method:'POST'});
    expect((await fetch(`${f.data}/api/hold?wave=${wave.id}&lane=0`)).status).toBe(503);
    expect((await fetch(`${f.data}/api/hold?wave=${wave.id}&lane=0`)).status).toBe(409);
    expect(await (await fetch(`${f.data}/api/hold?wave=${wave.id}&lane=1`)).text()).toBe('{broken');
    expect((await (await fetch(`${f.data}/api/hold?wave=${wave.id}&lane=2`)).json()).wave).not.toBe(wave.id);
    await expect(fetch(`${f.data}/api/hold?wave=${wave.id}&lane=3`).then(r=>r.text())).rejects.toThrow();
  } finally { await f.close(); }
});

test('held work times out', async () => {
  const f = await createFixture({port:0, controlPort:0, holdMs:20});
  try {
    const wave = await (await fetch(`${f.control}/api/open?count=1&errors=0`, {method:'POST'})).json();
    expect((await fetch(`${f.data}/api/hold?wave=${wave.id}&lane=0`)).status).toBe(408);
  } finally { await f.close(); }
});

test('held limit recovers after disconnect; shutdown settles a live held request', async () => {
  const f=await createFixture({port:0,controlPort:0,maxHeld:1});
  try {
    const wave=await (await fetch(`${f.control}/api/open?count=3&errors=0`,{method:'POST'})).json();
    const controller=new AbortController();
    const first=fetch(`${f.data}/api/hold?wave=${wave.id}&lane=0`,{signal:controller.signal}).catch(error=>error);
    await waitForHeld(f,1);
    expect((await fetch(`${f.data}/api/hold?wave=${wave.id}&lane=1`)).status).toBe(429);
    controller.abort();
    expect(await first).toBeInstanceOf(Error);
    await waitForHeld(f,0);
    expect((await (await fetch(`${f.control}/api/stats`)).json()).abandoned).toBe(1);
    // The rejected lane was not consumed; capacity is reusable after disconnect.
    const second=fetch(`${f.data}/api/hold?wave=${wave.id}&lane=1`).then(async r=>({status:r.status,body:await r.text()})).catch(error=>({error:String(error)}));
    await waitForHeld(f,1);
    await f.close();
    const result=await Promise.race([second,Bun.sleep(1000).then(()=>({timeout:true}))]);
    expect(result.timeout).not.toBe(true);
    expect(result.status===408 || typeof result.error==='string').toBe(true);
  } finally { await f.close(); }
});
