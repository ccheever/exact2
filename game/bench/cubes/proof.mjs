#!/usr/bin/env bun
// A live-clock headless transport proof, never a refresh-rate measurement.
import {spawnSync} from 'node:child_process';
if (process.argv[2] === 'linux') {
  const {proof} = await import('../../proof.mjs');
  await proof(import.meta, async ({open, check}) => {
    const s = await open();
    await s.clock(17);
    const state = await s.state('world', {world:true});
    check('bench completes a headless tick', state.world?.tick === 1, state.world?.tick);
    check('bench has moving cube entities', state.world?.entities > 2, state.world?.entities);
    await s.close();
  });
}
const r=spawnSync(process.execPath,[new URL('../run.mjs',import.meta.url).pathname,'exact-web','cubes','1000'],{
  env:{...process.env,BENCH_HEADLESS:'1',BENCH_SECONDS:'1'},encoding:'utf8',stdio:['ignore','pipe','inherit'],
});
if(r.status!==0) throw new Error(`bench sanity failed: ${r.status}`);
const result=JSON.parse(r.stdout.trim().split('\n').at(-1));
if(result.instances!==1000 || result.measurement!=='sanity-only' || result.frames<1 || !result.perf.tickMs.count) throw new Error(r.stdout);
console.log('PASS live world, 1,000 entities, 2560×1440, phase samples, clean Chrome exit');
