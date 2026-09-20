#!/usr/bin/env bun
// Run every requested measurement; retain commands, PIDs, numbers and all failures.
import { mkdirSync, appendFileSync, openSync, closeSync } from 'node:fs';
import { join } from 'node:path';
const root=import.meta.dir;
const out=join(process.env.HOME,'Library/Caches/exact2-cluster-lod/out/F1');
mkdirSync(out,{recursive:true});
const mode=process.argv[2]??'verify';
const commands=[];
if(mode==='verify') {
  commands.push(['tests',['cargo','test','--workspace','--no-fail-fast','--','--nocapture']],
    ['clippy',['cargo','clippy','--all-targets','--','-D','warnings']],
    ['fmt',['cargo','fmt','--all','--','--check']],
    ['wasm-format',['cargo','build','-p','clod-format','--target','wasm32-unknown-unknown']],
    ['wasm-view',['cargo','build','-p','clod-view','--lib','--target','wasm32-unknown-unknown']]);
} else if(mode==='sweep'||mode==='oracles') {
  for(const asset of ['gaul','washington']) {
    const source=join(out,'..',`${asset}-2.clod`);
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
  }
} else {
  console.error('usage: bun measure.mjs verify|sweep|oracles');
  process.exit(1);
}
const failures=[];
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
}
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
