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
    // A game is the files its author writes; no manifest, lock or app.json. Outside
    // this checkout it gets an app's runner and notes too (LLP 1086).
    assert.deepEqual(readdirSync(app).sort(), ['.gitignore','AGENTS.md','CLAUDE.md','README.md','app.contract','app.test.contract','exact.mjs','logic','pins.json','proof.mjs']);
    assert.deepEqual(readdirSync(resolve(app, 'logic')).sort(), ['src','tests']);
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
    const capture = JSON.parse(readFileSync(resolve(app, 'artifacts/web/replies.json'), 'utf8'));
    assert.equal(new Set(capture.map(reply => reply.session)).size, 1, 'a capture opens one document');
    assert.ok(!capture.some(reply => ['type','clock'].includes(reply.method)), 'a capture skips movement');
    assert.ok(existsSync(resolve(app, 'artifacts/web/game.png')), 'a capture writes its image');
    const receipt = resolve(app, 'artifacts/build-linux-gpu.sha256');
    const hostReceipt = resolve(app, 'artifacts/build-linux-host.sha256');
    const before = JSON.parse(readFileSync(receipt, 'utf8'));
    const hostBefore = readFileSync(hostReceipt, 'utf8');
    const source = resolve(app, 'logic/src/lib.rs');
    writeFileSync(source, readFileSync(source, 'utf8').replace('[2.0, 6.0]', '[2.0, 6.0, 10.0]') + '\n// External app edits must invalidate the proof build.\n');
    await run('bun', [resolve(app, 'proof.mjs'), 'linux', '--build-only']);
    env.EXACT_PROOF_REPIN = '1';
    await run('bun', [resolve(app, 'proof.mjs'), 'linux']);
    delete env.EXACT_PROOF_REPIN;
    const expanded = JSON.parse(readFileSync(resolve(app, 'artifacts/linux/summary.json'), 'utf8'));
    assert.deepEqual(expanded.failures, [], 'adding a third beacon must update victory without a Contract edit');
    assert.match(readFileSync(resolve(app, 'artifacts/linux/proof.txt'), 'utf8'), /victory waits for every beacon \(3\/3\)/);
    assert.notEqual(JSON.parse(readFileSync(receipt, 'utf8')).inputs, before.inputs);
    assert.equal(readFileSync(hostReceipt, 'utf8'), hostBefore, 'a logic edit retains the completed host');
    assert.deepEqual(readdirSync(app).filter(file => !['.shells','artifacts','target','dist','dist.previous','app.contract.d.ts'].includes(file)).sort(),
      ['.gitignore','AGENTS.md','CLAUDE.md','README.md','app.contract','app.test.contract','exact.mjs','logic','pins.json','proof.mjs'], 'bakes and proofs write only ignored outputs');
  } finally {
    rmSync(directory, {recursive:true, force:true});
  }
}, 600000);

test('generated commands run from an external author directory with quoted paths', async () => {
  const directory=realpathSync(mkdtempSync(resolve(tmpdir(), "game commands' workspace-")));
  const app=resolve(directory,'outside-game');mkdirSync(app);
  const quote=value=>`'${value.replaceAll("'", "'\\''")}'`;
  try {
    const created=spawnSync(process.execPath,[resolve(import.meta.dir,'new.mjs'),'.'],{cwd:app,encoding:'utf8'});
    assert.equal(created.status,0,created.stderr);
    // Its commands are its own runner's verbs, as an app's are (LLP 1086; the platformer's diary, R1).
    const verbs=created.stdout.split('\n').filter(line=>line.startsWith('  bun exact.mjs ')).map(line=>line.trim().split(/\s+/)[2]);
    assert.deepEqual(verbs,['test-rust','web','test','agent','mac','windows','prove'],created.stdout);
    const runner=readFileSync(resolve(app,'exact.mjs'),'utf8');
    for (const verb of verbs) assert.ok(new RegExp(`^  '?${verb}'?: \\[`,'m').test(runner),verb);
    assert.match(readFileSync(resolve(app,'AGENTS.md'),'utf8'),/<!-- exact:begin[^]*an Exact game[^]*game[\\/]README\.md[^]*## The authoring diary[^]*<!-- exact:end -->/);
    const inspect=resolve(directory,'inspect command.mjs');
    writeFileSync(inspect,`import {readFileSync} from 'node:fs'; import {resolve} from 'node:path';
      const [script,...args]=process.argv.slice(2); const path=resolve(script); readFileSync(path);
      console.log(JSON.stringify({path,args}));`);
    const run=async command=>{
      const raw=command.replace(/^bun /,`${quote(process.execPath)} ${quote(inspect)} `);
      const result=await Bun.$`${{raw}}`.cwd(app).quiet().nothrow();
      assert.equal(result.exitCode,0,`${command}\n${result.stderr}`);
      return JSON.parse(result.stdout.toString());
    };
    // Execute the generated Windows verb against a recording SDK, without
    // building or opening a host. Argument and app-path forwarding are real.
    const sdk=resolve(directory,'recording SDK');
    mkdirSync(resolve(sdk,'host/windows'),{recursive:true});
    writeFileSync(resolve(sdk,'host/windows/build.mjs'),`console.log(JSON.stringify({args:process.argv.slice(2),app:process.env.EXACT_APP_DIR}));`);
    const windows=spawnSync(process.execPath,['exact.mjs','windows','--release','--run'],{cwd:app,env:{...process.env,EXACT2:sdk},encoding:'utf8'});
    assert.equal(windows.status,0,windows.stderr);
    const forwarded=JSON.parse(windows.stdout);
    assert.deepEqual(forwarded.args,['outside-game','--release','--run']);
    assert.equal(realpathSync.native(forwarded.app),realpathSync.native(app));
    const readme=readFileSync(resolve(app,'README.md'),'utf8');
    assert.ok(!readme.includes('/path/to/exact2'),readme);
    const documented=[...readme.matchAll(/`(bun [^`]+)`/g)].map(match=>match[1]);
    const resolved=await Promise.all(documented.map(run));
    assert.ok(resolved.some(command=>command.path===resolve(import.meta.dir,'prove.mjs')));
    assert.ok(resolved.some(command=>command.path===resolve(import.meta.dir,'app/shells.mjs')));
  } finally {rmSync(directory,{recursive:true,force:true});}
});

test('ordinary cargo fmt accepts baked adapters for short and long paths', async () => {
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'game shell-format-')));
  const apps=[
    resolve(root,'normal-game'),
    resolve(root,'a deliberately long external author path','another nested directory','long-generated-adapter-game'),
  ];
  try {
    for (const [index,app] of apps.entries()) {
      createGame(app);
      if (index === 1) mkdirSync(resolve(app,'art'));
      const baked=spawnSync(process.execPath,[resolve(import.meta.dir,'app/shells.mjs'),app],{encoding:'utf8'});
      assert.equal(baked.status,0,baked.stderr);
      const formatted=spawnSync('cargo',['fmt','--all','--','--check'],{
        cwd:resolve(app,'.shells'),encoding:'utf8',env:{...process.env},
      });
      assert.equal(formatted.status,0,formatted.stderr || formatted.stdout);
    }
    const workspace=resolve(root,'short-sdk'), short=resolve(workspace,'games/short-game');
    mkdirSync(resolve(workspace,'bake/src'),{recursive:true});
    cpSync(resolve(import.meta.dir,'Cargo.toml'),resolve(workspace,'Cargo.toml'));
    cpSync(resolve(import.meta.dir,'bake/src/files.rs'),resolve(workspace,'bake/src/files.rs'));
    createGame(short);
    const {gameDefaults,gameShells}=await import('./app/shells.mjs');
    gameShells(short,gameDefaults(short).game,workspace);
    const build=resolve(short,'.shells/gpu/build.rs');
    assert.match(readFileSync(build,'utf8'),/"\.\.\/\.\.\/\.\.\/\.\.\/bake\/src\/files\.rs"/);
    const formatted=spawnSync('rustfmt',['--edition','2021','--check',build],{
      encoding:'utf8',env:{...process.env},
    });
    assert.equal(formatted.status,0,formatted.stderr || formatted.stdout);
  } finally {rmSync(root,{recursive:true,force:true});}
},30000);

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
    assert.match(createGame('added',dir), /proof\.mjs'? web --screenshot-only$/);
    assert.equal(treeHash(dir, resolve(dir,'games/added')), before);
    assert.equal(readFileSync(resolve(dir,'Cargo.lock'),'utf8'),'shared lock sentinel');
    assert.equal(statSync(resolve(dir,'Cargo.lock')).mtimeMs,stamp);
    assert.ok(!existsSync(resolve(dir,'games/existing/app.json')));
    assert.ok(!existsSync(resolve(dir,'.shells')));
    assert.ok(!existsSync(resolve(dir,'games/added/.shells')));
  } finally { rmSync(dir,{recursive:true,force:true}); }
});

test('shell test CLI honors an app path and keeps the no-path SDK sweep', () => {
  const parent = realpathSync(mkdtempSync(resolve(tmpdir(), 'game test scope-')));
  const sdk = resolve(parent, 'sdk'), external = resolve(parent, 'external-game');
  const calls = resolve(parent, 'cargo-calls.jsonl'), bin = resolve(parent, 'bin');
  try {
    for (const dir of ['app', '.cargo', 'games', 'bench']) mkdirSync(resolve(sdk, dir), {recursive:true});
    mkdirSync(bin);
    for (const file of ['Cargo.toml', '.cargo/config.toml', 'app/shells.mjs', 'app/shells.lock'])
      cpSync(resolve(import.meta.dir, file), resolve(sdk, file));
    cpSync(resolve(import.meta.dir, '../rust-toolchain.toml'), resolve(parent, 'rust-toolchain.toml'));
    cpSync(resolve(import.meta.dir, 'new'), resolve(sdk, 'new'), {recursive:true});
    const fixture = resolve(sdk, 'games/fixture'), bench = resolve(sdk, 'bench/example');
    for (const app of [external, fixture, bench]) {
      createGame(app, sdk);
      mkdirSync(resolve(app, 'art'));
    }
    // Only Cargo is substituted: exercise the real CLI, manifest resolution,
    // shell generation and failure aggregation without compiling unrelated apps.
    const cargo = resolve(bin, 'cargo');
    writeFileSync(cargo, `#!${process.execPath}\nimport {appendFileSync} from 'node:fs';
const args=process.argv.slice(2);
appendFileSync(process.env.SCOPE_CALLS, JSON.stringify({cwd:process.cwd(),args,target:process.env.CARGO_TARGET_DIR,sentinel:process.env.SCOPE_SENTINEL})+'\\n');
if (args[0] === 'metadata') console.log('{}');
if (args[0] === 'run' && args.at(-1) === process.env.SCOPE_BAKE_FAIL) process.exit(8);
if (args[0] === 'test' && process.cwd() === process.env.SCOPE_TEST_FAIL) process.exit(7);\n`);
    chmodSync(cargo, 0o755);
    const target = resolve(parent, 'target with spaces');
    const run = (args, {bakeFail='', testFail=resolve(fixture, '.shells')} = {}) => {
      writeFileSync(calls, '');
      const result = spawnSync(process.execPath, [resolve(sdk, 'app/shells.mjs'), ...args], {
        cwd:parent, encoding:'utf8', env:{...process.env, PATH:`${bin}:${process.env.PATH}`,
          CARGO_TARGET_DIR:target, SCOPE_CALLS:calls, SCOPE_BAKE_FAIL:bakeFail,
          SCOPE_TEST_FAIL:testFail, SCOPE_SENTINEL:'carried'},
      });
      const commands = readFileSync(calls, 'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
      return {result, commands, bakes:commands.filter(command => command.args[0] === 'run'),
        tests:commands.filter(command => command.args[0] === 'test')};
    };
    const selected = run([external, '--test']);
    assert.equal(selected.result.status, 0, selected.result.stderr);
    assert.deepEqual(selected.commands.map(command => command.args[0]), ['metadata','run','clippy','test']);
    assert.deepEqual(selected.bakes[0].args, ['run','--manifest-path',resolve(sdk,'Cargo.toml'),'-p',
      'exact-game-bake','--locked','--offline','--','--art',external]);
    assert.equal(selected.bakes[0].cwd, sdk);
    assert.ok(selected.commands.every(command => command.target === target && command.sentinel === 'carried'));
    assert.deepEqual(selected.tests.map(command => command.cwd), [resolve(external, '.shells')]);
    const all = run(['--test']);
    assert.equal(all.result.status, 1, 'a failing fixture must fail the sweep');
    assert.deepEqual(all.bakes.map(command => command.args.at(-1)), [fixture, bench]);
    assert.deepEqual(all.tests.map(command => command.cwd), [fixture, bench].map(app => resolve(app, '.shells')),
      'the sweep continues to the benchmark after a fixture fails');
    const bakeFailure = run(['--test'], {bakeFail:fixture, testFail:''});
    assert.equal(bakeFailure.result.status, 1, 'a failing art bake must fail the sweep');
    assert.deepEqual(bakeFailure.bakes.map(command => command.args.at(-1)), [fixture, bench]);
    assert.deepEqual(bakeFailure.tests.map(command => command.cwd), [resolve(bench, '.shells')],
      'the sweep skips the affected tests and continues after an art bake fails');
    const missing = run([resolve(parent, 'missing-game'), '--test']);
    assert.equal(missing.result.status, 1, 'an invalid explicit app must fail');
    assert.deepEqual(missing.bakes, [], 'an invalid app must not run an art bake');
    assert.deepEqual(missing.tests, [], 'an invalid app must not fall back to the SDK sweep');
  } finally {rmSync(parent, {recursive:true, force:true});}
});

test('changing app identity keeps adapter paths and the Cargo cache', async () => {
  const {gameDefaults, gameShells} = await import('./app/shells.mjs');
  const parent = realpathSync(mkdtempSync(resolve(tmpdir(), 'game-cache-'))), app = resolve(parent, 'my-game');
  try {
    createGame(app);
    gameShells(app, gameDefaults(app).game, import.meta.dir);
    const root = resolve(app, '.shells');
    const adapters = ['gpu','web','apple','linux'].map(name => resolve(root, name, 'Cargo.toml'));
    const stamps = adapters.map(path => statSync(path).mtimeMs);
    const cache = resolve(root, 'target/debug/warm-artifact');
    mkdirSync(resolve(cache, '..'), {recursive:true});
    writeFileSync(cache, 'completed Cargo output');
    const manifest = gameDefaults(app);
    delete manifest._generated;
    manifest.app.id = 'org.example.renamed';
    writeFileSync(resolve(app, 'app.json'), JSON.stringify(manifest));
    gameShells(app, manifest.game, import.meta.dir);
    assert.equal(readFileSync(cache, 'utf8'), 'completed Cargo output');
    assert.deepEqual(adapters.map(path => statSync(path).mtimeMs), stamps);
    assert.equal(JSON.parse(readFileSync(resolve(root, 'app.json'), 'utf8')).app.id, 'org.example.renamed');
  } finally { rmSync(parent, {recursive:true, force:true}); }
});

test('default manifests derive the crate from Game::ID and the title from the directory; authored overrides win', async () => {
  const {gameDefaults} = await import('./app/shells.mjs');
  const root = mkdtempSync(resolve(tmpdir(), 'e5-defaults-')), dir = resolve(root,'runner-game');
  try {
    mkdirSync(resolve(dir,'logic/src'), {recursive:true});
    writeFileSync(resolve(dir,'logic/src/lib.rs'), `pub struct MyGame; impl Game for MyGame { const ID: &'static str = "stable-save-id"; }`);
    const defaults = gameDefaults(dir);
    assert.equal(defaults.app.id, 'com.exact.stable-save-id');
    assert.equal(defaults.name, 'Runner Game');
    assert.equal(defaults.game.crate, 'stable-save-id-logic');
    assert.equal(defaults.game.type, 'MyGame');
    assert.deepEqual(readdirSync(dir), ['logic'], 'deriving writes nothing');
    // A clone under another directory name keeps its crate, so its tests still link.
    const renamed = resolve(root, 'renamed-game');
    renameSync(dir, renamed);
    assert.equal(gameDefaults(renamed).game.crate, 'stable-save-id-logic');
    writeFileSync(resolve(renamed,'logic/src/lib.rs'), `pub struct NewGame; impl Game for NewGame { const HZ: u32 = 120; const ID: &'static str = "new-save-id"; }`);
    const refreshed = gameDefaults(renamed);
    assert.equal(refreshed.game.type, 'NewGame');
    assert.equal(refreshed.app.id, 'com.exact.new-save-id');
    renameSync(renamed, dir);
    const override = {app:{id:'org.custom.game',name:'Custom'}};
    writeFileSync(resolve(dir,'app.json'),JSON.stringify(override));
    writeFileSync(resolve(dir,'logic/Cargo.toml'),'[package]\nname = "authored-logic"\n');
    assert.equal(gameDefaults(dir).app.id, 'org.custom.game');
    assert.equal(gameDefaults(dir).game.crate, 'authored-logic', 'an authored package names the crate');
    assert.equal(readFileSync(resolve(dir,'logic/Cargo.toml'),'utf8'),'[package]\nname = "authored-logic"\n');
  } finally {rmSync(root,{recursive:true,force:true});}
});

test('an authored logic manifest is the member and is never rewritten', async () => {
  const {gameDefaults, gameShells} = await import('./app/shells.mjs');
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'manifest-owner-'))), app=resolve(root,'my-game');
  try {
    createGame(app);
    const path=resolve(app,'logic/Cargo.toml');
    const authored='[package]\nname = "my-game-logic"\nversion = "0.1.0"\nedition = "2021"\nworkspace = "../.shells"\n\n[dependencies]\nexact-game.workspace = true\nexact-game-physics.workspace = true\n';
    writeFileSync(path,authored);
    for(let i=0;i<2;i++) {
      gameShells(app,gameDefaults(app).game,import.meta.dir);
      assert.equal(readFileSync(path,'utf8'),authored);
      assert.ok(Bun.TOML.parse(readFileSync(resolve(app,'.shells/Cargo.toml'),'utf8')).workspace.members.includes('../logic'));
      assert.ok(!existsSync(resolve(app,'.shells/logic')), 'no generated member beside an authored one');
    }
    writeFileSync(path,authored.replace('workspace = "../.shells"\n',''));
    assert.throws(()=>gameShells(app,gameDefaults(app).game,import.meta.dir),/set package.workspace = "..\/.shells"/);
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
    for (const file of ['Cargo.toml','Cargo.lock','app.json','logic/Cargo.toml'])
      assert.ok(!existsSync(resolve(app,file)),`${file}: derived, never written into the author directory`);
  } finally { rmSync(parent,{recursive:true,force:true}); }
},60000);


test('a game without logic/Cargo.toml owns a generated member over its own sources', async () => {
  const {gameDefaults, gameShells} = await import('./app/shells.mjs');
  const dir=mkdtempSync(resolve(tmpdir(),'r12-copy-'));
  try {
    cpSync(resolve(import.meta.dir,'new'),resolve(dir,'new'),{recursive:true});
    cpSync(resolve(import.meta.dir,'Cargo.toml'),resolve(dir,'Cargo.toml'));
    createGame('added',dir);
    const app=resolve(dir,'games/added');
    mkdirSync(resolve(app,'logic/examples/tour'),{recursive:true});
    writeFileSync(resolve(app,'logic/examples/tour/main.rs'),'fn main() {}\n');
    gameShells(app,gameDefaults(app).game,dir);
    const workspace=Bun.TOML.parse(readFileSync(resolve(app,'.shells/Cargo.toml'),'utf8'));
    assert.ok(workspace.workspace.members.includes('logic'));
    const logic=Bun.TOML.parse(readFileSync(resolve(app,'.shells/logic/Cargo.toml'),'utf8'));
    assert.equal(logic.package.name,'added-logic');
    assert.equal(resolve(app,'.shells/logic',logic.lib.path),resolve(app,'logic/src/lib.rs'));
    assert.deepEqual(logic.test.map(t=>[t.name,resolve(app,'.shells/logic',t.path)]),[['sim',resolve(app,'logic/tests/sim.rs')]]);
    assert.deepEqual(logic.example.map(t=>t.name),['tour']);
    assert.deepEqual(logic.dependencies,{'exact-game':{workspace:true}});
    assert.equal(resolve(app,'.shells',workspace.workspace.dependencies['exact-game'].path),resolve(dir,'engine'));
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


test('two copies of a new game derive byte-identical shell locks from the SDK lock and write none', async () => {
  const {prepareGame,gameDefaults}=await import('./app/shells.mjs');
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'r13-lock-copies-')));
  try {
    const a=resolve(root,'a/same-game'), b=resolve(root,'b/same-game');
    createGame(a);createGame(b);
    for(const dir of [a,b]) for (let i=0;i<2;i++) prepareGame(dir,gameDefaults(dir).game);
    const derived=readFileSync(resolve(a,'.shells/Cargo.lock'),'utf8');
    assert.equal(readFileSync(resolve(b,'.shells/Cargo.lock'),'utf8'),derived);
    assert.ok(!existsSync(resolve(a,'Cargo.lock')) && !existsSync(resolve(b,'Cargo.lock')));
    assert.notEqual(derived,readFileSync(resolve(import.meta.dir,'app/shells.lock'),'utf8'),'the SDK union is pruned to the game');
  } finally {rmSync(root,{recursive:true,force:true});}
}, 120000);

test('the SDK lock decides every version; a game that adds packages captures its own lock', async () => {
  const {outsideSdkLock,sdkLock}=await import('./app/shells.mjs');
  const lock=packages=>`version = 4\n\n${packages.map(([name,version,source,deps])=>`[[package]]\nname = "${name}"\nversion = "${version}"\n${source?`source = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "${source}"\n`:''}${deps?`dependencies = [\n${deps.map(d=>` "${d}",\n`).join('')}]\n`:''}`).join('\n')}`;
  const sdk=lock([['glam','0.33.7','aa',['approx','libm']],['approx','0.5.1','bb'],['libm','0.2.16','cc'],['exact-game','0.1.0',null,['glam']]]);
  const members=new Set(['x-logic','x-gpu']);
  // Fewer activated features drop edges, never versions.
  assert.deepEqual(outsideSdkLock(lock([['glam','0.33.7','aa',['libm']],['libm','0.2.16','cc'],['x-logic','0.1.0',null,['exact-game']],['exact-game','0.1.0',null,['glam']]]),sdk,members),[]);
  const added=lock([['glam','0.33.7','aa'],['itoa','1.0.15','dd'],['libm','0.2.9','ee']]);
  assert.deepEqual(outsideSdkLock(added,sdk,members),['itoa 1.0.15 registry+https://github.com/rust-lang/crates.io-index','libm 0.2.9 registry+https://github.com/rust-lang/crates.io-index']);
  assert.deepEqual(outsideSdkLock(lock([['glam','0.33.7','changed']]),sdk,members),['glam 0.33.7 registry+https://github.com/rust-lang/crates.io-index'],'a checksum is part of the identity');
  // The SDK must inherit every core patch, including host-only dependencies.
  const {outsideWorkspaceProblems} = await import('../scripts/app.mjs');
  assert.deepEqual(outsideWorkspaceProblems(import.meta.dir), []);
  const sdkCargo = Bun.TOML.parse(readFileSync(resolve(import.meta.dir, 'Cargo.toml'), 'utf8'));
  const coreCargo = Bun.TOML.parse(readFileSync(resolve(import.meta.dir, '../Cargo.toml'), 'utf8'));
  assert.deepEqual(sdkCargo.profile['host-dev'], coreCargo.profile['host-dev']);
  // The checked-in SDK lock is current for the union of every shell's dependencies.
  sdkLock();
}, 120000);

test('a root crate\'s new registry dependency or new path crate resolves before the SDK lock is refreshed', async () => {
  const {outsideSdkLock,withRootPins}=await import('./app/shells.mjs');
  const registry='registry+https://github.com/rust-lang/crates.io-index';
  const block=(name,version,checksum)=>`[[package]]\nname = "${name}"\nversion = "${version}"\n${checksum?`source = "${registry}"\nchecksum = "${checksum}"\n`:''}`;
  const lock=(...blocks)=>`version = 4\n\n${blocks.join('\n')}`;
  // 15856ff7 gave kernel cssparser 0.37.0 and updated only the root Cargo.lock.
  const sdk=lock(block('itoa','1.0.15','aa'),block('exact-kernel','0.1.0'));
  const root=lock(block('cssparser','0.37.0','bb'),block('itoa','1.0.9','cc'),block('itoa','2.0.1','ee'),block('exact-svg-filter','0.1.0'),block('exact-kernel','0.1.0'));
  const seeded=withRootPins(sdk,root), members=new Set(['x-logic']);
  // A new major of a package the SDK lock holds (itoa 2) is new to it; a compatible one is not.
  // 00d37ef9f split exact-svg-filter out of the kernel: a root path crate is the SDK's own.
  assert.deepEqual(Bun.TOML.parse(seeded).package.map(p=>`${p.name} ${p.version}`),['itoa 1.0.15','exact-kernel 0.1.0','cssparser 0.37.0','itoa 2.0.1','exact-svg-filter 0.1.0'],'only packages new to the SDK are added');
  assert.deepEqual(outsideSdkLock(lock(block('exact-svg-filter','0.1.0'),block('exact-kernel','0.1.0')),seeded,members),[]);
  assert.deepEqual(outsideSdkLock(lock(block('itoa','2.0.1','ee')),seeded,members),[]);
  assert.deepEqual(outsideSdkLock(lock(block('cssparser','0.37.0','bb'),block('itoa','1.0.15','aa')),seeded,members),[]);
  assert.deepEqual(outsideSdkLock(lock(block('cssparser','0.37.1','dd')),seeded,members),[`cssparser 0.37.1 ${registry}`],'the root lock decides the version');
  assert.deepEqual(outsideSdkLock(lock(block('itoa','1.0.9','cc')),seeded,members),[`itoa 1.0.9 ${registry}`],'the SDK lock still decides its own packages');
  assert.equal(withRootPins(sdk,''),sdk);
});


test('an empty Cargo cache permits adapter generation and refuses offline resolution',async()=>{
  const {prepareGame,gameDefaults}=await import('./app/shells.mjs');
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'r13-empty-cache-'))),dir=resolve(root,'cache-game');
  const previous=process.env.CARGO_HOME;
  try {
    createGame(dir);mkdirSync(resolve(root,'cargo-home'));process.env.CARGO_HOME=resolve(root,'cargo-home');
    const generated=spawnSync(process.execPath,[resolve(import.meta.dir,'app/shells.mjs'),dir],{env:{...process.env},encoding:'utf8'});
    assert.equal(generated.status,0,generated.stderr);
    // The shell starts from the SDK lock, so `cargo fetch` downloads the SDK's versions.
    assert.equal(readFileSync(resolve(dir,'.shells/Cargo.lock'),'utf8'),readFileSync(resolve(import.meta.dir,'app/shells.lock'),'utf8'));
    for (const host of ['gpu','web','apple','linux','logic']) assert.ok(existsSync(resolve(dir,`.shells/${host}/Cargo.toml`)));
    assert.throws(()=>prepareGame(dir,gameDefaults(dir).game),error=>/offline/.test(error.message)&&/cargo fetch --manifest-path/.test(error.message));
    assert.ok(!existsSync(resolve(dir,'Cargo.lock')));
  } finally {if(previous===undefined) delete process.env.CARGO_HOME;else process.env.CARGO_HOME=previous;rmSync(root,{recursive:true,force:true});}
});


test('lockDrift names what exact2 added and moved under a captured lock, never what the game chose', async () => {
  const {lockDrift,lockChanges}=await import('./app/shells.mjs');
  const lock=packages=>`version = 4\n\n${packages.map(([name,version,source,deps])=>`[[package]]\nname = "${name}"\nversion = "${version}"\n${source?`source = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "${source}"\n`:''}${deps?`dependencies = [\n${deps.map(d=>` "${d}",\n`).join('')}]\n`:''}`).join('\n')}`;
  // exact2's text crate moved to parley, which brings fontique and a newer harfrust;
  // another shell's rand 0.8 is in the SDK lock, and this game chose rand 0.9.
  const sdk=lock([['exact-game','0.1.0',null,['glam','parley']],['glam','0.33.7','aa',['libm']],['libm','0.2.16','bb'],['parley','0.6.0','cc',['fontique','harfrust']],['fontique','0.5.0','dd'],['harfrust','0.12.0','ee'],['rand','0.8.5','ff']]);
  const captured=lock([['x-logic','0.1.0',null,['exact-game','rand']],['exact-game','0.1.0',null,['glam','harfrust']],['glam','0.33.7','aa',['libm']],['libm','0.2.16','bb'],['harfrust','0.5.2','gg'],['rand','0.9.1','hh']]);
  assert.deepEqual(lockDrift(captured,sdk),{added:['fontique','parley'],updated:['harfrust 0.5.2 → 0.12.0']});
  assert.deepEqual(lockDrift(sdk,sdk),{added:[],updated:[]});
  // Two versions of one name: only the one exact2's crates reach is named (the game's own foo 2 is its choice).
  const two=lock([['x-logic','0.1.0',null,['exact-game','foo 2.0.0']],['exact-game','0.1.0',null,['foo 1.0.0']],['foo','1.0.0','ii'],['foo','2.0.0','jj']]);
  assert.deepEqual(lockDrift(two,lock([['exact-game','0.1.0',null,['foo']],['foo','1.1.0','kk']])).updated,['foo 1.0.0 → 1.1.0']);
  const refreshed=lock([['x-logic','0.1.0',null,['exact-game','rand']],['exact-game','0.1.0',null,['glam','parley']],['glam','0.33.7','aa',['libm']],['libm','0.2.16','bb'],['parley','0.6.0','cc',['fontique','harfrust']],['fontique','0.5.0','dd'],['harfrust','0.12.0','ee'],['rand','0.9.1','hh']]);
  assert.deepEqual(lockChanges(captured,refreshed),['harfrust 0.5.2 → 0.12.0','added parley 0.6.0','added fontique 0.5.0']);
  assert.deepEqual(lockChanges(refreshed,captured),['harfrust 0.12.0 → 0.5.2','removed parley 0.6.0','removed fontique 0.5.0']);
  // A git package that moved revision at the same version is a change too.
  const git=(rev)=>`version = 4\n\n[[package]]\nname = "tool"\nversion = "1.0.0"\nsource = "git+https://example.com/tool#${rev}"\n`;
  assert.deepEqual(lockChanges(git('aaa'),git('bbb')),['tool 1.0.0 (git+https://example.com/tool#aaa) → 1.0.0 (git+https://example.com/tool#bbb)']);
  // So is a path package becoming crates.io's at the same version.
  assert.deepEqual(lockChanges(lock([['helper','1.0.0']]),lock([['helper','1.0.0','ll']])),['helper 1.0.0 (path) → 1.0.0']);
});

test('lock seeds the SDK lock\'s versions, a same-series bump included, and keeps the game\'s own packages', async () => {
  const {lockSeed,staleLock}=await import('./app/shells.mjs');
  const block=(name,version,source)=>`[[package]]\nname = "${name}"\nversion = "${version}"\n${source?`source = "${source}"\n`:''}`;
  const lock=(...blocks)=>`version = 4\n\n${blocks.join('\n')}`;
  const crates='registry+https://github.com/rust-lang/crates.io-index';
  const sdk=lock(block('foo','0.12.1',crates),block('exact-game','0.1.0'));
  const own=lock(block('foo','0.12.0',crates),block('rand','0.9.1',crates),block('tool','1.0.0','git+https://example.com/tool#aaa'),block('x-logic','0.1.0'));
  // foo 0.12.0 → the SDK's 0.12.1 (one graph holds one version per series), not crates.io's newest.
  assert.deepEqual(Bun.TOML.parse(lockSeed(sdk,own)).package.map(p=>`${p.name} ${p.version}`),['foo 0.12.1','exact-game 0.1.0','rand 0.9.1','tool 1.0.0','x-logic 0.1.0']);
  assert.equal(lockSeed(sdk,sdk),sdk);
  // A git fork of an SDK crate, same name and series, is another package: kept at its revision.
  const fork=lock(block('foo','0.12.0','git+https://example.com/foo?branch=main#aaa'));
  assert.deepEqual(Bun.TOML.parse(lockSeed(sdk,fork)).package.map(p=>`${p.name} ${p.version} ${p.source??''}`),['foo 0.12.1 '+crates,'exact-game 0.1.0 ','foo 0.12.0 git+https://example.com/foo?branch=main#aaa']);
  // Only a lock that needs updating is stale; a crate missing from the cache is not.
  assert.ok(staleLock('error: cannot update the lock file /g/.shells/Cargo.lock because --locked was passed to prevent this'));
  assert.ok(staleLock('error: the lock file /g/.shells/Cargo.lock needs to be updated but --locked was passed to prevent this'));
  assert.ok(!staleLock('error: failed to download `rustix v1.1.5`\n\nCaused by:\n  attempting to make an HTTP request, but --offline was specified'));
});

test('a captured lock exact2 moved past is named at a build, at generation and at update, and left alone', async () => {
  const {prepareGame,gameDefaults,capturedLockDrift}=await import('./app/shells.mjs');
  const {createApp}=await import('./new.mjs');
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'lock-drift-'))), dir=resolve(root,'drift-game');
  try {
    createGame(dir);
    prepareGame(dir,gameDefaults(dir).game,undefined,{updateLock:true});
    // A current lock and a crate missing from Cargo's cache is not drift: the refusal says fetch.
    const home=process.env.CARGO_HOME;
    try {
      process.env.CARGO_HOME=resolve(root,'empty-cargo-home');mkdirSync(process.env.CARGO_HOME);
      assert.throws(()=>prepareGame(dir,gameDefaults(dir).game),error=>/^game Cargo graph: /.test(error.message)&&/cargo fetch --manifest-path/.test(error.message));
    } finally {if(home===undefined) delete process.env.CARGO_HOME;else process.env.CARGO_HOME=home;}
    const captured=Bun.TOML.parse(readFileSync(resolve(dir,'Cargo.lock'),'utf8')).package;
    // As if captured before exact2's move: exact-game's first registry dependency
    // (and its edge) absent, and another registry package an older version.
    const crate=captured.find(p=>p.name==='exact-game'), registry=n=>captured.filter(p=>p.name===n&&p.source).length===1;
    const [gone,older]=crate.dependencies.map(d=>d.split(' ')[0]).filter(registry);
    assert.ok(gone&&older,'exact-game has two registry dependencies to stand for the move');
    const text=readFileSync(resolve(dir,'Cargo.lock'),'utf8').split(/\n(?=\[\[package\]\]\n)/).filter(b=>!b.startsWith(`[[package]]\nname = "${gone}"\n`))
      .map(b=>b.startsWith('[[package]]\nname = "exact-game"\n')?b.replace(new RegExp(`\\n "${gone}( [^"]*)?",`),''):b)
      .map(b=>b.startsWith(`[[package]]\nname = "${older}"\n`)?b.replace(/^version = "[^"]*"$/m,'version = "0.0.1"'):b).join('\n');
    writeFileSync(resolve(dir,'Cargo.lock'),text);
    rmSync(resolve(dir,'.shells'),{recursive:true,force:true});
    const named=message=>message.startsWith("exact2 moved since this game's Cargo.lock was captured (added: ")
      &&message.includes(gone)&&message.includes(`updated: ${older} 0.0.1 → `)&&message.includes(`\`bun exact.mjs lock\` in ${dir}`);
    assert.throws(()=>prepareGame(dir,gameDefaults(dir).game),error=>named(error.message)&&/cargo fetch --manifest-path/.test(error.message));
    assert.equal(readFileSync(resolve(dir,'Cargo.lock'),'utf8'),text,'a build never changes a captured lock');
    assert.ok(named(capturedLockDrift(dir)));
    const generated=spawnSync(process.execPath,[resolve(import.meta.dir,'app/shells.mjs'),dir],{encoding:'utf8'});
    assert.equal(generated.status,0,generated.stderr);
    assert.ok(named(generated.stderr.replace(/^warning: /,'')),generated.stderr);
    // A runner generated before the verb existed is told to update first.
    const runner=resolve(dir,'exact.mjs');
    writeFileSync(runner,readFileSync(runner,'utf8').replace(/^\s*lock: \[.*\n/m,''));
    assert.ok(capturedLockDrift(dir).includes(`\`bun exact.mjs update\`, then \`bun exact.mjs lock\`, in ${dir}`));
    assert.match(createApp(dir,{update:true}),/exact2 moved since this game's Cargo.lock was captured/);
    assert.match(readFileSync(resolve(dir,'exact.mjs'),'utf8'),/lock: \['game\/app\/shells\.mjs', import\.meta\.dir, '--lock'\]/);
    assert.equal(readFileSync(resolve(dir,'Cargo.lock'),'utf8'),text);
  } finally {rmSync(root,{recursive:true,force:true});}
}, 180000);

test('a new game names its Rust type after the game, everywhere the template does', async () => {
  const {gameDefaults}=await import('./app/shells.mjs');
  const parent=realpathSync(mkdtempSync(resolve(tmpdir(),'game-new-type-'))), app=resolve(parent,'my-2d-game');
  try {
    createGame(app);
    assert.equal(gameDefaults(app).game.type,'My2dGame');
    const files=readdirSync(app,{recursive:true}).filter(file=>statSync(resolve(app,file)).isFile());
    for(const file of files) assert.doesNotMatch(readFileSync(resolve(app,file),'utf8'),/SmallGame|small[-_]game|Small game/,file);
    assert.match(readFileSync(resolve(app,'logic/tests/sim.rs'),'utf8'),/use my_2d_game_logic::My2dGame;\nuse my_2d_game_logic::\{Beacon, Options\};/);
    assert.match(readFileSync(resolve(app,'app.contract'),'utf8'),/^component My2dGame$/m);
  } finally {rmSync(parent,{recursive:true,force:true});}
});

test('game/dev.mjs takes the next free port unless --port names a busy one', async () => {
  const {devPort}=await import('./dev.mjs');
  const {createServer}=await import('node:net');
  const held=createServer();
  await new Promise(ok=>held.listen(8850,'127.0.0.1',ok));
  try {
    const chosen=await devPort(['--wasm'],{start:8850});
    assert.ok(chosen.port>8850 && chosen.port<8900);
    assert.deepEqual(chosen.args,['--wasm','--port',String(chosen.port)]);
    await assert.rejects(devPort(['--port','8850']),/--port 8850: 127\.0\.0\.1:8850 is in use; choose another --port/);
    await assert.rejects(devPort(['--port','x']),/--port needs a port number/);
    await assert.rejects(devPort(['--port=8850']),/--port 8850: 127\.0\.0\.1:8850 is in use/);
    assert.deepEqual(await devPort(['--port',String(chosen.port)]),{port:chosen.port,args:['--port',String(chosen.port)]});
  } finally {await new Promise(ok=>held.close(ok));}
});

test('E10 compact authored manifest survives two bakes byte for byte', async () => {
  const {gameDefaults,gameShells}=await import('./app/shells.mjs');
  const parent=realpathSync(mkdtempSync(resolve(tmpdir(),'e10-manifest-'))), app=resolve(parent,'my-game');
  try {
    createGame(app);
    const path=resolve(app,'app.json');
    const authored=JSON.stringify({app:{name:'My Game',id:'org.example.my-game'},game:{crate:'my-game-logic',type:'MyGame'},host:{macos:{window:{width:960}}}},null,2)+'\n';
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


test('D6 host shells discover arguments without an engine build dependency', () => {
  const parent=realpathSync(mkdtempSync(resolve(tmpdir(),'d6-shells-'))), app=resolve(parent,'my-game');
  try {
    createGame(app);
    // As in the new-game build above, use a file for subprocess output under bun test.
    const graph = resolve(parent, 'graph.json');
    const child = spawnSync(process.execPath, ['-e', `import {prepareGame,gameDefaults} from ${JSON.stringify(resolve(import.meta.dir,'app/shells.mjs'))}; import {writeFileSync} from 'node:fs'; const dir=${JSON.stringify(app)}; writeFileSync(${JSON.stringify(graph)},JSON.stringify(prepareGame(dir,gameDefaults(dir).game,${JSON.stringify(import.meta.dir)})));`], {encoding:'utf8'});
    assert.equal(child.status, 0, child.stderr);
    const metadata = JSON.parse(readFileSync(graph, 'utf8'));
    const packages = new Map(metadata.packages.map(pkg => [pkg.id, pkg.name]));
    const nodes = new Map(metadata.resolve.nodes.map(node => [node.id, node]));
    for (const name of ['web','apple','linux']) {
      const pending = [metadata.packages.find(pkg => pkg.name === `my-game-${name}`).id], seen = new Set();
      while (pending.length) {
        const id = pending.pop();
        if (seen.has(id)) continue;
        seen.add(id);
        for (const dep of nodes.get(id).deps)
          if (dep.dep_kinds.some(({kind}) => kind !== 'dev')) pending.push(dep.pkg);
      }
      const unwanted = [...seen].map(id => packages.get(id)).filter(pkg => ['exact-game','my-game-logic'].includes(pkg));
      assert.deepEqual(unwanted, [], `${name} links ${unwanted.join(', ')}`);
    }
  } finally {rmSync(parent,{recursive:true,force:true});}
}, 120000);

test('Beacons is the files its author writes and survives two shell bakes', async () => {
  const {gameDefaults,gameShells}=await import('./app/shells.mjs');
  const app=resolve(import.meta.dir,'games/beacons');
  const tracked=spawnSync('git',['ls-files'],{cwd:app,encoding:'utf8'}).stdout.trim().split('\n').sort();
  assert.deepEqual(tracked,['README.md','app.contract','logic/src/lib.rs','logic/tests/sim.rs','pins.json','proof.mjs']);
  for(let i=0;i<2;i++) {
    const manifest=gameDefaults(app);gameShells(app,manifest.game,import.meta.dir);
    assert.equal(JSON.parse(readFileSync(resolve(app,'.shells/app.json'),'utf8')).app.id,'com.exact.beacons');
    const untracked=spawnSync('git',['status','--porcelain','--untracked-files=all','--','.'],{cwd:app,encoding:'utf8'}).stdout.split('\n').filter(line=>line.startsWith('??'));
    assert.deepEqual(untracked,[],'bakes write only ignored files');
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

test('the determinism lints refuse f32::sin, HashMap and Instant in game logic, naming each fix', async () => {
  const {prepareGame,gameDefaults,lintGame}=await import('./app/shells.mjs');
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'determinism-lints-'))), app=resolve(root,'lint-game');
  try {
    createGame(app);
    const game=gameDefaults(app).game, env={...process.env,CARGO_TARGET_DIR:resolve(import.meta.dir,'target')};
    prepareGame(app,game);
    lintGame(app,game,{env});
    const source=resolve(app,'logic/src/lib.rs');
    writeFileSync(source,readFileSync(source,'utf8').replace(/fn tick\(w: &mut World[^{]*\{/,match=>`${match}
        let _wobble = w.dt().sin();
        let _seen: std::collections::HashMap<u32, u32> = Default::default();
        let _wall = std::time::Instant::now();`));
    assert.throws(()=>lintGame(app,game,{env}),error=>['f32::sin differs across hosts; use exact_game::math::sin',
      'use std::collections::BTreeMap','use world time: w.now()'].every(fix=>error.message.includes(fix)) || assert.fail(error.message));
  } finally {rmSync(root,{recursive:true,force:true});}
},300000);

test('RUSTFLAGS without -fp-contract=off is refused in a generated game shell, naming the fix', async () => {
  const {gameDefaults,gameShells}=await import('./app/shells.mjs');
  const root=realpathSync(mkdtempSync(resolve(tmpdir(),'rustflags-guard-'))), app=resolve(root,'flags-game');
  try {
    createGame(app);
    gameShells(app,gameDefaults(app).game,import.meta.dir);
    assert.match(readFileSync(resolve(app,'.shells/.cargo/config.toml'),'utf8'),/EXACT_GAME_FP_CONTRACT = "off"/);
    const env={...process.env,RUSTFLAGS:'-C debug-assertions=off',CARGO_TARGET_DIR:resolve(root,'target'),CARGO_BUILD_BUILD_DIR:resolve(root,'build')};
    delete env.CARGO_ENCODED_RUSTFLAGS;
    const checked=spawnSync('cargo',['check','--offline','-p','flags-game-logic'],{cwd:resolve(app,'.shells'),env,encoding:'utf8'});
    assert.notEqual(checked.status,0);
    assert.match(checked.stderr,/RUSTFLAGS replaced the game workspace's `-C llvm-args=-fp-contract=off`/);
    assert.match(checked.stderr,/RUSTFLAGS="\$RUSTFLAGS -C llvm-args=-fp-contract=off"/);
  } finally {rmSync(root,{recursive:true,force:true});}
},300000);

test('copies of the same game own distinct intermediate build directories', async () => {
  const {gameDefaults,gameShells} = await import('./app/shells.mjs');
  const root = realpathSync(mkdtempSync(resolve(tmpdir(), 'game-copies-')));
  try {
    const a = resolve(root,'first'), b = resolve(root,'second');
    createGame(a); cpSync(a,b,{recursive:true});
    for (const app of [a,b]) {
      gameShells(app,gameDefaults(app).game,import.meta.dir);
      const config = Bun.TOML.parse(readFileSync(resolve(app,'.shells/.cargo/config.toml'),'utf8'));
      assert.equal(config.build['build-dir'], resolve(app,'target'));
    }
  } finally {rmSync(root,{recursive:true,force:true});}
});

test('a game whose type would shadow an engine export or a template item is refused', async () => {
  const {createGame,takenTypes}=await import('./new.mjs');
  const taken=takenTypes();
  for (const type of ['World','Camera','Transform','Beacon','Options']) assert.ok(taken.has(type),type);
  for (const name of ['world','camera','beacon']) assert.throws(()=>createGame(resolve(tmpdir(),`zz-${process.pid}`,name)),/would collide/);
  assert.ok(!taken.has('Garden'));
});

test('--render scaffolds a render crate with an explicit shader root', async () => {
  const {gameDefaults, gameShells} = await import('./app/shells.mjs');
  const root = realpathSync(mkdtempSync(resolve(tmpdir(), 'render-scaffold-'))), app = resolve(root, 'my-game');
  try {
    createGame(app, undefined, {render:true});
    const manifest = JSON.parse(readFileSync(resolve(app, 'app.json'), 'utf8'));
    assert.deepEqual(manifest, {game:{render:{crate:'my-game-render', hooks:'Passes', shaders:'shaders::SHADERS'}}, gpu:{shaderRoots:['render/shaders']}});
    for (const file of ['render/Cargo.toml', 'render/build.rs', 'render/src/lib.rs']) assert.ok(existsSync(resolve(app, file)), file);
    assert.ok(statSync(resolve(app, 'render/shaders')).isDirectory());
    gameShells(app, gameDefaults(app).game, import.meta.dir);
    assert.equal(readFileSync(resolve(app, '.shells/gpu/src/lib.rs'), 'utf8'),
      'exact_game_render::module!(game_logic::MyGame, hooks = game_render::Passes, shaders = game_render::shaders::SHADERS);\n');
  } finally { rmSync(root, {recursive:true, force:true}); }
});
