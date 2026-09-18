import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {artifactDigest, closeSessions} from './proof.mjs';
import {typeArguments, typeFor, browserKey, render} from '../scripts/agent.mjs';

test('held keys release the original carrier and retain partial failure steps', async () => {
  const calls = [], node = {id:17};
  let clockRan = false;
  const carrier = {input:async (id, kind, options) => {
    calls.push([id, kind, options.phase]);
    if (options.phase === 'up') expect(clockRan).toBe(true);
    return {delivery:'platform'};
  }};
  let failure;
  try {
    await typeFor({node, target:'world', options:{key:'KeyW',for:1500}, carrier,
      clock:async () => { clockRan = true; throw new Error('world disappeared'); }, tagged:r => r});
  } catch (error) { failure = error; }
  expect(calls).toEqual([[17,'key','down'],[17,'key','up']]);
  expect(failure.message).toBe('world disappeared');
  expect(failure.steps.map(s => s.op)).toEqual(['type','clock','type']);
  expect(render('type', {steps:failure.steps})).toContain('ERROR world disappeared');
});
test('a failed down still releases; a failed up keeps the original error and both failures', async () => {
  const calls = [];
  const carrier = {input:async (_, __, {phase}) => {calls.push(phase); throw new Error(phase);}};
  let failure;
  try { await typeFor({node:{id:1},target:'world',options:{key:'KeyW',for:1},carrier,clock:()=>{throw new Error('unexpected clock');},tagged:r=>r}); }
  catch(error) {failure=error;}
  expect(calls).toEqual(['down','up']);
  expect(failure.message).toBe('down');
  expect(failure.steps.map(s=>s.error)).toEqual(['down','up']);
});
test('CLI held key syntax cannot capture an ordinary text suffix', () => {
  expect(typeArguments(['world','key','KeyW','for','1500'])).toEqual(['world',{key:'KeyW',for:1500}]);
  expect(typeArguments(['editor','hello','for','100'])).toEqual(['editor','hello for 100']);
  expect(typeArguments(['world','KeyW','for','100'])).toEqual(['world','KeyW for 100']);
});
test('proof receipts bind the web manifest and the actual app executable', () => {
  const dir=mkdtempSync(resolve(tmpdir(),'g1b-receipt-')), bundle=resolve(dir,'Game.app');
  try {
    expect(artifactDigest('web',dir)).toBe(null);
    writeFileSync(resolve(dir,'exact.json'),'one');
    const web=artifactDigest('web',dir);
    writeFileSync(resolve(dir,'exact.json'),'two');
    expect(artifactDigest('web',dir)).not.toBe(web);
    writeFileSync(resolve(dir,'built-macos'),'');
    expect(artifactDigest('macos',dir,{bundle})).toBe(null);
    mkdirSync(resolve(bundle,'Contents/MacOS'),{recursive:true});
    const exe=resolve(bundle,'Contents/MacOS/ExactMac');
    writeFileSync(exe,'native one'); const mac=artifactDigest('macos',dir,{bundle});
    writeFileSync(exe,'native two'); expect(artifactDigest('macos',dir,{bundle})).not.toBe(mac);
    const binary=resolve(dir,'standalone');
    writeFileSync(binary,'carrier one'); const carrier=artifactDigest('macos',dir,{bundle,binary});
    writeFileSync(binary,'carrier two'); expect(artifactDigest('macos',dir,{bundle,binary})).not.toBe(carrier);
    rmSync(binary); expect(artifactDigest('macos',dir,{bundle,binary})).toBe(null);
    rmSync(bundle,{recursive:true}); expect(artifactDigest('macos',dir,{bundle})).toBe(null);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
test('inventory failure clears the timer before unconditional session cleanup', async () => {
  let polls=0; const monitor=setInterval(()=>polls++,1), calls=[];
  await closeSessions(monitor,()=>{throw new Error('ps failed');},[
    {close:async()=>{calls.push('first');throw new Error('close failed');}},
    {close:async()=>{await new Promise(r=>setTimeout(r,15));calls.push('second');}},
  ],(...args)=>calls.push(args[0]));
  expect(polls).toBe(0);
  expect(calls).toEqual(['first','session cleanup','second']);
});

test('held key release survives removal of its canvas', async () => {
  let canvas = true, down = false;
  const carrier = {input: async (id, _, opts) => browserKey({id, opts,
    evaluate: async () => { if (!canvas) throw new Error('canvas removed'); return true; },
    ask: async () => { if (!canvas) throw new Error('canvas removed'); return {ok:true}; },
    call: async (_, event) => { down=event.type==='keyDown'; }, frame: async () => {},
  })};
  await typeFor({node:{id:17},target:'world',options:{key:'KeyW',for:10},carrier,
    clock:async () => {canvas=false; return {};},tagged:r=>r});
  expect(down).toBe(false);
});

test('native receipt changes with game dylibs and embedded plan/assets', () => {
  const dir=mkdtempSync(resolve(tmpdir(),'g1c-receipt-')), bundle=resolve(dir,'Game.app');
  try {
    mkdirSync(resolve(bundle,'Contents/MacOS'),{recursive:true});
    mkdirSync(resolve(bundle,'Contents/Resources'),{recursive:true});
    writeFileSync(resolve(bundle,'Contents/MacOS/ExactMac'),'executable');
    for (const file of ['Contents/MacOS/libexact_gpu.dylib','Contents/MacOS/libexact_web.dylib','Contents/Resources/app.plan','Contents/Resources/texture.bin']) {
      writeFileSync(resolve(bundle,file),'before'); const before=artifactDigest('macos',dir,{bundle});
      writeFileSync(resolve(bundle,file),'after'); expect(artifactDigest('macos',dir,{bundle})).not.toBe(before);
    }
  } finally {rmSync(dir,{recursive:true,force:true});}
});
