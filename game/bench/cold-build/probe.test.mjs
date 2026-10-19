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
