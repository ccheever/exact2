import {test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, writeFileSync, readFileSync, readdirSync, statSync, utimesSync, rmSync, existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {gameCrateKey, keptGameCrates, registryPackages} from './crates.mjs';

const root = resolve(import.meta.dir,'../..');
const workspace = () => {
  const base=resolve(root,'target/game-cache-tests'); mkdirSync(base,{recursive:true});
  return mkdtempSync(resolve(base,'case-'));
};
const facts = {channel:'1.97.0',profile:'gpu-dev',triple:'aarch64-apple-darwin',rustc:'compiler A',flags:['-C','llvm-args=-fp-contract=off']};

test('compiler, channel, profile, target and flags cannot share a namespace', () => {
  const key=gameCrateKey(facts);
  for(const change of [{channel:'nightly'},{profile:'release'},{triple:'wasm32-unknown-unknown'},
    {rustc:'compiler B'},{flags:['-C','opt-level=2']},{env:{RUSTFLAGS:''}},
    {env:{CARGO_ENCODED_RUSTFLAGS:'-C\u001fopt-level=2'}},{env:{CARGO_PROFILE_GPU_DEV_OPT_LEVEL:'2'}},{env:{CARGO_BUILD_RUSTFLAGS:'-Copt-level=2'}},
    {env:{CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS:'-Copt-level=2'}}])
    expect(gameCrateKey({...facts,...change})).not.toBe(key);
  expect(gameCrateKey({...facts,env:{EXACT_APP_DIR:'/another/game'}})).toBe(key);
});

test('registry-only eligibility excludes path crates, git crates and all their dependants', () => {
  const dir=workspace(), lock=resolve(dir,'Cargo.lock');
  try {
    writeFileSync(lock, `[[package]]\nname="leaf"\nversion="1.0.0"\nsource="registry+example"\n[[package]]\nname="engine"\nversion="1.0.0"\n[[package]]\nname="above"\nversion="1.0.0"\nsource="registry+example"\ndependencies=["engine"]\n[[package]]\nname="git-leaf"\nversion="1.0.0"\nsource="git+example"\n[[package]]\nname="above-git"\nversion="1.0.0"\nsource="registry+example"\ndependencies=["git-leaf"]\n`);
    expect([...registryPackages(lock)]).toEqual(['leaf']);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

const hash = n => n.toString(16).padStart(16,'0');
function unit(target, name, n, profile='gpu-dev') {
  const id=`${name}-${hash(n)}`, dir=resolve(target,profile);
  mkdirSync(resolve(dir,'.fingerprint',id),{recursive:true});
  mkdirSync(resolve(dir,'deps'),{recursive:true});
  writeFileSync(resolve(dir,'.fingerprint',id,'invoked.timestamp'),'');
  writeFileSync(resolve(dir,'deps',`lib${id}.rlib`),'artifact');
  const dep=resolve(dir,'deps',`${id}.d`);
  writeFileSync(dep,`${target}/${profile}/deps/lib${id}.rlib: /registry/${name}/lib.rs\n`);
  utimesSync(dep,1700000000,1700000000);
  return {id,dep};
}

test('a fresh target takes one coherent generation, preserves dep-info times and refuses mixing', () => {
  const dir=workspace(), lock=resolve(dir,'Cargo.lock'), directory=resolve(dir,'cache');
  const a=resolve(dir,'a'), b=resolve(dir,'b'), independent=resolve(dir,'independent');
  try {
    writeFileSync(lock,['leaf','second','third'].map(name=>`[[package]]\nname="${name}"\nversion="1.0.0"\nsource="registry+example"\n`).join('')+'[[package]]\nname="engine"\nversion="1.0.0"\n');
    const cache=keptGameCrates({root,env:process.env,...facts,directory});
    const original=unit(a,'leaf',1); unit(a,'engine',2);
    cache.keep(a,'native',lock);
    cache.take(b,'native');
    const dep=resolve(b,'gpu-dev/deps',`${original.id}.d`);
    expect(readFileSync(dep,'utf8')).toContain(`${b}/gpu-dev/deps/`);
    expect(readFileSync(dep,'utf8')).not.toContain(a+'/');
    expect(statSync(dep).mtimeMs).toBe(statSync(original.dep).mtimeMs);
    expect(readdirSync(resolve(b,'gpu-dev/.fingerprint'))).toEqual([original.id]);
    unit(independent,'third',3); cache.keep(independent,'native',lock);
    expect(readdirSync(resolve(cache.store,'native')).filter(n=>n.startsWith('gen-'))).toHaveLength(1);
    unit(b,'second',4); cache.keep(b,'native',lock);
    expect(readdirSync(resolve(cache.store,'native')).filter(n=>n.startsWith('gen-'))).toHaveLength(2);
    unit(b,'third',6); cache.keep(b,'native',lock);
    expect(readdirSync(resolve(cache.store,'native')).filter(n=>n.startsWith('gen-'))).toHaveLength(2);
    const populated=resolve(dir,'populated'); unit(populated,'engine',5);
    cache.take(populated,'native');
    expect(existsSync(resolve(populated,'gpu-dev/deps',`lib${original.id}.rlib`))).toBe(false);
    // A changed profile is a cache miss, even though the compiler and triple agree.
    const other=keptGameCrates({root,env:process.env,...facts,profile:'release',directory});
    const miss=resolve(dir,'miss'); mkdirSync(miss,{recursive:true});other.take(miss,'native');
    expect(existsSync(resolve(miss,'release'))).toBe(false);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
