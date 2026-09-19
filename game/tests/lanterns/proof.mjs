#!/usr/bin/env bun
// Every host consumes the same ordinary-input checkpoints and runs the real
// compiled branch for 120 ticks. Full saves, not just hashes, must match the
// invariant-checked three-mode production Sim oracle at every observation.
import {spawnSync} from 'node:child_process';
import {readFileSync, mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {proof} from '../../proof.mjs';
import {resolveApp} from '../../../scripts/app.mjs';

await proof(import.meta, async ({open, check, pin, pinSave, out}) => {
  const app = resolveApp(import.meta.dir), oracle = resolve(out, 'oracle');
  mkdirSync(oracle,{recursive:true});
  const result = spawnSync('cargo',['test','--locked','--offline','--manifest-path',resolve(app.workspace,'Cargo.toml'),'-p','lanterns-evidence-logic','--test','difficult_moment','--','--nocapture'], {
    cwd:app.dir,env:{...process.env,CARGO_TARGET_DIR:app.target,EXACT_I3_OUT:oracle},encoding:'utf8',maxBuffer:16*1024*1024,
  });
  check('all I3 invariants and complete checkpoint/continuation bytes agree in continuous, Save and FreshGame',result.status===0,result.status===0?undefined:result.stdout+result.stderr);
  if (result.status!==0) throw new Error('I3 reference assertions failed');
  const report = JSON.parse(readFileSync(resolve(oracle,'all-continuations.json'),'utf8'));
  for (const moment of ['apex','moving-crate']) for (const [variant,name] of ['v1','placement','physics','clip','appearance'].entries()) {
    const row = moment==='apex'?report[name]:report.supplementary_moving_crate[name];
    const initial = resolve(oracle,moment==='apex'?'difficult-moment.sim':'moving-crate.sim');
    const s = await open({app:app.dir,plan:resolve(oracle,`${variant}.plan`),world:initial,worldMode:'carry',fresh:true});
    try {
      for (let offset=0;offset<=120;offset++) {
        if(offset) await s.world('world').ticks(1);
        const state = (await s.state('world',{world:true})).world;
        const expected = offset?row.trajectory[offset-1]:row.restored;
        const key = `${name}/${moment}/${expected.tick}`;
        check(`${key} complete simulation hash`,state.hash===expected.hash && state.tick===expected.tick);
        pin(expected.tick,state,key);
        const save=resolve(out,`${variant}-${moment}-${offset}.world`);
        await s.world('world').save(save);
        const actual=readFileSync(save), wanted=readFileSync(resolve(oracle,`${row.save_key}-${offset}.sim`));
        const same=actual.equals(wanted);
        check(`${key} full save including queued controls`,same,same?undefined:{actual:actual.length,expected:wanted.length,firstDifference:actual.findIndex((b,i)=>b!==wanted[i])});
        pinSave(key,save);
        if(!same) throw new Error(`${key}: host continuation differs from invariant-checked production Sim`);
      }
    } finally {await s.close();}
  }
});
