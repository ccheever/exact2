import {test,expect} from 'bun:test';
import {open,assertWebDistApp} from '../../../scripts/agent.mjs';
import {refuseStale,webChanges} from '../../../scripts/agent-launch.mjs';

import {resolve} from 'node:path';
import {resolveApp} from '../../../scripts/app.mjs';
import {jsTargetBuild} from '../serve.mjs';
const app=resolveApp('caltrain');
const dist=resolve(process.env.EXACT_WEB_DIST ?? new URL('../dist',import.meta.url).pathname);
let unavailable;
try {
  await assertWebDistApp(dist,app);
  refuseStale('web',resolve(dist,'.exact-build.json'),webChanges(dist,app).app,
    `bun host/web/build.mjs ${app.crate('web')}${jsTargetBuild(dist) ? '' : ' --wasm'}`);
} catch(error) {
  if (!error.message.startsWith('web dist is not a complete build') && !error.message.startsWith('web build is stale')) throw error;
  unavailable=error.message;
  console.warn(`SKIP: ${unavailable}`);
}
const check = unavailable ? test.skip : test;
check(`Caltrain web frame-only stack remains a plain column${unavailable ? ` — ${unavailable}` : ''}`,async()=>{
  const s=await open({host:'web'});
  try {
    await s.clock('settle');await s.tap('deck-toggle');await s.clock('settle');
    const cards=(await s.layout()).nodes.filter(n=>n.testId?.startsWith('card-'));
    expect(cards.length).toBeGreaterThan(1);
    for(let i=1;i<cards.length;i++) {
      expect(Math.abs(cards[i].x-cards[0].x)).toBeLessThan(.1);
      expect(Math.abs(cards[i].w-cards[0].w)).toBeLessThan(.1);
      expect(cards[i].y).toBeGreaterThanOrEqual(cards[i-1].y+cards[i-1].h-.1);
    }
  } finally {await s.close();}
},60000);
