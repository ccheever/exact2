// @ref LLP 1043.000 §3 D7/D8 — flow settlement must not change LLP 1012's API.
import { test, expect } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { createHash } from 'node:crypto';
import vm from 'node:vm';
import { render, sourceMapReader, identifyInspectedNode } from '../../scripts/agent.mjs';
import { retainDevGeneration, readDevGeneration, readDevGenerationAsync } from './serve.mjs';
import { focusController } from './navigation.js';
import { storageKey } from './storage-environment.js';
import { open } from '../../scripts/agent.mjs';

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
    const missing = spawnSync(process.execPath, [runner], {
      cwd:dir, env:{...process.env,PATH:join(dir,'absent')}, encoding:'utf8',
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
    state, outline, logs, textflow: null, flowLoading: null, flowContexts: [], flowDue: null, lists: new Map(),
    Date: { now: () => 123 }, performance: { now: () => 10 }, TextEncoder, Uint8Array,
    HTMLInputElement: class {}, HTMLTextAreaElement: class {}, HTMLIFrameElement: class {}, HTMLVideoElement: class {},
    document: { activeElement: null, body: {}, querySelector: () => null },
    innerWidth: 300, innerHeight: 200, devicePixelRatio: 1, scrollX: 0, scrollY: 0,
    INHERITED_CSS: {}, getComputedStyle: () => ({}), inertAncestor: () => false,
    navigation: { observation: () => ({ location: '/' }), reset() {} },
    ask: req => req.op === 'state' ? state : req.op === 'logs' ? logs : req.op === 'node' ? { id: req.id, type: 'Text' }
      : req.op === 'tags' ? { epoch: 2, incarnation: 1, clock: 0 } : { error: 'unknown op' },
    tree: () => outline, now: () => 0, environment: () => ({}),
    agentClock: 0, SETTLE_DEADLINE_MS: 20000, inflight: new Set(),
    // The page's own wait (http-body.js) races the deadline; here a request marked `stuck` is what a passed deadline finds.
    waitForInflight: async () => { const all = [...context.inflight]; if (all.some(p => p.stuck)) return false; await Promise.all(all); return true; },
    memory: { buffer: new ArrayBuffer(1024) }, readOut: value => value,
    wasm: { exact_in: () => 0, exact_plan: () => 1, exact_out: () => 0,
      exact_plan_fonts: () => '{}', exact_boot: () => '{"ops":[],"timers":true}',
      exact_boot_plan: () => '{"ops":[],"timers":true}', exact_advance: () => '{"ops":[],"clock":16}' },
    devAssets: null, bootAttempt: 0, encoder: new TextEncoder(), location: { pathname: '/', search: '' },
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
    preferences: () => '{}',
  });
  vm.runInContext(source.match(/^let gpuLoading = .*$/m)[0] + '\n' + ['nodeDetail', 'agent', 'agentNow', 'agentReply', 'settleGpu', 'agentSettled', 'tagged', 'clock', 'startClock', 'mutate', 'boot', 'bootNow'].map(declaration).join('\n') + '\n' + publicObject, context);
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
  expect(await f.exact.agent({ op: 'clock', to: 700 })).toEqual({ clock: 700, epoch: 2, incarnation: 1 });
  // The last advance fires no timer, so the request it left in flight is not waited for.
  expect(f.events).toEqual(['reply boot', 'advance 700 until a request → 300', 'reply 300', 'advance 700 until a request → 600', 'advance 700 → 700']);
  expect(f.inflight.size).toBe(1);
  // A due time at the target itself is the last stop: what is in flight lands first, nothing follows.
  f.events.length = 0;
  expect(await f.exact.agent({ op: 'clock', to: 900 })).toEqual({ clock: 900, epoch: 2, incarnation: 1 });
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
  expect(await f.exact.agent({ op: 'clock', to: 1000 })).toEqual({ clock: 1000, epoch: 2, incarnation: 1 });
  expect(f.events).toEqual(['advance 1000 until a request → 300', 'advance 1000 → 1000']);
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
  const actual = Object.entries(samples).map(([name, value]) => `--- ${name}\n${render(['empty', 'dropped'].includes(name) ? 'logs' : name, value)}`).join('\n\n') + '\n';
  expect(actual).toBe(expected);
});

// Exercise the moved module through the real attach() adapter, with browser
// scheduling under the test's control. Geometry remains a browser concern.
function inputFixture() {
  const listeners = new Map(), frames = new Map(), sent = [], captured = [];
  let serial = 0;
  const el = { dataset: {}, inert: false, disabled: false, isConnected: true,
    addEventListener(kind, fn) { const list = listeners.get(kind) ?? []; list.push(fn); listeners.set(kind, list); },
    closest() { return this.disabled ? this : null; }, setPointerCapture(id) { captured.push(id); } };
  const buttons = [];
  // `page` is a built document being adopted (LLP 1048.000 D6); this page was not built.
  const f = vm.createContext({ inputReady: false, inputHandlers: null, page: null,
    views: new Map([[7, el]]), retiredViews: new WeakSet(), frames, sent, captured, el,
    root: { querySelectorAll: () => buttons, addEventListener() {} }, // press feedback's listener: press.test.mjs drives it
    document: { addEventListener(kind, fn) { if (kind === 'keydown') f.keydown = fn; }, activeElement: { closest: () => null } },
    HTMLIFrameElement: class {}, HTMLInputElement: class {}, HTMLTextAreaElement: class {}, HTMLButtonElement: class {},
    inertAncestor: node => node.inert, getComputedStyle: () => ({ visibility: 'visible' }),
    requestAnimationFrame(fn) { frames.set(++serial, fn); return serial; },
    cancelAnimationFrame(id) { frames.delete(id); },
    wasm: { exact_dispatch: (...args) => args }, writeIn: value => value, now: () => 0,
    send: value => sent.push(value),
  });
  const module = readFileSync(new URL('./input-glue.js', import.meta.url), 'utf8');
  vm.runInContext(module.replace('export function', 'function') + '\n' + declaration('attach'), f);
  f.attach(el, 7, ['pan']);
  function event(extra = {}) {
    return { isPrimary: true, button: 0, pointerId: 1, clientX: 0, clientY: 0,
      target: { closest: () => null }, prevented: false, stopped: false,
      preventDefault() { this.prevented = true; }, stopPropagation() { this.stopped = true; },
      stopImmediatePropagation() { this.stopped = true; }, ...extra };
  }
  return { f, el, buttons, frames, sent, captured,
    load() {
      vm.runInContext(`inputHandlers = createInputHandlers({ root, views, retiredViews,
        ready: () => inputReady, inertAncestor,
        dispatch: (id, payload) => send(wasm.exact_dispatch(id, 20, writeIn(payload), now())),
      }); inputReady = true;`, f);
    },
    pointer(kind, extra) { const e = event(extra); for (const fn of listeners.get(kind) ?? []) fn(e); return e; },
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
  for (const cancel of [h => h.pointer('pointercancel'), h => h.pointer('lostpointercapture'),
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
    fetch: async () => ({}), WebAssembly: { instantiateStreaming: async () => ({ instance: { exports } }), Module: { customSections: () => [] } },
    moduleCall() {}, rustImports: {}, readOut: value => value,
    boot: async () => events.push('boot'), loadGpuIfNeeded() {}, startClock() {}, httpHelpers() {}, pieces: { pending: () => null },
    requestAnimationFrame: fn => frames.push(fn), console: { error: error => errors.push(String(error)) },
    motion: { commit() {} }, collections: { dataReady: () => events.push('collections') },
    applyBatch: () => events.push('batch'), inertAncestor: () => false, focusAutofocus() {},
    resolveModuleReady: () => events.push('ready'), page: null, pageNative: undefined,
    loadAfterPaint(file) {
      loads.push(file);
      if (file === './input-glue.js') return input.promise;
      if (file === './timer-glue.js') return timer.promise;
      if (file === './module-glue.js') return Promise.resolve({ baked: () => data.promise, prepare: async () => ({ id: 0 }) });
      throw new Error('unexpected startup module: ' + file);
    },
  });
  f.exact = {};
  vm.runInContext(['setInputReady', 'activateData', 'main'].map(declaration).join('\n')
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
  iframe.getAttribute = () => '/guest'; iframe.matches = () => false;
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

function listFixture() {
  const frames = [], observers = [], events = [], reports = [], listeners = new Map();
  const document = { activeElement: null,
    addEventListener: (name, fn) => listeners.set(name, fn),
    createElement: () => ({}), head: { append() {} } };
  const lists = new Map(), views = new Map();
  const root = { addEventListener() {} };
  const context = vm.createContext({ document, root, lists, views, globalThis: { exact: {} },
    getSelection: () => ({ isCollapsed: true, anchorNode: null, focusNode: null, anchorOffset: 0, focusOffset: 0,
      toString() { throw new Error('rendered selection text forces layout'); } }),
    requestAnimationFrame: fn => { frames.push(fn); return frames.length; },
    ResizeObserver: class {
      constructor(callback) { this.callback = callback; this.watched = new Map(); observers.push(this); }
      observe(el, options) { this.watched.set(el, options); }
      unobserve(el) { this.watched.delete(el); }
      disconnect() { this.watched.clear(); }
    },
  });
  vm.runInContext(readFileSync(new URL('./list-selection.js', import.meta.url), 'utf8'), context);
  let pending = false;
  const controller = context.globalThis.exact.installListSelection({ root, lists, views,
    report: (id, geometry, measurements, limit) => {
      events.push('write'); reports.push({ id, geometry, measurements, limit }); return pending;
    }, index() {}, text() {},
  });
  function add(id) {
    const handlers = new Map(), row = { dataset: { view: String(id + 100) },
      getBoundingClientRect() { throw new Error('row geometry must come from ResizeObserver'); } };
    const content = { children: [row], get clientWidth() { events.push('read'); return el.width; },
      getBoundingClientRect() { events.push('read'); return { top: 12 - el.scrollTop }; } };
    const el = { isConnected: true, firstElementChild: content, scrollTop: 0, clientTop: 0, width: 390,
      get clientHeight() { events.push('read'); return 240; },
      getBoundingClientRect() { events.push('read'); return { top: 0 }; },
      contains: target => target === row,
      addEventListener: (name, fn) => handlers.set(name, fn),
    };
    row.closest = () => row;
    lists.set(el, { id, measured: true, window: true }); views.set(id, el); controller.sync();
    const observer = observers.at(-1);
    const measure = (target = row, width = el.width, height = 31.5) => observer.callback([
      { target, borderBoxSize: [{ inlineSize: width, blockSize: height }] },
    ]);
    return { el, row, content, handlers, observer, measure };
  }
  return { add, document, controller, lists, views, frames, reports, events, listeners,
    pending(value) { pending = value; },
    frame() { const queued = frames.splice(0); for (const fn of queued) fn(); },
  };
}

test('list feedback reads all ports before writing, coalesces events and drains a budget across frames', () => {
  const f = listFixture(), a = f.add(1), b = f.add(2);
  f.controller.before(); f.controller.after();
  a.measure(); b.measure();
  for (let i = 0; i < 10; i++) a.handlers.get('scroll')();
  f.pending(true);
  expect(f.frames).toHaveLength(1);
  f.frame();
  expect(f.reports).toHaveLength(2);
  expect(f.events.slice(f.events.indexOf('write'))).toEqual(['write', 'write']);
  expect(f.reports[0]).toEqual({ id: 1, geometry: [0, 240, 390, 12, 0, 0], measurements: '101,31.5', limit: 2 });
  expect(f.frames).toHaveLength(1);
  f.pending(false); f.frame();
  expect(f.reports).toHaveLength(4); // Pending bypasses the unchanged-geometry stamp.
  expect(f.frames).toHaveLength(0);
  a.handlers.get('scroll')(); f.frame();
  expect(f.reports).toHaveLength(4);
});

test('list measurements discard retired elements and old widths while retaining focus and pointer pins', () => {
  const f = listFixture(), a = f.add(1);
  a.measure(); f.frame();
  f.document.activeElement = a.row;
  a.handlers.get('pointerdown')({ target: a.row });
  a.handlers.get('focusin')(); f.frame();
  expect(f.reports.at(-1).geometry.slice(4)).toEqual([101, 101]);
  f.listeners.get('pointerup')();
  expect(f.lists.get(a.el).pointer).toBe(101); // The following click still owns its source.
  f.frame(); f.frame();
  expect(f.reports.at(-1).geometry.slice(4)).toEqual([101, 0]);
  a.el.width = 200; f.controller.sync(); f.frame();
  expect(f.reports.at(-1).measurements).toBe('');
  a.measure(); f.frame();
  expect(f.reports.at(-1).measurements).toBe('101,31.5');
  const replacement = { dataset: { view: '101' } };
  a.content.children = [replacement]; f.controller.sync();
  a.measure(a.row, 200, 99); f.frame();
  expect(a.observer.watched.has(a.row)).toBe(false);
  expect(f.reports.at(-1).measurements).toBe('');
  a.measure(replacement, 200, 52); f.frame();
  expect(f.reports.at(-1).measurements).toBe('101,52');
  const count = f.reports.length;
  f.controller.forget(a.el); f.lists.delete(a.el); f.views.delete(1);
  a.measure(replacement, 200, 80); f.frame();
  expect(a.observer.watched.size).toBe(0);
  expect(f.reports).toHaveLength(count);
});

test('a virtualized list takes logical selection but never windowed feedback', () => {
  const f = listFixture(), el = { isConnected: true, firstElementChild: { children: [] }, addEventListener() { throw new Error('navigation.js owns its events'); } };
  f.lists.set(el, { id: 3, measured: false, window: false }); f.views.set(3, el);
  f.controller.sync(); f.controller.before(); f.controller.after(); f.frame();
  expect(f.reports).toHaveLength(0);
  expect(f.lists.get(el).observer).toBeUndefined();
  expect(f.listeners.has('copy')).toBe(true);
});

test("a drive's storage is only a scratch store it names, apart from the app's own", async () => {
  expect(storageKey('com.example.app', 'http://127.0.0.1:1/')).toBe('com.example.app');
  expect(storageKey('com.example.app', 'http://127.0.0.1:1/?agent=1')).toBeNull();
  expect(storageKey('com.example.app', 'http://127.0.0.1:1/?agent=1&storage=run-2.a')).toBe('com.example.app/agent/run-2.a');
  expect(storageKey('com.example.app', 'http://127.0.0.1:1/?storage=x')).toBe('com.example.app'); // only a drive's is scratch
  for (const name of ['', '.', '..', 'a/b', '%2e%2e']) expect(() => storageKey('com.example.app', `http://127.0.0.1:1/?agent=1&storage=${name}`)).toThrow('storage: one name');
  for (const host of ['web', 'linux']) await expect(open({ host, storage: '../x' })).rejects.toThrow('--storage: one name');
});
