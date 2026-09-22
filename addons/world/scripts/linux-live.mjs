import {killChildren} from './processes.mjs';
import {spawn} from 'node:child_process';
import {resolve} from 'node:path';
import {writeFileSync} from 'node:fs';
const runs=[];
try {
 for(const save of [true,false]) {
  const env={...process.env,X1_TRACE:'1',EXACT_SIZE:'820x900',EXACT_FONTS:resolve(import.meta.dirname,'../../../scripts/fixtures/fonts/assets'),EXACT_FONT:'DejaVu Sans',EXACT_ASSETS:resolve(import.meta.dirname,'../tally'),EXACT_UPDATE_TRUST:'development'};
  delete env.EXACT_AGENT;if(save)env.X1_LIVE_SAVE='1';
  const child=spawn(resolve(import.meta.dirname,'../target/release/tally-linux-live'),[],{env,stdio:['ignore','pipe','pipe']});
  let output='';child.stdout.on('data',b=>output+=b);child.stderr.on('data',b=>output+=b);
  await new Promise(r=>setTimeout(r,save?4400:1400));await killChildren();runs.push({save,output});
 }
}finally{await killChildren();writeFileSync(resolve(import.meta.dirname,'../evidence/linux-live.json'),JSON.stringify(runs,null,2));}
console.log(runs.map(r=>({save:r.save,lines:r.output.split('\n').filter(l=>l.startsWith('X1 {'))})));
