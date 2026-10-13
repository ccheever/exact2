// @ref LLP 1086 D12 — a fresh wired app, not backend-less hello, qualifies the device contract.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, existsSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
const sdk = resolve(import.meta.dirname, '..');
const binary = process.env.SNAPBACK4_BIN ?? resolve(sdk, `node_modules/snapback4-${process.platform}-${process.arch}/snapback4`);
const packageDir = process.env.SNAPBACK4_PACKAGE ?? resolve(sdk, 'node_modules/snapback4');
for (const online of [false, true]) for (const mode of ['dev', 'serve']) test(`npm Exact ${online ? 'online write' : 'guestbook syncs, writes offline and reconnects'} against ${mode}`, async () => {
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
    // Qualify the shipped npm shape, not a source-directory symlink.
    const packed = JSON.parse(run('npm', ['pack', packageDir, '--ignore-scripts', '--json', '--pack-destination', project]));
    const tarball = join(project, packed[0].filename);
    writeFileSync(join(project, 'package.json'), JSON.stringify({type: 'module', dependencies: {snapback4: `file:${tarball}`}}));
    run('npm', ['install', '--ignore-scripts', '--omit=optional', '--no-audit', '--no-fund']);
    assert.ok(readFileSync(join(project, 'node_modules/snapback4/dist/contract.d.ts')).length);
    if (online) {
      mkdirSync(join(project, 'snapback'), {recursive: true});
      writeFileSync(join(project, 'snapback/schema.q'), `use personas alice, bob

table notes:
  author: principal
  body: text 1..200
  at: time
  by byTime: at, id
  public 'notes are public'
  insert <- .author = viewer
  update <- deny
  delete <- .author = viewer
  online only

query recent():
  return notes last 50 by byTime

mutation post(body: text 1..200):
  require body != 'refused' else FULL
  row = insert notes { author: viewer, body, at: now }
  return { id: row.id }
`);
    }
    run(process.execPath, [join(sdk, 'scripts/exact.mjs'), 'new', app, '--snapback', ...(online ? ['--online'] : []), '--port', String(port)]);
    if (online) {
      assert.equal(existsSync(join(app, 'assets/snapback4.wasm')), false);
      writeFileSync(join(app, 'app.contract'), `shape Board
  latest: string
shape Post
  id: string
  status: enum<"sent", "failed">
component App
  state draft = ""
  resource board = notes() as shape Board else empty()
  mutation posted as shape Post refreshes board
  action edit(value: string)
    draft = value
  action post(mode: enum<"send", "preview">)
    if mode == "send"
      send posted = post(draft)
  view
    column
      input value=draft input=edit testId="draft"
      button "Post" press=post("send") testId="post"
      text board.latest testId="latest"
      text (match failure(posted) { case some(f) => f.code, case none => "ok" }) testId="failure"
      text (match posted { case some(p) => p.status, case none => "" }) testId="fate"
`);
      writeFileSync(join(app, 'app.ts'), `import type { Answer, Result, Sources } from './app.contract.d.ts';
import { Snapback, refused } from './snapback4/snapback.ts';
import { api } from './backend/api.ts';
export const appId = 'com.example.app';
const origin = 'http://127.0.0.1:${port}';
export const grants = \`net.fetch \${origin}\`;
const persona = 'alice';
const db = new Snapback({origin, headers: () => ({ 'x-snapback-persona': persona })});
const sources: Sources = {
  notes: async (): Promise<Result<'notes'>> => {
    const read = await db.read(api.recent, {});
    if (read.denied) throw refused(read.denied);
    return { latest: read.data?.[0]?.body ?? '' };
  },
  post: async ([body]): Promise<Result<'post'>> => {
    const sent = await db.send(api.post, {body});
    if (sent.state === 'failed') throw refused(sent.why);
    return { ...sent.result, status: body === 'malformed' ? 'bogus' as 'sent' : 'sent' };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
`);
      writeFileSync(join(app, 'app.test.contract'), `epoch now

test "one online write"
  clock data
  type "draft" "from the tunnel"
  tap "post"
  clock data
  expect text "latest" == "from the tunnel"
  expect text "fate" == "sent"
  type "draft" "refused"
  tap "post"
  clock data
  expect text "failure" == "FULL"
  expect text "fate" == "sent"
  type "draft" "malformed"
  tap "post"
  clock data
  expect text "failure" == "shape"
  expect text "fate" == "sent"
  type "draft" "again"
  tap "post"
  clock data
  expect text "failure" == "ok"
  expect text "latest" == "again"
`);
    }
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
      writeFileSync(join(app, 'app.ts'), online ? source.replace("'x-snapback-persona': persona", `authorization: ${JSON.stringify('Bearer ' + signed.session.token)}`) : source);
      const tests = readFileSync(join(app, 'app.test.contract'), 'utf8');
      if (!online) writeFileSync(join(app, 'app.test.contract'), 'epoch now\n\n' + tests.slice(tests.indexOf('test "offline,'))
        .replace('  clock data\n  fail fetch', '  clock data\n  expect text "status" == "Synced"\n  fail fetch')
        + `  expect text "status" == "Synced"\n  expect text "latest" == ${JSON.stringify(signed.session.principal + ': from the tunnel')}\n`);
    }
    // Execute the exact recovery command the server's refusal prescribes.
    if (!online) writeFileSync(join(app, 'assets/snapback4.wasm'), Buffer.from([0,97,115,109,1,0,0,0]));
    if (!online) run(process.execPath, ['exact.mjs', 'snapback'], app);
    if (mode === 'dev' && !online) {
      const tests = join(app, 'app.test.contract');
      writeFileSync(tests, readFileSync(tests, 'utf8').replace('  expect text "sending" == "0 sending"',
        '  expect text "sending" == "0 sending"\n  expect text "status" == "Synced"\n  expect text "latest" == "dev:alice: from the tunnel"'));
    }
    // Upstream check currently gates backend-only mounts; record the coordinated deferral.
    if (!online) run(binary, ['check']);
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
