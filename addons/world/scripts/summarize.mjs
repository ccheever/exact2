import {readFileSync,writeFileSync} from 'node:fs';
import {gzipSync} from 'node:zlib';
const read=name=>JSON.parse(readFileSync(new URL('../evidence/'+name,import.meta.url)));
const median=xs=>{const a=xs.toSorted((a,b)=>a-b),m=a.length/2;return a.length%2?a[Math.floor(m)]:(a[m-1]+a[m])/2;};
const out={proofs:{},startup:{},live:{},size:{},runnerCost:[]};
for(const host of ['web','macos','linux']){
 const p=read(host+'-proof.json');out.proofs[host]={passed:p.checks.filter(c=>c.ok).length,total:p.checks.length,initial:p.observations.initial.hash,continued:p.observations.continued.hash};
 const m=read(host+'-measure.json');
 if(host==='web')out.startup.web={runs:m.cold.length,firstTickBeforeMs:median(m.cold.map(x=>x.firstTick.before)),firstTickAfterMs:median(m.cold.map(x=>x.firstTick.after)),fcpMs:median(m.cold.map(x=>x.fcp)),tickBeforeFcpMs:median(m.cold.map(x=>x.fcp-x.firstTick.after)),wasmFetches:m.cold.map(x=>x.wasmFetches.length)};
 else out.startup[host]={runs:m.cold.length,firstTickMs:median(m.cold.map(x=>x.firstTickMs)),hostBootMs:median(m.cold.map(x=>x.hostBootMs))};
}
for(const interval of [10,100]){
 const m=read(`web-measure${interval===10?'':'-'+interval}.json`),a=m.live.from,b=m.live.to,seconds=(b.at-a.at)/1000;
 out.live['web'+interval]={seconds,ticksPerSecond:(b.state.resources.world.tick-a.state.resources.world.tick)/seconds,kernelEpochsPerSecond:(b.state.epoch-a.state.epoch)/seconds,allocationsPerSecond:(b.allocations-a.allocations)/seconds,domPublicationsPerSecond:(b.publications-a.publications)/seconds,hostAdvancesPerSecond:(b.advances-a.advances)/seconds};
}
for(const host of ['macos','linux']){
 const runs=read(host+'-live.json');
 const rows=runs[0].output.split('\n').filter(l=>l.startsWith('X1 {')).map(l=>JSON.parse(l.slice(3)));
 const a=rows.find(x=>x.now===2000)??rows[1],b=rows.at(-1),seconds=(b.wallUs-a.wallUs)/1e6;
 out.live[host]={seconds,ticksPerSecond:(b.tick-a.tick)/seconds,resourceQueriesPerSecond:(b.queries-a.queries)/seconds,allocationsPerSecond:(b.allocations-a.allocations)/seconds,requestedBytesPerSecond:(b.bytes-a.bytes)/seconds};
 const saved=runs[0].output.split('\n').find(l=>l.startsWith('X1SAVED '))?.split(' ').slice(1).join(' ');
 const restored=runs[1].output.split('\n').find(l=>l.startsWith('X1RESTORE '))?.split(' ').slice(1).join(' ');
 out.live[host].durableCheckpointEquality=!!saved&&saved===restored;
}
for(const [name,path] of [['tally','../dist/app.wasm'],['videoPlayer','../evidence/baseline-dist/app.wasm']]){
 const b=readFileSync(new URL(path,import.meta.url));out.size[name]={raw:b.length,gzip9:gzipSync(b,{level:9}).length};
}
const cost=readFileSync(new URL('../evidence/runner-cost.log',import.meta.url),'utf8');
for(const line of cost.split('\n'))if(line.includes('COST '))out.runnerCost.push(JSON.parse(line.slice(line.indexOf('COST ')+5)));
const tests=readFileSync(new URL('../evidence/final-tests.log',import.meta.url),'utf8');out.testsPassed=[...tests.matchAll(/test result: ok\. (\d+) passed/g)].reduce((a,m)=>a+Number(m[1]),0);
writeFileSync(new URL('../evidence/summary.json',import.meta.url),JSON.stringify(out,null,2));console.log(JSON.stringify(out,null,2));
