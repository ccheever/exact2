// @ref LLP 1086 D12 — a fresh wired app, not backend-less hello, qualifies the device contract.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, symlinkSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
const sdk = resolve(import.meta.dirname, '..');
const binary = process.env.SNAPBACK4_BIN ?? resolve(sdk, `node_modules/snapback4-${process.platform}-${process.arch}/snapback4`);
const packageDir = process.env.SNAPBACK4_PACKAGE ?? resolve(sdk, 'node_modules/snapback4');
for (const mode of ['dev', 'serve']) test(`fresh Exact guestbook syncs, writes offline and reconnects against ${mode}`, async () => {
  const project = mkdtempSync(join(sdk, 'target/hs-guestbook-'));
  const app = join(project, 'app'); let server;
  const env = {...process.env, SNAPBACK4_BIN: binary, CARGO_INCREMENTAL: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0'};
  const run = (command, args, cwd = project) => {
    const result = spawnSync(command, args, {cwd, env, encoding: 'utf8', timeout: 300_000});
    assert.equal(result.status, 0, `${command} ${args.join(' ')}\n${result.stdout}\n${result.stderr}`);
    return result.stdout;
  };
  try {
    const reservation = createServer().listen(0, '127.0.0.1');
    await new Promise(ok => reservation.once('listening', ok));
    const port = reservation.address().port; await new Promise(ok => reservation.close(ok));
    mkdirSync(join(project, 'node_modules/.bin'), {recursive: true});
    symlinkSync(packageDir, join(project, 'node_modules/snapback4'), 'dir');
    symlinkSync(binary, join(project, 'node_modules/.bin/snapback4'));
    writeFileSync(join(project, 'package.json'), JSON.stringify({type: 'module', dependencies: {snapback4: '0.4.25'}}));
    run(process.execPath, [join(sdk, 'scripts/exact.mjs'), 'new', app, '--snapback', '--port', String(port)]);
    assert.match(readFileSync(join(app, 'app.ts'), 'utf8'), /import \{ api \} from '.\/backend\/api.ts'/);
    if (mode === 'serve') {
      const schema = join(project, 'snapback/schema.q');
      writeFileSync(schema, readFileSync(schema, 'utf8').replace('use personas alice, bob', 'use identity guests'));
      run(binary, ['deploy', join(project, 'release')]);
    }
    server = spawn(binary, mode === 'dev' ? ['dev', '--port', String(port), '--no-watch'] : ['serve', join(project, 'release'), '--port', String(port)],
      {cwd: project, env, stdio: ['ignore', 'pipe', 'pipe']});
    await new Promise((ok, fail) => {
      let text = ''; const timer = setTimeout(() => fail(new Error(`server readiness timeout: ${text}`)), 30_000);
      const read = bytes => { text += bytes; if (/SNAPBACK4_READY/.test(text)) {clearTimeout(timer); ok();} };
      server.stderr.on('data', read); server.stdout.on('data', read);
      server.once('error', fail); server.once('exit', code => {clearTimeout(timer); fail(new Error(`server exited ${code}: ${text}`));});
    });
    if (mode === 'serve') {
      const response = await fetch(`http://127.0.0.1:${port}/auth/guest`, {method: 'POST', headers: {'content-type': 'application/json'}, body: '{}'});
      const signed = await response.json(); assert.equal(response.status, 200); assert.ok(signed.session?.token);
      const source = readFileSync(join(app, 'app.ts'), 'utf8').replace('viewer: `dev:${persona}`, headers: () => ({ \'x-snapback-persona\': persona })',
        `viewer: ${JSON.stringify(signed.session.principal)}, headers: () => ({ authorization: ${JSON.stringify('Bearer ' + signed.session.token)} })`);
      writeFileSync(join(app, 'app.ts'), source);
      const tests = readFileSync(join(app, 'app.test.contract'), 'utf8');
      writeFileSync(join(app, 'app.test.contract'), 'epoch now\n\n' + tests.slice(tests.indexOf('test "offline,'))
        .replace('  clock data\n  fail fetch', '  clock data\n  expect text "status" == "Synced"\n  fail fetch')
        + `  expect text "status" == "Synced"\n  expect text "latest" == ${JSON.stringify(signed.session.principal + ': from the tunnel')}\n`);
    }
    // Execute the exact recovery command the server's refusal prescribes.
    writeFileSync(join(app, 'assets/snapback4.wasm'), Buffer.from([0,97,115,109,1,0,0,0]));
    run(process.execPath, ['exact.mjs', 'snapback', '--rebuild'], app);
    if (mode === 'dev') {
      const tests = join(app, 'app.test.contract');
      writeFileSync(tests, readFileSync(tests, 'utf8').replace('  expect text "sending" == "0 sending"',
        '  expect text "sending" == "0 sending"\n  expect text "status" == "Synced"\n  expect text "latest" == "dev:alice: from the tunnel"'));
    }
    const output = run(process.execPath, ['exact.mjs', 'test', 'web'], app);
    assert.match(output, /0 failed/);
    // Check server authority too: a refused queued write must not satisfy reconnect.
    const headers = mode === 'dev' ? {'x-snapback-persona': 'alice'} : {};
    if (mode === 'serve') {
      const signed = await (await fetch(`http://127.0.0.1:${port}/auth/guest`, {method: 'POST', headers: {'content-type': 'application/json'}, body: '{}'})).json();
      headers.authorization = 'Bearer ' + signed.session.token;
    }
    const read = await fetch(`http://127.0.0.1:${port}/q/recent`, {method: 'POST', headers: {...headers, 'content-type': 'application/json'}, body: '{"args":{}}'});
    assert.equal(read.status, 200); assert.ok((await read.json()).data.some(row => row.body === 'from the tunnel'));

  } finally {
    if (server && server.exitCode === null) { const ended = new Promise(ok => server.once('exit', ok)); server.kill('SIGTERM'); await ended; }
    rmSync(project, {recursive: true, force: true});
  }
}, 600_000);
