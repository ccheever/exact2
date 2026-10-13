import {test, expect} from 'bun:test';
import {createHash} from 'node:crypto';
import {existsSync, mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync} from 'node:fs';
import {resolve, dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {buildInputHash, ensureBuildReceipt, ensureLinuxReceipts, proofInputReader, proofInputs} from './proof.mjs';

test('authored edits during a build refuse the proof and force the next build', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-authored-'));
  const app = resolve(dir, 'game/games/sample'), cache = resolve(dir, 'artifacts/input-digests.json');
  const receipt = resolve(dir, 'artifacts/build.sha256');
  const write = (file, text) => { const path = resolve(dir, file); mkdirSync(dirname(path), {recursive:true}); writeFileSync(path, text); };
  try {
    mkdirSync(dirname(receipt), {recursive:true});
    for (const file of ['game/games/sample/logic/src/lib.rs', 'game/games/sample/art/crate.gltf',
      'game/games/sample/assets/authored.bin', 'host/linux/src/lib.rs']) {
      write(file, 'v1');
      let product = 'old', builds = 0;
      writeFileSync(receipt, 'old receipt');
      const run = () => {
        const read = proofInputReader(dir, app, cache);
        return ensureBuildReceipt({receipt, inputs:() => read().all, artifact:() => product, build:async () => {
          builds++;
          product = readFileSync(resolve(dir, file), 'utf8');
          if (builds === 1) write(file, 'v2');
        }});
      };
      await expect(run()).rejects.toThrow('authored proof inputs changed during build');
      expect(product).toBe('v1');
      expect(existsSync(receipt)).toBe(false);
      expect(await run()).toBe(true);
      expect(product).toBe('v2');
      expect(await run()).toBe(false);
      expect(builds).toBe(2);
    }
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('a split proof also rejects authored changes while building its host', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-host-edit-')), app = resolve(dir, 'game/games/sample');
  const out = resolve(dir, 'artifacts'), source = resolve(app, 'app.contract');
  try {
    mkdirSync(app, {recursive:true});
    writeFileSync(source, 'v1');
    const read = proofInputReader(dir, app, resolve(out, 'input-digests.json'));
    await expect(ensureLinuxReceipts({directory:out, gpuInputs:() => read().gpu, hostInputs:() => read().host,
      gpuArtifact:() => 'gpu', hostArtifact:() => 'host', build:async part => {
        if (part === 'host') writeFileSync(source, 'v2');
      }})).rejects.toThrow('authored proof inputs changed during build');
    expect(existsSync(resolve(out, 'build-linux-host.sha256'))).toBe(false);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('bake output creation and pruning preserve authored identity, while source additions and deletions do not', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-output-')), app = resolve(dir, 'game/games/sample');
  const manifest = resolve(app, '.baked-assets.json'), asset = resolve(app, 'assets/new.tex');
  try {
    mkdirSync(dirname(asset), {recursive:true});
    const read = proofInputReader(dir, app), before = read();
    writeFileSync(asset, 'generated');
    writeFileSync(manifest, JSON.stringify({'new.tex':createHash('sha256').update('generated').digest('hex')}));
    const baked = read();
    expect(baked.all).not.toBe(before.all);
    expect(baked.authored).toBe(before.authored);
    rmSync(asset); rmSync(manifest);
    expect(read().all).toBe(before.all);
    writeFileSync(resolve(app, 'app.contract'), 'authored');
    expect(read).toThrow('authored proof inputs changed during build');
    const afterAddition = proofInputReader(dir, app); afterAddition();
    rmSync(resolve(app, 'app.contract'));
    expect(afterAddition).toThrow('authored proof inputs changed during build');
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('E11 receipts and proof provenance bind post-bake generated assets', async () => {
  const dir=mkdtempSync(resolve(tmpdir(),'e11-generated-')), app=resolve(dir,'game/games/sample'), out=resolve(dir,'artifacts/linux');
  const write=(file,bytes)=>{const path=resolve(dir,file);mkdirSync(dirname(path),{recursive:true});writeFileSync(path,bytes);};
  const generated=bytes=>{write('game/games/sample/assets/textures/crate.astc.tex',bytes);write('game/games/sample/.baked-assets.json',JSON.stringify({'textures/crate.astc.tex':createHash('sha256').update(bytes).digest('hex')}));};
  const surface=()=>readFileSync(resolve(app,'.shells/surfaces.json'),'utf8');
  try {
    write('game/games/sample/art/crate.gltf','authored');write('game/games/sample/logic/src/lib.rs','surface');generated('mac-simd');
    let inputs;const read=()=>inputs=proofInputs(dir,app,resolve(out,'input-digests.json')), inputHash=value=>buildInputHash('linux','target').update(value).digest('hex'), before=read();let gpu='old-product',binary='old-host',builds=[];
    const run=()=>{const scan=proofInputReader(dir,app,resolve(out,'input-digests.json'));const read=()=>inputs=scan();return ensureLinuxReceipts({directory:out,gpuInputs:()=>inputHash(read().gpu),hostInputs:()=>inputHash(read().host+surface()),gpuArtifact:()=>gpu,hostArtifact:()=>binary,build:async part=>{builds.push(part);if(part==='gpu'){generated('linux-simd');write('game/games/sample/.shells/surfaces.json',readFileSync(resolve(app,'logic/src/lib.rs'),'utf8'));gpu='linux-product';}else binary='linux-host';}});};await run();
    const summary=inputs.all, after=proofInputs(dir,app,resolve(out,'input-digests.json')), gpuReceipt=JSON.parse(readFileSync(resolve(out,'build-linux-gpu.sha256'))), hostReceipt=JSON.parse(readFileSync(resolve(out,'build-linux-host.sha256')));
    expect(before.all).not.toBe(after.all);
    expect({gpu:gpuReceipt.inputs,host:hostReceipt.inputs,summary}).toEqual({gpu:inputHash(after.gpu),host:inputHash(after.host+surface()),summary:after.all});
    expect(builds.splice(0)).toEqual(['gpu','host']);await run();expect(builds.splice(0)).toEqual([]);write('game/games/sample/art/crate.gltf','changed');await run();expect(builds.splice(0)).toEqual(['gpu']);
    write('game/games/sample/logic/src/lib.rs','surface with added argument');await run();expect(builds.splice(0)).toEqual(['gpu','host']);
    write('game/games/sample/app.contract','changed');await run();expect(builds.splice(0)).toEqual(['host']);gpu=null;await run();expect(builds.splice(0)).toEqual(['gpu']);
  } finally {rmSync(dir,{recursive:true,force:true});}
});

test('E11 clean Git blobs and stat-cached dirty inputs have stable identities', async () => {
  const {spawnSync}=await import('node:child_process');
  const dir=mkdtempSync(resolve(tmpdir(),'e11-digest-')), app=resolve(dir,'game/games/sample');
  const write=(file,text)=>{const path=resolve(dir,file);mkdirSync(dirname(path),{recursive:true});writeFileSync(path,text);};
  const git=(...args)=>{const r=spawnSync('git',args,{cwd:dir,encoding:'utf8'});expect(r.status).toBe(0);};
  try {
    write('game/games/sample/logic/src/lib.rs','logic');write('host/linux/src/lib.rs','host');
    write('game/games/sample/app.contract','plan');git('init','-q');git('add','.');
    const cache=resolve(dir,'cache'), read=()=>proofInputs(dir,app,cache);
    const a=read(); expect(a.reads).toBe(0);
    write('game/games/sample/logic/src/lib.rs','changed');
    const b=read();expect(b.gpu).not.toBe(a.gpu);expect(b.host).toBe(a.host);expect(b.reads).toBe(1);
    expect(read()).toMatchObject({gpu:b.gpu,host:b.host,reads:0});
    git('add','.');expect(read().all).toBe(b.all);
    write('QUEUE.md','new task');write('host/web/glue.test.mjs','test');expect(read().all).toBe(b.all);
    write('host/apple/Sources/CExact/module.modulemap','module CExact {}');expect(read().host).not.toBe(b.host);
    const beforeEngine=read();write('game/engine/src/lib.rs','engine change');
    const engine=read();expect(engine.gpu).not.toBe(beforeEngine.gpu);expect(engine.host).toBe(beforeEngine.host);
    write('game/games/sample/logic/src/lib.rs','logic');expect(read().all).not.toBe(b.all);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
