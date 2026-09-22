import {killChildren} from './processes.mjs';
import {resolve} from 'node:path';
import {writeFileSync} from 'node:fs';
process.env.EXACT_APP_DIR=resolve(import.meta.dirname,'../tally');process.env.CARGO_TARGET_DIR=resolve(import.meta.dirname,'../target');
const {open}=await import('../../../scripts/agent.mjs');let s;const out={};
async function start(){s=await open({host:'macos',app:'tally',env:{EXACT_STORE:'real'},size:[820,900]});await new Promise(r=>setTimeout(r,250));}
async function close(){await killChildren();if(s)await s.close();s=null;}
async function inspect(){await s.tap('inspect');return (await s.state()).slots.inspection;}
async function play(){await s.tap('draw');await s.clock('+100');await s.tap('hold');await s.clock('+100');}
try{
 await start();await s.tap('forget');await close();
 await start();out.initial=await inspect();await s.clock('+1000');await play();await s.tap('save');out.saved=await inspect();await play();out.expected=await inspect();await close();
 await start();out.restored=await inspect();await play();out.continued=await inspect();out.equal=out.expected.hash===out.continued.hash&&out.expected.tick===out.continued.tick;
 await s.tap('forget');
}catch(e){out.error=String(e.stack??e);console.error(e);}
finally{await close();writeFileSync(resolve(import.meta.dirname,'../evidence/macos-store.json'),JSON.stringify(out,null,2));}
console.log(JSON.stringify({equal:out.equal,initial:out.initial?.hash,continued:out.continued?.hash,error:out.error}));
