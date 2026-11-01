// `state <name>` (agent-inspect.mjs `stateOp`, LLP 1116.007 D3.9): a state, derive or resource (or a field of the record)
// reads by name, down a dotted path, as `expect state` does, even where a view carries the name; a plain view's is the record.
import { test, expect } from 'bun:test';
import { render, renderTrace, renderFrameHealthReport, renderFrameHealthMetrics, perfOp, readTrack, renderPerf, stateOp, tapRefusal, nextCommitTasks, settleBound, planLocators, perfDelta } from './agent-inspect.mjs';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

test('bounded frame report and capture controls bypass legacy perf collection', async () => {
  const asked = [];
  const s = { op: async request => { asked.push(request); return { capture: true }; }, perf: () => { throw new Error('legacy sampler must stay off'); } };
  await perfOp(s, ['frames', 'report']);
  await perfOp(s, ['frames', 'capture', 'start', '120000']);
  await perfOp(s, ['frames', 'capture', 'stop']);
  expect(asked).toEqual([{ op: 'perf', frames: true, report: true }, { op: 'perf', frames: true, capture: 'start', durationMs: 120000 }, { op: 'perf', frames: true, capture: 'stop' }]);
  for (const args of [['frames', 'report', 'all'], ['frames', 'capture', 'start', '120001'], ['frames', 'capture', 'stop', '5'], ['frames', 'capture', 'start', 'NaN']]) {
    await expect(perfOp(s, args)).rejects.toThrow();
  }
});

test('health windows use finalized cursors and preserve the source without starting legacy collection', async () => {
  const requests = [], operations = [], cursor = { version: 1 };
  const s = { op: async request => { requests.push(request); return { cursor }; }, perf: () => { throw new Error('legacy sampler'); } };
  await perfOp(s, ['frames', 'health', 'scroll', 'during', 'tap list'], 'perf frames health scroll during "tap list"', async op => { operations.push(op); });
  expect(operations).toEqual(['tap list']);
  expect(requests).toEqual([{ op: 'perf', frames: true, health: true, source: 'scroll' }, { op: 'perf', frames: true, health: true, source: 'scroll', cursor }]);
  const stale = renderFrameHealthMetrics({ source: 'app', status: 'stale', observationAgeNs: '2000000000' }).split('\n')[0];
  expect(stale).toBe('frame health · app · stale · observation age 2000.0 ms · unqualified');
  expect(renderFrameHealthMetrics({ source: 'app', status: 'inactive', observationAgeNs: null })).toContain('observation age unknown');
  const text = renderFrameHealthMetrics({ source: 'app', status: 'observing', frames: { callbacks: '3', held: '1', intervals: '1', perSecond: 120,
    missedFrames: '1', requestedSlots: '3', p99BoundsNs: [8000000, 8500000] }, work: { records: '1', late: null, latenessUnknown: 'simulator-deadlines', pending: 1, overdue: null } });
  expect(text).toContain('late unknown (simulator-deadlines) · in flight 1, overdue unknown');
  expect(text).toContain('held 1 · frame intervals 1 · 120.00 FPS · missed 1 of 3 requested slots');
  expect(text).toContain('p99 8.00–8.50 ms');
  expect(text).toContain('Core Animation commits, input latency and GPU execution remain unmeasured');
  expect(text).toContain('assessments unavailable');
  const assessed = renderFrameHealthMetrics({ source: 'app', assessments: { policy: 'exact-frame-health-policy/2',
    cadence: { result: 'fail', reason: 'skipped-refreshes', evidence: 'callback-and-owner-work-proxies', predicates: { frameIntervals: '240', skippedRefreshes: '3', lateFrames: '1', overdueFrames: '0', unclassifiedIntervals: '0' } },
    target: { result: 'restricted', reason: 'unknown', evidence: 'callback-proxy', predicates: { requestedSlots: '480', missedFrames: '240', restrictedIntervals: '12' }, context: ['simulator'] },
    contentLatency: { result: 'insufficient-evidence', reason: 'content-lateness-unobserved', evidence: 'unavailable' } } }).split('\n');
  expect(assessed.slice(1, 4)).toEqual([
    'cadence fail · skipped-refreshes (callback-and-owner-work-proxies evidence) · frame intervals 240, skipped refreshes 3 · late frames 1, overdue 0 · across schedule changes 0',
    'target rate restricted · unknown (callback-proxy evidence) · requested slots 480, missed 240 · restricted intervals 12 · at read: simulator',
    'content latency insufficient-evidence · content-lateness-unobserved (unavailable evidence)']);
  requests.length = 0;
  const unobserved = { op: async request => {
    requests.push(request);
    if (requests.length === 1) { throw Object.assign(new Error('unobserved'), { reply: { error: 'native-scroll-unobserved' } }); }
    return { cursor };
  } };
  await perfOp(unobserved, ['frames', 'health', 'scroll', 'during'], 'perf frames health scroll during "tap list"', async () => {});
  expect(requests).toEqual(Array(2).fill({ op: 'perf', frames: true, health: true, source: 'scroll' }));
  requests.length = 0;
  await expect(perfOp(unobserved, ['frames', 'health', 'scroll'])).rejects.toThrow('unobserved');
});

test('Apple live windows use a host-side wait and release their lease on errors', async () => {
  const requests = [], cursor = { version: 1 };
  const s = { host: 'ios', op: async request => {
    requests.push(request);
    if (request.capture) return { capture: request.capture === 'start' };
    if (request.cursor) throw new Error('disconnected');
    return { available: true, cursor };
  }, perf: () => { throw new Error('legacy sampler'); } };
  await expect(perfOp(s, ['frames', 'live', '1'])).rejects.toThrow('disconnected');
  expect(requests.map(request => request.capture ?? 'health')).toEqual(['health', 'start', 'health', 'stop']);
  expect(requests.some(request => request.op === 'clock')).toBe(false);
  await expect(perfOp(s, ['frames', 'live', '30001'])).rejects.toThrow();
});

test('compact reports expose measured stages and preserve unknown evidence', () => {
  const report = { format: 'exact.frame-health/2', available: true, qualification: 'unqualified', scope: { armed: true },
    retention: { overwrittenSinceClear: '7', expiredAtRead: 2 },
    columns: ['sequence', 'capture', 'generation', 'age_us', 'flags', 'target_offset_us', 'period_us', 'requested_period_us', 'gap_us', 'callback_us',
      'before_gap_us', 'before_callback_us', 'after_offset_us', 'after_gap_us', 'after_callback_us',
      'owner_queued_offset_us', 'owner_began_offset_us', 'owner_finished_offset_us', 'main_landed_offset_us',
      'apply_began_offset_us', 'apply_finished_offset_us', 'work_target_offset_us', 'operations'],
    hitches: [['12', '1', '0', 1000, 17, 8000, 8000, 8000, 24000, 1000, 8000, 500, null, null, null,
      -10000, -9000, -5000, -3000, -3000, -1000, 0, 40]] };
  const text = renderFrameHealthReport(report);
  expect(text).toContain('overwritten 7');
  expect(text).toContain('last observed apply ended 1.000 ms before event');
  expect(() => renderFrameHealthReport({ ...report, padding: 'x'.repeat(4000) })).toThrow();
  expect(text).toContain('gap 24.000 ms / scheduled 8.000 ms');
  expect(text).toContain('owner queue 1.000 ms · owner wall 4.000 ms · main wait 2.000 ms · apply 2.000 ms');
  expect(text).toContain('following gap/work unknown / unknown');
  expect(text).toContain('not causal attribution');
  const powered = structuredClone(report);
  powered.hitches[0][4] += 2 ** 23 + 2 ** 16 + 2 ** 33 + 2 ** 34 + 2 ** 36;
  expect(renderFrameHealthReport(powered)).toContain('workload motion · thermal serious · low power true');
  expect(renderFrameHealthReport({ ...report, time: { clamped: true } })).toContain('Timing values were clamped');
  const packed = structuredClone(report);
  packed.hitches[0][4] += 64;
  expect(renderFrameHealthReport(packed)).toContain('owner queue unknown · owner wall unknown');
  expect(renderTrace(report)).toBe(text);
  expect(render('perf', report)).toBe(text);
  expect(() => renderFrameHealthReport({ ...report, hitches: Array(17).fill(report.hitches[0]) })).toThrow();
  expect(() => renderFrameHealthReport({ ...report, hitches: [['bad']] })).toThrow();
});

test('log rendering skips canvas error rows and preserves journal order', () => {
  const error = { canvas: 7, error: 'journal unavailable' };
  const row = lines => ({ canvas: 7, lines });
  expect(render('logs', { lines: ['host entry'], world: [error, row(['first']), error, row(['second']), { canvas: 8, lines: null }] }))
    .toBe('host entry\nworld first\nworld second');
  expect(render('logs', { lines: [], world: [error] })).toBe('(nothing new)');
});

test('a screenshot says what it shows that the app does not on lines of their own', () => {
  const text = render('screenshot', { screenshot: 'a.png', w: 402, h: 874, painted: [{ id: 9, testId: 'menu' }], paintedNote: 'the agent painted it' });
  expect(text).toBe('screenshot a.png · 402×874 · painted=[{"id":9,"testId":"menu"}]\n  NOTE: the agent painted it');
});

const record = { slots: { liked: true, draft: '' }, derives: { left: 2 }, resources: { groceries: [{ title: 'Eggs', done: false }, { title: 'Milk', done: true }], weather: { temp: 21 } }, mediaSession: { owner: null }, transport: { state: 'stopped' }, clock: 0 };
// A session whose views are `like` and `transport` (testIds); asking `state` of anything else is refused as a missing view, as
// the driver's is. A view's answer is the whole record, as every host's is for a view that is not a world.
const session = () => {
  const asked = [];
  return { asked, state: async (target) => {
    asked.push(target);
    if (target != null && target !== 'like' && target !== 'transport') throw new Error(`no view matches ${target} by testId, label or text; targets here: like, transport`);
    return record;
  } };
};

test('a view named like a section of the record reads that section, once', async () => {
  const s = session();
  expect(await stateOp(s, ['transport'])).toEqual({ state: 'stopped' }); // a sequencer's `transport` bar: 142 KB was the record
  expect(s.asked).toEqual(['transport']);
  expect(await stateOp(session(), ['transport.state'])).toBe('stopped');
});

test('a world\'s view keeps its own answer: it has no slots', async () => {
  const world = { tick: 3, entities: [{ name: 'transport' }] };
  expect(await stateOp({ state: async () => world }, ['transport'])).toBe(world);
});

test('a view keeps `state <testId>`; a state, derive, resource or field of the record reads by name and dotted path', async () => {
  expect(await stateOp(session(), [])).toBe(record);
  expect(await stateOp(session(), ['like'])).toBe(record); // a view: the host's own answer
  expect(await stateOp(session(), ['liked'])).toBe(true);
  expect(await stateOp(session(), ['left'])).toBe(2);
  expect(await stateOp(session(), ['weather'])).toEqual({ temp: 21 });
  expect(await stateOp(session(), ['weather.temp'])).toBe(21);
  expect(await stateOp(session(), ['groceries.1.done'])).toBe(true);
  expect(await stateOp(session(), ['mediaSession'])).toEqual({ owner: null });
  const s = session();
  await stateOp(s, ['like']);
  expect(s.asked).toEqual(['like']); // a view is read once, not looked up as a name
});

test('a name that is nothing is refused with the view\'s refusal, and what the state holds; a missing field says what is there', async () => {
  await expect(stateOp(session(), ['nope'])).rejects.toThrow(/^no view matches nope .*; nor is it a state, derive, resource or field of `state`: groceries, weather, left, liked, draft/);
  await expect(stateOp(session(), ['groceries.0.price'])).rejects.toThrow('state groceries.0 has no field "price" (its fields: title, done)');
  await expect(stateOp(session(), ['liked.x'])).rejects.toThrow('state liked has no field "x" (it is true)');
  await expect(stateOp(session(), ['groceries.7'])).rejects.toThrow('state groceries has no field "7"');
});

test('a release build\'s first trace says its frames were not measured yet, and keeps its sites', () => {
  const frames = { unmeasured: 'a release build measures frames from the first agent request or trace; this is the first',
    period: { ms: null, source: 'target' }, lifetime: { presented: 0, late: 0, missed: 0, overruns: 0, segments: 0 }, window: {}, late: [], records: [] };
  const text = renderTrace({ identity: { host: 'ios', app: 'euxy' }, frames, journal: { lines: [] }, perf: { seq: 1, clock: 0, incarnation: 1, sites: [] } });
  expect(text).toContain(`frames not measured: ${frames.unmeasured}`);
  expect(text).not.toContain('presented 0');
});

// LLP 1142 D1, D2: a late frame's phases, and the build to look for when no map fits.
test('a late frame names its phases, and a trace no map fits names the host build', () => {
  const late = { t: 41210, interval: 16.67, missed: 1, overrun: 17, seq: null, batches: 2, apply: 0.3,
    phases: [{ phase: 'text', ms: 4.1, segments: 12 }, { phase: 'rows', ms: 9.4, segments: 3 }], unattributed: 3.17 };
  const frames = { period: { ms: 8.33, source: 'target' }, lifetime: { presented: 9, late: 1, missed: 1, overruns: 1, segments: 1 }, window: {}, late: [late], records: [] };
  const text = renderTrace({ identity: { host: 'macos', app: 'excord', hostBuild: { commit: 'abcdef1234567890', dirty: true, machine: 'M5' } }, frames, journal: { lines: [] },
    plan: 'ab'.repeat(32), perf: { plan: 'ab'.repeat(32), seq: 1, clock: 0, incarnation: 1, sites: [{ site: 4, source: { status: 'unavailable' } }] } });
  expect(text).toContain('no transaction range · 2 batches');
  expect(text).toContain('of its 16.67 ms: rows 9.4 (3) · text 4.1 (12) · unattributed 3.17');
  expect(text).toContain('host built from abcdef123456 (dirty) on M5');
  expect(text).toContain('no map here fits plan abababababab: pass --plan');
});

test('ink traces retain phase and coverage information without inventing frame joins', () => {
  const text = renderTrace({ identity: { host: 'apple-painted' }, presented: [1, 2], dropped: 3,
    traceEvents: [{ cat: 'ink', name: 'paint walk', dur: 1000 }, { cat: 'ink', name: 'paint walk', dur: 2000 },
      { cat: 'ink', name: 'presented', dur: 0 }] });
  expect(text).toContain('2 presented timestamps, 3 intervals/marks, 3 dropped');
  expect(text).toContain('inclusive phases, not additive; idle gaps are not classified as late');
  expect(text).toContain('paint walk: 2 spans · total 3.000 ms · p99 2.000 ms');
  expect(text).not.toContain('world trace');
});

test('--plan finds plans with a map beside them in a directory, and refuses one with none', () => {
  const dir = mkdtempSync(join(tmpdir(), 'plan-'));
  mkdirSync(join(dir, 'bake', 'dev'), { recursive: true });
  writeFileSync(join(dir, 'bake', 'dev', 'macos.plan.map.json'), '{}');
  expect(planLocators(dir)).toEqual([join(dir, 'bake', 'dev', 'macos.plan')]);
  expect(planLocators(join(dir, 'x.plan.map.json'))).toEqual([join(dir, 'x.plan')]);
  expect(() => planLocators(join(dir, 'bake', 'dev', 'empty-dir-does-not-exist'))).toThrow();
});

test('perf during subtracts each handler\'s dispatches and drops the ones that did not run', () => {
  const read = (n) => ({ plan: 'p', incarnation: 1, seq: n, sites: [], handlers: [{ site: 3, event: 'hover in', dispatched: n, committed: n }, { site: 5, event: 'press', dispatched: 1, committed: 0 }] });
  expect(perfDelta(read(2), read(60)).handlers).toEqual([{ site: 3, event: 'hover in', dispatched: 58, committed: 58 }]);
});

test('state world events uses the world seam and an optional type filter', async () => {
  const calls = [], s = {state: async (...args) => { calls.push(args); return {events:[]}; }};
  await stateOp(s, ['world','events','Hit']);
  expect(calls[0][0]).toBe('world');
  expect(calls[0][4]).toEqual({world:true, events:true, event_type:'Hit'});
  await stateOp(s, ['world','events']);
  expect(calls[1][4]).toEqual({world:true, events:true});
});

// A target covered by what may leave on a timer the frozen clock never reaches (euxy FRICTION 9): the refusal names the
// tasks still waiting and the step that runs them; with none waiting it is as it was.
test('a covered target\'s refusal names the tasks waiting on the clock', async () => {
  const covered = (tasks, clock = 0) => tapRefusal({ state: async () => ({ clock, tasks }), tree: async () => ({ nodes: [] }) }, 'go', new Error('tap #333: node #6 covers its middle'));
  expect((await covered({ boot: 1, splash: 900, charge: null })).message).toBe('tap #333: node #6 covers its middle; the agent\'s clock stands at 0 ms, and the app\'s tasks wait on it (`boot` at 1 ms, `splash` at 900 ms): if what covers it leaves on a timer, `clock +900` runs them');
  expect((await covered({ splash: 900 }, 1000)).message).toBe('tap #333: node #6 covers its middle');
  expect((await covered({})).message).toBe('tap #333: node #6 covers its middle');
});

// A task due a commit later (an `after(1, …)` an answer armed, euxy FRICTION 7) is what `clock settle` runs and
// `clock data` names; one due later, one spent and one past are not.
test('the tasks due a commit later are those within 1 ms of the clock', async () => {
  const session = { state: async () => ({ tasks: { importLink: 1001, later: 1002, spent: null, past: 999, now: 1000, adopt: 1000.5 } }) };
  expect(await nextCommitTasks(session, 1000)).toEqual([['adopt', 1000.5], ['importLink', 1001]]);
  expect(await nextCommitTasks(session, 2000)).toEqual([]);
});

// Past its 16 rounds, `clock settle` is not settled while a task is still due a commit later, and names it.
test('a settle that runs out of rounds says which tasks are still due', () => {
  const r = settleBound([['chain', 17]]);
  expect([r.settled, r.reason, r.tasks]).toEqual([false, 'tasks', { chain: 17 }]);
  expect(r.diagnostic).toContain('`chain` is due again at 17 ms');
});

test('work deltas retain scope and refuse reset measurement windows', () => {
  const read = n => ({plan:'p',incarnation:1,seq:n,sites:[],work:{since:4,scope:'session',timebase:'runner-clock-ms',counts:{kernelLayouts:n}},hostWork:{since:8,scope:'process',timebase:'media-clock-ms',counts:{ownerSync:n*2}}});
  const delta = perfDelta(read(3), read(9));
  expect(delta.work.counts.kernelLayouts).toBe(6);
  expect(delta.hostWork.counts.ownerSync).toBe(12);
  const reset = read(10); reset.hostWork.since = 9;
  expect(() => perfDelta(read(3), reset)).toThrow('measurement coverage changed');
  expect(() => perfDelta(read(9), read(3))).toThrow('seq went back');
});

test('resource declarations require the state reply’s own plan digest', async () => {
  const {identifyResources} = await import('./agent-inspect.mjs');
  const maps = {get: d => d === 'new' ? {resources:{inbox:{file:'app.contract',line:7,col:3,end_col:8,component:'App'}}} : null};
  const reply = digest => ({planDigest:digest,resourceStatus:{inbox:{resource:1,source:'Inbox'}}});
  expect(identifyResources(reply('new'), maps).resourceStatus.inbox.declaration).toMatchObject({status:'compatible',file:'app.contract',line:7});
  expect(identifyResources(reply('old'), maps).resourceStatus.inbox.declaration.status).toBe('unavailable');
  expect(identifyResources(reply(undefined), maps).resourceStatus.inbox.declaration.status).toBe('unavailable');
});

test('resource declarations refuse incomplete or malformed source spans', async () => {
  const {identifyResources} = await import('./agent-inspect.mjs');
  const valid = {file:'app.contract', line:7, col:3, end_col:8, component:'App'};
  for (const bad of [{end_col:undefined}, {end_col:2}, {end_col:3.5}, {component:undefined}, {line:Number.MAX_SAFE_INTEGER+1}]) {
    const maps = {get: () => ({resources:{inbox:{...valid,...bad}}})};
    const reply = {planDigest:'same',resourceStatus:{inbox:{resource:1,source:'Inbox'}}};
    expect(identifyResources(reply, maps).resourceStatus.inbox.declaration.status).toBe('unavailable');
  }
});

// LLP 1144 D3: a scroll handler that changed the tree on every event of a fling is said once, with what to do; one that
// commits rarely, or a press, is only counted.
test('perf names a scroll handler that commits on every event', () => {
  const r = { target: 'transcript', seq: 9, clock: 0, incarnation: 1, sites: [], handlers: [
    { site: 4, event: 'scroll', dispatched: 180, committed: 180 }, { site: 5, event: 'scroll', dispatched: 180, committed: 3 }, { site: 6, event: 'press', dispatched: 40, committed: 40 }] };
  const text = render('perf', r);
  expect(text.match(/each scroll event changed the tree, and each was a runner turn/g)?.length).toBe(1);
  expect(text).toContain('scroll at site 4: 180 dispatched, 180 changed the tree\n    each scroll event');
  expect(text).not.toContain('each press event');
});

// LLP 1146 D3: each resource's asks between two reads; a host that counts none has no `resources`.
test('perf … during subtracts each resource\'s asks', () => {
  const read = (asked) => ({ plan: 'p', incarnation: 1, seq: asked, clock: 0, sites: [], resources: [{ name: 'channels', asked }, { name: 'servers', asked: 4 }] });
  expect(perfDelta(read(2), read(8)).resources).toEqual([{ name: 'channels', asked: 6 }]);
  const bare = { plan: 'p', incarnation: 1, seq: 1, clock: 0, sites: [] };
  expect(perfDelta(bare, bare).resources).toBeUndefined();
});

test('layout shows what a pressed paint shows, beside the computed base', () => {
  const lines = render('layout', { viewport: { w: 400, h: 300 }, clock: 0, nodes: [], node: { id: 3, type: 'View', style: {}, press_shown: { paint: 1, opacity: '0.4', backgroundColor: 'rgb(0, 0, 255)', color: 'rgb(255, 0, 0)' } } });
  expect(lines).toContain('press shows opacity=0.4 background="rgb(0, 0, 255)" color="rgb(255, 0, 0)" (paint 1)');
});

test('a scroll track starts on a node, keeps its cursor, and its stop joins the last page', async () => {
  const requests = [];
  const fmt = 'exact.frame-health.track/1', columns = ['t_us', 'interval_us', 'missed', 'offset_cpt', 'phase'];
  let written = 2, stopped = false;
  const rowAt = i => [i * 8333, i ? 8333 : 0, 0, (i + 1) * 100, 'd'];
  const s = { host: 'ios', target: async name => ({ id: name === 'list' ? 7 : 0 }), op: async request => {
    requests.push(request);
    if (request.track === 'start') { written = 2; stopped = false; return { format: fmt, track: true, node: 7, leaseMs: request.durationMs, capacity: 4096, replaced: false }; }
    if (stopped) return { format: fmt, track: false, reason: 'no-track' };
    const from = Number(request.cursor ?? 0), to = Math.min(written, from + 1);
    if (request.track === 'stop') { stopped = true; return { format: fmt, columns, node: 7, rows: [...Array(written - from).keys()].map(k => rowAt(from + k)), cursor: String(written), more: false, dropped: 0, period_us: 16667, stopped: true }; }
    return { format: fmt, columns, node: 7, rows: from < to ? [rowAt(from)] : [], cursor: String(to), more: to < written, dropped: request.cursor === undefined ? 3 : 0, period_us: 16667 };
  } };
  await perfOp(s, ['frames', 'track', 'start', 'list', '5000']);
  expect(requests[0]).toEqual({ op: 'perf', frames: true, track: 'start', node: 7, durationMs: 5000 });
  requests.length = 0;
  const first = await perfOp(s, ['frames', 'track', 'read']);
  expect(first.rows.map(row => row[3])).toEqual([100, 200]);
  expect(first.dropped).toBe(3);
  written = 3;
  const second = await perfOp(s, ['frames', 'track', 'read']);
  expect(second.rows.map(row => row[3])).toEqual([300]);
  expect(requests.map(request => request.cursor)).toEqual([undefined, '1', '2']);
  written = 5; // two rows land; the read takes one more page, then rows arrive before the stop
  requests.length = 0;
  const origOp = s.op;
  s.op = async request => { const reply = await origOp(request); if (request.track === 'read' && !reply.more) written = 6; return reply; };
  const last = await perfOp(s, ['frames', 'track', 'stop']);
  expect(last.rows.map(row => row[3])).toEqual([400, 500, 600]);
  expect(last.stopped).toBe(true);
  expect(last.more).toBe(false);
  expect(requests.at(-1)).toMatchObject({ track: 'stop' });
  expect(renderPerf(last)).toContain('3 rows');
  // After the stop, no track: a read surfaces it, rendered.
  const none = await perfOp(s, ['frames', 'track', 'read']);
  expect(none).toMatchObject({ track: false, reason: 'no-track' });
  expect(renderPerf(none)).toContain('no track is leased');
  expect(renderPerf({ format: fmt, track: true, node: 7, leaseMs: 5000, capacity: 4096, replaced: true })).toContain('replaced');
  expect(renderPerf({ format: fmt, available: false, track: false, reason: 'virtual-clock' })).toContain('unavailable: virtual-clock');
  await expect(perfOp(s, ['frames', 'track', 'start', 'list', '120001'])).rejects.toThrow();
  await expect(perfOp(s, ['frames', 'track', 'start'])).rejects.toThrow();
  await expect(perfOp(s, ['frames', 'track', 'read', 'more'])).rejects.toThrow();
});

test('a scroll track that ends between pages surfaces no-track, not a partial page saying more', async () => {
  const fmt = 'exact.frame-health.track/1', columns = ['t_us', 'interval_us', 'missed', 'offset_cpt', 'phase'];
  const s = { host: 'ios', op: async request => request.cursor === undefined
    ? { format: fmt, columns, node: 7, rows: [[0, 0, 0, 100, 'd']], cursor: '1', more: true, dropped: 0, period_us: 16667 }
    : { format: fmt, track: false, reason: 'no-track' } };
  const read = await readTrack(s);
  expect(read).toMatchObject({ track: false, reason: 'no-track' });
  expect(read.rowsBeforeEnd).toHaveLength(1);
  expect(read.more).toBeUndefined();
});

test('a scroll track start by view id sends no tree, and cursors are kept per app session', async () => {
  const fmt = 'exact.frame-health.track/1', columns = ['t_us', 'interval_us', 'missed', 'offset_cpt', 'phase'];
  const requests = [], written = { A: 5, B: 2 };
  const s = { host: 'macos', session: 'A', target: async () => { throw new Error('a view id needs no tree'); }, op: async request => {
    requests.push({ ...request, session: s.session });
    if (request.op !== 'perf') throw new Error(`unexpected ${request.op}`);
    if (request.track === 'start') return { format: fmt, track: true, node: request.node, leaseMs: 30000, capacity: 4096, replaced: false };
    const from = Number(request.cursor ?? 0), n = written[s.session];
    return { format: fmt, columns, node: 7, rows: [...Array(Math.max(0, n - from)).keys()].map(k => [k, 0, 0, (from + k) * 100, 'd']), cursor: String(n), more: false, dropped: 0, period_us: 8333 };
  } };
  await perfOp(s, ['frames', 'track', 'start', '42']);
  expect(requests).toEqual([{ op: 'perf', frames: true, track: 'start', node: 42, durationMs: 30000, session: 'A' }]);
  s.session = 'B';
  await perfOp(s, ['frames', 'track', 'start', '43']);
  s.session = 'A';
  expect((await perfOp(s, ['frames', 'track', 'read'])).rows).toHaveLength(5);
  s.session = 'B';
  const b = await perfOp(s, ['frames', 'track', 'read']);
  expect(b.rows).toHaveLength(2);
  expect(requests.filter(r => r.track === 'read').map(r => [r.session, r.cursor])).toEqual([['A', undefined], ['B', undefined]]);
  s.session = 'A'; written.A = 6;
  expect((await perfOp(s, ['frames', 'track', 'read'])).rows.map(row => row[3])).toEqual([500]);
  expect(requests.at(-1)).toMatchObject({ session: 'A', cursor: '5' });
});

test('a stop freezes the track and the driver reads a multi-page tail to the end, dropped exact', async () => {
  const fmt = 'exact.frame-health.track/1', columns = ['t_us', 'interval_us', 'missed', 'offset_cpt', 'phase'];
  const requests = [];
  let frozen = false, released = false;
  let written = 7; const page = 2, oldest = 1; // one row overwritten before any read
  const s = { host: 'ios', session: null, op: async request => {
    requests.push(request);
    if (released) return { format: fmt, track: false, reason: 'no-track' };
    if (request.track === 'stop') { written = 13; frozen = true; } // six rows land between the last read and the stop
    const asked = Number(request.cursor ?? 0), from = Math.max(asked, oldest), to = Math.min(written, from + page);
    const more = to < written;
    if (frozen && !more) released = true;
    return { format: fmt, columns, node: 7, rows: [...Array(to - from).keys()].map(k => [from + k, 0, 0, (from + k) * 100, 'd']),
      cursor: String(to), more, dropped: from - Math.min(asked, from), period_us: 16667, ...(frozen ? { stopped: true } : {}) };
  } };
  // The stop's own tail is three pages: its first in the stop's reply, the rest read to the end.
  const out = await perfOp(s, ['frames', 'track', 'stop']);
  expect(out.rows.map(row => row[0])).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
  expect(out.dropped).toBe(1);
  expect(out.more).toBe(false);
  expect(out.stopped).toBe(true);
  expect(released).toBe(true);
  expect(requests.map(r => [r.track, r.cursor])).toEqual([['read', undefined], ['read', '3'], ['read', '5'], ['stop', '7'], ['read', '9'], ['read', '11']]);
  expect(renderPerf(out)).toContain('12 rows');
});
