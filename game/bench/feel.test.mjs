import { test, expect } from 'bun:test';
import { analyze, quantile, script, cli, consoleState, exactAdapter, prepareExact, runFeel, table } from './feel.mjs';
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
  raw.frames[8] = raw.frames[0];
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
  const names = ['exact', 'window', 'document', 'setTimeout'];
  const saved = names.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  let clicks = 0, reloads = 0;
  try {
    globalThis.window = globalThis;
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
  for (const hz of [60, 120]) {
    const adapter = await prepareExact(cli(['exact', '--no-build', '--hz', String(hz)]));
    expect(adapter.hz).toBe(hz);
    expect(adapter.build).toMatchObject({ source: '--no-build', freshness_checked: false });
    expect(adapter.build.artifact_sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(adapter.root).toEndWith(hz === 60 ? '/beacons/dist' : '/beacons/target/feel120');
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
