// Regenerate the Q2 lead table from the actual v4 assets; continue through every failed row.
import {mkdirSync,openSync,closeSync,appendFileSync,readFileSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
const root=import.meta.dir;
const cache=join(process.env.HOME,'Library/Caches/exact2-cluster-lod/out');
const out=join(cache,'L3b','benchmark');mkdirSync(out,{recursive:true});
const rows=[],failures=[];
async function run(name,args) {
  const log=join(out,`${name}.log`),fd=openSync(log,'w'),start=performance.now();
  const p=Bun.spawn(args,{cwd:root,stdout:fd,stderr:fd});
  const record={name,pid:p.pid,args};appendFileSync(join(out,'processes.jsonl'),JSON.stringify(record)+'\n');console.log(JSON.stringify(record));
  const exit=await p.exited;closeSync(fd);console.log(JSON.stringify({...record,exit,seconds:(performance.now()-start)/1000}));
  const data=readFileSync(log,'utf8').split('\n').flatMap(line=>{try{return [JSON.parse(line)];}catch{return [];}});
  if(exit)failures.push({name,exit,errors:data.filter(x=>x.error||x.failures?.length)});
  return data;
}
await run('build',['cargo','build','-p','clod-view']);
for(const asset of ['gaul','washington']) {
  for(const layout of ['single','ring:12','avenue:25','grid:400','field:5000,1']) {
    const name=`${asset}-${layout.replaceAll(/[:,]/g,'-')}`;
    const data=await run(name,['target/debug/clod-view','bench',join(cache,`${asset}-4.clod`),'--layout',layout,'--frames','7','--size','2560x1440','--t',layout==='avenue:25'?'0.45':'0','--threshold-px','1']);
    const measured=data.filter(r=>r.command==='bench').map(r=>({...r,asset}));rows.push(...measured);
    if(measured.length!==1)failures.push({name,reason:`expected 1 measured row, got ${measured.length}`});
  }
}
const sweep=(await run('washington-quality',['target/debug/clod-view','bench',join(cache,'washington-4.clod'),'--layout','avenue:25','--frames','7','--size','2560x1440','--t','0.75','--threshold-px','0.5,1,2,4'])).filter(r=>r.command==='bench');
if(sweep.length!==4)failures.push({name:'quality',reason:`expected 4 rows, got ${sweep.length}`});
for(const r of [...rows,...sweep]) {
  if(!r.coverage_interior_pixels||r.overflow||!Number.isFinite(r.ratio)||r.gpu_main_ms<=0)failures.push({asset:r.asset,layout:r.layout,reason:'empty coverage, overflow or invalid timing'});
}
const f=(n,d=3)=>Number(n).toFixed(d),n=x=>Number(x).toLocaleString('en-US');
const lines=[
'<!-- L3B_BENCHMARK_START -->',
'Apple M5 Max / Metal, format v4, 2560×1440, 4× MSAA, 4096² shadows, 1 px. Regenerate with `bun measure.mjs benchmark` (from this directory). Seven measured frames after one warmup per renderer. Single/ring/grid/field use t=0; avenue uses t=.45. Both renderers use the same instance-frustum cull. Times are medians in milliseconds; ratio is naive main / cluster main.',
'',
'| Asset · layout | Source triangles × instances | Drawn triangles | GPU main / shadow / select ms | CPU ms | Naive main ms | Ratio | Resident bytes cluster / naive | RGB mean / p99.9 | Coverage cracks / interior |',
'|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|',
...rows.map(r=>`| ${r.asset} · ${r.layout} | ${n(r.source_triangles)} × ${r.instances} | ${n(r.triangles_drawn)} | ${f(r.gpu_main_ms)} / ${f(r.gpu_shadow_ms)} / ${f(r.gpu_select_ms)} | ${f(r.cpu_ms)} | ${f(r.naive_gpu_main_ms)} | ${f(r.ratio,2)}× | ${n(r.bytes_resident_cluster)} / ${n(r.bytes_resident_naive)} | ${f(r.image_mean,7)} / ${f(r.image_p999,7)} | ${r.coverage_cracks} / ${n(r.coverage_interior_pixels)} |`),
'',
'Hero quality dial, Washington avenue t=.75, same settings:',
'',
'| Threshold px | Drawn triangles | GPU total / main / shadow / select ms | RGB mean / p99.9 | Coverage cracks / interior |',
'|---:|---:|---:|---:|---:|',
...sweep.map(r=>`| ${r.threshold_px} | ${n(r.triangles_drawn)} | ${f(r.gpu_ms)} / ${f(r.gpu_main_ms)} / ${f(r.gpu_shadow_ms)} / ${f(r.gpu_select_ms)} | ${f(r.image_mean,7)} / ${f(r.image_p999,7)} | ${r.coverage_cracks} / ${n(r.coverage_interior_pixels)} |`),
'',
'RGB mean is the mean absolute sRGB channel difference, normalized to 0–1; p99.9 is the nearest-rank percentile of the maximum RGB-channel difference per pixel. Lit comparisons include each renderer’s own shadows. Coverage counts completely missing pixels inside the fully covered naive mask after one-pixel erosion; silhouettes and partial MSAA samples are excluded. These image metrics are measured errors, not a proof that quadric bake error bounds screen pixels. Resident bytes count each renderer separately, including its allocated geometry, selection buffers and attachments, excluding driver overhead and diagnostic readbacks. GPU select includes main and shadow selection. Raw measurements: [L3b benchmark records](results/l3b-benchmark.json).',
'',`Measured rows: ${rows.length} + ${sweep.length} sweep; failures: ${failures.length}.`,
'<!-- L3B_BENCHMARK_END -->'
];
const result={rows,sweep,failures};writeFileSync(join(root,'results/l3b-benchmark.json'),JSON.stringify(result,null,2)+'\n');
const path=join(root,'README.md');let readme=readFileSync(path,'utf8');
if(readme.includes('<!-- L3B_BENCHMARK_START -->'))readme=readme.replace(/<!-- L3B_BENCHMARK_START -->[\s\S]*?<!-- L3B_BENCHMARK_END -->/,lines.join('\n'));
else readme=readme.replace('# Cluster LOD on core WebGPU\n','# Cluster LOD on core WebGPU\n\n'+lines.join('\n')+'\n');
writeFileSync(path,readme);console.log(JSON.stringify({benchmark_rows:rows.length,sweep_rows:sweep.length,failures}));process.exitCode=Number(failures.length>0);
