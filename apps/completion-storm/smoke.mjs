#!/usr/bin/env bun
// Opt-in web/macOS/Linux-host correctness drive; start the local fixture first.
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {platform} from 'node:os';
import {open} from '../../scripts/agent.mjs';
const args=process.argv.slice(2);
const outputIndex=args.indexOf('--out');
assert.ok(outputIndex<0 || args[outputIndex+1],'--out needs a writable directory');
const output=outputIndex<0?new URL('./target/',import.meta.url).pathname:resolve(args[outputIndex+1]);
const positional=outputIndex<0?args:args.filter((_,i)=>i!==outputIndex && i!==outputIndex+1);
const host=positional[0] ?? 'web';
const lanes=Number(positional[1] ?? (host==='web'?128:6));
assert.ok(['web','macos','linux'].includes(host),'host: web, macos, linux');
assert.ok([6,32,128].includes(lanes),'lanes: 6, 32, 128');
mkdirSync(output,{recursive:true});
const acknowledgements=[];
const app = await open({host,app:'completion-storm',...(host==='web'?{url:'http://127.0.0.1:4320'}:{}),size:[1000,2000]});
const release = async (wave=0) => {
  const response=await fetch(`http://127.0.0.1:4320/api/release?wave=${wave}`,{method:'POST'});
  assert.equal(response.status,200);
  return response.json();
};
async function typeAndAcknowledge(value, phase) {
  const start=performance.now();
  await app.type('stress-input',value);
  assert.equal((await app.find('stress-echo')).props.text,value);
  assert.equal((await app.state()).slots.note,value);
  acknowledgements.push({phase,commandToStateMs:performance.now()-start});
}
async function until(predicate, label) {
  const deadline=Date.now()+20000;
  while (Date.now()<deadline) {
    const state=await app.state();
    if (predicate(state)) return state;
    await Bun.sleep(20);
  }
  const state=await app.state();
  throw Error('timeout: '+label+' '+JSON.stringify({slots:state.slots,pending:state.pending,logs:await app.logs()}));
}
try {
  await app.tap(`count-${lanes}`);
  await app.tap('start-wave');
  const first=await until(s=>s.derives.pendingCount===lanes,'first pending');
  assert.equal(first.derives.validCount,0);
  const firstId=first.derives.waveId;
  await typeAndAcknowledge(`typing while ${lanes} logical requests are pending`,'held');
  await app.tap('interact');
  assert.equal((await app.state()).slots.clicks,1);
  await app.tap('navigate-away');
  assert.equal((await app.state()).derives.pendingCount,0);
  assert.equal((await app.tree()).nodes.filter(n=>n.props.testId==='lane-0').length,0);
  // On native, the old held HTTP request blocks the one FIFO worker.
  // External release precedes remount/admission so that queue can progress.
  const oldRelease=host==='web'?null:await release(firstId);
  await app.tap('start-wave');
  const second=await until(s=>s.derives.pendingCount===lanes,'remounted pending');
  assert.notEqual(second.derives.waveId,firstId);
  const freshRelease=await release(host==='web'?0:second.derives.waveId);
  await typeAndAcknowledge('input during real completion settlement','release');
  const complete=await until(s=>s.derives.pendingCount===0,'completed');
  assert.equal(complete.derives.validCount,lanes);
  assert.equal(complete.derives.failedCount,0);
  assert.equal((await app.find('stress-echo')).props.text,'input during real completion settlement');
  for (let lane=0;lane<lanes;lane++) {
    assert.equal(complete.resources['r'+lane].wave,second.derives.waveId);
    assert.equal(complete.resources['r'+lane].ok,true);
  }
  const logs=await app.logs();
  const dropped=logs.lines.filter(line=>String(line).includes('dropped: no such request')).length;
  assert.ok(dropped>0,'old replies should be logged as dropped');
  await app.tap('errors-50');
  await app.tap('start-wave');
  await until(s=>s.derives.pendingCount===lanes,'mixed pending');
  if (host==='web') await app.tap('release-wave');
  else await release((await app.state()).derives.waveId);
  const mixed=await until(s=>s.derives.pendingCount===0,'mixed complete');
  const failed=Array.from({length:lanes},(_,lane)=>lane).filter(lane=>(lane*37)%100<50).length;
  assert.equal(mixed.derives.failedCount,failed);
  assert.equal(mixed.derives.validCount,lanes-failed);
  const report={passed:true,host,os:platform(),lanes,execution:host==='linux'&&platform()!=='linux'?'Linux host executed on '+platform()+'; not actual Linux':host,acknowledgements,measurement:'agent command to echoed Contract state acknowledgement; not hardware key latency, display presentation, or FPS',fixtureRelease:{old:oldRelease,fresh:freshRelease},firstWave:firstId,remountedWave:second.derives.waveId,validAfterRemount:complete.derives.validCount,mixedValid:mixed.derives.validCount,mixedFailed:mixed.derives.failedCount,staleDropLinesInBoundedJournal:dropped,echo:complete.slots.note,hostErrors:logs.host.filter(x=>x.includes('exception:'))};
  await app.screenshot(resolve(output,`${host}-check.png`));
  writeFileSync(resolve(output,`${host}-check.json`),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
} finally {
  try { await release(); }
  finally { await app.close(); }
}
