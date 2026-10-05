// @ref LLP 1043.000 §3 D7/D8 — flow settlement must not change LLP 1012's API.
import { test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { createHash } from 'node:crypto';
import vm from 'node:vm';
import { render, sourceMapReader, identifyInspectedNode, heldTicket, holdOf, pickedPaths } from '../../scripts/agent.mjs';
import { retainDevGeneration, readDevGeneration, readDevGenerationAsync, serveBuildTree } from './serve.mjs';
import { focusController, placeReporter, timeReporter, pageReporter, viewBox, grantOrigins, launchLocation } from './navigation.js';
import { storageKey } from './storage-environment.js';
import { open } from '../../scripts/agent.mjs';
import { launchFacts, launchEnvironment, parseFlags } from '../../scripts/agent-launch.mjs';
import { nodeNamed } from '../../scripts/agent-test.mjs';

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
console.log('[]');\n`;
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
      expect(JSON.parse(good.stdout)).toEqual({passed:0,failed:0,results:[]});
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

test('driver deadlines release their timer on success, rejection and timeout', async () => {
  const driver = readFileSync(new URL('../../scripts/agent.mjs', import.meta.url), 'utf8');
  const implementation = driver.match(/async function waitAtMost\([^]*?\n\}/)[0];
  const timers = new Set();
  const wait = vm.runInNewContext(`(${implementation})`, {
    setTimeout(callback) { timers.add(callback); return callback; },
    clearTimeout(callback) { timers.delete(callback); },
  });
  expect(await wait(Promise.resolve('ready'), 20000)).toBe('ready');
  expect(timers.size).toBe(0);
  const failed = new Error('app exited');
  await expect(wait(Promise.reject(failed), 20000)).rejects.toBe(failed);
  expect(timers.size).toBe(0);
  const pending = new Promise(() => {});
  const closed = wait(pending, 2000);
  expect(timers.size).toBe(1);
  [...timers][0]();
  expect(await closed).toBeUndefined();
  expect(timers.size).toBe(0);
  const late = wait(pending, 20000, () => { throw failed; });
  [...timers][0]();
  await expect(late).rejects.toBe(failed);
  expect(timers.size).toBe(0);
});

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

// Like http-body/request-refusal.test.mjs, run the actual glue with host doubles.
// Include the real public object and nodeDetail so registration and flow facts
// are covered too. No browser geometry or scheduling is simulated as evidence.
const source = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
// A one-line declaration is the whole line (with any state it declares first).
const declaration = name => (source.match(new RegExp(`^(?:let [^;\\n]+; )?(?:async )?function ${name}\\(.*\\}$`, 'm'))
  ?? source.match(new RegExp(`(?:async )?function ${name}\\([^]*?\\n\\}`)))[0];
const publicObject = source.match(/globalThis.exact = \{[^]*?\n\};/)[0];
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function fixture(agentMode = true) {
  const events = [], state = { slots: { backPresses: 3, navigatePresses: 4 } };
  const outline = { roots: [1], nodes: [{ id: 1, type: 'Text' }] };
  const logs = { next: 1, from: 0, lines: ['boot'] };
  const box = { x: 0, y: 0, width: 100, height: 50, left: 0, top: 0, right: 100, bottom: 50 };
  const el = { isConnected: true, localName: 'p', dataset: {}, clientWidth: 100, clientHeight: 50,
    getBoundingClientRect: () => box, hasAttribute: () => false };
  const root = { dataset: {}, replaceChildren() { events.push('replace'); } };
  const context = vm.createContext({ events, agentMode, root, views: new Map([[1, el]]),
    state, outline, logs, textflow: null, flowLoading: null, flowContexts: [], flowDue: null, flowFrames: false, present() {}, lists: new Map(),
    Date: class extends Date { static now() { return 123; } }, performance: { now: () => 10 }, TextEncoder, Uint8Array,
    HTMLInputElement: class {}, HTMLTextAreaElement: class {}, HTMLIFrameElement: class {}, HTMLVideoElement: class {}, HTMLMediaElement: class {},
    foldBits: () => 0 /* no fold posture (LLP 1078 D6) */,
    document: { activeElement: null, body: {}, querySelector: () => null },
    innerWidth: 300, innerHeight: 200, devicePixelRatio: 1, scrollX: 0, scrollY: 0,
    INHERITED_CSS: {}, getComputedStyle: () => ({}), inertAncestor: () => false,
    viewBox: element => element.getBoundingClientRect(),
    navigation: { observation: () => ({ location: '/' }), reset() {} },
    presence: { live: null },
    ask: req => req.op === 'state' ? state : req.op === 'logs' ? logs : req.op === 'node' ? { id: req.id, type: 'Text' }
      : req.op === 'tags' ? { epoch: 2, incarnation: 1, clock: 0 } : { error: 'unknown op' },
    tree: () => outline, now: () => 0, environment: () => ({}),
    agentClock: 0, imageHold: null /* no animated image loaded (image-glue.js, LLP 1011.000) */, SETTLE_DEADLINE_MS: 20000, inflight: new Set(), forgettable: new Map(), waiting: () => [...context.inflight],
    // The page's own wait (http-body.js) races the deadline; here a request marked `stuck` is what a passed deadline finds.
    waitForInflight: async () => { const all = [...context.inflight]; if (all.some(p => p.stuck)) return false; await Promise.all(all); return true; },
    memory: { buffer: new ArrayBuffer(1024) }, readOut: value => value,
    wasm: { exact_in: () => 0, exact_plan: () => 1, exact_out: () => 0,
      exact_plan_fonts: () => '[]', exact_boot: () => '{"ops":[],"timers":true}',
      exact_boot_plan: () => '{"ops":[],"timers":true}', exact_advance: () => '{"ops":[],"clock":16}' },
    devAssets: null, bootAttempt: 0, encoder: new TextEncoder(), location: { pathname: '/', search: '', href: 'https://fixture.invalid/' }, URL,
    focus: focusController({ ready: () => true, elements: () => [], inert: () => false }), loadGpuIfNeeded() {}, writeIn: value => value, send() {}, messageViews: new Set(), markupModule: null,
    prepareFonts: async () => [], commitFonts() {}, releaseAssets() {}, incarnation: 0,
    // Ordinary boot tests model an already-loaded post-paint scheduler.
    timerFactory: () => ({ update() { events.push('ticker'); }, dispose() {} }),
    ticker: null, motion: { reset() {} }, arrange: { reset() {} }, pieces: { pending: () => null }, retiredViews: new WeakSet(), followedScrolls: new Map(),
    pendingScrolls: new Set(), collections: { reset() {} }, messageFrames: new Map(),
    storageRequests: null, controllers: new Set(), grants: [],
    commitTurns: [],
    applyBatch: batch => { events.push('batch'); context.agentClock = batch.clock ?? 0;
      queueMicrotask(() => context.commitTurns.push([...events])); return { timers: batch.timers, batch }; },
    activateData: () => events.push('activate'), setInterval: () => { events.push('ticker'); return 1; },
    requestAnimationFrame: () => events.push('raf'), clearInterval() {},
    ready: Promise.resolve(), moduleReady: Promise.resolve(), inputReady: true, logicInfo: null, activeModule: null,
    page: null, // a built document's boot (LLP 1048.000 D6); these pages have none
    loadStage: () => Promise.resolve(), stageLoaded: () => true, // every stage linked (LLP 1047.000 §9)
    preferences: () => '{}', localAssetURL: source => source,
    launchLocation: () => launchLocation(context), // the launch path less the drive's facts (feed F16)
  });
  vm.runInContext(`let toldOffset = null; const folded = () => false; /* no folded text here (LLP 1007.001) */ const viewBox = ${viewBox};\n` + ['let gpuLoading', 'const POST_BOUND', 'let frameSampler', 'const followOffset'].map(head => source.match(new RegExp(`^${head} = .*$`, 'm'))[0]).join('\n') + '\n' + ['nodeDetail', 'agent', 'agentNow', 'agentReply', 'settleGpu', 'gpuPendingReply', 'agentSettled', 'tagged', 'clock', 'startClock', 'mutate', 'boot', 'bootNow'].map(declaration).join('\n') + '\n' + publicObject, context);
  context.reportPlace = placeReporter(new URLSearchParams(agentMode ? 'agent=1' : ''), context);
  context.reportTime = timeReporter(new URLSearchParams(agentMode ? 'agent=1' : ''), context);
  return context;
}
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

// A timer every 300 ms whose action sends a request: the runner keeps one
// request per target, so a jump that fired every due tick at once would
// drop each reply but the last (runner/src/runner/commit.rs `enqueue`).
// Advancing until a request, the runner stops after the tick that sent.
function timedRequests(f, { stuck = false } = {}) {
  let due = 300;
  f.flowDue = due; // the boot batch's `timer_due_ms`
  f.wasm.exact_advance = (to, untilRequest) => {
    let clock = to;
    while (due <= to) {
      const at = due, p = stuck ? new Promise(() => {}) : new Promise(resolve => setTimeout(() => { f.events.push(`reply ${at}`); resolve(); }, 0));
      p.stuck = stuck; f.inflight.add(p); p.finally(() => f.inflight.delete(p));
      due += 300;
      if (untilRequest) { clock = at; break; }
    }
    f.events.push(`advance ${to}${untilRequest ? ' until a request' : ''} → ${clock}`);
    return JSON.stringify({ ops: [], clock, timers: true, timer_due_ms: due });
  };
  f.applyBatch = batch => { f.agentClock = batch.clock; f.flowDue = batch.timer_due_ms; return { timers: true, batch }; };
}
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
  f.applyBatch = batch => { f.agentClock = batch.clock; f.flowDue = batch.timer_due_ms; return { timers: true, batch }; };
  expect(await f.exact.agent({ op: 'clock', to: 300 })).toEqual({ clock: 300, epoch: 2, incarnation: 1 });
  expect(counted).toBe(1);
  expect(f.events).toEqual(['advance 300 until a request → 300', 'advance 300 until a request → 300', 'advance 300 → 300']);
});
test('a timer whose request never lands cannot hold the clock: past the deadline the rest is one advance', async () => {
  const f = fixture();
  timedRequests(f, { stuck: true });
  expect(await f.exact.agent({ op: 'clock', to: 1000 })).toEqual({ clock: 1000, epoch: 2, incarnation: 1, inflight: 3 });
  expect(f.events).toEqual(['advance 1000 until a request → 300', 'advance 1000 → 1000']);
});

test('launch setup supplies fixed defaults and carries CLI overrides to every host', () => {
  expect(launchFacts({})).toEqual({seed:1, locale:'en-US', timeZone:'UTC', epoch:Date.UTC(2026, 0, 1)});
  const {flags, rest} = parseFlags(['web', '--browser', 'firefox', '--seed', '9007199254740991', '--locale', 'fr-ca', '--time-zone', 'America/Toronto', '--epoch', '2026-09-21T14:13:20Z', 'tree']);
  expect(rest).toEqual(['web', 'tree']);
  expect(flags.browser).toBe('firefox');
  const facts = launchFacts(flags);
  expect(facts).toEqual({seed:9007199254740991, locale:'fr-CA', timeZone:'America/Toronto', epoch:1790000000000});
  expect(launchFacts({epoch:'1790000000000'}).epoch).toBe(1790000000000);
  const time = timeReporter(new URLSearchParams({agent:'1', ...facts}), new Proxy({}, {get() { throw new Error('agent read the platform'); }}));
  expect(time(60000)).toEqual([1790000000000, -240]);
  expect(timeReporter(new URLSearchParams({agent:'1'}))(0)).toEqual([Date.UTC(2026, 0, 1), 0]);
  // The offset follows the virtual date across a DST change (LLP 1069.007 D2).
  const spring = timeReporter(new URLSearchParams({agent:'1', timeZone:'America/Los_Angeles', epoch:String(Date.UTC(2026, 2, 8, 9))}));
  expect(spring(1800000)).toEqual([Date.UTC(2026, 2, 8, 9), -480]);
  expect(spring(7200000)).toEqual([Date.UTC(2026, 2, 8, 9), -420]);
  for (const epoch of ['-1', 'yesterday']) expect(() => launchFacts({epoch})).toThrow('epoch:');
  expect(launchFacts({env:launchEnvironment(facts)})).toEqual(facts);
  const params = new URLSearchParams({agent:'1', ...facts});
  const report = placeReporter(params, new Proxy({}, {get() { throw new Error('agent read the platform'); }}));
  expect(report()).toBe(['fr-CA', 'America/Toronto', 9007199254740991].join('\0'));
  expect(report()).toBe(report());
  for (const seed of [-1, 0.5, NaN, Infinity, 9007199254740992]) expect(() => launchFacts({seed})).toThrow('seed:');
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
  const compile = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { encoding:'utf8' });
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
    await chrome.close(); chrome = null;
    const { firefox } = await import('playwright-core');
    if (!existsSync(firefox.executablePath())) {
      console.log('skip: Firefox is not installed; bunx playwright@1.63.0 install firefox webkit');
      return;
    }
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
  expect(real.bits()).toBe(1 | 2 | 4);
  platform.document.visibilityState = 'visible'; platform.navigator = { onLine: true };
  expect(real.bits()).toBe(0);
  // The document pickers (studio diary R31): bit 3 where the browser has them.
  platform.showOpenFilePicker = () => {};
  expect(real.bits()).toBe(8);
  delete platform.showOpenFilePicker;
  real.onChange(() => {});
  expect(listened).toEqual(['visibilitychange', 'online', 'offline']);
  const agent = pageReporter(true, new Proxy({}, {get() { throw new Error('agent read the platform'); }}));
  expect(agent.bits()).toBe(4 | 8);
  agent.prefer({ 'visibility-state': 'hidden', online: false, 'can-open-files': false });
  expect(agent.bits()).toBe(1 | 2 | 4);
  expect(() => agent.prefer({ online: 'maybe', 'can-share': false })).toThrow('prefer: online');
  expect(agent.read()['can-share']).toBe(true);
  agent.onChange(() => { throw new Error('agent listened to the platform'); });
});

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
  expect(reported).toEqual([date, ['en-US', 'UTC', 1].join('\0'), date, ['en-US', 'UTC', 1].join('\0')]);
  expect(draws).toBe(0);
});

test('ordinary web reloads keep the launch seed', async () => {
  const f = fixture(false), reported = [];
  f.navigator = { language: 'fr-FR' };
  f.Intl = { DateTimeFormat: () => ({ resolvedOptions: () => ({timeZone:'Europe/Paris'}) }) };
  let draws = 0;
  f.crypto = { getRandomValues: bytes => { bytes.set([0, ++draws]); return bytes; } };
  f.wasm.exact_set_place = wire => { reported.push(wire); return '{"ops":[]}'; };
  await f.boot(null);
  await f.boot(new Uint8Array([1]));
  expect(reported).toEqual([['fr-FR', 'Europe/Paris', 1].join('\0'), ['fr-FR', 'Europe/Paris', 1].join('\0')]);
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

// Exercise the moved module through the real attach() adapter, with browser
// scheduling under the test's control. Geometry remains a browser concern.
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
    document: { addEventListener(kind, fn, capture) { if (kind === 'keydown') f[capture ? 'keydown' : 'keyActivate'] = fn; }, activeElement: { closest: () => null } },
    HTMLIFrameElement: class {}, HTMLInputElement: class {}, HTMLTextAreaElement: class {}, HTMLButtonElement: class {},
    inertAncestor: node => node.inert, getComputedStyle: () => ({ visibility: 'visible' }),
    requestAnimationFrame(fn) { frames.set(++serial, fn); return serial; },
    cancelAnimationFrame(id) { frames.delete(id); },
    wasm: { exact_dispatch: (...args) => args }, writeIn: value => value, now: () => 0,
    send: value => sent.push(value),
    // The window's capture listeners: they hear a contact's events wherever they land.
    addEventListener(kind, fn) { const list = windowListeners.get(kind) ?? []; list.push(fn); windowListeners.set(kind, list); },
    removeEventListener(kind, fn) { windowListeners.set(kind, (windowListeners.get(kind) ?? []).filter(g => g !== fn)); },
  });
  const module = readFileSync(new URL('./input-glue.js', import.meta.url), 'utf8');
  vm.runInContext(module.replace('export function', 'function') + '\n' + declaration('attach'), f);
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
  const watching = h => ['pointermove', 'pointerup', 'pointercancel'].map(k => h.windowListeners.get(k)?.length ?? 0);
  // Released outside the node inside the slop: the contact ends, and a later
  // buttonless move back over the node pans nothing (it did, stale, before).
  let h = inputFixture(); h.load();
  const down = h.pointer('pointerdown', nested);
  expect([down.prevented, down.stopped, h.captured]).toEqual([false, false, []]); // a tap stays the button's
  expect(watching(h)).toEqual([1, 1, 1]);
  h.outside('pointerup', { ...nested, clientX: 2, buttons: 0 });
  expect(watching(h)).toEqual([0, 0, 0]);
  h.pointer('pointermove', { ...nested, clientX: 30, buttons: 0 }); h.tick();
  expect(h.sent).toEqual([]);
  // Dragged out past the slop and released there: the pan begins and ends once.
  h = inputFixture(); h.load(); h.pointer('pointerdown', nested);
  h.outside('pointermove', { ...nested, clientX: 60 });
  h.outside('pointerup', { ...nested, clientX: 60, buttons: 0 });
  expect(h.sent).toEqual([[7, 20, '60,0', 0]]);
  h.pointer('pointermove', { ...nested, clientX: 90, buttons: 0 }); h.tick();
  expect(h.sent.length).toBe(1);
  // An up no one heard (outside the window): the next move without the button ends it.
  h = inputFixture(); h.load(); h.pointer('pointerdown', nested);
  h.pointer('pointermove', { ...nested, clientX: 30, buttons: 0 }); h.tick();
  expect([h.sent, watching(h)]).toEqual([[], [0, 0, 0]]);
  // Dragged inside: once the pan begins it captures, the window stops
  // watching, and each later event is handled once.
  h = inputFixture(); h.load(); h.pointer('pointerdown', nested);
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
  const pressable = (tag, role = null) => ({ dataset: { exactOn: 'press' }, matches: s => s.split(', ').includes(tag), getAttribute: () => role, click() { clicks++; } });
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
async function startupFixture(rustOnly = false) {
  const data = deferred(), input = deferred(), timer = deferred();
  const frames = [], loads = [], errors = [], events = [];
  const exports = { memory: {}, exact_compat: () => '{"inputs":{}}',
    exact_logic: () => '{"appId":"test.startup"}',
    exact_data_ready: () => { events.push('activate'); return '{"ops":[]}'; },
    ...(rustOnly ? {} : { exact_module_artifact() {} }) };
  const f = vm.createContext({ frames, loads, errors, events,
    root: { dataset: {}, setAttribute(key, value) { this[key] = value; } },
    views: new Map(), retiredViews: new WeakSet(), authoredDisabled: new WeakMap(),
    inputReady: false, inputHandlers: null, wasm: null, memory: null, logicInfo: null,
    moduleLoader: null, activeModule: null, timerFactory: null, agentMode: false,
    performance: { now: () => 1 }, t0: 0, URL, localStorage: { length: 0 }, AbortController,
    document: { querySelectorAll: () => [] }, // no preload: the glue fetches ./app.wasm
    fetch: async () => ({}), WebAssembly: { instantiateStreaming: async () => ({ instance: { exports } }), Module: { customSections: () => [], imports: () => [] } },
    moduleCall() {}, rustImports: {}, dataImports: {}, grantOrigins, readOut: value => value,
    boot: async () => events.push('boot'), loadGpuIfNeeded() {}, startClock() {}, httpHelpers() {}, pieces: { pending: () => null },
    requestAnimationFrame: fn => frames.push(fn), console: { error: error => errors.push(String(error)) },
    motion: { commit() {} }, collections: { dataReady: () => events.push('collections') },
    applyBatch: () => events.push('batch'), inertAncestor: () => false, focusAutofocus() {},
    resolveModuleReady: () => events.push('ready'), page: null, pageNative: undefined, log() {}, // the input piece's journal (a cancelled pan's line)
    loadAfterPaint(file) {
      loads.push(file);
      if (file === './input-glue.js') return input.promise;
      if (file === './timer-glue.js') return timer.promise;
      if (file === './module-glue.js') return Promise.resolve({ baked: () => data.promise, prepare: async () => ({ id: 0 }) });
      if (file === './frames.js') return new Promise(() => {}); // a development page's frame sampler (LLP 1079 D3) is not what these tests start
      throw new Error('unexpected startup module: ' + file);
    },
  });
  f.exact = {};
  vm.runInContext([source.match(/^const AGENT_ADMITTED = .*$/m)[0], ...['setInputReady', 'activateData', 'main'].map(declaration)].join('\n')
    .replaceAll('import.meta.url', '"https://fixture.invalid/glue.js"'), f);
  await f.main();
  expect(loads).toEqual([]); // Neither optional nor app modules run before paint.
  frames.shift()();
  expect(loads).toEqual([]);
  const activation = frames.shift()();
  return { f, data, input, timer, activation };
}

test('pending, failed and invalid optional modules leave Rust and JS apps ready after paint', async () => {
  for (const rustOnly of [false, true]) for (const failure of ['pending', 'reject', 'invalid']) {
    const h = await startupFixture(rustOnly), { f } = h;
    if (!rustOnly) expect(f.inputReady).toBe(false);
    h.data.resolve({});
    await checkpoint();
    expect(f.root.dataset.moduleReady).toBe('true');
    expect(f.inputReady).toBe(true);
    expect(f.root['aria-busy']).toBe('false');
    expect(f.events).toEqual(['boot', 'activate', 'batch', 'collections', 'ready']);
    expect(f.inputHandlers).toBeNull(); // A stalled import has no readiness deadline.
    expect(f.loads.filter(file => file === './input-glue.js')).toHaveLength(1);
    h.timer.reject(new Error('timer unavailable'));
    if (failure === 'reject') h.input.reject(new Error('input unavailable'));
    else if (failure === 'invalid') h.input.resolve(undefined);
    else h.input.resolve(() => ({ pan: 'installed after readiness' }));
    await h.activation;
    await checkpoint();
    expect(f.root.dataset.error).toBeUndefined();
    expect(f.root.dataset.moduleReady).toBe('true');
    expect(f.events.filter(event => event === 'activate')).toHaveLength(1);
    expect(f.errors).toHaveLength(failure === 'pending' ? 1 : 2);
    if (failure === 'pending') expect(f.inputHandlers.pan).toBe('installed after readiness');
  }
});

test('required app module failure still keeps dispatch gated and settles the error', async () => {
  const h = await startupFixture();
  h.data.reject(new Error('required app unavailable'));
  await h.activation;
  expect(h.f.inputReady).toBe(false);
  expect(h.f.root.dataset.moduleReady).toBeUndefined();
  expect(h.f.root.dataset.error).toContain('required app unavailable');
  expect(h.f.events).toEqual(['boot', 'ready']);
  h.input.resolve(() => ({})); h.timer.resolve(() => ({}));
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

test('module replacement drains current requests before swapping and rechecks supersession', async () => {
  for (const result of ['accept', 'superseded', 'timeout']) {
    const f = fixture(false), entered = deferred(), drained = deferred();
    let current = true, swaps = 0;
    f.waitForInflight = async deadline => {
      expect(deadline).toBe(20010);
      entered.resolve();
      return drained.promise;
    };
    f.setInputReady = value => { f.inputReady = value; };
    f.wasm.exact_boot_module = () => { swaps++; return '{"ops":[],"timers":true}'; };
    const candidate = {realm:{id:1},receipt:new Uint8Array([1])};
    const update = f.boot(new Uint8Array([1]), null, () => current, candidate);
    const outcome = update.then(value => ({value}), error => ({error}));
    await entered.promise;
    expect(swaps).toBe(0);
    expect(f.views.size).toBe(1);
    expect(f.inputReady).toBe(true);
    if (result === 'superseded') current = false;
    drained.resolve(result !== 'timeout');
    const reply = await outcome;
    if (result === 'accept') {
      expect(reply.error).toBeUndefined();
      expect(swaps).toBe(1);
      expect(f.activeModule).toBe(candidate);
    } else {
      expect(swaps).toBe(0);
      expect(f.views.size).toBe(1);
      expect(f.activeModule).toBeNull();
      if (result === 'timeout') expect(String(reply.error)).toContain('in-flight requests');
      else expect(reply.value).toBeNull();
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

// @ref LLP 1080.002 D7–D9 — the findings, parity and transcript over hand-written replies.
import { axFindings, axParity, axClean, axFinish, renderAx, webAx, axTree } from '../../scripts/agent-ax.mjs';
const axReply = (elements, extra = {}) => ({ epoch: 3, incarnation: 1, clock: 0, ax: { source: 'chrome-cdp', platform: 'Chrome 1', order: 'tree',
  coverage: { roots: ['document'], complete: true, visited: elements.length }, modal: { present: false }, elements, ...extra } });
const el = (i, role, name, more = {}) => ({ i, parent: null, id: i + 10, via: 'self', role, name, states: {}, interactive: ['button', 'link', 'textbox', 'checkbox'].includes(role), native: { role }, ...more });
const axPlain = { nodes: [
  { id: 10, parent: null, props: { testId: 'play' }, children: [] },
  { id: 11, parent: null, props: { testId: 'box', inert: true }, children: [12] },
  { id: 12, parent: 11, props: { testId: 'inside' }, children: [] },
  { id: 13, parent: null, props: { testId: 'sheet' }, children: [14] },
  { id: 14, parent: 13, props: { testId: 'ok' }, children: [] },
  { id: 15, parent: null, props: { testId: 'behind' }, children: [] } ] };

test('tree --ax: unnamed fires on an interactive element with no name and is silent once named', () => {
  const unnamed = axFinish(axReply([el(0, 'button', '')]), axPlain, null).ax.findings;
  expect(unnamed.map(f => [f.kind, f.testId])).toEqual([['unnamed', 'play']]);
  expect(axFinish(axReply([el(0, 'button', 'Play')]), axPlain, null).ax.findings).toEqual([]);
  // Text is not interactive: an empty name there is no finding.
  expect(axFinish(axReply([el(0, 'StaticText', '')]), axPlain, null).ax.findings).toEqual([]);
});

test('tree --ax: exposed-hidden fires under an inert ancestor in intent and is silent outside it', () => {
  const r = axFinish(axReply([el(2, 'button', 'Hidden')]), axPlain, null);
  expect(r.ax.findings.map(f => [f.kind, f.testId, f.under])).toEqual([['exposed-hidden', 'inside', 11]]);
  expect(r.ax.intent[11]).toEqual({ inert: true, testId: 'box' });
  expect(axFinish(axReply([el(4, 'button', 'Ok')]), axPlain, null).ax.findings).toEqual([]);
});

test('tree --ax: outside-modal fires for an element outside the open modal, by the platform or UIKit\'s rule', () => {
  const sheet = el(3, 'dialog', 'Sheet'), ok = el(4, 'button', 'Ok', { parent: 0 }), behind = el(5, 'button', 'Behind');
  sheet.i = 0; ok.i = 1; behind.i = 2;
  const modal = { present: true, element: 0, id: 13, by: 'dialog:modal' };
  const web = axFinish(axReply([sheet, ok, behind], { modal }), axPlain, null).ax.findings;
  expect(web.map(f => [f.kind, f.testId, f.basis])).toEqual([['outside-modal', 'behind', 'platform']]);
  const kit = axFinish(axReply([sheet, ok, { ...behind, outsideModal: false }], { modal, source: 'uikit' }), axPlain, null).ax.findings;
  expect(kit).toEqual([]);
  const leak = axFinish(axReply([sheet, ok, { ...behind, outsideModal: true }], { modal, source: 'uikit' }), axPlain, null).ax.findings;
  expect(leak.map(f => [f.kind, f.basis])).toEqual([['outside-modal', 'documented-rule']]);
});

test('tree --ax: a targeted read keeps its subtree, the chain above it, and its ancestors\' intent', () => {
  const box = el(1, 'group', 'Box'), inside = el(2, 'button', 'Hidden', { parent: 0 });
  box.i = 0; inside.i = 1;
  const r = axFinish(axReply([el(0, 'button', 'Play'), box, inside].map((e, i) => ({ ...e, i, parent: e === inside ? 1 : null }))), axPlain, 12);
  expect(r.ax.elements.map(e => e.testId)).toEqual(['inside']);
  expect(r.ax.ancestors).toEqual([{ id: 11, role: 'group', name: 'Box' }]);
  expect(r.ax.intent[11].inert).toBe(true);
});

test('tree --ax: parity joins by unique testId, normalizes the fixture controls, and refuses a partial side', () => {
  const web = axReply([el(0, 'checkbox', 'Checked', { testId: 'check', states: { checked: true } }), el(1, 'button', 'Add', { testId: 'add' }), el(2, 'button', 'A', { testId: 'dup' }), el(3, 'button', 'B', { testId: 'dup' })]);
  const ios = axReply([
    el(0, 'checkbox', 'Checked', { testId: 'check', value: 'checked', native: { role: ['button'] } }),
    el(1, 'button', 'Plus', { testId: 'add', native: { role: ['button'] } })], { source: 'uikit' });
  const { findings, unjoined } = axParity(web, ios);
  expect(findings.map(f => f.detail)).toEqual(['name "Add" (web) vs "Plus" (uikit)']);
  expect(unjoined).toEqual([]);
  expect(() => axParity({ ...web, spanned: true }, ios)).toThrow(/spanned/);
});

test('tree --ax: no findings means clean only with complete coverage and the expected views joined', () => {
  const r = axFinish(axReply([el(0, 'button', 'Play')]), axPlain, null);
  expect(axClean(r, ['play'])).toBe(true);
  expect(axClean(r, ['missing'])).toBe(false);
  const partial = axReply([el(0, 'button', 'Play')], { truncated: { elements: 'unknown', fields: 0 } });
  expect(axClean(partial)).toBe(false);
  expect(renderAx(partial)).toContain('(no findings — coverage incomplete)');
});

test('tree --ax renders each element with its join and frame, then its findings', () => {
  const r = axFinish(axReply([el(0, 'button', '', { frame: { x: 1, y: 2, w: 3, h: 4, source: 'layout' } }), el(2, 'button', 'Hidden', { via: 'owner' })]), axPlain, null);
  const text = renderAx(r);
  expect(text).toMatch(/^ax {7}chrome-cdp · Chrome 1 · order tree · epoch 3 · incarnation 1 · clock 0 ms · 2 elements/);
  expect(text).toContain('button "" #10 [play] 1,2 3×4');
  expect(text).toContain('button "Hidden" #12^ [inside]');
  expect(text).toContain('! unnamed button #10 [play]');
  expect(text).toContain('! exposed while hidden: button "Hidden" under #11 (inert) #12 [inside]');
  expect(renderAx({ ax: { unavailable: true, reason: 'no AT-SPI tree (LLP 1015 §7)' } })).toBe('ax       unavailable: no AT-SPI tree (LLP 1015 §7)');
});

// Astra's review of a6d847f1 (llp/reviews/code-2026-10-03-1080.002-ax-tree.astra.md): 2, 5, 8, 11, 12.
const fakeChrome = ({ nodes, stamps, plain }) => {
  const order = [];
  const snapshot = { strings: ['data-agent-view', '1'], documents: [{ nodes: { backendNodeId: [100], attributes: [[0, 1]], parentIndex: [-1], nodeType: [1], nodeName: [0] }, layout: { nodeIndex: [], bounds: [] } }] };
  return { order, carrier: { browser: 'chrome', axEnabled: 'Chrome 1', async call(m) { order.push(m); return m === 'Accessibility.getFullAXTree' ? { nodes } : snapshot; },
    async evaluate() { return 'Chrome/1'; }, async ask() { order.push('stamp'); return stamps.shift(); } },
    readPlain: async () => { order.push('plain'); return plain.shift(); } };
};
const button = { nodeId: 'b', ignored: false, role: { value: 'button' }, name: { value: 'Go' }, backendDOMNodeId: 100, childIds: [] };

test('tree --ax (web): the plain tree is read inside the document bracket, and a reload between retries the whole read', async () => {
  const stamp = (nonce, epoch = 1) => ({ nonce, epoch, incarnation: 1, clock: 0 });
  const f = fakeChrome({ nodes: [{ nodeId: 'r', role: { value: 'RootWebArea' }, childIds: ['b'] }, button],
    stamps: [stamp(1), stamp(2), stamp(2), stamp(2)], plain: [{ epoch: 1, incarnation: 1, nodes: [] }, { epoch: 1, incarnation: 1, nodes: [{ id: 1, props: { testId: 'new' }, children: [] }] }] });
  const { reply, plain } = await webAx(f.carrier, {}, f.readPlain);
  expect(f.order.slice(0, 5)).toEqual(['stamp', 'plain', 'Accessibility.getFullAXTree', 'DOMSnapshot.captureSnapshot', 'stamp']);
  expect(reply.spanned).toBeUndefined();
  expect(plain.nodes[0].props.testId).toBe('new'); // the second document's intent, never the first's
});

test('tree --ax (web): a depth cutoff is incomplete coverage with an unknown remainder, never clean', async () => {
  const chain = Array.from({ length: 300 }, (_, k) => ({ nodeId: `g${k}`, ignored: true, role: { value: 'generic' }, childIds: [k < 299 ? `g${k + 1}` : 'b'] }));
  const f = fakeChrome({ nodes: [{ nodeId: 'r', role: { value: 'RootWebArea' }, childIds: ['g0'] }, ...chain, button], stamps: [{ nonce: 1, epoch: 1, incarnation: 1 }, { nonce: 1, epoch: 1, incarnation: 1 }], plain: [null] });
  const { reply } = await webAx(f.carrier, {}, f.readPlain);
  expect(reply.ax.elements).toEqual([]);
  expect(reply.ax.coverage.complete).toBe(false);
  expect(reply.ax.truncated.elements).toBe('unknown');
  expect(axClean(reply)).toBe(false);
});

test('tree --ax: a target inside a modal is judged against the whole modal, not reported outside it', () => {
  const dialog = el(3, 'dialog', 'Sheet'), ok = el(4, 'button', 'Ok');
  dialog.i = 0; ok.i = 1; ok.parent = 0;
  const r = axFinish(axReply([dialog, ok], { modal: { present: true, element: 0, id: 13, by: 'dialog:modal' } }), axPlain, 14);
  expect(r.ax.elements.map(e => e.testId)).toEqual(['ok']);
  expect(r.ax.findings).toEqual([]);
});

test('tree --ax: the reply fits 256 KB of UTF-8, and its intent and findings cover only the elements it keeps', () => {
  const many = Array.from({ length: 2000 }, (_, i) => ({ ...el(0, 'button', ''), i, id: 10, description: 'é'.repeat(200) }));
  const r = axFinish(axReply(many), axPlain, null);
  expect(Buffer.byteLength(JSON.stringify(r))).toBeLessThanOrEqual(256 * 1024);
  expect(r.ax.elements.length).toBeLessThan(2000);
  expect(r.ax.findings.length).toBe(r.ax.elements.length);
  expect(r.ax.truncated.elements).toBe(2000 - r.ax.elements.length);
  expect(r.ax.coverage.complete).toBe(false);
});

test('tree --ax: parity reports a state one side can observe and does not, before comparing values', () => {
  const web = axReply([el(0, 'heading', 'Section', { testId: 'h', states: { level: 2 } }), el(1, 'checkbox', 'C', { testId: 'c', states: { checked: false } })]);
  const mac = axReply([el(0, 'heading', 'Section', { testId: 'h', states: {}, native: { role: 'AXHeading' } }), el(1, 'checkbox', 'C', { testId: 'c', states: {}, native: { role: 'AXCheckBox' } })], { source: 'appkit' });
  expect(axParity(web, mac).findings.map(f => f.detail)).toEqual(['level missing on appkit (2 vs —)', 'checked missing on appkit (false vs —)']);
  // disabled is reported only when true: its absence on both sides agrees.
  expect(axParity(web, axReply([el(0, 'heading', 'Section', { testId: 'h', states: { level: 2 } }), el(1, 'checkbox', 'C', { testId: 'c', states: { checked: false } })])).findings).toEqual([]);
});

test('tree --ax: an aria-pressed toggle is a button on every source', () => {
  const web = axReply([el(0, 'button', 'Shown', { testId: 'pressed', states: {} })]);
  const mac = axReply([el(0, 'checkbox', 'Shown', { testId: 'pressed', native: { role: 'AXCheckBox', subrole: 'AXToggle' } })], { source: 'appkit' });
  const ios = axReply([el(0, 'toggleButton', 'Shown', { testId: 'pressed', value: '1', native: { role: ['toggleButton'] } })], { source: 'uikit' });
  expect(axParity(web, mac).findings).toEqual([]);
  expect(axParity(web, ios).findings).toEqual([]);
});

// Astra's round 2 (llp/reviews/code-2026-10-03-1080.002-ax-tree-r2.astra.md): 3 and 4.
test('tree --ax: parity compares expanded where a side reports it, and AppKit\'s true-only report', () => {
  const web = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', states: { expanded: true } }), el(1, 'button', 'Other', { testId: 'other', states: { expanded: false } })]);
  const ios = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', states: { expanded: false }, native: { role: ['button'] } }), el(1, 'button', 'Other', { testId: 'other', states: { expanded: false }, native: { role: ['button'] } })], { source: 'uikit' });
  expect(axParity(web, ios).findings.map(f => f.detail)).toEqual(['expanded true (web) vs false (uikit)']);
  const mac = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', native: { role: 'AXButton' } }), el(1, 'button', 'Other', { testId: 'other', native: { role: 'AXButton' } })], { source: 'appkit' });
  expect(axParity(web, mac).findings.map(f => f.detail)).toEqual(['expanded true (web) vs not reported (appkit)']);
  const agree = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', states: { expanded: true }, native: { role: 'AXButton' } }), el(1, 'button', 'Other', { testId: 'other', native: { role: 'AXButton' } })], { source: 'appkit' });
  expect(axParity(web, agree).findings).toEqual([]);
});

test('tree --ax: sixty long guest-frame URLs still leave a reply within 256 KB, their truncation counted', async () => {
  const docs = Array.from({ length: 61 }, (_, k) => ({ documentURL: k + 2, nodes: { backendNodeId: [], attributes: [], parentIndex: [], nodeType: [], nodeName: [] }, layout: { nodeIndex: [], bounds: [] } }));
  const strings = ['data-agent-view', '1', ...Array.from({ length: 61 }, (_, k) => `https://example.com/${k}/` + 'x'.repeat(5000))];
  docs[0] = { nodes: { backendNodeId: [100], attributes: [[0, 1]], parentIndex: [-1], nodeType: [1], nodeName: [0] }, layout: { nodeIndex: [], bounds: [] } };
  const snapshot = { strings, documents: docs };
  const carrier = { browser: 'chrome', axEnabled: 'Chrome 1', async evaluate() { return 'Chrome/1'; },
    async call(m) { return m === 'Accessibility.getFullAXTree' ? { nodes: [{ nodeId: 'r', role: { value: 'RootWebArea' }, childIds: ['b'] }, button] } : snapshot; },
    async ask() { return { nonce: 1, epoch: 1, incarnation: 1, clock: 0 }; } };
  const { reply } = await webAx(carrier, {}, async () => ({ epoch: 1, incarnation: 1, nodes: [{ id: 1, props: {}, children: [] }] }));
  const r = axFinish(reply, { nodes: [{ id: 1, props: {}, children: [] }] }, null);
  expect(Buffer.byteLength(JSON.stringify(r))).toBeLessThanOrEqual(256 * 1024);
  expect(r.ax.coverage.excluded.length).toBe(8);
  expect(r.ax.coverage.excludedMore).toBe(52);
  expect(r.ax.coverage.excluded.every(x => x.frame.length <= 200)).toBe(true);
  expect(r.ax.coverage.complete).toBe(false);
  expect(r.ax.elements.length).toBe(1);
  // And a reply whose metadata alone is too big gives the metadata up, counted, never the bound.
  const huge = axReply([], { coverage: { roots: ['document'], complete: true, visited: 0, excluded: Array.from({ length: 60 }, () => ({ frame: 'x'.repeat(5000), reason: 'r' })) } });
  const h = axFinish(huge, { nodes: [] }, null);
  expect(Buffer.byteLength(JSON.stringify(h))).toBeLessThanOrEqual(256 * 1024);
  expect(h.ax.truncated.excluded).toBe(60);
});

// Astra's round 3 (llp/reviews/code-2026-10-03-1080.002-ax-tree-r3.astra.md): 1, 2 and 4.
test('tree --ax: the host resolves the target (the active route\'s view first), and the reply is scoped by that id', async () => {
  const plain = { epoch: 1, incarnation: 1, roots: [1], nodes: [
    { id: 1, parent: null, props: {}, children: [2, 3] }, { id: 2, parent: 1, props: { testId: 'dup' }, inactive: true, children: [] }, { id: 3, parent: 1, props: { testId: 'dup' }, children: [] }] };
  const native = { epoch: 1, incarnation: 1, ax: { source: 'uikit', order: 'containment', coverage: { roots: [], complete: true, visited: 3 }, modal: { present: false },
    elements: [{ ...el(0, 'button', 'Covered'), id: 2 }, { ...el(1, 'button', 'Showing'), i: 1, id: 3 }] } };
  const asked = [];
  const s = { carrier: { host: 'ios' }, async op(req) { asked.push(req); if (req.ax) return structuredClone(native); if (req.target === 'dup') return { ...plain, roots: [3] }; return req.target != null ? plain : structuredClone(plain); } };
  const r = await axTree(s, 'dup');
  expect(r.ax.elements.map(e => e.name)).toEqual(['Showing']);
  expect(r.ax.target).toBe(3);
  expect(asked.findIndex(q => q.target === 'dup')).toBeLessThan(asked.findIndex(q => q.ax)); // resolved inside the bracket, before the read
});

test('tree --ax: a 70-deep inert ancestor still hides, and targeting it keeps its deepest descendant', () => {
  const nodes = Array.from({ length: 71 }, (_, k) => ({ id: k + 1, parent: k ? k : null, props: k === 0 ? { inert: true } : k === 70 ? { testId: 'deep' } : {}, children: k < 70 ? [k + 2] : [] }));
  const reply = axReply([{ ...el(0, 'button', 'Deep'), id: 71 }]);
  const r = axFinish(reply, { nodes }, null);
  expect(r.ax.findings.map(f => [f.kind, f.under])).toEqual([['exposed-hidden', 1]]);
  expect(axClean(r)).toBe(true); // complete, and the finding is reported
  const t = axFinish(axReply([{ ...el(0, 'button', 'Deep'), id: 71 }]), { nodes }, 1);
  expect(t.ax.elements.map(e => e.testId)).toEqual(['deep']);
});

test('tree --ax: parity skips a state the runtime cannot observe (UIKit expanded before iOS 18)', () => {
  const web = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', states: { expanded: true } })]);
  const ios17 = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', native: { role: ['button'] } })], { source: 'uikit', observes: ['checked', 'disabled'] });
  expect(axParity(web, ios17).findings).toEqual([]);
  const ios18 = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', native: { role: ['button'] } })], { source: 'uikit', observes: ['checked', 'disabled', 'expanded'] });
  expect(axParity(web, ios18).findings.map(f => f.detail)).toEqual(['expanded missing on uikit (true vs —)']);
});

// files F11: a held device request is named by the node its answer arrives at
// or by its capability, whatever ticket the host's counter gave it.
test('a hold is addressed by its node or capability, and an unclear name is refused', () => {
  const pending = [{ name: 'notes', ticket: 4 }, { name: 'folder-input', ticket: 7, device: { capability: 'open-directory', args: { id: 'folder-input' } } },
    { name: 'share', ticket: 9, device: { capability: 'share', args: {} } }, { name: 'export-input', ticket: 11, device: { capability: 'export', args: { id: 'export-input' } } }];
  expect([holdOf('@7'), holdOf('@folder-input'), holdOf('folder-input'), holdOf('@')]).toEqual([true, true, false, false]);
  expect([heldTicket(pending, '@folder-input'), heldTicket(pending, '@open-directory'), heldTicket(pending, '@share'), heldTicket(pending, '@export')]).toEqual([7, 7, 9, 11]);
  expect(() => heldTicket(pending, '@notes')).toThrow(/no held device request .*held: @7 open-directory at "folder-input"/);
  expect(() => heldTicket([...pending, { name: 'other', ticket: 12, device: { capability: 'open-directory', args: { id: 'other' } } }], '@open-directory')).toThrow(/2 holds match/);
});

// Review A2: an authored test's `pick "photo" "my photo.png"` reaches the driver as one path per line, so a space
// stays in its path; the CLI's `type @photo a.png b.png` still names two.
test('a pick answer keeps a path with a space when it comes one per line', () => {
  expect(pickedPaths('/tmp/my photo.png\n')).toEqual(['/tmp/my photo.png']);
  expect(pickedPaths('/tmp/a b.png\n/tmp/c.png\n')).toEqual(['/tmp/a b.png', '/tmp/c.png']);
  expect(pickedPaths('a.png b.png')).toEqual(['a.png', 'b.png']);
});

// Review A1: `tap … drag … mouse` (an authored test's `drag … from x y mouse`) holds the left button on the web: the
// page hears a mouse's pointerdown and pointerup, where `tap` itself refuses `mouse` beside `down` (its click form).
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
  const compile = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', contract, '-o', plan], { encoding:'utf8' });
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

// Review A1 delta: a native contact's held left button ends with its lift, its cancel, or a down that failed — an
// error reply or a request that threw — so a later finger is not sent as a mouse.
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
