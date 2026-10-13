import {test, expect} from 'bun:test';
import {createHash} from 'node:crypto';
import {existsSync, mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync, renameSync} from 'node:fs';
import {resolve, dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {buildInputHash, ensureBuildReceipt, ensureLinuxReceipts, proofInputFiles, proofInputReader, proofInputs} from './proof.mjs';
import {writeFixtureFile} from './tests/proof-helpers.mjs';

test('receipts issued before directory checks rebuild once even when source and artifact digests still match', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-receipt-version-')), receipt = resolve(dir, 'build.sha256');
  try {
    const oldInputs = createHash('sha256').update('linux').update('target').update('gpu-dev').update('source').digest('hex');
    const inputs = buildInputHash('linux', 'target', '0', 'gpu-dev').update('source').digest('hex');
    writeFileSync(receipt, JSON.stringify({inputs:oldInputs, artifact:'artifact'}));
    let builds = 0;
    const run = () => ensureBuildReceipt({receipt, inputs, artifact:() => 'artifact', build:async () => { builds++; }});
    expect(await run()).toBe(true);
    expect(await run()).toBe(false);
    expect(builds).toBe(1);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('authored edits during a build refuse the proof and force the next build', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-authored-'));
  const app = resolve(dir, 'game/games/sample'), cache = resolve(dir, 'artifacts/input-digests.json');
  const receipt = resolve(dir, 'artifacts/build.sha256');
  try {
    mkdirSync(dirname(receipt), {recursive:true});
    for (const file of ['game/games/sample/logic/src/lib.rs', 'game/games/sample/art/crate.gltf',
      'game/games/sample/assets/authored.bin', 'host/linux/src/lib.rs']) {
      writeFixtureFile(dir, file, 'v1');
      let product = 'old', builds = 0;
      writeFileSync(receipt, 'old receipt');
      const run = () => {
        const read = proofInputReader(dir, app, cache);
        return ensureBuildReceipt({receipt, inputs:() => read().all, artifact:() => product, build:async () => {
          builds++;
          product = readFileSync(resolve(dir, file), 'utf8');
          if (builds === 1) writeFixtureFile(dir, file, 'v2');
        }});
      };
      await expect(run()).rejects.toThrow('changed during build');
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
  const out = resolve(dir, 'artifacts');
  try {
    mkdirSync(app, {recursive:true});
    writeFixtureFile(dir, 'game/games/sample/app.contract', 'v1');
    const read = proofInputReader(dir, app, resolve(out, 'input-digests.json'));
    await expect(ensureLinuxReceipts({directory:out, gpuInputs:() => read().gpu, hostInputs:() => read().host,
      gpuArtifact:() => 'gpu', hostArtifact:() => 'host', build:async part => {
        if (part === 'host') writeFixtureFile(dir, 'game/games/sample/app.contract', 'v2');
      }})).rejects.toThrow('authored proof inputs changed during build');
    expect(existsSync(resolve(out, 'build-linux-host.sha256'))).toBe(false);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('transient sources consumed by a build invalidate its receipt, including in empty directories', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-transient-')), app = resolve(dir, 'game/games/sample');
  const receipt = resolve(dir, 'artifacts/build.sha256');
  try {
    mkdirSync(dirname(receipt), {recursive:true});
    mkdirSync(resolve(app, 'art/empty'), {recursive:true});
    mkdirSync(resolve(app, 'assets/textures'), {recursive:true});
    writeFixtureFile(dir, 'game/games/sample/app.contract', 'plan');
    for (const file of ['art/transient.gltf', 'art/empty/transient.png', 'art/transient-tree/mesh.glb',
      'assets/transient.bin', 'assets/textures/transient.bin', 'assets/transient-tree/file.bin']) {
      rmSync(resolve(app, 'assets/textures/generated.tex'), {force:true});
      rmSync(resolve(app, '.baked-assets.json'), {force:true});
      let product = 'old', builds = 0;
      writeFileSync(receipt, 'old receipt');
      const run = () => {
        const read = proofInputReader(dir, app);
        return ensureBuildReceipt({receipt, inputs:() => read().all, artifact:() => product, build:async () => {
          builds++;
          if (builds === 1) {
            writeFixtureFile(app, file, 'transient source');
            product = readFileSync(resolve(app, file), 'utf8');
            rmSync(resolve(app, file.includes('/transient-tree/') ? dirname(file) : file), {recursive:true});
            writeFixtureFile(app, 'assets/textures/generated.tex', product);
            writeFixtureFile(app, '.baked-assets.json', JSON.stringify({'textures/generated.tex':createHash('sha256').update(product).digest('hex')}));
          } else product = 'stable';
        }});
      };
      await expect(run()).rejects.toThrow('changed during build');
      expect(product).toBe('transient source');
      expect(existsSync(receipt)).toBe(false);
      expect(await run()).toBe(true);
      expect(product).toBe('stable');
      expect(await run()).toBe(false);
      expect(builds).toBe(2);
    }
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('the shared test helper is excluded from proof inputs and in-flight authored state', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-helper-')), app = resolve(dir, 'game/games/sample');
  const helper = 'game/tests/proof-helpers.mjs';
  try {
    writeFixtureFile(app, 'app.contract', 'plan');
    writeFixtureFile(dir, helper, 'test helper v1');
    const read = proofInputReader(dir, app), before = read();
    expect(proofInputFiles(dir, app)).not.toContain(helper);
    writeFixtureFile(dir, helper, 'test helper v2');
    const after = read();
    for (const key of ['all', 'gpu', 'host', 'authored', 'authoredState', 'assetState']) expect(after[key]).toBe(before[key]);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('generated entry changes refuse a receipt until a stable rerun; in-place rewrites can complete immediately', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-output-')), app = resolve(dir, 'game/games/sample');
  const manifest = resolve(app, '.baked-assets.json'), asset = resolve(app, 'assets/textures/new.tex');
  const receipt = resolve(dir, 'artifacts/build.sha256');
  const generated = bytes => {
    writeFixtureFile(app, 'assets/textures/new.tex', bytes);
    writeFileSync(manifest, JSON.stringify({'textures/new.tex':createHash('sha256').update(bytes).digest('hex')}));
  };
  try {
    mkdirSync(dirname(receipt), {recursive:true});
    mkdirSync(app, {recursive:true});
    for (const operation of ['create', 'prune', 'replace', 'rewrite']) {
      rmSync(resolve(app, 'assets'), {recursive:true, force:true});
      rmSync(manifest, {force:true});
      if (operation !== 'create') generated('before');
      const before = proofInputs(dir, app);
      writeFileSync(receipt, 'old receipt');
      let product = 'old', builds = 0;
      const run = () => {
        const read = proofInputReader(dir, app);
        return ensureBuildReceipt({receipt, inputs:() => read().all, artifact:() => product, build:async () => {
          if (++builds === 1) {
            if (operation === 'prune') { rmSync(asset); rmSync(manifest); }
            else {
              if (operation === 'replace') { writeFileSync(asset + '.tmp', 'after'); renameSync(asset + '.tmp', asset); }
              generated('after');
            }
          }
          product = existsSync(asset) ? readFileSync(asset, 'utf8') : 'empty assets';
        }});
      };
      if (operation === 'rewrite') expect(await run()).toBe(true);
      else {
        await expect(run()).rejects.toThrow('asset directories changed during build');
        expect(existsSync(receipt)).toBe(false);
        expect(await run()).toBe(true);
      }
      expect(await run()).toBe(false);
      expect(builds).toBe(operation === 'rewrite' ? 1 : 2);
      const after = proofInputs(dir, app);
      expect(after.all).not.toBe(before.all);
      expect(after.authored).toBe(before.authored);
      expect(JSON.parse(readFileSync(receipt, 'utf8'))).toEqual({inputs:after.all, artifact:product});
    }
    const read = proofInputReader(dir, app); read();
    writeFixtureFile(app, 'app.contract', 'authored');
    expect(read).toThrow('authored proof inputs changed during build');
    const afterAddition = proofInputReader(dir, app); afterAddition();
    rmSync(resolve(app, 'app.contract'));
    expect(afterAddition).toThrow('authored proof inputs changed during build');
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('an initially absent assets tree cannot appear, feed a build and disappear without invalidating its receipt', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-absent-assets-')), app = resolve(dir, 'game/games/sample');
  const receipt = resolve(dir, 'artifacts/build.sha256');
  try {
    writeFixtureFile(app, 'app.contract', 'plan');
    mkdirSync(dirname(receipt), {recursive:true});
    writeFileSync(receipt, 'old receipt');
    let product = 'old';
    const read = proofInputReader(dir, app);
    await expect(ensureBuildReceipt({receipt, inputs:() => read().all, artifact:() => product, build:async () => {
      const path = writeFixtureFile(app, 'assets/transient.bin', 'transient bytes');
      product = readFileSync(path, 'utf8');
      rmSync(resolve(app, 'assets'), {recursive:true});
    }})).rejects.toThrow('asset directories changed during build');
    expect(product).toBe('transient bytes');
    expect(existsSync(receipt)).toBe(false);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('E11 receipts and proof provenance bind post-bake generated assets', async () => {
  const dir=mkdtempSync(resolve(tmpdir(),'e11-generated-')), app=resolve(dir,'game/games/sample'), out=resolve(dir,'artifacts/linux');
  const write=(file,bytes)=>writeFixtureFile(dir,file,bytes);
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
  const write=(file,text)=>writeFixtureFile(dir,file,text);
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
