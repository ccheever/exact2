import {test, expect} from 'bun:test';
import {existsSync, mkdtempSync, readFileSync, writeFileSync, rmSync} from 'node:fs';
import {resolve} from 'node:path';

test('R15 real fast and shipping game proofs refuse profile drift without changing pins', async () => {
  const {createGame} = await import('./new.mjs');
  const root = process.env.EXACT_PROFILE_ORACLE_DIR ?? mkdtempSync(resolve(import.meta.dir, '../target/profile-oracle-'));
  const app = resolve(root, 'profile-drift');
  const run = async (args, label, extra = {}) => {
    const logPath=resolve(root,label+'.log'),errors=resolve(root,label+'.stderr.log');
    writeFileSync(logPath,'');writeFileSync(errors,'');
    const log = Bun.file(logPath);
    const child = Bun.spawn([process.execPath, ...args], {cwd:resolve(import.meta.dir,'..'),
      env:{...process.env, PATH:`${process.env.HOME}/.cargo/bin:${process.env.PATH}`, ...extra}, stdout:log, stderr:Bun.file(errors)});
    const status = await child.exited;
    return {status, output:readFileSync(logPath,'utf8')+readFileSync(errors,'utf8')};
  };
  try {
    if (!existsSync(app)) createGame(app);
    writeFileSync(resolve(app,'logic/Cargo.toml'), `[package]\nname = "profile-drift-logic"\nversion = "0.1.0"\nedition = "2021"\nworkspace = "../.shells"\n[dependencies]\nexact-game.workspace = true\n`);
    // Both profiles disable debug_assertions. Cargo's actual OPT_LEVEL is 1 for
    // gpu-dev and z for shipping web; a build script exposes precisely that input.
    writeFileSync(resolve(app,'logic/build.rs'), 'fn main() { println!("cargo:rustc-env=PROOF_OPT_LEVEL={}", std::env::var("OPT_LEVEL").unwrap()); }');
    const logic = resolve(app,'logic/src/lib.rs');
    if (!readFileSync(logic,'utf8').includes('struct ProfileFlag')) writeFileSync(logic, readFileSync(logic,'utf8').replace('        w.reseed(args.seed);', '        w.insert_resource(ProfileFlag(env!("PROOF_OPT_LEVEL") == "z"));\n        w.reseed(args.seed);') + '\n#[derive(Default, Resource)]\nstruct ProfileFlag(bool);\n');
    const out = resolve(root,'observed');
    const fast = await run([resolve(app,'proof.mjs'),'web'], 'fast', {EXACT_PROOF_COMPARE:'0', EXACT_PROOF_REPIN:'1', EXACT_PROOF_OUT:out, EXACT_GAME_PROOF_PROFILE:'gpu-dev'});
    expect({status:fast.status,tail:fast.status ? fast.output.slice(-4000) : ''}).toEqual({status:0,tail:''});
    const fastPins = JSON.parse(readFileSync(resolve(out,'summary.json'),'utf8')).pins;
    const shipping = await run([resolve(app,'proof.mjs'),'web'], 'shipping', {EXACT_PROOF_COMPARE:'0', EXACT_PROOF_REPIN:'1', EXACT_PROOF_OUT:out, EXACT_GAME_PROOF_PROFILE:'release'});
    expect({status:shipping.status,tail:shipping.status ? shipping.output.slice(-4000) : ''}).toEqual({status:0,tail:''});
    const shippingPins = JSON.parse(readFileSync(resolve(out,'summary.json'),'utf8')).pins;
    expect(shippingPins.ticks).not.toEqual(fastPins.ticks);
    const pinFile=resolve(app,'pins.json'), before=JSON.stringify({...fastPins,game:'profile-drift'});
    writeFileSync(pinFile,before);
    const repin=await run([resolve(import.meta.dir,'prove.mjs'),app,'--repin','--hosts','linux,web','--reason','negative control: Cargo OPT_LEVEL changes saved ProfileFlag only in shipping web'], 'repin');
    expect(repin.status).not.toBe(0);
    expect(repin.output).toContain('web release proof missing, failed, or disagrees with gpu-dev');
    expect(readFileSync(pinFile,'utf8')).toBe(before);
  } finally {
    // Keep failed evidence under target; a passing run owns no remaining fixture.
    if (process.env.EXACT_KEEP_PROFILE_ORACLE !== '1') rmSync(root,{recursive:true,force:true});
  }
}, 3_600_000);

