// Probe elapsed spans, charged once to the innermost phase. @ref LLP 1149.001 §3.
// bun bench/heavy-list/gpui/exclusive.mjs <probe.json.trace.json> [--rest] [--seconds=10] [--json]
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';

const sum = xs => xs.reduce((a, b) => a + b, 0);
const asyncNames = new Set(['gpu command', 'gpu present', 'handoff wait', 'presented']);
const quantile = (xs, p) => {
  const a = [...xs].sort((a, b) => a - b);
  return a.length ? a[Math.min(Math.floor(a.length * p), a.length - 1)] : null;
};
const stats = xs => ({ count: xs.length, mean: xs.length ? sum(xs) / xs.length : null,
  p50: quantile(xs, .5), p95: quantile(xs, .95), p99: quantile(xs, .99), max: xs.length ? Math.max(...xs) : null });

export function exclusive(events, start, end) {
  const rows = new Map(), edges = [], active = new Set();
  for (const [index, event] of events.entries()) {
    assert(Number.isFinite(event.ts) && Number.isFinite(event.dur) && event.dur >= 0,
      `invalid interval: ${event.name}`);
    const lo = Math.max(start, event.ts), hi = Math.min(end, event.ts + event.dur);
    if (hi <= lo) continue;
    const name = event.name;
    if (!rows.has(name)) rows.set(name, { phase: name, count: 0, exclusive_us: 0, inclusive_us: 0 });
    const row = rows.get(name);
    row.count++; row.inclusive_us += hi - lo;
    const span = { index, start: event.ts, end: event.ts + event.dur, row };
    edges.push({ time: lo, enter: true, span }, { time: hi, enter: false, span });
  }
  edges.sort((a, b) => a.time - b.time || Number(a.enter) - Number(b.enter));
  let previous = start;
  for (const edge of edges) {
    // Latest start is innermost. Equal starts use the earliest end, then file order.
    let inner;
    for (const span of active) if (!inner || span.start > inner.start
        || (span.start === inner.start && (span.end < inner.end
          || (span.end === inner.end && span.index < inner.index)))) inner = span;
    if (inner) inner.row.exclusive_us += edge.time - previous;
    if (edge.enter) active.add(edge.span); else active.delete(edge.span);
    previous = edge.time;
  }
  return [...rows.values()].map(row => ({ ...row,
    ms_per_s: row.exclusive_us * 1000 / (end - start),
    mean_exclusive_us: row.exclusive_us / row.count,
  })).sort((a, b) => b.exclusive_us - a.exclusive_us || a.phase.localeCompare(b.phase));
}

function verify() {
  const e = (name, ts, dur) => ({ name, ts, dur });
  const r = exclusive([e('outer', 0, 100), e('child', 10, 30), e('inner', 20, 5),
    e('child', 70, 40), e('mark', 80, 0)], 15, 90);
  assert.deepEqual(Object.fromEntries(r.map(x => [x.phase, x.exclusive_us])),
    { child: 40, outer: 30, inner: 5 });
  assert.equal(r.find(x => x.phase === 'child').count, 2);
  assert.equal(sum(r.map(x => x.exclusive_us)), 75);
  const overlap = exclusive([e('a', 0, 20), e('b', 10, 20)], 0, 30);
  assert.equal(sum(overlap.map(x => x.exclusive_us)), 30);
  assert.equal(overlap.find(x => x.phase === 'a').exclusive_us, 10);
  const tied = exclusive([e('a', 0, 20), e('b', 0, 10)], 0, 20);
  assert(tied.every(x => x.exclusive_us === 10));
  assert.deepEqual(exclusive([e('outside', 0, 10)], 10, 20), []);
  console.log('exclusive: nested, clipped, crossing, equal-start and empty windows passed');
}

function report(path, rest, duration) {
  const bytes = readFileSync(path), trace = JSON.parse(bytes);
  const base = path.replace(/\.trace\.json$/, '');
  const read = p => existsSync(p) ? JSON.parse(readFileSync(p, 'utf8')) : undefined;
  const raw = read(base), phase = rest ? 'rest' : 'fling';
  const phaseData = raw?.[phase] ?? read(`${base}.${phase}-data`);
  const start = phaseData?.start ?? read(`${base}.${phase}`);
  // Rejected historical runs retain endpoints even when their result has no phase.
  const end = phaseData?.end ?? (!rest ? read(`${base}.rest`) : undefined);
  assert(start && end && end.time > start.time, `${path}: missing ${phase} CPU/window endpoints`);
  const cpuSeconds = end.time - start.time;
  const seconds = duration ?? cpuSeconds;
  assert(seconds > 0 && seconds <= cpuSeconds, 'window must fit inside the measured phase');
  const lo = start.time * 1e6, hi = lo + seconds * 1e6;
  const events = trace.traceEvents.filter(e => e.ph === 'X');
  assert.equal(trace.dropped, 0, `${path}: dropped events invalidate attribution`);
  const inWindow = e => e.ts >= lo && e.ts < hi;
  const threads = [...new Set(events.filter(e => !asyncNames.has(e.name)).map(e => e.tid))]
    .map(tid => ({ tid, main: tid === trace.mainThread,
      rows: exclusive(events.filter(e => e.tid === tid && !asyncNames.has(e.name)), lo, hi) }))
    .filter(t => t.rows.length);
  const asynchronous = [...asyncNames].filter(n => n !== 'presented').map(phase => {
    const spans = events.filter(e => e.name === phase && e.dur > 0 && e.ts < hi && e.ts + e.dur > lo);
    const clipped = spans.map(e => (Math.min(hi, e.ts + e.dur) - Math.max(lo, e.ts)) / 1000);
    return { phase, ...stats(clipped), elapsed_ms_per_s: sum(clipped) / seconds };
  });
  const service = events.filter(e => ['carrier service', 'draw service'].includes(e.name) && inWindow(e));
  const causes = {};
  const serviceCause = new Map();
  for (const e of events.filter(e => ['carrier service', 'draw service'].includes(e.name))) {
    const mask = (e.args?.value ?? 0) - 1;
    const cause = e.args?.cause ?? (mask < 0 ? 'unmarked' : mask === 0 ? 'dispatch' :
      ['wheel', 'link', 'worker', 'timer', 'agent', 'window', 'draw']
        .filter((_, bit) => mask & (1 << bit)).join('+'));
    if (inWindow(e)) causes[cause] = (causes[cause] ?? 0) + 1;
    serviceCause.set(e, cause);
  }
  const causalElapsed = threads.flatMap(t => exclusive(events
    .filter(e => e.tid === t.tid && !asyncNames.has(e.name))
    .map(e => serviceCause.has(e) ? { ...e, name: `service cause: ${serviceCause.get(e)}` } : e), lo, hi))
    .filter(r => r.phase.startsWith('service cause: '));
  const callbacks = events.filter(e => e.name === 'display callback' && inWindow(e));
  const shown = events.filter(e => e.name === 'presented' && e.ts > 0).map(e => e.ts).sort((a, b) => a - b);
  const period = 1e6 / (raw?.screenHz ?? 120);
  const offsets = [];
  for (const callback of callbacks) {
    // Nearest observed refresh anchors the grid; gaps are whole refresh periods.
    let left = 0, right = shown.length;
    while (left < right) { const m = (left + right) >> 1; if (shown[m] < callback.ts) left = m + 1; else right = m; }
    const anchor = shown[left] ?? shown.at(-1);
    if (anchor !== undefined) offsets.push(((callback.ts - anchor) % period + period) % period / 1000);
  }
  const cpu = { seconds: cpuSeconds, process_ms_per_s: (end.cpu - start.cpu) * 1000 / cpuSeconds,
    main_ms_per_s: (end.main - start.main) * 1000 / cpuSeconds };
  const threadStarts = new Map((start.threads ?? []).map(t => [t.id, t]));
  const threadCpu = (end.threads ?? []).map(t => ({ id: t.id, port: t.port, name: t.name,
    present_at_start: threadStarts.has(t.id),
    cpu_ms_per_s: (t.cpu - (threadStarts.get(t.id)?.cpu ?? 0)) * 1000 / cpuSeconds,
  })).sort((a, b) => b.cpu_ms_per_s - a.cpu_ms_per_s);
  const anchor = shown[0] ?? lo;
  const firstPeriod = Math.ceil((lo - anchor) / period), lastPeriod = Math.floor((hi - anchor) / period);
  const passes = Array.from({ length: Math.max(0, lastPeriod - firstPeriod) }, () => 0);
  const passesByCause = Object.fromEntries(Object.keys(causes).map(c => [c, passes.map(() => 0)]));
  for (const e of service) {
    const index = Math.floor((e.ts - anchor) / period) - firstPeriod;
    if (index >= 0 && index < passes.length) {
      passes[index]++;
      passesByCause[serviceCause.get(e)][index]++;
    }
  }
  const histogram = {};
  for (const count of passes) histogram[count] = (histogram[count] ?? 0) + 1;
  const wakes = {};
  for (const e of events.filter(e => inWindow(e) && /wake|probe timer|probe display tick|window event/.test(e.name))) {
    wakes[e.name] = (wakes[e.name] ?? 0) + 1;
  }
  const wakeCallers = {};
  for (const e of events.filter(e => inWindow(e) && ['run loop wake request', 'run loop timer arm'].includes(e.name))) {
    const callers = wakeCallers[e.name] ??= {};
    const caller = e.args?.caller ?? 'unmarked';
    callers[caller] = (callers[caller] ?? 0) + 1;
  }
  const commands = new Set(events.filter(e => e.name === 'gpu command').map(e => `${e.ts}:${e.dur}`));
  const presents = events.filter(e => e.name === 'gpu present' && inWindow(e));
  const reread = { worker_reads: presents.length,
    identical_later_main_reads: presents.filter(e => commands.has(`${e.ts}:${e.dur}`)).length };
  const gaugeNames = new Set(['service_passes_in_period', 'targets_retained', 'target_bytes', 'target_partition_unix_us', 'target_partition_completed',
    'targets_incomplete', 'targets_reserved_this_frame', 'targets_idle', 'target_idle_bytes']);
  for (const name of Object.keys(end.counters?.values ?? {})) if (name.startsWith('period_')) gaugeNames.add(name);
  const counters = start.counters && end.counters ? {
    delta: Object.fromEntries(Object.entries(end.counters.values).filter(([n]) => !gaugeNames.has(n))
      .map(([n, v]) => [n, v == null || start.counters.values[n] == null ? null : v - start.counters.values[n]])),
    gauges: Object.fromEntries([...gaugeNames].map(n => [n, { start: start.counters.values[n],
      end: end.counters.values[n], process_high_water: end.counters.highWater[n] }])),
    displayed: end.counters.displayedFrames === null ? null : end.counters.displayedFrames - start.counters.displayedFrames,
    footprint_peak: phaseData?.peakSample ?? null,
  } : null;
  return { path, sha256: createHash('sha256').update(bytes).digest('hex'), phase,
    start_seconds: start.time, end_seconds: hi / 1e6, seconds, dropped: trace.dropped,
    accepted: raw?.accepted ?? null, cpu_only: raw?.cpuOnly ?? false, cpu, threadCpu, threads, asynchronous,
    service: { count: service.length, causes, exclusive_elapsed_by_cause: causalElapsed, callbacks: callbacks.length,
      per_callback: callbacks.length ? service.length / callbacks.length : null,
      complete_refresh_periods: { ...stats(passes), histogram,
        by_cause: Object.fromEntries(Object.entries(passesByCause).map(([c, counts]) => [c, stats(counts)])),
        grid: shown.length ? 'presented-time anchor' : 'window-start nominal grid' } },
    wakes, wake_callers: wakeCallers, gpu_timestamp_reread: reread, counters,
    display_phase_ms: { period: period / 1000, ...stats(offsets) },
    main_cpu_minus_marked_elapsed_ms_per_s: duration === undefined ? cpu.main_ms_per_s
      - sum(threads.find(t => t.main)?.rows.map(r => r.ms_per_s) ?? []) : null,
  };
}

function markdown(r) {
  console.log(`${r.path}\nSHA-256 ${r.sha256}\n${r.phase}: ${r.seconds.toFixed(6)} s; [${r.start_seconds}, ${r.end_seconds}); dropped ${r.dropped}; accepted ${r.accepted}`);
  console.log(`CPU over ${r.cpu.seconds.toFixed(6)} s: process ${r.cpu.process_ms_per_s.toFixed(3)} ms/s; main ${r.cpu.main_ms_per_s.toFixed(3)} ms/s.`);
  if (r.cpu_only) console.log('CPU-only readiness; no displayed-startup or cadence qualification.');
  if (r.threadCpu.length) {
    console.log('\nThread CPU endpoints (exited threads are absent):\n| ID | Name | CPU ms/s | Present at start |\n|---|---|---:|---|');
    for (const t of r.threadCpu) console.log(`| ${t.id} | ${t.name} | ${t.cpu_ms_per_s.toFixed(3)} | ${t.present_at_start} |`);
  }
  for (const t of r.threads) {
    console.log(`\nThread ${t.tid}${t.main ? ' (main)' : ''}: exclusive elapsed time, not CPU.\n| Phase | ms/s | Count | Mean exclusive µs |\n|---|---:|---:|---:|`);
    for (const row of t.rows) console.log(`| ${row.phase} | ${row.ms_per_s.toFixed(3)} | ${row.count} | ${row.mean_exclusive_us.toFixed(3)} |`);
  }
  console.log('\nAsynchronous intervals (may overlap CPU and each other):\n| Phase | Count | Total ms/s | Mean ms | p99 ms |\n|---|---:|---:|---:|---:|');
  for (const a of r.asynchronous) console.log(`| ${a.phase} | ${a.count} | ${a.elapsed_ms_per_s.toFixed(3)} | ${a.mean?.toFixed(3) ?? '—'} | ${a.p99?.toFixed(3) ?? '—'} |`);
  console.log(`\nService causes: ${JSON.stringify(r.service)}\nDisplay callback phase in refresh interval, ms: ${JSON.stringify(r.display_phase_ms)}`);
  console.log(`Wake marks: ${JSON.stringify(r.wakes)}\nGPU timestamps read after worker completion and again during main-thread retirement: ${JSON.stringify(r.gpu_timestamp_reread)}`);
  if (Object.keys(r.wake_callers).length) console.log(`Wake and timer request callers: ${JSON.stringify(r.wake_callers)}`);
  if (r.counters) {
    console.log('\nCumulative counter deltas over the phase:\n| Counter | Delta |\n|---|---:|');
    for (const [name, value] of Object.entries(r.counters.delta).sort()) console.log(`| ${name} | ${value} |`);
    console.log(`Displayed frame delta: ${r.counters.displayed ?? 'unavailable'}.`);
    console.log('\nGauges (high water includes startup):\n| Gauge | Start | End | Process high water |\n|---|---:|---:|---:|');
    for (const [name, v] of Object.entries(r.counters.gauges)) console.log(`| ${name} | ${v.start} | ${v.end} | ${v.process_high_water} |`);
  }
  if (r.main_cpu_minus_marked_elapsed_ms_per_s !== null) console.log(`Main CPU minus marked elapsed: ${r.main_cpu_minus_marked_elapsed_ms_per_s.toFixed(3)} ms/s (different instruments; not an attribution).`);
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.includes('--verify')) verify();
  else {
    assert(args[0] && args.slice(1).every(a => ['--json', '--rest'].includes(a) || /^--seconds=\d+(\.\d+)?$/.test(a)), 'expected trace path [--rest] [--seconds=10] [--json], or --verify');
    const duration = args.find(a => a.startsWith('--seconds='))?.split('=')[1];
    const result = report(args[0], args.includes('--rest'), duration === undefined ? undefined : Number(duration));
    if (args.includes('--json')) console.log(JSON.stringify(result, null, 2)); else markdown(result);
  }
}
