import { test, expect } from 'bun:test';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { analyze, quantile, script, cli, consoleState, exactAdapter, prepareExact, runFeel, table, projectPixels, screenMotion, DISPLACEMENT_CHANGE_PX, reanalyze } from './feel.mjs';
import { inputRecorder, keys } from './probes/input.mjs';

// Independent 100 Hz presentation / 50 Hz motion fixture. Sampling is five ms
// after the frame timestamp, deliberately exposing the rAF/event clock pitfall.
function fixture(stepped = false) {
  const plan = script();
  const raw = { schema: 1, stride: 8, overflow: false,
    events: plan.schedule.flatMap(e => [1000 + e.at_ms, e.code, +e.down, e.trial]), frames: [] };
  for (let t = 0; t <= plan.duration_ms; t += 10) {
    const motionTime = stepped ? Math.floor(t / 20) * 20 : t;
    let z = Math.min(2500, Math.max(0, motionTime - 500)) * .004;
    for (const e of plan.schedule) if (e.trial >= 0 && t >= e.at_ms + 10) z += .01;
    raw.frames.push(t + 995, t + 1000, 0, .9, z, 0, 15, z + 19);
  }
  return { raw, plan };
}

test('the live script keeps all holds, release order, and twenty separated trials', () => {
  const p = script();
  expect(p.schedule.length).toBe(50);
  expect(p.schedule[3].at_ms - p.schedule[2].at_ms).toBe(2500);
  expect(p.schedule[5].at_ms - p.schedule[4].at_ms).toBe(1500);
  expect(p.schedule[9].at_ms - p.schedule[8].at_ms).toBe(1500);
  expect(p.schedule.filter(e => e.trial >= 0).length).toBe(20);
  expect(script(30).schedule[10].at_ms).toBe(30000);
  expect(script(7.02).schedule[10].at_ms).toBe(10020);
  expect(p.duration_ms).toBe(80900);
});

test('constant velocity has zero judder; alternating move/repeat approaches one', () => {
  const s = fixture(), q = fixture(true);
  const smooth = analyze(s.raw, s.plan), stepped = analyze(q.raw, q.plan);
  expect(smooth.refresh_interval_ms).toBe(10);
  expect(smooth.hitches).toBe(0);
  expect(smooth.player.judder).toBeLessThan(1e-12);
  expect(smooth.player.repeated_fraction).toBe(0);
  expect(stepped.player.judder).toBeCloseTo(1, 1);
  expect(stepped.player.repeated_fraction).toBeCloseTo(.5, 1);
  expect(stepped.camera.judder).toBeCloseTo(stepped.player.judder, 10);
});

test('latency uses the draw sample clock, not the older frame timestamp', () => {
  const { raw, plan } = fixture(), m = analyze(raw, plan);
  expect(m.latency.valid_trials).toBe(20);
  expect(m.latency.median_ms).toBe(10);
  expect(m.latency.p95_ms).toBe(10);
  expect(m.latency.median_intervals).toBe(1);
  expect(m.latency.trials.every(t => t.stationary_before)).toBe(true);
});

test('residual motion invalidates a trial instead of claiming a quick response', () => {
  const { raw, plan } = fixture();
  for (let i = 0; i < raw.frames.length; i += 8) if (raw.frames[i + 1] === 12950) raw.frames[i + 2] = .00001;
  const m = analyze(raw, plan);
  expect(m.latency.valid_trials).toBe(19);
  expect(m.latency.trials[0].latency_ms).toBeNull();
});

test('quantiles interpolate; corrupt, overflowed, or missing records fail explicitly', () => {
  expect(quantile([7, 1, 5, 3], .5)).toBe(4);
  expect(quantile([10, 0], .95)).toBe(9.5);
  const { raw, plan } = fixture();
  expect(() => analyze({ ...raw, overflow: true }, plan)).toThrow('buffer');
  expect(() => analyze({ ...raw, events: raw.events.slice(4) }, plan)).toThrow('delivered');
  const redraw = { ...raw, frames: [...raw.frames.slice(0, 16), ...raw.frames.slice(8, 16), ...raw.frames.slice(16)] };
  expect(analyze(redraw, plan).frames).toEqual(analyze(raw, plan).frames); // a redraw at the same time is not a frame
  raw.frames[8] = raw.frames[0] - 1;
  expect(() => analyze(raw, plan)).toThrow('Nonmonotonic');
});

test('exact adapts only the frame layout, preserving both clocks and all precision', async () => {
  const { normalize } = await import('./probes/exact.mjs');
  const { adapters } = await import('./feel.mjs');
  expect(adapters.exact.transport).toBe('web');
  expect(adapters.exact.page).not.toContain('agent');
  const { raw, plan } = fixture();
  const exact = { stride: 14, missing: false, frames: [] };
  for (let i = 0; i < raw.frames.length; i += 8) exact.frames.push(...raw.frames.slice(i, i + 8), .123456789, 1, 10, .2, .3, .4);
  expect(normalize(exact)).toEqual(raw.frames);
  expect(analyze({ ...raw, frames: normalize(exact) }, plan)).toEqual(analyze(raw, plan));
  expect(() => normalize({ ...exact, missing: true })).toThrow('Invalid');
  expect(() => normalize({ ...exact, stride: 8 })).toThrow('Invalid');
  expect(() => normalize({ ...exact, frames: [1] })).toThrow('Invalid');
});

function deliver(target, item, repeat = false) {
  const code = Object.keys(keys).find(name => keys[name][0] === item.code);
  const e = new Event(item.down ? 'keydown' : 'keyup');
  Object.defineProperties(e, { code: { value: code }, repeat: { value: repeat }, timeStamp: { value: item.at_ms } });
  target.dispatchEvent(e);
}

test('shared recorder captures each of the 50 scheduled edges once, including trial labels and stamps', () => {
  const target = new EventTarget(), { raw, plan } = fixture();
  let now = 0;
  const recorder = inputRecorder(target, () => now);
  try {
    recorder.begin();
    for (const item of plan.schedule) {
      now = 1000 + item.at_ms;
      if (item.trial >= 0) recorder.arm(item.trial);
      deliver(target, item);
    }
    const actual = recorder.end();
    expect(actual.events).toEqual(raw.events);
    expect(actual.event_stamps_ms).toEqual(plan.schedule.map(e => e.at_ms));
    expect(actual.overflow).toBe(false);
    expect(analyze({ ...raw, ...actual }, plan).delivered_events).toBe(50);
    expect(analyze({ ...raw, ...actual }, plan).latency.valid_trials).toBe(20);
    recorder.begin(); // Reset does not add another listener or retain the previous run.
    deliver(target, plan.schedule[0]);
    deliver(target, plan.schedule[0], true);
    expect(recorder.end().events.length).toBe(4);
  } finally { recorder.dispose(); }
});

test('extra delivered input is retained and invalidates the attempt, never deduplicated', () => {
  const target = new EventTarget(), { raw, plan } = fixture();
  const recorder = inputRecorder(target, () => 1);
  try {
    recorder.begin();
    for (const item of plan.schedule) deliver(target, item);
    deliver(target, { code: 65, down: true, at_ms: 99999 });
    const captured = recorder.end();
    expect(captured.events.length / 4).toBe(51);
    expect(() => analyze({ ...raw, ...captured }, plan)).toThrow('Expected 50 delivered events, got 51');
    recorder.begin();
    for (let i = 0; i < 129; i++) deliver(target, plan.schedule[0]);
    expect(recorder.end().overflow).toBe(true);
  } finally { recorder.dispose(); }
});

test('exact installation has one explicit entry and is idempotent while warming', async () => {
  const names = ['exact', 'window', 'document', 'setTimeout', 'requestAnimationFrame'];
  const saved = names.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  let clicks = 0, reloads = 0;
  try {
    globalThis.window = globalThis;
    globalThis.requestAnimationFrame = () => 0;
    const play = { click() { clicks++; }, focus() {} };
    globalThis.document = { querySelector: s => s === '#start' ? play : { dataset: { view: '1' } } };
    globalThis.exact = { ready: Promise.resolve(), gpu: { wantsInput: () => true }, reload: async () => { reloads++; } };
    globalThis.setTimeout = fn => { queueMicrotask(fn); return 0; };
    const { install } = await import('./probes/exact.mjs?install-test');
    expect(clicks).toBe(0); // Import alone must not install.
    const first = install({ play: '#start', entity: 'hero' });
    expect(install({ play: '#start', entity: 'hero' })).toBe(first);
    await first;
    expect(clicks).toBe(1);
    expect(reloads).toBe(1);
  } finally {
    delete globalThis.feel;
    for (const [name, descriptor] of saved) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete globalThis[name];
    }
  }
});

test('console preflight fails closed on locked, active, missing or malformed ioreg state', () => {
  const root = '"IOConsoleLocked" = No', idle = '"HIDIdleTime" = 30000000000';
  expect(consoleState(root, idle)).toEqual({ locked: false, idle_seconds: 30 });
  expect(() => consoleState('"IOConsoleLocked" = Yes', idle)).toThrow('display is locked');
  expect(() => consoleState(root, '"HIDIdleTime" = 29999999999')).toThrow('console active');
  expect(() => consoleState(root, '"HIDIdleTime" = 0')).toThrow('console active');
  expect(() => consoleState('', idle)).toThrow('cannot read');
  expect(() => consoleState(root, '"HIDIdleTime" = unknown')).toThrow('cannot read');
});

test('CLI accepts --game, --no-build and --hz 120 with explicit adapter parameters', () => {
  const options = cli(['exact', '--no-build', '--game', 'another-game', '--hz', '120', '--entity', 'hero', '--play', '#start', '--attempts', '1']);
  expect(options).toMatchObject({ noBuild: true, game: 'another-game', hz: 120, attempts: 1 });
  const adapter = exactAdapter(options);
  expect(adapter.root).toEndWith('/games/another-game/target/feel120');
  expect(adapter.options).toEqual({ entity: 'hero', play: '#start' });
  expect(adapter.variants).toEqual(['120hz']);
  expect(cli(['exact']).attempts).toBe(3);
  expect(cli(['exact']).noBuild).toBe(false);
  for (const args of [['exact', '--game'], ['exact', '--game', '../bad'], ['exact', '--hz', '90'], ['exact', '--attempts', '4'], ['compare', '--game', 'other']]) {
    expect(() => cli(args)).toThrow();
  }
});

test('--no-build reads existing 60 and 120 Hz bakes without running the proof build gate', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'feel-bakes-'));
  const previous = process.env.FEEL_EXACT_DIST;
  try {
    // No developer-local bake is required: these are the adapter's three inputs.
    for (const name of ['index.html', 'exact.json', 'gpu_bg.wasm']) writeFileSync(join(dir, name), 'fixture');
    process.env.FEEL_EXACT_DIST = dir;
    for (const hz of [60, 120]) {
      const adapter = await prepareExact(cli(['exact', '--no-build', '--hz', String(hz)]));
      expect(adapter.hz).toBe(hz);
      expect(adapter.build).toMatchObject({ source: '--no-build', freshness_checked: false });
      expect(adapter.build.artifact_sha256).toMatch(/^[0-9a-f]{64}$/);
      expect(adapter.root).toBe(dir);
    }
  } finally {
    if (previous === undefined) delete process.env.FEEL_EXACT_DIST;
    else process.env.FEEL_EXACT_DIST = previous;
    rmSync(dir, {recursive:true, force:true});
  }
});

test('one command preflights before preparation, interleaves three attempts, and labels 120 Hz separately', async () => {
  const order = [], rows = [];
  const { raw, plan } = fixture(), metrics = analyze(raw, plan);
  const result = await runFeel(cli(['compare', '--no-build']), {
    ready() { order.push('preflight'); return { locked: false, idle_seconds: 30 }; },
    async prepare(options) { order.push(`prepare-${options.hz}`); expect(options.noBuild).toBe(true); return exactAdapter(options); },
    async take(engine, variant, run) {
      order.push(`${engine}/${variant}`);
      const row = { ...metrics, engine, variant, run, attempt: 1, valid: true, load1: 35, provisional: true, frontmost_visible_confirmed: true };
      rows.push(row); return row;
    },
    save(_batch, written) { expect(written).toEqual(rows); },
  });
  expect(order.slice(0, 4)).toEqual(['preflight', 'prepare-60', 'prepare-120', 'preflight']);
  const variants = ['exact/60hz', 'exact/120hz', 'three/shipped', 'godot/shipped', 'godot/interpolation'];
  expect(order.slice(4)).toEqual([...variants, ...variants, ...variants]);
  expect(result.length).toBe(15);
  const report = table(result);
  expect(report).toContain('| exact/120hz | 1 | PROVISIONAL |');
  expect(report).toContain('| exact/60hz |');
  expect(report).toContain('| three/shipped |');
  expect(report).toContain('| godot/shipped |');
});

test('refused preflight never prepares or measures; --attempts 1 never retakes an invalid row', async () => {
  let calls = 0;
  const unexpected = () => { calls++; throw new Error('unexpected work'); };
  await expect(runFeel(cli(['exact', '--no-build']), {
    ready() { throw new Error('display is locked'); }, prepare: unexpected, take: unexpected, save: unexpected,
  })).rejects.toThrow('display is locked');
  expect(calls).toBe(0);
  const result = await runFeel(cli(['exact', '--no-build', '--attempts', '1']), {
    ready: () => ({}), prepare: async options => exactAdapter(options),
    take: async () => { calls++; return { valid: false, error: 'extra input' }; },
    save: (_batch, rows) => expect(rows.length).toBe(1),
  });
  expect(calls).toBe(1);
  expect(result[0].valid).toBe(false);
});


test('a mid-batch refusal saves completed/invalid rows and stops without another attempt', async () => {
  let takes = 0, saved;
  const invalid = { valid: false, error: 'console active', engine: 'exact', variant: '60hz', run: 2, attempt: 1, load1: 35, provisional: true };
  await expect(runFeel(cli(['exact', '--no-build']), {
    ready: () => ({}), prepare: async options => exactAdapter(options),
    take: async () => {
      if (++takes === 2) throw Object.assign(new Error('console active'), { row: invalid });
      return { valid: true };
    },
    save: (_batch, rows) => { saved = rows; },
  })).rejects.toThrow('console active');
  expect(takes).toBe(2);
  expect(saved).toEqual([{ valid: true }, invalid]);
  expect(table([invalid])).toContain('INVALID PROVISIONAL console active | 35.0');
});


test('raw callbacks and drawn clocks have independent intervals, hitches and latency units', () => {
  const { raw, plan } = fixture();
  raw.drawn_clock_ms = raw.frames.filter((_, i) => i % 8 === 0).map((_, i) => 995 + i * 5);
  const m = analyze(raw, plan);
  expect(m.frame_ms.p50).toBe(10);
  expect(m.drawn_clock_ms.p50).toBe(5);
  expect(m.latency.median_intervals).toBe(1);
  raw.raw_callback_ms = raw.frames.filter((_, i) => i % 8 === 0).map((t, i) => t + (i % 2 ? 6 : 0));
  const jitter = analyze(raw, plan);
  expect(jitter.hitches).toBeGreaterThan(0);
  expect(jitter.drawn_hitches).toBe(0);
  raw.drawn_clock_ms[100] = raw.drawn_clock_ms[99]; // repeated drawn slot is still a new raw callback
  expect(analyze(raw, plan).frames).toBe(m.frames);
  raw.raw_callback_ms[100] = raw.raw_callback_ms[99];
  expect(analyze(raw, plan).frames).toBe(m.frames - 1);
  raw.raw_callback_ms[100] = raw.raw_callback_ms[99] - 1;
  expect(() => analyze(raw, plan)).toThrow('Nonmonotonic');
});

test('known perspective camera projects world pose into physical canvas pixels', () => {
  // 90-degree vertical FOV, aspect 2, camera at z=5 looking along -Z.
  const projection = [.5,0,0,0, 0,1,0,0, 0,0,-1,-1, 0,0,4,5, 800,400];
  expect(projectPixels([0,0,0], projection)).toEqual([400,200]);
  expect(projectPixels([1,1,3], projection)).toEqual([500,100]);
  expect(projectPixels([0,0,6], projection)).toBeNull();
  const { raw, plan } = fixture();
  const identity = [1,0,0,0, 0,1,0,0, 0,0,0,0, 0,0,.5,1, 800,400];
  raw.camera_projection = [];
  for (let i = 0; i < raw.frames.length; i += 8) {
    raw.frames[i + 2] = raw.frames[i + 4];
    raw.camera_projection.push(...identity);
  }
  const m = analyze(raw, plan);
  expect(m.screen_player.mean_displacement_px).toBeCloseTo(16, 10);
  expect(m.screen_player.judder).toBeLessThan(1e-12);
});

test('half-pixel change threshold is strictly greater, with repeated frames counted separately', () => {
  expect(DISPLACEMENT_CHANGE_PX).toBe(.5);
  expect(screenMotion([[0,0],[1,0],[2.5,0]]).change_fraction).toBe(0);
  expect(screenMotion([[0,0],[1,0],[2.500001,0]]).change_fraction).toBe(1);
  expect(screenMotion([[0,0],[1,0],[1.5,0]]).change_fraction).toBe(0);
  expect(screenMotion([[0,0],[1,0],[1.499999,0]]).change_fraction).toBe(1);
  expect(screenMotion([[0,0],[0,0],[1,0]]).repeated_fraction).toBe(.5);
  expect(screenMotion([[0,0],[0,0],[0,0]])).toMatchObject({judder:null, repeated_fraction:1, change_fraction:0});
});

test('missing projection and historical Exact raw clock stay unavailable, never wall-clock substitutes', () => {
  const { raw, plan } = fixture();
  const m = analyze({...raw, exact_trace:{}}, plan);
  expect(m.screen_player).toBeNull();
  expect(m.frame_ms).toBeNull();
  expect(m.hitches).toBeNull();
  expect(m.latency.median_ms).toBe(10);
  expect(m.latency.median_intervals).toBeNull();
  const report = table([{...m, engine:'exact', variant:'60hz', valid:true, provisional:true}]);
  expect(report).toContain('| — | — | — |');
  expect(report).not.toContain('NaN');
  expect(() => analyze({...raw, camera_projection:[1]}, plan)).toThrow('camera_projection');
});

test('Exact recorder joins host draws causally and ignores intervening callbacks', async () => {
  const { callbackRecorder } = await import('./probes/exact.mjs');
  const target = {exact:{}, requestAnimationFrame: cb => cb(999)};
  const recorder = callbackRecorder(target);
  recorder.begin(10);
  target.exact.drawCallback(100, 99, 1);
  target.exact.drawCallback(100, 99, 1); // resize, same callback generation
  target.exact.drawCallback(100.3, 99, 2);
  target.requestAnimationFrame(() => {});
  target.exact.drawCallback(110, 109, 3);
  const result = recorder.end([99,103,0,0,0,0,0,0, 99,112,0,0,0,0,0,0, 99,112.5,0,0,0,0,0,0, 109,113,0,0,0,0,0,0]);
  expect(result).toEqual({raw_callback_ms:[100,100,100.3,110], callback_generation:[1,1,2,3], overflow:false});
});

test('Exact recorder overflow skips the join and is refused by analysis', async () => {
  const { callbackRecorder } = await import('./probes/exact.mjs');
  const target = {exact:{}, requestAnimationFrame: () => {}};
  const recorder = callbackRecorder(target);
  recorder.begin(1);
  target.exact.drawCallback(10, 9);
  target.exact.drawCallback(20, 19);
  const result = recorder.end([19,21,0,0,0,0,0,0]);
  expect(result).toEqual({overflow:true});
  const {raw, plan} = fixture();
  expect(() => analyze({...raw,...result},plan)).toThrow('overflowed');
});

test('legacy Exact retains distinct rows at the same drawn time', () => {
  const {raw,plan} = fixture();
  raw.exact_trace = {};
  const before = analyze(raw,plan);
  raw.frames[8] = raw.frames[0];
  const after = analyze(raw,plan);
  expect(after.frames).toBe(before.frames);
  expect(after.legacy_clock_caveat).toContain('every drawn row');
});

test('orthographic clip depth rejects behind-camera points', () => {
  const projection = [1,0,0,0, 0,1,0,0, 0,0,-.1,0, 0,0,0,1, 800,400];
  expect(projectPixels([0,0,1],projection)).toBeNull();
  expect(projectPixels([0,0,-1],projection)).toEqual([400,200]);
  expect(projectPixels([0,0,-11],projection)).toBeNull();
  expect(projectPixels([0,0,0],undefined)).toBeNull();
});

test('landmark motion measures the world under a player-follow camera', () => {
  const {raw,plan} = fixture();
  raw.landmark_xyz = [0,0,-1];
  raw.camera_projection = [];
  for (let i=0;i<raw.frames.length;i+=8) {
    const z=raw.frames[i+4];
    raw.camera_projection.push(1,0,0,0, 0,0,0,0, 0,1,0,0, 0,-z,.5,1, 800,400);
  }
  const m=analyze(raw,plan);
  expect(m.screen_player.judder).toBeNull();
  expect(m.screen_landmark.mean_displacement_px).toBeGreaterThan(1);
  expect(m.screen_landmark.judder).toBeLessThan(1e-10);
  expect(m.screen_landmark.repeated_fraction).toBe(0);
  expect(table([{...m,engine:'exact',variant:'60hz',valid:true}])).toContain('landmark CV');
});

test('reanalyze retains focus, trial, edge and provisional rules from saved evidence', () => {
  const dir = mkdtempSync(join(tmpdir(), 'f3-reanalyze-'));
  const path = join(dir, 'feel-2026-09-18T11-56-12-978Z-three-shipped-1-attempt1.json.gz');
  const { raw, plan } = fixture();
  const save = overrides => writeFileSync(path, gzipSync(JSON.stringify({...raw, schedule:plan.schedule,
    hidden_frames:0, unfocused_frames:0, ...overrides})));
  try {
    writeFileSync(join(dir, 'feel-2026-09-18.jsonl'), JSON.stringify({trace:path, load1:12, provisional:true}));
    save({unfocused_frames:1});
    expect(reanalyze([path])[0]).toMatchObject({valid:false, provisional:true, load1:12, trace:path});
    save({});
    expect(reanalyze([path])[0].valid).toBe(true);
    const frames = [...raw.frames];
    for (let i = 0; i < frames.length; i += 8) if (frames[i + 1] === 12950) frames[i + 2] = .00001;
    save({frames});
    expect(reanalyze([path])[0]).toMatchObject({valid:false, latency:{valid_trials:19}});
    save({events:raw.events.slice(4)});
    expect(reanalyze([path])[0].error).toContain('Expected 50 delivered events');
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('callback generations discard resize rows but retain distinct same-paced callbacks',()=>{
  const {raw,plan}=fixture();
  const n=raw.frames.length/8;
  raw.exact_trace={};
  raw.raw_callback_ms=Array.from({length:n},(_,i)=>raw.frames[i*8]);
  raw.callback_generation=Array.from({length:n},(_,i)=>i+1);
  raw.drawn_clock_ms=[...raw.raw_callback_ms];
  const original=analyze(raw,plan).frames;
  // Insert a resize from callback 11, then callback 12 draws that same slot.
  const at=11;
  raw.frames.splice(at*8,0,...raw.frames.slice((at-1)*8,at*8));
  raw.raw_callback_ms.splice(at,0,raw.raw_callback_ms[at-1]);
  raw.callback_generation.splice(at,0,raw.callback_generation[at-1]);
  raw.drawn_clock_ms.splice(at,0,raw.drawn_clock_ms[at-1]);
  raw.drawn_clock_ms[at+1]=raw.drawn_clock_ms[at];
  expect(analyze(raw,plan).frames).toBe(original);
});
