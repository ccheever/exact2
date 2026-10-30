// @ref LLP 1043.000 §3 D7/D8 — flow settlement must not change LLP 1012's API.
import { test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import vm from 'node:vm';
import { render, sourceMapReader, identifyInspectedNode, heldTicket, holdOf, pickedPaths } from '../../scripts/agent.mjs';
import { retainDevGeneration, readDevGeneration, readDevGenerationAsync, serveBuildTree } from './serve.mjs';
import { placeReporter, timeReporter, pageReporter, appRootFontSize, grantOrigins } from './navigation.js';
import { storageKey } from './storage-environment.js';
import { open } from '../../scripts/agent.mjs';
import { launchFacts, launchEnvironment, parseFlags } from '../../scripts/agent-launch.mjs';
import { nodeNamed } from '../../scripts/agent-test.mjs';
import { browserMissing } from '../../scripts/agent-playwright.mjs'; import { required } from '../../scripts/coverage.mjs';
import { declaration, deferred, fixture, requestClock, source, timedRequests } from './glue-fixture.mjs';

const compiler = resolve(process.env.CARGO_TARGET_DIR ?? fileURLToPath(new URL('../../target', import.meta.url)), 'debug', process.platform === 'win32' ? 'contract.exe' : 'contract');

const mapAt = (digest, line = 12) => ({digest, nodes: [{file: '/app/ui/bubble.contract', line, col: 3, end_col: 9, component: 'Bubble',
  chain: [{file: '/app/app.contract', line: 45, col: 5, end_col: 11, component: 'App'}],
  bindings: [{row:'color',origin:'class:Bubble'}, {row:'font-size',origin:'own'}]}]});
const inspected = planDigest => ({id: 1, site: 0, planDigest, props: {testId:'bubble'}, type:'Text', style: {
  color: {value:'red',source:'dynamic'}, 'font-size': {value:14,source:'inherited',from:2}}});

test('authored tests use the configured compiler target and preserve compiler and launch failures', () => {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), 'exact-test-compiler-')));
  const root = new URL('../../', import.meta.url).pathname;
  const target = join(dir, 'custom target'), tools = join(dir, 'tools');
  const file = join(dir, 'suite.test.contract'), runner = join(dir, 'run.mjs');
  mkdirSync(tools);
  // A Cargo fixture materializes a compiler only in its configured target.
  // It supports both build-then-execute and cargo run; no real Cargo lock is
  // acquired from inside a test that may itself be running under Cargo.
  const compiler = `#!${process.execPath}\nimport {readFileSync,writeFileSync} from 'node:fs';
writeFileSync(process.env.COMPILER_TRACE, JSON.stringify(process.argv.slice(2)));
if (readFileSync(process.argv[3], 'utf8') === 'refuse') { console.error('fixture.contract:7:3: invalid test step'); process.exit(2); }
console.log('{"tests":[],"properties":[]}');\n`;
  writeFileSync(join(tools, 'cargo'), `#!${process.execPath}\nimport {mkdirSync,writeFileSync} from 'node:fs';
import {resolve,dirname} from 'node:path'; import {spawnSync} from 'node:child_process';
const bin=resolve(process.env.CARGO_TARGET_DIR,'debug/contract'), args=process.argv.slice(2);
mkdirSync(dirname(bin),{recursive:true}); writeFileSync(bin,${JSON.stringify(compiler)},{mode:0o755});
if(args.includes('run')) { const r=spawnSync(bin,args.slice(args.indexOf('--')+1),{stdio:'inherit'}); process.exit(r.status ?? 1); }
`, {mode: 0o755});
  writeFileSync(runner, `import {runTests} from ${JSON.stringify(new URL('../../scripts/agent.mjs', import.meta.url).href)};
try { console.log(JSON.stringify(await runTests({host:'linux',file:'suite.test.contract'}))); }
catch(e) { console.error(e.message); process.exitCode=1; }
`);
  try {
    for (const configured of [target, relative(root, target)]) {
      const trace = join(dir, 'compiler.json');
      const env = {...process.env, PATH: tools, CARGO_TARGET_DIR: configured, COMPILER_TRACE: trace};
      writeFileSync(file, 'accept');
      const good = spawnSync(process.execPath, [runner], {cwd: dir, env, encoding:'utf8'});
      expect(good.status).toBe(0);
      expect(JSON.parse(good.stdout)).toEqual({passed:0,failed:0,skipped:0,results:[]});
      expect(JSON.parse(readFileSync(trace, 'utf8'))).toEqual(['test', file]);
      writeFileSync(file, 'refuse');
      const bad = spawnSync(process.execPath, [runner], {cwd: dir, env, encoding:'utf8'});
      expect(bad.status).toBe(1);
      expect(bad.stderr).toContain('fixture.contract:7:3: invalid test step');
    }
    // No cargo on PATH and none where rustup puts it, which the driver adds back (scripts/app.mjs cargoOnPath).
    const missing = spawnSync(process.execPath, [runner], {
      cwd:dir, env:{...process.env,PATH:join(dir,'absent'),CARGO_HOME:join(dir,'absent')}, encoding:'utf8',
    });
    expect(missing.status).toBe(1);
    expect(missing.stderr).toContain('cargo');
    expect(missing.stderr).not.toContain('TypeError');
  } finally { rmSync(dir, {recursive:true,force:true}); }
}, 60_000); // Six Bun launches of the driver: 25 s at load 110, so a hang bound, not a speed claim.

test('driver joins only the inspected plan, retains old compatible maps, and labels formatting-only revisions honestly', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-driver-map-')), path = join(dir, 'app.plan');
  const a = 'a'.repeat(64), b = 'b'.repeat(64), reader = sourceMapReader(path);
  try {
    expect(await reader.refresh()).toBe(false);
    writeFileSync(path + '.map.json', JSON.stringify(mapAt(a)));
    expect(await reader.refresh()).toBe(true);
    const node = inspected(a); reader.attach(node);
    expect(node.sourceMap.status).toBe('compatible');
    expect(node.sourceMap.line).toBe(12);
    expect(node.style.color.origin).toBe('class:Bubble');
    expect(node.style['font-size'].origin).toBeUndefined();
    writeFileSync(path + '.map.json', JSON.stringify(mapAt(b, 22)));
    await reader.refresh();
    const refusedReload = inspected(a); reader.attach(refusedReload);
    expect(refusedReload.sourceMap.line).toBe(12);
    const accepted = inspected(b); reader.attach(accepted);
    expect(accepted.sourceMap.line).toBe(22);
    const stale = inspected('c'.repeat(64)); reader.attach(stale);
    expect(stale.sourceMap.status).toBe('unavailable');
    writeFileSync(path + '.map.json', JSON.stringify(mapAt(b, 24)));
    await reader.refresh(); reader.attach(accepted);
    expect(accepted.sourceMap.line).toBe(24);
    expect(accepted.sourceMap.status).toBe('compatible');
    const malformed = mapAt(b); malformed.nodes[0].line = -1;
    writeFileSync(path + '.map.json', JSON.stringify(malformed));
    await reader.refresh(); reader.attach(inspected(a));
    const invalid = inspected(b); reader.attach(invalid);
    expect(invalid.sourceMap.status).toBe('unavailable');
    writeFileSync(path + '.map.json', '{bad JSON');
    expect(await reader.refresh()).toBe(true); // Valid older entries remain useful.
    const noIdentity = inspected(undefined); reader.attach(noIdentity);
    expect(noIdentity.sourceMap.status).toBe('unavailable');
  } finally { rmSync(dir, {recursive:true,force:true}); }
});

test('local map refresh observes same-size edits with restored timestamps and recovers after missing or malformed files', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-driver-map-refresh-')), path = join(dir, 'app.plan');
  const file = path + '.map.json', digest = 'a'.repeat(64), reader = sourceMapReader(path);
  const node = inspected(digest);
  try {
    const first = JSON.stringify(mapAt(digest, 12)), changed = JSON.stringify(mapAt(digest, 24));
    expect(first.length).toBe(changed.length);
    writeFileSync(file, first);
    const stat = statSync(file);
    for (let i = 0; i < 3; i++) {
      expect(await reader.refresh()).toBe(true); reader.attach(node);
      expect(node.sourceMap.line).toBe(12);
    }
    writeFileSync(file, changed); utimesSync(file, stat.atime, stat.mtime);
    expect(await reader.refresh()).toBe(true); reader.attach(node);
    expect(node.sourceMap.line).toBe(24);
    for (const missing of [false, true]) {
      if (missing) rmSync(file); else writeFileSync(file, '{bad JSON');
      expect(await reader.refresh()).toBe(true); reader.attach(node);
      expect(node.sourceMap.line).toBe(24);
      writeFileSync(file, changed);
      expect(await reader.refresh()).toBe(true); reader.attach(node);
      expect(node.sourceMap.line).toBe(24);
    }
    writeFileSync(file, first);
    expect(await reader.refresh()).toBe(true); reader.attach(node);
    expect(node.sourceMap.line).toBe(12);
  } finally { rmSync(dir, {recursive:true,force:true}); }
});

test('HTTP map discovery validates its card, origin and plan, independently of the node identity', async () => {
  const digest = 'a'.repeat(64), body = Buffer.from(JSON.stringify(mapAt(digest)));
  let mapURL = '/map', cardHash = createHash('sha256').update(body).digest('hex'), planHash = digest, mapReads = 0;
  const server = createServer((req,res) => {
    if (req.url === '/exact.json') res.end(JSON.stringify({plan:{sha256:planHash},dev:{sourceMap:{url:mapURL,sha256:cardHash,bytes:body.length}}}));
    else { mapReads++; res.end(body); }
  });
  await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
  const url = `http://127.0.0.1:${server.address().port}/nested/app`;
  try {
    const reader = sourceMapReader(url);
    expect(await reader.refresh()).toBe(true);
    expect(await reader.refresh()).toBe(true);
    expect(mapReads).toBe(1);
    const node = inspected(digest); reader.attach(node);
    expect(node.sourceMap.status).toBe('compatible');
    const different = inspected('b'.repeat(64)); reader.attach(different);
    expect(different.sourceMap.status).toBe('unavailable');
    cardHash = 'b'.repeat(64);
    expect(await sourceMapReader(url).refresh()).toBe(false);
    cardHash = createHash('sha256').update(body).digest('hex'); planHash = 'b'.repeat(64);
    expect(await sourceMapReader(url).refresh()).toBe(false);
    planHash = digest; mapURL = 'http://different.invalid/map';
    expect(await sourceMapReader(url).refresh()).toBe(false);
    expect(mapReads).toBe(3);
  } finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
});

test('targeted inspection uses its own node identity and renders declaration, callers and winning styles', async () => {
  const node = inspected('a'.repeat(64)), reply = {node,nodes:[{id:1,x:0,y:0,w:20,h:10}],viewport:{w:100,h:100},clock:0};
  identifyInspectedNode(reply,'bubble');
  expect(reply.nodes[0].testId).toBe('bubble');
  expect(() => identifyInspectedNode(reply,'replacement')).toThrow('changed during inspection');
  identifyInspectedNode(reply,1);
  node.sourceMap = {status:'compatible',...mapAt(node.planDigest).nodes[0]};
  node.style.color.origin = 'class:Bubble';
  const output = render('layout',reply);
  expect(output).toContain('@ /app/ui/bubble.contract:12:3 (Bubble) · compatible source map');
  expect(output).toContain('called from App @ /app/app.contract:45:5');
  expect(output).toContain('(dynamic, class:Bubble)');
  expect(output).not.toContain(node.planDigest);
});

test('retained development source maps survive later generations and refuse mismatched plans', async () => {
  const cache = mkdtempSync(join(tmpdir(), 'exact-dev-source-map-'));
  const epoch = 'a'.repeat(32), prefix = `/__dev/generation/${epoch}/`;
  const digest = bytes => createHash('sha256').update(bytes).digest('hex');
  const card = bytes => ({ bytes: bytes.length, sha256: digest(bytes) });
  try {
    for (const seq of [1, 2, 3, 4]) {
      const plan = Buffer.from(`plan ${seq}`);
      const map = Buffer.from(JSON.stringify({digest: seq === 3 ? digest('different') : digest(plan), nodes: [{file: `source-${seq}.contract`}]}));
      const envelope = Buffer.from(JSON.stringify({exact: 1, plan: card(plan), assets: [], dev: {epoch, seq, ...(seq === 4 ? {} : {sourceMap: card(map)})}}));
      retainDevGeneration(cache, epoch, seq, new Map([['app.plan', plan], ['app.plan.map.json', map], ['exact.json', envelope]]));
    }
    expect(JSON.parse(readDevGeneration(cache, prefix + '1/app.plan.map.json').body).nodes[0].file).toBe('source-1.contract');
    expect(JSON.parse((await readDevGenerationAsync(cache, prefix + '2/app.plan.map.json')).body).nodes[0].file).toBe('source-2.contract');
    expect(readDevGeneration(cache, prefix + '3/app.plan.map.json')).toBeNull();
    expect(await readDevGenerationAsync(cache, prefix + '4/app.plan.map.json')).toBeNull();
  } finally { rmSync(cache, {recursive: true, force: true}); }
});

function plain(reply) {
  expect(reply).toBeDefined();
  expect(typeof reply).toBe('object');
  expect(reply.then).toBeUndefined();
}

test('only an explicit targeted layout carries its same-reply accepted plan', () => {
  const c = fixture(), requests = [];
  c.ask = request => {
    requests.push(request);
    if (request.op === 'node') return {id: request.id, type: 'Text', props: {testId:'current'}, ...(request.plan ? {planDigest:'a'.repeat(64)} : {})};
    return {epoch:2,incarnation:1,clock:0};
  };
  const initial = c.exact.agent({op:'layout',id:1});
  plain(initial);
  expect(initial.error).toBeUndefined();
  expect(initial.node.planDigest).toBeUndefined();
  const result = c.exact.agent({op:'layout',id:1,plan:true});
  plain(result);
  expect(result.node.planDigest).toBe('a'.repeat(64));
  expect(requests.filter(r=>r.op==='node').map(r=>r.plan ?? false)).toEqual([false,true]);
});

test('ordinary reads and inputs are synchronous; the awaited entry returns the same reply', async () => {
  const f = fixture();
  for (const op of ['state', 'tree', 'logs', 'tags', 'layout', 'focus', 'tap', 'type', 'unknown']) {
    const request = { op, id: 1 }, reply = f.exact.agent(request);
    plain(reply);
    expect(await f.exact.agentSettled(request)).toEqual(reply);
  }
  expect(f.exact.agent({ op: 'state' }).slots.backPresses).toBe(3);
  expect(f.exact.agent({ op: 'tree' }).roots[0]).toBe(1);
  expect(await f.exact.agentSettled({ op: 'logs' })).toBe(f.logs);
  expect(f.exact.agent({ op: 'layout', id: 1 }).node.flow).toBeUndefined();
  // No flow means not even one internal microtask before dispatch.
  delete f.state.navigation;
  const pending = f.exact.agentSettled({ op: 'state' });
  expect(f.state.navigation).toEqual({ location: '/' });
  await pending;
  f.wasm = null;
  expect(f.exact.agent({ op: 'state' })).toEqual({ error: 'not booted' });
});

test('GPU pending prevents the awaited operation from reading live state', async () => {
  const f = fixture();
  let reads = 0;
  f.ask = request => { reads++; return request.op === 'tags' ? {epoch:2,incarnation:1,clock:0} : f.state; };
  f.exact.gpu = { settled: async () => [{name:'GPU recovery world'}] };
  const reply = await f.exact.agentSettled({op:'state'});
  expect(reply.error).toContain('GPU is not settled');
  expect(reply.pending).toEqual(['GPU recovery world']);
  expect(reads).toBe(0);
  const clock = await f.exact.agentSettled({op:'clock',settle:true});
  expect(clock).toEqual({clock:0,settled:false,reason:'gpu',pending:['GPU recovery world']});
  expect(reads).toBe(0);
});

test('only calls before the inspection stage arrives wait for it; later ones are synchronous again', async () => {
  const f = fixture();
  let loaded = false, loads = 0;
  f.stageLoaded = () => loaded;
  f.loadStage = () => { loads++; loaded = true; return Promise.resolve(); };
  const first = f.exact.agent({ op: 'state' });
  expect(typeof first.then).toBe('function');
  expect((await first).slots.backPresses).toBe(3);
  plain(f.exact.agent({ op: 'state' }));
  expect(loads).toBe(1);
});

test('pending module and settlement leave synchronous reads usable with last settled facts', async () => {
  const f = fixture(), loading = deferred(), settling = deferred();
  const old = { shapes: [{ kind: 'Circle', cx: 10 }], fragments: [{ start: 0, end: 3 }] };
  const next = { shapes: [{ kind: 'Circle', cx: 20 }], fragments: [{ start: 0, end: 6 }] };
  let facts = old, calls = 0;
  f.flowLoading = loading.promise;
  const pending = f.exact.agentSettled({ op: 'layout', id: 1 });
  let answered = false;
  pending.then(() => { answered = true; });
  // Worst realistic input: a module that has not loaded must not stall reads.
  for (let i = 0; i < 1000; i++) expect(f.exact.agent({ op: 'state' }).slots.backPresses).toBe(3);
  const entered = deferred();
  f.textflow = { facts: () => facts, async settle() { calls++; entered.resolve(); await settling.promise; facts = next; } };
  loading.resolve();
  await entered.promise; // VM promise assimilation may cross more than one microtask.
  expect(calls).toBe(1);
  expect(answered).toBe(false);
  const before = f.exact.agent({ op: 'layout', id: 1 });
  plain(before);
  expect(before.node.flow).toBe(old);
  expect(before.node.flow_shapes).toBe(old.shapes);
  settling.resolve();
  const after = await pending;
  expect(after.node.flow).toBe(next);
  expect(after).toEqual(f.exact.agent({ op: 'layout', id: 1 }));
  expect(calls).toBe(1); // Synchronous reads never trigger settlement work.
  expect(await f.exact.agentSettled({ op: 'logs' })).toBe(f.logs);
});

test('flow loader or executor failure rejects only the awaited entry', async () => {
  const f = fixture();
  f.flowLoading = Promise.reject(new Error('module failed'));
  expect(f.exact.agent({ op: 'tree' })).toBe(f.outline);
  await expect(f.exact.agentSettled({ op: 'tree' })).rejects.toThrow('module failed');
  f.flowLoading = null;
  f.textflow = { settle: async () => { throw Error('settle failed'); } };
  await expect(f.exact.agentSettled({ op: 'tree' })).rejects.toThrow('settle failed');
  expect(f.exact.agent({ op: 'tree' })).toBe(f.outline);
});

test('agent mode alone exposes both entry points; clock keeps its existing Promise', async () => {
  const normal = fixture(false);
  expect(normal.exact.agent).toBeUndefined();
  expect(normal.exact.agentSettled).toBeUndefined();
  const f = fixture();
  const result = f.exact.agent({ op: 'clock', to: 16 });
  expect(typeof result.then).toBe('function');
  let completed = false;
  result.then(() => { completed = true; });
  await Promise.resolve(); await Promise.resolve();
  expect(completed).toBe(true); // The clock's and the tags' only: no flow or GPU microtasks for ordinary apps.
  expect(await result).toEqual({ clock: 16, epoch: 2, incarnation: 1 });
});

test('a clock waits for an animated image to land on the clock it moved to (LLP 1011.000)', async () => {
  const f = fixture();
  let land;
  const landed = new Promise(r => { land = r; });
  f.imageHold = Promise.resolve({ ready: () => landed });
  let completed = false;
  const result = f.exact.agent({ op: 'clock', to: 16 });
  result.then(() => { completed = true; });
  for (let i = 0; i < 5; i++) await Promise.resolve();
  expect(completed).toBe(false);
  land();
  expect(await result).toEqual({ clock: 16, epoch: 2, incarnation: 1 });
});

test('a clock waits for runner requests with no host promise, through release or the deadline', async () => {
  for (const request of [{ settle: true }, { data: true }]) {
    for (const release of [false, true]) {
      const f = fixture(), elapsed = requestClock(f);
      f.SETTLE_DEADLINE_MS = 100;
      f.requestsPending = 1;
      f.pollRequests = () => { if (release) { f.requestsPending = 0; f.events.push('release'); } };
      f.wasm.exact_advance = to => { f.events.push('advance'); return JSON.stringify({ ops: [], clock: to }); };
      const reply = await f.exact.agent({ op: 'clock', ...request });
      expect(f.inflight.size).toBe(0);
      if (release) {
        expect(f.events[0]).toBe('release');
        expect(reply.settled).toBe(true);
      } else {
        expect(elapsed()).toBe(100);
        expect(reply).toMatchObject({ settled: false, reason: 'requests' });
      }
    }
  }
});

test('a runner request started by the final commit prevents a settled clock reply', async () => {
  for (const request of [{ settle: true }, { data: true }]) {
    const f = fixture(); requestClock(f);
    let pending = false;
    f.wasm.exact_requests_pending = () => Number(pending);
    f.waitForInflight = async () => !pending;
    f.wasm.exact_advance = to => { pending = true; return JSON.stringify({ ops: [], clock: to }); };
    expect(await f.exact.agent({ op: 'clock', ...request })).toMatchObject({ settled: false, reason: 'requests' });
  }
});

test('a declared server hold stays named without holding settle or data', async () => {
  const f = fixture(), elapsed = requestClock(f);
  f.state.pending = [{ name: 'changes', ticket: 7, held: true }];
  f.requestsPending = 0; // has_pending() excludes this hold; pending() still contains its ticket.
  f.heldAsks.set(7, { target: 'changes', owner: 0 });
  for (const request of [{ settle: true }, { data: true }]) {
    expect(await f.exact.agent({ op: 'clock', ...request })).toMatchObject({ settled: true, held: 1, heldTargets: ['changes'] });
  }
  expect(elapsed()).toBe(0);
  f.state.pending.push({ name: 'library', ticket: 8 }); f.requestsPending = 1;
  for (const request of [{ settle: true }, { data: true }]) {
    expect(await f.exact.agent({ op: 'clock', ...request })).toMatchObject({ settled: false, reason: 'requests', held: 1, heldTargets: ['changes'] });
  }
});

test('a clock jump stops at each intermediate timer when a runner request is parked', async () => {
  for (const pending of [0, 1]) {
    const f = fixture(), elapsed = requestClock(f);
    f.requestsPending = pending;
    f.heldAsks.set(7, { target: 'changes', owner: 0 });
    timedRequests(f, { parked: true });
    expect(await f.exact.agent({ op: 'clock', to: 1000 })).toMatchObject({ clock: 1000, held: 1, heldTargets: ['changes'] });
    expect(f.events).toEqual(['advance 1000 until a request → 300', 'advance 1000 until a request → 600', 'advance 1000 until a request → 900', 'advance 1000 → 1000']);
    expect(elapsed()).toBe(0);
    expect(f.inflight.size).toBe(0);
    expect(f.requestsPending).toBe(1);
  }
});

test('a clock jump lands what is in flight before each timer fires', async () => {
  const f = fixture();
  timedRequests(f);
  const boot = new Promise(resolve => setTimeout(() => { f.events.push('reply boot'); resolve(); }, 0));
  f.inflight.add(boot); boot.finally(() => f.inflight.delete(boot));
  // The last advance fires no timer, so the request it left in flight is not waited for; the reply names it (calendar F10).
  expect(await f.exact.agent({ op: 'clock', to: 700 })).toEqual({ clock: 700, epoch: 2, incarnation: 1, inflight: 1 });
  expect(f.events).toEqual(['reply boot', 'advance 700 until a request → 300', 'reply 300', 'advance 700 until a request → 600', 'advance 700 → 700']);
  expect(f.inflight.size).toBe(1);
  // A due time at the target itself is the last stop: what is in flight lands first, nothing follows.
  f.events.length = 0;
  expect(await f.exact.agent({ op: 'clock', to: 900 })).toEqual({ clock: 900, epoch: 2, incarnation: 1, inflight: 1 });
  expect(f.events).toEqual(['reply 600', 'advance 900 until a request → 900', 'advance 900 → 900']);
});
test('a jump does not wait on a request the server holds, and names it', async () => {
  const f = fixture();
  timedRequests(f);
  // A held ask (`exactHeld`, LLP 1016.000 D9) is listed, never tracked: it would never land in this test.
  f.heldAsks.set(7, { target: 'changes', owner: 0 });
  expect(await f.exact.agent({ op: 'clock', to: 700 })).toEqual({ clock: 700, epoch: 2, incarnation: 1, inflight: 1, held: 1, heldTargets: ['changes'] });
  expect(f.events).toEqual(['advance 700 until a request → 300', 'reply 300', 'advance 700 until a request → 600', 'advance 700 → 700']);
  // Let go by the runner (a send to its target superseded it), it is no longer named.
  f.letGoTickets = new Set([7]);
  expect(await f.exact.agent({ op: 'clock', to: 900 })).toEqual({ clock: 900, epoch: 2, incarnation: 1, inflight: 1 });
});
test('a jump that stops at its target still fires the other timers due there', async () => {
  const f = fixture();
  // Two timers due at 300: the first sends, the second only counts.
  const due = [300, 300];
  let counted = 0;
  f.flowDue = 300;
  f.wasm.exact_advance = (to, untilRequest) => {
    let clock = to;
    while (due.length && due[0] <= to) {
      const at = due.shift();
      if (due.length) { const p = Promise.resolve(); f.inflight.add(p); p.finally(() => f.inflight.delete(p)); if (untilRequest) { clock = at; break; } }
      else counted++;
    }
    f.events.push(`advance ${to}${untilRequest ? ' until a request' : ''} → ${clock}`);
    return JSON.stringify({ ops: [], clock, timers: true, timer_due_ms: due[0] ?? null });
  };
  f.applyBatch = batch => { f.agentClock = batch.clock; f.flowDue = batch.timer_due_ms; f.flowLanding = !!batch.landing; return { timers: true, batch }; };
  expect(await f.exact.agent({ op: 'clock', to: 300 })).toEqual({ clock: 300, epoch: 2, incarnation: 1 });
  expect(counted).toBe(1);
  expect(f.events).toEqual(['advance 300 until a request → 300', 'advance 300 until a request → 300', 'advance 300 → 300']);
});
test("a jump lands a data module's released timer before it goes on, so chained sleeps are due one after another", async () => {
  const f = fixture();
  // An answer sleeping 100, 200 and 400 ms in turn: each timer is armed when the last one's continuation lands,
  // due from the clock then. A released continuation is `landing` until it does, with no timer due meanwhile.
  const gaps = [200, 400];
  let now = 0, due = 100, landing = false;
  f.flowDue = 100;
  f.wasm.exact_advance = (to, untilRequest) => {
    let clock = to;
    if (due !== null && due <= to) {
      clock = due; due = null; landing = true;
      const p = new Promise(r => setTimeout(r, 5)).then(() => { landing = false; const gap = gaps.shift(); if (gap !== undefined) due = now + gap; });
      f.inflight.add(p); p.finally(() => f.inflight.delete(p));
      if (!untilRequest) clock = to;
    }
    now = clock;
    f.events.push(`advance ${to}${untilRequest ? ' until a request' : ''} → ${clock}`);
    return JSON.stringify({ ops: [], clock, timers: due !== null, timer_due_ms: due, ...(landing ? { landing: true } : {}) });
  };
  f.applyBatch = batch => { f.agentClock = batch.clock; f.flowDue = batch.timer_due_ms; f.flowLanding = !!batch.landing; return { timers: true, batch }; };
  expect(await f.exact.agent({ op: 'clock', to: 1000 })).toEqual({ clock: 1000, epoch: 2, incarnation: 1 });
  expect(f.events.filter(e => / → (100|300|700)$/.test(e))).toEqual(['advance 1000 until a request → 100', 'advance 1000 until a request → 300', 'advance 1000 until a request → 700']);
});
test('a timer whose request never lands cannot hold the clock: past the deadline the rest is one advance', async () => {
  const f = fixture();
  timedRequests(f, { stuck: true });
  expect(await f.exact.agent({ op: 'clock', to: 1000 })).toEqual({ clock: 1000, epoch: 2, incarnation: 1, inflight: 3 });
  expect(f.events).toEqual(['advance 1000 until a request → 300', 'advance 1000 → 1000']);
});

test('launch setup supplies fixed defaults and carries CLI overrides to every host', () => {
  expect(launchFacts({})).toEqual({seed:1, locale:'en-US', timeZone:'UTC', epoch:Date.UTC(2026, 0, 1)});
  const {flags, rest} = parseFlags(['web', '--browser', 'firefox', '--seed', '4294967295', '--locale', 'fr-ca', '--time-zone', 'America/Toronto', '--epoch', '2026-09-21T14:13:20Z', 'tree']);
  expect(rest).toEqual(['web', 'tree']);
  expect(flags.browser).toBe('firefox');
  const facts = launchFacts(flags);
  expect(facts).toEqual({seed:4294967295, locale:'fr-CA', timeZone:'America/Toronto', epoch:1790000000000});
  expect(launchFacts({epoch:'1790000000000'}).epoch).toBe(1790000000000);
  const time = timeReporter(new URLSearchParams({agent:'1', ...facts}), new Proxy({}, {get() { throw new Error('agent read the platform'); }}));
  expect(time(60000)).toEqual([1790000000000, -240]);
  expect(timeReporter(new URLSearchParams({agent:'1'}))(0)).toEqual([Date.UTC(2026, 0, 1), 0]);
  // The offset follows the virtual date across a DST change (LLP 1069.007 D2).
  const spring = timeReporter(new URLSearchParams({agent:'1', timeZone:'America/Los_Angeles', epoch:String(Date.UTC(2026, 2, 8, 9))}));
  expect(spring(1800000)).toEqual([Date.UTC(2026, 2, 8, 9), -480]);
  expect(spring(7200000)).toEqual([Date.UTC(2026, 2, 8, 9), -420]);
  for (const epoch of ['-1', 'yesterday', 'NOW', 'now ']) expect(() => launchFacts({epoch})).toThrow('epoch:');
  expect(launchFacts({env:launchEnvironment(facts)})).toEqual(facts);
  const params = new URLSearchParams({agent:'1', ...facts});
  const report = placeReporter(params, new Proxy({}, {get() { throw new Error('agent read the platform'); }}));
  expect([report(), report()]).toEqual(Array(2).fill(['fr-CA', 'America/Toronto', 4294967295, ''].join('\0')));
  for (const seed of [-1, 0.5, NaN, Infinity, 4294967296]) for (const f of [() => launchFacts({seed}), () => placeReporter(new URLSearchParams({agent:'1', seed:String(seed)}))()]) expect(f).toThrow('seed: a whole number from 0 through 2^32 - 1'); // below 2^32 on every host (LLP 1116.005 D5): refused by the driver and the page,
  for (let i = 0, drawn; i < 64; i++) expect([(drawn = placeReporter(new URLSearchParams()))(), Number(drawn().split('\0')[2]) < 2 ** 32]).toEqual([drawn(), true]); // and drawn below it, once a launch
  expect(() => launchFacts({locale:'en_US'})).toThrow();
  expect(() => launchFacts({timeZone:'Not/AZone'})).toThrow();
});

test('programmatic web opens stay on Chrome and Firefox drives a small Exact plan', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-firefox-agent-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist'), png = join(dir, 'page.png');
  writeFileSync(contract, `component BrowserFixture
  state presses = 0
  state words = ""
  action pressed
    presses = presses + 1
  action changed(value: string)
    words = value
  view
    column testId="root" gap=8 padding=8
      button testId="press" press=pressed
        text "Press"
      input testId="field" value=words input=changed
      column testId="touch" touch-action="none" width=160 height=80 background-color="#cccccc"
      scroll testId="scroll" width=160 height=60
        column height=600
          text "Long"
`);
  const compile = spawnSync(compiler, ['build', contract, '-o', plan], { encoding:'utf8' });
  expect(compile.status, compile.stderr).toBe(0);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd:new URL('../../', import.meta.url).pathname, encoding:'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${server.address().port}/`;
  let session, browserProcess, chrome;
  const selected = process.env.EXACT_WEB_BROWSER;
  try {
    process.env.EXACT_WEB_BROWSER = 'firefox';
    chrome = await open({ host:'web', url });
    expect(chrome.carrier.browser).toBe('chrome');
    // evaluate takes a function of no arguments as Playwright's carriers do (r23 t2).
    expect(await chrome.carrier.evaluate(() => 1 + 1)).toBe(2);
    expect(await chrome.carrier.evaluate('1 + 2')).toBe(3);
    expect(await chrome.carrier.evaluate(async () => 4)).toBe(4);
    expect(await chrome.carrier.evaluate(({ probe() { return 5; } }).probe)).toBe(5);
    expect(await chrome.carrier.evaluate(({ async probe() { return 6; } }).probe)).toBe(6);
    await chrome.close(); chrome = null;
    // The Firefox half: a missing Firefox fails under EXACT_REQUIRED (scripts/coverage.mjs), else the Chrome half stands.
    const firefoxMissing = await browserMissing('firefox');
    if (firefoxMissing && required()) throw new Error(`missing firefox: ${firefoxMissing} (EXACT_REQUIRED=1)`);
    if (firefoxMissing) return console.warn(`SKIP: missing firefox: ${firefoxMissing}`);
    session = await open({ host:'web', browser:'firefox', url, onProcess: child => { browserProcess = child; } });
    expect(session.carrier.browser).toBe('firefox');
    expect(browserProcess?.pid).toBeGreaterThan(0);
    expect((await session.tree()).nodes.some(node => node.props.testId === 'press')).toBe(true);
    expect((await session.layout()).nodes.some(node => node.testId === 'scroll')).toBe(true);
    expect((await session.state()).slots.presses).toBe(0);
    expect(Array.isArray((await session.logs()).lines)).toBe(true);
    await session.tap('press');
    await session.type('field', 'hi');
    await session.type('field', {key:'a'});
    await session.type('press', {key:'Enter',phase:'down'});
    await session.type('press', {key:'Enter',phase:'up'});
    await session.type('press', {key:'a'}); // a key by its name, on a button too (pomodoro F5)
    await expect(session.type('press', {key:'Hyper'})).rejects.toThrow('key: unsupported key Hyper');
    const beforeRefusals = JSON.stringify((await session.state()).slots);
    await expect(session.tap('touch', {down:true})).rejects.toThrow('firefox down unsupported:');
    await expect(session.pointer('move', {dx:20,dy:10,ms:32})).rejects.toThrow('firefox move unsupported:');
    await expect(session.pointer('up')).rejects.toThrow('firefox up unsupported:');
    await expect(session.tap('touch', {pinch:1.2})).rejects.toThrow('firefox pinch unsupported:');
    expect(JSON.stringify((await session.state()).slots)).toBe(beforeRefusals);
    // A mouse drag is a real button, twice, so the first lift cleared the contact (drums R13).
    const dragged = await session.tap('touch', { drag: { dx: 20, dy: 0, mouse: true, over: 16 } });
    expect(dragged.delivery).toBe('platform');
    expect(dragged.drag.mouse).toBe(true);
    await session.tap('touch', { drag: { dx: -20, dy: 0, mouse: true, over: 16 } });
    await session.carrier.evaluate(`(() => { window.__exactShift = null; addEventListener('pointerdown', (e) => { window.__exactShift = e.shiftKey; }, { capture: true, once: true }); })()`);
    await session.tap('press', { modifiers: 'Shift' });
    expect(await session.carrier.evaluate('window.__exactShift')).toBe(true);
    await session.tap('scroll', {wheel:[0,120]});
    const state = (await session.state()).slots;
    expect(state.presses).toBeGreaterThan(0);
    expect(state.words).toBe('hia');
    await session.type('root', { key: ' ' }); // the column takes no focus; the key is still pressed (drums R13)
    expect((await session.layout()).nodes.find(node => node.testId === 'scroll').sy).toBeGreaterThan(0);
    expect((await session.clock('+25')).clock).toBe(25);
    expect((await session.prefer({'prefers-color-scheme':'dark'})).media['prefers-color-scheme']).toBe('dark');
    const media = await session.prefer({'prefers-reduced-motion':'reduce'});
    expect(media.media['prefers-color-scheme']).toBe('dark');
    expect(media.media['prefers-reduced-motion']).toBe('reduce');
    await expect(session.prefer({'prefers-contrast':'less'})).rejects.toThrow('firefox prefer cannot emulate prefers-contrast less through Playwright');
    await expect(session.prefer({'prefers-contrast':'custom'})).rejects.toThrow('firefox prefer cannot emulate prefers-contrast custom through Playwright');
    const button = (await session.layout()).nodes.find(node => node.testId === 'press');
    await session.carrier.evaluate(`(() => { const e=document.createElement('div'); e.id='cover'; Object.assign(e.style,{position:'fixed',zIndex:'9999',left:'${button.x}px',top:'${button.y}px',width:'${button.w}px',height:'${button.h}px'}); document.body.append(e); })()`);
    await expect(session.tap('press')).rejects.toThrow('covers its middle');
    await session.carrier.evaluate(`document.getElementById('cover').remove()`);
    expect((await session.screenshot(png)).w).toBeGreaterThan(0);
    expect(statSync(png).size).toBeGreaterThan(0);
  } finally {
    if (selected === undefined) delete process.env.EXACT_WEB_BROWSER; else process.env.EXACT_WEB_BROWSER = selected;
    await chrome?.close?.();
    await session?.close?.();
    await new Promise(resolve => server.close(resolve));
    rmSync(dir, {recursive:true,force:true});
  }
}, 120_000);

test('page facts: the platform off the agent, the drive\'s values under it (LLP 1069.000 D2, D6)', () => {
  const listened = [];
  const platform = { document: { visibilityState: 'hidden', addEventListener: name => listened.push(name) }, navigator: { onLine: false, share() {} }, addEventListener: name => listened.push(name) };
  const real = pageReporter(false, platform);
  expect(real.bits()).toBe(1 | 2 | 4 | 32); // no navigation entry is a page the browser navigated to (bits 5–6: 1)
  platform.document.visibilityState = 'visible'; platform.navigator = { onLine: true };
  expect(real.bits()).toBe(32);
  expect(pageReporter(false, { ...platform, performance: { getEntriesByType: t => t === 'navigation' ? [{ type: 'reload' }] : [] } }).bits()).toBe(2 << 5);
  // The document pickers (studio diary R31): bit 3 where the browser has them.
  platform.showOpenFilePicker = () => {}; expect(real.bits()).toBe(32 | 8);
  delete platform.showOpenFilePicker; let focused = false; platform.document.hasFocus = () => focused; expect(real.bits()).toBe(32 | 16); focused = true; expect(real.bits()).toBe(32); // #114: bit 4 while `document.hasFocus()` is false
  const on = {}; platform.addEventListener = (name, f) => { listened.push(name); on[name] = f; }; let told = 0; real.onChange(() => { told++; }); expect(listened).toEqual(['visibilitychange', 'online', 'offline', 'focus', 'blur', 'pagehide', 'pageshow']);
  on.pageshow({ persisted: false }); on.pagehide({ persisted: false }); expect([real.bits(), told]).toEqual([32, 0]); on.pagehide({ persisted: true }); expect([real.bits(), told]).toEqual([32 | 1024, 1]); on.pageshow({ persisted: true }); expect([real.bits(), told]).toEqual([32 | 1 << 16, 2]); // the back/forward cache (LLP 1069.000 D2): bit 10 while put away, the returns above bit 16; a load's and an unload's events are neither
  const agent = pageReporter(true, new Proxy({}, {get(_, name) { if (name === 'devicePixelRatio') return 2; throw new Error('agent read the platform'); }})); // the display's scale alone is the page's own under the agent too: no drive value stands in for it (LLP 1115.000 D10.3)
  expect(agent.bits()).toBe(4 | 8 | 32 | 256); // the drive navigated to the page: `navigate` (bits 5–6: 1); files in a share, never a picture (LLP 1125 D12)
  agent.prefer({ 'visibility-state': 'hidden', online: false, 'can-open-files': false, 'has-focus': false }); expect(agent.bits()).toBe(1 | 2 | 4 | 16 | 32 | 256); agent.prefer({ 'has-focus': 'true' });
  expect(agent.bits()).toBe(1 | 2 | 4 | 32 | 256); agent.prefer({ 'can-share-files': false, 'can-share-pictures': true }); expect(agent.bits()).toBe(1 | 2 | 4 | 32 | 512); agent.prefer({ 'can-share-files': true, 'can-share-pictures': false });
  const asked = [], sharing = pageReporter(false, { ...platform, File: class { constructor(_, name, { type }) { asked.push([name, type]); } }, navigator: { onLine: true, share() {}, canShare: d => d.files.length === 1 } }); expect(sharing.bits()).toBe(4 | 32 | 256); expect(asked).toEqual([['a.txt', 'text/plain']]); // files in a share where `navigator.canShare({files})` takes a small text file (LLP 1125 D12)
  expect(() => agent.prefer({ online: 'maybe', 'can-share': false })).toThrow('prefer: online');
  expect(agent.read()['can-share']).toBe(true); expect(agent.read()['display-scale']).toBe(2); expect(real.displayScale()).toBe(1); agent.prefer({ 'display-scale': 2 }); expect(() => agent.prefer({ 'display-scale': 3 })).toThrow("prefer: display-scale: 3: the page's is 2");
  agent.onChange(() => { throw new Error('agent listened to the platform'); });
});

test('the app\'s root font size: a rule over the root, read past by the host (LLP 1069.000 D3)', () => { const rules = new Map(), said = [], say = line => said.push(line), document = { documentElement: {}, head: { append: el => rules.set(el.id, el) }, getElementById: id => rules.get(id) ?? null, createElement: () => ({ remove() { rules.delete(this.id); } }) }; const platform = { document, getComputedStyle: () => { const r = [...rules.values()].find(r => !r.disabled); return { fontSize: r ? r.textContent.match(/font-size:([\d.]+)px/)[1] + 'px' : '18px' }; } }, host = pageReporter(false, platform); // the browser's own 18; an enabled rule's `!important` wins
  appRootFontSize(20, say, platform); expect(rules.get('exact-root-font-size').textContent).toBe(':root{font-size:20px!important}'); expect(platform.getComputedStyle().fontSize).toBe('20px'); expect(host.rootFontSize()).toBe(18); for (const bad of [0, -4, NaN, '20px', 1e300]) appRootFontSize(bad, say, platform); expect(said).toHaveLength(5); expect(said[0]).toBe('setRootFontSize(0) refused: the root font size is a number of px above 0, a percentage of the host\'s above 0 ("125%"), or "medium"'); expect(rules.get('exact-root-font-size').textContent).toBe(':root{font-size:20px!important}'); appRootFontSize('medium', null, platform); expect(rules.size).toBe(0); expect(host.rootFontSize()).toBe(18); }); test('the app\'s percentage root font size is a share of the host\'s, the agent\'s substitute included (LLP 1121 D6)', () => { const rules = new Map(), said = [], say = line => said.push(line), style = { setProperty(k, v) { this[k] = v; } }, document = { documentElement: { style }, head: { append: el => rules.set(el.id, el) }, getElementById: id => rules.get(id) ?? null, createElement: () => ({ remove() { rules.delete(this.id); } }) }, platform = { document }, rule = () => rules.get('exact-root-font-size').textContent; appRootFontSize('125%', null, platform); expect(rule()).toBe(':root{font-size:calc(var(--exact-host-font-size, 100%) * 1.25)!important}'); appRootFontSize('.5e2%', null, platform); expect(rule()).toContain('* 0.5)'); for (const bad of ['0%', '-5%', '125', '125.%', ' 125%', 'inf%', '1e400%', '1e300%', '1e-100%']) appRootFontSize(bad, say, platform); expect(said).toHaveLength(9); expect(rule()).toContain('* 0.5)'); const sized = px => ({ document, getComputedStyle: () => ({ fontSize: `${px}px` }) }); said.length = 0; /* the share is the runner's, host * (n / 100) as an f32, at the host's own size beneath the app's rule */ appRootFontSize('3.8924957342356035e-45%', say, sized(18)); appRootFontSize('1.4178431532488902e+39%', say, sized(24)); expect(said).toHaveLength(1); expect(rule()).toContain('* 1.41784315324889e+37)'); pageReporter(true, platform).prefer({ 'root-font-size': 20 }); expect(style.fontSize).toBe('20px'); expect(style['--exact-host-font-size']).toBe('20px'); });
test('agent launch facts never read the platform locale, zone or entropy', async () => {
  const f = fixture(), reported = [];
  f.navigator = { language: 'fr-FR' };
  f.Intl = { DateTimeFormat: () => ({ resolvedOptions: () => ({timeZone:'Europe/Paris'}) }) };
  let draws = 0;
  f.crypto = { getRandomValues: bytes => { draws++; bytes.set([1, 7]); return bytes; } };
  f.wasm.exact_set_place = wire => { reported.push(wire); return '{"ops":[]}'; };
  f.Date = { now() { throw new Error('agent read the machine clock'); } };
  f.wasm.exact_set_time = (epoch, offset) => { reported.push([epoch, offset]); return '{"ops":[]}'; };
  await f.boot(null);
  await f.boot(new Uint8Array([1]));
  const date = [Date.UTC(2026, 0, 1), 0];
  expect(reported).toEqual([date, ['en-US', 'UTC', 1, ''].join('\0'), date, ['en-US', 'UTC', 1, ''].join('\0')]); // no formatLocale given (#952)
  expect(draws).toBe(0);
});

test('ordinary web reloads keep the launch seed', async () => {
  const f = fixture(false), reported = [];
  f.navigator = { language: 'fr-FR' };
  f.Intl = { DateTimeFormat: () => ({ resolvedOptions: () => ({timeZone:'Europe/Paris'}) }) };
  let draws = 0;
  f.crypto = { getRandomValues: words => { words.set([++draws]); return words; } }; // one 32-bit word (LLP 1116.005 D5)
  f.wasm.exact_set_place = wire => { reported.push(wire); return '{"ops":[]}'; };
  await f.boot(null);
  await f.boot(new Uint8Array([1]));
  expect(reported).toEqual([['fr-FR', 'Europe/Paris', 1, ''].join('\0'), ['fr-FR', 'Europe/Paris', 1, ''].join('\0')]); // no formatLocale given (#952)
  expect(draws).toBe(1);
});

test('ordinary boot and restart do not yield between DOM commit and ticker startup', async () => {
  for (const bytes of [null, new Uint8Array([1])]) {
    const f = fixture(false), el = f.views.get(1);
    const boot = f.boot(bytes);
    expect(f.events).toEqual([]);
    expect(await boot).toBe(0);
    const expected = ['replace', 'batch', ...(bytes ? ['activate'] : []), 'ticker', ...(bytes ? ['raf'] : [])];
    expect(f.events).toEqual(expected);
    // A yield after DOM commit would let its queued microtask see no ticker.
    expect(f.commitTurns).toEqual([expected]);
    expect(f.retiredViews.has(el)).toBe(false);
  }
});

test('restart waits for an old flow load, disposes synchronously, and ordinary clocks resume', async () => {
  const f = fixture(false), loading = deferred(), el = f.views.get(1);
  f.flowLoading = loading.promise;
  const restart = f.boot(new Uint8Array([1]));
  await Promise.resolve();
  expect(f.events).toEqual([]);
  f.textflow = { dispose() {
    expect(f.retiredViews.has(el)).toBe(true);
    f.events.push('dispose');
  } };
  loading.resolve();
  await restart;
  expect(f.events).toEqual(['dispose', 'replace', 'batch', 'activate', 'ticker', 'raf']);
  expect(f.textflow).toBeNull();
  expect(f.flowLoading).toBeNull();
});

test('non-flow operation transcripts remain byte-for-byte equal to the existing fixture', () => {
  const samples = JSON.parse(readFileSync(new URL('../../scripts/fixtures/transcript.json', import.meta.url), 'utf8'));
  const expected = readFileSync(new URL('../../scripts/fixtures/transcript.txt', import.meta.url), 'utf8');
  const actual = Object.entries(samples).map(([name, value]) => `--- ${name}\n${render(['empty', 'dropped'].includes(name) ? 'logs' : name.split(' ')[0], value)}`).join('\n\n') + '\n';
  expect(actual).toBe(expected);
});

// Run the input adapter with controlled scheduling; browser tests cover geometry.
let inputSource = readFileSync(new URL('./input-glue.js', import.meta.url), 'utf8');
for (const name of ['prompt-glue', 'alert-message']) {
  const stripped = inputSource.replace(new RegExp(`^import\\s+["']\\./${name}\\.js["'];[^\\n]*\\n`, 'm'), '');
  if (stripped === inputSource) throw new Error(`input glue must retain its ${name} side-effect import`);
  inputSource = stripped;
}
const inputCode = inputSource.replace(/^export (async function|function|const) /gm, '$1 ');
const inputScript = new vm.Script(inputCode + '\n' + declaration('attach'), { filename: 'input-glue.js' });

function inputFixture() {
  const listeners = new Map(), windowListeners = new Map(), frames = new Map(), sent = [], captured = [];
  let serial = 0;
  const el = { dataset: {}, inert: false, disabled: false, isConnected: true,
    addEventListener(kind, fn) { const list = listeners.get(kind) ?? []; list.push(fn); listeners.set(kind, list); },
    closest() { return this.disabled ? this : null; }, matches: () => false, contains: () => true, setPointerCapture(id) { captured.push(id); } };
  const buttons = [];
  // `page` is a built document being adopted (LLP 1048.000 D6); this page was not built.
  const f = vm.createContext({ inputReady: false, inputHandlers: null, page: null,
    views: new Map([[7, el]]), retiredViews: new WeakSet(), frames, sent, captured, el,
    root: { querySelectorAll: s => s.includes('aria-modal') ? [] : buttons, addEventListener() {}, contains: () => true }, // press feedback's listener: press.test.mjs drives it
    // The shortcuts' keydown listens in the capture phase, a pressable's activation in the bubble phase.
    document: { addEventListener(kind, fn, capture) { if (kind === 'keydown') f[capture ? 'keydown' : 'keyActivate'] = fn; }, activeElement: { closest: () => null }, getElementById: id => (id === 'exact-snap' ? {} : null), styleSheets: [] },
    HTMLIFrameElement: class {}, HTMLInputElement: class {}, HTMLTextAreaElement: class {}, HTMLButtonElement: class {},
    MutationObserver: class { observe() {} disconnect() {} takeRecords() { return []; } }, queueMicrotask, Element: class {}, matchMedia: () => ({ matches: false, addEventListener() {} }), // the input glue's start: sheets, looks, hover
    inertAncestor: node => node.inert, getComputedStyle: () => ({ visibility: 'visible' }),
    requestAnimationFrame(fn) { frames.set(++serial, fn); return serial; },
    cancelAnimationFrame(id) { frames.delete(id); },
    wasm: { exact_dispatch: (...args) => args }, writeIn: value => value, now: () => 0, performance: { now: () => 0 }, // (a pressable's press dims from a key too: its start, bf527d835f)
    send: value => sent.push(value),
    // The window's capture listeners: they hear a contact's events wherever they land.
    addEventListener(kind, fn) { const list = windowListeners.get(kind) ?? []; list.push(fn); windowListeners.set(kind, list); },
    removeEventListener(kind, fn) { windowListeners.set(kind, (windowListeners.get(kind) ?? []).filter(g => g !== fn)); },
  });
  inputScript.runInContext(f);
  f.attach(el, 7, ['pan']);
  function event(extra = {}) {
    return { isPrimary: true, button: 0, pointerId: 1, clientX: 0, clientY: 0,
      target: { closest: () => null }, composedPath() { return [this.target]; }, prevented: false, stopped: false,
      preventDefault() { this.prevented = true; }, stopPropagation() { this.stopped = true; },
      stopImmediatePropagation() { this.stopped = true; }, ...extra };
  }
  return { f, el, buttons, frames, sent, captured, windowListeners,
    // An event at this node reaches the window's capture listeners first; one outside it, only them.
    outside(kind, extra) { const e = event(extra); for (const fn of [...windowListeners.get(kind) ?? []]) fn(e); return e; },
    load() {
      vm.runInContext(`inputHandlers = createInputHandlers({ root, views, retiredViews,
        ready: () => inputReady, inertAncestor,
        dispatch: (id, payload) => send(wasm.exact_dispatch(id, 20, writeIn(payload), now())),
      }); inputReady = true;`, f);
    },
    pointer(kind, extra) { const e = event(extra); for (const fn of [...windowListeners.get(kind) ?? [], ...listeners.get(kind) ?? []]) fn(e); return e; },
    key(extra) { const e = event({ key: 'k', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, ...extra }); f.keydown(e); return e; },
    tick() { const pending = [...frames.values()]; frames.clear(); pending.forEach(fn => fn()); },
  };
}

test('post-paint pan keeps one pending frame through 10000 moves and flushes the release', () => {
  const h = inputFixture();
  expect(h.pointer('pointerdown').prevented).toBe(false); // Input is gated while the module is absent.
  h.load();
  expect(h.pointer('pointerdown').prevented).toBe(true);
  expect(h.captured).toEqual([1]);
  h.pointer('pointermove', { clientX: 4 }); h.tick();
  expect(h.sent).toEqual([]); // Preserve the platform slop before activating.
  for (let x = 5; x <= 10004; x++) h.pointer('pointermove', { clientX: x });
  expect(h.frames.size).toBe(1);
  expect(h.sent).toEqual([]);
  h.tick();
  expect(h.sent).toEqual([[7, 20, '10004,0', 0]]); // Negative control: an empty handler fails.
  h.pointer('pointermove', { clientX: 10005, clientY: 2 });
  h.pointer('pointerup', { clientX: 10006, clientY: 3 });
  expect(h.sent.at(-1)).toEqual([7, 20, '2,3', 0]);
  expect(h.frames.size).toBe(0);
  h.pointer('pointerdown'); h.pointer('pointerup', { clientX: 8 });
  expect(h.sent.at(-1)).toEqual([7, 20, '8,0', 0]); // Reuse does not install duplicate listeners.
  expect(h.sent.length).toBe(3);
});

test('pan rejects foreign and editable contacts and drops cancelled or stale queued work', () => {
  for (const extra of [{ isPrimary: false }, { button: 1 }, { target: { closest: () => ({}) } }]) {
    const h = inputFixture(); h.load();
    expect(h.pointer('pointerdown', extra).prevented).toBe(false);
    h.pointer('pointermove', { clientX: 20 }); h.tick();
    expect(h.sent).toEqual([]);
  }
  for (const cancel of [h => h.pointer('pointercancel'), h => h.pointer('lostpointercapture', { target: h.el }),
    h => h.f.views.set(7, {}), h => h.f.retiredViews.add(h.el),
    h => { h.el.inert = true; }, h => { h.el.disabled = true; }, h => { h.f.inputReady = false; }]) {
    const h = inputFixture(); h.load(); h.pointer('pointerdown');
    h.pointer('pointermove', { pointerId: 2, clientX: 100 });
    expect(h.frames.size).toBe(0);
    h.pointer('pointermove', { clientX: 20 }); cancel(h); h.tick();
    expect(h.frames.size).toBe(0);
    expect(h.sent).toEqual([]);
  }
});

test('a pan under a nested button hears its contact on the window until it begins, and a release anywhere ends it (kanban F6)', () => {
  const nested = { target: { closest: s => s.startsWith('button') ? {} : null }, pointerType: 'mouse', buttons: 1 };
  const count = h => ['pointermove', 'pointerup', 'pointercancel'].map(k => h.windowListeners.get(k)?.length ?? 0), fixture = () => { const h = inputFixture(); h.load(); h.base = count(h); return h; }, watching = h => count(h).map((n, i) => n - h.base[i]); // the contact's own, beyond those kept from load (a press's dim fade, bf527d835f)
  // Released outside the node inside the slop: the contact ends, and a later
  // buttonless move back over the node pans nothing (it did, stale, before).
  let h = fixture();
  const down = h.pointer('pointerdown', nested);
  expect([down.prevented, down.stopped, h.captured]).toEqual([false, false, []]); // a tap stays the button's
  expect(watching(h)).toEqual([1, 1, 1]);
  h.outside('pointerup', { ...nested, clientX: 2, buttons: 0 });
  expect(watching(h)).toEqual([0, 0, 0]);
  h.pointer('pointermove', { ...nested, clientX: 30, buttons: 0 }); h.tick();
  expect(h.sent).toEqual([]);
  // Dragged out past the slop and released there: the pan begins and ends once.
  h = fixture(); h.pointer('pointerdown', nested);
  h.outside('pointermove', { ...nested, clientX: 60 });
  h.outside('pointerup', { ...nested, clientX: 60, buttons: 0 });
  expect(h.sent).toEqual([[7, 20, '60,0', 0]]);
  h.pointer('pointermove', { ...nested, clientX: 90, buttons: 0 }); h.tick();
  expect(h.sent.length).toBe(1);
  // An up no one heard (outside the window): the next move without the button ends it.
  h = fixture(); h.pointer('pointerdown', nested);
  h.pointer('pointermove', { ...nested, clientX: 30, buttons: 0 }); h.tick();
  expect([h.sent, watching(h)]).toEqual([[], [0, 0, 0]]);
  // Dragged inside: once the pan begins it captures, the window stops
  // watching, and each later event is handled once.
  h = fixture(); h.pointer('pointerdown', nested);
  h.pointer('pointermove', { ...nested, clientX: 10 }); h.tick();
  expect([h.sent, h.captured, watching(h)]).toEqual([[[7, 20, '10,0', 0]], [1], [0, 0, 0]]);
  h.pointer('pointermove', { ...nested, clientX: 15 }); h.tick();
  h.pointer('pointerup', { ...nested, clientX: 15, buttons: 0 });
  expect(h.sent).toEqual([[7, 20, '10,0', 0], [7, 20, '5,0', 0]]);
});

test('moved keyboard shortcuts preserve modifiers, readiness, repeat and modal gating', () => {
  const h = inputFixture(); h.load(); let clicks = 0;
  const button = { isConnected: true, disabled: false, inert: false,
    getClientRects: () => [{}], getAttribute: () => 'Meta+k Escape', click() { clicks++; } };
  h.buttons.push(button);
  expect(h.key().prevented).toBe(true);
  expect(clicks).toBe(1); // Negative control for the moved document listener.
  expect(h.key({ repeat: true }).prevented).toBe(true);
  expect(clicks).toBe(1);
  for (const extra of [{ isComposing: true }, { defaultPrevented: true }, { shiftKey: true }, { metaKey: false }]) {
    expect(h.key(extra).prevented).toBe(false);
  }
  h.f.inputReady = false; expect(h.key().prevented).toBe(false); h.f.inputReady = true;
  h.f.document.activeElement.closest = () => ({ contains: () => false });
  expect(h.key().prevented).toBe(false);
  h.f.document.activeElement.closest = () => null;
  button.inert = true; expect(h.key().prevented).toBe(false); button.inert = false;
  button.disabled = true; expect(h.key().prevented).toBe(true); button.disabled = false;
  expect(clicks).toBe(1);
  expect(h.key({ key: 'Escape', metaKey: false }).prevented).toBe(true);
  expect(clicks).toBe(2);
});

// chat F14: a pressable that is no button takes Enter, or Space unless it is
// a link, as a click, after the key's handlers and unless one prevented it.
test('a pressable that is no button activates by Enter or Space', () => {
  const h = inputFixture(); h.load(); let clicks = 0;
  const pressable = (tag, role = null) => ({ dataset: { exactOn: 'press' }, matches: s => s.split(', ').includes(tag), getAttribute: () => role, closest: () => null /* no disabled ancestor (LLP 1121 D8) */, click() { clicks++; }, toggleAttribute() {}, hasAttribute: () => false, style: { getPropertyValue: () => '' }, addEventListener() {}, removeEventListener() {} }); // Space also shows the press (tests/press.test.mjs)
  const key = (target, extra) => { const e = { key: 'Enter', target, preventDefault() { this.prevented = true; }, ...extra }; h.f.keyActivate(e); return e; };
  expect(key(pressable('div')).prevented).toBe(true);
  expect(key(pressable('div'), { key: ' ' }).prevented).toBe(true);
  expect(clicks).toBe(2);
  for (const [target, extra] of [[pressable('div'), { defaultPrevented: true }], [pressable('div'), { repeat: true }], [pressable('div'), { metaKey: true }],
    [pressable('div', 'link'), { key: ' ' }], [pressable('button'), {}], [pressable('div'), { key: 'a' }]]) expect(key(target, extra).prevented).toBeUndefined();
  expect(clicks).toBe(2);
  expect(key(pressable('div', 'link')).prevented).toBe(true);
  expect(clicks).toBe(3);
});

// @ref LLP 1043.000 §3 D7/D8 — optional host code cannot gate data readiness.
const checkpoint = () => new Promise(resolve => setImmediate(resolve));
async function startupFixture(rustOnly = false, served = null, timerFirst = false) {
  const data = deferred(), input = deferred(), timer = deferred();
  const frames = [], timeouts = [], loads = [], errors = [], events = [];
  const exports = { memory: {}, exact_compat: () => '{"inputs":{}}',
    exact_logic: () => '{"appId":"test.startup"}',
    exact_data_ready: () => { events.push('activate'); return '{"ops":[]}'; },
    ...(rustOnly ? {} : { exact_module_artifact() {} }) };
  const f = vm.createContext({ frames, loads, errors, events,
    root: { dataset: {}, setAttribute(key, value) { this[key] = value; } },
    views: new Map(), retiredViews: new WeakSet(), authoredDisabled: new WeakMap(),
    inputReady: false, inputHandlers: null, wasm: null, memory: null, logicInfo: null,
    moduleLoader: null, activeModule: null, timerFactory: null, agentMode: false, agentKeepsStore: true, // glue.js: !agentMode || ?storage
    performance: { now: () => 1 }, t0: 0, URL, localStorage: { length: 0 }, AbortController,
    document: { querySelectorAll: () => [] }, // no preload: the glue fetches ./app.wasm
    fetch: async () => ({}), WebAssembly: { instantiateStreaming: async () => ({ instance: { exports } }), Module: { customSections: () => [], imports: () => [] } },
    moduleCall() {}, rustImports: {}, dataImports: {}, grantOrigins, readOut: value => value,
    boot: async () => events.push('boot'), loadGpuIfNeeded() {}, startClock() {}, httpHelpers() {}, pieces: { pending: () => null },
    requestAnimationFrame: fn => frames.push(fn), setTimeout: (fn, ms) => timeouts.push([fn, ms]), console: { error: error => errors.push(String(error)) },
    motion: { commit() {} }, collections: { dataReady: () => events.push('collections') },
    applyBatch: () => events.push('batch'), inertAncestor: () => false, editMenu: () => false, focusAutofocus() {},
    resolveModuleReady: () => events.push('ready'), page: served && { holding: true, early: [] }, pageNative: undefined, log() {}, // the input piece's journal (a cancelled pan's line)
    loadAfterPaint(file) {
      loads.push(file);
      if (file === './input-glue.js') return input.promise;
      if (file === './timer-glue.js') return timer.promise;
      if (file === './module-glue.js') return Promise.resolve({ baked: () => data.promise, prepare: async () => ({ id: 0 }) });
      if (file === './frames.js') return new Promise(() => {}); // a development page's frame sampler (LLP 1079 D3) is not what these tests start
      throw new Error('unexpected startup module: ' + file);
    },
  });
  f.exact = served ? { documentPage: { connect: () => ({ started: Promise.resolve(), ...served }) } } : {}; // a served document's capture (document-glue.js)
  // navigation.js's export, which a context global would confuse with the browser's own `globalThis.navigation`.
  vm.runInContext([source.match(/^const AGENT_ADMITTED = .*$/m)[0], "const navigation = { activate: () => events.push('navigation') };", ...['setInputReady', 'activateData', 'main'].map(declaration)].join('\n')
    .replaceAll('import.meta.url', '"https://fixture.invalid/glue.js"'), f);
  await f.main();
  if (served) return { f }; // a served document's first pixel is painted: activation is not a frame away
  expect(events).toEqual(['boot', 'navigation']); // Adoption activates navigation before optional modules or data.
  expect(loads).toEqual([]); // Neither optional nor app modules run before paint.
  const early = timerFirst && timeouts[0][0](); // a frameless page's second (glue.js activateOnce) first: its frames, back later, start nothing more
  frames.shift()(); if (!early) expect(loads).toEqual([]);
  const activation = frames.shift()(); expect(timeouts.map(([, ms]) => ms)).toEqual([1000]); expect(timeouts[0][0]()).toBe(activation); if (early) expect(activation).toBe(early); // one activation, whichever comes first
  return { f, data, input, timer, activation };
}

test('pending, failed and invalid optional modules leave Rust and JS apps ready after paint', async () => {
  for (const rustOnly of [false, true]) for (const failure of ['pending', 'reject', 'invalid']) for (const timerFirst of [false, true]) {
    const h = await startupFixture(rustOnly, null, timerFirst), { f } = h;
    if (!rustOnly) expect(f.inputReady).toBe(false);
    h.data.resolve({});
    await checkpoint();
    expect(f.root.dataset.moduleReady).toBe('true');
    expect(f.inputReady).toBe(true);
    expect(f.root['aria-busy']).toBe('false');
    expect(f.events).toEqual(['boot', 'navigation', 'activate', 'batch', 'collections', 'ready']);
    expect(f.inputHandlers).toBeNull(); // A stalled import has no readiness deadline.
    expect(f.loads.filter(file => file === './input-glue.js')).toHaveLength(1);
    h.timer.reject(new Error('timer unavailable'));
    if (failure === 'reject') h.input.reject(new Error('input unavailable'));
    else if (failure === 'invalid') h.input.resolve(undefined);
    else h.input.resolve(() => ({ pan: 'installed after readiness', arrive() {} }));
    await h.activation;
    await checkpoint();
    expect(f.root.dataset.error).toBeUndefined();
    expect(f.root.dataset.moduleReady).toBe('true');
    expect(f.events.filter(event => event === 'activate')).toHaveLength(1);
    expect(f.errors).toHaveLength(failure === 'pending' ? 1 : 2);
    if (failure === 'pending') expect(f.inputHandlers.pan).toBe('installed after readiness');
  }
});

// @ref LLP 1115.002 D4 — a served document is projected only once the target has adopted it.
test('a served document activates navigation when release has adopted it, not at boot', async () => {
  const released = [];
  const { f } = await startupFixture(true, { hold: () => true, release: (...args) => released.push(args) });
  await checkpoint();
  expect(f.events).toEqual(['boot', 'activate', 'batch', 'collections', 'ready']); // a composition defers the adoption
  expect(released).toHaveLength(1);
  released[0][2]();
  expect(f.events.at(-1)).toBe('navigation');
});

test('required app module failure still keeps dispatch gated and settles the error', async () => {
  const h = await startupFixture();
  h.data.reject(new Error('required app unavailable'));
  await h.activation;
  expect(h.f.inputReady).toBe(false);
  expect(h.f.root.dataset.moduleReady).toBeUndefined();
  expect(h.f.root.dataset.error).toContain('required app unavailable');
  expect(h.f.events).toEqual(['boot', 'navigation', 'ready']);
  h.input.resolve(() => ({ arrive() {} })); h.timer.resolve(() => ({}));
  await checkpoint();
});

test('ready pan nodes tolerate a missing module and install exactly once when it arrives', () => {
  const h = inputFixture();
  h.f.inputReady = true;
  for (let i = 0; i < 1000; i++) expect(h.pointer('pointerdown').prevented).toBe(false);
  expect(h.frames.size).toBe(0);
  expect(h.sent).toEqual([]);
  h.load();
  h.pointer('pointerdown'); h.pointer('pointerup', { clientX: 10 });
  expect(h.sent).toEqual([[7, 20, '10,0', 0]]);
});

test('initial flow loading cannot hold the first frame or module activation', async () => {
  const f = fixture(false), loading = deferred();
  const apply = f.applyBatch;
  f.loadAfterPaint = () => loading.promise;
  f.log = line => f.logs.lines.push(line);
  vm.runInContext(declaration('flowBatch'), f);
  f.applyBatch = batch => {
    const result = apply(batch);
    f.flowBatch({ ops: [{ op: 'textflow', contexts: [{ root: 1 }] }] });
    return result;
  };
  let painted = false;
  const boot = f.boot(null).then(() => { painted = true; });
  await checkpoint();
  expect(painted).toBe(true);
  expect(f.events).toEqual(['replace', 'batch']);
  loading.reject(new Error('flow unavailable'));
  await checkpoint();
  expect(f.logs.lines.at(-1)).toContain('textflow module: Error: flow unavailable');
  await boot;
});

for (const letGo of [false, true]) test(`module replacement reaches the runner without waiting on a parked ticket (letGo=${letGo})`, async () => {
  const f = fixture(), elapsed = requestClock(f), calls = [];
  f.requestsPending = 1;
  f.setInputReady = value => { f.inputReady = value; };
  f.wasm.exact_boot_module = (...args) => {
    calls.push(args[3]);
    return letGo ? '{"ops":[]}' : '{"error":"runner still has a pending request"}';
  };
  const candidate = { realm: { id: 1 }, receipt: new Uint8Array([1]), letGo };
  const result = f.boot(new Uint8Array([1]), null, () => true, candidate);
  if (letGo) { await result; expect(f.activeModule).toBe(candidate); }
  else await expect(result).rejects.toThrow('runner still has a pending request');
  expect(calls).toEqual([Number(letGo)]);
  expect(elapsed()).toBe(0);
});

test('module replacement waits for requests, rechecks supersession; the dev loop swaps after its settle window, an update refuses', async () => {
  for (const [result, letGo] of [['accept', true], ['superseded', true], ['pending', true], ['accept', false], ['pending', false]]) {
    const f = fixture(false), entered = deferred(), drained = deferred(), lines = [];
    let current = true, swaps = 0, flag = null;
    f.waitForInflight = async deadline => {
      expect(deadline).toBe(letGo ? 1010 : 20010);
      entered.resolve();
      return drained.promise;
    };
    f.log = line => lines.push(line);
    f.setInputReady = value => { f.inputReady = value; };
    f.wasm.exact_boot_module = (...args) => { swaps++; flag = args[3]; return '{"ops":[],"timers":true}'; };
    // An app that always has a request out, here a read and a write; the write is named when let go.
    if (result === 'pending') f.inflight.add(new Promise(() => {})).add(Object.assign(new Promise(() => {}), { write: 'POST https://api.test/like' }));
    const candidate = {realm:{id:1},receipt:new Uint8Array([1]),letGo};
    const update = f.boot(new Uint8Array([1]), null, () => current, candidate);
    const outcome = update.then(value => ({value}), error => ({error}));
    await entered.promise;
    expect(swaps).toBe(0);
    expect(f.views.size).toBe(1);
    expect(f.inputReady).toBe(true);
    if (result === 'superseded') current = false;
    drained.resolve(result !== 'pending');
    const reply = await outcome;
    if (result === 'superseded' || result === 'pending' && !letGo) {
      expect(swaps).toBe(0);
      expect(f.views.size).toBe(1);
      expect(f.activeModule).toBeNull();
      if (letGo) expect(reply.value).toBeNull();
      else expect(String(reply.error)).toContain('in-flight requests');
    } else {
      expect(reply.error).toBeUndefined();
      expect(swaps).toBe(1);
      expect(flag).toBe(letGo ? 1 : 0);
      expect(f.activeModule).toBe(candidate);
      expect(f.inflight.size).toBe(0);
      expect(lines.filter(l => l.startsWith('module replacement'))).toEqual(result === 'pending' ? ['module replacement let go of 2 requests still in flight after 1000 ms; aborted writes the server may or may not have seen: POST https://api.test/like'] : []);
    }
  }
});


test('scoped tree rendering trims only shared indentation and preserves guest depth', () => {
  const node = {id:7,parent:3,depth:4,type:'WebView',props:{testId:'panel'},handlers:[],children:[8],guest:[{depth:0,tag:'button',text:'guest'}]};
  const child = {id:8,parent:7,depth:5,type:'Text',props:{text:'child'},handlers:[],children:[]};
  const lines = render('tree',{epoch:1,incarnation:2,clock:0,roots:[7],nodes:[node,child]}).split('\n');
  expect(lines.slice(1)).toEqual(['WebView#7 [panel]','  [guest] button "guest"','  Text#8 "child"']);
  expect(node.depth).toBe(4);
});


test('tree forwards its target and preserves host annotations and runner errors', () => {
  const f = fixture(), requests = [];
  vm.runInContext(declaration('tree'), f);
  const iframe = new f.HTMLIFrameElement();
  iframe.getAttribute = name => name === 'src' ? '/guest' : null; iframe.matches = () => false;
  f.views.set(7, iframe); f.iframeLoading = new Map([[iframe,false]]);
  f.guestOutline = () => [{tag:'button',depth:0,text:'guest'}];
  f.ask = request => {
    requests.push(request);
    return request.target === 'missing' ? {error:'no view matches missing'} : {roots:[7],nodes:[{id:7,type:'WebView'}]};
  };
  const reply = f.exact.agent({op:'tree',target:'panel',shallow:true});
  expect(requests[0]).toEqual({op:'tree',target:'panel',shallow:true});
  expect(reply.nodes[0]).toEqual({id:7,type:'WebView',focused:false,url:'/guest',loading:false,guest:[{tag:'button',depth:0,text:'guest'}]});
  expect(f.exact.agent({op:'tree',target:'missing'})).toEqual({error:'no view matches missing'});
});

// Uncontrolled fields live in the presenter; inspection must read what the
// person typed without writing it into Contract state.
test('wasm tree and node detail report live text field values without mutating runner props', () => {
  const f = fixture();
  vm.runInContext(declaration('tree'), f);
  for (const C of [f.HTMLInputElement, f.HTMLTextAreaElement]) {
    const field = Object.assign(new C(), f.views.get(1), { localName: C === f.HTMLInputElement ? 'input' : 'textarea', value: 'typed text' });
    f.views.set(1, field);
    const props = { testId: 'field', value: 'initial' };
    f.ask = req => req.op === 'tree'
      ? { nodes: [{ id: 1, type: 'TextInput', props: { ...props } }] }
      : { id: 1, type: 'TextInput', props: { ...props } };
    expect(f.exact.agent({ op: 'tree' }).nodes[0].props.value).toBe('typed text');
    expect(f.exact.agent({ op: 'layout', id: 1 }).node.props.value).toBe('typed text');
    expect(props.value).toBe('initial');
    field.value = '';
    expect(f.exact.agent({ op: 'tree' }).nodes[0].props.value).toBe('');
  }
});

function listFixture() {
  const frames = [], listeners = new Map();
  const document = { activeElement: null,
    addEventListener: (name, fn) => listeners.set(name, fn),
    createElement: () => ({}), head: { append() {} } };
  const lists = new Map();
  const root = { addEventListener() {} };
  const context = vm.createContext({ document, root, lists, globalThis: { exact: {} },
    getSelection: () => ({ isCollapsed: true, anchorNode: null, focusNode: null, anchorOffset: 0, focusOffset: 0,
      toString() { throw new Error('rendered selection text forces layout'); } }),
    requestAnimationFrame: fn => { frames.push(fn); return frames.length; },
  });
  vm.runInContext(readFileSync(new URL('./list-selection.js', import.meta.url), 'utf8'), context);
  const controller = context.globalThis.exact.installListSelection({ root, lists, index() {}, text() {} });
  return { document, controller, lists, frames, listeners,
    frame() { const queued = frames.splice(0); for (const fn of queued) fn(); },
  };
}

test('a virtualized list takes logical selection and leaves its geometry to navigation.js', () => {
  const f = listFixture(), el = { isConnected: true, firstElementChild: { children: [] }, addEventListener() { throw new Error('navigation.js owns its events'); } };
  f.lists.set(el, { id: 3 });
  f.controller.sync(); f.controller.before(); f.controller.after(); f.frame();
  expect(f.listeners.has('copy')).toBe(true);
});

test("a drive's storage is only a scratch store it names, apart from the app's own", async () => {
  expect(storageKey('com.example.app', 'http://127.0.0.1:1/')).toBe('com.example.app');
  expect(storageKey('com.example.app', 'http://127.0.0.1:1/?agent=1')).toBeNull();
  expect(storageKey('com.example.app', 'http://127.0.0.1:1/?agent=1&storage=run-2.a')).toBe('com.example.app/agent/run-2.a');
  expect(storageKey('com.example.app', 'http://127.0.0.1:1/?storage=x')).toBe('com.example.app'); // only a drive's is scratch
  for (const name of ['', '.', '..', 'a/b', '%2e%2e']) expect(() => storageKey('com.example.app', `http://127.0.0.1:1/?agent=1&storage=${name}`)).toThrow('storage: one name');
  for (const host of ['web', 'linux']) await expect(open({ host, storage: '../x' })).rejects.toThrow('--storage: one name');
  // The page's launch URL names the store, not where the app's router has
  // since moved `location` (recipes F9: a pick after a navigation went to
  // another store than the data module's).
  const entries = performance.getEntriesByType;
  performance.getEntriesByType = type => type === 'navigation' ? [{ name: 'http://127.0.0.1:1/?agent=1&storage=s1' }] : [];
  try { expect(storageKey('com.example.app')).toBe('com.example.app/agent/s1'); } finally { performance.getEntriesByType = entries; }
});

// A held request is addressed by node or capability, independently of its ticket.
test('a hold is addressed by its node or capability, and an unclear name is refused', () => {
  const pending = [{ name: 'notes', ticket: 4 }, { name: 'folder-input', ticket: 7, device: { capability: 'open-directory', args: { id: 'folder-input' } } },
    { name: 'share', ticket: 9, device: { capability: 'share', args: {} } }, { name: 'export-input', ticket: 11, device: { capability: 'export', args: { id: 'export-input' } } }];
  expect([holdOf('@7'), holdOf('@folder-input'), holdOf('folder-input'), holdOf('@')]).toEqual([true, true, false, false]);
  expect([heldTicket(pending, '@folder-input'), heldTicket(pending, '@open-directory'), heldTicket(pending, '@share'), heldTicket(pending, '@export')]).toEqual([7, 7, 9, 11]);
  expect(() => heldTicket(pending, '@notes')).toThrow(/no held device request .*held: @7 open-directory at "folder-input"/);
  expect(() => heldTicket([...pending, { name: 'other', ticket: 12, device: { capability: 'open-directory', args: { id: 'other' } } }], '@open-directory')).toThrow(/2 holds match/);
});

test('a pick answer keeps a path with a space when it comes one per line', () => {
  const paths = new Function(readFileSync(new URL('../web-js/files.js', import.meta.url), 'utf8').match(/^const paths = .*;$/m)[0] + ';return paths;')();
  for (const input of ['/tmp/my photo.png\n', '/tmp/a b.png\n/tmp/c.png\n', 'a.png b.png', '']) expect(paths(input)).toEqual(pickedPaths(input));
  expect(pickedPaths('/tmp/my photo.png\n')).toEqual(['/tmp/my photo.png']);
  expect(pickedPaths('/tmp/a b.png\n/tmp/c.png\n')).toEqual(['/tmp/a b.png', '/tmp/c.png']);
  expect(pickedPaths('a.png b.png')).toEqual(['a.png', 'b.png']);
});

test('a mouse drag presses and releases the left button on the web', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-mouse-drag-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
  writeFileSync(contract, `component MouseDrag
  state log = ""
  action at(kind: string, e: PointerEvent)
    log = \`\${log}\${kind}:\${e.pointerType};\`
  view
    column testId="root"
      column testId="pad" width=300 height=100 touch-action="none" pointerdown=at("down") pointerup=at("up") background-color="#dddddd"
      text log testId="log"
`);
  const compile = spawnSync(compiler, ['build', contract, '-o', plan], { encoding:'utf8' });
  expect(compile.status, compile.stderr).toBe(0);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd:new URL('../../', import.meta.url).pathname, encoding:'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let s;
  try {
    s = await open({ host:'web', url:`http://127.0.0.1:${server.address().port}/` });
    await expect(s.tap('pad', { down: true, mouse: true })).rejects.toThrow('mouse cannot be combined');
    const r = await s.tap('pad', { drag: { dx: 40, dy: 20, from: [12, 8], mouse: true, over: 64 } });
    expect(r.drag.mouse).toBe(true);
    expect(s.contact).toBeNull();
    await s.clock('+16');
    expect((await s.state()).slots.log).toBe('down:mouse;up:mouse;');
  } finally {
    await s?.close();
    server.close();
    rmSync(dir, { recursive: true, force: true });
  }
}, 120000);

test('a native mouse contact holds the button until it lifts or its down fails', async () => {
  const { mouseContact } = await import('../../scripts/agent.mjs');
  const sent = [], c = mouseContact(), send = (reply) => (button) => { sent.push(button.mouse === true); if (reply instanceof Error) throw reply; return reply; };
  await c.ask('down', { mouse: true }, send({}));
  await c.ask('move', {}, send({}));
  await c.ask('up', {}, send({}));
  await c.ask('down', {}, send({}));
  expect(sent).toEqual([true, true, true, false]);
  await expect(c.ask('down', { mouse: true }, send(new Error('the app stopped answering')))).rejects.toThrow('stopped answering');
  expect(c.held).toBe(false);
  await c.ask('down', { mouse: true }, send({ error: 'no input under it' }));
  expect(c.held).toBe(false);
});

test('perf frames live lends the clock to the wall for its window and measures what it presented (LLP 1079 D4; platformer R11)', async () => {
  const saved = Object.fromEntries(['document', 'addEventListener', 'requestAnimationFrame', 'cancelAnimationFrame', 'performance'].map(k => [k, Object.getOwnPropertyDescriptor(globalThis, k)]));
  let wall = 1000, next = 0; const queued = new Map();
  const set = (k, value) => Object.defineProperty(globalThis, k, { value, configurable: true, writable: true });
  set('document', { visibilityState: 'visible', addEventListener() {}, getAnimations: () => [] });
  set('addEventListener', () => {});
  set('requestAnimationFrame', fn => (queued.set(++next, fn), next));
  set('cancelAnimationFrame', id => queued.delete(id));
  set('performance', { now: () => wall });
  try {
    await import('./frames.js');
    let clock = 500;
    const advanced = [], gpu = [];
    const window = globalThis.exact.liveFrames({ ms: 200, origin: () => 0, log() {}, clock: () => clock,
      advance: to => { advanced.push(to); clock = to; },
      gpu: { live: on => (gpu.push(on), on ? [] : [{ canvas: 7, perf: { frameMs: { p50: 16.7 } } }]) } });
    let done = false; window.then(() => { done = true; });
    for (let i = 0; i < 40 && !done; i++) {
      wall += 1000 / 60;
      const due = [...queued.values()]; queued.clear();
      for (const fn of due) fn(wall);
      await new Promise(resolve => setImmediate(resolve));
    }
    const reply = await window;
    expect(gpu).toEqual([true, false]); // the world left the seek for its own frames, and came back
    expect(reply.live).toEqual({ ms: 200, from: 500, to: 700 });
    expect(advanced.at(-1)).toBe(700); // the runner followed the wall to the window's end, and no further
    expect(reply.window.samples).toBeGreaterThan(8);
    expect(reply.window.p50).toBeCloseTo(16.67, 1);
    expect(reply.world).toEqual([{ canvas: 7, perf: { frameMs: { p50: 16.7 } } }]);
    expect(globalThis.exact.frames).toBeUndefined(); // the window's sampler does not outlive it
  } finally {
    for (const [k, d] of Object.entries(saved)) d ? Object.defineProperty(globalThis, k, d) : delete globalThis[k];
  }
});
// A target no testId carries, by the label or text a person reads (Exact-new iOS feedback, 2026-10-04).
const N = (id, depth, type, props = {}, handlers = [], inactive = false) => ({ id, depth, type, props, handlers, inactive });
test('a scroller with a scroll handler does not steal a button name', () => {
  const nodes = [N(1, 0, 'View', {}, ['scroll']), N(2, 1, 'Pressable', { accessibilityLabel: 'Save' }, ['press']), N(3, 2, 'Text', { text: 'Save' })];
  expect(nodeNamed(nodes, 'Save').id).toBe(2);
  const t = [N(1, 0, 'View', {}, ['scroll']), N(2, 1, 'Pressable', {}, ['press']), N(3, 2, 'Text', { text: 'Save' })];
  expect(nodeNamed(t, 'Save').id).toBe(2);
});
test('an active screen heading beats a covered button', () => {
  const nodes = [N(1, 0, 'View'), N(2, 1, 'Pressable', {}, ['press'], true), N(3, 2, 'Text', { text: 'Settings' }, [], true), N(4, 1, 'Text', { text: 'Settings' })];
  expect(nodeNamed(nodes, 'Settings').id).toBe(4);
});
test('ambiguity refuses, fast on a flat list', () => {
  const nodes = [N(0, 0, 'View')];
  for (let i = 1; i <= 5000; i++) nodes.push(N(i, 1, 'Pressable', { accessibilityLabel: 'Delete' }, ['press']));
  const t0 = performance.now();
  expect(() => nodeNamed(nodes, 'Delete')).toThrow(/names 5000 views/);
  expect(performance.now() - t0).toBeLessThan(200);
});
test('a press beats an ancestor taking only focus, a pan or a context menu', () => {
  for (const h of ['focus', 'pan', 'contextmenu']) {
    const nodes = [N(1, 0, 'View', {}, [h]), N(2, 1, 'Pressable', {}, ['press']), N(3, 2, 'Text', { text: 'Save' })];
    expect(nodeNamed(nodes, 'Save').id).toBe(2);
  }
});
test('active text beats a covered label, and a covered descendant names no active ancestor', () => {
  const covered = [N(1, 0, 'View'), N(2, 1, 'Pressable', { accessibilityLabel: 'Settings' }, ['press'], true), N(4, 1, 'Text', { text: 'Settings' })];
  expect(nodeNamed(covered, 'Settings').id).toBe(4);
  const only = [N(1, 0, 'View'), N(2, 1, 'Pressable', {}, ['press'], true), N(3, 2, 'Text', { text: 'Save' }, [], true)];
  expect(nodeNamed(only, 'Save').id).toBe(2);
});
test('innermost non-interactive text; none is null', () => {
  const nodes = [N(1, 0, 'View'), N(2, 1, 'View'), N(3, 2, 'Text', { text: 'Hi' })];
  expect(nodeNamed(nodes, 'Hi').id).toBe(3);
  expect(nodeNamed(nodes, 'Nope')).toBe(null);
});
test('iOS drives serialize the same device and bundle and release on launch or close failure', async () => {
  const { exclusiveIOS } = await import('../../scripts/agent-launch.mjs');
  const directory = mkdtempSync(join(tmpdir(), 'exact-drive-lock-')), events = [];
  const options = { directory, timeout: 1000 };
  const launch = label => async () => { events.push(label); return { close: async () => events.push('close ' + label) }; };
  let first, second, other;
  try {
    first = await exclusiveIOS('sim', 'app', launch('first'), options);
    const waiting = exclusiveIOS('sim', 'app', launch('second'), options);
    other = await exclusiveIOS('sim', 'other', launch('other'), options);
    expect(events).toEqual(['first', 'other']);
    await first.close(); second = await waiting;
    expect(events).toEqual(['first', 'other', 'close first', 'second']);
    await expect(exclusiveIOS('sim', 'app', launch('blocked'), {...options, timeout:0})).rejects.toThrow('iOS drive busy');
    await second.close(); await other.close();
    await expect(exclusiveIOS('sim', 'app', async () => { throw Error('launch failed'); }, options)).rejects.toThrow('launch failed');
    first = await exclusiveIOS('sim', 'app', async () => ({ close: async () => { throw Error('close failed'); } }), options);
    await expect(first.close()).rejects.toThrow('close failed'); first = null;
    second = await exclusiveIOS('sim', 'app', launch('released'), options); await second.close();
    // Kill the recorded driver PID while it holds the lock: EOF must release
    // the helper's OS lock without a stale-file cleanup or a wall-clock lease.
    const script = join(directory, 'holder.mjs');
    writeFileSync(script, `import {exclusiveIOS} from ${JSON.stringify(new URL('../../scripts/agent-launch.mjs', import.meta.url).href)};
await exclusiveIOS('sim','app',async()=>({close:async()=>{}}),${JSON.stringify(options)});console.log('held');`);
    const child = spawn(process.execPath, [script], {stdio:['ignore','pipe','pipe']});
    const exited = new Promise(resolve => child.once('exit', resolve));
    try {
      await new Promise((resolve,reject)=>{child.stdout.once('data',resolve);child.once('error',reject);child.once('exit',()=>reject(Error('holder exited before locking')));});
      child.kill('SIGKILL'); await exited;
      second = await exclusiveIOS('sim', 'app', launch('after death'), options); await second.close();
    } finally { child.kill('SIGKILL'); await exited; }
  } finally { await first?.close(); await second?.close(); await other?.close(); rmSync(directory,{recursive:true,force:true}); }
}, 60000);
test('tap parses every mouse form, refuses a word it does not use, and a carrier that cannot deliver one says unsupported (#107)', async () => {
  const { tapWords: t, pointerGap: gap } = await import('../../scripts/agent.mjs'); expect(t(['p', 'auxclick', 'at', '30', '40', 'modifiers', 'Meta'])).toEqual(['p', { auxclick: true, at: [30, 40], modifiers: 'Meta' }]); expect(t(['p', 'clicks', '3'])).toEqual(['p', { clicks: 3 }]); expect(t(['p', 'wheel', '0', '20', 'at', '10', '10'])).toEqual(['p', { wheel: [0, 20], at: [10, 10] }]); expect(t(['p', 'down', 'at', '10', '20', 'modifiers', 'Shift'])).toEqual(['p', { down: true, at: [10, 20], modifiers: 'Shift' }]); expect(t(['up', 'modifiers', 'Shift'], true)).toEqual([null, { phase: 'up', modifiers: 'Shift' }]); for (const words of [['p', 'mouse', 'foo'], ['p', 'auxclick', 'at', '1'], ['p', 'hover', 'x'], ['p', 'clicks'], ['p', 'frob']]) expect(() => t(words)).toThrow(/unknown word|takes a number/); expect(gap('macos', undefined, { auxclick: true })).toBeNull(); expect(gap('web', 'chrome', { clicks: 3, modifiers: 'Shift', down: true })).toBeNull(); for (const host of ['linux', 'windows', 'ios']) expect(gap(host, undefined, { wheel: [0, 1], at: [1, 1] })).toContain('a wheel at a point'); expect(gap('web', 'firefox', { drag: { modifiers: 'Shift' } })).toContain('modifiers'); });
