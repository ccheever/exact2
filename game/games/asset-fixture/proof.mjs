#!/usr/bin/env bun
import { proof } from '../../proof.mjs';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

await proof(import.meta, async ({open,check,out,say}) => {
  const web = (process.argv[2] ?? 'web') === 'web';
  let delivered = false, requested = false, server, fetched;
  const requestSeen = new Promise(resolve => { fetched = resolve; });
  // Delay only this one file, making fetch timing observable. The carrier still
  // uses the production page, ABI, asset fetch and its real agent clock barrier.
  if (web) server = Bun.serve({hostname:'127.0.0.1',port:0,async fetch(request) {
    const path = decodeURIComponent(new URL(request.url).pathname);
    if (path === '/assets/crate.model') {
      requested = true; fetched();
      await Bun.sleep(700);
      delivered = true;
    }
    if (path.startsWith('/__')) return new Response('', {status:204});
    if (path.includes('..')) return new Response('',{status:404});
    const file = Bun.file(resolve(import.meta.dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
    return await file.exists() ? new Response(file) : new Response('',{status:404});
  }});
  try {
    const s = await open(server ? {url:`http://127.0.0.1:${server.port}/`} : {});
    const play = s.tap('play');
    if (web) await requestSeen; else await play;
    check('clock issued after Play while bytes are outstanding', !web || requested && !delivered, {requested,delivered});
    const start = performance.now();
    const clock = s.clock(1000);
    await play;
    await clock;
    const elapsed = performance.now() - start;
    const state = await s.state();
    const world = state.world[0];
    check('immediate clock waits for declared asset delivery', !web || requested && delivered, {requested,delivered,clockWallMs:elapsed});
    check('setup ran and exactly 60 deterministic ticks completed', world.tick === 60 && world.loading.length === 0, world);
    check('native and web simulation hash agrees', world.hash === '0x8f6d518f39634478', world.hash);
    const layout = await s.layout('world:crate');
    check('model bounds available to layout', JSON.stringify(layout).includes('bounds'), layout);
    await s.screenshot(resolve(out,`crate-${process.argv[2] ?? 'web'}.png`));
    await s.screenshot(resolve(out,'crate.world'),'world','save');
    const receipt = web ? JSON.parse(readFileSync(resolve(import.meta.dir,'dist/bake.json'),'utf8')).compat : null;
    say(`asset model bytes: ${readFileSync(resolve(import.meta.dir,'assets/crate.model')).length}; baked assets: ${JSON.stringify(receipt?.embedded.assets ?? [])}`);
    await s.close();
  } finally { server?.stop(true); }
});
