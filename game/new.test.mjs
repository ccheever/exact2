import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, rmSync, mkdtempSync, mkdirSync, writeFileSync, statSync, utimesSync } from 'node:fs';
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
      timer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }, 60000);
    };
    for (const stream of [child.stdout, child.stderr]) stream.on('data', data => { process.stdout.write(data); reset(); });
    reset();
    child.on('error', error => { clearTimeout(timer); fail(error); });
    child.on('close', (code, signal) => { clearTimeout(timer); code === 0 ? ok() : fail(new Error(`${command} exited ${code ?? signal}`)); });
  });
  assert.ok(!existsSync(app));
  const lockfile = resolve(import.meta.dir, 'Cargo.lock'), originalLock = readFileSync(lockfile);
  let registeredLock;
  try {
    await run('bun', ['game/new.mjs', name]);
    assert.deepEqual(readdirSync(app).sort(), ['app.contract','app.json','logic','proof.mjs']);
    env.EXACT_APP_DIR = app;
    await run('bun', ['-e', 'import {resolveApp} from "./scripts/app.mjs"; resolveApp();']);
    registeredLock = readFileSync(lockfile);
    for (const file of ['Cargo.toml','Cargo.lock','web','apple','gpu']) assert.ok(!existsSync(resolve(app, file)), file);
    const shellFiles = ['gpu','web','apple'].flatMap(kind => ['Cargo.toml','src/lib.rs', ...(kind === 'gpu' ? [] : ['build.rs'])]
      .map(file => resolve(import.meta.dir, '.shells', `${name}-${kind}`, file)));
    const stamps = shellFiles.map(file => statSync(file).mtimeMs);
    await run('bun', ['-e', 'import {resolveApp} from "./scripts/app.mjs"; resolveApp();']);
    assert.deepEqual(shellFiles.map(file => statSync(file).mtimeMs), stamps, 'resolving again leaves Cargo inputs untouched');
    await run('cargo', ['test', '-p', `${name}-logic`], import.meta.dir);
    await run('bun', [resolve(app, 'proof.mjs'), 'web']);
    assert.match(readFileSync(resolve(app, 'artifacts/proof.txt'), 'utf8'), new RegExp(`PROOF PASS ${name} web: 0 failures`));
  } finally {
    rmSync(app, {recursive:true, force:true});
    for (const kind of ['gpu','web','apple']) rmSync(resolve(import.meta.dir, '.shells', `${name}-${kind}`), {recursive:true, force:true});
    if (registeredLock) assert.deepEqual(readFileSync(lockfile), registeredLock, 'shared lockfile changed during test');
    writeFileSync(lockfile, originalLock);
  }
}, 300000);

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
