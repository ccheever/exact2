#!/usr/bin/env bun
// Run every requested measurement; retain commands, PIDs, numbers and all failures.
import { mkdirSync, appendFileSync, openSync, closeSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
const root=import.meta.dir;
const out=join(process.env.HOME,'Library/Caches/exact2-cluster-lod/out/F3');
mkdirSync(out,{recursive:true});
const mode=process.argv[2]??'verify';
if(mode==='benchmark') { await import('./benchmark.mjs'); process.exit(process.exitCode??0); }
const commands=[];
if(mode==='verify') {
  commands.push(['tests',['cargo','test','--workspace','--no-fail-fast','--','--nocapture']],
    ['clippy',['cargo','clippy','--all-targets','--','-D','warnings']],
    ['fmt',['cargo','fmt','--all','--','--check']],
    ['wasm-format',['cargo','build','-p','clod-format','--target','wasm32-unknown-unknown']],
    ['wasm-view',['cargo','build','-p','clod-view','--lib','--target','wasm32-unknown-unknown']]);
 } else if(mode==='pacing') {
  commands.push(['build-view',['cargo','build','-p','clod-view']]);
  for(const asset of ['gaul','washington']) for(const mode of ['cluster','naive']) for(const instrumented of [true,false]) {
    commands.push([`${asset}-${mode}-${instrumented?'timed':'intervals'}-600`,['target/debug/clod-view','demo',join(out,'..',`${asset}-5.clod`),'--mode',mode,'--frames','600','--exit',...(instrumented?[]:['--intervals-only'])]]);
  }
} else if(mode==='bake') {
  commands.push(['build-baker',['cargo','build','--release','-p','clod-bake']]);
  for(const [asset,file] of [['gaul','smk-dying-gaul-kas1312/smk-190-inv-dying-gladiator.stl'],['washington','si-george-washington-greenough/george-washington-greenough-statue-(1840)-master-geometry.obj']]) {
    for(let repeat=0;repeat<2;repeat++) commands.push([`${asset}-bake-${repeat}`,['target/release/clod-bake',join(out,'../../assets',file),join(out,'..',`${asset}-5${repeat?'-repeat':''}.clod`)]]);
  }
} else if(mode==='images') {
  for(const asset of ['gaul','washington']) {
    const source=join(out,'..',`${asset}-5.clod`);
    commands.push([`${asset}-compare`,['target/debug/clod-view','compare',source,'--threshold-px','0.5,1,2,4,8','--t','0,0.25,0.5,0.75,1','--out',join(out,`${asset}-compare`)]]);
    commands.push([`${asset}-pop`,['target/debug/clod-view','pop',source,'--steps','240','--out',join(out,`${asset}-pop`)]]);
    for(const view of ['lit','clusters']) commands.push([`${asset}-close-${view}`,['target/debug/clod-view','render',source,'--t','1','--view',view,'--out',join(out,`${asset}-close-${view}.png`)]]);
  }
} else if(mode==='sweep'||mode==='oracles') {
  for(const asset of ['gaul','washington']) {
    const source=join(out,'..',`${asset}-5.clod`);
    if(mode==='oracles') {
      commands.push([`${asset}-oracle`,['target/debug/clod-view','oracle',source,'--steps','64','--size','256x256']]);
      continue;
    }
    for(const layout of ['single','ring:12','grid:400','field:5000,1']) {
      for(const [selector,shadows] of [['gpu','on'],['brute','on'],['cpu','on'],['naive','on'],['gpu','off'],['naive','off']]) {
        const name=[asset,layout.replaceAll(/[:,]/g,'-'),selector,`shadows-${shadows}`].join('-');
        const command=['target/debug/clod-view','time',source,'--layout',layout,'--frames','7','--size','2560x1440','--threshold-px','1','--shadows',shadows,'--out',join(out,`${name}.png`)];
        command.push(...(selector==='naive'?['--mode','naive']:['--select',selector]));
        commands.push([name,command]);
      }
    }
    const [layout,capacity]=asset==='gaul'?['field:5000,1',3000]:['grid:400',12000];
    for(const shadows of ['on','off']) {
      const name=[asset,layout.replaceAll(/[:,]/g,'-'),'gpu',`capacity-${capacity}`,`shadows-${shadows}`].join('-');
      commands.push([name,['target/debug/clod-view','time',source,'--layout',layout,'--capacity',String(capacity),'--frames','7','--size','2560x1440','--threshold-px','1','--shadows',shadows,'--out',join(out,`${name}.png`)]]);
    }
  }
} else {
  console.error('usage: bun measure.mjs verify|sweep|oracles|bake|images|benchmark|pacing');
  process.exit(1);
}
const failures=[];
const pacing=[];
for(const [name,command] of commands) {
  const start=performance.now();
  const fd=openSync(join(out,`${name}.log`),'w');
  const child=Bun.spawn(command,{cwd:root,stdout:fd,stderr:fd});
  const record={name,pid:child.pid,command};
  appendFileSync(join(out,'processes.jsonl'),JSON.stringify(record)+'\n');
  console.log(JSON.stringify(record));
  const code=await child.exited;
  closeSync(fd);
  Object.assign(record,{exit:code,seconds:(performance.now()-start)/1000});
  appendFileSync(join(out,'runs.jsonl'),JSON.stringify(record)+'\n');
  console.log(JSON.stringify(record));
  if(code) failures.push(name);
  if(mode==='pacing' && name!=='build-view') {
    const data=readFileSync(join(out,`${name}.log`),'utf8').split('\n').flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}});
    const row=data.findLast(r=>r.command==='demo');
    if(row)pacing.push(row);
    if(!row||row.frames!==600||row.intervals!==600||!row.refresh_hz||row.overflow!==0||row.shadow_overflow!==0||row.overflow_frames_checked!==600)failures.push(`${name}: incomplete pacing run`);
  }
  if(mode==='sweep' && !code) {
    const rows=readFileSync(join(out,`${name}.log`),'utf8').split('\n').flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}});
    const measured=rows.findLast(row=>row.command==='time');
    if(!measured || measured.overflow || measured.shadow_overflow) failures.push(`${name}: missing measurement or dropped geometry`);
  }
}
if(mode==='pacing') writeFileSync(join(root,'results/f3-pacing.json'),JSON.stringify({pacing,failures},null,2)+'\n');
const lengths=[];
for await(const path of new Bun.Glob('**/*.{rs,wgsl,mjs,c,cpp,h}').scan({cwd:root})) {
  if(path.startsWith('vendor/')||path.startsWith('target/')) continue;
  const text=await Bun.file(join(root,path)).text();
  const lines=text.split('\n').length-Number(text.endsWith('\n'));
  lengths.push([lines,path]);
  if(lines>1500) failures.push(`${path}: ${lines} lines`);
}
lengths.sort((a,b)=>b[0]-a[0]||a[1].localeCompare(b[1]));
console.log(JSON.stringify({mode,commands:commands.length,source_files:lengths.length,max_lines:lengths[0],failures}));
process.exit(Number(failures.length>0));
