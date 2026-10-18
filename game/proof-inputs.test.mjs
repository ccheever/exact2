import {test, expect} from 'bun:test';
import {createHash} from 'node:crypto';
import {cpSync, existsSync, mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync, realpathSync, renameSync} from 'node:fs';
import {resolve, dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {agreePins, buildInputHash, ensureBuildReceipt, ensureLinuxReceipts, pinInputs, proofInputFiles, proofInputReader, proofInputs} from './proof.mjs';
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
    writeFixtureFile(dir, 'game/Cargo.toml', '[workspace]\n');
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
    mkdirSync(resolve(app, '.shells'), {recursive:true});
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

function provenanceFixture(root, generated) {
  const app = resolve(root, 'game/games/sample');
  writeFixtureFile(root, 'game/engine/src/lib.rs', 'shared SDK');
  for (const [file, bytes] of Object.entries({
    'logic/Cargo.toml':'[package]\nname = "sample-logic"\nversion = "0.1.0"\n',
    'logic/src/lib.rs':'pub fn value() {}', 'app.contract':'authored plan',
    'logic/tests/sim.rs':'#[test] fn simulation() {}', 'logic/examples/demo.rs':'fn main() {}',
    'art/crate.gltf':'authored model', 'art/hit.ogg':'authored sound',
    'assets/handmade.astc.tex':'unowned texture', 'assets/handmade.sound':'unowned sound',
  })) writeFixtureFile(app, file, bytes);
  const manifest = {};
  for (const name of ['textures/crate.astc.tex', 'hit.sound']) {
    writeFixtureFile(app, 'assets/' + name, generated);
    manifest[name] = createHash('sha256').update(generated).digest('hex');
  }
  writeFixtureFile(app, '.baked-assets.json', JSON.stringify(manifest));
  return app;
}

test('portable authored provenance excludes owned texture and sound bytes, not unowned assets', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-portable-'));
  const roots = ['x86', 'arm'].map(name => resolve(dir, name));
  try {
    const apps = roots.map((root, i) => provenanceFixture(root, `baked on machine ${i}`));
    const snapshots = roots.map((root, i) => proofInputs(root, apps[i]));
    expect(snapshots[0].authored).toBe(snapshots[1].authored);
    expect(snapshots[0].all).not.toBe(snapshots[1].all);
    expect(() => pinInputs(snapshots.map(row => ({inputs:row.all, sourceInputs:row.authored})))).toThrow('inputs');
    for (const file of ['logic/src/lib.rs', 'art/hit.ogg', 'assets/handmade.astc.tex', 'assets/handmade.sound']) {
      const path = resolve(apps[0], file), before = readFileSync(path);
      writeFileSync(path, 'changed authored bytes');
      const changed = proofInputs(roots[0], apps[0]);
      expect(changed.authored).not.toBe(snapshots[0].authored);
      expect(changed.all).not.toBe(snapshots[0].all);
      writeFileSync(path, before);
    }
    const moved = resolve(dir, 'external/renamed-game');
    cpSync(apps[1], moved, {recursive:true});
    expect(proofInputs(roots[1], moved).authored).toBe(snapshots[1].authored);
    for (const app of [apps[1], moved]) {
      const before = proofInputs(roots[1], app);
      for (const file of ['logic/tests/sim.rs', 'logic/examples/demo.rs'])
        writeFileSync(resolve(app, file), 'changed crate target');
      const after = proofInputs(roots[1], app);
      expect(after.authored).toBe(before.authored);
      expect(after.all).toBe(before.all);
    }
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('art in examples and tests directories changes both proof digests and invalidates receipts', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-art-examples-')), root = resolve(dir, 'sdk');
  try {
    const internal = provenanceFixture(root, 'baked');
    const external = resolve(dir, 'external/sample');
    cpSync(internal, external, {recursive:true});
    for (const app of [external, internal]) {
      for (const folder of ['examples', 'tests']) {
        const art = writeFixtureFile(app, `art/${folder}/coin.png`, 'original PNG');
        const receipt = resolve(app, 'artifacts/build-linux.sha256');
        mkdirSync(dirname(receipt), {recursive:true});
        let product = 'old', builds = 0;
        const run = () => {
          const read = proofInputReader(root, app);
          return ensureBuildReceipt({receipt, inputs:() => read().all, artifact:() => product, build:async () => {
            builds++;
            product = readFileSync(art, 'utf8');
          }});
        };
        expect(await run()).toBe(true);
        expect(await run()).toBe(false);
        const before = proofInputs(root, app);
        writeFileSync(art, 'changed PNG');
        const after = proofInputs(root, app);
        expect(after.authored).not.toBe(before.authored);
        expect(after.all).not.toBe(before.all);
        expect(await run()).toBe(true);
        expect(product).toBe('changed PNG');
        expect(builds).toBe(2);
        expect(await run()).toBe(false);
      }
    }
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('repin records portable source provenance separately and still refuses mixed execution rows', async () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'proof-pin-provenance-'));
  const roots = ['x86', 'arm'].map(name => resolve(dir, name));
  const baseline = {ticks:{1:'0x' + 'a'.repeat(16)}, saves:{continuation:'a'.repeat(64)}};
  try {
    const apps = roots.map((root, i) => provenanceFixture(root, `baked on machine ${i}`));
    const snapshots = roots.map((root, i) => proofInputs(root, apps[i]));
    for (const app of apps) {
      writeFixtureFile(app, 'pins.json', JSON.stringify(baseline));
      writeFixtureFile(app, 'proof.mjs', `
        import {mkdirSync, writeFileSync} from 'node:fs';
        const out = process.env.EXACT_PROOF_OUT, mode = process.env.EXACT_GAME_PARANOID;
        const row = {host:process.argv[2], mode, status:'UNVERIFIED', failures:[], seconds:0,
          pins:${JSON.stringify(baseline)}, inputs:process.env.TEST_EXECUTION_INPUTS,
          sourceInputs:process.env.TEST_SOURCE_INPUTS};
        if (mode === 'fresh-game') {
          if (process.env.TEST_MIXED === 'execution') row.inputs = 'f'.repeat(64);
          if (process.env.TEST_MIXED === 'source') row.sourceInputs = 'f'.repeat(64);
          if (process.env.TEST_MIXED === 'missing-source') delete row.sourceInputs;
        }
        mkdirSync(out, {recursive:true});
        writeFileSync(out + '/summary.json', JSON.stringify(row));
      `);
    }
    const run = async (i, mixed = '') => {
      const child = Bun.spawn([process.execPath, resolve(import.meta.dir, 'prove.mjs'), apps[i], '--repin', '--hosts', 'linux'], {
        env:{...process.env, TEST_EXECUTION_INPUTS:snapshots[i].all, TEST_SOURCE_INPUTS:snapshots[i].authored, TEST_MIXED:mixed},
        stdout:'pipe', stderr:'pipe',
      });
      const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
      return {code, text:stdout + stderr};
    };
    const pins = [];
    for (let i = 0; i < apps.length; i++) {
      const file = resolve(apps[i], 'pins.json'), before = readFileSync(file, 'utf8');
      const mixed = await run(i, 'execution');
      expect(mixed.code, mixed.text).toBe(1);
      expect(mixed.text).toContain('proof inputs missing or changed between runs');
      expect(readFileSync(file, 'utf8')).toBe(before);
      const result = await run(i);
      expect(result.code, result.text).toBe(0);
      pins.push(JSON.parse(readFileSync(file, 'utf8')));
      expect(pins[i].inputs).toBe(snapshots[i].all);
      expect(pins[i].sourceInputs).toBe(snapshots[i].authored);
      expect(pins[i].at).toBe('inputs:' + snapshots[i].authored);
      for (const mixed of ['source', 'missing-source']) {
        const before = readFileSync(file, 'utf8'), refused = await run(i, mixed);
        expect(refused.code, refused.text).toBe(1);
        expect(refused.text).toContain('source inputs missing or changed between runs');
        expect(readFileSync(file, 'utf8')).toBe(before);
      }
    }
    expect(pins[0].sourceInputs).toBe(pins[1].sourceInputs);
    expect(pins[0].inputs).not.toBe(pins[1].inputs);
  } finally { rmSync(dir, {recursive:true, force:true}); }
}, 15000);

test('proof summaries record both identities from the completed bake', async () => {
  // Match the child's import.meta URL when macOS's temporary root is a symlink.
  const dir = realpathSync(mkdtempSync(resolve(tmpdir(), 'proof-summary-provenance-')));
  const root = resolve(import.meta.dir, '..'), app = provenanceFixture(dir, 'before bake');
  const target = resolve(app, 'target'), out = resolve(app, 'artifacts/linux');
  try {
    mkdirSync(target); mkdirSync(out, {recursive:true});
    writeFixtureFile(app, 'pins.json', JSON.stringify({ticks:{}, saves:{}}));
    const before = proofInputs(root, app);
    // Exercise the real proof/receipt/summary lifecycle with a deterministic baker,
    // without compiling a game to test which snapshot gets serialized.
    writeFixtureFile(app, 'proof.mjs', `
      import {mock} from 'bun:test';
      import {createHash} from 'node:crypto';
      import {mkdirSync, writeFileSync} from 'node:fs';
      import {resolve} from 'node:path';
      const app = ${JSON.stringify(app)}, target = ${JSON.stringify(target)};
      const path = ${JSON.stringify(resolve(root, 'scripts/app.mjs'))};
      const original = await import(path);
      mock.module(path, () => ({...original,
        resolveApp:() => ({dir:app, target, crate:kind => 'sample-' + kind}),
        buildBake:(_app, _host, triple, {profile}) => {
          const product = resolve(target, triple, profile); mkdirSync(product, {recursive:true});
          const library = 'libsample_gpu.' + (process.platform === 'darwin' ? 'dylib' : 'so');
          for (const name of ['sample-linux', library]) writeFileSync(resolve(product, name), 'product');
          const manifest = {}, bytes = 'after bake';
          for (const name of ['textures/crate.astc.tex', 'hit.sound']) {
            writeFileSync(resolve(app, 'assets', name), bytes);
            manifest[name] = createHash('sha256').update(bytes).digest('hex');
          }
          writeFileSync(resolve(app, '.baked-assets.json'), JSON.stringify(manifest));
        },
      }));
      const {proof} = await import(${JSON.stringify(resolve(import.meta.dir, 'proof.mjs'))});
      await proof(import.meta, async () => {});
    `);
    const child = Bun.spawn([process.execPath, resolve(app, 'proof.mjs'), 'linux', '--build-only'], {
      env:{...process.env, EXACT_GAME_PROOF_PROFILE:'release', EXACT_PROOF_OUT:out}, stdout:'pipe', stderr:'pipe',
    });
    const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
    expect(code, stdout + stderr).toBe(0);
    const after = proofInputs(root, app), summary = JSON.parse(readFileSync(resolve(out, 'summary.json')));
    expect(after.all).not.toBe(before.all);
    expect(after.authored).toBe(before.authored);
    expect(summary.inputs).toBe(after.all);
    expect(summary.sourceInputs).toBe(after.authored);
    const receipt = JSON.parse(readFileSync(resolve(app, 'artifacts/build-linux.sha256')));
    expect(receipt.inputs).toBe(buildInputHash('linux', target, '0', 'release').update(after.all).digest('hex'));
  } finally { rmSync(dir, {recursive:true, force:true}); }
}, 15000);


test('web repin requires shipping observations and receipts distinguish profiles', () => {
  const pins = {ticks:{60:'0x123456789abcdef0'}, saves:{continuation:'a'.repeat(64)}};
  const rows = ['linux', 'web'].flatMap(host => ['0', '1', 'fresh-game', 'release'].map(mode => ({host, mode:mode === 'release' ? '0' : mode, profile:mode === 'release' ? mode : 'gpu-dev', pins:structuredClone(pins)})));
  const old = pins;
  expect(() => agreePins(rows.filter(row => !(row.host === 'web' && row.profile === 'release')), old, ['linux', 'web'], '.')).toThrow('web release');
  rows.find(row => row.host === 'web' && row.profile === 'release').pins.ticks[60] = '0xdddddddddddddddd';
  expect(() => agreePins(rows, old, ['linux', 'web'], '.')).toThrow('disagrees with gpu-dev');
  expect(buildInputHash('web', 'target', '0', 'release').digest('hex'))
    .not.toBe(buildInputHash('web', 'target', '0', 'gpu-dev').digest('hex'));
});
