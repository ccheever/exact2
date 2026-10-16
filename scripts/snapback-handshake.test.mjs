// @ref LLP 1086 D12 — a fresh wired app, not backend-less hello, qualifies the device contract.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, existsSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { listenOn } from './agent-launch.mjs';
const sdk = resolve(import.meta.dirname, '..');
const installedBinary = process.env.SNAPBACK4_BIN ?? resolve(sdk, `node_modules/snapback4-${process.platform}-${process.arch}/snapback4`);
const pinnedVersion=JSON.parse(readFileSync(resolve(sdk,'package.json'),'utf8')).devDependencies.snapback4;
const packageDir = process.env.SNAPBACK4_PACKAGE ?? resolve(sdk, 'node_modules/snapback4');
for (const online of [false, true]) for (const mode of ['dev', 'serve']) test(`npm Exact ${online ? 'online write' : 'guestbook syncs, writes offline and reconnects'} against ${mode}`, async () => {
  const project = mkdtempSync(join(sdk, 'target/hs-guestbook-'));
  const app = join(project, 'app'); let server, binary = installedBinary;
  const env = {...process.env, SNAPBACK4_BIN: binary, CARGO_INCREMENTAL: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0'};
  const run = (command, args, cwd = project) => {
    const result = spawnSync(command, args, {cwd, env, encoding: 'utf8', timeout: 300_000});
    assert.equal(result.status, 0, `${command} ${args.join(' ')}\n${result.stdout}\n${result.stderr}`);
    if (args.includes('test') && args.includes('web') && env.PORT_LO && env.PORT_HI) {
      const page = /served at http:\/\/127\.0\.0\.1:(\d+)/.exec(result.stderr);
      assert.ok(page, `test web did not report its page origin: ${result.stderr}`);
      assert.ok(Number(page[1]) >= Number(env.PORT_LO) && Number(page[1]) <= Number(env.PORT_HI), `test web escaped its assigned range: ${page[1]}`);
    }
    return result.stdout;
  };
  try {
    const reservation = createServer();
    await listenOn(reservation, 0);
    const port = reservation.address().port; await new Promise(ok => reservation.close(ok));
    // Qualify the shipped npm shape, not a source-directory symlink.
    if (process.env.SNAPBACK4_REGISTRY) {
      writeFileSync(join(project, 'package.json'), JSON.stringify({type: 'module', dependencies: {snapback4: pinnedVersion}}));
      run(process.execPath, ['install', '--registry', process.env.SNAPBACK4_REGISTRY]);
    } else {
      const packed = JSON.parse(run('npm', ['pack', packageDir, '--ignore-scripts', '--json', '--pack-destination', project]));
      const tarball = join(project, packed[0].filename);
      writeFileSync(join(project, 'package.json'), JSON.stringify({type: 'module', dependencies: {snapback4: `file:${tarball}`}}));
      run('npm', ['install', '--ignore-scripts', '--omit=optional', '--no-audit', '--no-fund']);
    }
    if (process.env.SNAPBACK4_REGISTRY) {
      binary = join(project, `node_modules/snapback4-${process.platform}-${process.arch}/snapback4`);
      env.SNAPBACK4_BIN = binary;
      assert.equal(JSON.parse(readFileSync(join(project, `node_modules/snapback4-${process.platform}-${process.arch}/package.json`))).version, pinnedVersion);
    }
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
    if (!online) run(process.execPath, ['exact.mjs', 'snapback'], app);
    if (online) {
      run(process.execPath, ['exact.mjs', 'update'], app);
      run(process.execPath, ['exact.mjs', 'snapback'], app);
      assert.match(JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).typescript.sources.snapback4, /snapback4\/online$/);
      assert.equal(existsSync(join(app, 'assets/snapback4.wasm')), false);
      assert.ok(!readFileSync(join(app, 'app.ts'), 'utf8').includes('sqlite.open'));
      writeFileSync(join(app, 'app.contract'), `shape Choice
  value: enum<"a", "b">
shape Board
  latest: string
  choices: Choice[]
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
      for choice in board.choices key=choice.value
        text choice.value
      text (match failure(posted) { case some(f) => f.code, case none => "ok" }) testId="failure"
      text (match posted { case some(p) => p.status, case none => "" }) testId="fate"
`);
      writeFileSync(join(app, 'app.ts'), `import type { Answer, Result, Sources } from './app.contract.d.ts';
import { Snapback, refused } from './snapback4/snapback.ts';
import { api } from './backend/api.ts';
export const appId = '${JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id}';
const origin = 'http://127.0.0.1:${port}';
export const grants = \`net.fetch \${origin}\`;
const persona = 'alice';
const db = new Snapback({origin, headers: () => ({ 'x-snapback-persona': persona })});
const sources: Sources = {
  notes: async (): Promise<Result<'notes'>> => {
    const read = await db.read(api.recent, {});
    if (read.denied) throw refused(read.denied);
    return { latest: read.data?.[0]?.body ?? '', choices: [{value: 'a'}, {value: 'b'}] };
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
    if (!online && mode === 'dev') {
      const oldPort = port === 30199 ? 30198 : 30199;
      const from = `http://127.0.0.1:${port}`, old = `http://127.0.0.1:${oldPort}`;
      const source = readFileSync(join(app, 'app.ts'), 'utf8').replaceAll(from, old).replace(`const origin = '${old}'`, `const origin = '${old}/'`);
      writeFileSync(join(app, 'app.ts'), source);
      writeFileSync(join(app, 'app.test.contract'), readFileSync(join(app, 'app.test.contract'), 'utf8').replaceAll(from, old));
      run(process.execPath, ['exact.mjs', 'snapback', '--port', String(port)], app);
      assert.equal(readFileSync(join(app, 'app.ts'), 'utf8'), source);
      assert.equal(readFileSync(join(app,'.exact/backend-origin'),'utf8').trim(),from);
      assert.ok(readFileSync(join(app, 'app.test.contract'), 'utf8').includes('fail fetch "backend"'));
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
      const read = bytes => { text += bytes; if (/SNAPBACK4_READY|^http:\/\/127\.0\.0\.1:\d+\s*$/m.test(text)) {clearTimeout(timer); ok();} };
      server.stderr.on('data', read); server.stdout.on('data', read);
      server.once('error', fail); server.once('exit', code => {clearTimeout(timer); fail(new Error(`server exited ${code}: ${text}`));});
    });
    if (mode === 'serve') {
      const root = await fetch(`http://127.0.0.1:${port}/`);
      assert.equal(root.status, 401); assert.equal((await root.json()).denied.code, 'E_AUTH');
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
    if (!online) {
      // Divergent mounts are kept byte-for-byte during artifact-only repair.
      const manifest = join(app, 'app.json'), original = readFileSync(manifest, 'utf8');
      const edited = JSON.parse(original); edited.typescript.sources.backend = '../custom/generated';
      writeFileSync(manifest, JSON.stringify(edited));
      const before = ['app.json', 'app.ts', 'app.test.contract', 'AGENTS.md', '.gitignore'].map(file => readFileSync(join(app, file), 'utf8'));
      run(process.execPath, ['exact.mjs', 'snapback', '--rebuild', '--artifact-only'], app);
      assert.deepEqual(['app.json', 'app.ts', 'app.test.contract', 'AGENTS.md', '.gitignore'].map(file => readFileSync(join(app, file), 'utf8')), before);
      writeFileSync(manifest, original);
    }
    if (mode === 'dev' && !online) {
      const tests = join(app, 'app.test.contract');
      writeFileSync(tests, readFileSync(tests, 'utf8').replace('  expect text "sending" == "0 sending"',
        '  expect text "sending" == "0 sending"\n  expect text "status" == "Synced"\n  expect text "latest" == "dev:alice: from the tunnel"'));
    }
    // 0.4.28 gates device consumers, and admits device-free online mounts.
    run(binary, ['check']);
    const output = run(process.execPath, ['exact.mjs', 'test', 'web'], app);
    assert.match(output, /0 failed/);
    if (online && mode === 'dev') {
      // With the backend holding the sole assigned port, an un-stored drive
      // must preserve range exhaustion rather than dereference kept.port.
      const refused = spawnSync(process.execPath, ['exact.mjs', 'agent', 'web', 'tree'],
        {cwd: app, env: {...env, PORT_LO: String(port), PORT_HI: String(port)}, encoding: 'utf8', timeout: 30_000});
      assert.notEqual(refused.status, 0);
      assert.match(refused.stderr, /PORT_LO\/PORT_HI.*all in use/);
      assert.doesNotMatch(refused.stderr, /TypeError|kept.port/);
    }
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

// The published README is executable input: keep the complete session flow and scaffold qualified together.
test('registry online scaffold and complete password example work through test web', async () => {
  const project = mkdtempSync(join(sdk, 'target/hs-password-')), app = join(project, 'app');
  const env = {...process.env, CARGO_INCREMENTAL: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0'};
  let server;
  const run = (args, cwd = project) => {
    const r = spawnSync(process.execPath, args, {cwd, env, encoding: 'utf8', timeout: 300_000});
    assert.equal(r.status, 0, `${args.join(' ')}\n${r.stdout}\n${r.stderr}`);
    if (args.includes('test') && args.includes('web')) {
      assert.match(r.stdout, /0 failed/);
      const page = /served at http:\/\/127\.0\.0\.1:(\d+)/.exec(r.stderr);
      assert.ok(page);
      if (env.PORT_LO && env.PORT_HI) assert.ok(+page[1] >= +env.PORT_LO && +page[1] <= +env.PORT_HI);
    }
    return r;
  };
  try {
    writeFileSync(join(project, 'package.json'), JSON.stringify({type: 'module', dependencies: {snapback4: pinnedVersion}}));
    run(['install', '--registry', process.env.SNAPBACK4_REGISTRY ?? 'https://registry.npmjs.org/']);
    const binary = join(project, `node_modules/snapback4-${process.platform}-${process.arch}/snapback4`);
    env.SNAPBACK4_BIN = binary;
    assert.equal(JSON.parse(readFileSync(join(project, `node_modules/snapback4-${process.platform}-${process.arch}/package.json`))).version, pinnedVersion);
    const reservation = createServer(); await listenOn(reservation, 0);
    const port = reservation.address().port; await new Promise(ok => reservation.close(ok));
    const guide = readFileSync(join(sdk, 'snapback4/README.md'), 'utf8');
    const online = guide.split('### A complete online app\n')[1].split('### A complete online password app')[0];
    const schema = /```quarry\n([\s\S]*?)\n```/.exec(online)[1];
    mkdirSync(join(project, 'snapback'), {recursive: true});
    writeFileSync(join(project, 'snapback/schema.q'), schema.replace('use personas alice, bob', 'use identity password') + '\n');
    run([join(sdk, 'scripts/exact.mjs'), 'new', app, '--snapback', '--online', '--port', String(port)]);
    assert.match(readFileSync(join(app, 'app.ts'), 'utf8'), /db.read\(api.recent, args\)/);
    assert.match(readFileSync(join(app, 'app.ts'), 'utf8'), /db.send\(api.post, args\)/);
    server = spawn(binary, ['dev', '--port', String(port), '--no-watch'], {cwd: project, env, stdio: ['ignore', 'pipe', 'pipe']});
    await new Promise((ok, fail) => {
      let log = ''; const timer = setTimeout(() => fail(new Error(log)), 30_000);
      const read = bytes => {log += bytes; if (/SNAPBACK4_READY|^http:\/\/127\.0\.0\.1:\d+\s*$/m.test(log)) {clearTimeout(timer); ok();}};
      server.stdout.on('data', read); server.stderr.on('data', read);
      server.once('error', fail); server.once('exit', code => {clearTimeout(timer); fail(new Error(`backend exited ${code}: ${log}`));});
    });
    run(['exact.mjs', 'test', 'web'], app); // the generated existing-schema explorer compiles and boots
    const password = guide.split('### A complete online password app\n')[1].split('### Live resources')[0];
    const blocks = [...password.matchAll(/```(?:contract|ts)\n([\s\S]*?)\n```/g)].map(m => m[1]);
    for (const [i, file] of ['app.contract', 'app.ts', 'app.test.contract'].entries())
      writeFileSync(join(app, file), blocks[i].replaceAll('http://127.0.0.1:30100', `http://127.0.0.1:${port}`).replaceAll("'com.example.app'", JSON.stringify(JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id)) + '\n');
    // Going offline for logout must still remove the browser's stored session.
    const tests = join(app, 'app.test.contract');
    writeFileSync(tests, readFileSync(tests, 'utf8').replace('\ntest "a new test', '\n  fail fetch "backend"\n  tap "logout"\n  clock data\n  expect text "session" == "Signed out"\n  pass fetch "backend"\n  reload\n  clock data\n  expect text "session" == "Signed out"\n\ntest "a new test'));
    run(['exact.mjs', 'contract', 'types', 'app.contract', '-o', 'app.contract.d.ts'], app);
    const first = run(['exact.mjs', 'test', 'web'], app);
    assert.match(first.stderr, /fresh: reseeding before each of 2 tests/);
    // Repeated invocation proves backend reset and browser secret isolation.
    run(['exact.mjs', 'test', 'web'], app);
  } finally {
    if (server && server.exitCode === null) {const ended = new Promise(ok => server.once('exit', ok)); server.kill('SIGTERM'); await ended;}
    rmSync(project, {recursive: true, force: true});
  }
}, 600_000);

test('test web advances time only for mutation requests and preserves action time snapshots', async()=>{
  const {createApp}=await import('../game/new.mjs');
  const project=mkdtempSync(join(sdk,'target/ex8-mutation-clock-')), app=join(project,'app');
  const env={...process.env,CARGO_INCREMENTAL:'0',CARGO_PROFILE_DEV_DEBUG:'0',CARGO_PROFILE_TEST_DEBUG:'0'};
  const run=args=>{
    const result=spawnSync(process.execPath,['exact.mjs',...args],{cwd:app,env,encoding:'utf8',timeout:300_000});
    assert.equal(result.status,0,`${result.stdout}\n${result.stderr}`); return result;
  };
  try {
    createApp(app);
    writeFileSync(join(app,'app.contract'),`component App
  state automatic = false
  task auto when automatic
    after(1, save)
  action enable
    automatic = true
  mutation saved as shape number queue
  action save
    send saved = save(performanceNow())
  action idle
    let value = 1
  view
    column
      button "Save" press=save testId="save"
      button "Idle" press=idle testId="idle"
      button "Enable timer" press=enable testId="enable"
      text (match saved { case some(n) => toString(n), case none => "-" }) testId="reply"
      text toString(performanceNow()) testId="time"
`);
    writeFileSync(join(app,'app.ts'),`import type {Answer} from './app.contract.d.ts';
export const appId='com.example.app'; export const grants='';
export const answer:Answer=(_source,[now])=>now;
`);
    writeFileSync(join(app,'app.test.contract'),`test "only mutations move the test clock"
  tap "idle"
  expect text "time" == "0"
  tap "save"
  clock data
  expect text "reply" == "0"
  tap "idle"
  expect text "time" == "1"
  tap "save"
  clock data
  expect text "reply" == "1"
  tap "save"
  clock data
  expect text "reply" == "2"
  reload
  tap "save"
  clock data
  expect text "reply" == "0"

test "a timer mutation gets its own automatic millisecond"
  tap "enable"
  tap "save"
  tap "save"
  clock data
  expect text "reply" == "2"
`);
    run(['contract','types','app.contract','-o','app.contract.d.ts']);
    assert.match(run(['test','web']).stdout,/2 passed, 0 failed/);
  } finally {rmSync(project,{recursive:true,force:true});}
},600_000);

for (const online of [false,true]) test(`README ${online ? 'online' : 'synced'} guestbook passes both tests as written on the pinned registry pack`,async()=>{
  const project=mkdtempSync(join(sdk,'target/ex8-readme-')),app=join(project,'app');
  const env={...process.env,CARGO_INCREMENTAL:'0',CARGO_PROFILE_DEV_DEBUG:'0',CARGO_PROFILE_TEST_DEBUG:'0'};
  let server;
  const run=(args,cwd=project)=>{
    const r=spawnSync(process.execPath,args,{cwd,env,encoding:'utf8',timeout:300_000});
    assert.equal(r.status,0,`${args.join(' ')}\n${r.stdout}\n${r.stderr}`); return r;
  };
  try {
    writeFileSync(join(project,'package.json'),JSON.stringify({private:true,type:'module',dependencies:{snapback4:pinnedVersion}}));
    run(['install','--registry',process.env.SNAPBACK4_REGISTRY ?? 'http://127.0.0.1:4873/']);
    const binary=join(project,`node_modules/snapback4-${process.platform}-${process.arch}/snapback4`);
    env.SNAPBACK4_BIN=binary;
    assert.equal(JSON.parse(readFileSync(join(project,'node_modules/snapback4/package.json'),'utf8')).version,pinnedVersion);
    const guide=readFileSync(join(sdk,'snapback4/README.md'),'utf8');
    const section=guide.split(online ? '### A complete online app\n' : '### A complete app\n')[1].split(online ? '### A complete online password app' : '### A complete online app')[0];
    const blocks=[...section.matchAll(/```(?:quarry|contract|ts)?\n([\s\S]*?)\n```/g)].map(m=>m[1]);
    assert.equal(blocks.length,4);
    mkdirSync(join(project,'snapback'),{recursive:true});
    writeFileSync(join(project,'snapback/schema.q'),blocks[0]+'\n');
    const reservation=createServer();await listenOn(reservation,0);
    const port=reservation.address().port;await new Promise(ok=>reservation.close(ok));
    run([join(sdk,'scripts/exact.mjs'),'new',app,'--snapback',...(online ? ['--online'] : []),'--port',String(port)]);
    // The examples themselves remain byte-for-byte: runtime backend input selects the test server.
    for(const [i,file] of ['app.contract','app.ts','app.test.contract'].entries()) writeFileSync(join(app,file),blocks[i+1]+'\n');
    const authored=['app.contract','app.ts','app.test.contract'].map(f=>readFileSync(join(app,f),'utf8'));
    run(['exact.mjs','snapback','--port',String(port)],app);
    if (!online) { const update=run(['exact.mjs','update'],app); assert.match(update.stdout,/rebuilt; app sources and mounts kept unchanged/); }
    assert.deepEqual(['app.contract','app.ts','app.test.contract'].map(f=>readFileSync(join(app,f),'utf8')),authored);
    run(['exact.mjs','contract','types','app.contract','-o','app.contract.d.ts'],app);
    server=spawn(binary,['dev','--port',String(port),'--no-watch'],{cwd:project,env,stdio:['ignore','pipe','pipe']});
    await new Promise((ok,fail)=>{
      let log='';const timer=setTimeout(()=>fail(new Error(log)),30_000);
      const read=bytes=>{log+=bytes;if(/SNAPBACK4_READY|^http:\/\/127\.0\.0\.1:\d+\s*$/m.test(log)){clearTimeout(timer);ok();}};
      server.stdout.on('data',read);server.stderr.on('data',read);server.once('error',fail);server.once('exit',code=>{clearTimeout(timer);fail(new Error(`backend exited ${code}: ${log}`));});
    });
    for(let round=0;round<2;round++) {
      const result=run(['exact.mjs','test','web','--backend',`http://127.0.0.1:${port}`],app);
      assert.match(result.stdout,/2 passed, 0 failed/);
    }
    assert.deepEqual(['app.contract','app.ts','app.test.contract'].map(f=>readFileSync(join(app,f),'utf8')),authored);
    run(['exact.mjs','contract','types','--check','app.contract'],app);
  } finally {
    if(server && server.exitCode===null){const ended=new Promise(ok=>server.once('exit',ok));server.kill('SIGTERM');await ended;}
    rmSync(project,{recursive:true,force:true});
  }
},600_000);
