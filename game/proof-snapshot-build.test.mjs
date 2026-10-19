import {test, expect} from 'bun:test';
import {spawn} from 'node:child_process';
import {cpSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {createGame} from './new.mjs';

// One scaffold and its shared Cargo target exercise the whole snapshot/build boundary.
test('a proof leaves ordinary builds usable and repins changed logic and added art', async () => {
  const parent = realpathSync(mkdtempSync(resolve(import.meta.dir, '../../proof-build-'))), app = resolve(parent, 'tidal-proof');
  const env = {...process.env, EXACT_UPDATE_TRUST:'development'};
  for (const name of ['EXACT_APP_DIR','CARGO_TARGET_DIR','CARGO_BUILD_BUILD_DIR','EXACT_PROOF_BUILD_OUT','EXACT_PROOF_OUT','EXACT_WEB_DIST']) delete env[name];
  const run = args => new Promise((ok, fail) => {
    const child = spawn(process.execPath, args, {cwd:app, env, stdio:['ignore','pipe','pipe']});
    let log = '';
    for (const stream of [child.stdout,child.stderr]) stream.on('data', bytes => {log += bytes; process.stdout.write(bytes);});
    child.on('error', fail);
    child.on('close', code => code === 0 ? ok(log) : fail(new Error(`exit ${code}: ${args.join(' ')}\n${log.slice(-8000)}`)));
  });
  const prove = async args => {
    const log = await run(['exact.mjs','prove',...args]);
    const root = log.match(/^ARTIFACTS (.+)$/m)[1];
    return {root, pins:JSON.parse(readFileSync(resolve(app,'pins.json'),'utf8'))};
  };
  try {
    createGame(app, undefined, {assets:true});
    // Keep the regression about build identity; the starter's gameplay has its own proof.
    writeFileSync(resolve(app,'proof.mjs'), `import {resolve} from 'node:path';
import {proof} from ${JSON.stringify(resolve(import.meta.dir,'proof.mjs'))};
if (import.meta.main) await proof(import.meta, async ({open,pin,pinSave,out}) => {
  const session = await open(); await session.tap('play');
  const world = session.world('world'); pin(0, await world.snapshot());
  const save = resolve(out,'start.world'); await world.save(save); pinSave('start',save);
  await session.close();
});\n`);
    const first = await prove([]);
    await run(['exact.mjs','web-build']);
    const source = resolve(app,'logic/src/lib.rs');
    const original = readFileSync(source,'utf8');
    expect(original).toContain('Transform::at(-4.0, 0.5, -3.0)');
    writeFileSync(source, original.replace('Transform::at(-4.0, 0.5, -3.0)', 'Transform::at(-5.0, 0.5, -3.0)'));
    const changed = await prove(['--repin']);
    expect(changed.pins.ticks['0']).not.toBe(first.pins.ticks['0']);
    expect(readFileSync(resolve(changed.root,'linux-0-1/run.log'),'utf8')).toContain('BUILD gpu stale');
    cpSync(resolve(app,'art/crate.gltf'),resolve(app,'art/second.gltf'));
    const added = await prove(['--repin']);
    expect(Object.keys(JSON.parse(readFileSync(resolve(added.root,'src/.baked-assets.json'),'utf8')))).toContain('second.model');
    await run(['exact.mjs','web-build']);
  } finally {rmSync(parent,{recursive:true,force:true});}
}, 1800000);
