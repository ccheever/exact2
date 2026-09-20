import {killChildren} from './processes.mjs';
import {spawn} from 'node:child_process';
import {resolve} from 'node:path';
import {writeFileSync} from 'node:fs';
process.env.EXACT_APP_DIR=resolve(import.meta.dirname,'../tally');
process.env.CARGO_TARGET_DIR=resolve(import.meta.dirname,'../target');
const {resolveApp}=await import('../../../scripts/app.mjs');
const {appleArtifacts}=await import('../../../host/apple/build.mjs');
const binary=appleArtifacts(resolveApp('tally'),{destination:'macos'}).binary;
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const runs=[];
try{
 for(const save of [true,false]){
  const env={...process.env,X1_TRACE:'1',EXACT_UPDATE_TRUST:'development'};delete env.EXACT_AGENT;
  if(save)env.X1_LIVE_SAVE='1';else delete env.X1_LIVE_SAVE;
  const child=spawn(binary,[],{env,stdio:['ignore','pipe','pipe']});let output='';
  child.stderr.on('data',b=>output+=b);child.stdout.on('data',b=>output+=b);
  await sleep(save?4500:1400);
  await killChildren();
  runs.push({save,output});
 }
}finally{await killChildren();writeFileSync(resolve(import.meta.dirname,'../evidence/macos-live.json'),JSON.stringify(runs,null,2));}
console.log(runs.map(r=>({save:r.save,lines:r.output.split('\n').filter(l=>l.startsWith('X1 {'))})));
