import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';

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
    child.on('exit', (code, signal) => { clearTimeout(timer); code === 0 ? ok() : fail(new Error(`${command} exited ${code ?? signal}`)); });
  });
  assert.ok(!existsSync(app));
  try {
    await run('bun', ['game/new.mjs', name]);
    for (const file of ['Cargo.toml','Cargo.lock','web/build.rs','apple/build.rs']) assert.ok(!existsSync(resolve(app, file)), file);
    await run('cargo', ['test', '-p', `${name}-logic`], import.meta.dir);
    await run('bun', [resolve(app, 'proof.mjs'), 'web']);
    assert.match(readFileSync(resolve(app, 'artifacts/proof.txt'), 'utf8'), new RegExp(`PROOF PASS ${name} web: 0 failures`));
  } finally {
    rmSync(app, {recursive:true, force:true});
    await run('cargo', ['update', '--workspace', '--offline', '--quiet'], import.meta.dir);
  }
}, 300000);
