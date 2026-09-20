import {killChildren} from './processes.mjs';
import {resolve} from 'node:path';
import {writeFileSync} from 'node:fs';
process.env.EXACT_APP_DIR=resolve(import.meta.dirname,'../tally');
process.env.CARGO_TARGET_DIR=resolve(import.meta.dirname,'../target');
process.env.EXACT_UPDATE_TRUST='development';
const {open}=await import('../../../scripts/agent.mjs');
const host=process.argv[2]??'linux';const result={host,cold:[]};let s;
try{
 for(let i=0;i<10;i++){
  const launched=performance.now();s=await open({host,app:'tally',size:[820,900]});
  const readyMs=performance.now()-launched;
  // Native ready precedes AppKit's first usable button dispatch. The stored
  // source timestamps were already captured; this wait is outside timing.
  if(host==='macos')await new Promise(r=>setTimeout(r,250));
  await s.tap('inspect');const state=await s.state();
  result.cold.push({hostBootMs:s.boot,parentReadyMs:readyMs,...state.slots.metrics,firstTickMs:(state.slots.metrics.firstTickUs-state.slots.metrics.entryUs)/1000});
  await killChildren();await s.close();s=null;
 }
 s=await open({host,app:'tally',size:[820,900]});
 if(host==='macos')await new Promise(r=>setTimeout(r,250));
 await s.clock('+1000');await s.tap('inspect');const a=await s.state();
 await s.clock('+1000');await s.tap('inspect');const b=await s.state();
 result.agentCost={from:a,to:b};
}catch(e){result.error=String(e.stack??e);console.error(e);}
finally{await killChildren();if(s)await s.close();writeFileSync(resolve(import.meta.dirname,`../evidence/${host}-measure.json`),JSON.stringify(result,null,2));}
console.log(JSON.stringify({host,cold:result.cold.length,error:result.error}));
