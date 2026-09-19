import { createHash } from 'node:crypto';
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { cpSync, existsSync, readFileSync, readdirSync, realpathSync, rmSync, mkdtempSync, mkdirSync, writeFileSync, statSync, utimesSync, renameSync, lstatSync, readlinkSync, chmodSync, symlinkSync } from 'node:fs';
import { resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createGame } from './new.mjs';

// Exercise the copied files unchanged, including their manifests and shared bake.
test('a newly generated game builds, refuses empty pins and captures without editing', async () => {
  const directory = realpathSync(mkdtempSync(resolve(tmpdir(), 'game new-proof-')));
  const name = `new-proof-${process.pid}`, app = resolve(directory, name);
  const env = {...process.env, EXACT_UPDATE_TRUST:'development'};
  delete env.EXACT_APP_DIR;
  delete env.CARGO_TARGET_DIR;
  const run = (command, args, cwd = resolve(import.meta.dir, '..'), expected = 0) => new Promise((ok, fail) => {
    const child = spawn(command, args, {cwd, env, detached:true, stdio:['ignore','pipe','pipe']});
    let timer;
    const reset = () => {
      clearTimeout(timer);
      // The proof's cold native build can finish several quiet compiler steps.
      timer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }, 180000);
    };
    for (const stream of [child.stdout, child.stderr]) stream.on('data', data => { process.stdout.write(data); reset(); });
    reset();
    child.on('error', error => { clearTimeout(timer); fail(error); });
    child.on('close', (code, signal) => { clearTimeout(timer); code === expected ? ok() : fail(new Error(`${command} exited ${code ?? signal}; expected ${expected}`)); });
  });
  assert.ok(!existsSync(app));
  try {
    await run('bun', ['game/new.mjs', app]);
    assert.deepEqual(readdirSync(app).sort(), ['.gitignore','Cargo.lock','README.md','app.contract','app.json','logic','pins.json','proof.mjs']);
    const authoredManifest = readFileSync(resolve(app, 'app.json'), 'utf8');
    rmSync(resolve(app, 'logic/Cargo.toml'));
    env.EXACT_APP_DIR = app;
    await run('bun', ['-e', 'import {resolveApp} from "./scripts/app.mjs"; resolveApp();']);
    for (const file of ['web','apple','gpu']) assert.ok(!existsSync(resolve(app, file)), file);
    // The child's stdout arrives empty under `bun test` (exit 0, nothing written);
    // the located shells travel through a file instead.
    const locatedFile = resolve(directory, 'shells.json');
    const located = spawnSync('bun', ['-e', `import {resolveApp} from "./scripts/app.mjs"; import {dirname} from "node:path"; import {writeFileSync} from "node:fs"; const app=resolveApp(); writeFileSync(${JSON.stringify(locatedFile)}, JSON.stringify(["gpu","web","apple","linux"].map(kind=>dirname(app.cargoPackage(kind).manifest_path))));`], {cwd:resolve(import.meta.dir,'..'), env, encoding:'utf8'});
    assert.equal(located.status, 0, located.stderr);
    assert.ok(existsSync(locatedFile), 'the shell-location child wrote nothing');
    const shellDirs = JSON.parse(readFileSync(locatedFile, 'utf8'));
    assert.equal(shellDirs.length, 4, 'four shells located');
    assert.ok(shellDirs.every(dir => dir.startsWith(resolve(app, '.shells') + '/')), 'the game owns its generated hosts');
    const shellFiles = shellDirs.flatMap((dir, index) => ['Cargo.toml',index === 3 ? 'src/main.rs' : 'src/lib.rs', ...(index === 0 ? [] : ['build.rs'])].map(file=>resolve(dir,file)));
    const stamps = shellFiles.map(file => statSync(file).mtimeMs);
    await run('bun', ['-e', 'import {resolveApp} from "./scripts/app.mjs"; resolveApp();']);
    assert.deepEqual(shellFiles.map(file => statSync(file).mtimeMs), stamps, 'resolving again leaves Cargo inputs untouched');
    await run('cargo', ['test', '--locked', '--offline', '--no-fail-fast', '-p', `${name}-logic`], resolve(app,'.shells'));
    // The starter must refuse an empty baseline. All-mode first-pin agreement
    // is covered by prove's tests; it need not rebuild seven bakes in this fixture.
    await run('bun', [resolve(app, 'proof.mjs')], undefined, 1);
    assert.match(readFileSync(resolve(app, 'artifacts/linux/proof.txt'), 'utf8'), /UNVERIFIED: no pins — run bun game\/prove.mjs/);
    await run('bun', [resolve(app, 'proof.mjs'), 'web', '--screenshot-only']);
    assert.match(readFileSync(resolve(app, 'artifacts/web/proof.txt'), 'utf8'), new RegExp(`PROOF UNVERIFIED ${name} web: 0 failures`));
    const receipt = resolve(app, 'artifacts/build-linux.sha256');
    const before = JSON.parse(readFileSync(receipt, 'utf8'));
    const source = resolve(app, 'logic/src/lib.rs');
    writeFileSync(source, readFileSync(source, 'utf8') + '\n// External app edits must invalidate the proof build.\n');
    await run('bun', [resolve(app, 'proof.mjs'), 'linux', '--build-only']);
    assert.notEqual(JSON.parse(readFileSync(receipt, 'utf8')).inputs, before.inputs);
    assert.equal(readFileSync(resolve(app, 'app.json'), 'utf8'), authoredManifest);
  } finally {
    rmSync(directory, {recursive:true, force:true});
  }
}, 600000);

test('generator owns shells and does not normalize another game or write the shared lock', () => {
  const dir=mkdtempSync(resolve(tmpdir(),'game-new-local-'));
  try {
    cpSync(resolve(import.meta.dir,'new'),resolve(dir,'new'),{recursive:true});
    cpSync(resolve(import.meta.dir,'Cargo.toml'),resolve(dir,'Cargo.toml'));
    writeFileSync(resolve(dir,'Cargo.lock'),'shared lock sentinel');
    mkdirSync(resolve(dir,'games/existing/logic/src'),{recursive:true});
    writeFileSync(resolve(dir,'games/existing/logic/src/lib.rs'),`impl Game for Existing { const ID: &'static str = "existing"; }`);
    const before = treeHash(dir, resolve(dir,'games/added'));
    const stamp=statSync(resolve(dir,'Cargo.lock')).mtimeMs;
    createGame('added',dir);
    assert.equal(treeHash(dir, resolve(dir,'games/added')), before);
    assert.equal(readFileSync(resolve(dir,'Cargo.lock'),'utf8'),'shared lock sentinel');
    assert.equal(statSync(resolve(dir,'Cargo.lock')).mtimeMs,stamp);
    assert.ok(!existsSync(resolve(dir,'games/existing/app.json')));
    assert.ok(!existsSync(resolve(dir,'.shells')));
    assert.ok(!existsSync(resolve(dir,'games/added/.shells')));
  } finally { rmSync(dir,{recursive:true,force:true}); }
});

test('changing app identity prunes old adapters without deleting the Cargo cache', async () => {
  const {gameDefaults, gameShells} = await import('./app/shells.mjs');
  const parent = realpathSync(mkdtempSync(resolve(tmpdir(), 'game-cache-'))), app = resolve(parent, 'my-game');
  try {
    createGame(app);
    gameShells(app, gameDefaults(app).game, import.meta.dir);
    const root = resolve(app, '.shells');
    const oldAdapters = readdirSync(root).filter(name => /^[a-f0-9]{24}-/.test(name));
    assert.equal(oldAdapters.length, 4);
    const cache = resolve(root, 'target/debug/warm-artifact');
    mkdirSync(resolve(cache, '..'), {recursive:true});
    writeFileSync(cache, 'completed Cargo output');
    const manifest = gameDefaults(app);
    delete manifest._generated;
    manifest.app.id = 'org.example.renamed';
    writeFileSync(resolve(app, 'app.json'), JSON.stringify(manifest));
    gameShells(app, manifest.game, import.meta.dir);
    assert.equal(readFileSync(cache, 'utf8'), 'completed Cargo output');
    assert.ok(oldAdapters.every(name => !existsSync(resolve(root, name))));
    assert.equal(readdirSync(root).filter(name => /^[a-f0-9]{24}-/.test(name)).length, 4);
  } finally { rmSync(parent, {recursive:true, force:true}); }
});

test('default manifests derive the directory and literal Game ID; authored overrides win', async () => {
  const {gameDefaults} = await import('./app/shells.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'e5-defaults-')), dir = resolve(root,'runner-game');
  try {
    mkdirSync(resolve(dir,'logic/src'), {recursive:true});
    writeFileSync(resolve(dir,'logic/src/lib.rs'), `pub struct MyGame; impl Game for MyGame { const ID: &'static str = "stable-save-id"; }`);
    const defaults = gameDefaults(dir);
    assert.equal(defaults.app.id, 'com.exact.stable-save-id');
    assert.equal(defaults.game.crate, 'runner-game-logic');
    assert.equal(defaults.game.type, 'MyGame');
    assert.match(readFileSync(resolve(dir,'logic/Cargo.toml'),'utf8'), /name = "runner-game-logic"/);
    const renamed = resolve(root, 'renamed-game');
    renameSync(dir, renamed);
    writeFileSync(resolve(renamed,'logic/src/lib.rs'), `pub struct NewGame; impl Game for NewGame { const HZ: u32 = 120; const ID: &'static str = "new-save-id"; }`);
    const refreshed = gameDefaults(renamed);
    assert.equal(refreshed.game.crate, 'renamed-game-logic');
    assert.equal(refreshed.game.type, 'NewGame');
    assert.equal(refreshed.app.id, 'com.exact.new-save-id');
    assert.match(readFileSync(resolve(renamed,'logic/Cargo.toml'),'utf8'), /name = "renamed-game-logic"/);
    renameSync(renamed, dir);
    const override = {...defaults, app:{id:'org.custom.game',name:'Custom'}};
    delete override._generated;
    writeFileSync(resolve(dir,'app.json'),JSON.stringify(override));
    writeFileSync(resolve(dir,'logic/Cargo.toml'),'authored dependencies\n');
    assert.deepEqual(gameDefaults(dir), override);
    assert.equal(readFileSync(resolve(dir,'logic/Cargo.toml'),'utf8'),'authored dependencies\n');
  } finally {rmSync(root,{recursive:true,force:true});}
});

 test('authored Cargo package survives a generated app directory rename', async () => {
  const {gameDefaults} = await import('./app/shells.mjs');
  const root=mkdtempSync(resolve(tmpdir(),'r6-rename-')), before=resolve(root,'before'), after=resolve(root,'after');
  try {
    mkdirSync(resolve(before,'logic/src'),{recursive:true});
    writeFileSync(resolve(before,'logic/src/lib.rs'), `impl Game for Demo { const ID: &'static str = "demo"; }`);
    gameDefaults(before);
    const cargo='[package]\nname = "authored-logic"\n';
    writeFileSync(resolve(before,'logic/Cargo.toml'),cargo);
    renameSync(before,after);
    assert.equal(gameDefaults(after).game.crate,'authored-logic');
    assert.equal(readFileSync(resolve(after,'logic/Cargo.toml'),'utf8'),cargo);
  } finally {rmSync(root,{recursive:true,force:true});}
});


test('title-only and audio-only overrides retain derived defaults', async () => {
  const {gameDefaults} = await import('./app/shells.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 's4-overrides-'));
  try {
    mkdirSync(resolve(root,'logic/src'), {recursive:true});
    const rust = (id, type) => `impl Game for ${type} { const ID: &'static str = "${id}"; }`;
    writeFileSync(resolve(root,'logic/src/lib.rs'), rust('lanterns','Lanterns'));
    writeFileSync(resolve(root,'app.json'), JSON.stringify({name:'Lanterns'}));
    const title = gameDefaults(root);
    assert.equal(title.name, 'Lanterns');
    assert.equal(title.app.name, 'Lanterns');
    assert.equal(title.host.macos.window.width, 1280);
    writeFileSync(resolve(root,'logic/src/lib.rs'), rust('lanterns-next','Next'));
    assert.equal(gameDefaults(root).game.type, 'Next');
    assert.equal(gameDefaults(root).app.id, 'com.exact.lanterns-next');
    writeFileSync(resolve(root,'app.json'), JSON.stringify({game:{audio:true}}));
    const audio = gameDefaults(root);
    assert.equal(audio.game.audio, true);
    assert.equal(audio.game.type, 'Next');
    assert.equal(audio.host.ios.minimumOS, '17.0');
    const before = readFileSync(resolve(root,'app.json'),'utf8');
    gameDefaults(root);
    assert.equal(readFileSync(resolve(root,'app.json'),'utf8'), before);
    assert.deepEqual(JSON.parse(before), {game:{audio:true}}, 'the author owns only authored keys');
  } finally { rmSync(root, {recursive:true, force:true}); }
});

test.skipIf(process.platform !== 'darwin')('generation inside an empty author directory needs no writes outside it', async () => {
  const parent=realpathSync(mkdtempSync(resolve(tmpdir(),'r11-author-'))), app=resolve(parent,'my-game');
  mkdirSync(app);
  try {
    const before = treeHash(parent, app);
    const policy=`(version 1)(allow default)(deny file-write* (subpath "/"))(allow file-write* (subpath ${JSON.stringify(app)}) (literal "/dev/null"))`;
    const child=Bun.spawn(['/usr/bin/sandbox-exec','-p',policy,process.execPath,resolve(import.meta.dir,'new.mjs'),'.'],{cwd:app,env:process.env,stdout:'pipe',stderr:'pipe'});
    const [status,stderr]=await Promise.all([child.exited,new Response(child.stderr).text()]);
    assert.equal(status,0,stderr);
    assert.equal(treeHash(parent, app), before);
    assert.ok(!existsSync(resolve(app,'.shells')));
    assert.ok(!existsSync(resolve(app,'Cargo.toml')),'workspace scaffolding is generated and ignored');
    assert.ok(existsSync(resolve(app,'Cargo.lock')), 'the template source lock is copied inside the author directory');
  } finally { rmSync(parent,{recursive:true,force:true}); }
},60000);


test('R12 copied workspace owns its logic as a member', async () => {
  const {gameDefaults, gameShells} = await import('./app/shells.mjs');
  const dir=mkdtempSync(resolve(tmpdir(),'r12-copy-'));
  try {
    cpSync(resolve(import.meta.dir,'new'),resolve(dir,'new'),{recursive:true});
    cpSync(resolve(import.meta.dir,'Cargo.toml'),resolve(dir,'Cargo.toml'));
    createGame('added',dir);
    const app=resolve(dir,'games/added');
    const logic=Bun.TOML.parse(readFileSync(resolve(app,'logic/Cargo.toml'),'utf8'));
    assert.equal(resolve(app,'logic',logic.dependencies['exact-game'].path),resolve(dir,'engine'));
    gameShells(app,gameDefaults(app,dir).game,dir);
    const workspace=Bun.TOML.parse(readFileSync(resolve(app,'.shells/Cargo.toml'),'utf8'));
    assert.ok(workspace.workspace.members.includes('../logic'));
    assert.equal(Bun.TOML.parse(readFileSync(resolve(app,'logic/Cargo.toml'),'utf8')).package.workspace,'../.shells');
  } finally {rmSync(dir,{recursive:true,force:true});}
});

function treeHash(root, excluded) {
  const hash=createHash('sha256');
  const walk=dir=>{
    for(const entry of readdirSync(dir,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))) {
      const path=resolve(dir,entry.name);
      if(path===excluded) continue;
      const stat=lstatSync(path);
      hash.update(path.slice(root.length)).update(String(stat.mode));
      if(entry.isDirectory()) walk(path);
      else if(entry.isSymbolicLink()) hash.update(readlinkSync(path));
      else hash.update(readFileSync(path));
    }
  };
  walk(root); return hash.digest('hex');
}


test('R12 tree hash detects outside bytes and empty directories, excluding only the new game',()=>{
  const root=mkdtempSync(resolve(tmpdir(),'r12-tree-')),app=resolve(root,'new-game'),file=resolve(root,'outside');
  try {
    mkdirSync(app);writeFileSync(file,'before');
    const before=treeHash(root,app), stamp=statSync(file);
    writeFileSync(resolve(app,'allowed'),'allowed');assert.equal(treeHash(root,app),before);
    writeFileSync(file,'after!');utimesSync(file,stamp.atime,stamp.mtime);
    assert.notEqual(treeHash(root,app),before);
    const edited=treeHash(root,app);mkdirSync(resolve(root,'outside-directory'));
    assert.notEqual(treeHash(root,app),edited);
  } finally {rmSync(root,{recursive:true,force:true});}
});


test('R13 generation hash ignores touch but sees modes and symlink targets; writes through directory symlinks are not seen',()=>{
  const root=mkdtempSync(resolve(tmpdir(),'r13-tree-')), outside=mkdtempSync(resolve(tmpdir(),'r13-outside-'));
  try {
    const file=resolve(root,'source');writeFileSync(file,'same');const before=treeHash(root);
    utimesSync(file,new Date(),new Date(Date.now()+10000));assert.equal(treeHash(root),before);
    chmodSync(file,0o755);assert.notEqual(treeHash(root),before);
    symlinkSync(outside,resolve(root,'link'));const linked=treeHash(root);
    writeFileSync(resolve(outside,'file'),'outside');assert.equal(treeHash(root),linked);
    rmSync(resolve(root,'link'));symlinkSync('source',resolve(root,'link'));assert.notEqual(treeHash(root),linked);
  } finally {rmSync(root,{recursive:true,force:true});rmSync(outside,{recursive:true,force:true});}
});


test('R13 two copies of a new game bake offline locked with byte-identical captured locks',async()=>{
  const {prepareGame,gameDefaults}=await import('./app/shells.mjs');
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'r13-lock-copies-')));
  try {
    const a=resolve(root,'a/same-game'), b=resolve(root,'b/same-game');
    createGame(a);createGame(b);
    const before=readFileSync(resolve(a,'Cargo.lock'),'utf8');
    for(const dir of [a,b]) prepareGame(dir,gameDefaults(dir).game);
    assert.equal(readFileSync(resolve(a,'Cargo.lock'),'utf8'),before);
    assert.equal(readFileSync(resolve(b,'Cargo.lock'),'utf8'),before);
  } finally {rmSync(root,{recursive:true,force:true});}
});


test('R13 an empty Cargo cache refuses offline resolution with an explicit prefetch command',async()=>{
  const {prepareGame,gameDefaults}=await import('./app/shells.mjs');
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'r13-empty-cache-'))),dir=resolve(root,'cache-game');
  const previous=process.env.CARGO_HOME;
  try {
    createGame(dir);mkdirSync(resolve(root,'cargo-home'));process.env.CARGO_HOME=resolve(root,'cargo-home');
    const lock=readFileSync(resolve(dir,'Cargo.lock'),'utf8');
    assert.throws(()=>prepareGame(dir,gameDefaults(dir).game),error=>/offline/.test(error.message)&&/cargo fetch --locked --manifest-path/.test(error.message));
    assert.equal(readFileSync(resolve(dir,'Cargo.lock'),'utf8'),lock);
  } finally {if(previous===undefined) delete process.env.CARGO_HOME;else process.env.CARGO_HOME=previous;rmSync(root,{recursive:true,force:true});}
});


test('E10 compact authored manifest survives two bakes byte for byte', async () => {
  const {gameDefaults,gameShells}=await import('./app/shells.mjs');
  const parent=realpathSync(mkdtempSync(resolve(tmpdir(),'e10-manifest-'))), app=resolve(parent,'my-game');
  try {
    createGame(app);
    const path=resolve(app,'app.json');
    const authored=JSON.stringify({app:{name:'My Game',id:'org.example.my-game'},game:{crate:'my-game-logic',type:'SmallGame'},host:{macos:{window:{width:960}}}},null,2)+'\n';
    writeFileSync(path,authored);
    for(let i=0;i<2;i++) {
      const manifest=gameDefaults(app);
      gameShells(app,manifest.game,import.meta.dir);
      assert.equal(readFileSync(path,'utf8'),authored);
      const resolved=JSON.parse(readFileSync(resolve(app,'.shells/app.json'),'utf8'));
      assert.equal(resolved.app.id,'org.example.my-game');
      assert.equal(resolved.name,'My Game');
      assert.equal(resolved.host.macos.window.width,960);
      assert.equal(resolved.host.macos.window.height,720);
    }
  } finally {rmSync(parent,{recursive:true,force:true});}
});


test('D6 host shells discover arguments from the GPU declaration, never gameplay', async () => {
  const {gameDefaults,gameShells}=await import('./app/shells.mjs');
  const parent=realpathSync(mkdtempSync(resolve(tmpdir(),'d6-shells-'))), app=resolve(parent,'my-game');
  try {
    createGame(app);
    gameShells(app,gameDefaults(app).game,import.meta.dir);
    for (const name of readdirSync(resolve(app,'.shells')).filter(n=>/-web$|-apple$|-linux$/.test(n))) {
      const shell=resolve(app,'.shells',name);
      const cargo=Bun.TOML.parse(readFileSync(resolve(shell,'Cargo.toml'),'utf8'));
      assert.equal(cargo['build-dependencies']['game-logic'],undefined);
      assert.match(readFileSync(resolve(shell,'build.rs'),'utf8'),/bake_declaration/);
    }
  } finally {rmSync(parent,{recursive:true,force:true});}
});

test('R15 Beacons compact manifest survives two shell bakes', async () => {
  const {gameDefaults,gameShells}=await import('./app/shells.mjs');
  const app=resolve(import.meta.dir,'games/beacons'), path=resolve(app,'app.json');
  const authored=readFileSync(path,'utf8');
  assert.ok(!authored.includes('_generated'));
  assert.ok(Object.keys(JSON.parse(authored)).length <= 2);
  for(let i=0;i<2;i++) {
    const manifest=gameDefaults(app);gameShells(app,manifest.game,import.meta.dir);
    assert.equal(readFileSync(path,'utf8'),authored);
    assert.equal(JSON.parse(readFileSync(resolve(app,'.shells/app.json'),'utf8')).app.id,'com.exact.beacons');
  }
});
test('R15 explicit crate and type without an inferred declaration require identity',async()=>{
  const {gameDefaults}=await import('./app/shells.mjs');
  const parent=realpathSync(mkdtempSync(resolve(tmpdir(),'r15-identity-'))),app=resolve(parent,'odd-game');
  try {
    createGame(app);writeFileSync(resolve(app,'logic/src/lib.rs'),'pub use elsewhere::Game;');
    writeFileSync(resolve(app,'app.json'),JSON.stringify({game:{crate:'odd-game-logic',type:'Game'}}));
    assert.throws(()=>gameDefaults(app),/explicit.*id.*name/);
    writeFileSync(resolve(app,'app.json'),JSON.stringify({id:'org.example.odd',name:'Odd',game:{crate:'odd-game-logic',type:'Game'}}));
    assert.equal(gameDefaults(app).app.id,'org.example.odd');
  } finally {rmSync(parent,{recursive:true,force:true});}
});
