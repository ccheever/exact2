import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {readFileSync, mkdtempSync, openSync, ftruncateSync, closeSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {open} from '../../../scripts/agent.mjs';
const source = readFileSync(process.env.E2B_DEV_SOURCE || new URL('../dev.mjs', import.meta.url), 'utf8');
function scheduler() {
  const legacy = !source.includes('function drainBuilds()');
  const functions = legacy ? source.slice(source.indexOf('function produceRust()'), source.indexOf('  if (!rustChild)')) + 'calls.push(["rust"]);rustActive=true;}'
    : source.slice(source.indexOf('function produceRust()'), source.indexOf('function produceRustNow()'));
  return new Function(`
    let building=false, rustActive=false, buildPending=false, rustPending=false, rustDirty=false, again=false, changed=new Set(), calls=[];
    const gpuOnly = files => files.length && files.every(f=>f==='gpu.rs');
    const produceGpu = files => { building=true; calls.push(['gpu',files]); };
    const rebuildNow = files => { building=true; calls.push(['full',files]); };
    const produceRustNow = () => { rustActive=true; calls.push(['rust']); };
    ${legacy ? 'function rebuild(){if(building){again=true;return;}const files=[...changed];changed.clear();if(gpuOnly(files))produceGpu(files);else rebuildNow(files);}function drainBuilds(){rebuild();}' : ''}
    ${functions}
    return {calls, rust:produceRust, change(file){changed.add(file);buildPending=true;drainBuilds();},
      finish(){building=false;rustActive=false;${legacy ? 'if(again||changed.size){again=false;rebuild();}' : 'drainBuilds();'}}};
  `)();
}
test('a Rust request during GPU build stays Rust, with no empty full rebuild', () => {
  const s=scheduler(); s.change('gpu.rs'); s.rust();
  assert.equal(s.calls.length,1); s.finish();
  assert.deepEqual(s.calls,[['gpu',['gpu.rs']],['rust']]); s.finish(); assert.equal(s.calls.length,2);
});
test('GPU and full builds wait for the active resident Rust producer', () => {
  const s=scheduler(); s.rust(); s.change('gpu.rs'); assert.equal(s.calls.length,1);
  s.finish(); assert.equal(s.calls[1][0],'gpu'); s.change('app.rs'); s.finish();
  assert.deepEqual(s.calls[2],['full',['app.rs']]);
});
test('compiler cleanup kills the recorded GPU cargo group too', () => {
  const cleanup=source.slice(source.indexOf('const killCompiler = () => {'),source.indexOf('\nstartCompiler();'));
  const killed=[];
  new Function('process', `let gpuBuildChild={pid:123},manualTypescript,rustSourceWatch,rustOutputWatch,rustHeartbeat,rustActive,rustChild,moduleWatch,moduleTimer,moduleRun=0,dev,moduleStage; ${cleanup}; killCompiler();`)
    ({kill:(...args)=>killed.push(args)});
  assert.deepEqual(killed,[[-123,'SIGKILL']]);
});
test('failed Cargo metadata refuses startup before choosing a dev binary', () => {
  const body=source.slice(source.indexOf('  const metadata = spawnSync(\'cargo\''),source.indexOf('  const me = dev;'));
  let spawned=false;
  assert.throws(()=>new Function('spawnSync','spawn',`const app={workspace:'.',crate:()=> 'fixture-web'},buildEnv={},root='.',source='',plan='';let dev;${body}`)
    (()=>({status:1,stderr:'metadata fixture failure'}),()=>{spawned=true;}),/cargo metadata failed.*metadata fixture failure/);
  assert.equal(spawned,false);
});
test('world file size is refused before reading or launching either host', async () => {
  const dir=mkdtempSync(join(tmpdir(),'exact-world-cap-')), path=join(dir,'oversized.world');
  const fd=openSync(path,'w'); ftruncateSync(fd,256*1024*1024+1); closeSync(fd);
  try { for (const host of ['web','macos']) await assert.rejects(open({host,world:path}), /world carrier exceeds 256 MiB limit/); }
  finally { rmSync(dir,{recursive:true,force:true}); }
});
