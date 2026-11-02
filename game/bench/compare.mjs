// One sitting: all five engines at each N, then their five refresh-rate sweeps.
import {spawn} from 'node:child_process';
import {appendFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {displayReady} from './exact.mjs';

export async function compare() {
  displayReady();
  if(process.env.BENCH_HEADLESS==='1') throw new Error('Comparison requires an unlocked display');
  const engines=[['exact-web'],['three','instanced','webgl'],['godot-web','multimesh'],['exact-macos'],['godot','multimesh']];
  const out=resolve(import.meta.dir,'results');mkdirSync(out,{recursive:true});
  const stamp=new Date().toISOString().replaceAll(':','-');
  const path=resolve(out,`cubes-${stamp}.jsonl`), table=[];
  const run=async(args)=>{
    const child=spawn(process.execPath,[resolve(import.meta.dir,'run.mjs'),...args],{stdio:['ignore','pipe','inherit'],env:process.env});
    let stdout='';child.stdout.on('data',d=>{stdout+=d;});
    const code=await new Promise((ok,no)=>{child.once('exit',ok);child.once('error',no);});
    if(code!==0) throw new Error(`${args.join(' ')} failed (${code}); completed runs retained in ${path}`);
    return stdout.trim().split('\n').map(JSON.parse);
  };
  const record=(row)=>{const line=JSON.stringify(row);appendFileSync(path,line+'\n');console.log(line);};
  for(const n of [10000,100000,200000,500000]) {
    let accepted;
    for(let attempt=1;attempt<=3;attempt++) {
      const group=[];
      for(const [engine,...mode] of engines) {
        displayReady();const rows=await run([engine,'cubes',String(n),...mode]);
        group.push(rows.at(-1));
      }
      // All five being within 2× is stricter than either web/native triple.
      const loads=group.map(r=>r.load1), valid=Math.max(...loads)<=2*Math.min(...loads);
      for(const row of group) record({...row,attempt,load_group_valid:valid});
      if(valid){accepted=group;break;}
      console.error(`N=${n}: load moved more than 2×; retaking all five engines`);
    }
    if(!accepted) throw new Error(`N=${n}: three load-mismatched attempts; stop and retake later`);
    table.push(`| ${n.toLocaleString('en-US')} | ${accepted.map(r=>`${r.fps_avg} / ${r.ms_p95} / ${r.script_ms_avg} (load ${r.load1})`).join(' | ')} |`);
  }
  const sweeps=[];
  for(const [engine,...mode] of engines) {
    displayReady();const rows=await run(['sweep',engine,'cubes',...mode]);
    for(const row of rows) record({...row,sweep_engine:engine});
    sweeps.push(rows.at(-1));
  }
  const markdown=`# Cubes — ${stamp}\n\nCells: fps / p95 ms / script or tick ms (load1).\n\n| N | exact-web | three instanced webgl | godot-web multimesh | exact-macos | godot multimesh native |\n|---|---|---|---|---|---|\n${table.join('\n')}\n\n| Engine | Holds | Breaks | load1 |\n|---|---|---|---|\n${sweeps.map(r=>`| ${r.engine} | ${r.holds} | ${r.breaks} | ${r.load1} |`).join('\n')}\n`;
  writeFileSync(resolve(out,`cubes-${stamp}.md`),markdown);console.error(markdown);
  console.error(`Raw phases and loads: ${path}`);
}
