import {spawnSync} from 'node:child_process';
import {test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, readFileSync, writeFileSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';
import {inventory, snapshot, validateSnapshot, validateProof, reusableLeg, digest, writeJSON} from './proof-snapshot.mjs';

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

test('driver and proof ignore proof-only and status edits but admit logic changes', async () => {
  const {webInputDigests} = await import('../scripts/agent-launch.mjs');
  const sdk = resolve(import.meta.dir,'..');
  const app = mkdtempSync(resolve(sdk,'target/input-agreement-'));
  mkdirSync(resolve(app,'logic/src'), {recursive:true});
  writeFileSync(resolve(app,'logic/src/lib.rs'), 'before');
  writeFileSync(resolve(app,'proof.mjs'), 'export const proof = 1;');
  const info = {dir:app, manifest:{game:{}}};
  try {
    const proof = inventory(sdk,app), driver = webInputDigests(info,false);
    writeFileSync(resolve(app,'proof.mjs'), 'export const proof = 2;');
    mkdirSync(resolve(app,'.exact'));
    writeFileSync(resolve(app,'.exact/status.json'), '{"state":"running"}');
    expect(inventory(sdk,app)).toEqual(proof);
    expect(webInputDigests(info,false)).toEqual(driver);
    writeFileSync(resolve(app,'logic/src/lib.rs'), 'after');
    expect(inventory(sdk,app)).not.toEqual(proof);
    expect(webInputDigests(info,false)).not.toEqual(driver);
  } finally { rmSync(app,{recursive:true,force:true}); }
}, 60000);


test('proof closure and explicit data are checked independently from bake inputs', () => {
  const sdk = mkdtempSync(resolve(import.meta.dir,'../target/proof-closure-'));
  spawnSync('git',['init','-q',sdk]);
  const app = resolve(sdk,'game/games/sample'); mkdirSync(resolve(app,'logic/tests'),{recursive:true});
  const helper = resolve(sdk,'game/proof.mjs');
  writeFileSync(helper,'export const value = 1;');
  writeFileSync(resolve(app,'logic/Cargo.toml'),'[package]');
  writeFileSync(resolve(app,'logic/tests/pin.json'),'[1]');
  writeFileSync(resolve(app,'pins.json'),'{}');
  writeFileSync(resolve(app,'proof.mjs'),`import {value} from '../../proof.mjs';
export const proofData = ["logic/tests/pin.json"];`);
  try {
    const record = snapshot(sdk,app,resolve(app,'artifacts/prove/one'));
    expect(readFileSync(resolve(record.src,'logic/tests/pin.json'),'utf8')).toBe('[1]');
    writeFileSync(helper,'export const value = 2;');
    expect(()=>validateProof(record,sdk,app)).toThrow('sdk/game/proof.mjs changed');
    writeFileSync(helper,'export const value = 1;');
    writeFileSync(resolve(record.src,'proof.mjs'),'changed');
    expect(()=>validateProof(record,sdk,app)).toThrow('snapshot proof.mjs changed');
  } finally { rmSync(sdk,{recursive:true,force:true}); }
});

test('snapshot run paths do not enter execution input digests and app status is an input', async () => {
  const {proofInputs} = await import('./proof.mjs');
  const sdk = mkdtempSync(resolve(import.meta.dir,'../target/stable-proof-'));
  spawnSync('git',['init','-q',sdk]);
  const app = resolve(sdk,'sample'); mkdirSync(app);
  writeFileSync(resolve(app,'proof.mjs'),'export const proof = 1;');
  writeFileSync(resolve(app,'pins.json'),'{}');
  writeFileSync(resolve(app,'status.json'),'1');
  try {
    const first = snapshot(sdk,app,resolve(app,'artifacts/prove/one'));
    const second = snapshot(sdk,app,resolve(app,'artifacts/prove/two'));
    expect(proofInputs(sdk,first.src).all).toBe(proofInputs(sdk,second.src).all);
    const before = inventory(sdk,app); writeFileSync(resolve(app,'status.json'),'2');
    expect(inventory(sdk,app)).not.toEqual(before);
  } finally { rmSync(sdk,{recursive:true,force:true}); }
});
