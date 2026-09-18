import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, rmSync, mkdtempSync, mkdirSync, writeFileSync, statSync, utimesSync, renameSync } from 'node:fs';
import { resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createGame } from './new.mjs';

// Exercise the copied files unchanged, including their manifests and shared bake.
test('a newly generated game builds, tests and proves without editing', async () => {
  const name = `new-proof-${process.pid}`, app = resolve(import.meta.dir, 'games', name);
  const env = {...process.env, EXACT_UPDATE_TRUST:'development'};
  delete env.EXACT_APP_DIR;
  delete env.CARGO_TARGET_DIR;
  const run = (command, args, cwd = resolve(import.meta.dir, '..')) => new Promise((ok, fail) => {
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
    child.on('close', (code, signal) => { clearTimeout(timer); code === 0 ? ok() : fail(new Error(`${command} exited ${code ?? signal}`)); });
  });
  assert.ok(!existsSync(app));
  const lockfile = resolve(import.meta.dir, 'Cargo.lock'), originalLock = readFileSync(lockfile);
  let registeredLock;
  let shellDirs = [], locatedDir;
  try {
    await run('bun', ['game/new.mjs', name]);
    assert.deepEqual(readdirSync(app).sort(), ['app.contract','app.json','logic','pins.json','proof.mjs']);
    rmSync(resolve(app, 'app.json'));
    rmSync(resolve(app, 'logic/Cargo.toml'));
    env.EXACT_APP_DIR = app;
    await run('bun', ['-e', 'import {resolveApp} from "./scripts/app.mjs"; resolveApp();']);
    registeredLock = readFileSync(lockfile);
    for (const file of ['Cargo.toml','Cargo.lock','web','apple','gpu']) assert.ok(!existsSync(resolve(app, file)), file);
    // The child's stdout arrives empty under `bun test` (exit 0, nothing written);
    // the located shells travel through a file instead.
    locatedDir = mkdtempSync(resolve(tmpdir(), 'new-proof-'));
    const locatedFile = resolve(locatedDir, 'shells.json');
    const located = spawnSync('bun', ['-e', `import {resolveApp} from "./scripts/app.mjs"; import {dirname} from "node:path"; import {writeFileSync} from "node:fs"; const app=resolveApp(); writeFileSync(${JSON.stringify(locatedFile)}, JSON.stringify(["gpu","web","apple","linux"].map(kind=>dirname(app.cargoPackage(kind).manifest_path))));`], {cwd:resolve(import.meta.dir,'..'), env, encoding:'utf8'});
    assert.equal(located.status, 0, located.stderr);
    assert.ok(existsSync(locatedFile), 'the shell-location child wrote nothing');
    shellDirs = JSON.parse(readFileSync(locatedFile, 'utf8'));
    assert.equal(shellDirs.length, 4, 'four shells located');
    const shellFiles = shellDirs.flatMap((dir, index) => ['Cargo.toml',index === 3 ? 'src/main.rs' : 'src/lib.rs', ...(index === 0 ? [] : ['build.rs'])].map(file=>resolve(dir,file)));
    const stamps = shellFiles.map(file => statSync(file).mtimeMs);
    await run('bun', ['-e', 'import {resolveApp} from "./scripts/app.mjs"; resolveApp();']);
    assert.deepEqual(shellFiles.map(file => statSync(file).mtimeMs), stamps, 'resolving again leaves Cargo inputs untouched');
    await run('cargo', ['test', '-p', `${name}-logic`], import.meta.dir);
    await run('bun', [resolve(app, 'proof.mjs'), 'linux']);
    await run('bun', [resolve(app, 'proof.mjs'), 'web', '--screenshot-only']);
    assert.match(readFileSync(resolve(app, 'artifacts/proof.txt'), 'utf8'), new RegExp(`PROOF PASS ${name} web: 0 failures`));
  } finally {
    rmSync(app, {recursive:true, force:true});
    if (locatedDir) rmSync(locatedDir, {recursive:true, force:true});
    for (const dir of shellDirs) rmSync(dir, {recursive:true, force:true});
    if (registeredLock) assert.deepEqual(readFileSync(lockfile), registeredLock, 'shared lockfile changed during test');
    writeFileSync(lockfile, originalLock);
  }
}, 600000);

// Run the real generator and Cargo against a tiny offline workspace.
test('generator preserves an already locked workspace and registers only missing packages', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'game-new-lock-'));
  const cargo = Bun.which('cargo');
  const run = args => {
    const result = spawnSync(cargo, args, {cwd:dir, encoding:'utf8', timeout:60000});
    assert.equal(result.status, 0, result.stderr);
  };
  try {
    mkdirSync(resolve(dir, 'new/logic/src'), {recursive:true});
    mkdirSync(resolve(dir, 'games/existing/logic/src'), {recursive:true});
    const manifest = name => `[package]\nname = "${name}"\nversion = "0.1.0"\nedition = "2021"\n`;
    writeFileSync(resolve(dir, 'Cargo.toml'), '[workspace]\nmembers = ["games/*/logic"]\nresolver = "2"\n');
    writeFileSync(resolve(dir, 'new/logic/Cargo.toml'), manifest('small-game-logic'));
    writeFileSync(resolve(dir, 'new/logic/src/lib.rs'), 'pub fn game() {}\n');
    writeFileSync(resolve(dir, 'games/existing/logic/Cargo.toml'), manifest('exact-game'));
    writeFileSync(resolve(dir, 'games/existing/logic/src/lib.rs'), 'pub fn existing() {}\n');
    run(['generate-lockfile', '--offline']);
    const first = createGame('added', dir);
    assert.ok(existsSync(resolve(dir, 'games/added/logic/Cargo.toml')), `generator made no game: ${first}`);
    run(['metadata', '--locked', '--offline', '--format-version', '1']);
    const locked = readFileSync(resolve(dir, 'Cargo.lock'));
    rmSync(resolve(dir, 'games/added'), {recursive:true});
    utimesSync(resolve(dir, 'Cargo.lock'), 1, 1);
    const modified = statSync(resolve(dir, 'Cargo.lock')).mtimeMs;
    const commands = [];
    const second = createGame('added', dir, (command, args, options) => {
      commands.push(args[0]);
      return spawnSync(command, args, options);
    });
    assert.deepEqual(commands, ['metadata'], 'an already locked game must only run a read-only check');
    assert.deepEqual(readFileSync(resolve(dir, 'Cargo.lock')), locked);
    assert.equal(statSync(resolve(dir, 'Cargo.lock')).mtimeMs, modified);
    assert.match(first, /lockfile.*registered/i);
    assert.match(second, /lockfile.*unchanged/i);
    rmSync(resolve(dir, 'Cargo.lock'));
    assert.match(createGame('another', dir), /generate-lockfile/);
    run(['metadata', '--locked', '--offline', '--format-version', '1']);
  } finally { rmSync(dir, {recursive:true, force:true}); }
}, 180000);

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
    assert.deepEqual(JSON.parse(before)._generated.overrides, {game:{audio:true}}, 'the author owns only authored keys');
  } finally { rmSync(root, {recursive:true, force:true}); }
});
