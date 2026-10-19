import {test, expect} from 'bun:test';
import {mkdirSync, mkdtempSync, writeFileSync, readFileSync, readdirSync, statSync, realpathSync, symlinkSync, utimesSync, rmSync, existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {gameCrateKey, keptGameCrates, registryPackages} from './crates.mjs';

const root = resolve(import.meta.dir,'../..');
const workspace = () => {
  const base=resolve(root,'target/game-cache-tests'); mkdirSync(base,{recursive:true});
  return mkdtempSync(resolve(base,'case-'));
};
const facts = {channel:'1.97.0',profile:'gpu-dev',triple:'aarch64-apple-darwin',rustc:'compiler A',strip:false,flags:['-C','llvm-args=-fp-contract=off']};

test('compiler, channel, profile, target and flags cannot share a namespace', () => {
  const key=gameCrateKey(facts);
  for(const change of [{channel:'nightly'},{profile:'release'},{triple:'wasm32-unknown-unknown'},
    {rustc:'compiler B'},{strip:true},{env:{RUSTUP_TOOLCHAIN:'nightly'}},{flags:['-C','opt-level=2']},{env:{RUSTFLAGS:''}},
    {env:{CARGO_ENCODED_RUSTFLAGS:'-C\u001fopt-level=2'}},{env:{CARGO_PROFILE_GPU_DEV_OPT_LEVEL:'2'}},{env:{CARGO_BUILD_RUSTFLAGS:'-Copt-level=2'}},
    {env:{CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS:'-Copt-level=2'}}])
    expect(gameCrateKey({...facts,...change})).not.toBe(key);
  expect(gameCrateKey({...facts,env:{EXACT_APP_DIR:'/another/game'}})).toBe(key);
});

test('registry-only eligibility excludes path crates, git crates and all their dependants', () => {
  const dir=workspace(), lock=resolve(dir,'Cargo.lock');
  try {
    writeFileSync(lock, `[[package]]\nname="leaf"\nversion="1.0.0"\nsource="registry+example"\n[[package]]\nname="engine"\nversion="1.0.0"\n[[package]]\nname="above"\nversion="1.0.0"\nsource="registry+example"\ndependencies=["engine 1.0.0"]\n[[package]]\nname="git-leaf"\nversion="1.0.0"\nsource="git+example"\n[[package]]\nname="above-git"\nversion="1.0.0"\nsource="registry+example"\ndependencies=["git-leaf"]\n`);
    writeFileSync(lock, readFileSync(lock,'utf8') + `[[package]]\nname="engine"\nversion="2.0.0"\nsource="registry+example"\n[[package]]\nname="above-registry-engine"\nversion="1.0.0"\nsource="registry+example"\ndependencies=["engine 2.0.0"]\n`);
    expect([...registryPackages(lock)]).toEqual(['leaf']);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

const hash = n => n.toString(16).padStart(16,'0');
const profile = `${facts.triple}/gpu-dev`;
const metadata = target => ({target_directory:target,build_directory:target});
function unit(target, name, n, profile=`${facts.triple}/gpu-dev`) {
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
    writeFileSync(lock,['leaf','second','third'].map(name=>`[[package]]\nname="${name}"\nversion="1.0.0"\nsource="registry+example"\n`).join('')+'[[package]]\nname="engine"\nversion="1.0.0"\n[[package]]\nname="engine"\nversion="2.0.0"\nsource="registry+example"\n[[package]]\nname="above"\nversion="1.0.0"\nsource="registry+example"\ndependencies=["engine 1.0.0"]\n');
    const cache=keptGameCrates({root,env:process.env,...facts,directory});
    const original=unit(a,'leaf',1); unit(a,'engine',2); unit(a,'above',7);
    cache.keep(metadata(a),'native',lock);
    cache.take(metadata(b),'native');
    const dep=resolve(b,profile,'deps',`${original.id}.d`);
    expect(readFileSync(dep,'utf8')).toContain(`${b}/${profile}/deps/`);
    expect(readFileSync(dep,'utf8')).not.toContain(a+'/');
    expect(statSync(dep).mtimeMs).toBe(statSync(original.dep).mtimeMs);
    expect(readdirSync(resolve(b,profile,'.fingerprint'))).toEqual([original.id]);
    unit(independent,'third',3); cache.keep(metadata(independent),'native',lock);
    expect(readdirSync(resolve(cache.store,'native')).filter(n=>n.startsWith('gen-'))).toHaveLength(1);
    unit(b,'second',4); cache.keep(metadata(b),'native',lock);
    expect(readdirSync(resolve(cache.store,'native')).filter(n=>n.startsWith('gen-'))).toHaveLength(2);
    unit(b,'third',6); cache.keep(metadata(b),'native',lock);
    expect(readdirSync(resolve(cache.store,'native')).filter(n=>n.startsWith('gen-'))).toHaveLength(2);
    const populated=resolve(dir,'populated'); unit(populated,'engine',5);
    cache.take(metadata(populated),'native');
    expect(existsSync(resolve(populated,profile,'deps',`lib${original.id}.rlib`))).toBe(false);
    // A changed profile is a cache miss, even though the compiler and triple agree.
    const other=keptGameCrates({root,env:process.env,...facts,profile:'release',directory});
    const miss=resolve(dir,'miss'); mkdirSync(miss,{recursive:true});other.take(metadata(miss),'native');
    expect(existsSync(resolve(miss,'release'))).toBe(false);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('host and target profiles seed independently without retaining an unrelated populated host', () => {
  const dir=workspace(), lock=resolve(dir,'Cargo.lock');
  try {
    writeFileSync(lock,'[[package]]\nname="leaf"\nversion="1.0.0"\nsource="registry+example"\n');
    const cache=keptGameCrates({root,env:process.env,...facts,directory:resolve(dir,'cache')});
    const a=resolve(dir,'a'), b=resolve(dir,'b'), c=resolve(dir,'c');
    const original=unit(a,'leaf',1); unit(a,'leaf',2,'gpu-dev');
    cache.keep(metadata(a),'native',lock);
    const host=unit(b,'leaf',3,'gpu-dev');
    cache.take(metadata(b),'native');
    expect(existsSync(resolve(b,profile,'deps',`lib${original.id}.rlib`))).toBe(true);
    expect(readdirSync(resolve(b,'gpu-dev/.fingerprint'))).toEqual([host.id]);
    unit(b,'leaf',4); cache.keep(metadata(b),'native',lock);
    cache.take(metadata(c),'native');
    expect(existsSync(resolve(c,'gpu-dev/deps',`lib${host.id}.rlib`))).toBe(false);
    const d=resolve(dir,'d'); unit(d,'leaf',5);
    cache.take(metadata(d),'native');
    expect(existsSync(resolve(d,'gpu-dev/deps','libleaf-'+hash(2)+'.rlib'))).toBe(true);
    expect(readdirSync(resolve(d,profile,'.fingerprint'))).toEqual(['leaf-'+hash(5)]);
    // A generation without host units must not adopt an existing private host.
    const sparse=keptGameCrates({root,env:process.env,...facts,directory:resolve(dir,'sparse-cache')});
    const e=resolve(dir,'e'), f=resolve(dir,'f'), g=resolve(dir,'g');
    unit(e,'leaf',6); sparse.keep(metadata(e),'native',lock);
    const privateHost=unit(f,'leaf',7,'gpu-dev'); sparse.take(metadata(f),'native');
    unit(f,'leaf',8); sparse.keep(metadata(f),'native',lock); sparse.take(metadata(g),'native');
    expect(existsSync(resolve(g,'gpu-dev/.fingerprint',privateHost.id))).toBe(false);

  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('Cargo metadata places registry intermediates correctly with only CARGO_TARGET_DIR overridden', () => {
  const dir=workspace(), source=resolve(dir,'source'), dest=resolve(dir,'dest'), lock=resolve(dir,'Cargo.lock');
  try {
    writeFileSync(lock,'[[package]]\nname="leaf"\nversion="1.0.0"\nsource="registry+example"\n');
    const resolved = workspace => {
      mkdirSync(resolve(workspace,'.cargo'),{recursive:true});
      mkdirSync(resolve(workspace,'src'),{recursive:true});
      writeFileSync(resolve(workspace,'src/lib.rs'),'');
      writeFileSync(resolve(workspace,'Cargo.toml'),'[workspace]\n[package]\nname="placement"\nversion="0.1.0"\nedition="2021"\n');
      writeFileSync(resolve(workspace,'.cargo/config.toml'),`[build]\nbuild-dir=${JSON.stringify(resolve(workspace,'intermediate'))}\n`);
      const env={...process.env,CARGO_TARGET_DIR:resolve(workspace,'products')};
      delete env.CARGO_BUILD_BUILD_DIR;
      const result=spawnSync('cargo',['metadata','--offline','--no-deps','--format-version','1'],{cwd:workspace,env,encoding:'utf8'});
      expect(result.status).toBe(0);
      return JSON.parse(result.stdout);
    };
    const a=resolved(source), b=resolved(dest);
    expect(a.build_directory).toBe(realpathSync(source)+'/intermediate');
    expect(a.target_directory).toBe(realpathSync(source)+'/products');
    const cache=keptGameCrates({root,env:process.env,...facts,directory:resolve(dir,'cache')});
    const original=unit(a.build_directory,'leaf',1);
    // Cover product paths as well as intermediate paths in split-layout dep-info.
    writeFileSync(original.dep,readFileSync(original.dep,'utf8')+`${a.target_directory}/lib: /registry/leaf/lib.rs\n`);
    cache.keep(a,'native',lock); cache.take(b,'native');
    const dep=resolve(b.build_directory,profile,'deps',`${original.id}.d`);
    expect(readFileSync(dep,'utf8')).toContain(b.build_directory);
    expect(readFileSync(dep,'utf8')).toContain(b.target_directory);
    expect(readFileSync(dep,'utf8')).not.toContain(source);
    expect(existsSync(resolve(b.target_directory,profile,'.fingerprint'))).toBe(false);
    expect(existsSync(resolve(b.build_directory,profile,'.fingerprint',original.id))).toBe(true);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('dep-info relocation accepts the realpath spelling of a /tmp target', () => {
  const dir=mkdtempSync(resolve('/tmp','exact-crate-alias-'));
  try {
    const a=resolve(dir,'a'), b=resolve(dir,'b'), lock=resolve(dir,'Cargo.lock');
    writeFileSync(lock,'[[package]]\nname="leaf"\nversion="1.0.0"\nsource="registry+example"\n');
    const original=unit(a,'leaf',1);
    // /tmp resolves to /private/tmp on macOS. A symlink also covers this off macOS.
    const alias=resolve(dir,'alias'); symlinkSync(realpathSync(a),alias);
    const paths={target_directory:alias,build_directory:alias};
    writeFileSync(original.dep,`${realpathSync(a)}/${profile}/deps/lib${original.id}.rlib: /registry/leaf/lib.rs\n`);
    const cache=keptGameCrates({root,env:process.env,...facts,directory:resolve(dir,'cache')});
    cache.keep(paths,'native',lock); cache.take(metadata(b),'native');
    const dep=readFileSync(resolve(b,profile,'deps',`${original.id}.d`),'utf8');
    expect(dep).toContain(`${realpathSync(b)}/${profile}/deps/`);
    expect(dep).not.toContain(realpathSync(a));
  } finally {rmSync(dir,{recursive:true,force:true});}
});
