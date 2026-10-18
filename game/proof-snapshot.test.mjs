import {spawnSync} from 'node:child_process';
import {test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';
import {inventory, snapshot, validateSnapshot, reusableLeg, digest, writeJSON} from './proof-snapshot.mjs';

test('snapshots isolate edits; continuation names changed inputs and corrupt receipts', () => {
  mkdirSync(resolve(import.meta.dir, '../target'), {recursive:true});
  const sdk = mkdtempSync(resolve(import.meta.dir, '../target/snapshot-test-'));
  spawnSync('git', ['init', '-q', sdk]);
  const app = resolve(sdk, 'game'), root = resolve(app, 'artifacts/prove/run-one');
  const write = (file, value) => {mkdirSync(resolve(app,file,'..'), {recursive:true}); writeFileSync(resolve(app,file), value);};
  try {
    write('logic/src/lib.rs', 'original'); write('proof.mjs', 'export const proof = 1;');
    write('pins.json', '{}'); write('tools/probe.mjs', 'probe');
    const run = snapshot(sdk, app, root);
    expect(inventory(sdk, run.src)['app/logic/src/lib.rs']).toBe(run.inputs['app/logic/src/lib.rs']);
    write('tools/probe.mjs', 'edited probe');
    validateSnapshot(run, sdk, app);
    write('logic/src/lib.rs', 'edited during leg');
    expect(readFileSync(resolve(run.src,'logic/src/lib.rs'),'utf8')).toBe('original');
    expect(() => validateSnapshot(run,sdk,app)).toThrow('app/logic/src/lib.rs changed');
    write('logic/src/lib.rs', 'original');
    validateSnapshot(run,sdk,app);
    writeJSON(resolve(root,'summary.json'), {status:'UNVERIFIED', failures:[], pins:{ticks:{1:'x'}}});
    const leg = {state:'complete', digest:run.digest, exitCode:3, summaryPath:'summary.json', summaryDigest:digest(readFileSync(resolve(root,'summary.json')))};
    expect(reusableLeg(leg, root, run.digest).pins.ticks[1]).toBe('x');
    expect(reusableLeg({...leg,state:'running'},root,run.digest)).toBeNull();
    writeJSON(resolve(root,'summary.json'), {status:'PASS', failures:[]});
    expect(() => reusableLeg(leg,root,run.digest)).toThrow('corrupt receipt');
  } finally { rmSync(sdk,{recursive:true,force:true}); }
});
