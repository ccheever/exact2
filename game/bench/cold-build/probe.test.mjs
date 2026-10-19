import {test, expect} from 'bun:test';
import {spawnSync} from 'node:child_process';
import {cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, utimesSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {homedir} from 'node:os';

test('the probe stops a real Cargo rebuild of a copied completed fingerprint', () => {
  const dir=mkdtempSync('/tmp/exact-d2-probe-test-'), probe=resolve(import.meta.dir,'probe.py');
  try {
    mkdirSync(resolve(dir,'src'));
    writeFileSync(resolve(dir,'Cargo.toml'),'[workspace]\n[package]\nname="probe-observer"\nversion="0.1.0"\nedition="2021"\n');
    const source=resolve(dir,'src/lib.rs'); writeFileSync(source,'pub fn value() -> u32 { 1 }\n');
    const channel=Bun.TOML.parse(readFileSync(resolve(import.meta.dir,'../../../rust-toolchain.toml'),'utf8')).toolchain.channel;
    const env={...process.env,TMPDIR:'/tmp',PATH:`${homedir()}/.cargo/bin:${process.env.PATH}`,RUSTUP_TOOLCHAIN:channel};
    const run=(name,extra={})=>spawnSync('python3',[probe,'cargo','build','--offline'],{cwd:dir,env:{...env,D2_LOG:resolve(dir,'logs',name),CARGO_TARGET_DIR:resolve(dir,name),CARGO_BUILD_BUILD_DIR:resolve(dir,name),...extra},encoding:'utf8'});
    const first=run('cold'); expect(first.status).toBe(0);
    const fingerprints=resolve(dir,'cold/debug/.fingerprint');
    const units=readdirSync(fingerprints).filter(name=>name.startsWith('probe-observer-'));
    expect(units).toHaveLength(1);
    expect(statSync(resolve(fingerprints,units[0],'lib-probe_observer')).size).toBeGreaterThan(0);
    cpSync(resolve(dir,'cold'),resolve(dir,'copied'),{recursive:true,preserveTimestamps:true});
    writeFileSync(resolve(dir,'hashes.json'),JSON.stringify(units.map(name=>name.split('-').at(-1))));
    // A real Cargo invalidation, rather than a direct invocation of the guard.
    writeFileSync(source,'pub fn value() -> u32 { 2 }\n');
    const later=statSync(resolve(fingerprints,units[0],'invoked.timestamp')).mtimeMs/1000+2;
    utimesSync(source,later,later);
    const rejected=run('copied',{D2_STOP_ON_REBUILD:resolve(dir,'hashes.json')});
    expect(rejected.status).not.toBe(0);
    const events=readFileSync(resolve(dir,'logs/copied/units.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
    expect(events.some(row=>row.kind==='cache-rejected' && row.code===86)).toBe(true);
  } finally {rmSync(dir,{recursive:true,force:true});}
});


test('cargo rustc reaches Cargo and records both compiler and command events', () => {
  const dir=mkdtempSync('/tmp/exact-d2-cargo-rustc-'), probe=resolve(import.meta.dir,'probe.py');
  try {
    mkdirSync(resolve(dir,'src'));
    writeFileSync(resolve(dir,'Cargo.toml'),'[workspace]\n[package]\nname="probe-dispatch"\nversion="0.1.0"\nedition="2021"\n');
    writeFileSync(resolve(dir,'src/lib.rs'),'pub const VALUE: u32 = 1;\n');
    const channel=Bun.TOML.parse(readFileSync(resolve(import.meta.dir,'../../../rust-toolchain.toml'),'utf8')).toolchain.channel;
    const log=resolve(dir,'logs');
    const result=spawnSync('python3',[probe,'cargo','rustc','--offline','--','-C','opt-level=1'],{cwd:dir,encoding:'utf8',
      env:{...process.env,TMPDIR:'/tmp',PATH:`${homedir()}/.cargo/bin:${process.env.PATH}`,RUSTUP_TOOLCHAIN:channel,D2_LOG:log,CARGO_TARGET_DIR:resolve(dir,'target'),CARGO_BUILD_BUILD_DIR:resolve(dir,'target')}});
    expect(result.status).toBe(0);
    const events=readFileSync(resolve(log,'units.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
    const cargo=events.find(row=>row.kind==='cargo' && row.args[0]==='rustc');
    expect(cargo?.code).toBe(0);
    // `--timings` is Cargo's; what follows the first `--` is rustc's.
    expect(cargo.args.indexOf('--timings')).toBeLessThan(cargo.args.indexOf('--'));
    expect(events.some(row=>row.kind==='rustc' && row.args.includes('--crate-name') && row.args.includes('opt-level=1') && row.code===0)).toBe(true);
    expect(events.some(row=>row.kind==='rustc' && row.args.includes('--crate-name') && row.code===0)).toBe(true);
    expect(events.some(row=>row.kind==='command' && row.code===0)).toBe(true);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('the phase audit refuses failed or incomplete sampled commands', () => {
  const dir=mkdtempSync('/tmp/exact-d2-audit-');
  try {
    writeFileSync(resolve(dir,'cargo-timing-fixture.html'),'const UNIT_DATA = [];');
    writeFileSync(resolve(dir,'Cargo.lock'),'version = 4\npackage = []\n');
    const events=resolve(dir,'units.jsonl');
    const run=()=>spawnSync('python3',[resolve(import.meta.dir,'audit.py'),dir,'--phases',dir,'--lock',resolve(dir,'Cargo.lock')],{encoding:'utf8'});
    writeFileSync(events,JSON.stringify({kind:'command',code:0,start:0,end:1})+'\n');
    expect(run().status).toBe(0);
    for (const rows of [[],[{kind:'command',code:1,start:0,end:1}],[{kind:'command',code:0,start:0,end:1},{kind:'command',code:86,start:1,end:2}]]) {
      writeFileSync(events,rows.map(JSON.stringify).join('\n'));
      const refused=run(); expect(refused.status).not.toBe(0);
      expect(refused.stderr).toContain('phase samples require a completed command with exit code 0');
      expect(refused.stdout).not.toContain('eligibleFraction');
    }
  } finally {rmSync(dir,{recursive:true,force:true});}
});
