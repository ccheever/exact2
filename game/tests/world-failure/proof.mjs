#!/usr/bin/env bun
// A real wasm failure through the existing browser carrier, independent of Tally.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {resolve} from 'node:path';
import {open} from '../../../scripts/agent.mjs';
import {closeFilesystemReader} from '../../../scripts/filesystem.mjs';
const root=resolve(import.meta.dir,'../../..');
Object.assign(process.env,{EXACT_APP_DIR:import.meta.dir,EXACT_WEB_DIST:resolve(import.meta.dir,'dist')});
const built=spawnSync(process.execPath,[resolve(root,'host/web/build.mjs')],{env:process.env,stdio:'inherit'});
if(built.status!==0)process.exit(built.status??1);
const children=[];
const s=await open({host:'web',onProcess:child=>children.push(child)});
try {
  assert.equal((await s.state()).world[0].tick,1);
  const target=await s.target('world');
  await assert.rejects(s.op({op:'clock',...target,world:true,ticks:216000}),/fixture tick refused/);
  const failed=(await s.state()).world[0];
  assert.equal(failed.tick,2);assert.equal(failed.failed,true);assert.equal(failed.ready,false);
  assert.match(failed.error,/tick 3/);
  assert.ok((await s.tree('world')).entities);
  assert.match(JSON.stringify(await s.logs()),/fixture tick refused/);
  await s.tap('alive');
  assert.ok((await s.tree()).nodes.some(n=>n.props?.text==='Clicks 1'),'negative control: page actions still execute after the wasm failure');
  assert.equal((await s.state()).world[0].tick,2);
  console.log('PASS: tick 3 returned an error; state/tree/logs and page action remain alive (10 assertions)');
} finally {
  closeFilesystemReader();
  await s.close(); // SIGKILL the recorded browser process group, then await it.
  assert.ok(children.every(c=>c.exitCode!==null || c.signalCode!==null));
  console.log('leaked browser PIDs: none; recorded:',children.map(c=>c.pid));
}
