// Authored expects over a tree (agent-test.mjs): a node the tree carries but the
// shipped app does not show — a header node the iOS navigation bar leaves out,
// which the agent's own chrome still paints — fails, and says why (LLP 1116 D2).
import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { backendFetchPrefixes, churnLines, disabledExpectation, nativeFetchOrigins, NO_FAILURES, perfExpectation, propertyHolds, testMutationClock, refusalExpectation, refuseDisabled, runTests, serverRequest, testScope, textExpectation, treeExpectation, unarranged } from './agent-test.mjs';
import { axFindings } from './agent-ax.mjs';
import { withEpoch } from './agent-launch.mjs';
import { serveBuildTree } from '../host/web/serve.mjs';

test('always predicates preserve JSON types, option kinds and the previous state', () => {
  const state = { slots: { n: 2, flag: false, absent: null, answer: { ok: true }, empty: [], text: '2' }, derives: { doubled: 4 } };
  const check = (name, test, rest = {}, previous) => propertyHolds({ name, test, ...rest }, state, previous);
  expect(check('n', 'whole')).toBe(true);
  expect(check('text', 'whole')).toBe(false);
  expect(check('doubled', 'at-least', { number: 4 })).toBe(true);
  expect(check('doubled', 'at-most', { number: 3 })).toBe(false);
  expect(check('text', 'at-least', { number: 1 })).toBe(false);
  expect(check('n', 'in', { values: [{ kind: 'number', value: 2 }] })).toBe(true);
  expect(check('text', 'in', { values: [{ kind: 'number', value: 2 }] })).toBe(false);
  expect(check('absent', 'in', { values: [{ kind: 'none' }] })).toBe(true);
  expect(check('answer', 'in', { values: [{ kind: 'some' }] })).toBe(true);
  expect(check('absent', 'in', { values: [{ kind: 'some' }] })).toBe(false);
  expect(check('empty', 'equal', { value: { kind: 'empty-list' } })).toBe(true);
  expect(check('missing', 'in', { values: [{ kind: 'some' }] })).toBe(false);
  expect(check('flag', 'stays', { value: { kind: 'bool', value: true } }, { slots: { flag: true } })).toBe(false);
  expect(check('flag', 'stays', { value: { kind: 'bool', value: true } }, { slots: { flag: false } })).toBe(true);
  expect(check('flag', 'stays', { value: { kind: 'bool', value: true } })).toBe(true);
});

test('device properties see boot, typing and clock changes, and reset stays between tests', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-properties-')), contract = join(dir, 'app.contract'), file = join(dir, 'app.test.contract');
  const plan = join(dir, 'app.plan'), dist = join(dir, 'dist');
  writeFileSync(contract, `component App
  state ready = true
  state sessionReady = false
  action enable
    sessionReady = true
  state textValue = ""
  state n = 0
  derive doubled = n * 2
  task tick mount
    every(100, step)
  action step
    n = n + 1
  action toggle
    ready = not ready
  action edit(v: string)
    textValue = v
  view
    column
      button testId="toggle" press=toggle
        text "Toggle"
      input testId="field" value=textValue input=edit
      button "Enable" testId="enable" press=enable
`);
  writeFileSync(file, `always "ready stays"
  expect state ready stays true
always "session readiness survives reload"
  expect state sessionReady stays true
always "short input"
  expect state textValue in "" "ok"
always "small derive"
  expect state doubled <= 1
after "toggle" "unchecked"
  writes none
  command focus "field"
never refuses "toggle"
test "the three observations"
  tap "toggle"
  type "field" "bad"
  clock +100
test "a fresh history"
  type "field" "ok"
test "reload is observed"
  tap "enable"
  reload
`);
  const root = new URL('..', import.meta.url).pathname;
  const compiler = join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', 'contract');
  const compile = spawnSync(compiler, ['build', contract, '-o', plan], { cwd: root, encoding: 'utf8' });
  expect(compile.status, compile.stderr).toBe(0);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: root, encoding: 'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const server = createServer((req, res) => serveBuildTree(dist, req, res));
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const r = await runTests({ host: 'web', file, app: 'caltrain', url: `http://127.0.0.1:${server.address().port}/` });
    expect([r.passed, r.failed]).toEqual([1, 2]);
    const failures = r.results[0].failures.join('\n');
    expect(failures).toContain('PROPERTY "ready stays" is false on web');
    expect(failures).toContain('at step 1: tap "toggle"');
    expect(failures).toContain('before web: slot ready true');
    expect(failures).toContain('PROPERTY "short input" is false on web');
    expect(failures).toContain('at step 3: clock +100');
    expect(failures).toContain('web: derive doubled 2');
    expect(failures).not.toContain('PROPERTY "unchecked"');
    expect(r.results[1].failures).toEqual([]);
    expect(r.results[2].failures.join('\n')).toContain('at step 2: reload');
    expect(r.results[2].failures.join('\n')).toContain('PROPERTY "session readiness survives reload" is false on web');
    writeFileSync(file, `always "false at boot"
  expect state n == 1
test "boot only"
  expect state n == 0
`);
    const boot = await runTests({ host: 'web', file, app: 'caltrain', url: `http://127.0.0.1:${server.address().port}/` });
    expect(boot.failed).toBe(1);
    expect(boot.results[0].failures.join('\n')).toContain('at step 0: boot');
  } finally { server.close(); rmSync(dir, { recursive: true, force: true }); }
}, 180_000);

test('a presented popover belongs to its AppKit modal for outside-modal findings', () => {
  const elements = [
    { i: 0, parent: null, id: null, via: 'none', role: 'scrollarea', name: '' },
    { i: 1, parent: 0, id: 10, via: 'self', role: 'dialog', name: 'Setup' },
    { i: 2, parent: 1, id: 11, via: 'self', role: 'button', name: 'Options' },
    { i: 3, parent: 0, id: 12, via: 'self', role: 'group', name: 'Options' },
    { i: 4, parent: 3, id: 13, via: 'self', role: 'textbox', name: 'Filter' },
  ];
  const modal = { present: true, by: 'aria-modal', element: 1, presented: [3] };
  expect(axFindings({ ax: { source: 'appkit', modal, elements } }).filter(f => f.kind === 'outside-modal')).toEqual([]);
  expect(axFindings({ ax: { source: 'appkit', modal: { ...modal, presented: [] }, elements } }).filter(f => f.kind === 'outside-modal').map(f => f.id)).toEqual([12, 13]);
});

const why = 'the iOS navigation bar does not show this node; in the app it is not visible';
const nodes = [
  { id: 3, depth: 0, type: 'View', props: {} },
  { id: 4, depth: 1, type: 'Text', props: { text: 'Shopping List' } },
  { id: 5, depth: 1, type: 'Text', props: { text: '2 items left', testId: 'count' } },
  { id: 6, depth: 1, type: 'Text', props: { text: 'Shared', testId: 'tagline' }, unshown: why },
];

test('a text the bar shows passes; one it leaves out fails naming why', () => {
  expect(textExpectation(nodes, 'count', '2 items left')).toBeNull();
  expect(textExpectation(nodes, 'count', '1 item left').unshown).toBeUndefined();
  const f = textExpectation(nodes, 'tagline', 'Shared');
  expect(f.unshown).toBe(true);
  expect(f.message).toBe(`text of "tagline" is "Shared", but ${why}`);
  // A wrong text is the ordinary failure, which a clock step may still settle.
  expect(textExpectation(nodes, 'tagline', 'Other').unshown).toBeUndefined();
});

test('present means shown in the app; missing is the tree\'s', () => {
  expect(treeExpectation(nodes, 'count', true)).toBeNull();
  expect(treeExpectation(nodes, 'tagline', true)).toEqual({ message: `testId "tagline" is in the tree, but ${why}`, unshown: true });
  expect(treeExpectation(nodes, 'gone', true).message).toBe('expected testId "gone" present, it was absent');
  expect(treeExpectation(nodes, 'gone', false)).toBeNull();
  expect(treeExpectation(nodes, 'tagline', false).message).toBe('expected testId "tagline" absent, it was present');
});

// LLP 1116.010 D3 (chat-rooms and password-generator, round 12): a disabled control is asserted, and pressing one fails the step.
test('expect disabled and enabled read the prop every host\'s tree carries; a press is refused while context eligibility follows the element type', async () => {
  const tree = [{ id: 1, type: 'Pressable', props: { testId: 'send', disabled: true } }, { id: 2, props: { testId: 'arm' } }, { id: 3, props: { testId: 'native', disabled: 'true' } }, { id: 4, props: { testId: 'update', accessibilityDisabled: true } },
    { id: 5, type: 'View', props: { disabled: true } }, { id: 6, type: 'Pressable', props: { disabled: true, href: '/help' } },
    { id: 7, type: 'TextInput', props: { disabled: true } }, { id: 8, type: 'Control', props: { disabled: true } }, { id: 9, type: 'Pressable', props: { disabled: true, href: '' } }];
  expect(disabledExpectation(tree, 'send', true)).toBeNull();
  expect(disabledExpectation(tree, 'native', true)).toBeNull();
  // `aria-disabled` reads as disabled for the expectation…
  expect(disabledExpectation(tree, 'update', true)).toBeNull();
  expect(disabledExpectation(tree, 'update', false).message).toBe('expected "update" enabled, it was disabled');
  expect(disabledExpectation(tree, 'arm', false)).toBeNull();
  expect(disabledExpectation(tree, 'send', false).message).toBe('expected "send" enabled, it was disabled');
  expect(disabledExpectation(tree, 'arm', true).message).toBe('expected "arm" disabled, it was enabled');
  expect(disabledExpectation(tree, 'gone', true).message).toBe('expected "gone" disabled, no view carries that testId');
  const s = { op: async (req) => ({ nodes: tree.filter((n) => n.id === req.target) }) };
  await expect(refuseDisabled(s, { id: 1 }, 'send', {})).rejects.toThrow('send is disabled: a press does nothing');
  await expect(refuseDisabled(s, { id: 3 }, 'native', { dblclick: true })).rejects.toThrow('native is disabled');
  await refuseDisabled(s, { id: 2 }, 'arm', {});
  // …but its press still dispatches, so the tap is not refused.
  await refuseDisabled(s, { id: 4 }, 'update', {});
  await refuseDisabled(s, { id: 1 }, 'send', { hover: true });
  for (const id of [5, 6]) await refuseDisabled(s, { id }, String(id), { contextmenu: true });
  for (const id of [1, 7, 8, 9]) await expect(refuseDisabled(s, { id }, String(id), { contextmenu: true })).rejects.toThrow('is disabled');
  await expect(refuseDisabled(s, { id: 5 }, 'container', { contextmenu: false })).rejects.toThrow('container is disabled');
  await refuseDisabled(s, { id: 1, entity: 'unit' }, 'send', {});
});

test('an active screen\'s node is read before a covered one\'s', () => {
  const covered = [{ id: 9, depth: 0, type: 'Text', props: { text: 'x', testId: 'count' }, inactive: true, unshown: why }, ...nodes];
  expect(textExpectation(covered, 'count', '2 items left')).toBeNull();
});

// LLP 1116.006 D8: a test's `request` line is the driver calling the app's server, the persona's name its bearer token.
test('a request line sends the method, the JSON body and the persona\'s token under the server URL; a refusal says its status and body', async () => {
  const seen = [];
  const server = Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    const url = new URL(req.url);
    seen.push([req.method, url.pathname, req.headers.get('authorization'), req.headers.get('content-type'), await req.text()]);
    return url.pathname === '/refused' ? Response.json({ error: 'Sign in again.' }, { status: 401 }) : url.pathname === '/gone' ? new Response(null, { status: 404 }) : Response.json({ ok: true }, { status: 201 });
  } });
  const base = `http://127.0.0.1:${server.port}`;
  try {
    await serverRequest(base, { method: 'POST', path: '/polls/1/votes', body: '{"option":2}', as: 'bob' });
    await serverRequest(base, { method: 'GET', path: `${base}/health`, body: null, as: null });
    expect(seen).toEqual([['POST', '/polls/1/votes', 'Bearer bob', 'application/json', '{"option":2}'], ['GET', '/health', null, null, '']]);
    await expect(serverRequest(base, { method: 'PATCH', path: '/refused', body: '{}', as: 'mallory' })).rejects.toThrow(`request PATCH ${base}/refused as mallory: the server answered 401 {"error":"Sign in again."}`);
    await expect(serverRequest(base, { method: 'DELETE', path: '/gone', body: null, as: 'bob' })).rejects.toThrow(`request DELETE ${base}/gone as bob: the server answered 404`);
    await expect(serverRequest(null, { method: 'GET', path: '/items', body: null, as: null })).rejects.toThrow('app.ts calls no local server');
    // LLP 1116.010 D3 (polls, round 12): `expect <status>` passes on that status and fails on any other, a success included.
    await serverRequest(base, { method: 'DELETE', path: '/gone', body: null, as: 'bob', expect: 404 });
    await serverRequest(base, { method: 'PATCH', path: '/refused', body: '{}', as: 'mallory', expect: 401 });
    await expect(serverRequest(base, { method: 'DELETE', path: '/gone', body: null, as: 'bob', expect: 409 })).rejects.toThrow(`request DELETE ${base}/gone as bob: the server answered 404, expected 409`);
    await expect(serverRequest(base, { method: 'POST', path: '/items', body: '{}', as: 'bob', expect: 409 })).rejects.toThrow('the server answered 201 {"ok":true}, expected 409');
  } finally { await server.stop(true); }
  await expect(serverRequest(base, { method: 'GET', path: '/items', body: null, as: null })).rejects.toThrow(/nothing answered/);
});

// LLP 1116.009 D1: a `reload` relaunches at the virtual time the test had reached (the date it launched at plus every
// `clock +N`), not at its launch date: a phone restarted at 9:05 does not wake at 9:00 (habit-tracker, rounds 10 and 11).
test('the page address a reload loads takes the date it relaunches at, and keeps the rest', () => {
  const href = 'http://127.0.0.1:9/detail/3?agent=1&seed=1&epoch=1773489600000&timeZone=UTC&q=a';
  expect(new URL(withEpoch(href, 1773576000000)).searchParams.get('epoch')).toBe('1773576000000');
  expect(new URL(withEpoch(href, 1773576000000)).searchParams.get('q')).toBe('a');
  expect(new URL(withEpoch('http://127.0.0.1:9/', 5)).searchParams.get('epoch')).toBe('5');
  expect(withEpoch(href, undefined)).toBe(href);
});

test('a reload keeps the clock the test had reached, again and again, and the run ends with it', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-reload-clock-')), contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist'), file = join(dir, 'app.test.contract');
  writeFileSync(contract, `shape Time
  epochAtZero: number
  utcOffset: number

component Day
  state now = 0
  resource time = exactTime() as shape Time
  derive day = formatDate(time.epochAtZero + now, time.utcOffset, "iso")
  task tick mount
    every(60000, step)
  action step
    now = performanceNow()
  view
    column testId="root"
      text day testId="day"
`);
  writeFileSync(file, `test "a relaunch is on the day the test reached"
  epoch "2026-03-14T12:00:00Z"
  expect text "day" == "2026-03-14"
  clock +86400000
  clock data
  expect text "day" == "2026-03-15"
  reload
  expect text "day" == "2026-03-15"
  clock +86400000
  clock data
  expect text "day" == "2026-03-16"
  reload
  expect text "day" == "2026-03-16"
`);
  const root = new URL('..', import.meta.url).pathname;
  const compiler = join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', process.platform === 'win32' ? 'contract.exe' : 'contract');
  const compile = spawnSync(compiler, ['build', contract, '-o', plan], { cwd: root, encoding: 'utf8' });
  expect(compile.status, compile.stderr).toBe(0);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: root, encoding: 'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const ends = [];
  try {
    const r = await runTests({ host: 'web', file, app: 'caltrain', url: `http://127.0.0.1:${server.address().port}/`, size: [420, 900], atEnd: async (s, t) => { ends.push([t.launch.epoch, t.reached]); return {}; } });
    expect(r.results.map((x) => x.failures)).toEqual([[]]);
    expect(r.passed).toBe(1);
    // verify's relaunch starts where the test left off too: the test names the date it launched at and the date it had reached.
    expect(ends).toEqual([[Date.UTC(2026, 2, 14, 12), Date.UTC(2026, 2, 16, 12)]]);
  } finally {
    server.close();
    rmSync(dir, { recursive: true, force: true });
  }
}, 180_000);


test('expect refusal asserts an observed mutation failure and preserves unrelated wiring findings', async () => {
  const {refusalsOf} = await import('./verify.mjs');
  const lines=[
    't=1 refused reply saved; wall 3 ms: Not permitted',
    't=1 request 4 (saved) failed and is no longer pending: it ends unsent: Not permitted',
    't=2 refused reply saved; wall 2 ms: outside its shape',
    't=2 request 5 (saved) failed and is no longer pending: it is not retried: outside its shape',
    't=3 refused reply other; wall 1 ms: Not permitted',
    't=3 request 6 (other) failed and is no longer pending: it ends unsent: Not permitted',
    't=4 showModal absent refused: no dialog has that id',
    't=5 request 7 (resource) failed and is no longer pending: it keeps its last value: Offline',
  ];
  const first=refusalExpectation({lines:lines.slice(0,2)},'saved');
  expect(first).toEqual({expected:[lines[1],lines[0]]});
  expect(refusalsOf({lines},first.expected)).toEqual([
    'refused reply saved; wall 2 ms: outside its shape',
    'refused reply other; wall 1 ms: Not permitted',
    'showModal absent refused: no dialog has that id',
  ]);
  expect(refusalExpectation({lines},'resource').message).toContain('no refused reply was observed');
  expect(refusalExpectation({lines:[]},'saved').message).toContain('clock data');
  // The same assertion cannot consume a refusal before the preceding assertion of the same slot's log cursor.
  expect(refusalExpectation({lines:lines.slice(2)},'saved').expected).toEqual([lines[3],lines[2]]);
  const synchronous='t=6 send saved failed: Refused on purpose; it ends unsent';
  const sync=refusalExpectation({lines:[...lines,synchronous]},'saved');
  expect(sync).toEqual({expected:[synchronous]});
  expect(refusalsOf({lines:[synchronous]},sync.expected)).toEqual([]);
  const repeated=[...lines.slice(0,2),...lines.slice(0,2)];
  expect(refusalsOf({lines:repeated},first.expected)).toEqual(['refused reply saved; wall 3 ms: Not permitted']);
  const mirror='exact: refused reply saved; wall 3 ms: Not permitted';
  expect(refusalsOf({lines:lines.slice(0,2),host:[mirror]},first.expected)).toEqual([]);
  expect(refusalsOf({lines:lines.slice(0,2),host:[mirror,mirror]},first.expected)).toEqual(['refused reply saved; wall 3 ms: Not permitted']);
  expect(refusalsOf({lines:repeated,host:[mirror,mirror,mirror]},[...first.expected,...first.expected])).toEqual(['refused reply saved; wall 3 ms: Not permitted']);
});


test('an authored expected refusal keeps a prior send value, skips then, and is accepted by verify', async () => {
  const {refusalsOf} = await import('./verify.mjs');
  const dir=mkdtempSync(join(tmpdir(),'exact-expected-refusal-')), dist=join(dir,'dist'), file=join(dir,'app.test.contract');
  writeFileSync(join(dir,'app.json'),JSON.stringify({name:'Refusal',app:{id:'test.exact.refusal',name:'Refusal'},host:{},deploy:{}}));
  writeFileSync(join(dir,'app.contract'),`shape Saved
  value: string
component App
  state completions = 0
  mutation saved as shape Saved then completed
  derive stored = match saved { case some(value) => value.value, case none => "empty" }
  derive error = match failure(saved) { case some(value) => value.code, case none => "none" }
  action completed
    completions = completions + 1
  action accept
    send saved = write(false)
  action refuse
    send saved = write(true)
  action refuseNow
    send saved = reject()
  view
    column
      button "Accept" press=accept testId="accept"
      button "Refuse" press=refuse testId="refuse"
      button "Refuse immediately" press=refuseNow testId="refuse-now"
      text stored testId="stored"
      text error testId="error"
`);
  writeFileSync(join(dir,'app.ts'),`import type {Answer} from './app.contract.d.ts';
export const appId='test.exact.refusal';
export const grants='';
export const answer: Answer=(_source,args)=>{
  const refused=()=>Object.assign(new Error('Refused on purpose'),{reason:'DENIED'});
  if(_source==='reject') throw refused();
  return Promise.resolve().then(()=>{
    if(args[0]) throw refused();
    return {value:'kept'};
  });
};
`);
  writeFileSync(file,`test "a refused send keeps the previous reply"
  tap "accept"
  clock data
  expect text "stored" == "kept"
  expect state completions == 1
  tap "refuse"
  clock data
  expect refusal "saved"
  expect text "stored" == "kept"
  expect text "error" == "DENIED"
  expect state completions == 1

test "an expected refusal before reload does not accept the next journal"
  tap "refuse-now"
  clock data
  expect refusal "saved"
  reload
  tap "refuse-now"
  clock data
  expect text "error" == "DENIED"
`);
  const root=new URL('..',import.meta.url).pathname;
  const build=spawnSync(process.execPath,['host/web-js/build.mjs','refusal','--out',dist,'--render','none'],{cwd:root,env:{...process.env,EXACT_APP_DIR:dir},encoding:'utf8'});
  expect(build.status,build.stderr).toBe(0);
  const server=createServer((req,res)=>serveBuildTree(dist,req,res));
  await new Promise(ok=>server.listen(0,'127.0.0.1',ok));
  const findings=[], observed=[];
  try {
    const r=await runTests({host:'web',file,app:'caltrain',url:`http://127.0.0.1:${server.address().port}/`,atEnd:async(s,t)=>{
      const logs=await s.logs();
      observed.push(refusalsOf(logs).length);
      findings.push({expected:t.expectedRefusals,refused:refusalsOf(logs,t.expectedRefusals),lines:logs.lines});
      return {};
    }});
    expect(r.results.map(t=>t.failures)).toEqual([[],[]]);
    expect(r.passed).toBe(2);
    expect(observed).toHaveLength(2);
    expect(observed.every(count=>count>0)).toBe(true);
    expect(r.results.map(t=>t.end?.error)).toEqual([undefined,undefined]);
    expect(findings[0].refused).toEqual([]);
    expect(findings[1].expected).toEqual([]);
    expect(findings[1].refused).toEqual(['send saved failed: Refused on purpose; it ends unsent']);
    expect(findings[1].lines).toContain('t=0 send saved failed: Refused on purpose; it ends unsent');
  } finally { server.close(); rmSync(dir,{recursive:true,force:true}); }
},180_000);

test('authored media preferences apply before observations, override by name, and survive reload', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-test-media-'));
  const contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist'), file = join(dir, 'app.test.contract');
  writeFileSync(contract, `shape Viewport
  prefersColorScheme: string
  prefersReducedMotion: bool
component App
  resource viewport = exactViewport() as shape Viewport
  view
    column
      text viewport.prefersColorScheme testId="scheme"

`);
  writeFileSync(file, `prefer prefers-color-scheme dark
prefer prefers-reduced-motion reduce

test "inherits"
  before data
  expect text "scheme" == "dark"
  expect state viewport.prefersReducedMotion == true
  reload
  expect text "scheme" == "dark"
  prefer prefers-color-scheme light
  expect text "scheme" == "light"
  reload
  expect text "scheme" == "light"
  prefer prefers-color-scheme dark
  expect text "scheme" == "dark"
  prefer prefers-color-scheme light

test "overrides only scheme"
  prefer prefers-color-scheme light
  expect text "scheme" == "light"
  expect state viewport.prefersReducedMotion == true

test "invalid launch value"
  prefer prefers-color-scheme banana
  expect text "scheme" == "dark"

test "invalid launch name"
  prefer unknown dark
  expect text "scheme" == "dark"

test "invalid later value"
  expect text "scheme" == "dark"
  prefer prefers-color-scheme banana

test "page facts are outside the test media grammar"
  prefer online false

test "a failure does not stop the next test"
  expect text "scheme" == "dark"
`);
  const root = new URL('..', import.meta.url).pathname;
  const compile = spawnSync(join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', 'contract'), ['build', contract, '-o', plan], { cwd: root, encoding: 'utf8' });
  expect(compile.status, compile.stderr).toBe(0);
  const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'caltrain', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: root, encoding: 'utf8' });
  expect(build.status, build.stderr).toBe(0);
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const ends = [];
  try {
    const r = await runTests({ host: 'web', file, app: 'caltrain', url: `http://127.0.0.1:${server.address().port}/`, atEnd: async (s, t) => {
      if (t.failures.length) return {};
      const dark = t.launch.media['prefers-color-scheme'] === 'dark';
      expect(await s.carrier.evaluate("matchMedia('(prefers-color-scheme: dark)').matches")).toBe(dark);
      await t.restart();
      expect(textExpectation((await s.tree()).nodes, 'scheme', dark ? 'dark' : 'light')).toBeNull();
      ends.push(t.launch.media);
      return {};
    } });
    expect(r.results.map(t => t.end?.error)).toEqual(Array(7).fill(undefined));
    expect(r.results.map(t => t.failures.length)).toEqual([0, 0, 1, 1, 1, 1, 0]);
    for (const [index, line, name, value] of [[2, 24, 'prefers-color-scheme', 'banana'], [3, 28, 'unknown', 'dark'], [4, 33, 'prefers-color-scheme', 'banana']]) {
      expect(r.results[index].failures[0]).toContain(`line ${line}: prefer: ${name} ${value}: expected`);
    }
    expect(r.results[5].failures[0]).toContain('test prefer: expected a CSS media feature');
    expect(ends).toEqual([
      { 'prefers-color-scheme': 'light', 'prefers-reduced-motion': 'reduce' },
      { 'prefers-color-scheme': 'light', 'prefers-reduced-motion': 'reduce' },
      { 'prefers-color-scheme': 'dark', 'prefers-reduced-motion': 'reduce' },
    ]);
  } finally { server.close(); rmSync(dir, { recursive: true, force: true }); }
}, 180_000);

test('authored wakelock expectations follow the holder record and name holders on failure', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-test-wakelock-'));
  const contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist'), file = join(dir, 'app.test.contract');
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Wake Lock', app: { id: 'test.exact.wakelock', name: 'Wake Lock' }, host: {}, deploy: {} }));
  writeFileSync(contract, `component App
  state on = true
  action toggle
    on = !on
  view
    column wakelock=(on ? "screen" : "none") testId="awake"
      button "Toggle" press=toggle testId="toggle"
`);
  writeFileSync(file, `test "toggle the holder"
  expect wakelock held
  tap "toggle"
  expect wakelock released
  tap "toggle"
  expect wakelock held

test "a holder prevents release"
  expect wakelock released

test "nothing holds after toggling off"
  tap "toggle"
  expect wakelock held
`);
  const root = new URL('..', import.meta.url).pathname;
  const compiler = join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', 'contract');
  try {
    const compile = spawnSync(compiler, ['build', contract, '-o', plan], { cwd: root, encoding: 'utf8' });
    expect(compile.status, compile.stderr).toBe(0);
    const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'wakelock', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, encoding: 'utf8' });
    expect(build.status, build.stderr).toBe(0);
    const server = createServer((request, response) => serveBuildTree(dist, request, response));
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    try {
      const r = await runTests({ host: 'web', file, app: 'caltrain', url: `http://127.0.0.1:${server.address().port}/` });
      expect(r.passed).toBe(1);
      expect(r.results.map(t => t.failures.length)).toEqual([0, 1, 1]);
      expect(r.results[1].failures[0]).toMatch(/line 9: expected wakelock released; held by view \d+ \("awake"\)$/);
      expect(r.results[2].failures[0]).toBe('nothing holds after toggling off: line 13: expected wakelock held; nothing holds it');
    } finally { server.close(); }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 180_000);

// docs/recipes/navigation-stack.md, "A screen open twice" (issues/closed/20261021-repeated-routes-share-root-state.md):
// a route drawn by its own component has `state` per stack entry (LLP 1017.000 P4c), so two entries of one route keep
// separate state, and the entry a pop removed takes its state with it: the next one opens fresh. The two entries have
// different URLs, since a push of the location already on top is no new visit (route/src/router.rs `push`).
test('a screen open twice keeps state per entry, and a popped entry\'s is gone', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-test-twice-'));
  const contract = join(dir, 'app.contract'), plan = join(dir, 'app.plan'), dist = join(dir, 'dist'), file = join(dir, 'app.test.contract');
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Twice', app: { id: 'test.exact.twice', name: 'Twice' }, host: {}, deploy: {} }));
  writeFileSync(contract, `routes nav
  home "/"
    finder "/finder/:from"

component App
  action open(from: string)
    nav = push(nav, path("finder", from))
  action back
    nav = back(nav)
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        if e.name == "home"
          column navigationKey=\`\${e.id}\` position="absolute" inset=0
            button press=open("home") testId="open"
              text "Find a shoe"
        if e.name == "finder"
          Finder(entry=e, open=open)

component Finder
  props
    entry: Entry
    open: action
  state pick = ""
  action choose(v: string)
    pick = v
  view
    column navigationKey=\`\${entry.id}\` position="absolute" inset=0
      header
        text "Finder" role="heading" aria-level=2
      text pick testId=\`pick-\${entry.id}\`
      button press=choose("trail") testId=\`trail-\${entry.id}\`
        text "Trail"
      button press=open(\`\${entry.id}\`) testId=\`again-\${entry.id}\`
        text "Another"
`);
  writeFileSync(file, `test "two entries keep their own state; a popped one's goes with it"
  tap "open"
  clock settle
  tap "trail-1"
  expect text "pick-1" == "trail"
  tap "again-1"
  clock settle
  expect text "pick-2" == ""
  tap "trail-2"
  expect text "pick-2" == "trail"
  back
  clock settle
  expect tree missing "pick-2"
  expect text "pick-1" == "trail"
  tap "again-1"
  clock settle
  expect text "pick-3" == ""
`);
  const root = new URL('..', import.meta.url).pathname;
  const compiler = join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', 'contract');
  try {
    const compile = spawnSync(compiler, ['build', contract, '-o', plan], { cwd: root, encoding: 'utf8' });
    expect(compile.status, compile.stderr).toBe(0);
    const build = spawnSync(process.execPath, ['host/web-js/build.mjs', 'twice', '--plan', plan, '--out', dist, '--render', 'none'], { cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, encoding: 'utf8' });
    expect(build.status, build.stderr).toBe(0);
    const server = createServer((request, response) => serveBuildTree(dist, request, response));
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    try {
      const r = await runTests({ host: 'web', file, app: 'caltrain', url: `http://127.0.0.1:${server.address().port}/` });
      expect(r.results.map(t => t.failures)).toEqual([[]]);
    } finally { server.close(); }
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 180_000);

// LLP 1146 D4: what failed that the test did not arrange.
test('a failure is unarranged unless a fetch fault fired or an expect refusal names it', () => {
  const now = { ...NO_FAILURES, failures: { gateway: 2 }, failed: { gateway: 'turn 868 ms over 100 ms' }, mutations: { vote: { code: 'refused', message: 'no' }, send: { code: 'network', message: 'down' } } };
  expect(unarranged(NO_FAILURES, now, { refusals: ['vote'] })).toEqual(['resource gateway was refused 2 times (turn 868 ms over 100 ms)', 'send send failed (network: down)']);
  expect(unarranged(now, { ...now, failures: { gateway: 3 } }, { refusals: ['vote', 'send'] })).toEqual(['resource gateway was refused once (turn 868 ms over 100 ms)']);
  expect(unarranged(NO_FAILURES, { ...now, faults: { '/api': 1 } }, { refusals: ['vote', 'send'] })).toEqual([]);
  expect(unarranged(NO_FAILURES, { ...NO_FAILURES, failed: { list: 'shape' } })).toEqual(['resource list failed (shape)']);
  const churn = churnLines(['t=12 query r: src', 't=13 `r` was asked 50 times in a second, its arguments new each time (argument 1 last): …']);
  expect(churn).toEqual(['`r` was asked 50 times in a second, its arguments new each time (argument 1 last): …']);
  expect(unarranged(NO_FAILURES, NO_FAILURES, { churn })).toEqual(churn);
});

// LLP 1146 D3: an `expect perf` line against a block's counts.
test('expect perf reads a resource\'s asks and a component\'s or a view\'s work', () => {
  const site = (component, created, evaluated, line) => ({ site: line, created, rowConstructed: 0, evaluated, source: { status: 'compatible', component, file: '/a/app.contract', line } });
  const measured = { line: 3, components: new Set(['MemberRow', 'Rail', 'Sheet']), whole: { resources: [{ name: 'channelList', asked: 1 }], sites: [site('Rail', 2, 9, 10), site('Rail', 1, 4, 12)] }, under: new Map([['member-sheet', { sites: [site('Sheet', 5, 0, 40)] }]]) };
  const at = (subject, target, compare, value) => perfExpectation(measured, { subject, target, compare, value }, ['channelList', 'serverList']);
  expect(at('query', 'channelList', '<=', 1)).toBeNull();
  expect(at('query', 'serverList', '==', 0)).toBeNull();
  expect(at('query', 'nope', '==', 0)).toContain('no resource is named nope');
  expect(at('built', 'Rail', '==', 3)).toBeNull();
  expect(at('evaluated', 'Rail', '<=', 12)).toBe('evaluated "Rail" counted 13 in the `perf` block at line 3, expected <= 12 (app.contract:10 9, app.contract:12 4)');
  expect(at('built', 'MemberRow', '==', 0)).toBeNull(); // declared, never built
  expect(at('built', 'member-sheet', '==', 5)).toBeNull(); // a view by testId
  expect(at('built', 'Ghost', '==', 0)).toContain('names no component');
  expect(perfExpectation(null, { subject: 'query', target: 'a', compare: '==', value: 0 })).toContain('no `perf` block');
  expect(perfExpectation({ ...measured, whole: { sites: [] } }, { subject: 'query', target: 'a', compare: '==', value: 0 })).toContain('counts no resource asks');
});

test('a test scoped on macos runs under the mac alias and the sample host too', () => {
  expect(testScope('mac')).toBe('macos');
  expect(testScope('macos')).toBe('macos');
  expect(testScope('host')).toBe('macos');
  expect(testScope('host-ios')).toBe('ios');
  expect(testScope('web')).toBe('web');
});

test('always observes each mutation-clock configuration, including the sixteenth', async () => {
  let requests = 1, ticks = 0;
  const seen = [], clock = testMutationClock();
  const session = {
    state: async () => ({ mutationRequests: requests, slots: { ticks } }),
    clock: async () => { ticks++; if (ticks < 16) requests++; },
  };
  expect(await clock.advance(session, async state => { seen.push(state.slots.ticks); })).toBe(16);
  expect(seen).toEqual(Array.from({ length: 17 }, (_, i) => i));
});

// LLP 1153 D4: a native run has no web build's grants to read. The site pattern's API is a setting, so its `/planets`
// fault resolves against the grant app.ts names as the build evaluates it, `.env.local`'s port over `.env`'s.
test('a native run resolves a fetch fault under the API a setting names', async () => {
  const { createApp, applyPattern } = await import('../game/new.mjs');
  const parent = mkdtempSync(join(tmpdir(), 'exact-native-origins-'));
  try {
    const dir = join(parent, 'site'), env = { EXACT_BUN_LOADED: '' };
    createApp(dir);
    applyPattern(dir, 'site');
    expect(nativeFetchOrigins(dir, env)).toEqual(['http://127.0.0.1:8787']);
    writeFileSync(join(dir, '.env.local'), 'API_BASE_URL=http://127.0.0.1:9123\n');
    const faults = () => [{ steps: [{ op: 'fail-fetch', prefix: '/planets' }, { op: 'pass-fetch', prefix: '/planets' }] }];
    expect(backendFetchPrefixes(faults(), nativeFetchOrigins(dir, env))[0].steps.map((s) => s.prefix)).toEqual(['http://127.0.0.1:9123/planets', 'http://127.0.0.1:9123/planets']);
    expect(nativeFetchOrigins(dir, { ...env, API_BASE_URL: 'http://127.0.0.1:7000' })).toEqual(['http://127.0.0.1:7000']);
    // The backend a build selected, and a scaffold's literal origin, still come first.
    expect(nativeFetchOrigins(dir, { ...env, EXACT_BACKEND_ORIGIN: 'http://127.0.0.1:4411' })).toEqual(['http://127.0.0.1:4411']);
    writeFileSync(join(dir, 'app.ts'), "const origin = 'http://127.0.0.1:4410';\n");
    expect(nativeFetchOrigins(dir, env)).toEqual(['http://127.0.0.1:4410']);
  } finally { rmSync(parent, { recursive: true, force: true }); }
});

// LLP 1103: a fault on a path is the API's. The driver arms it under the one origin the grants name, and the host
// matches that as a prefix of the whole URL, so `fail fetch "/planets"` fails the site's absolute fetch of a planet.
test('a fault on a path is armed under the API, and fails that API\'s absolute URL', async () => {
  const { faultOp, takeFault } = await import('../host/web/faults.js');
  const [planets] = backendFetchPrefixes([{ steps: [{ op: 'fail-fetch', prefix: '/planets' }] }], ['http://127.0.0.1:4790']);
  expect(planets.steps[0].prefix).toBe('http://127.0.0.1:4790/planets');
  try {
    faultOp({ fail: planets.steps[0].prefix });
    expect(takeFault('http://127.0.0.1:4790/planets/5')).toBe(true);
    expect(takeFault('http://127.0.0.1:4791/planets/5')).toBe(false);
  } finally { delete globalThis.__exactFaults; }
});
